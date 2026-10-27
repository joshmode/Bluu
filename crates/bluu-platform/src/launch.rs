//! Arguments shared by the app and the elevated helper. Deliberately tiny: the helper
//! does not accept arbitrary commands, file paths, or registry operations.

use bluu_core::{
    backend::BackendError,
    model::{CaptureOutcome, CaptureRequest},
};

pub const HELPER_ARG: &str = "--etw-helper";
pub const RESULT_ARG: &str = "--result-file";
pub const AAC_HELPER_ARG: &str = "--aac-helper";

#[derive(Clone, Debug)]
pub struct HelperInvocation {
    pub duration_seconds: u32,
    pub result_path: std::path::PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AacHelperAction {
    Set(bool),
    Restore,
}

#[derive(Clone, Debug)]
pub struct AacHelperInvocation {
    pub action: AacHelperAction,
    pub dry_run: bool,
    pub result_path: std::path::PathBuf,
}

pub fn parse_helper_args(args: &[String]) -> Result<HelperInvocation, BackendError> {
    if !args.iter().any(|a| a == HELPER_ARG) {
        return Err(BackendError::HelperStart(
            "helper flag was not present".into(),
        ));
    }
    let duration = args
        .iter()
        .position(|a| a == "--seconds")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(25)
        .clamp(5, 120);
    let result_path = args
        .iter()
        .position(|a| a == RESULT_ARG)
        .and_then(|i| args.get(i + 1))
        .map(std::path::PathBuf::from)
        .ok_or_else(|| BackendError::HelperStart("missing result file argument".into()))?;
    Ok(HelperInvocation {
        duration_seconds: duration,
        result_path,
    })
}

pub fn parse_aac_helper_args(args: &[String]) -> Result<AacHelperInvocation, BackendError> {
    if !args.iter().any(|a| a == AAC_HELPER_ARG) {
        return Err(BackendError::HelperStart(
            "AAC helper flag was not present".into(),
        ));
    }
    let action = if args.iter().any(|a| a == "--restore-aac") {
        AacHelperAction::Restore
    } else {
        let value = args
            .iter()
            .position(|a| a == "--set-aac")
            .and_then(|i| args.get(i + 1))
            .ok_or_else(|| {
                BackendError::HelperStart("missing --set-aac 0|1 or --restore-aac operation".into())
            })?;
        match value.as_str() {
            "0" => AacHelperAction::Set(false),
            "1" => AacHelperAction::Set(true),
            _ => {
                return Err(BackendError::HelperStart(
                    "--set-aac accepts only 0 or 1".into(),
                ))
            }
        }
    };
    let dry_run = args.iter().any(|a| a == "--dry-run");
    if dry_run && action == AacHelperAction::Restore {
        return Err(BackendError::HelperStart(
            "--restore-aac does not have a dry-run mode because the previous value is read by the unelevated app first"
                .into(),
        ));
    }
    let result_path = args
        .iter()
        .position(|a| a == RESULT_ARG)
        .and_then(|i| args.get(i + 1))
        .map(std::path::PathBuf::from)
        .ok_or_else(|| BackendError::HelperStart("missing result file argument".into()))?;
    Ok(AacHelperInvocation {
        action,
        dry_run,
        result_path,
    })
}

pub fn demo_capture(request: &CaptureRequest) -> CaptureOutcome {
    use bluu_core::{codec::VerificationMethod, Confidence};
    let observation = bluu_core::model::ActiveCodecObservation {
        codec: bluu_core::CodecId::Aac,
        vendor_id: None,
        vendor_codec_id: None,
        observed_at: chrono::Utc::now(),
        method: VerificationMethod::A2dpEtwEvent,
        device_id: request.device_id.clone(),
        device_association_confidence: Confidence::Medium,
        note: "DEMO: synthetic observation. This is not proof of live codec operation.".into(),
    };
    CaptureOutcome {
        observation: Some(observation),
        captured_events: 3,
        session_started: true,
        note: format!(
            "Demo capture completed after {} s. No ETW session was started.",
            request.duration_seconds
        ),
    }
}

/// The elevated helper accepts a result path only from the current user's temp
/// directory and only with a JSON extension. This limits a compromised or
/// accidentally misdirected caller from writing into privileged locations.
pub fn safe_result_path(path: &std::path::Path) -> bool {
    safe_temp_json_path(path, "bluu-etw-")
}

/// Restrict a helper output path to a canonical JSON file directly under the
/// current user's temporary directory with a component-specific filename prefix.
pub fn safe_temp_json_path(path: &std::path::Path, required_prefix: &str) -> bool {
    let Ok(temp) = std::env::temp_dir().canonicalize() else {
        return false;
    };
    let Ok(parent) = path
        .parent()
        .unwrap_or(std::path::Path::new(""))
        .canonicalize()
    else {
        return false;
    };
    parent == temp
        && path.extension().and_then(|e| e.to_str()) == Some("json")
        && path
            .file_name()
            .and_then(|e| e.to_str())
            .is_some_and(|n| n.starts_with(required_prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_args_are_bounded_and_require_a_result_file() {
        let args = vec![
            "helper".to_string(),
            "--etw-helper".to_string(),
            "--seconds".to_string(),
            "999".to_string(),
            "--result-file".to_string(),
            "/tmp/x.json".to_string(),
        ];
        let invocation = parse_helper_args(&args).unwrap();
        assert_eq!(invocation.duration_seconds, 120);
        let missing = vec!["helper".to_string(), "--etw-helper".to_string()];
        assert!(parse_helper_args(&missing).is_err());
    }

    #[test]
    fn helper_result_paths_are_prefix_scoped() {
        let temp = std::env::temp_dir();
        assert!(safe_temp_json_path(
            &temp.join("bluu-aac-123.json"),
            "bluu-aac-"
        ));
        assert!(!safe_temp_json_path(
            &temp.join("bluu-etw-123.json"),
            "bluu-aac-"
        ));
        assert!(!safe_temp_json_path(
            &temp.join("bluu-aac-123.txt"),
            "bluu-aac-"
        ));
    }

    #[test]
    fn aac_helper_accepts_only_typed_operations() {
        let base = vec![
            "helper".to_string(),
            "--aac-helper".to_string(),
            "--result-file".to_string(),
            "/tmp/bluu-aac-1.json".to_string(),
        ];
        let mut disable = base.clone();
        disable.extend(["--set-aac".into(), "0".into()]);
        let parsed = parse_aac_helper_args(&disable).unwrap();
        assert_eq!(parsed.action, AacHelperAction::Set(false));
        assert!(!parsed.dry_run);

        let mut dry = base.clone();
        dry.extend(["--set-aac".into(), "1".into(), "--dry-run".into()]);
        assert!(parse_aac_helper_args(&dry).unwrap().dry_run);

        let mut restore = base.clone();
        restore.push("--restore-aac".into());
        assert_eq!(
            parse_aac_helper_args(&restore).unwrap().action,
            AacHelperAction::Restore
        );

        let mut invalid = base;
        invalid.extend(["--set-aac".into(), "2".into()]);
        assert!(parse_aac_helper_args(&invalid).is_err());
    }
}
