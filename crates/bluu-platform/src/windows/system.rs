use bluu_core::model::SystemInfo;
use windows::Win32::System::SystemInformation::{GetVersionExW, OSVERSIONINFOEXW};

pub fn system_info(demo: bool) -> SystemInfo {
    let version = version();
    let elevated = crate::windows::helper::is_elevated();
    SystemInfo {
        os_name: if version.2 >= 22_000 {
            "Windows 11".into()
        } else if version.2 >= 10_240 {
            "Windows 10".into()
        } else if cfg!(windows) {
            "Windows".into()
        } else {
            std::env::consts::OS.into()
        },
        os_version: format!("{}.{}.{}", version.0, version.1, version.2),
        os_build: version.2,
        architecture: std::env::consts::ARCH.into(),
        elevated,
        demo,
    }
}

fn version() -> (u32, u32, u32) {
    unsafe {
        // `GetVersionExW` is documented but can lie without a compatibility manifest.
        // Bluu uses it only as a fallback display value. Codec decisions are based on
        // documented version thresholds and can be overridden by a diagnostic note.
        let mut info = OSVERSIONINFOEXW {
            dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOEXW>() as u32,
            ..Default::default()
        };
        let _ = GetVersionExW((&mut info as *mut OSVERSIONINFOEXW).cast());
        (info.dwMajorVersion, info.dwMinorVersion, info.dwBuildNumber)
    }
}
