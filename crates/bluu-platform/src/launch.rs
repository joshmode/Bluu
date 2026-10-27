//! Arguments shared by the app and the elevated helper. Deliberately tiny: the helper
//! does not accept arbitrary commands, file paths, or registry operations.

use bluu_core::{
    backend::BackendError,
    model::{CaptureOutcome, CaptureRequest},
};

pub const HELPER_ARG: &str = "--etw-helper";
pub const RESULT_ARG: &str = "--result-file";

#[derive(Clone, Debug)]
pub struct HelperInvocation {
    pub duration_seconds: u32,
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
            .is_some_and(|n| n.starts_with("bluu-etw-"))
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
}
