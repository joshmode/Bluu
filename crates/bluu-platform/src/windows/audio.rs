use std::ffi::c_void;

use bluu_core::{
    backend::BackendError,
    model::{AudioEndpoint, AudioFormat, EndpointState, Flow, Snapshot},
};
use windows::{
    core::{Interface, PWSTR},
    Win32::{
        Devices::Properties::DEVPKEY_Device_ContainerId,
        Devices::Properties::{DEVPKEY_Device_DeviceDesc, DEVPKEY_Device_FriendlyName},
        Media::Audio::{
            eAll, eCapture, eCommunications, eConsole, eRender, IAudioClient, IMMDevice,
            IMMDeviceEnumerator, IMMEndpoint, MMDeviceEnumerator, PKEY_AudioEndpoint_FormFactor,
            PKEY_AudioEngine_DeviceFormat, DEVICE_STATE, DEVICE_STATEMASK_ALL,
        },
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED,
            STGM_READ,
        },
        UI::Shell::PropertiesSystem::IPropertyStore,
    },
};

use super::helper::device_property;

fn take_pwstr(p: PWSTR) -> String {
    if p.is_null() {
        return String::new();
    }
    unsafe {
        let s = p.to_string().unwrap_or_default();
        CoTaskMemFree(Some(p.0 as *const c_void));
        s
    }
}

fn parse_wave_format(bytes: &[u8]) -> Option<AudioFormat> {
    if bytes.len() < 16 {
        return None;
    }
    let tag = u16::from_le_bytes([bytes[0], bytes[1]]);
    let channels = u16::from_le_bytes([bytes[2], bytes[3]]);
    let rate = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
    let bits = u16::from_le_bytes([bytes[14], bytes[15]]);
    let mut format = AudioFormat {
        sample_rate_hz: rate,
        channels,
        bits_per_sample: bits,
        valid_bits_per_sample: bits,
        float: tag == 3,
        channel_mask: 0,
    };
    if tag == 0xFFFE && bytes.len() >= 40 {
        format.valid_bits_per_sample = u16::from_le_bytes([bytes[18], bytes[19]]);
        format.channel_mask = u32::from_le_bytes(bytes[20..24].try_into().ok()?);
        let sub = u32::from_le_bytes(bytes[24..28].try_into().ok()?);
        format.float = sub == 3;
    }
    Some(format)
}

pub fn scan(snapshot: &mut Snapshot) -> Result<(), BackendError> {
    // The backend worker is a dedicated thread. Initialize COM/MTA once for its
    // lifetime; uninitializing between scans would invalidate the cached enumerator
    // and can disturb apartment-sensitive endpoint objects.
    COM_INIT.with(|initialized| {
        if !initialized.get() {
            let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            if hr.0 < 0 && hr != windows::Win32::Foundation::RPC_E_CHANGED_MODE {
                return Err(BackendError::AudioAccess(format!("CoInitializeEx: {hr:?}")));
            }
            initialized.set(true);
        }
        scan_inner(snapshot)
    })
}

fn scan_inner(snapshot: &mut Snapshot) -> Result<(), BackendError> {
    COM.with(|cell| {
        let mut guard = cell.borrow_mut();
        if guard.is_none() {
            let enumerator: IMMDeviceEnumerator = unsafe {
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                    .map_err(|e| BackendError::AudioAccess(format!("MMDeviceEnumerator: {e}")))?
            };
            *guard = Some(enumerator);
        }
        let Some(enumerator) = guard.as_ref() else {
            return Err(BackendError::AudioAccess(
                "Core Audio enumerator was not initialized".into(),
            ));
        };
        let defaults = defaults(enumerator);
        let collection = unsafe {
            enumerator
                .EnumAudioEndpoints(eAll, DEVICE_STATE(DEVICE_STATEMASK_ALL))
                .map_err(|e| BackendError::AudioAccess(format!("EnumAudioEndpoints: {e}")))?
        };
        let count = unsafe { collection.GetCount() }.unwrap_or(0);
        snapshot.endpoints.clear();
        for index in 0..count {
            let Ok(device) = (unsafe { collection.Item(index) }) else {
                continue;
            };
            if let Ok(endpoint) = read_endpoint(&device, &defaults) {
                snapshot.endpoints.push(endpoint);
            }
        }
        Ok(())
    })
}

