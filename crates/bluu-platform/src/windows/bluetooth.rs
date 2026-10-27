use std::collections::HashMap;

use bluu_core::{
    backend::BackendError,
    codec::Confidence,
    model::{
        AssociationMethod, BluetoothAddress, BluetoothAudioDevice, EndpointAssociation, Snapshot,
        Transport,
    },
    sdp::ServiceRecord,
};
use chrono::Utc;
use windows::{
    core::{Interface, HSTRING},
    Devices::{
        Bluetooth::{BluetoothCacheMode, BluetoothConnectionStatus, BluetoothDevice},
        Enumeration::{DeviceInformation, DeviceInformationKind},
    },
    Foundation::{IPropertyValue, PropertyType},
    Storage::Streams::DataReader,
    Win32::Devices::Bluetooth::{
        BluetoothFindDeviceClose, BluetoothFindFirstDevice, BluetoothFindNextDevice,
        BLUETOOTH_DEVICE_INFO, BLUETOOTH_DEVICE_SEARCH_PARAMS,
    },
};

pub fn scan(snapshot: &mut Snapshot) -> Result<(), BackendError> {
    let selector = BluetoothDevice::GetDeviceSelectorFromPairingState(true)
        .map_err(|e| BackendError::DeviceAccess(e.to_string()))?;
    let properties = windows_collections::IIterable::from(vec![
        HSTRING::from("System.Devices.ContainerId"),
        HSTRING::from("System.Devices.InterfaceClassGuid"),
        HSTRING::from("System.Devices.Aep.DeviceAddress"),
    ]);
    let collection =
        DeviceInformation::FindAllAsyncAqsFilterAndAdditionalProperties(&selector, &properties)
            .map_err(|e| BackendError::DeviceAccess(e.to_string()))?;
    let collection = wait(collection, 8_000)
        .ok_or_else(|| BackendError::DeviceAccess("Bluetooth enumeration timed out".into()))?
        .map_err(|e| BackendError::DeviceAccess(e.to_string()))?;
    let size = collection.Size().unwrap_or(0);
    let mut container_ids = HashMap::new();
    for index in 0..size {
        let info = match collection.GetAt(index) {
            Ok(info) => info,
            Err(_) => continue,
        };
        let id = info.Id().map(|v| v.to_string()).unwrap_or_default();
        if id.is_empty() {
            continue;
        }
        let device = match BluetoothDevice::FromIdAsync(&HSTRING::from(id.clone())) {
            Ok(op) => wait(op, 3_000).and_then(Result::ok),
            Err(_) => None,
        };
        let Some(device) = device else {
            continue;
        };
        if !is_audio_capable(&device) {
            let _ = device.Close();
            continue;
        }
        if let Ok((mut entry, container)) = read_device(&info, &device, &id) {
            if let Some(container) = container {
                container_ids.insert(id.clone(), container.clone());
                entry.container_id = Some(container);
            }
            snapshot.devices.push(entry);
        }
        // A connected BTHENUM audio devnode exposes the endpoint interface ID.
        // The documented association remains best-effort: we use the container
        // when available and name matching only as a labelled fallback.
        let _ = device.Close();
    }
    associate_endpoints(snapshot, &container_ids);
    mark_stale_endpoints(snapshot);
    read_adapter(snapshot);
    Ok(())
}

fn mark_stale_endpoints(snapshot: &mut Snapshot) {
    let connected: std::collections::HashSet<&str> = snapshot
        .devices
        .iter()
        .filter(|d| d.connected)
        .flat_map(|d| d.endpoint_ids.iter().map(String::as_str))
        .collect();
    for endpoint in &mut snapshot.endpoints {
        if !connected.contains(endpoint.id.as_str())
            && endpoint.container_id.is_some()
            && endpoint.association.is_some()
        {
            // An endpoint can legitimately be default while the link is in HFP. Do
            // not fabricate a disconnected state; only attach the observation note.
            endpoint.association = endpoint.association.take().map(|mut a| {
                a.note.push_str(" The Bluetooth link may have changed; refresh before relying on this association.");
                a
            });
        }
    }
}

