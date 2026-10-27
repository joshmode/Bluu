//! Codec-provider contract.
//!
//! Windows has no public A2DP codec registration API. The supported Bluu extension
//! point is therefore a separately installed provider that owns the codec encoder
//! and transport (or wraps a vendor/replacement stack). Bluu validates manifests,
//! orders preferences, invokes the provider with bounded JSON, and never equates a
//! provider's "preference applied" acknowledgement with actual negotiation.

use std::collections::{BTreeMap, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    codec::{CodecId, Confidence, VerificationMethod},
    model::{ActiveCodecObservation, BluetoothAddress},
};

pub const PROVIDER_API_VERSION: u32 = 1;
pub const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
pub const MAX_PROVIDER_RESULT_BYTES: u64 = 128 * 1024;
pub const MAX_CODECS: usize = 32;
pub const MAX_PARAMETERS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderStack {
    /// A provider that controls a vendor service/driver already installed by its
    /// publisher. It must not pretend to be the Windows in-box stack.
    VendorService,
    /// A replacement A2DP driver/stack provided by a third party.
    ReplacementA2dpDriver,
    /// A hardware/audio driver supplied by the OEM.
    OemDriver,
    /// A local test/demo provider with no real transport.
    Simulated,
}

impl ProviderStack {
    pub fn label(self) -> &'static str {
        match self {
            Self::VendorService => "Vendor service",
            Self::ReplacementA2dpDriver => "Replacement A2DP driver",
            Self::OemDriver => "OEM driver",
            Self::Simulated => "Simulated provider",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    #[serde(default)]
    pub preferred_codec: bool,
    #[serde(default)]
    pub priority: bool,
    #[serde(default)]
    pub parameters: bool,
    #[serde(default)]
    pub active_codec: bool,
    #[serde(default)]
    pub rollback: bool,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProviderParameter {
    Choice {
        key: String,
        label: String,
        options: Vec<String>,
        default: String,
        #[serde(default)]
        note: String,
    },
    Integer {
        key: String,
        label: String,
        min: i64,
        max: i64,
        default: i64,
        #[serde(default)]
        unit: String,
        #[serde(default)]
        note: String,
    },
    Boolean {
        key: String,
        label: String,
        default: bool,
        #[serde(default)]
        note: String,
    },
}

impl ProviderParameter {
    pub fn key(&self) -> &str {
        match self {
            Self::Choice { key, .. } | Self::Integer { key, .. } | Self::Boolean { key, .. } => key,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Choice { label, .. }
            | Self::Integer { label, .. }
            | Self::Boolean { label, .. } => label,
        }
    }

    pub fn note(&self) -> &str {
        match self {
            Self::Choice { note, .. } | Self::Integer { note, .. } | Self::Boolean { note, .. } => {
                note
            }
        }
    }

    pub fn default_value(&self) -> String {
        match self {
            Self::Choice { default, .. } => default.clone(),
            Self::Integer { default, .. } => default.to_string(),
            Self::Boolean { default, .. } => default.to_string(),
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Choice { .. } => "choice",
            Self::Integer { .. } => "integer",
            Self::Boolean { .. } => "boolean",
        }
    }

    pub fn validate_value(&self, value: &str) -> Result<(), ProviderValidationError> {
        match self {
            Self::Choice { options, .. } => {
                if options.iter().any(|v| v == value) {
                    Ok(())
                } else {
                    Err(ProviderValidationError::InvalidParameter(format!(
                        "value '{value}' is not one of the allowed options"
                    )))
                }
            }
            Self::Integer { min, max, .. } => {
                let parsed = value.parse::<i64>().map_err(|_| {
                    ProviderValidationError::InvalidParameter(format!(
                        "value '{value}' is not an integer"
                    ))
                })?;
                if (*min..=*max).contains(&parsed) {
                    Ok(())
                } else {
                    Err(ProviderValidationError::InvalidParameter(format!(
                        "value {parsed} is outside {min}..={max}"
                    )))
                }
            }
            Self::Boolean { .. } => value.parse::<bool>().map(|_| ()).map_err(|_| {
                ProviderValidationError::InvalidParameter(format!(
                    "value '{value}' is not a boolean"
                ))
            }),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderCodec {
    pub codec: CodecId,
    #[serde(default)]
    pub notes: String,
    /// Whether this provider has validated the selected remote device's codec
    /// capability. A manifest advertises provider support, not remote support.
    #[serde(default)]
    pub hardware_validated: bool,
    #[serde(default)]
    pub remote_validated: bool,
    #[serde(default)]
    pub parameters: Vec<ProviderParameter>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderManifest {
    pub api_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub executable: String,
    #[serde(default)]
    pub publisher: String,
    #[serde(default)]
    pub homepage: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub requires_elevation: bool,
    pub stack: ProviderStack,
    pub capabilities: ProviderCapabilities,
    pub codecs: Vec<ProviderCodec>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderStatus {
    Ready,
    Invalid,
    Disabled,
    Unavailable,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderRecord {
    pub manifest: ProviderManifest,
    pub manifest_path: String,
    pub directory: String,
    pub status: ProviderStatus,
    pub issues: Vec<String>,
    pub executable_sha256: Option<String>,
    pub simulated: bool,
}

impl ProviderRecord {
    pub fn is_usable(&self) -> bool {
        self.status == ProviderStatus::Ready
            && self.manifest.capabilities.preferred_codec
            && !self.manifest.codecs.is_empty()
    }

    pub fn supports(&self, codec: CodecId) -> bool {
        self.manifest.codecs.iter().any(|c| c.codec == codec)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProviderPreference {
    pub provider_id: String,
    pub device_id: String,
    pub ordered_codecs: Vec<CodecId>,
    #[serde(default)]
    pub parameters: BTreeMap<String, String>,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderDevice {
    pub id: String,
    pub name: String,
    pub address: Option<BluetoothAddress>,
    pub connected: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAction {
    Describe,
    Apply,
    Rollback,
    Verify,
}

impl ProviderAction {
    pub fn wire_name(self) -> &'static str {
        match self {
            Self::Describe => "describe",
            Self::Apply => "apply",
            Self::Rollback => "rollback",
            Self::Verify => "verify",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderRequest {
    pub api_version: u32,
    pub action: ProviderAction,
    #[serde(default)]
    pub device: Option<ProviderDevice>,
    #[serde(default)]
    pub preference: Option<ProviderPreference>,
    #[serde(default)]
    pub rollback_token: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProviderResponse {
    pub api_version: u32,
    pub ok: bool,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub active_codec: Option<CodecId>,
    #[serde(default)]
    pub active_vendor_id: Option<u32>,
    #[serde(default)]
    pub active_vendor_codec_id: Option<u16>,
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub verification_method: Option<String>,
    #[serde(default)]
    pub rollback_token: Option<String>,
    #[serde(default)]
    pub applied_preference: Option<ProviderPreference>,
    #[serde(default)]
    pub requires_reconnect: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderOutcome {
    pub provider_id: String,
    pub action: ProviderAction,
    pub at: DateTime<Utc>,
    pub response: ProviderResponse,
    pub observation: Option<ActiveCodecObservation>,
}

impl ProviderOutcome {
    pub fn from_response(
        provider_id: impl Into<String>,
        action: ProviderAction,
        response: ProviderResponse,
        request_device: Option<&ProviderDevice>,
    ) -> Result<Self, ProviderValidationError> {
        validate_response(&response)?;
        let provider_id = provider_id.into();
        let observation = response.active_codec.map(|codec| {
            let device_id = request_device.map(|d| d.id.clone());
            let verified = response.verified
                && device_id.is_some()
                && response
                    .verification_method
                    .as_deref()
                    .is_some_and(|m| !m.trim().is_empty());
            ActiveCodecObservation {
                codec,
                vendor_id: response.active_vendor_id,
                vendor_codec_id: response.active_vendor_codec_id,
                observed_at: Utc::now(),
                method: VerificationMethod::ProviderReport,
                device_id: device_id.clone(),
                device_association_confidence: if verified {
                    Confidence::High
                } else {
                    Confidence::Medium
                },
                note: if verified {
                    format!(
                        "Provider '{}' attests that {} is active on this device via {}. This is provider evidence, not an independent trace.",
                        provider_id,
                        codec.label(),
                        response
                            .verification_method
                            .as_deref()
                            .unwrap_or("provider API")
                    )
                } else {
                    format!(
                        "Provider '{}' reported {} via {}. This is a provider report, not independent verification.",
                        provider_id,
                        codec.label(),
                        response
                            .verification_method
                            .as_deref()
                            .unwrap_or("provider API")
                    )
                },
            }
        });
        Ok(Self {
            provider_id,
            action,
            at: Utc::now(),
            response,
            observation,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderValidationError {
    #[error("provider API version {0} is not supported (expected {PROVIDER_API_VERSION})")]
    UnsupportedVersion(u32),
    #[error("invalid provider manifest: {0}")]
    Manifest(String),
    #[error("invalid provider preference: {0}")]
    Preference(String),
    #[error("invalid provider response: {0}")]
    Response(String),
    #[error("invalid provider parameter: {0}")]
    InvalidParameter(String),
}

pub fn validate_manifest(manifest: &ProviderManifest) -> Result<(), ProviderValidationError> {
    if manifest.api_version != PROVIDER_API_VERSION {
        return Err(ProviderValidationError::UnsupportedVersion(
            manifest.api_version,
        ));
    }
    validate_id(&manifest.id)?;
    if manifest.name.trim().is_empty() {
        return Err(ProviderValidationError::Manifest("name is empty".into()));
    }
    if manifest.version.trim().is_empty() {
        return Err(ProviderValidationError::Manifest("version is empty".into()));
    }
    validate_executable_path(&manifest.executable)?;
    if manifest.codecs.is_empty() {
        return Err(ProviderValidationError::Manifest(
            "provider advertises no codecs".into(),
        ));
    }
    for entry in &manifest.codecs {
        if entry.remote_validated && !entry.hardware_validated {
            return Err(ProviderValidationError::Manifest(format!(
                "{} claims remote validation without local hardware validation",
                entry.codec.label()
            )));
        }
    }
    if manifest.codecs.len() > MAX_CODECS {
        return Err(ProviderValidationError::Manifest(format!(
            "too many codecs (maximum {MAX_CODECS})"
        )));
    }
    let mut codecs = HashSet::new();
    let mut parameters = HashSet::new();
    for entry in &manifest.codecs {
        if entry.codec == CodecId::Other {
            return Err(ProviderValidationError::Manifest(
                "the Other codec cannot be registered".into(),
            ));
        }
        if !codecs.insert(entry.codec) {
            return Err(ProviderValidationError::Manifest(format!(
                "duplicate codec {}",
                entry.codec.label()
            )));
        }
        if entry.parameters.len() > MAX_PARAMETERS {
            return Err(ProviderValidationError::Manifest(format!(
                "{} has too many parameters",
                entry.codec.label()
            )));
        }
        for parameter in &entry.parameters {
            if parameter.key().trim().is_empty() {
                return Err(ProviderValidationError::Manifest(
                    "parameter key is empty".into(),
                ));
            }
            if !parameters.insert(parameter.key().to_string()) {
                return Err(ProviderValidationError::Manifest(format!(
                    "duplicate parameter key '{}'; parameter keys must be unique across codecs (use codec-qualified keys)",
                    parameter.key()
                )));
            }
            match parameter {
                ProviderParameter::Choice {
                    options, default, ..
                } => {
                    if options.is_empty() {
                        return Err(ProviderValidationError::Manifest(format!(
                            "parameter '{}' has no options",
                            parameter.key()
                        )));
                    }
                    if !options.iter().any(|v| v == default) {
                        return Err(ProviderValidationError::Manifest(format!(
                            "parameter '{}' default is not an option",
                            parameter.key()
                        )));
                    }
                }
                ProviderParameter::Integer {
                    min, max, default, ..
                } => {
                    if min > max || !(*min..=*max).contains(default) {
                        return Err(ProviderValidationError::Manifest(format!(
                            "parameter '{}' has an invalid range/default",
                            parameter.key()
                        )));
                    }
                }
                ProviderParameter::Boolean { .. } => {}
            }
        }
    }
    if manifest.capabilities.priority && manifest.codecs.len() < 2 {
        return Err(ProviderValidationError::Manifest(
            "priority capability needs at least two codecs".into(),
        ));
    }
    if manifest.capabilities.parameters && !manifest.codecs.iter().any(|c| !c.parameters.is_empty())
    {
        return Err(ProviderValidationError::Manifest(
            "parameters capability is declared but no parameter exists".into(),
        ));
    }
    Ok(())
}

pub fn validate_preference(
    manifest: &ProviderManifest,
    preference: &ProviderPreference,
) -> Result<(), ProviderValidationError> {
    if preference.provider_id != manifest.id {
        return Err(ProviderValidationError::Preference(
            "provider ID does not match the manifest".into(),
        ));
    }
    if preference.device_id.trim().is_empty() {
        return Err(ProviderValidationError::Preference(
            "device ID is empty".into(),
        ));
    }
    if preference.ordered_codecs.is_empty() {
        return Err(ProviderValidationError::Preference(
            "at least one codec is required".into(),
        ));
    }
    if preference.ordered_codecs.len() > manifest.codecs.len() {
        return Err(ProviderValidationError::Preference(
            "more codecs requested than the provider advertises".into(),
        ));
    }
    let mut seen = HashSet::new();
    for codec in &preference.ordered_codecs {
        if !manifest.codecs.iter().any(|c| c.codec == *codec) {
            return Err(ProviderValidationError::Preference(format!(
                "{} is not advertised by this provider",
                codec.label()
            )));
        }
        if !seen.insert(*codec) {
            return Err(ProviderValidationError::Preference(format!(
                "duplicate codec {}",
                codec.label()
            )));
        }
    }
    for (key, value) in &preference.parameters {
        let (owner, parameter) = manifest
            .codecs
            .iter()
            .find_map(|codec| {
                codec
                    .parameters
                    .iter()
                    .find(|p| p.key() == key)
                    .map(|p| (codec.codec, p))
            })
            .ok_or_else(|| {
                ProviderValidationError::Preference(format!("unknown parameter '{key}'"))
            })?;
        if !preference.ordered_codecs.contains(&owner) {
            return Err(ProviderValidationError::Preference(format!(
                "parameter '{key}' belongs to {}, which is not in the ordered codec list",
                owner.label()
            )));
        }
        parameter.validate_value(value)?;
    }
    if !manifest.capabilities.parameters && !preference.parameters.is_empty() {
        return Err(ProviderValidationError::Preference(
            "provider does not expose codec parameters".into(),
        ));
    }
    Ok(())
}

pub fn validate_response(response: &ProviderResponse) -> Result<(), ProviderValidationError> {
    if response.api_version != PROVIDER_API_VERSION {
        return Err(ProviderValidationError::UnsupportedVersion(
            response.api_version,
        ));
    }
    if !response.ok && response.error.as_deref().unwrap_or("").trim().is_empty() {
        return Err(ProviderValidationError::Response(
            "failed response has no error message".into(),
        ));
    }
    if response.verified && response.active_codec.is_none() {
        return Err(ProviderValidationError::Response(
            "verified response has no active codec".into(),
        ));
    }
    if response.verified
        && response
            .verification_method
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
    {
        return Err(ProviderValidationError::Response(
            "verified response has no verification method".into(),
        ));
    }
    if response.active_codec.is_none()
        && (response.active_vendor_id.is_some() || response.active_vendor_codec_id.is_some())
    {
        return Err(ProviderValidationError::Response(
            "vendor IDs were supplied without an active codec".into(),
        ));
    }
    if !response.ok
        && (response.active_codec.is_some()
            || response.verified
            || response.applied_preference.is_some())
    {
        return Err(ProviderValidationError::Response(
            "a failed response cannot report active/verified/applied state".into(),
        ));
    }
    Ok(())
}

pub fn validate_response_for_manifest(
    manifest: &ProviderManifest,
    response: &ProviderResponse,
    action: ProviderAction,
) -> Result<(), ProviderValidationError> {
    validate_response(response)?;
    if let Some(codec) = response.active_codec {
        if !manifest.codecs.iter().any(|c| c.codec == codec) {
            return Err(ProviderValidationError::Response(format!(
                "provider reported {} but did not advertise it",
                codec.label()
            )));
        }
        if !manifest.capabilities.active_codec {
            return Err(ProviderValidationError::Response(
                "provider reported active codec without the active_codec capability".into(),
            ));
        }
    }
    if response.verified && action != ProviderAction::Verify {
        return Err(ProviderValidationError::Response(
            "verified=true is only accepted for a verify action".into(),
        ));
    }
    if response.rollback_token.is_some() && !manifest.capabilities.rollback {
        return Err(ProviderValidationError::Response(
            "provider returned a rollback token without rollback capability".into(),
        ));
    }
    if action == ProviderAction::Apply
        && response.ok
        && response.rollback_token.is_none()
        && !manifest.capabilities.rollback
    {
        // Tokenless rollback may be valid only if the provider explicitly declares
        // rollback capability. Otherwise the UI must not promise reversibility.
        return Ok(());
    }
    if let Some(preference) = &response.applied_preference {
        validate_preference(manifest, preference)?;
        if action != ProviderAction::Apply {
            return Err(ProviderValidationError::Response(
                "applied_preference is only accepted for an apply action".into(),
            ));
        }
    }
    if action == ProviderAction::Apply && response.ok && response.applied_preference.is_none() {
        return Err(ProviderValidationError::Response(
            "successful apply did not echo the applied preference".into(),
        ));
    }
    Ok(())
}

fn validate_id(id: &str) -> Result<(), ProviderValidationError> {
    if id.is_empty()
        || id.len() > 96
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    {
        return Err(ProviderValidationError::Manifest(
            "id must contain only letters, digits, '.', '-' or '_'".into(),
        ));
    }
    Ok(())
}

fn validate_executable_path(path: &str) -> Result<(), ProviderValidationError> {
    let p = std::path::Path::new(path);
    if p.is_absolute()
        || path.trim().is_empty()
        || p.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(ProviderValidationError::Manifest(
            "executable must be a relative path inside the provider directory".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> ProviderManifest {
        ProviderManifest {
            api_version: PROVIDER_API_VERSION,
            id: "vendor.ldac".into(),
            name: "LDAC provider".into(),
            version: "1.0.0".into(),
            executable: "provider.exe".into(),
            publisher: "Vendor".into(),
            homepage: String::new(),
            description: String::new(),
            requires_elevation: true,
            stack: ProviderStack::ReplacementA2dpDriver,
            capabilities: ProviderCapabilities {
                preferred_codec: true,
                priority: true,
                parameters: true,
                active_codec: true,
                rollback: true,
                dry_run: true,
            },
            codecs: vec![
                ProviderCodec {
                    codec: CodecId::Ldac,
                    notes: "LDAC".into(),
                    hardware_validated: true,
                    remote_validated: false,
                    parameters: vec![ProviderParameter::Choice {
                        key: "ldac_quality".into(),
                        label: "Quality".into(),
                        options: vec!["330".into(), "660".into(), "990".into()],
                        default: "990".into(),
                        note: String::new(),
                    }],
                },
                ProviderCodec {
                    codec: CodecId::AptXHd,
                    notes: String::new(),
                    hardware_validated: true,
                    remote_validated: false,
                    parameters: vec![],
                },
                ProviderCodec {
                    codec: CodecId::AptXAdaptive,
                    notes: String::new(),
                    hardware_validated: true,
                    remote_validated: false,
                    parameters: vec![],
                },
                ProviderCodec {
                    codec: CodecId::Sbc,
                    notes: String::new(),
                    hardware_validated: true,
                    remote_validated: true,
                    parameters: vec![],
                },
            ],
        }
    }

    #[test]
    fn valid_manifest_accepts_ldac_hd_adaptive() {
        validate_manifest(&manifest()).unwrap();
    }

    #[test]
    fn manifest_rejects_traversal_and_duplicate_codecs() {
        let mut m = manifest();
        m.executable = "../provider.exe".into();
        assert!(validate_manifest(&m).is_err());
        let mut m = manifest();
        m.codecs.push(m.codecs[0].clone());
        assert!(validate_manifest(&m).is_err());
    }

    #[test]
    fn manifest_rejects_ambiguous_parameter_keys_across_codecs() {
        let mut m = manifest();
        m.codecs[1].parameters = vec![ProviderParameter::Choice {
            key: "ldac_quality".into(),
            label: "Ambiguous".into(),
            options: vec!["1".into()],
            default: "1".into(),
            note: String::new(),
        }];
        assert!(validate_manifest(&m).is_err());
    }

    #[test]
    fn preference_rejects_unadvertised_codec_and_bad_parameter() {
        let m = manifest();
        let pref = ProviderPreference {
            provider_id: m.id.clone(),
            device_id: "dev".into(),
            ordered_codecs: vec![CodecId::Lc3],
            parameters: BTreeMap::new(),
            dry_run: false,
        };
        assert!(validate_preference(&m, &pref).is_err());
        let pref = ProviderPreference {
            provider_id: m.id.clone(),
            device_id: "dev".into(),
            ordered_codecs: vec![CodecId::Ldac, CodecId::Sbc],
            parameters: BTreeMap::from([("ldac_quality".into(), "1000".into())]),
            dry_run: false,
        };
        assert!(validate_preference(&m, &pref).is_err());
    }

    #[test]
    fn successful_apply_must_echo_preference() {
        let m = manifest();
        let response = ProviderResponse {
            api_version: 1,
            ok: true,
            message: String::new(),
            error: None,
            active_codec: None,
            active_vendor_id: None,
            active_vendor_codec_id: None,
            verified: false,
            verification_method: None,
            rollback_token: None,
            applied_preference: None,
            requires_reconnect: true,
        };
        assert!(validate_response_for_manifest(&m, &response, ProviderAction::Apply).is_err());
    }

    #[test]
    fn response_cannot_report_an_unadvertised_codec() {
        let m = manifest();
        let response = ProviderResponse {
            api_version: 1,
            ok: true,
            message: String::new(),
            error: None,
            active_codec: Some(CodecId::Lc3),
            active_vendor_id: None,
            active_vendor_codec_id: None,
            verified: false,
            verification_method: None,
            rollback_token: None,
            applied_preference: None,
            requires_reconnect: false,
        };
        assert!(validate_response_for_manifest(&m, &response, ProviderAction::Verify).is_err());
    }

    #[test]
    fn failed_response_requires_error() {
        let response = ProviderResponse {
            api_version: 1,
            ok: false,
            message: String::new(),
            error: None,
            active_codec: None,
            active_vendor_id: None,
            active_vendor_codec_id: None,
            verified: false,
            verification_method: None,
            rollback_token: None,
            applied_preference: None,
            requires_reconnect: false,
        };
        assert!(validate_response(&response).is_err());
    }
}
