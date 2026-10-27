use std::{
    fs,
    path::{Path, PathBuf},
};

use bluu_core::backend::BackendError;
use bluu_core::provider::{
    validate_manifest, ProviderManifest, ProviderRecord, ProviderStatus, MAX_MANIFEST_BYTES,
};
use sha2::{Digest, Sha256};

/// Search user-installed providers first, then providers bundled beside the app.
/// `BLUU_PROVIDER_DIR` is an explicit development/testing override and is never
/// downloaded or created automatically.
pub fn provider_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(override_dir) = std::env::var_os("BLUU_PROVIDER_DIR") {
        dirs.push(PathBuf::from(override_dir));
    }
    dirs.push(bluu_core::settings::provider_dir());
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("providers"));
        }
    }
    dirs
}

pub fn discover() -> Vec<ProviderRecord> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for root in provider_dirs() {
        if root.join("provider.json").is_file() && root.is_dir() {
            let manifest_path = root.join("provider.json");
            let record = read_manifest(&root, &manifest_path);
            if seen.insert(record.manifest.id.clone()) {
                out.push(record);
            }
            continue;
        }
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let manifest_path = path.join("provider.json");
            if !manifest_path.is_file() {
                continue;
            }
            let record = read_manifest(&path, &manifest_path);
            if seen.insert(record.manifest.id.clone()) {
                out.push(record);
            }
        }
    }
    out.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
    out
}

fn read_manifest(directory: &Path, manifest_path: &Path) -> ProviderRecord {
    let fallback = ProviderManifest {
        api_version: 0,
        id: format!(
            "invalid.{}",
            directory
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or("unknown")
        ),
        name: directory
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("Invalid provider")
            .into(),
        version: "0".into(),
        executable: "provider.exe".into(),
        publisher: String::new(),
        homepage: String::new(),
        description: String::new(),
        requires_elevation: false,
        stack: bluu_core::provider::ProviderStack::VendorService,
        capabilities: Default::default(),
        codecs: Vec::new(),
    };
    let metadata = match fs::metadata(manifest_path) {
        Ok(m) if m.len() <= MAX_MANIFEST_BYTES => m,
        Ok(_) => {
            return invalid(
                fallback,
                directory,
                manifest_path,
                vec!["manifest is larger than 64 KiB".into()],
            )
        }
        Err(e) => {
            return invalid(
                fallback,
                directory,
                manifest_path,
                vec![format!("manifest metadata failed: {e}")],
            )
        }
    };
    let _ = metadata;
    let bytes = match fs::read(manifest_path) {
        Ok(bytes) => bytes,
        Err(e) => {
            return invalid(
                fallback,
                directory,
                manifest_path,
                vec![format!("manifest read failed: {e}")],
            )
        }
    };
    let manifest: ProviderManifest = match serde_json::from_slice(&bytes) {
        Ok(m) => m,
        Err(e) => {
            return invalid(
                fallback,
                directory,
                manifest_path,
                vec![format!("manifest JSON error: {e}")],
            )
        }
    };
    let mut issues = Vec::new();
    if let Err(e) = validate_manifest(&manifest) {
        issues.push(e.to_string());
    }
    let canonical_dir = match directory.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            issues.push(format!("provider directory could not be resolved: {e}"));
            return record(manifest, directory, manifest_path, issues, None, false);
        }
    };
    let executable = canonical_dir.join(&manifest.executable);
    let canonical_exe = match executable.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            issues.push(format!("provider executable not found: {e}"));
            return record(manifest, directory, manifest_path, issues, None, false);
        }
    };
    if !canonical_exe.starts_with(&canonical_dir) {
        issues.push("provider executable resolves outside its directory".into());
    }
    if !canonical_exe.is_file() {
        issues.push("provider executable is not a file".into());
    }
    let hash = file_hash(&canonical_exe).ok();
    let simulated = matches!(
        manifest.stack,
        bluu_core::provider::ProviderStack::Simulated
    );
    record(manifest, directory, manifest_path, issues, hash, simulated)
}

fn invalid(
    fallback: ProviderManifest,
    directory: &Path,
    manifest_path: &Path,
    issues: Vec<String>,
) -> ProviderRecord {
    record(fallback, directory, manifest_path, issues, None, false)
}

fn record(
    manifest: ProviderManifest,
    directory: &Path,
    manifest_path: &Path,
    issues: Vec<String>,
    executable_sha256: Option<String>,
    simulated: bool,
) -> ProviderRecord {
    ProviderRecord {
        manifest,
        manifest_path: manifest_path.display().to_string(),
        directory: directory.display().to_string(),
        status: if issues.is_empty() {
            ProviderStatus::Ready
        } else {
            ProviderStatus::Invalid
        },
        issues,
        executable_sha256,
        simulated,
    }
}

pub fn file_hash(path: &Path) -> Result<String, std::io::Error> {
    let bytes = fs::read(path)?;
    let digest = Sha256::digest(&bytes);
    Ok(format!("{digest:x}"))
}

