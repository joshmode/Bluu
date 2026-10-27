use std::{ffi::c_void, mem::size_of, path::Path, time::Instant};

use bluu_core::{
    backend::BackendError,
    model::{CaptureOutcome, CaptureRequest},
};
use windows::core::PCWSTR;
use windows::Win32::{
    Foundation::{CloseHandle, ERROR_CANCELLED, HANDLE},
    Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY},
    System::Threading::{GetCurrentProcess, OpenProcessToken, WaitForSingleObject},
    UI::{
        Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW},
        WindowsAndMessaging::SW_SHOWNORMAL,
    },
};

pub fn is_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut size = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok && elevation.TokenIsElevated != 0
    }
}

pub fn device_property(
    store: &windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore,
    key: &windows::Win32::Foundation::PROPERTYKEY,
) -> Option<windows::Win32::System::Com::StructuredStorage::PROPVARIANT> {
    unsafe { store.GetValue(key) }.ok()
}

pub fn capture(request: &CaptureRequest) -> Result<CaptureOutcome, BackendError> {
    let exe = helper_path()?;
    let temp = std::env::temp_dir().join(format!(
        "bluu-etw-{}-{}.json",
        std::process::id(),
        chrono::Utc::now().timestamp_millis()
    ));
    let params = format!(
        "--etw-helper --seconds {} --result-file {}",
        request.duration_seconds.clamp(5, 120),
        temp.display()
    );
    run_elevated(&exe, &params).inspect_err(|_e| {
        let _ = std::fs::remove_file(&temp);
    })?;
    let metadata = std::fs::metadata(&temp)
        .map_err(|e| BackendError::HelperStart(format!("helper produced no result file: {e}")))?;
    if metadata.len() > 64 * 1024 {
        let _ = std::fs::remove_file(&temp);
        return Err(BackendError::HelperStart(
            "helper result was unexpectedly large and was rejected".into(),
        ));
    }
    let bytes = std::fs::read(&temp)
        .map_err(|e| BackendError::HelperStart(format!("helper produced no result file: {e}")))?;
    let outcome: HelperResult = serde_json::from_slice(&bytes)
        .map_err(|e| BackendError::HelperStart(format!("invalid helper result: {e}")))?;
    let _ = std::fs::remove_file(&temp);
    if let Some(error) = outcome.error {
        return Err(BackendError::HelperStart(error));
    }
    Ok(CaptureOutcome {
        observation: outcome.observation,
        captured_events: outcome.captured_events,
        session_started: outcome.session_started,
        note: outcome.note,
    })
}

fn run_elevated(exe: &Path, params: &str) -> Result<(), BackendError> {
    let verb = windows::core::HSTRING::from("runas");
    let file = windows::core::HSTRING::from(exe.to_string_lossy().as_ref());
    let parameters = windows::core::HSTRING::from(params);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(parameters.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    unsafe {
        if let Err(e) = ShellExecuteExW(&mut info) {
            if e.code().0 as u32 == ERROR_CANCELLED.0 {
                return Err(BackendError::ElevationRequired(
                    "the Windows elevation prompt was cancelled".into(),
                ));
            }
            return Err(BackendError::HelperStart(format!(
                "could not start the elevated ETW helper: {e}"
            )));
        }
        if info.hProcess.is_invalid() {
            return Err(BackendError::HelperStart(
                "Windows did not return a process handle for the ETW helper".into(),
            ));
        }
        let started = Instant::now();
        let wait = WaitForSingleObject(info.hProcess, 150_000);
        let _ = CloseHandle(info.hProcess);
        if wait.0 != 0 {
            return Err(BackendError::HelperStart(
                "the ETW helper did not finish within 150 seconds".into(),
            ));
        }
        if started.elapsed() < std::time::Duration::from_secs(1) {
            return Err(BackendError::ElevationRequired(
                "the helper exited immediately; elevation may have been cancelled or the trace could not start"
                    .into(),
            ));
        }
    }
    Ok(())
}

#[derive(serde::Deserialize)]
struct HelperResult {
    observation: Option<bluu_core::model::ActiveCodecObservation>,
    captured_events: u32,
    session_started: bool,
    note: String,
    error: Option<String>,
}

fn helper_path() -> Result<std::path::PathBuf, BackendError> {
    let exe = std::env::current_exe()
        .map_err(|e| BackendError::HelperStart(format!("current exe: {e}")))?;
    let dir = exe
        .parent()
        .ok_or_else(|| BackendError::HelperStart("application folder unavailable".into()))?;
    for name in ["bluu-etw-helper.exe", "bluu-etw-helper"] {
        let candidate = dir.join(name);
        if Path::new(&candidate).exists() {
            return Ok(candidate);
        }
    }
    Err(BackendError::HelperStart(format!(
        "the helper is not next to the application ({})",
        dir.display()
    )))
}
