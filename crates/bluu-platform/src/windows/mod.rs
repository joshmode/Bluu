//! Windows implementation. Inventory calls are read-only and least-privilege.
//!
//! Key boundaries:
//! - WinRT device enumeration and cached SDP records are documented interfaces.
//! - Windows exposes no documented A2DP codec-registration API. One narrow,
//!   build-sensitive BthA2dp AAC preference is available through explicit consent.
//! - The negotiated codec is observed through an *undocumented event payload* from a
//!   documented ETW provider. That observation requires an elevated trace session;
//!   therefore the UI process stays unelevated and launches a tiny helper.
//! - The endpoint property store's `PKEY_AudioEngine_DeviceFormat` is readable without
//!   admin. Bluu intentionally does not write it.

mod audio;
mod bluetooth;
pub mod codec_toggle;
mod helper;
mod system;

use bluu_core::{
    backend::{BackendCapabilities, BackendDescriptor, BackendError, ConfigOperation},
    model::{
        CaptureOutcome, CaptureRequest, ProviderOperationOutcome, ProviderOperationRequest,
        Snapshot,
    },
    AudioBackend,
};

pub use system::system_info;

pub struct WindowsBackend {
    captured_codec: Option<bluu_core::model::ActiveCodecObservation>,
    last_snapshot: Option<Snapshot>,
}

impl WindowsBackend {
    pub fn new() -> Self {
        Self {
            captured_codec: None,
            last_snapshot: None,
        }
    }
}

impl Default for WindowsBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioBackend for WindowsBackend {
    fn descriptor(&self) -> BackendDescriptor {
        BackendDescriptor {
            id: "windows-native".into(),
            name: "Windows Bluetooth + Core Audio".into(),
            platform: "Windows".into(),
            read_only: true,
            demo: false,
        }
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            enumerate_bluetooth_devices: true,
            read_connection_state: true,
            read_remote_sdp_profiles: true,
            read_audio_endpoints: true,
            read_endpoint_formats: true,
            observe_active_codec: true,
            observe_codec_parameters: false,
            configure_preferred_codec: true,
            configure_codec_parameters: true,
            configure_endpoint_format: false,
            configure_inbox_codec_toggle: true,
            supported_operations: vec![
                ConfigOperation::SetPreferredCodec,
                ConfigOperation::SetCodecPriority,
                ConfigOperation::SetQualityPreference,
            ],
        }
    }

    fn scan(&mut self) -> Result<Snapshot, BackendError> {
        let mut snapshot = Snapshot::empty(system_info(false));
        snapshot.backend = self.descriptor();
        audio::scan(&mut snapshot)?;
        bluetooth::scan(&mut snapshot)?;
        snapshot.providers = crate::providers::discover();
        snapshot.provider_preferences = crate::providers::load_preferences();
        snapshot.inbox_codec =
            codec_toggle::read_toggle().unwrap_or_else(|e| bluu_core::model::InboxCodecToggle {
                supported: false,
                aac_enabled: None,
                registry_path: format!(r"HKLM\{}", codec_toggle::parameters_path()),
                note: e.to_string(),
            });
        if self.captured_codec.is_none() {
            self.captured_codec = crate::providers::load_observation();
        }
        // A previously observed codec is not automatically still active after a
        // scan, reconnect, or reboot. Keep it only if the selected device is
        // connected and the observation is younger than the configured capture
        // window plus a short grace period.
        snapshot.active_codec = self.captured_codec.clone().filter(|observation| {
            let age = chrono::Utc::now().signed_duration_since(observation.observed_at);
            let device_connected = observation
                .device_id
                .as_ref()
                .and_then(|id| snapshot.device(id))
                .is_some_and(|device| device.connected);
            age >= chrono::Duration::zero()
                && age <= chrono::Duration::seconds(180)
                && (observation.device_id.is_none() || device_connected)
        });
        self.last_snapshot = Some(snapshot.clone());
        Ok(snapshot)
    }

    fn capture_active_codec(
        &mut self,
        request: &CaptureRequest,
    ) -> Result<CaptureOutcome, BackendError> {
        let outcome = helper::capture(request)?;
        if let Some(observation) = outcome.observation.clone() {
            self.captured_codec = Some(observation);
        }
        Ok(outcome)
    }

    fn provider_operation(
        &mut self,
        request: &ProviderOperationRequest,
    ) -> Result<ProviderOperationOutcome, BackendError> {
        crate::providers::operation(request, self.last_snapshot.as_ref())
    }

    fn set_inbox_codec_toggle(
        &mut self,
        aac_enabled: bool,
        dry_run: bool,
    ) -> Result<bluu_core::model::InboxCodecChange, BackendError> {
        helper::set_inbox_codec(aac_enabled, dry_run)
    }

    fn restore_inbox_codec_toggle(
        &mut self,
    ) -> Result<bluu_core::model::InboxCodecChange, BackendError> {
        helper::restore_inbox_codec()
    }

    fn inbox_codec_toggle(&mut self) -> Result<bluu_core::model::InboxCodecToggle, BackendError> {
        codec_toggle::read_toggle()
    }
}