pub fn copy_provider_tree(source: &Path, destination: &Path) -> Result<(), BackendError> {
    let root = bluu_core::settings::provider_dir();
    let root = root.canonicalize().unwrap_or(root);
    let parent = destination
        .parent()
        .ok_or_else(|| BackendError::Provider("provider destination has no parent".into()))?;
    let parent = parent
        .canonicalize()
        .unwrap_or_else(|_| parent.to_path_buf());
    if !parent.starts_with(&root) {
        return Err(BackendError::Provider(
            "provider destination is outside Bluu's provider directory".into(),
        ));
    }
    fs::create_dir_all(destination)
        .map_err(|e| BackendError::Provider(format!("provider directory: {e}")))?;
    copy_provider_tree_inner(source, destination, 0, &mut 0usize, &mut 0u64)
}

fn copy_provider_tree_inner(
    source: &Path,
    destination: &Path,
    depth: usize,
    files: &mut usize,
    bytes: &mut u64,
) -> Result<(), BackendError> {
    const MAX_PROVIDER_DEPTH: usize = 8;
    const MAX_PROVIDER_FILES: usize = 512;
    const MAX_PROVIDER_TREE_BYTES: u64 = 256 * 1024 * 1024;
    if depth > MAX_PROVIDER_DEPTH {
        return Err(BackendError::Provider(
            "provider package is nested too deeply".into(),
        ));
    }
    fs::create_dir_all(destination)
        .map_err(|e| BackendError::Provider(format!("provider directory: {e}")))?;
    for entry in fs::read_dir(source)
        .map_err(|e| BackendError::Provider(format!("provider source read: {e}")))?
    {
        let entry = entry.map_err(|e| BackendError::Provider(format!("provider entry: {e}")))?;
        let path = entry.path();
        let name = entry.file_name();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|e| BackendError::Provider(format!("provider entry metadata: {e}")))?;
        if metadata.file_type().is_symlink() {
            return Err(BackendError::Provider(
                "provider packages may not contain symlinks".into(),
            ));
        }
        *files += 1;
        if *files > MAX_PROVIDER_FILES {
            return Err(BackendError::Provider(
                "provider package contains too many files".into(),
            ));
        }
        *bytes = bytes.saturating_add(metadata.len());
        if *bytes > MAX_PROVIDER_TREE_BYTES {
            return Err(BackendError::Provider(
                "provider package exceeds the 256 MiB limit".into(),
            ));
        }
        if path.is_dir() {
            copy_provider_tree_inner(&path, &destination.join(name), depth + 1, files, bytes)?;
        } else if path.is_file() {
            fs::copy(&path, destination.join(name))
                .map_err(|e| BackendError::Provider(format!("provider file copy: {e}")))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn discovers_a_valid_manifest_and_hashes_executable() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let provider_dir = dir.path().join("demo");
        fs::create_dir_all(&provider_dir).unwrap();
        fs::write(provider_dir.join("provider.exe"), b"fixture").unwrap();
        let manifest = serde_json::json!({
            "api_version": 1,
            "id": "test.ldac",
            "name": "Test LDAC",
            "version": "1.0.0",
            "executable": "provider.exe",
            "requires_elevation": true,
            "stack": "replacement_a2dp_driver",
            "capabilities": {
                "preferred_codec": true,
                "priority": true,
                "parameters": false,
                "active_codec": true,
                "rollback": true,
                "dry_run": true
            },
            "codecs": [
                {"codec": "ldac", "notes": "LDAC", "parameters": []},
                {"codec": "aptx_hd", "notes": "", "parameters": []}
            ]
        });
        fs::write(
            provider_dir.join("provider.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        std::env::set_var("BLUU_PROVIDER_DIR", dir.path());
        let records = discover();
        std::env::remove_var("BLUU_PROVIDER_DIR");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].status, ProviderStatus::Ready);
        assert!(records[0].supports(bluu_core::CodecId::Ldac));
        assert!(records[0].executable_sha256.is_some());
    }

    #[test]
    fn rejects_executable_traversal() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let provider_dir = dir.path().join("bad");
        fs::create_dir_all(&provider_dir).unwrap();
        let manifest = serde_json::json!({
            "api_version": 1,
            "id": "bad.provider",
            "name": "Bad",
            "version": "1",
            "executable": "../evil.exe",
            "stack": "vendor_service",
            "capabilities": {"preferred_codec": true},
            "codecs": [{"codec": "sbc"}]
        });
        fs::write(
            provider_dir.join("provider.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        std::env::set_var("BLUU_PROVIDER_DIR", dir.path());
        let records = discover();
        std::env::remove_var("BLUU_PROVIDER_DIR");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].status, ProviderStatus::Invalid);
    }

    #[test]
    fn accepts_a_provider_directory_as_the_override() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("provider.exe"), b"fixture").unwrap();
        let manifest = serde_json::json!({
            "api_version": 1,
            "id": "test.dir",
            "name": "Directory provider",
            "version": "1",
            "executable": "provider.exe",
            "stack": "simulated",
            "capabilities": {"preferred_codec": true},
            "codecs": [{"codec": "ldac"}]
        });
        fs::write(
            dir.path().join("provider.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        std::env::set_var("BLUU_PROVIDER_DIR", dir.path());
        let records = discover();
        std::env::remove_var("BLUU_PROVIDER_DIR");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].manifest.id, "test.dir");
    }
}
