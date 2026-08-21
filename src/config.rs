use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    pub overlay_x: Option<i32>,
    pub overlay_y: Option<i32>,
    pub scale: u16,
    pub opacity: u8,
    pub visible: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            overlay_x: None,
            overlay_y: None,
            scale: 100,
            opacity: 72,
            visible: true,
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
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("ludi-pq-stage-8-tool")
        .join("settings.json")
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
}
