use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use bluu_core::{
    backend::BackendError,
    provider::{
        validate_response, ProviderAction, ProviderDevice, ProviderPreference, ProviderRecord,
        ProviderResponse, MAX_PROVIDER_RESULT_BYTES,
    },
};

pub struct ProviderRunContext<'a> {
    pub record: &'a ProviderRecord,
    pub action: ProviderAction,
    pub device: Option<&'a ProviderDevice>,
    pub preference: Option<&'a ProviderPreference>,
    pub rollback_token: Option<String>,
    pub timeout: Duration,
}

pub fn run_provider(context: &ProviderRunContext<'_>) -> Result<ProviderResponse, BackendError> {
    if context.record.status != bluu_core::ProviderStatus::Ready {
        return Err(BackendError::Provider(format!(
            "provider '{}' is not ready: {}",
            context.record.manifest.name,
            context.record.issues.join("; ")
        )));
    }
    let request = super::request(
        context.action,
        context.device,
        context.preference,
        context.rollback_token.clone(),
    );
    let temp = tempfile::Builder::new()
        .prefix("bluu-provider-")
        .tempdir()
        .map_err(|e| BackendError::Provider(format!("provider temp directory: {e}")))?;
    let request_path = temp.path().join("request.json");
    let result_path = temp.path().join("result.json");
    let bytes = serde_json::to_vec(&request)
        .map_err(|e| BackendError::Provider(format!("provider request JSON: {e}")))?;
    fs::write(&request_path, bytes)
        .map_err(|e| BackendError::Provider(format!("provider request write: {e}")))?;
    let exe = Path::new(&context.record.directory).join(&context.record.manifest.executable);
    if !exe.is_file() {
        return Err(BackendError::Provider(format!(
            "provider executable is missing: {}",
            exe.display()
        )));
    }
    let canonical_dir = Path::new(&context.record.directory)
        .canonicalize()
        .map_err(|e| BackendError::Provider(format!("provider directory: {e}")))?;
    let canonical_exe = exe
        .canonicalize()
        .map_err(|e| BackendError::Provider(format!("provider executable: {e}")))?;
    if !canonical_exe.starts_with(&canonical_dir) {
        return Err(BackendError::Provider(
            "provider executable resolves outside its package".into(),
        ));
    }
    let hash = super::discover::file_hash(&canonical_exe)
        .map_err(|e| BackendError::Provider(format!("provider executable hash: {e}")))?;
    if let Some(expected) = &context.record.executable_sha256 {
        if !hash.eq_ignore_ascii_case(expected) {
            return Err(BackendError::Provider(
                "provider executable changed after discovery; refresh and verify the package"
                    .into(),
            ));
        }
    }
    if context.record.manifest.requires_elevation {
        let params = format!(
            "--request {} --result {}",
            windows_quote(&request_path),
            windows_quote(&result_path)
        );
        run_elevated(&exe, &params, context.timeout)?;
    } else {
        run_plain(&exe, &request_path, &result_path, context.timeout)?;
    }
    let metadata = fs::metadata(&result_path).map_err(|e| {
        BackendError::Provider(format!(
            "provider '{}' produced no result: {e}",
            context.record.manifest.name
        ))
    })?;
    if metadata.len() > MAX_PROVIDER_RESULT_BYTES {
        return Err(BackendError::Provider(
            "provider result exceeded 128 KiB and was rejected".into(),
        ));
    }
    let result = fs::read(&result_path)
        .map_err(|e| BackendError::Provider(format!("provider result read: {e}")))?;
    let response: ProviderResponse = serde_json::from_slice(&result)
        .map_err(|e| BackendError::Provider(format!("provider result JSON: {e}")))?;
    validate_response(&response).map_err(|e| BackendError::Provider(e.to_string()))?;
    Ok(response)
}

fn run_plain(
    exe: &Path,
    request_path: &Path,
    result_path: &Path,
    timeout: Duration,
) -> Result<(), BackendError> {
    let mut command = Command::new(exe);
    command
        .arg("--request")
        .arg(request_path)
        .arg("--result")
        .arg(result_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = command.spawn().map_err(|e| {
        BackendError::Provider(format!("could not start provider {}: {e}", exe.display()))
    })?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                return Err(BackendError::Provider(format!(
                    "provider exited with {status}"
                )))
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(25)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(BackendError::Provider(
                    "provider timed out; no configuration was confirmed".into(),
                ));
            }
            Err(e) => return Err(BackendError::Provider(format!("provider wait: {e}"))),
        }
    }
}

fn windows_quote(path: &Path) -> String {
    windows_quote_arg(&path.to_string_lossy())
}

/// CommandLineToArgvW-compatible quoting for one argument. This is used only
/// for `ShellExecuteExW`'s parameter string, where arguments cannot be passed as
/// an argv array.
fn windows_quote_arg(value: &str) -> String {
    if !value.is_empty() && !value.chars().any(|c| c.is_whitespace() || c == '"') {
        return value.into();
    }
    let mut out = String::from("\"");
    let mut backslashes = 0;
    for ch in value.chars() {
        match ch {
            '\\' => backslashes += 1,
            '"' => {
                out.push_str(&"\\".repeat(backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            _ => {
                out.push_str(&"\\".repeat(backslashes));
                backslashes = 0;
                out.push(ch);
            }
        }
    }
    out.push_str(&"\\".repeat(backslashes * 2));
    out.push('"');
    out
}

#[cfg(windows)]
fn run_elevated(exe: &Path, params: &str, timeout: Duration) -> Result<(), BackendError> {
    use std::mem::size_of;
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::{
        Foundation::{CloseHandle, ERROR_CANCELLED},
        System::Threading::{GetExitCodeProcess, TerminateProcess, WaitForSingleObject},
        UI::{
            Shell::{
                ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
            },
            WindowsAndMessaging::SW_SHOWNORMAL,
        },
    };

    let verb = HSTRING::from("runas");
    let file = HSTRING::from(exe.to_string_lossy().as_ref());
    let parameters = HSTRING::from(params);
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
            if e.code() == ERROR_CANCELLED.into() {
                return Err(BackendError::ElevationRequired(
                    "the provider elevation prompt was cancelled".into(),
                ));
            }
            return Err(BackendError::Provider(format!(
                "could not start elevated provider: {e}"
            )));
        }
        if info.hProcess.is_invalid() {
            return Err(BackendError::Provider(
                "provider process handle was not returned".into(),
            ));
        }
        let wait = WaitForSingleObject(info.hProcess, timeout.as_millis() as u32);
        let mut exit_code = 0u32;
        let exit_ok = GetExitCodeProcess(info.hProcess, &mut exit_code).is_ok();
        if wait.0 != 0 {
            let _ = TerminateProcess(info.hProcess, 1);
            let _ = CloseHandle(info.hProcess);
            return Err(BackendError::Provider("elevated provider timed out".into()));
        }
        let _ = CloseHandle(info.hProcess);
        if exit_ok && exit_code != 0 {
            return Err(BackendError::Provider(format!(
                "elevated provider exited with code {exit_code}"
            )));
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn run_elevated(_exe: &Path, _params: &str, _timeout: Duration) -> Result<(), BackendError> {
    Err(BackendError::Provider(
        "elevated providers are only supported on Windows".into(),
    ))
}
