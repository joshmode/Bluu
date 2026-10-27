//! Windows in-box A2DP codec preference toggle.
//!
//! Windows 11 ships both AAC and aptX Classic in the A2DP stack. On affected
//! builds, `BluetoothAacEnable=0` under the BthA2dp service parameters prevents
//! AAC from being offered and lets aptX win when both endpoints support it.
//!
//! This is an **undocumented, build-sensitive** Windows service parameter, not a
//! codec-registration API. It cannot add LDAC/aptX HD/Adaptive. Bluu therefore
//! keeps it in a separate, explicitly consented, backed-up, reversible path.

use bluu_core::{
    backend::BackendError,
    model::{InboxCodecChange, InboxCodecToggle},
};

use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
        System::Registry::{
            RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
            HKEY_LOCAL_MACHINE, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_DWORD, REG_SZ,
        },
    },
};

const PARAMETERS: &str = r"SYSTEM\CurrentControlSet\Services\BthA2dp\Parameters";
const VALUE: &str = "BluetoothAacEnable";
const BACKUP_VALUE: &str = "BluuPreviousBluetoothAacEnable";
const BACKUP_PRESENT: &str = "BluuPreviousBluetoothAacEnableWasPresent";

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn open(read: bool) -> Result<HKEY, BackendError> {
    let path = wide(PARAMETERS);
    let mut key = HKEY::default();
    let access = if read {
        KEY_QUERY_VALUE
    } else {
        KEY_QUERY_VALUE | KEY_SET_VALUE
    };
    let result = unsafe {
        RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(path.as_ptr()),
            None,
            access,
            &mut key,
        )
    };
    if result != ERROR_SUCCESS {
        return Err(BackendError::Platform(format!(
            "cannot open BthA2dp parameters (Windows error {}); this build or driver does not expose the preference key",
            result.0
        )));
    }
    Ok(key)
}

fn read_dword(key: HKEY, name: &str) -> Result<Option<u32>, BackendError> {
    let name = wide(name);
    let mut kind = REG_SZ;
    let mut size = std::mem::size_of::<u32>() as u32;
    let mut value = 0u32;
    let result = unsafe {
        RegQueryValueExW(
            key,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut kind),
            Some(&mut value as *mut u32 as *mut u8),
            Some(&mut size),
        )
    };
    if result == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if result != ERROR_SUCCESS {
        return Err(BackendError::Platform(format!(
            "cannot read {name:?} (Windows error {})",
            result.0
        )));
    }
    if kind != REG_DWORD || size != std::mem::size_of::<u32>() as u32 {
        return Err(BackendError::Platform(format!(
            "{name:?} has an unexpected registry type"
        )));
    }
    Ok(Some(value))
}

fn write_dword(key: HKEY, name: &str, value: u32) -> Result<(), BackendError> {
    let name = wide(name);
    let bytes = value.to_le_bytes();
    let result =
        unsafe { RegSetValueExW(key, PCWSTR(name.as_ptr()), None, REG_DWORD, Some(&bytes)) };
    if result != ERROR_SUCCESS {
        return Err(BackendError::Platform(format!(
            "cannot write {name:?} (Windows error {})",
            result.0
        )));
    }
    Ok(())
}

fn delete_value(key: HKEY, name: &str) -> Result<(), BackendError> {
    let name = wide(name);
    let result = unsafe { RegDeleteValueW(key, PCWSTR(name.as_ptr())) };
    if result != ERROR_SUCCESS && result != ERROR_FILE_NOT_FOUND {
        return Err(BackendError::Platform(format!(
            "cannot delete {name:?} (Windows error {})",
            result.0
        )));
    }
    Ok(())
}

pub fn read_toggle() -> Result<InboxCodecToggle, BackendError> {
    let key = open(true)?;
    let result = read_dword(key, VALUE);
    let _ = unsafe { RegCloseKey(key) };
    let value = result?;
    Ok(InboxCodecToggle {
        supported: true,
        aac_enabled: value.map(|v| v != 0),
        registry_path: format!(r"HKLM\{PARAMETERS}\{VALUE}"),
        note: if value.is_some() {
            "On affected Windows 11 builds, disabling AAC lets aptX Classic win when both endpoints support it. This is an undocumented BthA2dp service parameter, not a codec-registration API."
                .into()
        } else {
            "The BthA2dp service parameter is absent; Windows is using its default codec preference. Setting it is build/driver dependent."
                .into()
        },
    })
}