thread_local! {
    static COM_INIT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static COM: std::cell::RefCell<Option<IMMDeviceEnumerator>> =
        const { std::cell::RefCell::new(None) };
}

fn defaults(enumerator: &IMMDeviceEnumerator) -> Vec<(String, bool, bool)> {
    let mut out = vec![];
    for (flow, role) in [(eRender, eConsole), (eRender, eCommunications)] {
        if let Ok(device) = unsafe { enumerator.GetDefaultAudioEndpoint(flow, role) } {
            if let Ok(id) = unsafe { device.GetId() } {
                let id = take_pwstr(id);
                let is_console = role == eConsole;
                if let Some(entry) = out.iter_mut().find(|(x, _, _)| *x == id) {
                    entry.1 |= is_console;
                    entry.2 |= !is_console;
                } else {
                    out.push((id, is_console, !is_console));
                }
            }
        }
    }
    out
}

fn read_endpoint(
    device: &IMMDevice,
    defaults: &[(String, bool, bool)],
) -> windows::core::Result<AudioEndpoint> {
    let id = take_pwstr(unsafe { device.GetId()? });
    let state = match unsafe { device.GetState()? }.0 & 0xF {
        1 => EndpointState::Active,
        2 => EndpointState::Disabled,
        4 => EndpointState::NotPresent,
        8 => EndpointState::Unplugged,
        _ => EndpointState::Unknown,
    };
    let flow = match device
        .cast::<IMMEndpoint>()
        .and_then(|e| unsafe { e.GetDataFlow() })
    {
        Ok(f) if f == eCapture => Flow::Capture,
        _ => Flow::Render,
    };
    let store = unsafe { device.OpenPropertyStore(STGM_READ)? };
    let friendly_name = property_string(&store, &property_key(&DEVPKEY_Device_FriendlyName))
        .unwrap_or_else(|| id.clone());
    let device_name = property_string(&store, &property_key(&DEVPKEY_Device_DeviceDesc));
    let form_factor = property_u32(&store, &PKEY_AudioEndpoint_FormFactor)
        .map(form_factor_name)
        .unwrap_or("Audio endpoint")
        .into();
    let container_id =
        property_guid(&store, &property_key(&DEVPKEY_Device_ContainerId)).map(|g| format!("{g:?}"));
    let shared_format =
        property_blob(&store, &PKEY_AudioEngine_DeviceFormat).and_then(|b| parse_wave_format(&b));
    let mix_format = if state == EndpointState::Active {
        unsafe {
            device
                .Activate::<IAudioClient>(CLSCTX_ALL, None)
                .and_then(|client| client.GetMixFormat())
                .ok()
                .and_then(|p| {
                    if p.is_null() {
                        return None;
                    }
                    let head = std::slice::from_raw_parts(p as *const u8, 18);
                    let extra = u16::from_le_bytes([head[16], head[17]]) as usize;
                    if extra > 4096 {
                        CoTaskMemFree(Some(p as *const c_void));
                        return None;
                    }
                    let all = std::slice::from_raw_parts(p as *const u8, 18 + extra).to_vec();
                    CoTaskMemFree(Some(p as *const c_void));
                    parse_wave_format(&all)
                })
        }
    } else {
        None
    };
    let engine_period = if state == EndpointState::Active {
        unsafe {
            device
                .Activate::<IAudioClient>(CLSCTX_ALL, None)
                .ok()
                .and_then(|client| {
                    let mut def = 0i64;
                    client
                        .GetDevicePeriod(Some(&mut def), None)
                        .ok()
                        .map(|_| def)
                })
        }
    } else {
        None
    };
    let (_, console, communications) = defaults
        .iter()
        .find(|(x, _, _)| *x == id)
        .map(|(_, console, communications)| (String::new(), *console, *communications))
        .unwrap_or((String::new(), false, false));
    Ok(AudioEndpoint {
        id,
        friendly_name,
        device_name,
        flow,
        state,
        form_factor,
        is_default_console: console,
        is_default_communications: communications,
        shared_mode_format: shared_format,
        mix_format,
        engine_period_100ns: engine_period,
        container_id,
        association: None,
    })
}

