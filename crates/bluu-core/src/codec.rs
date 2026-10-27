use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CodecId {
    Sbc,
    Aac,
    AptX,
    AptXHd,
    AptXLowLatency,
    AptXAdaptive,
    AptXLossless,
    Ldac,
    Lc3,
    Mp3,
    Other,
}

impl Serialize for CodecId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.key())
    }
}

impl<'de> Deserialize<'de> for CodecId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        CodecId::from_key(&value).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "unknown codec key '{value}'; expected one of sbc, aac, aptx, aptx_hd, aptx_low_latency, aptx_adaptive, aptx_lossless, ldac, lc3, mp3, other"
            ))
        })
    }
}

impl CodecId {
    pub fn from_key(value: &str) -> Option<Self> {
        Some(match value {
            "sbc" => Self::Sbc,
            "aac" => Self::Aac,
            "aptx" => Self::AptX,
            "aptx_hd" => Self::AptXHd,
            "aptx_low_latency" => Self::AptXLowLatency,
            "aptx_adaptive" => Self::AptXAdaptive,
            "aptx_lossless" => Self::AptXLossless,
            "ldac" => Self::Ldac,
            "lc3" => Self::Lc3,
            "mp3" => Self::Mp3,
            "other" => Self::Other,
            _ => return None,
        })
    }

