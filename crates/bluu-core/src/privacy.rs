use serde::{Deserialize, Serialize};

use crate::model::Snapshot;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrivacyOptions {
    pub redact_device_names: bool,
    pub redact_addresses: bool,
    pub redact_endpoint_ids: bool,
    pub redact_paths: bool,
}

impl Default for PrivacyOptions {
    fn default() -> Self {
        Self {
            redact_device_names: true,
            redact_addresses: true,
            redact_endpoint_ids: true,
            redact_paths: true,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct DiagnosticExport {
    pub schema: &'static str,
    pub generated_at: chrono::DateTime<chrono::Utc>,
    pub application: String,
    pub notice: String,
    pub privacy: PrivacyOptions,
    pub snapshot: Snapshot,
}

impl DiagnosticExport {
    pub fn redacted(snapshot: Snapshot) -> Self {
        Self::with_options(snapshot, PrivacyOptions::default())
    }

    pub fn with_options(snapshot: Snapshot, options: PrivacyOptions) -> Self {
        let mut snapshot = snapshot;
        let mut device_aliases = std::collections::BTreeMap::new();
        for (index, endpoint) in snapshot.endpoints.iter_mut().enumerate() {
            if options.redact_device_names {
                if endpoint.device_name.is_some() {
                    endpoint.device_name = Some(format!("Bluetooth audio device {}", index + 1));
                }
                endpoint.friendly_name = format!(
                    "Audio endpoint {} · {} · {}",
                    index + 1,
                    match endpoint.flow {
                        crate::model::Flow::Render => "output",
                        crate::model::Flow::Capture => "input",
                    },
                    endpoint.state.label()
                );
            }
            if options.redact_endpoint_ids {
                endpoint.id = format!("endpoint-{index}");
            }
            if options.redact_addresses {
                endpoint.container_id = endpoint
                    .container_id
                    .as_ref()
                    .map(|_| format!("container-{}", index + 1));
            }
        }
        for (index, device) in snapshot.devices.iter_mut().enumerate() {
            let alias = format!("Bluetooth device {}", index + 1);
            let old_id = device.id.clone();
            device_aliases.insert(old_id.clone(), alias.clone());
            if options.redact_device_names {
                device.name = alias;
            }
            if options.redact_addresses {
                device.address = None;
                device.container_id = device
                    .container_id
                    .as_ref()
                    .map(|_| format!("container-{}", index + 1));
            }
            if options.redact_endpoint_ids {
                device.endpoint_ids = device
                    .endpoint_ids
                    .iter()
                    .enumerate()
                    .map(|(i, _)| format!("endpoint-{i}"))
                    .collect();
            }
            if options.redact_device_names {
                device.id = format!("device-{}", index + 1);
            } else if options.redact_endpoint_ids {
                device.id = old_id;
            }
            if options.redact_paths {
                device.backend_note = device.backend_note.as_ref().map(|s| redact_text(s));
            }
        }
        if let Some(adapter) = snapshot.adapter.as_mut() {
            if options.redact_device_names {
                adapter.name = "Bluetooth adapter".into();
            }
            if options.redact_addresses {
                adapter.address = None;
                adapter.id = adapter.id.as_ref().map(|_| "adapter-1".into());
            }
            if options.redact_device_names {
                adapter.manufacturer = adapter.manufacturer.as_ref().map(|_| "<redacted>".into());
            }
        }
        if let Some(active) = snapshot.active_codec.as_mut() {
            if options.redact_device_names {
                active.device_id = active
                    .device_id
                    .as_ref()
                    .and_then(|id| device_aliases.get(id).cloned())
                    .or_else(|| active.device_id.as_ref().map(|_| "device-unknown".into()));
            }
        }
        if options.redact_paths {
            snapshot.warnings = snapshot.warnings.iter().map(|s| redact_text(s)).collect();
            snapshot.notes = snapshot.notes.iter().map(|s| redact_text(s)).collect();
        }
        Self {
            schema: "bluu.diagnostics.v1",
            generated_at: chrono::Utc::now(),
            application: format!("{} {}", crate::APP_NAME, crate::VERSION),
            notice: "Local export. Review before sharing. Bluetooth addresses, device names, endpoint IDs, and paths are redacted by default. The snapshot does not contain usernames, computer names, audio, packet captures, or credentials.".into(),
            privacy: options,
            snapshot,
        }
    }

    pub fn to_pretty_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

pub fn redact_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for token in input.split_inclusive(char::is_whitespace) {
        let trimmed = token.trim_end();
        if is_windows_path(trimmed) || looks_like_address(trimmed) {
            out.push_str("<redacted>");
            out.push_str(&token[trimmed.len()..]);
        } else {
            out.push_str(token);
        }
    }
    out
}

fn is_windows_path(token: &str) -> bool {
    let bytes = token.as_bytes();
    bytes.len() > 2 && bytes[1] == b':' && (bytes[2] == b'\\' || bytes[2] == b'/')
        || token.starts_with("\\\\")
}

fn looks_like_address(token: &str) -> bool {
    let groups: Vec<_> = token.split([':', '-']).collect();
    groups.len() == 6
        && groups
            .iter()
            .all(|g| g.len() == 2 && g.chars().all(|c| c.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SystemInfo;

    #[test]
    fn default_export_removes_identifier_fields() {
        let mut snapshot = Snapshot::empty(SystemInfo::default());
        snapshot.devices.push(crate::model::BluetoothAudioDevice {
            id: "BTHENUM\\SECRET".into(),
            name: "Josh's Headphones".into(),
            address: Some(crate::model::BluetoothAddress([1, 2, 3, 4, 5, 6])),
            container_id: Some("secret-container".into()),
            connected: true,
            paired: true,
            transport: crate::model::Transport::Classic,
            class_of_device: None,
            category: None,
            profiles: vec![],
            endpoint_ids: vec!["secret".into()],
            association: Default::default(),
            last_seen: None,
            backend_note: None,
        });
        let export = DiagnosticExport::redacted(snapshot);
        let json = export.to_pretty_json().unwrap();
        assert!(!json.contains("Josh's Headphones"));
        assert!(!json.contains("BTHENUM"));
        assert!(!json.contains("01:02:03:04:05:06"));
        assert!(json.contains("Bluetooth device 1"));
    }

    #[test]
    fn text_redaction_catches_paths_and_addresses() {
        let text = "path C:\\Users\\josh\\file.log from AA:BB:CC:DD:EE:FF";
        let redacted = redact_text(text);
        assert!(!redacted.contains("josh"));
        assert!(!redacted.contains("AA:BB:CC:DD:EE:FF"));
    }
}
