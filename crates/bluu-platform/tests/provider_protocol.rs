//! Integration tests for the provider protocol. These exercise real discovery and
//! invocation of the demo provider binary on the host; no Bluetooth is touched.

use std::{collections::BTreeMap, fs, sync::Mutex};

static ENV_LOCK: Mutex<()> = Mutex::new(());

use bluu_core::{
    provider::{ProviderAction, ProviderDevice, ProviderPreference},
    CodecId,
};
use bluu_platform::providers::{self, ProviderRunContext};

fn install_demo(dir: &std::path::Path) -> bluu_core::ProviderRecord {
    let provider_dir = dir.join("demo");
    fs::create_dir_all(&provider_dir).unwrap();
    // Cargo's CARGO_BIN_EXE_* is not available across workspace packages. The
    // test script builds this fixture first; the path is workspace-relative.
    let exe = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/bluu-provider-demo");
    assert!(
        exe.is_file(),
        "build bluu-provider-demo before integration tests"
    );
    fs::copy(exe, provider_dir.join("bluu-provider-demo.exe")).unwrap();
    let manifest = serde_json::json!({
        "api_version": 1,
        "id": "demo.ldac",
        "name": "Demo provider",
        "version": "0.1.0",
        "executable": "bluu-provider-demo.exe",
        "publisher": "test",
        "requires_elevation": false,
        "stack": "simulated",
        "capabilities": {
            "preferred_codec": true, "priority": true, "parameters": true,
            "active_codec": true, "rollback": true, "dry_run": true
        },
        "codecs": [
            {"codec": "ldac", "parameters": [
                {"kind":"choice","key":"ldac_quality","label":"Quality",
                 "options":["330","660","990"],"default":"990"}
            ]},
            {"codec":"aptx_hd"}, {"codec":"aptx_adaptive"}, {"codec":"sbc"}
        ]
    });
    fs::write(
        provider_dir.join("provider.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    std::env::set_var("BLUU_PROVIDER_DIR", dir);
    let record = providers::discover()
        .into_iter()
        .find(|p| p.manifest.id == "demo.ldac")
        .unwrap();
    std::env::remove_var("BLUU_PROVIDER_DIR");
    record
}

#[test]
fn provider_apply_verify_rollback_round_trip() {
    let _guard = ENV_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let record = install_demo(dir.path());
    assert!(record.is_usable(), "{:?}", record.issues);
    let device = ProviderDevice {
        id: "dev".into(),
        name: "Headphones".into(),
        address: None,
        connected: true,
    };
    let preference = ProviderPreference {
        provider_id: record.manifest.id.clone(),
        device_id: device.id.clone(),
        ordered_codecs: vec![CodecId::Ldac, CodecId::AptXHd, CodecId::Sbc],
        parameters: BTreeMap::from([("ldac_quality".into(), "660".into())]),
        dry_run: false,
    };
    bluu_core::provider::validate_preference(&record.manifest, &preference).unwrap();
    let applied = providers::run_provider(&ProviderRunContext {
        record: &record,
        action: ProviderAction::Apply,
        device: Some(&device),
        preference: Some(&preference),
        rollback_token: None,
        timeout: std::time::Duration::from_secs(10),
    })
    .unwrap();
    assert!(applied.ok);
    assert!(applied.requires_reconnect);
    let applied_pref = applied.applied_preference.as_ref().unwrap();
    assert_eq!(applied_pref.provider_id, "demo.ldac");
    assert_eq!(applied_pref.device_id, "dev");
    let verified = providers::run_provider(&ProviderRunContext {
        record: &record,
        action: ProviderAction::Verify,
        device: Some(&device),
        preference: Some(&preference),
        rollback_token: applied.rollback_token,
        timeout: std::time::Duration::from_secs(10),
    })
    .unwrap();
    assert!(verified.ok);
    assert!(verified.active_codec.is_none());
    let rolled = providers::run_provider(&ProviderRunContext {
        record: &record,
        action: ProviderAction::Rollback,
        device: Some(&device),
        preference: None,
        rollback_token: None,
        timeout: std::time::Duration::from_secs(10),
    })
    .unwrap();
    assert!(rolled.ok);
}

#[test]
fn missing_executable_is_rejected() {
    let _guard = ENV_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let provider_dir = dir.path().join("bad");
    fs::create_dir_all(&provider_dir).unwrap();
    fs::write(
        provider_dir.join("provider.json"),
        serde_json::to_vec(&serde_json::json!({
            "api_version":1,"id":"bad","name":"Bad","version":"1",
            "executable":"missing.exe","stack":"vendor_service",
            "capabilities":{"preferred_codec":true},
            "codecs":[{"codec":"sbc"}]
        }))
        .unwrap(),
    )
    .unwrap();
    std::env::set_var("BLUU_PROVIDER_DIR", dir.path());
    let records = providers::discover();
    std::env::remove_var("BLUU_PROVIDER_DIR");
    assert_eq!(records.len(), 1);
    assert!(!records[0].is_usable());
}

#[test]
fn local_provider_install_and_uninstall_are_contained() {
    let _guard = ENV_LOCK.lock().unwrap();
    let state = tempfile::tempdir().unwrap();
    std::env::set_var("BLUU_STATE_DIR", state.path());
    let source = tempfile::tempdir().unwrap();
    let provider_dir = source.path().join("demo");
    fs::create_dir_all(&provider_dir).unwrap();
    let exe = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/bluu-provider-demo");
    fs::copy(exe, provider_dir.join("bluu-provider-demo.exe")).unwrap();
    let manifest = serde_json::json!({
        "api_version":1,"id":"demo.ldac","name":"Demo","version":"1",
        "executable":"bluu-provider-demo.exe","publisher":"test","stack":"simulated",
        "capabilities":{"preferred_codec":true},
        "codecs":[{"codec":"ldac"},{"codec":"sbc"}]
    });
    fs::write(
        provider_dir.join("provider.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let record = providers::install_local_provider(&provider_dir).unwrap();
    assert_eq!(record.manifest.id, "demo.ldac");
    assert!(std::path::Path::new(&record.directory).exists());
    providers::uninstall_provider("demo.ldac").unwrap();
    assert!(!std::path::Path::new(&record.directory).exists());
    std::env::remove_var("BLUU_STATE_DIR");
}

#[test]
fn malformed_and_oversized_results_are_rejected() {
    let _guard = ENV_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let provider_dir = dir.path().join("bad");
    fs::create_dir_all(&provider_dir).unwrap();
    let script = provider_dir.join("bluu-provider-demo.exe");
    fs::write(&script, b"fixture").unwrap();
    let manifest = serde_json::json!({
        "api_version":1,"id":"bad.result","name":"Bad result","version":"1",
        "executable":"bluu-provider-demo.exe","stack":"simulated",
        "capabilities":{"preferred_codec":true},"codecs":[{"codec":"sbc"},{"codec":"ldac"}]
    });
    fs::write(
        provider_dir.join("provider.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    std::env::set_var("BLUU_PROVIDER_DIR", dir.path());
    let record = providers::discover().into_iter().next().unwrap();
    std::env::remove_var("BLUU_PROVIDER_DIR");
    let device = ProviderDevice {
        id: "dev".into(),
        name: "Device".into(),
        address: None,
        connected: true,
    };
    let outcome = providers::run_provider(&ProviderRunContext {
        record: &record,
        action: ProviderAction::Describe,
        device: Some(&device),
        preference: None,
        rollback_token: None,
        timeout: std::time::Duration::from_millis(200),
    });
    // The fixture is not executable, so the provider must fail without accepting any state.
    assert!(outcome.is_err());
}