    pub const ALL: [CodecId; 11] = [
        Self::Sbc,
        Self::Aac,
        Self::AptX,
        Self::AptXHd,
        Self::AptXLowLatency,
        Self::AptXAdaptive,
        Self::AptXLossless,
        Self::Ldac,
        Self::Lc3,
        Self::Mp3,
        Self::Other,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Self::Sbc => "sbc",
            Self::Aac => "aac",
            Self::AptX => "aptx",
            Self::AptXHd => "aptx_hd",
            Self::AptXLowLatency => "aptx_low_latency",
            Self::AptXAdaptive => "aptx_adaptive",
            Self::AptXLossless => "aptx_lossless",
            Self::Ldac => "ldac",
            Self::Lc3 => "lc3",
            Self::Mp3 => "mp3",
            Self::Other => "other",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Sbc => "SBC",
            Self::Aac => "AAC",
            Self::AptX => "aptX Classic",
            Self::AptXHd => "aptX HD",
            Self::AptXLowLatency => "aptX Low Latency",
            Self::AptXAdaptive => "aptX Adaptive",
            Self::AptXLossless => "aptX Lossless",
            Self::Ldac => "LDAC",
            Self::Lc3 => "LC3",
            Self::Mp3 => "MP3",
            Self::Other => "Unknown codec",
        }
    }

    pub fn is_a2dp(self) -> bool {
        !matches!(self, Self::Lc3)
    }

    pub fn is_lossy(self) -> bool {
        !matches!(self, Self::AptXLossless)
    }

    /// Maps the A2DP standard codec ID from an AVDTP media-codec descriptor or
    /// the Windows A2DP trace. `0xFF` requires vendor fields and is handled by
    /// [`from_a2dp_ids`].
    pub fn from_standard_id(id: u8) -> Self {
        match id {
            0x00 => Self::Sbc,
            0x01 => Self::Mp3,
            0x02 => Self::Aac,
            0x04 => Self::Other, // ATRAC is standard but not presented as a product claim
            0xFF => Self::Other,
            _ => Self::Other,
        }
    }

    pub fn from_a2dp_ids(
        standard_id: u8,
        vendor_id: Option<u32>,
        vendor_codec_id: Option<u16>,
    ) -> Self {
        if standard_id != 0xFF {
            return Self::from_standard_id(standard_id);
        }
        match (vendor_id, vendor_codec_id) {
            (Some(0x004F), Some(0x0001)) => Self::AptX,
            (Some(0x00D7), Some(0x0024)) => Self::AptXHd,
            // Vendor codec ID 0x0002 is aptX Low Latency in the A2DP vendor
            // tuples popularised by btcodecs/ValdikSS. It is not aptX HD.
            (Some(0x00D7), Some(0x0002)) | (Some(0x000A), Some(0x0002)) => Self::AptXLowLatency,
            (Some(0x012D), Some(0x00AA)) => Self::Ldac,
            // Adaptive/Lossless vendor tuples are not a public contract. Do not
            // assign a marketing name without hardware-validated evidence: show
            // Other and preserve the raw IDs instead.
            _ => Self::Other,
        }
    }

    pub fn recognized_by_vendor_tuple(self, standard_id: u8) -> bool {
        if standard_id != 0xFF {
            return !matches!(self, Self::Other);
        }
        !matches!(
            self,
            Self::Other | Self::Sbc | Self::Aac | Self::Mp3 | Self::Lc3
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodecState {
    ActiveVerified,
    ActiveProviderAttested,
    ActiveUnverified,
    ProviderPreferenceApplied,
    ProviderAvailable,
    SupportedAvailable,
    HardwareOnly,
    UnavailableOnBackend,
    Unsupported,
    Unknown,
}

impl CodecState {
    pub fn label(self) -> &'static str {
        match self {
            Self::ActiveVerified => "Active · independently verified",
            Self::ActiveProviderAttested => "Active · provider-attested",
            Self::ActiveUnverified => "Active · not independently verified",
            Self::ProviderPreferenceApplied => "Provider preference applied · active not verified",
            Self::ProviderAvailable => "Available through an advanced codec provider",
            Self::SupportedAvailable => "Supported by this Windows stack",
            Self::HardwareOnly => "Hardware support only; not exposed by this stack",
            Self::UnavailableOnBackend => "Not available through this backend",
            Self::Unsupported => "Unsupported",
            Self::Unknown => "Unknown",
        }
    }

    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::ActiveVerified | Self::ActiveProviderAttested | Self::ActiveUnverified
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

impl Confidence {
    pub fn label(self) -> &'static str {
        match self {
            Self::High => "high confidence",
            Self::Medium => "medium confidence",
            Self::Low => "low confidence",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Evidence {
    Documented,
    Observed,
    Inferred,
    NotObservable,
    Unverified,
}

impl Evidence {
    pub fn label(self) -> &'static str {
        match self {
            Self::Documented => "documented",
            Self::Observed => "observed",
            Self::Inferred => "inferred",
            Self::NotObservable => "not observable",
            Self::Unverified => "unverified",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationMethod {
    None,
    A2dpEtwEvent,
    ProviderReport,
    UserProvidedTrace,
    ManualExternalAnalyzer,
}

impl VerificationMethod {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::A2dpEtwEvent => "A2DP streaming trace (ETW)",
            Self::ProviderReport => "advanced codec provider report",
            Self::UserProvidedTrace => "user-provided protocol trace",
            Self::ManualExternalAnalyzer => "external analyzer",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CodecFact<T> {
    pub value: Option<T>,
    pub evidence: Evidence,
    pub confidence: Confidence,
    pub method: String,
    pub note: String,
}

impl<T> CodecFact<T> {
    pub fn unavailable(method: impl Into<String>, note: impl Into<String>) -> Self {
        Self {
            value: None,
            evidence: Evidence::NotObservable,
            confidence: Confidence::Low,
            method: method.into(),
            note: note.into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CodecReport {
    pub codec: CodecId,
    pub state: CodecState,
    pub capability: CodecFact<bool>,
    pub configuration: CodecFact<String>,
    pub negotiated: CodecFact<CodecId>,
    pub observed_active: CodecFact<CodecId>,
    pub verification: VerificationMethod,
    pub provider_selectable: bool,
    pub provider_ids: Vec<String>,
    pub confidence: Confidence,
    pub observed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub summary: String,
    pub limitations: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_ids_are_exact() {
        assert_eq!(CodecId::from_standard_id(0x00), CodecId::Sbc);
        assert_eq!(CodecId::from_standard_id(0x02), CodecId::Aac);
        assert_eq!(CodecId::from_standard_id(0xFF), CodecId::Other);
        assert_eq!(CodecId::from_standard_id(0x7f), CodecId::Other);
    }

    #[test]
    fn vendor_ids_map_without_promoting_other_codecs() {
        assert_eq!(
            CodecId::from_a2dp_ids(0xFF, Some(0x004F), Some(0x0001)),
            CodecId::AptX
        );
        assert_eq!(
            CodecId::from_a2dp_ids(0xFF, Some(0x00D7), Some(0x0024)),
            CodecId::AptXHd
        );
        assert_eq!(
            CodecId::from_a2dp_ids(0xFF, Some(0x012D), Some(0x00AA)),
            CodecId::Ldac
        );
        assert_eq!(
            CodecId::from_a2dp_ids(0xFF, Some(0x1234), Some(0x0001)),
            CodecId::Other
        );
        assert_eq!(
            CodecId::from_a2dp_ids(0x02, Some(0x004F), Some(0x0001)),
            CodecId::Aac
        );
    }

    #[test]
    fn low_latency_is_not_reported_as_hd() {
        assert_eq!(
            CodecId::from_a2dp_ids(0xFF, Some(0x00D7), Some(0x0002)),
            CodecId::AptXLowLatency
        );
        assert_eq!(
            CodecId::from_a2dp_ids(0xFF, Some(0x00D7), Some(0x0025)),
            CodecId::Other
        );
    }
}
