use anyhow::Result;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Config {
    pub recent_files: Vec<PathBuf>,
}

impl Config {
    pub fn load() -> Self {
        Self::get_path()
            .and_then(|path| fs::read_to_string(path).ok())
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        if let Some(path) = Self::get_path() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let content = serde_json::to_string_pretty(self)?;
            fs::write(path, content)?;
        }
        Ok(())
    }

    fn get_path() -> Option<PathBuf> {
        ProjectDirs::from("com", "GoldTone", "normie-rs")
            .map(|dirs| dirs.config_dir().join("config.json"))
    }

    pub fn add_recent(&mut self, path: PathBuf) {
        let path = fs::canonicalize(&path).unwrap_or(path);
        self.recent_files.retain(|p| p != &path);
        self.recent_files.insert(0, path);
        if self.recent_files.len() > 10 {
            self.recent_files.truncate(10);
        }
        let _ = self.save();
    }
}
