//! Advanced A2DP codec provider discovery and invocation.
//!
//! A provider is a separately distributed executable (and, for real advanced
//! codecs, usually a driver/service) that owns the encoder and transport.
//! Bluu owns the policy, validation, preference UI, audit trail, and rollback
//! handshake. It does not bundle proprietary codec binaries.

pub mod discover;
pub mod prefs;
pub mod run;

pub use discover::{discover, provider_dirs};
pub use prefs::{load_observation, load_preferences};
pub use run::{run_provider, ProviderRunContext};

use bluu_core::{
    backend::BackendError,
    provider::{ProviderAction, ProviderDevice, ProviderPreference, ProviderRequest},
    ProviderOutcome, ProviderRecord,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub fn outcome_from_response(
    record: &ProviderRecord,
    action: ProviderAction,
    response: bluu_core::ProviderResponse,
    device: Option<&ProviderDevice>,
) -> Result<ProviderOutcome, BackendError> {
    ProviderOutcome::from_response(record.manifest.id.clone(), action, response, device)
        .map_err(|e| BackendError::Provider(e.to_string()))
}

pub fn request(
    action: ProviderAction,
    device: Option<&ProviderDevice>,
    preference: Option<&ProviderPreference>,
    rollback_token: Option<String>,
) -> ProviderRequest {
    ProviderRequest {
        api_version: bluu_core::provider::PROVIDER_API_VERSION,
        action,
        device: device.cloned(),
        preference: preference.cloned(),
        rollback_token,
    }
}

pub fn operation(
    request: &bluu_core::model::ProviderOperationRequest,
    snapshot: Option<&bluu_core::Snapshot>,
) -> Result<bluu_core::model::ProviderOperationOutcome, BackendError> {
    let records = discover();
    let record = records
        .iter()
        .find(|p| p.manifest.id == request.provider_id)
        .ok_or_else(|| {
            BackendError::Provider(format!(
                "provider '{}' is not installed",
                request.provider_id
            ))
        })?;
    let snapshot =
        snapshot.ok_or_else(|| BackendError::Provider("scan has not completed".into()))?;
    let device = request
        .device_id
        .as_ref()
        .and_then(|id| snapshot.device(id))
        .map(|d| ProviderDevice {
            id: d.id.clone(),
            name: d.name.clone(),
            address: d.address.clone(),
            connected: d.connected,
        });
    if matches!(request.operation, ProviderAction::Apply)
        && device.as_ref().is_none_or(|d| !d.connected)
    {
        return Err(BackendError::Provider(
            "the selected Bluetooth device is not connected".into(),
        ));
    }
    if request.operation == ProviderAction::Apply {
        let Some(preference) = request.preference.as_ref() else {
            return Err(BackendError::InvalidInput(
                "apply requires a preference".into(),
            ));
        };
        bluu_core::provider::validate_preference(&record.manifest, preference)
            .map_err(|e| BackendError::Provider(e.to_string()))?;
    }
    let effective_rollback = request.rollback_token.clone().or_else(|| {
        request
            .device_id
            .as_ref()
            .and_then(|device_id| prefs::rollback_token(&request.provider_id, device_id))
    });
    let response = run_provider(&ProviderRunContext {
        record,
        action: request.operation,
        device: device.as_ref(),
        preference: request.preference.as_ref(),
        rollback_token: effective_rollback,
        timeout: std::time::Duration::from_secs(120),
    })?;
    bluu_core::provider::validate_response_for_manifest(
        &record.manifest,
        &response,
        request.operation,
    )
    .map_err(|e| BackendError::Provider(e.to_string()))?;
    if let Some(applied) = response.applied_preference.as_ref() {
        if applied.provider_id != request.provider_id
            || request.device_id.as_deref() != Some(applied.device_id.as_str())
        {
            return Err(BackendError::Provider(
                "provider returned an applied preference for a different provider/device".into(),
            ));
        }
    }
    let outcome =
        outcome_from_response(record, request.operation, response.clone(), device.as_ref())?;
    prefs::record_operation(&outcome, request, &response)?;
    Ok(bluu_core::model::ProviderOperationOutcome {
        provider_id: outcome.provider_id,
        operation: outcome.action,
        accepted: response.ok,
        message: response.message,
        requires_reconnect: response.requires_reconnect,
        rollback_token: outcome.response.rollback_token,
        observation: outcome.observation,
        applied_preference: outcome.response.applied_preference,
    })
}

pub fn default_parameters(
    record: &ProviderRecord,
    codec: bluu_core::CodecId,
) -> BTreeMap<String, String> {
    record
        .manifest
        .codecs
        .iter()
        .find(|c| c.codec == codec)
        .map(|c| {
            c.parameters
                .iter()
                .map(|p| (p.key().to_string(), p.default_value()))
                .collect()
        })
        .unwrap_or_default()
}

/// Explicit filesystem installation only. Bluu never downloads or silently
/// installs a provider/driver.
pub fn install_local_provider(source: &Path) -> Result<ProviderRecord, BackendError> {
    let source = source
        .canonicalize()
        .map_err(|e| BackendError::Provider(format!("provider source: {e}")))?;
    if !source.is_dir() {
        return Err(BackendError::InvalidInput(
            "choose a provider directory containing provider.json".into(),
        ));
    }
    let manifest_bytes = std::fs::read(source.join("provider.json"))
        .map_err(|e| BackendError::Provider(format!("provider root: {e}")))?;
    let manifest: bluu_core::ProviderManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|e| BackendError::Provider(format!("provider manifest: {e}")))?;
    bluu_core::provider::validate_manifest(&manifest)
        .map_err(|e| BackendError::Provider(e.to_string()))?;
    if !source.join(&manifest.executable).is_file() {
        return Err(BackendError::Provider(
            "provider executable is missing from the source directory".into(),
        ));
    }
    let destination = bluu_core::settings::provider_dir().join(&manifest.id);
    if destination.exists() {
        return Err(BackendError::Provider(format!(
            "provider '{}' is already installed; remove it first",
            manifest.id
        )));
    }
    discover::copy_provider_tree(&source, &destination)?;
    let record = discover()
        .into_iter()
        .find(|p| p.manifest.id == manifest.id)
        .ok_or_else(|| BackendError::Provider("installed provider was not discoverable".into()))?;
    if !record.is_usable() {
        let _ = std::fs::remove_dir_all(&destination);
        return Err(BackendError::Provider(format!(
            "installed provider failed validation: {}",
            record.issues.join("; ")
        )));
    }
    Ok(record)
}

