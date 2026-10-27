use std::fmt;

use serde::{Deserialize, Serialize};

use crate::model::{CaptureOutcome, CaptureRequest, Snapshot};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BackendDescriptor {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub read_only: bool,
    pub demo: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BackendCapabilities {
    pub enumerate_bluetooth_devices: bool,
    pub read_connection_state: bool,
    pub read_remote_sdp_profiles: bool,
    pub read_audio_endpoints: bool,
    pub read_endpoint_formats: bool,
    pub observe_active_codec: bool,
    pub observe_codec_parameters: bool,
    pub configure_preferred_codec: bool,
    pub configure_codec_parameters: bool,
    pub configure_endpoint_format: bool,
    pub supported_operations: Vec<ConfigOperation>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigOperation {
    SetPreferredCodec,
    SetCodecPriority,
    SetSampleRate,
    SetChannelMode,
    SetBitrate,
    SetQualityPreference,
    SetEndpointSharedModeFormat,
}

impl ConfigOperation {
    pub fn label(self) -> &'static str {
        match self {
            Self::SetPreferredCodec => "Preferred A2DP codec",
            Self::SetCodecPriority => "Codec priority",
            Self::SetSampleRate => "Codec sample rate",
            Self::SetChannelMode => "Codec channel mode",
            Self::SetBitrate => "Codec bitrate",
            Self::SetQualityPreference => "Codec quality preference",
            Self::SetEndpointSharedModeFormat => "Audio endpoint shared-mode format",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("this operation is not supported by the active backend: {0}")]
    Unsupported(String),
    #[error("Bluetooth device access failed: {0}")]
    DeviceAccess(String),
    #[error("audio endpoint access failed: {0}")]
    AudioAccess(String),
    #[error("the elevated codec trace helper could not be started: {0}")]
    HelperStart(String),
    #[error("codec tracing requires administrator approval: {0}")]
    ElevationRequired(String),
    #[error("the operation was cancelled")]
    Cancelled,
    #[error("platform failure: {0}")]
    Platform(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

impl BackendError {
    pub fn actionable(&self) -> &'static str {
        match self {
            Self::Unsupported(_) => {
                "Windows does not expose this control through a documented API. No system setting was changed."
            }
            Self::DeviceAccess(_) => {
                "Check that Bluetooth is on, the device is paired, and its audio profile is available."
            }
            Self::AudioAccess(_) => {
                "Open Windows Sound settings, confirm the Bluetooth endpoint exists, then refresh."
            }
            Self::HelperStart(_) => {
                "Close other trace tools, then retry. The helper is separate and exits after the capture."
            }
            Self::ElevationRequired(_) => {
                "Approve the one-time Windows elevation prompt only when you want to verify the active codec."
            }
            Self::Cancelled => "The operation was cancelled. No settings were changed.",
            Self::Platform(_) => "Export diagnostics and review the Windows compatibility notes.",
            Self::InvalidInput(_) => "Choose a connected Bluetooth audio device and try again.",
        }
    }
}

pub trait AudioBackend: Send {
    fn descriptor(&self) -> BackendDescriptor;
    fn capabilities(&self) -> BackendCapabilities;
    fn scan(&mut self) -> Result<Snapshot, BackendError>;
    fn capture_active_codec(
        &mut self,
        request: &CaptureRequest,
    ) -> Result<CaptureOutcome, BackendError>;
}

impl fmt::Display for ConfigOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}
