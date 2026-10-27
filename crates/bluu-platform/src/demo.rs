//! Clearly labelled demonstration data. It can inspect provider preferences and
//! exercise the provider workflow, but provider actions remain simulated.

use bluu_core::{
    backend::{BackendCapabilities, BackendDescriptor, BackendError},
    codec::{Confidence, Evidence},
    model::{
        ActiveCodecObservation, AdapterInfo, AssociationMethod, AudioEndpoint, AudioFormat,
        AudioProfile, BluetoothAddress, BluetoothAudioDevice, CaptureOutcome, CaptureRequest,
        EndpointAssociation, EndpointState, Flow, ProfileObservation, Snapshot, SystemInfo,
        Transport,
    },
    AudioBackend,
};
use chrono::Utc;

pub struct DemoBackend {
    system: SystemInfo,
    captured: Option<ActiveCodecObservation>,
}

impl DemoBackend {
    pub fn new() -> Self {
        let mut system = crate::native_system_info();
        system.demo = true;
        system.os_name = if cfg!(windows) {
            system.os_name
        } else {
            "Windows 11 (demo)".into()
        };
        system.os_version = "24H2 · demo dataset".into();
        system.os_build = 26_100;
        Self {
            system,
            captured: None,
        }
    }
}

impl Default for DemoBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioBackend for DemoBackend {
    fn descriptor(&self) -> BackendDescriptor {
        BackendDescriptor {
            id: "demo".into(),
            name: "Demo backend".into(),
            platform: "Simulated".into(),
            read_only: true,
            demo: true,
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
                bluu_core::ConfigOperation::SetPreferredCodec,
                bluu_core::ConfigOperation::SetCodecPriority,
                bluu_core::ConfigOperation::SetQualityPreference,
            ],
        }
    }

    fn scan(&mut self) -> Result<Snapshot, BackendError> {
        let mut snapshot = Snapshot::empty(self.system.clone());
        snapshot.backend = self.descriptor();
        snapshot.adapter = Some(AdapterInfo {
            id: Some("demo-adapter".into()),
            name: "Demo dual-mode radio".into(),
            manufacturer: Some("Demo radio vendor".into()),
            address: None,
            classic_supported: Some(true),
            low_energy_supported: Some(true),
            driver_provider: Some("Demo Bluetooth driver".into()),
            driver_version: Some("1.0.0".into()),
        });
        let render_id = "demo-endpoint-render".to_string();
        let capture_id = "demo-endpoint-capture".to_string();
        let association = EndpointAssociation {
            method: AssociationMethod::ContainerId,
            confidence: Confidence::High,
            note: "DEMO: endpoints share the device's simulated container ID.".into(),
        };
        snapshot.endpoints = vec![
            AudioEndpoint {
                id: render_id.clone(),
                friendly_name: "Studio Headphones · output".into(),
                device_name: Some("Studio Headphones".into()),
                flow: Flow::Render,
                state: EndpointState::Active,
                form_factor: "Headphones".into(),
                is_default_console: true,
                is_default_communications: false,
                shared_mode_format: Some(AudioFormat {
                    sample_rate_hz: 48_000,
                    channels: 2,
                    bits_per_sample: 32,
                    valid_bits_per_sample: 24,
                    float: true,
                    channel_mask: 3,
                }),
                mix_format: Some(AudioFormat {
                    sample_rate_hz: 48_000,
                    channels: 2,
                    bits_per_sample: 32,
                    valid_bits_per_sample: 24,
                    float: true,
                    channel_mask: 3,
                }),
                engine_period_100ns: Some(100_000),
                container_id: Some("demo-container-headphones".into()),
                association: Some(association.clone()),
            },
            AudioEndpoint {
                id: capture_id.clone(),
                friendly_name: "Studio Headphones · microphone".into(),
                device_name: Some("Studio Headphones".into()),
                flow: Flow::Capture,
                state: EndpointState::Unplugged,
                form_factor: "Microphone".into(),
                is_default_console: false,
                is_default_communications: false,
                shared_mode_format: Some(AudioFormat {
                    sample_rate_hz: 16_000,
                    channels: 1,
                    bits_per_sample: 16,
                    valid_bits_per_sample: 16,
                    float: false,
                    channel_mask: 4,
                }),
                mix_format: None,
                engine_period_100ns: None,
                container_id: Some("demo-container-headphones".into()),
                association: Some(association),
            },
        ];
        snapshot.devices = vec![
            BluetoothAudioDevice {
                id: "demo-headphones".into(),
                name: "Studio Headphones".into(),
                address: Some(BluetoothAddress([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF])),
                container_id: Some("demo-container-headphones".into()),
                connected: true,
                paired: true,
                transport: Transport::DualMode,
                class_of_device: Some(0x0024_0404),
                category: Some("Headphones".into()),
                profiles: vec![
                    ProfileObservation {
                        profile: AudioProfile::A2dpSink,
                        version: Some("1.4".into()),
                        evidence: Evidence::Observed,
                        note: "DEMO: cached SDP record.".into(),
                    },
                    ProfileObservation {
                        profile: AudioProfile::HandsFree,
                        version: Some("1.7".into()),
                        evidence: Evidence::Observed,
                        note: "DEMO: cached SDP record.".into(),
                    },
                    ProfileObservation {
                        profile: AudioProfile::Avrcp,
                        version: Some("1.6".into()),
                        evidence: Evidence::Observed,
                        note: "DEMO: cached SDP record.".into(),
                    },
                ],
                endpoint_ids: vec![render_id, capture_id],
                association: EndpointAssociation {
                    method: AssociationMethod::ContainerId,
                    confidence: Confidence::High,
                    note: "DEMO: endpoint and device share a container ID.".into(),
                },
                last_seen: Some(Utc::now()),
                backend_note: Some("Demo data. Nothing here was read from this PC.".into()),
            },
            BluetoothAudioDevice {
                id: "demo-speaker".into(),
                name: "Living Room Speaker".into(),
                address: Some(BluetoothAddress([0x11, 0x22, 0x33, 0x44, 0x55, 0x66])),
                container_id: Some("demo-container-speaker".into()),
                connected: false,
                paired: true,
                transport: Transport::Classic,
                class_of_device: Some(0x0024_0414),
                category: Some("Speaker".into()),
                profiles: vec![ProfileObservation {
                    profile: AudioProfile::A2dpSink,
                    version: Some("1.3".into()),
                    evidence: Evidence::Observed,
                    note: "DEMO: stale cached SDP record.".into(),
                }],
                endpoint_ids: vec![],
                association: EndpointAssociation::default(),
                last_seen: Some(Utc::now() - chrono::Duration::hours(19)),
                backend_note: Some("Demo data. Shows a paired but disconnected device.".into()),
            },
        ];
        snapshot.active_codec = self.captured.clone().map(|mut o| {
            o.device_association_confidence = Confidence::Medium;
            o
        });
        snapshot.notes.push(
            "Demo mode uses a fixed Windows 11 24H2 inventory. It never contacts a radio or reads a real device."
                .into(),
        );
        snapshot.notes.push(
            "All active-codec observations in demo mode are synthetic and must not be used as evidence."
                .into(),
        );
        snapshot.providers = demo_provider();
        snapshot.inbox_codec = bluu_core::model::InboxCodecToggle {
            supported: true,
            aac_enabled: Some(true),
            registry_path:
                r"HKLM\SYSTEM\CurrentControlSet\Services\BthA2dp\Parameters\BluetoothAacEnable"
                    .into(),
            note: "DEMO only. This does not read the real BthA2dp service parameter.".into(),
        };
        Ok(snapshot)
    }

    fn capture_active_codec(
        &mut self,
        request: &CaptureRequest,
    ) -> Result<CaptureOutcome, BackendError> {
        let outcome = crate::launch::demo_capture(request);
        self.captured = outcome.observation.clone();
        Ok(outcome)
    }

    fn provider_operation(
        &mut self,
        request: &bluu_core::model::ProviderOperationRequest,
    ) -> Result<bluu_core::model::ProviderOperationOutcome, bluu_core::backend::BackendError> {
        crate::providers::demo_operation(request)
    }
}

