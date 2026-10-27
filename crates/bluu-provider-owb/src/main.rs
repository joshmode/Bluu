//! Provider adapter for Bluu's optional GPLv3 native A2DP driver/service.
//!
//! The adapter speaks Bluu's provider JSON protocol on the outside and the
//! OpenWinBlue-compatible binary named-pipe protocol on the inside. It never
//! installs a driver or edits the registry.

use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};

use bluu_core::{
    provider::{
        ProviderAction, ProviderPreference, ProviderRequest, ProviderResponse, PROVIDER_API_VERSION,
    },
    CodecId,
};

const CODEC_SBC: u32 = 0;
const CODEC_LDAC: u32 = 1;
const CODEC_APTX: u32 = 2;
const CODEC_APTX_HD: u32 = 3;
const CODEC_AAC: u32 = 4;
const MSG_GET_STATUS: u16 = 0x0010;
const MSG_STATUS_REPLY: u16 = 0x0011;
const MSG_SET_CODEC: u16 = 0x0020;
const MSG_CODEC_ACK: u16 = 0x0021;
const CODEC_CONFIG_MAGIC: u32 = 0x3155_4C42; // "BLU1"
const SET_CODEC_PAYLOAD_LEN: usize = 48;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--manifest") {
        println!("{}", serde_json::to_string_pretty(&manifest()).unwrap());
        return;
    }
    let (Some(request_path), Some(result_path)) = (arg(&args, "--request"), arg(&args, "--result"))
    else {
        eprintln!("usage: bluu-provider-owb --request <json> --result <json>");
        std::process::exit(2);
    };
    let response = match fs::read_to_string(&request_path)
        .map_err(|e| format!("request: {e}"))
        .and_then(|text| {
            serde_json::from_str::<ProviderRequest>(&text).map_err(|e| format!("request JSON: {e}"))
        })
        .and_then(run)
    {
        Ok(response) => response,
        Err(error) => ProviderResponse {
            api_version: PROVIDER_API_VERSION,
            ok: false,
            message: String::new(),
            error: Some(error),
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
    if let Err(e) = fs::write(
        result_path,
        serde_json::to_vec_pretty(&response).unwrap_or_default(),
    ) {
        eprintln!("result write failed: {e}");
        std::process::exit(1);
    }
}

fn run(request: ProviderRequest) -> Result<ProviderResponse, String> {
    if request.api_version != PROVIDER_API_VERSION {
        return Err("unsupported provider API version".into());
    }
    let mut pipe = open_pipe()?;
    match request.action {
        ProviderAction::Describe => {
            let status = get_status(&mut pipe)?;
            Ok(success(format!(
                "Native A2DP service is reachable; driver-selected codec: {}.",
                status.codec_label()
            )))
        }
        ProviderAction::Apply => {
            let preference = request.preference.ok_or("apply requires a preference")?;
            let previous = get_status(&mut pipe)
                .ok()
                .and_then(|status| status.codec)
                .unwrap_or(CODEC_SBC);
            apply(&mut pipe, &preference)?;
            let status = get_status(&mut pipe).ok();
            Ok(ProviderResponse {
                api_version: PROVIDER_API_VERSION,
                ok: true,
                message: "Preference sent to the native A2DP service. Reconnect the headset if the stream does not restart.".into(),
                error: None,
                active_codec: None,
                active_vendor_id: None,
                active_vendor_codec_id: None,
                verified: false,
                verification_method: None,
                rollback_token: Some(format!("codec:{}", codec_name(previous))),
                applied_preference: Some(preference),
                requires_reconnect: status.map(|s| !s.streaming).unwrap_or(true),
            })
        }
        ProviderAction::Verify => {
            let status = get_status(&mut pipe)?;
            let codec = status
                .streaming
                .then_some(status.codec)
                .flatten()
                .map(codec_from_wire);
            Ok(ProviderResponse {
                api_version: PROVIDER_API_VERSION,
                ok: true,
                message: if status.streaming {
                    "Native driver reports an active A2DP stream.".into()
                } else {
                    "Native driver is reachable but no A2DP media stream is active.".into()
                },
                error: None,
                active_codec: codec,
                active_vendor_id: None,
                active_vendor_codec_id: None,
                verified: false,
                verification_method: Some(
                    "native A2DP driver state (provider-attested, not independent verification)"
                        .into(),
                ),
                rollback_token: None,
                applied_preference: None,
                requires_reconnect: false,
            })
        }
        ProviderAction::Rollback => {
            let codec = rollback_codec(request.rollback_token.as_deref());
            set_codec(&mut pipe, codec)?;
            Ok(ProviderResponse {
                api_version: PROVIDER_API_VERSION,
                ok: true,
                message: format!(
                    "Native service switched back to {}; reconnect to complete rollback.",
                    codec_name(codec)
                ),
                error: None,
                active_codec: None,
                active_vendor_id: None,
                active_vendor_codec_id: None,
                verified: false,
                verification_method: None,
                rollback_token: None,
                applied_preference: None,
                requires_reconnect: true,
            })
        }
    }
}

fn apply(pipe: &mut Pipe, preference: &ProviderPreference) -> Result<(), String> {
    let codec = preference
        .ordered_codecs
        .first()
        .ok_or("no codec selected")?;
    let wire = wire_from_codec(*codec)?;
    let previous = get_status(pipe).ok().and_then(|status| status.codec);
    set_codec(pipe, wire)?;
    for (key, value) in &preference.parameters {
        let (wire_key, wire_value) = parameter(*codec, key, value)?;
        if let Err(error) = set_param(pipe, wire, &wire_key, wire_value) {
            if let Some(previous) = previous {
                let _ = set_codec(pipe, previous);
            }
            return Err(error);
        }
    }
    Ok(())
}

fn codec_from_name(name: &str) -> Option<u32> {
    match name {
        "SBC" => Some(CODEC_SBC),
        "LDAC" => Some(CODEC_LDAC),
        "aptX" => Some(CODEC_APTX),
        "aptX-HD" => Some(CODEC_APTX_HD),
        "AAC" => Some(CODEC_AAC),
        _ => None,
    }
}

fn rollback_codec(token: Option<&str>) -> u32 {
    token
        .and_then(|token| token.strip_prefix("codec:"))
        .and_then(codec_from_name)
        .unwrap_or(CODEC_SBC)
}

fn parameter(codec: CodecId, key: &str, value: &str) -> Result<(String, i64), String> {
    if key == "ldac_quality" && codec == CodecId::Ldac {
        let value = match value {
            "990" => 0,
            "660" => 1,
            "330" => 2,
            _ => return Err("LDAC quality must be 330, 660 or 990".into()),
        };
        return Ok(("quality".into(), value));
    }
    let frequency_key = match codec {
        CodecId::Ldac => "ldac_frequency",
        CodecId::AptXHd => "aptx_hd_frequency",
        CodecId::AptX => "aptx_frequency",
        _ => "",
    };
    if !frequency_key.is_empty() && key == frequency_key {
        let value = value
            .parse::<i64>()
            .map_err(|_| "frequency must be an integer")?;
        if !matches!(value, 44_100 | 48_000) {
            return Err("native service currently accepts 44100 or 48000 Hz".into());
        }
        return Ok(("freq".into(), value));
    }
    Err(format!(
        "parameter '{key}' is not supported by the native provider for {}",
        codec.label()
    ))
}

fn success(message: String) -> ProviderResponse {
    ProviderResponse {
        api_version: PROVIDER_API_VERSION,
        ok: true,
        message,
        error: None,
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

fn wire_from_codec(codec: CodecId) -> Result<u32, String> {
    Ok(match codec {
        CodecId::Sbc => CODEC_SBC,
        CodecId::Ldac => CODEC_LDAC,
        CodecId::AptX => CODEC_APTX,
        CodecId::AptXHd => CODEC_APTX_HD,
        CodecId::Aac => CODEC_AAC,
        _ => {
            return Err(format!(
                "{} is not implemented by the native A2DP service",
                codec.label()
            ))
        }
    })
}

fn codec_from_wire(id: u32) -> CodecId {
    match id {
        CODEC_SBC => CodecId::Sbc,
        CODEC_LDAC => CodecId::Ldac,
        CODEC_APTX => CodecId::AptX,
        CODEC_APTX_HD => CodecId::AptXHd,
        CODEC_AAC => CodecId::Aac,
        _ => CodecId::Other,
    }
}

struct Status {
    codec: Option<u32>,
    streaming: bool,
}

impl Status {
    fn codec_label(&self) -> String {
        self.codec
            .map(codec_from_wire)
            .map(|c| c.label().to_string())
            .unwrap_or_else(|| "unknown".into())
    }
}

fn set_codec(pipe: &mut Pipe, codec: u32) -> Result<(), String> {
    set_param(pipe, codec, "switch", 1)
}

fn set_param(pipe: &mut Pipe, codec: u32, key: &str, value: i64) -> Result<(), String> {
    let mut payload = [0u8; SET_CODEC_PAYLOAD_LEN];
    let name = codec_name(codec);
    payload[..16].copy_from_slice(&name.as_bytes()[..name.len().min(15)]);
    let key = key.as_bytes();
    payload[16..16 + key.len().min(15)].copy_from_slice(&key[..key.len().min(15)]);
    payload[32..40].copy_from_slice(&value.to_le_bytes());
    payload[40..44].copy_from_slice(&CODEC_CONFIG_MAGIC.to_le_bytes());
    pipe.write_message(MSG_SET_CODEC, &payload)?;
    let (kind, response) = pipe.read_message()?;
    if kind != MSG_CODEC_ACK || response.len() < 4 {
        return Err("native service returned an unexpected codec acknowledgement".into());
    }
    if response[0] == 0 {
        return Err("native service rejected the codec configuration".into());
    }
    Ok(())
}

fn get_status(pipe: &mut Pipe) -> Result<Status, String> {
    pipe.write_message(MSG_GET_STATUS, &[])?;
    let (kind, response) = pipe.read_message()?;
    if kind != MSG_STATUS_REPLY || response.len() < 24 {
        return Err("native service returned an unexpected status reply".into());
    }
    let name = String::from_utf8_lossy(&response[..16])
        .trim_end_matches('\0')
        .to_string();
    let codec = match name.as_str() {
        "SBC" => Some(CODEC_SBC),
        "LDAC" => Some(CODEC_LDAC),
        "aptX" => Some(CODEC_APTX),
        "aptX-HD" => Some(CODEC_APTX_HD),
        "AAC" => Some(CODEC_AAC),
        _ => None,
    };
    Ok(Status {
        codec,
        streaming: response[20] != 0,
    })
}

fn codec_name(codec: u32) -> &'static str {
    match codec {
        CODEC_SBC => "SBC",
        CODEC_LDAC => "LDAC",
        CODEC_APTX => "aptX",
        CODEC_APTX_HD => "aptX-HD",
        CODEC_AAC => "AAC",
        _ => "SBC",
    }
}

struct Pipe(fs::File);

fn open_pipe() -> Result<Pipe, String> {
    #[cfg(windows)]
    {
        const PIPE: &str = r"\\.\pipe\openwinblue";
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(PIPE)
            .map_err(|e| {
                format!(
                    "cannot open {PIPE}: {e}. Install/start the optional native A2DP service first."
                )
            })?;
        Ok(Pipe(file))
    }
    #[cfg(not(windows))]
    {
        Err("the native A2DP provider is Windows-only".into())
    }
}

impl Pipe {
    fn write_message(&mut self, kind: u16, payload: &[u8]) -> Result<(), String> {
        let mut bytes = Vec::with_capacity(4 + payload.len());
        bytes.extend_from_slice(&kind.to_le_bytes());
        bytes.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        bytes.extend_from_slice(payload);
        self.0.write_all(&bytes).map_err(|e| e.to_string())
    }

    fn read_message(&mut self) -> Result<(u16, Vec<u8>), String> {
        let mut header = [0u8; 4];
        self.0.read_exact(&mut header).map_err(|e| e.to_string())?;
        let kind = u16::from_le_bytes([header[0], header[1]]);
        let len = u16::from_le_bytes([header[2], header[3]]) as usize;
        if len > 4096 {
            return Err("native service reply is too large".into());
        }
        let mut payload = vec![0u8; len];
        self.0.read_exact(&mut payload).map_err(|e| e.to_string())?;
        Ok((kind, payload))
    }
}

fn arg(args: &[String], key: &str) -> Option<PathBuf> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
}

fn manifest() -> serde_json::Value {
    serde_json::json!({
        "api_version": 1,
        "id": "bluu.native-a2dp",
        "name": "Bluu Native A2DP (LDAC / aptX HD)",
        "version": "0.4.0",
        "executable": "bluu-provider-owb.exe",
        "publisher": "Bluu / OpenWinBlue contributors",
        "homepage": "https://github.com/joshmode/Bluu",
        "description": "Adapter for the optional GPLv3 native A2DP driver and codec service.",
        "requires_elevation": true,
        "stack": "replacement_a2dp_driver",
        "capabilities": {
            "preferred_codec": true,
            "priority": false,
            "parameters": true,
            "active_codec": true,
            "rollback": true,
            "dry_run": false
        },
        "codecs": [
            {
                "codec": "ldac",
                "notes": "Requires a Sony-compatible LDAC sink and the native driver/service.",
                "hardware_validated": false,
                "remote_validated": false,
                "parameters": [
                    {
                        "kind": "choice",
                        "key": "ldac_quality",
                        "label": "LDAC quality",
                        "options": ["330", "660", "990"],
                        "default": "990",
                        "note": "Maps to LDAC EQMID MQ/SQ/HQ."
                    },
                    {
                        "kind": "choice",
                        "key": "ldac_frequency",
                        "label": "Sample rate",
                        "options": ["44100", "48000"],
                        "default": "44100",
                        "note": "Must match the active WASAPI capture rate."
                    }
                ]
            },
            {
                "codec": "aptx_hd",
                "notes": "FFmpeg LGPL aptX core; requires aptX HD-capable remote hardware.",
                "hardware_validated": false,
                "remote_validated": false,
                "parameters": [
                    {
                        "kind": "choice",
                        "key": "aptx_hd_frequency",
                        "label": "Sample rate",
                        "options": ["44100", "48000"],
                        "default": "44100"
                    }
                ]
            },
            {
                "codec": "aptx",
                "notes": "aptX Classic via the FFmpeg LGPL aptX core.",
                "hardware_validated": false,
                "remote_validated": false,
                "parameters": [
                    {
                        "kind": "choice",
                        "key": "aptx_frequency",
                        "label": "Sample rate",
                        "options": ["44100", "48000"],
                        "default": "44100"
                    }
                ]
            },
            {"codec": "sbc", "notes": "Fallback", "hardware_validated": true, "remote_validated": true}
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_ids_match_native_driver() {
        assert_eq!(wire_from_codec(CodecId::Ldac).unwrap(), 1);
        assert_eq!(wire_from_codec(CodecId::AptXHd).unwrap(), 3);
        assert_eq!(codec_from_wire(3), CodecId::AptXHd);
    }

    #[test]
    fn ldac_quality_maps_to_eqmid() {
        assert_eq!(
            parameter(CodecId::Ldac, "ldac_quality", "990").unwrap(),
            ("quality".into(), 0)
        );
        assert_eq!(
            parameter(CodecId::Ldac, "ldac_quality", "660").unwrap(),
            ("quality".into(), 1)
        );
        assert_eq!(
            parameter(CodecId::Ldac, "ldac_quality", "330").unwrap(),
            ("quality".into(), 2)
        );
    }

    #[test]
    fn embedded_and_shipped_manifests_are_valid() {
        let embedded: bluu_core::ProviderManifest =
            serde_json::from_value(manifest()).expect("embedded manifest JSON");
        bluu_core::provider::validate_manifest(&embedded).expect("embedded manifest must validate");

        let shipped = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../providers/bluu-native-a2dp/provider.json");
        let text = std::fs::read_to_string(shipped).expect("shipped provider manifest");
        let from_file: bluu_core::ProviderManifest =
            serde_json::from_str(&text).expect("shipped manifest JSON");
        bluu_core::provider::validate_manifest(&from_file).expect("shipped manifest must validate");
        assert_eq!(embedded.id, from_file.id);
        assert_eq!(embedded.version, from_file.version);
        assert_eq!(embedded.codecs.len(), from_file.codecs.len());
    }

    #[test]
    fn native_frequency_keys_are_codec_qualified() {
        assert_eq!(
            parameter(CodecId::Ldac, "ldac_frequency", "48000").unwrap(),
            ("freq".into(), 48_000)
        );
        assert_eq!(
            parameter(CodecId::AptXHd, "aptx_hd_frequency", "44100").unwrap(),
            ("freq".into(), 44_100)
        );
        assert!(parameter(CodecId::AptXHd, "ldac_frequency", "44100").is_err());
    }

    #[test]
    fn rollback_tokens_restore_known_codecs_and_fall_back_to_sbc() {
        assert_eq!(rollback_codec(Some("codec:LDAC")), CODEC_LDAC);
        assert_eq!(rollback_codec(Some("codec:aptX-HD")), CODEC_APTX_HD);
        assert_eq!(rollback_codec(Some("codec:AAC")), CODEC_AAC);
        assert_eq!(rollback_codec(Some("codec:unknown")), CODEC_SBC);
        assert_eq!(rollback_codec(Some("garbage")), CODEC_SBC);
        assert_eq!(rollback_codec(None), CODEC_SBC);
    }

    #[test]
    fn rollback_token_encodes_previous_codec() {
        assert_eq!(
            format!("codec:{}", codec_name(CODEC_APTX_HD)),
            "codec:aptX-HD"
        );
        assert_eq!(codec_from_name("aptX-HD"), Some(CODEC_APTX_HD));
        assert_eq!(codec_name(rollback_codec(Some("codec:aptX-HD"))), "aptX-HD");
    }

    #[test]
    fn ipc_payload_layout_is_versioned_and_zeroed() {
        assert_eq!(SET_CODEC_PAYLOAD_LEN, 48);
        assert_eq!(CODEC_CONFIG_MAGIC, 0x3155_4C42);
        let mut payload = [0u8; SET_CODEC_PAYLOAD_LEN];
        payload[40..44].copy_from_slice(&CODEC_CONFIG_MAGIC.to_le_bytes());
        assert_eq!(&payload[40..44], b"BLU1");
        assert_eq!(&payload[44..48], &[0u8; 4]);
    }
}