pub fn set_aac_enabled(aac_enabled: bool) -> Result<InboxCodecChange, BackendError> {
    let key = open(false)?;
    let result = (|| {
        let previous = read_dword(key, VALUE)?;
        let had_backup = read_dword(key, BACKUP_PRESENT)?.unwrap_or(0) != 0;
        if !had_backup {
            if let Some(previous) = previous {
                write_dword(key, BACKUP_VALUE, previous)?;
                write_dword(key, BACKUP_PRESENT, 1)?;
            } else {
                write_dword(key, BACKUP_PRESENT, 0)?;
            }
        }
        write_dword(key, VALUE, u32::from(aac_enabled))?;
        // Read back inside the same transaction so a silent write failure is visible.
        if read_dword(key, VALUE)? != Some(u32::from(aac_enabled)) {
            return Err(BackendError::Platform(
                "the registry accepted the write but readback did not match".into(),
            ));
        }
        Ok(InboxCodecChange {
            previous: previous.map(|v| v != 0),
            aac_enabled: Some(aac_enabled),
            applied: true,
            requires_reconnect: true,
            message: if aac_enabled {
                "AAC is enabled again. Disconnect and reconnect the headphones.".into()
            } else {
                "AAC is disabled for the BthA2dp stack; aptX can now be selected on aptX-capable hardware. Disconnect and reconnect the headphones. This affects the whole PC, not only one device."
                    .into()
            },
        })
    })();
    let _ = unsafe { RegCloseKey(key) };
    result
}

pub fn restore_toggle() -> Result<InboxCodecChange, BackendError> {
    let key = open(false)?;
    let result = (|| {
        let present = read_dword(key, BACKUP_PRESENT)?.unwrap_or(0) != 0;
        let backup = read_dword(key, BACKUP_VALUE)?;
        if present {
            if let Some(value) = backup {
                write_dword(key, VALUE, value)?;
            } else {
                delete_value(key, VALUE)?;
            }
        } else {
            delete_value(key, VALUE)?;
        }
        delete_value(key, BACKUP_VALUE)?;
        delete_value(key, BACKUP_PRESENT)?;
        Ok(InboxCodecChange {
            previous: None,
            aac_enabled: read_dword(key, VALUE)?.map(|v| v != 0),
            applied: true,
            requires_reconnect: true,
            message: "Restored the prior BthA2dp AAC preference. Reconnect the headphones.".into(),
        })
    })();
    let _ = unsafe { RegCloseKey(key) };
    result
}

pub fn preview_toggle(aac_enabled: bool) -> Result<InboxCodecChange, BackendError> {
    let key = open(true)?;
    let result = read_dword(key, VALUE);
    let _ = unsafe { RegCloseKey(key) };
    let previous = result?.map(|v| v != 0);
    Ok(InboxCodecChange {
        previous,
        aac_enabled: previous,
        applied: false,
        requires_reconnect: previous != Some(aac_enabled),
        message: format!(
            "Dry run: {} would be changed from {} to {} after explicit elevation. No registry value was written.",
            VALUE,
            previous
                .map(|v| if v { "enabled" } else { "disabled" })
                .unwrap_or("absent/default"),
            if aac_enabled { "enabled" } else { "disabled" }
        ),
    })
}

pub fn registry_value_name() -> &'static str {
    VALUE
}

pub fn backup_value_names() -> (&'static str, &'static str) {
    (BACKUP_VALUE, BACKUP_PRESENT)
}

pub fn parameters_path() -> &'static str {
    PARAMETERS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_names_are_exact_and_scoped() {
        assert_eq!(VALUE, "BluetoothAacEnable");
        assert_eq!(
            PARAMETERS,
            r"SYSTEM\CurrentControlSet\Services\BthA2dp\Parameters"
        );
        assert_eq!(std::mem::size_of::<HKEY>(), 8);
    }
}
