use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, sync::OnceLock};

const PROJECT_CONFIG_JSON: &str = include_str!("../project-config.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectConfig {
    service_origin: String,
}

pub fn service_origin() -> &'static str {
    static SERVICE_ORIGIN: OnceLock<String> = OnceLock::new();
    SERVICE_ORIGIN
        .get_or_init(|| {
            let config = serde_json::from_str::<ProjectConfig>(PROJECT_CONFIG_JSON)
                .expect("project-config.json must be valid");
            let origin = config.service_origin.trim_end_matches('/');
            assert!(
                origin.starts_with("https://") && !origin[8..].contains('/'),
                "project-config.json serviceOrigin must be an HTTPS origin"
            );
            origin.to_owned()
        })
        .as_str()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    pub overlay_x: Option<i32>,
    pub overlay_y: Option<i32>,
    pub scale: u16,
    pub opacity: u8,
    pub visible: bool,
    pub owner_guid: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            overlay_x: None,
            overlay_y: None,
            scale: 100,
            opacity: 72,
            visible: true,
            owner_guid: None,
        }
    }
}

pub struct SettingsStore {
    path: PathBuf,
    pub data: Settings,
}

impl SettingsStore {
    pub fn load() -> Self {
        let path = settings_path();
        let mut data = fs::read(&path)
            .or_else(|_| fs::read(legacy_settings_path()))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Settings>(&bytes).ok())
            .unwrap_or_default();
        data.scale = data.scale.clamp(50, 300);
        data.opacity = data.opacity.clamp(15, 100);
        Self { path, data }
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create settings directory {}", parent.display()))?;
        }
        let json = serde_json::to_vec_pretty(&self.data)?;
        fs::write(&self.path, json)
            .with_context(|| format!("write settings file {}", self.path.display()))
    }
}

fn settings_path() -> PathBuf {
    settings_root()
        .join("lpq101-stage-8-helper")
        .join("settings.json")
}

fn legacy_settings_path() -> PathBuf {
    settings_root()
        .join("ludi-pq-stage-8-tool")
        .join("settings.json")
}

fn settings_root() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        let settings = Settings::default();
        assert_eq!(settings.scale, 100);
        assert_eq!(settings.opacity, 72);
        assert!(settings.visible);
    }

    #[test]
    fn shared_project_origin_is_valid() {
        let origin = service_origin();
        assert!(origin.starts_with("https://"));
        assert!(!origin[8..].contains('/'));
    }
}