pub fn uninstall_provider(id: &str) -> Result<(), BackendError> {
    let record = discover()
        .into_iter()
        .find(|p| p.manifest.id == id)
        .ok_or_else(|| BackendError::Provider("provider is not installed".into()))?;
    let dir = PathBuf::from(&record.directory);
    let root = bluu_core::settings::provider_dir();
    let canonical = dir
        .canonicalize()
        .map_err(|e| BackendError::Provider(format!("provider directory: {e}")))?;
    let root = root.canonicalize().unwrap_or(root);
    if !canonical.starts_with(&root) {
        return Err(BackendError::Provider(
            "refusing to remove a provider outside the Bluu provider directory".into(),
        ));
    }
    std::fs::remove_dir_all(&canonical)
        .map_err(|e| BackendError::Provider(format!("provider removal: {e}")))?;
    Ok(())
}

pub fn demo_operation(
    request: &bluu_core::model::ProviderOperationRequest,
) -> Result<bluu_core::model::ProviderOperationOutcome, BackendError> {
    if request.provider_id != "demo.ldac" {
        return Err(BackendError::Provider(
            "demo backend only provides demo.ldac".into(),
        ));
    }
    let preference = request.preference.clone();
    match request.operation {
        ProviderAction::Apply => {
            let Some(preference) = preference else {
                return Err(BackendError::InvalidInput(
                    "apply requires a preference".into(),
                ));
            };
            if !preference
                .ordered_codecs
                .contains(&bluu_core::CodecId::Ldac)
            {
                return Err(BackendError::Provider(
                    "demo preference must include LDAC".into(),
                ));
            }
            Ok(bluu_core::model::ProviderOperationOutcome {
                provider_id: request.provider_id.clone(),
                operation: request.operation,
                accepted: true,
                message: "DEMO only: preference accepted; no transport was changed.".into(),
                requires_reconnect: true,
                rollback_token: Some("demo-rollback-token".into()),
                observation: None,
                applied_preference: Some(preference),
            })
        }
        ProviderAction::Rollback | ProviderAction::Describe => {
            Ok(bluu_core::model::ProviderOperationOutcome {
                provider_id: request.provider_id.clone(),
                operation: request.operation,
                accepted: true,
                message: "DEMO only: operation acknowledged.".into(),
                requires_reconnect: request.operation == ProviderAction::Rollback,
                rollback_token: None,
                observation: None,
                applied_preference: None,
            })
        }
        ProviderAction::Verify => Ok(bluu_core::model::ProviderOperationOutcome {
            provider_id: request.provider_id.clone(),
            operation: request.operation,
            accepted: true,
            message: "DEMO only: no real codec was verified.".into(),
            requires_reconnect: false,
            rollback_token: None,
            observation: None,
            applied_preference: None,
        }),
    }
}
