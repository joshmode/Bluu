//! End-to-end smoke check for a provider directory. It discovers manifests, selects
//! the demo provider (or the first usable provider), invokes Describe/Apply/Verify/
//! Rollback, validates responses, and prints a redacted summary.

use std::collections::BTreeMap;

use bluu_core::{
    provider::{ProviderAction, ProviderDevice, ProviderPreference},
    CodecId,
};
use bluu_platform::providers::{self, ProviderRunContext};

fn main() {
    let records = providers::discover();
    if records.is_empty() {
        eprintln!("no providers found; set BLUU_PROVIDER_DIR or install one");
        std::process::exit(2);
    }
    let record = records
        .iter()
        .find(|p| p.manifest.id == "demo.ldac")
        .or_else(|| records.iter().find(|p| p.is_usable()))
        .expect("a usable provider");
    println!(
        "provider: {} {} status={:?} hash={}",
        record.manifest.id,
        record.manifest.version,
        record.status,
        record.executable_sha256.as_deref().unwrap_or("none")
    );
    if !record.is_usable() {
        eprintln!("provider is not usable: {:?}", record.issues);
        std::process::exit(3);
    }
    let device = ProviderDevice {
        id: "demo-headphones".into(),
        name: "Studio Headphones".into(),
        address: None,
        connected: true,
    };
    let check =
        |action: ProviderAction, preference: Option<ProviderPreference>, token: Option<String>| {
            let response = providers::run_provider(&ProviderRunContext {
                record,
                action,
                device: Some(&device),
                preference: preference.as_ref(),
                rollback_token: token,
                timeout: std::time::Duration::from_secs(10),
            })
            .expect("provider response");
            println!(
                "{:?}: ok={} active={:?} verified={} reconnect={} error={:?}",
                action,
                response.ok,
                response.active_codec,
                response.verified,
                response.requires_reconnect,
                response.error
            );
            response
        };
    check(ProviderAction::Describe, None, None);
    let preference = ProviderPreference {
        provider_id: record.manifest.id.clone(),
        device_id: device.id.clone(),
        ordered_codecs: vec![CodecId::Ldac, CodecId::AptXHd, CodecId::Sbc],
        parameters: BTreeMap::from([("ldac_quality".into(), "660".into())]),
        dry_run: false,
    };
    bluu_core::provider::validate_preference(&record.manifest, &preference)
        .expect("preference validation");
    let applied = check(ProviderAction::Apply, Some(preference.clone()), None);
    bluu_core::provider::validate_response_for_manifest(
        &record.manifest,
        &applied,
        ProviderAction::Apply,
    )
    .expect("apply response validation");
    check(
        ProviderAction::Verify,
        Some(preference),
        applied.rollback_token,
    );
    check(ProviderAction::Rollback, None, None);
    println!("provider smoke test passed");
}