fn is_audio_capable(device: &BluetoothDevice) -> bool {
    let class = device.ClassOfDevice().and_then(|c| c.RawValue()).ok();
    let Some(class) = class else {
        // Keep unknown devices: they may be LE Audio or expose audio through a
        // vendor interface. The UI labels this lack of evidence.
        return true;
    };
    let major = (class >> 8) & 0x1F;
    // Major device class 4 is audio/video. Some headsets incorrectly encode
    // peripheral/wearable classes, so retain devices with an audio service bit
    // or a headphone/headset minor class as well.
    let minor = (class >> 2) & 0x3F;
    let service = class >> 13;
    let audio_minor = matches!(minor, 0x01 | 0x02 | 0x06 | 0x07);
    major == 4 || audio_minor || service & 0x20 != 0
}

fn wait<T: windows::core::RuntimeType + 'static>(
    operation: windows_future::IAsyncOperation<T>,
    timeout_ms: u64,
) -> Option<windows::core::Result<T>> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    loop {
        match operation.Status() {
            Ok(status) if status != windows_future::AsyncStatus::Started => {
                return Some(operation.GetResults());
            }
            Err(e) => return Some(Err(e)),
            _ => {}
        }
        if std::time::Instant::now() > deadline {
            let _ = operation.Cancel();
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn read_device(
    info: &DeviceInformation,
    device: &BluetoothDevice,
    id: &str,
) -> windows::core::Result<(BluetoothAudioDevice, Option<String>)> {
    let connected = device
        .ConnectionStatus()
        .map(|s| s == BluetoothConnectionStatus::Connected)
        .unwrap_or(false);
    let address = device
        .BluetoothAddress()
        .ok()
        .map(BluetoothAddress::from_le_u64);
    let raw_class = device.ClassOfDevice().and_then(|c| c.RawValue()).ok();
    let profile_observations = cached_profiles(device, id);
    let profiles = profile_observations.profiles;
    let name = info
        .Name()
        .map(|v| v.to_string())
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| device.Name().ok().map(|v| v.to_string()))
        .unwrap_or_else(|| "Bluetooth device".into());
    let category = raw_class.map(category).map(str::to_string);
    let transport = if raw_class.is_some() {
        Transport::Classic
    } else {
        Transport::Unknown
    };
    let container = container_id(info, id);
    Ok((
        BluetoothAudioDevice {
            id: id.to_string(),
            name,
            address,
            container_id: container.clone(),
            connected,
            paired: true,
            transport,
            class_of_device: raw_class,
            category,
            profiles,
            endpoint_ids: vec![],
            association: EndpointAssociation::default(),
            last_seen: Some(Utc::now()),
            backend_note: profile_observations.note,
        },
        container,
    ))
}

fn container_id(info: &DeviceInformation, id: &str) -> Option<String> {
    if let Ok(properties) = info.Properties() {
        if let Ok(value) = properties.Lookup(&HSTRING::from("System.Devices.ContainerId")) {
            if let Ok(property) = value.cast::<IPropertyValue>() {
                if property.Type().unwrap_or(PropertyType::Empty) == PropertyType::Guid {
                    if let Ok(guid) = property.GetGuid() {
                        return Some(format!("{guid:?}"));
                    }
                }
            }
        }
    }
    if let Ok(op) = DeviceInformation::CreateFromIdAsyncWithKindAndAdditionalProperties(
        &HSTRING::from(id),
        &windows_collections::IIterable::from(vec![HSTRING::from("System.Devices.ContainerId")]),
        DeviceInformationKind::DeviceInterface,
    ) {
        if let Some(Ok(detailed)) = wait(op, 1_500) {
            if let Ok(properties) = detailed.Properties() {
                if let Ok(value) = properties.Lookup(&HSTRING::from("System.Devices.ContainerId")) {
                    if let Ok(property) = value.cast::<IPropertyValue>() {
                        if property.Type().unwrap_or(PropertyType::Empty) == PropertyType::Guid {
                            if let Ok(guid) = property.GetGuid() {
                                return Some(format!("{guid:?}"));
                            }
                        }
                    }
                }
            }
        }
    }
    let _ = info;
    None
}

struct ProfileRead {
    profiles: Vec<bluu_core::model::ProfileObservation>,
    note: Option<String>,
}

fn cached_profiles(device: &BluetoothDevice, _device_id: &str) -> ProfileRead {
    let Ok(records) = device.GetRfcommServicesWithCacheModeAsync(BluetoothCacheMode::Cached) else {
        return ProfileRead {
            profiles: vec![],
            note: Some(
                "Windows returned no cached RFCOMM service set; profile support is unknown.".into(),
            ),
        };
    };
    let Some(Ok(result)) = wait(records, 4_000) else {
        return ProfileRead {
            profiles: vec![],
            note: Some(
                "Reading cached RFCOMM services timed out; profile support is unknown.".into(),
            ),
        };
    };
    let Ok(services) = result.Services() else {
        return ProfileRead {
            profiles: vec![],
            note: Some(
                "Windows returned no RFCOMM service result; profile support is unknown.".into(),
            ),
        };
    };
    let mut observations = Vec::new();
    let mut parsed_any = false;
    for index in 0..services.Size().unwrap_or(0) {
        let Ok(service) = services.GetAt(index) else {
            continue;
        };
        let Ok(attributes) =
            service.GetSdpRawAttributesWithCacheModeAsync(BluetoothCacheMode::Cached)
        else {
            continue;
        };
        let Some(Ok(map)) = wait(attributes, 2_000) else {
            continue;
        };
        let iterator = match map.First() {
            Ok(iterator) => iterator,
            Err(_) => continue,
        };
        let mut attributes = Vec::new();
        while iterator.HasCurrent().unwrap_or(false) {
            if let Ok(pair) = iterator.Current() {
                let attribute_id = pair.Key().unwrap_or_default();
                if let Ok(buffer) = pair.Value() {
                    if let Ok(reader) = DataReader::FromBuffer(&buffer) {
                        let mut bytes =
                            vec![0u8; reader.UnconsumedBufferLength().unwrap_or(0) as usize];
                        if reader.ReadBytes(&mut bytes).is_ok() {
                            if let Some(element) = bluu_core::sdp::parse(&bytes) {
                                if let Ok(id) = u16::try_from(attribute_id) {
                                    attributes.push((id, element));
                                }
                            }
                        }
                    }
                }
            }
            if !iterator.MoveNext().unwrap_or(false) {
                break;
            }
        }
        if !attributes.is_empty() {
            parsed_any = true;
            observations.extend(ServiceRecord::from_attributes(attributes).observations());
        }
    }
    let profiles = dedupe_profiles(observations);
    let note = if services.Size().unwrap_or(0) == 0 {
        Some("No cached RFCOMM services were reported. This is common for LE-only endpoints and does not prove the absence of A2DP.".into())
    } else if !parsed_any {
        Some("RFCOMM service records were present but their raw SDP attributes could not be decoded with the documented data-element parser; profile support is unknown.".into())
    } else {
        None
    };
    ProfileRead { profiles, note }
}

fn dedupe_profiles(
    values: Vec<bluu_core::model::ProfileObservation>,
) -> Vec<bluu_core::model::ProfileObservation> {
    let mut seen = std::collections::HashSet::new();
    values
        .into_iter()
        .filter(|v| seen.insert(v.profile as u8))
        .collect()
}

fn associate_endpoints(snapshot: &mut Snapshot, container_ids: &HashMap<String, String>) {
    let mut by_container: HashMap<String, Vec<String>> = HashMap::new();
    let mut by_name: HashMap<String, Vec<String>> = HashMap::new();
    for endpoint in &snapshot.endpoints {
        if let Some(container) = &endpoint.container_id {
            by_container
                .entry(container.clone())
                .or_default()
                .push(endpoint.id.clone());
        }
        if let Some(name) = endpoint
            .device_name
            .as_ref()
            .or(Some(&endpoint.friendly_name))
        {
            by_name
                .entry(normalize(name))
                .or_default()
                .push(endpoint.id.clone());
        }
    }
    for device in &mut snapshot.devices {
        if let Some(container) = container_ids.get(&device.id) {
            let ids = by_container.get(container).cloned().unwrap_or_default();
            if !ids.is_empty() {
                device.endpoint_ids = ids.clone();
                device.association = EndpointAssociation {
                    method: AssociationMethod::ContainerId,
                    confidence: Confidence::High,
                    note: "Audio endpoints and the Bluetooth device share a Windows container ID."
                        .into(),
                };
                for id in ids {
                    if let Some(endpoint) = snapshot.endpoints.iter_mut().find(|e| e.id == id) {
                        endpoint.association = Some(device.association.clone());
                    }
                }
                continue;
            }
        }
        let normalized = normalize(&device.name);
        let ids = by_name.get(&normalized).cloned().unwrap_or_default();
        if !ids.is_empty() {
            device.endpoint_ids = ids.clone();
            device.association = EndpointAssociation {
                method: AssociationMethod::FriendlyNameHeuristic,
                confidence: Confidence::Low,
                note: "Endpoint association is name-based because Windows exposes no documented Bluetooth-address-to-audio-endpoint API. Verify in Windows Sound settings.".into(),
            };
            for id in ids {
                if let Some(endpoint) = snapshot.endpoints.iter_mut().find(|e| e.id == id) {
                    endpoint.association = Some(device.association.clone());
                }
            }
        }
    }
}

fn normalize(value: &str) -> String {
    value
        .to_lowercase()
        .replace(" hands-free", "")
        .replace(" stereo", "")
        .replace(" output", "")
        .replace(" input", "")
        .trim()
        .to_string()
}

fn category(class: u32) -> &'static str {
    match (class >> 8) & 0x1F {
        1 => "Computer",
        2 => "Phone",
        4 => "Audio/video",
        5 => "Peripheral",
        6 => "Imaging",
        7 => "Wearable",
        _ => "Bluetooth device",
    }
}

