use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::codec::{CodecId, Confidence, Evidence, VerificationMethod};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BluetoothAddress(pub [u8; 6]);

impl BluetoothAddress {
    pub fn from_le_u64(value: u64) -> Self {
        let b = value.to_le_bytes();
        Self([b[0], b[1], b[2], b[3], b[4], b[5]])
    }

    pub fn to_le_u64(self) -> u64 {
        u64::from_le_bytes([
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5], 0, 0,
        ])
    }

    pub fn display_masked(&self) -> String {
        let b = self.0;
        format!("XX:XX:XX:XX:{:02X}:{:02X}", b[4], b[5])
    }

    pub fn display_hex(&self) -> String {
        let b = self.0;
        format!(
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            b[5], b[4], b[3], b[2], b[1], b[0]
        )
    }
}

impl fmt::Display for BluetoothAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display_masked())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os_name: String,
    pub os_version: String,
    pub os_build: u32,
    pub architecture: String,
    pub elevated: bool,
    pub demo: bool,
}

impl SystemInfo {
    pub fn is_windows_11(&self) -> bool {
        self.os_build >= 22_000
    }

    pub fn is_windows_10(&self) -> bool {
        self.os_build >= 10_240 && self.os_build < 22_000
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WindowsPlatformInfo {
    pub product_name: Option<String>,
    pub display_version: Option<String>,
    pub ubr: Option<u32>,
    pub isolation_64_bit: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdapterInfo {
    pub id: Option<String>,
    pub name: String,
    pub manufacturer: Option<String>,
    pub address: Option<BluetoothAddress>,
    pub classic_supported: Option<bool>,
    pub low_energy_supported: Option<bool>,
    pub driver_provider: Option<String>,
    pub driver_version: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    Classic,
    LowEnergy,
    DualMode,
    Unknown,
}

impl Transport {
    pub fn label(self) -> &'static str {
        match self {
            Self::Classic => "Bluetooth Classic",
            Self::LowEnergy => "Bluetooth LE",
            Self::DualMode => "Dual-mode Bluetooth",
            Self::Unknown => "Bluetooth (transport not exposed)",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioProfile {
    A2dpSink,
    A2dpSource,
    HandsFree,
    Headset,
    Avrcp,
    LeAudio,
    Unknown,
}

impl AudioProfile {
    pub fn label(self) -> &'static str {
        match self {
            Self::A2dpSink => "A2DP audio sink",
            Self::A2dpSource => "A2DP audio source",
            Self::HandsFree => "Hands-Free (HFP)",
            Self::Headset => "Headset (HSP)",
            Self::Avrcp => "Audio/video remote control",
            Self::LeAudio => "LE Audio",
            Self::Unknown => "Unknown audio profile",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileObservation {
    pub profile: AudioProfile,
    pub version: Option<String>,
    pub evidence: Evidence,
    pub note: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssociationMethod {
    ContainerId,
    DeviceInterface,
    FriendlyNameHeuristic,
    None,
}

impl AssociationMethod {
    pub fn label(self) -> &'static str {
        match self {
            Self::ContainerId => "Windows container ID",
            Self::DeviceInterface => "device interface path",
            Self::FriendlyNameHeuristic => "device name (heuristic)",
            Self::None => "not associated",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EndpointAssociation {
    pub method: AssociationMethod,
    pub confidence: Confidence,
    pub note: String,
}

impl Default for EndpointAssociation {
    fn default() -> Self {
        Self {
            method: AssociationMethod::None,
            confidence: Confidence::Low,
            note: "No audio endpoint was associated with this device.".into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BluetoothAudioDevice {
    pub id: String,
    pub name: String,
    pub address: Option<BluetoothAddress>,
    /// Windows device container shared with the audio endpoint, when exposed.
    /// This is used for association, never for codec capability inference.
    pub container_id: Option<String>,
    pub connected: bool,
    pub paired: bool,
    pub transport: Transport,
    pub class_of_device: Option<u32>,
    pub category: Option<String>,
    pub profiles: Vec<ProfileObservation>,
    pub endpoint_ids: Vec<String>,
    pub association: EndpointAssociation,
    pub last_seen: Option<DateTime<Utc>>,
    pub backend_note: Option<String>,
}

impl BluetoothAudioDevice {
    pub fn supports_a2dp(&self) -> Option<bool> {
        let has = self
            .profiles
            .iter()
            .any(|p| matches!(p.profile, AudioProfile::A2dpSink | AudioProfile::A2dpSource));
        if self.profiles.is_empty() {
            None
        } else {
            Some(has)
        }
    }

    pub fn supports_hfp(&self) -> bool {
        self.profiles
            .iter()
            .any(|p| matches!(p.profile, AudioProfile::HandsFree | AudioProfile::Headset))
    }

    /// True when cached SDP profile evidence was actually decoded. An empty
    /// vector means “unknown”, not “no profiles”.
    pub fn profile_evidence_available(&self) -> bool {
        !self.profiles.is_empty()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AudioFormat {
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub valid_bits_per_sample: u16,
    pub float: bool,
    pub channel_mask: u32,
}

impl AudioFormat {
    pub fn label(&self) -> String {
        format!(
            "{:.1} kHz · {} ch · {} bit{}",
            self.sample_rate_hz as f32 / 1000.0,
            self.channels,
            self.valid_bits_per_sample,
            if self.float { " float" } else { "" }
        )
    }

    pub fn pcm_kbps(&self) -> u32 {
        (self.sample_rate_hz as u64 * self.channels as u64 * self.bits_per_sample as u64 / 1000)
            as u32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointState {
    Active,
    Disabled,
    NotPresent,
    Unplugged,
    Unknown,
}

impl EndpointState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Disabled => "Disabled",
            Self::NotPresent => "Not present",
            Self::Unplugged => "Unplugged",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Flow {
    Render,
    Capture,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioEndpoint {
    pub id: String,
    pub friendly_name: String,
    pub device_name: Option<String>,
    pub flow: Flow,
    pub state: EndpointState,
    pub form_factor: String,
    pub is_default_console: bool,
    pub is_default_communications: bool,
    pub shared_mode_format: Option<AudioFormat>,
    pub mix_format: Option<AudioFormat>,
    pub engine_period_100ns: Option<i64>,
    pub container_id: Option<String>,
    pub association: Option<EndpointAssociation>,
}

impl AudioEndpoint {
    pub fn default_for(&self) -> Option<&'static str> {
        match (self.is_default_console, self.is_default_communications) {
            (true, true) => Some("media and communications"),
            (true, false) => Some("media"),
            (false, true) => Some("communications"),
            (false, false) => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActiveCodecObservation {
    pub codec: CodecId,
    pub vendor_id: Option<u32>,
    pub vendor_codec_id: Option<u16>,
    pub observed_at: DateTime<Utc>,
    pub method: VerificationMethod,
    pub device_id: Option<String>,
    pub device_association_confidence: Confidence,
    pub note: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub generated_at: DateTime<Utc>,
    pub system: SystemInfo,
    pub backend: crate::backend::BackendDescriptor,
    pub adapter: Option<AdapterInfo>,
    pub devices: Vec<BluetoothAudioDevice>,
    pub endpoints: Vec<AudioEndpoint>,
    pub active_codec: Option<ActiveCodecObservation>,
    pub warnings: Vec<String>,
    pub notes: Vec<String>,
    #[serde(default)]
    pub platform: Option<WindowsPlatformInfo>,
    #[serde(default)]
    pub providers: Vec<crate::provider::ProviderRecord>,
    #[serde(default)]
    pub provider_preferences: Vec<crate::provider::ProviderPreference>,
}

impl Snapshot {
    pub fn empty(system: SystemInfo) -> Self {
        Self {
            generated_at: Utc::now(),
            system,
            backend: crate::backend::BackendDescriptor::default(),
            adapter: None,
            devices: Vec::new(),
            endpoints: Vec::new(),
            active_codec: None,
            warnings: Vec::new(),
            notes: Vec::new(),
            platform: None,
            providers: Vec::new(),
            provider_preferences: Vec::new(),
        }
    }

    pub fn connected_devices(&self) -> impl Iterator<Item = &BluetoothAudioDevice> {
        self.devices.iter().filter(|d| d.connected)
    }

    pub fn device(&self, id: &str) -> Option<&BluetoothAudioDevice> {
        self.devices.iter().find(|d| d.id == id)
    }

    pub fn endpoint(&self, id: &str) -> Option<&AudioEndpoint> {
        self.endpoints.iter().find(|e| e.id == id)
    }

    pub fn selected_device(&self, id: Option<&str>) -> Option<&BluetoothAudioDevice> {
        id.and_then(|id| self.device(id))
            .or_else(|| self.connected_devices().next())
            .or_else(|| self.devices.first())
    }

    pub fn selected_render_endpoint(
        &self,
        device: Option<&BluetoothAudioDevice>,
    ) -> Option<&AudioEndpoint> {
        let device = device?;
        device
            .endpoint_ids
            .iter()
            .filter_map(|id| self.endpoint(id))
            .find(|e| e.flow == Flow::Render)
    }

    pub fn selected_capture_endpoint(
        &self,
        device: Option<&BluetoothAudioDevice>,
    ) -> Option<&AudioEndpoint> {
        let device = device?;
        device
            .endpoint_ids
            .iter()
            .filter_map(|id| self.endpoint(id))
            .find(|e| e.flow == Flow::Capture)
    }

    pub fn active_observation_for(&self, device_id: &str) -> Option<&ActiveCodecObservation> {
        self.active_codec
            .as_ref()
            .filter(|o| o.device_id.as_deref() == Some(device_id))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaptureRequest {
    pub device_id: Option<String>,
    pub duration_seconds: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaptureOutcome {
    pub observation: Option<ActiveCodecObservation>,
    pub captured_events: u32,
    pub session_started: bool,
    pub note: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderOperationRequest {
    pub provider_id: String,
    pub operation: crate::provider::ProviderAction,
    pub device_id: Option<String>,
    pub preference: Option<crate::provider::ProviderPreference>,
    pub rollback_token: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderOperationOutcome {
    pub provider_id: String,
    pub operation: crate::provider::ProviderAction,
    pub accepted: bool,
    pub message: String,
    pub requires_reconnect: bool,
    pub rollback_token: Option<String>,
    pub observation: Option<ActiveCodecObservation>,
    pub applied_preference: Option<crate::provider::ProviderPreference>,
}
