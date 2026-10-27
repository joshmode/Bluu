use std::{fs, path::PathBuf};

use bluu_core::{
    backend::BackendError,
    model::{ActiveCodecObservation, ProviderOperationRequest},
    provider::{ProviderPreference, ProviderResponse},
    settings::provider_state_path,
};

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct PreferencesFile {
    schema: u32,
    preferences: Vec<ProviderPreference>,
    rollback_tokens: Vec<StoredToken>,
    #[serde(default)]
    observations: Vec<StoredObservation>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct StoredToken {
    provider_id: String,
    device_id: String,
    token: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct StoredObservation {
    provider_id: String,
    device_id: String,
    observation: ActiveCodecObservation,
}

fn path() -> PathBuf {
    provider_state_path()
}

pub fn load_preferences() -> Vec<ProviderPreference> {
    let mut preferences = load().preferences;
    // Provider packages can be removed outside Bluu's UI. Keep only persisted
    // preferences backed by a provider that is still installed and usable.
    let installed: std::collections::HashSet<String> = super::discover()
        .into_iter()
        .map(|p| p.manifest.id)
        .collect();
    preferences.retain(|pref| installed.contains(&pref.provider_id));
    preferences
}

pub fn rollback_token(provider_id: &str, device_id: &str) -> Option<String> {
    load()
        .rollback_tokens
        .into_iter()
        .find(|t| t.provider_id == provider_id && t.device_id == device_id)
        .map(|t| t.token)
}

pub fn load_observation() -> Option<ActiveCodecObservation> {
    let now = chrono::Utc::now();
    load()
        .observations
        .into_iter()
        .map(|stored| stored.observation)
        .filter(|observation| {
            let age = now.signed_duration_since(observation.observed_at);
            age >= chrono::Duration::zero() && age <= chrono::Duration::minutes(10)
        })
        .max_by_key(|observation| observation.observed_at)
}

pub fn record_operation(
    outcome: &bluu_core::ProviderOutcome,
    request: &ProviderOperationRequest,
    response: &ProviderResponse,
) -> Result<(), BackendError> {
    let mut state = load();
    let device_id = request
        .device_id
        .clone()
        .or_else(|| {
            response
                .applied_preference
                .as_ref()
                .map(|p| p.device_id.clone())
        })
        .unwrap_or_default();
    if response.ok {
        if let Some(preference) = response.applied_preference.clone() {
            state.preferences.retain(|p| {
                !(p.provider_id == preference.provider_id && p.device_id == preference.device_id)
            });
            state.preferences.push(preference);
        }
    }
    if outcome.action == bluu_core::ProviderAction::Rollback {
        state
            .preferences
            .retain(|p| !(p.provider_id == request.provider_id && p.device_id == device_id));
        state
            .rollback_tokens
            .retain(|t| !(t.provider_id == request.provider_id && t.device_id == device_id));
        state
            .observations
            .retain(|o| !(o.provider_id == request.provider_id && o.device_id == device_id));
    } else if response.ok {
        if let Some(token) = response.rollback_token.clone() {
            state
                .rollback_tokens
                .retain(|t| !(t.provider_id == request.provider_id && t.device_id == device_id));
            state.rollback_tokens.push(StoredToken {
                provider_id: request.provider_id.clone(),
                device_id: device_id.clone(),
                token,
            });
        }
    }
    if response.ok {
        if let Some(observation) = outcome.observation.as_ref() {
            state
                .observations
                .retain(|o| !(o.provider_id == request.provider_id && o.device_id == device_id));
            state.observations.push(StoredObservation {
                provider_id: request.provider_id.clone(),
                device_id,
                observation: observation.clone(),
            });
        }
    }
    save(&state)
}

fn load() -> PreferencesFile {
    let path = path();
    fs::read(&path)
        .ok()
        .and_then(|b| {
            if b.len() > 512 * 1024 {
                return None;
            }
            serde_json::from_slice(&b).ok()
        })
        .unwrap_or(PreferencesFile {
            schema: 1,
            preferences: vec![],
            rollback_tokens: vec![],
            observations: vec![],
        })
}

fn save(state: &PreferencesFile) -> Result<(), BackendError> {
    let path = path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| BackendError::Provider(format!("provider state directory: {e}")))?;
    }
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|e| BackendError::Provider(format!("provider state JSON: {e}")))?;
    fs::write(&tmp, bytes)
        .map_err(|e| BackendError::Provider(format!("provider state write: {e}")))?;
    fs::rename(&tmp, &path)
        .map_err(|e| BackendError::Provider(format!("provider state replace: {e}")))?;
    Ok(())
}