fn read_adapter(snapshot: &mut Snapshot) {
    let Ok(op) = windows::Devices::Bluetooth::BluetoothAdapter::GetDefaultAsync() else {
        return;
    };
    let Some(Ok(adapter)) = wait(op, 4_000) else {
        return;
    };
    snapshot.adapter = Some(bluu_core::model::AdapterInfo {
        id: adapter.DeviceId().map(|v| v.to_string()).ok(),
        name: "Default Bluetooth adapter".into(),
        manufacturer: None,
        address: adapter
            .BluetoothAddress()
            .ok()
            .map(BluetoothAddress::from_le_u64),
        classic_supported: adapter.IsClassicSupported().ok(),
        low_energy_supported: adapter.IsLowEnergySupported().ok(),
        driver_provider: None,
        driver_version: None,
    });
    snapshot.notes.push(
        "Radio driver provider/version are not exposed by the documented WinRT BluetoothAdapter API."
            .into(),
    );
}

#[allow(dead_code)]
fn legacy_find_devices() -> Vec<BLUETOOTH_DEVICE_INFO> {
    // Retained as a documented fallback for Classic-only discovery. Kept read-only.
    let mut out = Vec::new();
    unsafe {
        let params = BLUETOOTH_DEVICE_SEARCH_PARAMS {
            dwSize: std::mem::size_of::<BLUETOOTH_DEVICE_SEARCH_PARAMS>() as u32,
            fReturnAuthenticated: true.into(),
            fReturnRemembered: true.into(),
            fReturnConnected: true.into(),
            fIssueInquiry: false.into(),
            cTimeoutMultiplier: 0,
            hRadio: windows::Win32::Foundation::HANDLE::default(),
            fReturnUnknown: false.into(),
        };
        let mut info = BLUETOOTH_DEVICE_INFO {
            dwSize: std::mem::size_of::<BLUETOOTH_DEVICE_INFO>() as u32,
            ..Default::default()
        };
        let Ok(find) = BluetoothFindFirstDevice(&params, &mut info) else {
            return out;
        };
        loop {
            out.push(info);
            info.dwSize = std::mem::size_of::<BLUETOOTH_DEVICE_INFO>() as u32;
            if BluetoothFindNextDevice(find, &mut info).is_err() {
                break;
            }
        }
        let _ = BluetoothFindDeviceClose(find);
    }
    out
}