fn demo_provider() -> Vec<bluu_core::ProviderRecord> {
    serde_json::from_value(serde_json::json!({
        "manifest": {
            "api_version": 1,
            "id": "demo.ldac",
            "name": "Demo LDAC / aptX HD provider",
            "version": "0.1.0",
            "executable": "bluu-provider-demo.exe",
            "publisher": "Bluu test fixture",
            "homepage": "https://github.com/joshmode/Bluu",
            "description": "Protocol fixture only. Does not touch Bluetooth or audio.",
            "requires_elevation": false,
            "stack": "simulated",
            "capabilities": {
                "preferred_codec": true,
                "priority": true,
                "parameters": true,
                "active_codec": true,
                "rollback": true,
                "dry_run": true
            },
            "codecs": [
                {"codec": "ldac", "notes": "DEMO ONLY", "hardware_validated": true, "remote_validated": false, "parameters": [
                    {"kind": "choice", "key": "ldac_quality", "label": "LDAC quality",
                     "options": ["330", "660", "990"], "default": "990",
                     "note": "Fixture values; no transport exists."}
                ]},
                {"codec": "aptx_hd", "notes": "DEMO ONLY", "hardware_validated": true, "remote_validated": false, "parameters": []},
                {"codec": "aptx_adaptive", "notes": "DEMO ONLY", "hardware_validated": true, "remote_validated": false, "parameters": []},
                {"codec": "sbc", "notes": "Fallback", "hardware_validated": true, "remote_validated": true, "parameters": []}
            ]
        },
        "manifest_path": "demo/provider.json",
        "directory": "demo",
        "status": "ready",
        "issues": [],
        "executable_sha256": "demo-fixture",
        "simulated": true
    }))
    .unwrap_or_default()
}
