use crate::debug;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub bar: BarConfig,
    pub workspaces: WorkspacesConfig,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct BarConfig {
    pub font: String,
    pub width: u32,
}

impl Default for BarConfig {
    fn default() -> Self {
        Self {
            font: "Sans Bold".into(),
            width: 28,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct WorkspacesConfig {
    pub gaps: [u32; 4],
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        let contents = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("Config file {} not found, using defaults", path.display());
                return Self::default();
            }
            Err(e) => panic!("Failed to read {}: {}", path.display(), e),
        };
        toml::from_str(&contents)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path.display(), e))
    }
}

fn config_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").expect("HOME not set");
            PathBuf::from(home).join(".config")
        });
    base.join("bare").join("config.toml")
}
