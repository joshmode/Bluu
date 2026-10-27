use serde::{Deserialize, Serialize};

use crate::{
    codec::{CodecId, Confidence, Evidence},
    model::SystemInfo,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeasibilityStatus {
    NativelySupported,
    ConfigurableOnSupportedSystems,
    ConditionallyImplementable,
    RequiresSubstantialSystemIntegration,
    NotFeasibleSafeSupported,
    UnknownRequiresHardwareTesting,
}

impl FeasibilityStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::NativelySupported => "Natively supported",
            Self::ConfigurableOnSupportedSystems => "Configurable on supported systems",
            Self::ConditionallyImplementable => "Conditionally implementable",
            Self::RequiresSubstantialSystemIntegration => "Requires substantial system integration",
            Self::NotFeasibleSafeSupported => "Not feasible through a safe supported approach",
            Self::UnknownRequiresHardwareTesting => "Unknown · requires hardware testing",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FeasibilityRow {
    pub codec: CodecId,
    pub status: FeasibilityStatus,
    pub confidence: Confidence,
    pub windows_native: String,
    pub oem_driver_dependent: String,
    pub config_only: String,
    pub needs_codec_implementation: String,
    pub needs_stack_or_driver: String,
    pub remote_must_advertise: String,
    pub licensing: String,
    pub open_implementations: String,
    pub parameters_observable: String,
    pub evidence: Evidence,
    pub limitations: Vec<String>,
}

pub fn feasibility_matrix() -> Vec<FeasibilityRow> {
    vec![
        FeasibilityRow {
            codec: CodecId::Sbc,
            status: FeasibilityStatus::NativelySupported,
            confidence: Confidence::High,
            windows_native: "Windows 10 1507+ and Windows 11 21H2+ (Microsoft A2DP audio documentation).".into(),
            oem_driver_dependent: "No for baseline SBC, although the radio/driver still affects link quality.".into(),
            config_only: "No documented per-device codec preference API. Codec negotiation is automatic.".into(),
            needs_codec_implementation: "No; supplied by Windows.".into(),
            needs_stack_or_driver: "No for ordinary A2DP.".into(),
            remote_must_advertise: "A2DP requires SBC, so remote support is mandatory.".into(),
            licensing: "SBC is part of the A2DP specification; no proprietary codec binary is bundled here.".into(),
            open_implementations: "Several (for analysis/encoding), but not needed for Windows A2DP.".into(),
            parameters_observable: "Active codec ID can be observed from the A2DP ETW event. Bitpool/parameters are not exposed by a stable public API.".into(),
            evidence: Evidence::Documented,
            limitations: vec![
                "No supported Windows API sets preferred codec or bitpool.".into(),
                "Observed selected configuration does not prove audio quality.".into(),
            ],
        },
        FeasibilityRow {
            codec: CodecId::Aac,
            status: FeasibilityStatus::NativelySupported,
            confidence: Confidence::High,
            windows_native: "Windows 11 21H2+ per Microsoft's Bluetooth Classic audio table; not listed for Windows 10.".into(),
            oem_driver_dependent: "Can depend on the radio/driver implementation.".into(),
            config_only: "No public preference or parameter API.".into(),
            needs_codec_implementation: "No; Windows contains the A2DP AAC path.".into(),
            needs_stack_or_driver: "No for supported Windows 11 builds.".into(),
            remote_must_advertise: "Yes. A2DP capability discovery is required and not exposed by Windows to apps.".into(),
            licensing: "Microsoft supplies the in-box implementation; this project does not redistribute it.".into(),
            open_implementations: "AAC implementations exist, but that alone cannot register an encoder with Windows Bluetooth.".into(),
            parameters_observable: "Negotiated codec can be observed in an A2DP trace; bitrate/object type are not a supported Windows API.".into(),
            evidence: Evidence::Documented,
            limitations: vec![
                "A remote device advertising AAC does not guarantee Windows will select it.".into(),
                "No documented API to force AAC.".into(),
            ],
        },
        FeasibilityRow {
            codec: CodecId::AptX,
            status: FeasibilityStatus::NativelySupported,
            confidence: Confidence::High,
            windows_native: "Windows 10 1507+ and Windows 11 21H2+ per Microsoft documentation.".into(),
            oem_driver_dependent: "The Windows table lists it as in-box; actual radio/driver behavior still matters.".into(),
            config_only: "No supported preference API.".into(),
            needs_codec_implementation: "No for the Windows in-box path.".into(),
            needs_stack_or_driver: "No for supported in-box configurations.".into(),
            remote_must_advertise: "Yes; Qualcomm licensing applies to the remote implementation.".into(),
            licensing: "aptX is a Qualcomm brand/technology with licensing requirements. This project does not bundle it.".into(),
            open_implementations: "libopenaptx (GPLv3+) exists but is an encoder/decoder, not a Windows A2DP stack extension.".into(),
            parameters_observable: "Codec ID can be observed; sample-rate bits may be inferred from a protocol trace, not a stable public API.".into(),
            evidence: Evidence::Documented,
            limitations: vec![
                "Marketing names do not establish codec support.".into(),
                "No supported codec preference control.".into(),
            ],
        },
        FeasibilityRow {
            codec: CodecId::AptXHd,
            status: FeasibilityStatus::ConditionallyImplementable,
            confidence: Confidence::High,
            windows_native: "Not in Microsoft's in-box A2DP codec table for Windows 10 or 11.".into(),
            oem_driver_dependent: "Only if an OEM/vendor supplies a complete Bluetooth A2DP path; not a generic toggle.".into(),
            config_only: "No. Use a validated advanced codec provider that owns the A2DP driver/transport; registry toggles are not a codec-registration mechanism.".into(),
            needs_codec_implementation: "Yes, plus a transport stack able to negotiate and send it.".into(),
            needs_stack_or_driver: "Yes: replacement A2DP driver/stack or vendor integration.".into(),
            remote_must_advertise: "Yes.".into(),
            licensing: "Qualcomm licensing plus driver signing and redistribution obligations.".into(),
            open_implementations: "libopenaptx provides codec coding, not the A2DP/Windows integration.".into(),
            parameters_observable: "Only through custom trace instrumentation or the replacement stack.".into(),
            evidence: Evidence::Documented,
            limitations: vec![
                "A decoder or encoder library does not make Windows transmit aptX HD.".into(),
                "A provider/replacement A2DP stack is required and raises kernel/driver, Secure Boot, support, and recovery risks.".into(),
            ],
        },
        FeasibilityRow {
            codec: CodecId::AptXAdaptive,
            status: FeasibilityStatus::ConditionallyImplementable,
            confidence: Confidence::High,
            windows_native: "Windows 11 24H2+ only on select devices with compatible Qualcomm Bluetooth radios (Microsoft documentation).".into(),
            oem_driver_dependent: "Yes; strongly hardware and OEM integration dependent.".into(),
            config_only: "No generic in-box path. An OEM/vendor provider may expose a preference; otherwise the vendor radio chooses automatically.".into(),
            needs_codec_implementation: "For non-qualified hardware, a complete licensed implementation and stack would be needed.".into(),
            needs_stack_or_driver: "Yes for generic PCs; no for qualifying OEM stacks.".into(),
            remote_must_advertise: "Yes, and the local radio must support the licensed path.".into(),
            licensing: "Qualcomm technology/licensing and likely confidential integration; redistribution rights must be verified.".into(),
            open_implementations: "No suitable complete open-source A2DP Adaptive host implementation was identified.".into(),
            parameters_observable: "Qualifying OEM trace may expose negotiated codec; bitrate adaptation is internal.".into(),
            evidence: Evidence::Documented,
            limitations: vec![
                "Do not present 24H2 support as universal aptX Adaptive support.".into(),
                "Lossless mode is even more hardware/licence constrained.".into(),
            ],
        },
        FeasibilityRow {
            codec: CodecId::AptXLossless,
            status: FeasibilityStatus::ConditionallyImplementable,
            confidence: Confidence::Medium,
            windows_native: "Microsoft labels the 24H2 entry 'aptX Adaptive (lossless)' for select Qualcomm radios. It is not a general Windows feature.".into(),
            oem_driver_dependent: "Yes, to an extreme degree.".into(),
            config_only: "No.".into(),
            needs_codec_implementation: "Yes; proprietary and hardware-coupled.".into(),
            needs_stack_or_driver: "Yes for generic hardware.".into(),
            remote_must_advertise: "Yes, along with compatible local radio/stack.".into(),
            licensing: "Qualcomm licensing, confidential technical information, and redistribution restrictions.".into(),
            open_implementations: "None identified that could legally and reliably provide a Windows A2DP source path.".into(),
            parameters_observable: "Only through vendor diagnostics; not a supported generic API.".into(),
            evidence: Evidence::Inferred,
            limitations: vec![
                "No safe supported route identified for generic PCs.".into(),
                "The exact mode selected by a qualifying OEM stack may remain unverified.".into(),
            ],
        },
        FeasibilityRow {
            codec: CodecId::AptXLowLatency,
            status: FeasibilityStatus::ConditionallyImplementable,
            confidence: Confidence::Medium,
            windows_native: "Not in Microsoft's documented in-box A2DP codec table. Some third-party stacks recognise the vendor tuple.".into(),
            oem_driver_dependent: "Yes; only a vendor stack can provide a usable path.".into(),
            config_only: "No documented Windows preference.".into(),
            needs_codec_implementation: "Yes, plus A2DP transport support.".into(),
            needs_stack_or_driver: "Yes for generic Windows.".into(),
            remote_must_advertise: "Yes.".into(),
            licensing: "Qualcomm aptX licensing and driver/signing obligations.".into(),
            open_implementations: "No complete A2DP Low Latency host path identified.".into(),
            parameters_observable: "Not through a documented Windows API.".into(),
            evidence: Evidence::Inferred,
            limitations: vec![
                "Do not treat the Low Latency vendor tuple as in-box support.".into(),
                "Latency cannot be inferred from the codec name.".into(),
            ],
        },
        FeasibilityRow {
            codec: CodecId::Ldac,
            status: FeasibilityStatus::ConditionallyImplementable,
            confidence: Confidence::High,
            windows_native: "Not in Microsoft's in-box Windows 10/11 A2DP codec table.".into(),
            oem_driver_dependent: "Could be supplied by a vendor dongle or replacement stack, but not exposed as a Windows preference.".into(),
            config_only: "No in-box path. A provider may expose LDAC ordering/parameters when it supplies the encoder and A2DP transport.".into(),
            needs_codec_implementation: "Yes, plus A2DP negotiation and transmission integration.".into(),
            needs_stack_or_driver: "Yes, unless using an external dongle that presents its own audio device.".into(),
            remote_must_advertise: "Yes.".into(),
            licensing: "Sony's LDAC source is in AOSP, but Sony states a licence is required to implement the software; patents/trademarks also require review.".into(),
            open_implementations: "AOSP libldac and other packages exist; open source does not by itself grant all implementation/distribution rights.".into(),
            parameters_observable: "Only via replacement-stack trace or external analyzer.".into(),
            evidence: Evidence::Documented,
            limitations: vec![
                "Do not bundle an LDAC binary without verified rights.".into(),
                "A software encoder does not create Windows A2DP support; use an installed provider that owns the driver/transport and licensing.".into(),
            ],
        },
        FeasibilityRow {
            codec: CodecId::Lc3,
            status: FeasibilityStatus::ConditionallyImplementable,
            confidence: Confidence::High,
            windows_native: "Windows 11 22H2+ LE Audio uses vendor-specific ACX/VSAP paths; LC3 may be hosted by the controller, audio DSP, or IHV driver.".into(),
            oem_driver_dependent: "Yes. Windows documents an IHV audio driver stack and vendor-specific data path.".into(),
            config_only: "No public app-level codec registration or preference API.".into(),
            needs_codec_implementation: "LC3 coding plus BAP/ISO transport and an ACX audio path. Some platforms offload it.".into(),
            needs_stack_or_driver: "Yes for hardware lacking a vendor LE Audio stack.".into(),
            remote_must_advertise: "Yes through PACS/BAP; profiles differ from A2DP.".into(),
            licensing: "LC3 is specified by the Bluetooth SIG; implementation/patent licensing still requires review.".into(),
            open_implementations: "Open LC3 implementations exist, but they do not substitute for Windows VSAP/ACX integration.".into(),
            parameters_observable: "PACS/BAP configuration is transported in the OS/driver path; not exposed as a documented desktop app API.".into(),
            evidence: Evidence::Documented,
            limitations: vec![
                "LE Audio is a separate transport/profile and must not be conflated with A2DP.".into(),
                "The active A2DP ETW event does not describe LE Audio.".into(),
            ],
        },
        FeasibilityRow {
            codec: CodecId::Mp3,
            status: FeasibilityStatus::NotFeasibleSafeSupported,
            confidence: Confidence::Medium,
            windows_native: "MP3 is a standard A2DP codec ID, but it is not in Microsoft's documented in-box Windows A2DP codec table.".into(),
            oem_driver_dependent: "Potential vendor path, but not established for this target.".into(),
            config_only: "No documented path.".into(),
            needs_codec_implementation: "If not provided by a vendor stack.".into(),
            needs_stack_or_driver: "Likely yes for generic Windows.".into(),
            remote_must_advertise: "Yes.".into(),
            licensing: "MP3 patent/licensing history is complex; current status requires legal review.".into(),
            open_implementations: "Several, but irrelevant without a Windows A2DP registration path.".into(),
            parameters_observable: "Not through the documented Windows API; trace only if negotiated.".into(),
            evidence: Evidence::Inferred,
            limitations: vec!["Not targeted by this project.".into()],
        },
        FeasibilityRow {
            codec: CodecId::Other,
            status: FeasibilityStatus::UnknownRequiresHardwareTesting,
            confidence: Confidence::Low,
            windows_native: "Unrecognised standard/vendor tuple; raw IDs are preserved.".into(),
            oem_driver_dependent: "Unknown.".into(),
            config_only: "No.".into(),
            needs_codec_implementation: "Unknown.".into(),
            needs_stack_or_driver: "Unknown.".into(),
            remote_must_advertise: "Yes for any codec.".into(),
            licensing: "Unknown; do not redistribute an unidentified codec.".into(),
            open_implementations: "Not applicable until identified.".into(),
            parameters_observable: "Raw IDs may be observable in a trace.".into(),
            evidence: Evidence::Unverified,
            limitations: vec!["No codec name is inferred from an unknown tuple.".into()],
        },
    ]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowsCodecSupport {
    Native,
    ConditionalQualcommRadio,
    NotNative,
}

pub fn windows_codec_support(codec: CodecId, system: &SystemInfo) -> WindowsCodecSupport {
    match codec {
        CodecId::Sbc | CodecId::AptX if system.is_windows_10() || system.is_windows_11() => {
            WindowsCodecSupport::Native
        }
        CodecId::Aac if system.is_windows_11() => WindowsCodecSupport::Native,
        CodecId::Aac => WindowsCodecSupport::NotNative,
        // Microsoft's table says "aptX Adaptive (lossless)" on select Qualcomm
        // radios. It does not establish two separately selectable codecs. Keep
        // these as conditional hardware-only entries, never generic features.
        CodecId::AptXAdaptive | CodecId::AptXLossless if system.os_build >= 26_100 => {
            WindowsCodecSupport::ConditionalQualcommRadio
        }
        _ => WindowsCodecSupport::NotNative,
    }
}

pub fn codec_inventory(system: &SystemInfo) -> Vec<CodecId> {
    if system.is_windows_10() || system.is_windows_11() {
        let mut out = vec![CodecId::Sbc, CodecId::AptX];
        if system.is_windows_11() {
            out.insert(0, CodecId::Aac);
        }
        out
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(build: u32) -> SystemInfo {
        SystemInfo {
            os_name: "Windows".into(),
            os_version: "test".into(),
            os_build: build,
            architecture: "x86_64".into(),
            elevated: false,
            demo: false,
        }
    }

    #[test]
    fn windows_11_has_aac_and_windows_10_does_not() {
        assert_eq!(
            windows_codec_support(CodecId::Aac, &os(22_631)),
            WindowsCodecSupport::Native
        );
        assert_eq!(
            windows_codec_support(CodecId::Aac, &os(19_045)),
            WindowsCodecSupport::NotNative
        );
        assert_eq!(
            codec_inventory(&os(22_631)),
            vec![CodecId::Aac, CodecId::Sbc, CodecId::AptX]
        );
        assert_eq!(
            codec_inventory(&os(19_045)),
            vec![CodecId::Sbc, CodecId::AptX]
        );
    }

    #[test]
    fn adaptive_is_conditional_and_not_generic() {
        assert_eq!(
            windows_codec_support(CodecId::AptXAdaptive, &os(26_100)),
            WindowsCodecSupport::ConditionalQualcommRadio
        );
        assert_eq!(
            windows_codec_support(CodecId::AptXAdaptive, &os(22_631)),
            WindowsCodecSupport::NotNative
        );
    }

    #[test]
    fn ldac_is_not_promised_as_native() {
        let rows = feasibility_matrix();
        let ldac = rows.iter().find(|r| r.codec == CodecId::Ldac).unwrap();
        assert_eq!(ldac.status, FeasibilityStatus::ConditionallyImplementable);
    }
}
