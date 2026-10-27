use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    codec::{
        CodecFact, CodecId, CodecReport, CodecState, Confidence, Evidence, VerificationMethod,
    },
    feasibility::{codec_inventory, windows_codec_support, WindowsCodecSupport},
    model::{BluetoothAudioDevice, Snapshot},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Attention,
    Limitation,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiagnosticFinding {
    pub severity: Severity,
    pub title: String,
    pub detail: String,
    pub action: String,
    pub evidence: Evidence,
}

pub fn codec_reports(
    snapshot: &Snapshot,
    device: Option<&BluetoothAudioDevice>,
) -> Vec<CodecReport> {
    CodecId::ALL
        .into_iter()
        .map(|codec| codec_report(codec, snapshot, device))
        .collect()
}

fn codec_report(
    codec: CodecId,
    snapshot: &Snapshot,
    device: Option<&BluetoothAudioDevice>,
) -> CodecReport {
    let support = windows_codec_support(codec, &snapshot.system);
    let native_inventory = codec_inventory(&snapshot.system);
    let observation = snapshot.active_codec.as_ref().filter(|o| {
        // A trace event without a device identifier is a system-level observation.
        // It must remain visible, but its state is ActiveUnverified rather than
        // attributed to the selected device.
        o.device_id.is_none()
            || device
                .map(|d| o.device_id.as_deref() == Some(d.id.as_str()))
                .unwrap_or(true)
    });
    let observed_for_codec = observation.filter(|o| o.codec == codec);
    let connected = device.map(|d| d.connected).unwrap_or(false);
    let a2dp = device.and_then(BluetoothAudioDevice::supports_a2dp);
    let provider_ids: Vec<String> = snapshot
        .providers
        .iter()
        .filter(|p| p.is_usable() && p.supports(codec))
        .map(|p| p.manifest.id.clone())
        .collect();
    let provider_selectable = !provider_ids.is_empty();
    let provider_preference = device.and_then(|d| {
        snapshot.provider_preferences.iter().find(|pref| {
            pref.device_id == d.id
                && provider_ids.iter().any(|id| id == &pref.provider_id)
                && pref.ordered_codecs.contains(&codec)
        })
    });
    let hfp_active = device.is_some_and(|d| {
        snapshot
            .selected_capture_endpoint(Some(d))
            .is_some_and(|e| e.state == crate::model::EndpointState::Active)
    });

    let (state, summary, limitations) = if codec == CodecId::Other {
        (
            CodecState::Unknown,
            "An unrecognised codec tuple was observed or listed. Raw IDs are preserved; Bluu does not guess a marketing name.".into(),
            vec![
                "Unknown codec tuples require a protocol trace or vendor documentation to identify.".into(),
            ],
        )
    } else if codec == CodecId::Lc3 {
        (
            CodecState::HardwareOnly,
            "LC3 belongs to Bluetooth LE Audio, a separate transport/profile path. A2DP tracing cannot verify it.".into(),
            vec![
                "Windows LE Audio uses vendor ACX/VSAP paths; this app does not register codecs.".into(),
            ],
        )
    } else if let Some(obs) = observed_for_codec {
        let provider_attested = obs.device_id.is_some()
            && obs.device_association_confidence == Confidence::High
            && obs.method == VerificationMethod::ProviderReport;
        let independently_verified = obs.device_id.is_some()
            && obs.device_association_confidence == Confidence::High
            && matches!(
                obs.method,
                VerificationMethod::UserProvidedTrace | VerificationMethod::ManualExternalAnalyzer
            );
        (
            if independently_verified {
                CodecState::ActiveVerified
            } else if provider_attested {
                CodecState::ActiveProviderAttested
            } else {
                CodecState::ActiveUnverified
            },
            if independently_verified {
                format!(
                    "A2DP negotiation observed and tied to this device by {}.",
                    obs.method.label()
                )
            } else if provider_attested {
                format!(
                    "The advanced provider attests that {} is active on this device. This is provider evidence, not an independent trace.",
                    codec.label()
                )
            } else {
                format!(
                    "An A2DP stream using {} was observed, but device-level association is {}.",
                    codec.label(),
                    obs.device_association_confidence.label()
                )
            },
            {
                let mut limits = vec![
                    "Observed negotiation does not measure perceived quality.".into(),
                    "Windows does not expose codec bitrate or bitpool through a stable public API."
                        .into(),
                ];
                if provider_ids.is_empty() {
                    limits.push(
                        "No installed advanced provider advertises this codec for configuration."
                            .into(),
                    );
                }
                limits
            },
        )
    } else if provider_preference.is_some() {
        (
            CodecState::ProviderPreferenceApplied,
            format!(
                "A preference including {} was applied through an advanced provider. Negotiation is not verified until the provider reports an active codec or an independent trace confirms it.",
                codec.label()
            ),
            vec![
                "Provider preference is not proof of active transmission.".into(),
                "The provider owns driver, transport, and codec licensing.".into(),
            ],
        )
    } else if provider_selectable {
        (
            CodecState::ProviderAvailable,
            format!(
                "{} is advertised by an installed advanced codec provider. That is provider capability, not proof of local or remote hardware support; a preference application does not by itself verify transmission.",
                codec.label()
            ),
            vec![
                "This is a provider capability, not an in-box Windows capability.".into(),
                "The provider must validate the radio and remote device before claiming success.".into(),
            ],
        )
    } else if native_inventory.contains(&codec) {
        (
            CodecState::SupportedAvailable,
            format!(
                "{} is part of this Windows build's documented A2DP codec inventory. It is not necessarily active.",
                codec.label()
            ),
            vec![
                "Windows selects codecs automatically; no public preference API is documented.".into(),
                "Remote AVDTP codec capabilities are not exposed to desktop applications.".into(),
            ],
        )
    } else if matches!(support, WindowsCodecSupport::ConditionalQualcommRadio) {
        (
            CodecState::Unknown,
            format!(
                "{} may be available on select hardware with a compatible Qualcomm radio and integrated stack. This PC's radio has not been verified.",
                codec.label()
            ),
            vec![
                "Generic Windows PCs cannot enable this through configuration.".into(),
                "The local radio and OEM integration must be verified before claiming support.".into(),
            ],
        )
    } else {
        (
            CodecState::UnavailableOnBackend,
            format!(
                "{} is not provided by this Windows A2DP stack. A codec library alone cannot register it.",
                codec.label()
            ),
            vec![
                "Requires a vendor stack, replacement A2DP driver, or external audio path.".into(),
                "No codec-registration registry toggle is used by Bluu; the separate AAC/aptX preference cannot add this codec.".into(),
            ],
        )
    };

    let remote_note = if a2dp == Some(false) {
        "The cached SDP records do not show an A2DP service."
    } else if a2dp == Some(true) {
        "The device advertises A2DP, but Windows does not expose its AVDTP codec capability list."
    } else {
        "The device's A2DP capability could not be determined from the available records."
    };

    let capability = if provider_selectable {
        CodecFact {
            value: Some(true),
            evidence: Evidence::Inferred,
            confidence: Confidence::Medium,
            method: format!("Provider manifest: {}", provider_ids.join(", ")),
            note: "The installed provider advertises this codec. The remote device and hardware still need provider validation.".into(),
        }
    } else if codec == CodecId::Other {
        CodecFact {
            value: None,
            evidence: Evidence::Unverified,
            confidence: Confidence::Low,
            method: "Unrecognised tuple".into(),
            note: "No codec name or capability is inferred.".into(),
        }
    } else if matches!(support, WindowsCodecSupport::ConditionalQualcommRadio) {
        CodecFact {
            value: None,
            evidence: Evidence::Documented,
            confidence: Confidence::Low,
            method: "Microsoft conditional Qualcomm OEM listing".into(),
            note: "Available only on select OEM hardware; this PC has not been verified.".into(),
        }
    } else if native_inventory.contains(&codec) {
        CodecFact {
            value: Some(true),
            evidence: Evidence::Documented,
            confidence: Confidence::High,
            method: "Microsoft Windows Bluetooth Classic audio codec table".into(),
            note: "Local Windows stack inventory, not proof that the remote device supports it."
                .into(),
        }
    } else {
        CodecFact {
            value: Some(false),
            evidence: Evidence::Documented,
            confidence: Confidence::High,
            method: "Microsoft Windows Bluetooth Classic audio codec table".into(),
            note: "Not documented as in-box for this Windows build.".into(),
        }
    };

    let negotiated = observation
        .map(|o| {
            if o.codec == codec {
                CodecFact {
                    value: Some(o.codec),
                    evidence: Evidence::Observed,
                    confidence: o.device_association_confidence,
                    method: o.method.label().into(),
                    note: o.note.clone(),
                }
            } else {
                CodecFact::unavailable(
                    o.method.label(),
                    format!("A different codec ({}) was observed.", o.codec.label()),
                )
            }
        })
        .unwrap_or_else(|| {
            CodecFact::unavailable(
                "No A2DP trace captured",
                "Windows has no documented API that reports the currently negotiated A2DP codec.",
            )
        });

    let observed_active = observed_for_codec
        .map(|o| CodecFact {
            value: Some(o.codec),
            evidence: Evidence::Observed,
            confidence: o.device_association_confidence,
            method: o.method.label().into(),
            note: if o.method == VerificationMethod::ProviderReport {
                "The advanced codec provider reported this active codec. See its verification method and caveats.".into()
            } else {
                "The A2dpStreaming event reported this codec ID.".into()
            },
        })
        .unwrap_or_else(|| {
            CodecFact::unavailable(
                "A2DP streaming trace",
                "No matching codec event has been captured in this session.",
            )
        });

    let configuration = if provider_selectable {
        CodecFact {
            value: Some(provider_ids.join(", ")),
            evidence: Evidence::Inferred,
            confidence: Confidence::Medium,
            method: "Advanced codec provider API".into(),
            note: format!(
                "{} can be configured through an installed provider. Open Providers to choose priority and parameters.",
                codec.label()
            ),
        }
    } else {
        CodecFact::unavailable(
            "No supported Windows API or installed provider",
            format!(
                "Bluu cannot set {} codec priority, sample rate, channels, or bitrate without a provider.",
                codec.label()
            ),
        )
    };

    let observed_at = observed_for_codec.map(|o| o.observed_at);
    let confidence = match observed_for_codec {
        Some(observation) => observation.device_association_confidence,
        None if native_inventory.contains(&codec) => Confidence::High,
        None => Confidence::Medium,
    };

    let mut limitations = limitations;
    if hfp_active && connected && matches!(codec, CodecId::Sbc | CodecId::Aac | CodecId::AptX) {
        limitations.push(
            "An active capture endpoint may indicate HFP mode. HFP uses narrowband/wideband speech codecs, not A2DP music codecs.".into(),
        );
    }
    limitations.push(remote_note.into());

    CodecReport {
        codec,
        state,
        capability,
        configuration,
        negotiated,
        observed_active,
        verification: observed_for_codec
            .map(|o| o.method)
            .unwrap_or(VerificationMethod::None),
        provider_selectable,
        provider_ids,
        confidence,
        observed_at: if observed_for_codec.is_some() {
            observed_at
        } else {
            None
        },
        summary,
        limitations,
    }
}