fn form_factor_name(value: u32) -> &'static str {
    match value {
        1 => "Speakers",
        2 => "Line level",
        3 => "Headphones",
        4 => "Microphone",
        5 => "Headset",
        6 => "Handset",
        7 => "Digital passthrough",
        8 => "SPDIF",
        9 => "HDMI",
        10 => "Audio endpoint",
        _ => "Audio endpoint",
    }
}

fn property_string(
    store: &IPropertyStore,
    key: &windows::Win32::Foundation::PROPERTYKEY,
) -> Option<String> {
    let mut value = device_property(store, key)?;
    unsafe {
        let p = windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc(&value)
            .ok()?;
        if p.is_null() {
            return None;
        }
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const c_void));
        // GetValue returns a PROPVARIANT whose contents we own; clear it even
        // though PropVariantToStringAlloc already produced a separate string.
        let _ = windows::Win32::System::Com::StructuredStorage::PropVariantClear(&mut value);
        s
    }
}

fn property_u32(
    store: &IPropertyStore,
    key: &windows::Win32::Foundation::PROPERTYKEY,
) -> Option<u32> {
    let mut value = device_property(store, key)?;
    let result =
        if unsafe { value.Anonymous.Anonymous.vt } == windows::Win32::System::Variant::VT_UI4 {
            Some(unsafe { value.Anonymous.Anonymous.Anonymous.ulVal })
        } else {
            None
        };
    unsafe {
        let _ = windows::Win32::System::Com::StructuredStorage::PropVariantClear(&mut value);
    }
    result
}

fn property_blob(
    store: &IPropertyStore,
    key: &windows::Win32::Foundation::PROPERTYKEY,
) -> Option<Vec<u8>> {
    let mut value = device_property(store, key)?;
    let vt = unsafe { value.Anonymous.Anonymous.vt };
    if vt != windows::Win32::System::Variant::VT_BLOB {
        unsafe {
            let _ = windows::Win32::System::Com::StructuredStorage::PropVariantClear(&mut value);
        }
        return None;
    }
    let blob = unsafe { value.Anonymous.Anonymous.Anonymous.blob };
    if blob.pBlobData.is_null() || blob.cbSize == 0 || blob.cbSize > 4096 {
        unsafe {
            let _ = windows::Win32::System::Com::StructuredStorage::PropVariantClear(&mut value);
        }
        return None;
    }
    let result =
        unsafe { std::slice::from_raw_parts(blob.pBlobData, blob.cbSize as usize) }.to_vec();
    unsafe {
        let _ = windows::Win32::System::Com::StructuredStorage::PropVariantClear(&mut value);
    }
    Some(result)
}

fn property_guid(
    store: &IPropertyStore,
    key: &windows::Win32::Foundation::PROPERTYKEY,
) -> Option<windows::core::GUID> {
    let mut value = device_property(store, key)?;
    if unsafe { value.Anonymous.Anonymous.vt } != windows::Win32::System::Variant::VT_CLSID {
        unsafe {
            let _ = windows::Win32::System::Com::StructuredStorage::PropVariantClear(&mut value);
        }
        return None;
    }
    let p = unsafe { value.Anonymous.Anonymous.Anonymous.puuid };
    if p.is_null() {
        unsafe {
            let _ = windows::Win32::System::Com::StructuredStorage::PropVariantClear(&mut value);
        }
        return None;
    }
    let result = unsafe { *p };
    unsafe {
        let _ = windows::Win32::System::Com::StructuredStorage::PropVariantClear(&mut value);
    }
    Some(result)
}

fn property_key(
    key: &windows::Win32::Foundation::DEVPROPKEY,
) -> windows::Win32::Foundation::PROPERTYKEY {
    windows::Win32::Foundation::PROPERTYKEY {
        fmtid: key.fmtid,
        pid: key.pid,
    }
}
