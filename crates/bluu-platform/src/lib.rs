//! Backend selection and platform implementations.
//!
//! `demo` is always available and is the only backend on non-Windows hosts.
//! The Windows backend uses documented WinRT/Core Audio APIs for inventory and a
//! separately elevated helper for A2DP ETW observation.

pub mod demo;
pub mod launch;
pub mod providers;

#[cfg(windows)]
pub mod windows;

pub mod screenshot;

use bluu_core::{model::SystemInfo, AudioBackend};

pub fn native_system_info() -> SystemInfo {
    #[cfg(windows)]
    {
        windows::system_info(false)
    }
    #[cfg(not(windows))]
    {
        SystemInfo {
            os_name: std::env::consts::OS.into(),
            os_version: "development host".into(),
            os_build: 0,
            architecture: std::env::consts::ARCH.into(),
            elevated: false,
            demo: false,
        }
    }
}

pub fn is_windows() -> bool {
    cfg!(windows)
}

/// Select the real backend on Windows and demo data elsewhere. A Windows user can
/// still explicitly request demo mode to inspect the UI without changing anything.
pub fn make_backend(demo_requested: bool) -> Box<dyn AudioBackend> {
    if demo_requested {
        return Box::new(demo::DemoBackend::new());
    }
    #[cfg(windows)]
    {
        Box::new(windows::WindowsBackend::new())
    }
    #[cfg(not(windows))]
    {
        Box::new(UnsupportedBackend)
    }
}

#[cfg(not(windows))]
struct UnsupportedBackend;

#[cfg(not(windows))]
impl AudioBackend for UnsupportedBackend {
    fn descriptor(&self) -> bluu_core::BackendDescriptor {
        bluu_core::BackendDescriptor {
            id: "unsupported".into(),
            name: "No native Bluetooth backend".into(),
            platform: std::env::consts::OS.into(),
            read_only: true,
            demo: false,
        }
    }

    fn capabilities(&self) -> bluu_core::BackendCapabilities {
        Default::default()
    }

    fn scan(&mut self) -> Result<bluu_core::Snapshot, bluu_core::backend::BackendError> {
        let mut snapshot = bluu_core::Snapshot::empty(native_system_info());
        snapshot.backend = self.descriptor();
        snapshot.providers = providers::discover();
        snapshot.provider_preferences = providers::load_preferences();
        snapshot
            .warnings
            .push("Native Bluetooth discovery is implemented for Windows 10/11. This build is running on a non-Windows host; use demo mode to inspect the interface.".into());
        Ok(snapshot)
    }

    fn capture_active_codec(
        &mut self,
        _request: &bluu_core::CaptureRequest,
    ) -> Result<bluu_core::CaptureOutcome, bluu_core::backend::BackendError> {
        Err(bluu_core::backend::BackendError::Unsupported(
            "active codec tracing is a Windows-only feature".into(),
        ))
    }

    fn provider_operation(
        &mut self,
        _request: &bluu_core::model::ProviderOperationRequest,
    ) -> Result<bluu_core::model::ProviderOperationOutcome, bluu_core::backend::BackendError> {
        Err(bluu_core::backend::BackendError::Unsupported(
            "advanced codec providers are Windows-only".into(),
        ))
    }

    fn inbox_codec_toggle(
        &mut self,
    ) -> Result<bluu_core::model::InboxCodecToggle, bluu_core::backend::BackendError> {
        Ok(bluu_core::model::InboxCodecToggle {
            supported: false,
            aac_enabled: None,
            registry_path: String::new(),
            note: "The AAC/aptX preference key is Windows-only.".into(),
        })
    }
}
