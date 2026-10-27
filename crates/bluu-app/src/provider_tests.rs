//! Tests for provider-order/parameter state transitions. These exercise the same
//! pure helpers used by the UI without opening a window.

use std::collections::BTreeMap;

use bluu_core::{CodecId, ProviderParameter, ProviderRecord};

use crate::provider_defaults::{defaults_for_order, operation_label};

fn record() -> ProviderRecord {
    serde_json::from_value(serde_json::json!({
        "manifest": {
            "api_version": 1,
            "id": "demo.ldac",
            "name": "Demo",
            "version": "1",
            "executable": "provider.exe",
            "stack": "simulated",
            "capabilities": {"preferred_codec": true, "priority": true, "parameters": true, "active_codec": true, "rollback": true},
            "codecs": [
                {"codec": "ldac", "parameters": [
                    {"kind": "choice", "key": "quality", "label": "Q", "options": ["330", "990"], "default": "990"}
                ]},
                {"codec": "aptx_hd", "parameters": []}
            ]
        },
        "manifest_path": "demo/provider.json",
        "directory": "demo",
        "status": "ready",
        "issues": [],
        "executable_sha256": "fixture",
        "simulated": true
    }))
    .unwrap()
}

#[test]
fn defaults_follow_order_and_include_codec_parameters() {
    let record = record();
    let defaults = defaults_for_order(&record, &[CodecId::Ldac, CodecId::AptXHd]);
    assert_eq!(defaults.get("quality"), Some(&"990".to_string()));
}

#[test]
fn operation_labels_are_user_readable() {
    assert_eq!(
        operation_label(bluu_core::ProviderAction::Apply),
        "Applying preference"
    );
}

#[test]
fn parameter_defaults_validate() {
    let parameter = ProviderParameter::Choice {
        key: "q".into(),
        label: "Q".into(),
        options: vec!["1".into(), "2".into()],
        default: "1".into(),
        note: String::new(),
    };
    assert!(parameter.validate_value("2").is_ok());
    assert!(parameter.validate_value("3").is_err());
    let map = BTreeMap::from([("q".to_string(), "2".to_string())]);
    assert_eq!(map.get("q").map(String::as_str), Some("2"));
}
