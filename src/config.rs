use crate::debug;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub bar: BarConfig,
    pub workspace: WorkspaceConfig,
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

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(from = "shadow::WorkspaceConfig")]
pub struct WorkspaceConfig {
    pub active: WorkspaceStateConfig,
    pub inactive: WorkspaceStateConfig,
    pub urgent: WorkspaceStateConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceStateConfig {
    pub gaps: [i32; 4],
    pub borders: [i32; 4],
}

impl WorkspaceConfig {
    pub fn scaled(&self, scale: i32) -> Self {
        Self {
            active: self.active.scaled(scale),
            inactive: self.inactive.scaled(scale),
            urgent: self.urgent.scaled(scale),
        }
    }
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            active: WorkspaceStateConfig {
                gaps: [0, 0, 0, 0],
                borders: [0, 0, 0, 0],
            },
            inactive: WorkspaceStateConfig {
                gaps: [0, 0, 0, 0],
                borders: [0, 0, 0, 0],
            },
            urgent: WorkspaceStateConfig {
                gaps: [0, 0, 0, 0],
                borders: [0, 0, 0, 0],
            },
        }
    }
}

impl WorkspaceStateConfig {
    pub fn scaled(&self, scale: i32) -> Self {
        Self {
            gaps: self.gaps.map(|v| v * scale),
            borders: self.borders.map(|v| v * scale),
        }
    }
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

mod shadow {
    use serde::Deserialize;

    #[derive(Default, Deserialize)]
    #[serde(default)]
    pub(super) struct WorkspaceConfig {
        pub active: WorkspaceStateConfig,
        pub inactive: WorkspaceStateConfig,
        pub urgent: WorkspaceStateConfig,
    }

    #[derive(Default, Deserialize)]
    #[serde(default)]
    pub(super) struct WorkspaceStateConfig {
        pub gaps: Option<[i32; 4]>,
        pub borders: Option<[i32; 4]>,
    }

    impl WorkspaceStateConfig {
        pub(super) fn resolve(
            self,
            default: &super::WorkspaceStateConfig,
        ) -> super::WorkspaceStateConfig {
            super::WorkspaceStateConfig {
                gaps: self.gaps.unwrap_or(default.gaps),
                borders: self.borders.unwrap_or(default.borders),
            }
        }
    }
}

impl From<shadow::WorkspaceConfig> for WorkspaceConfig {
    fn from(shadow: shadow::WorkspaceConfig) -> Self {
        let d = WorkspaceConfig::default();
        Self {
            active: shadow.active.resolve(&d.active),
            inactive: shadow.inactive.resolve(&d.inactive),
            urgent: shadow.urgent.resolve(&d.urgent),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_workspace_config(toml_str: &str) -> WorkspaceConfig {
        toml::from_str(toml_str).unwrap()
    }

    #[test]
    fn workspace_defaults() {
        let actual = parse_workspace_config("");

        assert_eq!(actual, WorkspaceConfig::default());
        assert_eq!(actual.active.gaps, [0, 0, 0, 0]);
        assert_eq!(actual.active.borders, [0, 0, 0, 0]);
        assert_eq!(actual.inactive.gaps, [0, 0, 0, 0]);
        assert_eq!(actual.inactive.borders, [0, 0, 0, 0]);
        assert_eq!(actual.urgent.gaps, [0, 0, 0, 0]);
        assert_eq!(actual.urgent.borders, [0, 0, 0, 0]);
    }

    #[test]
    fn workspace_partial_override() {
        let actual = parse_workspace_config(
            r#"
            [active]
            gaps = [10, 20, 30, 40]
            "#,
        );
        let mut expected = WorkspaceConfig::default();
        expected.active.gaps = [10, 20, 30, 40];

        assert_eq!(actual, expected);
    }
}
