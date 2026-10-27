//! A non-transport provider used to exercise Bluu's complete provider protocol.
//!
//! This binary never touches Bluetooth or a driver. It validates the request shape,
//! simulates a preference/rollback transaction, and reports an active codec only when
//! explicitly asked to verify. It is useful for UI and protocol testing, and must
//! never be presented as real codec support.

use std::{collections::BTreeMap, path::PathBuf};

use bluu_core::{
    model::ProviderOperationRequest,
    provider::{
        ProviderAction, ProviderPreference, ProviderRequest, ProviderResponse, PROVIDER_API_VERSION,
    },
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--manifest") {
        print!("{}", serde_json::to_string_pretty(&manifest()).unwrap());
        return;
    }
    let request_path = arg(&args, "--request");
    let result_path = arg(&args, "--result");
    let (Some(request_path), Some(result_path)) = (request_path, result_path) else {
        eprintln!("usage: bluu-provider-demo --request request.json --result result.json");
        std::process::exit(2);
    };
    let response = match run(PathBuf::from(request_path)) {
        Ok(response) => response,
        Err(e) => ProviderResponse {
            api_version: PROVIDER_API_VERSION,
            ok: false,
            message: String::new(),
            error: Some(e),
            active_codec: None,
            active_vendor_id: None,
            active_vendor_codec_id: None,
            verified: false,
            verification_method: None,
            rollback_token: None,
            applied_preference: None,
            requires_reconnect: false,
        },
    };
    if let Err(e) = std::fs::write(
        result_path,
        serde_json::to_vec_pretty(&response).unwrap_or_default(),
    ) {
        eprintln!("could not write result: {e}");
        std::process::exit(1);
    }
}

fn run(path: PathBuf) -> Result<ProviderResponse, String> {
    let request: ProviderRequest =
        serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("request JSON: {e}"))?;
    if request.api_version != PROVIDER_API_VERSION {
        return Err(format!(
            "unsupported provider API version {}",
            request.api_version
        ));
    }
    let Some(device) = request.device.as_ref() else {
        return Err("device is required".into());
    };
    if matches!(request.action, ProviderAction::Apply) {
        let preference = request
            .preference
            .as_ref()
            .ok_or("apply requires a preference")?;
        let manifest = serde_json::from_value::<bluu_core::ProviderManifest>(manifest())
            .map_err(|e| format!("demo manifest: {e}"))?;
        bluu_core::provider::validate_preference(&manifest, preference)
            .map_err(|e| format!("demo preference: {e}"))?;
        if !preference
            .ordered_codecs
            .contains(&bluu_core::CodecId::Ldac)
        {
            return Ok(failure("LDAC must be included in the demo preference"));
        }
        let token = format!("demo:{}:{}", device.id, chrono::Utc::now().timestamp());
        return Ok(ProviderResponse {
            api_version: PROVIDER_API_VERSION,
            ok: true,
            message: "DEMO only: preference accepted. No transport was changed.".into(),
            error: None,
            active_codec: None,
            active_vendor_id: None,
            active_vendor_codec_id: None,
            verified: false,
            verification_method: None,
            rollback_token: Some(token),
            applied_preference: Some(preference.clone()),
            requires_reconnect: true,
        });
    }
    if matches!(request.action, ProviderAction::Rollback) {
        return Ok(ProviderResponse {
            api_version: PROVIDER_API_VERSION,
            ok: true,
            message: "DEMO only: rollback acknowledged.".into(),
            error: None,
            active_codec: None,
            active_vendor_id: None,
            active_vendor_codec_id: None,
            verified: false,
            verification_method: None,
            rollback_token: None,
            applied_preference: None,
            requires_reconnect: true,
        });
    }
    if matches!(request.action, ProviderAction::Verify) {
        return Ok(ProviderResponse {
            api_version: PROVIDER_API_VERSION,
            ok: true,
            message: "DEMO only: no real codec was verified.".into(),
            error: None,
            active_codec: None,
            active_vendor_id: None,
            active_vendor_codec_id: None,
            verified: false,
            verification_method: None,
            rollback_token: None,
            applied_preference: None,
            requires_reconnect: false,
        });
    }
    Ok(ProviderResponse {
        api_version: PROVIDER_API_VERSION,
        ok: true,
        message: "Demo provider is healthy; no transport is present.".into(),
        error: None,
        active_codec: None,
        active_vendor_id: None,
        active_vendor_codec_id: None,
        verified: false,
        verification_method: None,
        rollback_token: None,
        applied_preference: None,
        requires_reconnect: false,
    })
}

fn failure(message: &str) -> ProviderResponse {
    ProviderResponse {
        api_version: PROVIDER_API_VERSION,
        ok: false,
        message: String::new(),
        error: Some(message.into()),
        active_codec: None,
        active_vendor_id: None,
        active_vendor_codec_id: None,
        verified: false,
        verification_method: None,
        rollback_token: None,
        applied_preference: None,
        requires_reconnect: false,
    }
}

fn arg(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn manifest() -> serde_json::Value {
    serde_json::json!({
        "api_version": 1,
        "id": "demo.ldac",
        "name": "Demo LDAC / aptX HD provider",
        "version": "0.1.0",
        "executable": "bluu-provider-demo.exe",
        "publisher": "Bluu test fixture",
        "homepage": "https://github.com/joshmode/Bluu",
        "description": "Protocol fixture only. Does not touch Bluetooth or audio.",
        "requires_elevation": false,
        "stack": "simulated",
        "capabilities": {
            "preferred_codec": true,
            "priority": true,
            "parameters": true,
            "active_codec": true,
            "rollback": true,
            "dry_run": true
        },
        "codecs": [
            {
                "codec": "ldac",
                "notes": "DEMO ONLY",
                "hardware_validated": true,
                "remote_validated": false,
                "parameters": [
                    {
                        "kind": "choice",
                        "key": "ldac_quality",
                        "label": "LDAC quality",
                        "options": ["330", "660", "990"],
                        "default": "990",
                        "note": "Fixture values; no transport exists."
                    }
                ]
            },
            {"codec": "aptx_hd", "notes": "DEMO ONLY", "hardware_validated": true, "remote_validated": false, "parameters": []},
            {"codec": "aptx_adaptive", "notes": "DEMO ONLY", "hardware_validated": true, "remote_validated": false, "parameters": []},
            {"codec": "sbc", "notes": "Fallback", "hardware_validated": true, "remote_validated": true, "parameters": []}
        ]
    })
}

#[allow(dead_code)]
fn _request_shape_example() -> ProviderOperationRequest {
    ProviderOperationRequest {
        provider_id: "demo.ldac".into(),
        operation: ProviderAction::Apply,
        device_id: Some("demo-headphones".into()),
        preference: Some(ProviderPreference {
            provider_id: "demo.ldac".into(),
            device_id: "demo-headphones".into(),
            ordered_codecs: vec![bluu_core::CodecId::Ldac],
            parameters: BTreeMap::new(),
            dry_run: false,
        }),
        rollback_token: None,
    }
}
