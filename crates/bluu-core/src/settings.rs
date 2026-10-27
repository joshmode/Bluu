use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    Dark,
    Light,
    System,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub schema_version: u32,
    pub theme: ThemePreference,
    pub verbose_logging: bool,
    pub capture_seconds: u32,
    pub include_device_names_in_export: bool,
    pub include_addresses_in_export: bool,
    pub last_selected_device: Option<String>,
    pub window: WindowSettings,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowSettings {
    pub width: f32,
    pub height: f32,
}

impl Default for WindowSettings {
    fn default() -> Self {
        Self {
            width: 1180.0,
            height: 780.0,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            theme: ThemePreference::Dark,
            verbose_logging: false,
            capture_seconds: 25,
            include_device_names_in_export: false,
            include_addresses_in_export: false,
            last_selected_device: None,
            window: WindowSettings::default(),
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), SettingsError> {
        if !(5..=120).contains(&self.capture_seconds) {
            return Err(SettingsError::Invalid(
                "capture duration must be between 5 and 120 seconds".into(),
            ));
        }
        if self.window.width < 760.0 || self.window.height < 520.0 {
            return Err(SettingsError::Invalid(
                "window size is below the supported minimum".into(),
            ));
        }
        Ok(())
    }

    pub fn load_or_default(path: &Path) -> Result<Self, SettingsError> {
        match fs::read(path) {
            Ok(bytes) => {
                let settings: Self = serde_json::from_slice(&bytes)
                    .map_err(|e| SettingsError::Parse(e.to_string()))?;
                settings.validate()?;
                Ok(settings)
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(SettingsError::Io(e.to_string())),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), SettingsError> {
        self.validate()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| SettingsError::Io(e.to_string()))?;
        }
        let tmp = path.with_extension("json.tmp");
        let bytes =
            serde_json::to_vec_pretty(self).map_err(|e| SettingsError::Parse(e.to_string()))?;
        fs::write(&tmp, bytes).map_err(|e| SettingsError::Io(e.to_string()))?;
        fs::rename(&tmp, path).map_err(|e| SettingsError::Io(e.to_string()))?;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("settings I/O error: {0}")]
    Io(String),
    #[error("settings parse error: {0}")]
    Parse(String),
    #[error("invalid settings: {0}")]
    Invalid(String),
}

pub fn app_data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("BLUU_STATE_DIR") {
        return PathBuf::from(dir);
    }
    directories::ProjectDirs::from("io", "Bluu", "Bluu")
        .map(|d| d.data_local_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".bluu"))
}

pub fn settings_path() -> PathBuf {
    app_data_dir().join("settings.json")
}

pub fn provider_dir() -> PathBuf {
    app_data_dir().join("providers")
}

pub fn provider_state_path() -> PathBuf {
    app_data_dir().join("provider-state.json")
}

pub fn log_dir() -> PathBuf {
    app_data_dir().join("logs")
}

pub fn export_dir() -> PathBuf {
    app_data_dir().join("exports")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let settings = Settings {
            theme: ThemePreference::Light,
            capture_seconds: 40,
            ..Settings::default()
        };
        settings.save(&path).unwrap();
        let loaded = Settings::load_or_default(&path).unwrap();
        assert_eq!(loaded.theme, ThemePreference::Light);
        assert_eq!(loaded.capture_seconds, 40);
    }

    #[test]
    fn capture_duration_is_validated() {
        let mut settings = Settings {
            capture_seconds: 2,
            ..Settings::default()
        };
        assert!(matches!(
            settings.validate(),
            Err(SettingsError::Invalid(_))
        ));
        settings.capture_seconds = 121;
        assert!(settings.validate().is_err());
    }

    #[test]
    fn missing_settings_use_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = Settings::load_or_default(&dir.path().join("none.json")).unwrap();
        assert_eq!(loaded.theme, ThemePreference::Dark);
    }
}