pub fn findings(
    snapshot: &Snapshot,
    device: Option<&BluetoothAudioDevice>,
    reports: &[CodecReport],
) -> Vec<DiagnosticFinding> {
    let mut out = Vec::new();
    if snapshot.system.demo {
        out.push(DiagnosticFinding {
            severity: Severity::Info,
            title: "Demo data is active".into(),
            detail:
                "These devices and endpoints are illustrative and were not read from this computer."
                    .into(),
            action: "Close demo mode and run a normal scan before drawing conclusions.".into(),
            evidence: Evidence::Unverified,
        });
    }
    if snapshot.devices.is_empty() {
        out.push(DiagnosticFinding {
            severity: Severity::Attention,
            title: "No paired Bluetooth devices found".into(),
            detail: "Windows returned no paired Bluetooth devices, or Bluetooth is unavailable."
                .into(),
            action: "Pair the headphones in Windows Settings, connect them, then refresh.".into(),
            evidence: Evidence::Observed,
        });
    }
    let Some(device) = device else {
        return out;
    };
    if !device.connected {
        out.push(DiagnosticFinding {
            severity: Severity::Attention,
            title: "Device is not connected".into(),
            detail: format!(
                "{} is paired but Windows reports it as disconnected.",
                device.name
            ),
            action: "Turn the device on and connect it in Windows Settings, then refresh.".into(),
            evidence: Evidence::Observed,
        });
    }
    match (device.supports_a2dp(), device.profile_evidence_available()) {
        (Some(false), true) => out.push(DiagnosticFinding {
            severity: Severity::Limitation,
            title: "No A2DP service observed".into(),
            detail: "The cached SDP records do not advertise an A2DP audio source/sink service."
                .into(),
            action: "This device may be microphone-, LE-, or vendor-protocol-only. Do not expect A2DP music codecs."
                .into(),
            evidence: Evidence::Observed,
        }),
        (Some(true), true) => out.push(DiagnosticFinding {
            severity: Severity::Info,
            title: "A2DP profile observed".into(),
            detail: "Windows' cached SDP records advertise A2DP. Windows does not expose negotiated AVDTP codec capabilities to apps."
                .into(),
            action: "Use Verify live codec while playing audio; that observes the Windows streaming event, not the device's full codec list."
                .into(),
            evidence: Evidence::Observed,
        }),
        _ => out.push(DiagnosticFinding {
            severity: Severity::Limitation,
            title: "A2DP capability unknown".into(),
            detail: "The SDP records were empty or could not be decoded.".into(),
            action: "Refresh while the device is connected. If it persists, export diagnostics.".into(),
            evidence: Evidence::NotObservable,
        }),
    }
    if device.endpoint_ids.is_empty() {
        out.push(DiagnosticFinding {
            severity: Severity::Attention,
            title: "No associated audio endpoint".into(),
            detail: "Bluu could not relate a Windows audio endpoint to this Bluetooth device.".into(),
            action: "Check Windows Sound settings and refresh. Endpoint association is required to show system audio format."
                .into(),
            evidence: Evidence::NotObservable,
        });
    } else if let Some(endpoint) = snapshot.selected_render_endpoint(Some(device)) {
        if endpoint.state != crate::model::EndpointState::Active {
            out.push(DiagnosticFinding {
                severity: Severity::Attention,
                title: "Render endpoint is not active".into(),
                detail: format!(
                    "{} reports state '{}'.",
                    endpoint.friendly_name,
                    endpoint.state.label()
                ),
                action: "Reconnect the device or select its output in Windows Sound settings."
                    .into(),
                evidence: Evidence::Observed,
            });
        }
    }
    if snapshot.selected_capture_endpoint(Some(device)).is_some() {
        out.push(DiagnosticFinding {
            severity: Severity::Info,
            title: "Microphone may switch the route to HFP".into(),
            detail: "On Windows 11 A2DP and HFP share one endpoint. Opening the microphone or using communications audio can switch the device to HFP."
                .into(),
            action: "For music, close apps using the headset microphone. HFP quality and latency are separate from A2DP codec selection."
                .into(),
            evidence: Evidence::Documented,
        });
    }
    if reports
        .iter()
        .any(|r| r.state == CodecState::SupportedAvailable)
    {
        out.push(DiagnosticFinding {
            severity: Severity::Info,
            title: "Supported does not mean active".into(),
            detail: "Some codecs are in the Windows inventory, but Windows chooses the negotiated codec automatically.".into(),
            action: "Run Verify live codec during playback to observe the negotiated codec.".into(),
            evidence: Evidence::Documented,
        });
    }
    let unsupported: Vec<_> = reports
        .iter()
        .filter(|r| r.state == CodecState::UnavailableOnBackend)
        .map(|r| r.codec.label())
        .collect();
    if !unsupported.is_empty() {
        out.push(DiagnosticFinding {
            severity: Severity::Limitation,
            title: "Advanced codecs are not in this Windows stack".into(),
            detail: format!(
                "{} are not presented as in-box A2DP codecs for this Windows build.",
                unsupported.join(", ")
            ),
            action: "Safe supported options are a different radio/PC, a vendor-provided solution, or an external Bluetooth audio dongle. Bluu will not patch the stack."
                .into(),
            evidence: Evidence::Documented,
        });
    }
    if let Some(provider) = snapshot.providers.iter().find(|p| p.is_usable()) {
        out.push(DiagnosticFinding {
            severity: Severity::Info,
            title: "Advanced codec provider available".into(),
            detail: format!(
                "{} advertises {} and may control the A2DP encoder/transport.",
                provider.manifest.name,
                provider
                    .manifest
                    .codecs
                    .iter()
                    .map(|c| c.codec.label())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            action: "Open Providers to review the manifest and apply a preference. Verify the active codec separately."
                .into(),
            evidence: Evidence::Inferred,
        });
    }
    out
}

pub fn latest_timestamp(snapshot: &Snapshot) -> DateTime<Utc> {
    snapshot.generated_at
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        backend::BackendDescriptor,
        model::{
            ActiveCodecObservation, AssociationMethod, AudioProfile, BluetoothAudioDevice,
            EndpointAssociation, ProfileObservation, SystemInfo, Transport,
        },
    };

    fn snapshot(build: u32) -> Snapshot {
        let mut s = Snapshot::empty(SystemInfo {
            os_name: "Windows".into(),
            os_version: "test".into(),
            os_build: build,
            architecture: "x86_64".into(),
            elevated: false,
            demo: false,
        });
        s.backend = BackendDescriptor {
            id: "test".into(),
            name: "Test".into(),
            platform: "Windows".into(),
            read_only: true,
            demo: false,
        };
        s
    }

    fn device() -> BluetoothAudioDevice {
        BluetoothAudioDevice {
            id: "dev".into(),
            name: "Headphones".into(),
            address: None,
            container_id: None,
            connected: true,
            paired: true,
            transport: Transport::DualMode,
            class_of_device: None,
            category: Some("Headphones".into()),
            profiles: vec![ProfileObservation {
                profile: AudioProfile::A2dpSink,
                version: Some("1.4".into()),
                evidence: Evidence::Observed,
                note: "test".into(),
            }],
            endpoint_ids: vec![],
            association: EndpointAssociation {
                method: AssociationMethod::None,
                confidence: Confidence::Low,
                note: "test".into(),
            },
            last_seen: None,
            backend_note: None,
        }
    }

    #[test]
    fn support_is_not_reported_as_active() {
        let s = snapshot(22_631);
        let reports = codec_reports(&s, Some(&device()));
        let aac = reports.iter().find(|r| r.codec == CodecId::Aac).unwrap();
        assert_eq!(aac.state, CodecState::SupportedAvailable);
        assert!(aac.observed_active.value.is_none());
    }

    #[test]
    fn generic_ldac_is_unavailable_on_backend() {
        let s = snapshot(26_100);
        let reports = codec_reports(&s, Some(&device()));
        let ldac = reports.iter().find(|r| r.codec == CodecId::Ldac).unwrap();
        assert_eq!(ldac.state, CodecState::UnavailableOnBackend);
    }

    #[test]
    fn provider_availability_is_separate_from_active_state() {
        let mut s = snapshot(22_631);
        let mut provider: crate::ProviderRecord = serde_json::from_value(serde_json::json!({
            "manifest": {
                "api_version": 1, "id": "vendor.ldac", "name": "Vendor LDAC",
                "version": "1", "executable": "provider.exe", "stack": "vendor_service",
                "capabilities": {"preferred_codec": true, "active_codec": true},
                "codecs": [{"codec": "ldac"}, {"codec": "aptx_hd"}]
            },
            "manifest_path": "x", "directory": "x", "status": "ready",
            "issues": [], "executable_sha256": "x", "simulated": false
        }))
        .unwrap();
        provider.status = crate::ProviderStatus::Ready;
        s.providers.push(provider);
        let reports = codec_reports(&s, Some(&device()));
        let ldac = reports.iter().find(|r| r.codec == CodecId::Ldac).unwrap();
        assert_eq!(ldac.state, CodecState::ProviderAvailable);
        assert!(ldac.provider_selectable);
        assert!(ldac.observed_active.value.is_none());
    }

    #[test]
    fn provider_preference_is_not_reported_active() {
        let mut s = snapshot(22_631);
        s.providers.push(
            serde_json::from_value(serde_json::json!({
                "manifest": {
                    "api_version": 1, "id": "vendor.ldac", "name": "Vendor LDAC",
                    "version": "1", "executable": "provider.exe", "stack": "vendor_service",
                    "capabilities": {"preferred_codec": true},
                    "codecs": [{"codec": "ldac"}]
                },
                "manifest_path": "x", "directory": "x", "status": "ready",
                "issues": [], "executable_sha256": "x", "simulated": false
            }))
            .unwrap(),
        );
        s.provider_preferences.push(crate::ProviderPreference {
            provider_id: "vendor.ldac".into(),
            device_id: "dev".into(),
            ordered_codecs: vec![CodecId::Ldac],
            parameters: Default::default(),
            dry_run: false,
        });
        let reports = codec_reports(&s, Some(&device()));
        let ldac = reports.iter().find(|r| r.codec == CodecId::Ldac).unwrap();
        assert_eq!(ldac.state, CodecState::ProviderPreferenceApplied);
        assert!(!ldac.state.is_active());
    }

    #[test]
    fn observation_without_device_association_is_not_verified() {
        let mut s = snapshot(22_631);
        s.active_codec = Some(ActiveCodecObservation {
            codec: CodecId::Aac,
            vendor_id: None,
            vendor_codec_id: None,
            observed_at: Utc::now(),
            method: VerificationMethod::A2dpEtwEvent,
            device_id: None,
            device_association_confidence: Confidence::Low,
            note: "one A2DP stream observed".into(),
        });
        let reports = codec_reports(&s, Some(&device()));
        let aac = reports.iter().find(|r| r.codec == CodecId::Aac).unwrap();
        assert_eq!(aac.state, CodecState::ActiveUnverified);
    }
}
