use crate::color::Color;
use crate::debug;
use serde::Deserialize;
use std::path::PathBuf;

const COLOR_BACKGROUND: Color = Color::rgb(0, 0, 0);
const COLOR_TEXT: Color = Color::rgb(100, 100, 100);
const COLOR_DIMMED: Color = Color::rgb(50, 50, 50);

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub bar: BarConfig,
    pub workspace: WorkspaceConfig,
    pub volume: VolumeConfig,
    pub battery: BatteryConfig,
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
    pub block: BlockConfig,
    pub active: WorkspaceStateConfig,
    pub inactive: WorkspaceStateConfig,
    pub urgent: WorkspaceStateConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceStateConfig {
    pub color: ColorConfig,
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
#[serde(default)]
pub struct BlockConfig {
    pub gaps: [i32; 4],
    pub borders: [i32; 4],
}

impl BlockConfig {
    pub fn scaled(&self, scale: i32) -> Self {
        Self {
            gaps: self.gaps.map(|v| v * scale),
            borders: self.borders.map(|v| v * scale),
        }
    }
}

#[derive(Debug, Default, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct ColorConfig {
    pub text: Color,
    pub background: Color,
    pub border: Color,
}

impl WorkspaceConfig {
    pub fn scaled(&self, scale: i32) -> Self {
        Self {
            block: self.block.scaled(scale),
            active: self.active.clone(),
            inactive: self.inactive.clone(),
            urgent: self.urgent.clone(),
        }
    }
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            block: BlockConfig::default(),
            active: WorkspaceStateConfig {
                color: ColorConfig {
                    text: Color::rgb(0xff, 0xff, 0xff),
                    background: Color::rgb(0x28, 0x55, 0x77),
                    border: Color::rgb(0x4c, 0x78, 0x99),
                },
            },
            inactive: WorkspaceStateConfig {
                color: ColorConfig {
                    text: Color::rgb(0x88, 0x88, 0x88),
                    background: Color::rgb(0x22, 0x22, 0x22),
                    border: Color::rgb(0x33, 0x33, 0x33),
                },
            },
            urgent: WorkspaceStateConfig {
                color: ColorConfig {
                    text: Color::rgb(0xff, 0xff, 0xff),
                    background: Color::rgb(0x90, 0, 0),
                    border: Color::rgb(0x2f, 0x34, 0x3a),
                },
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(from = "shadow::VolumeConfig")]
pub struct VolumeConfig {
    pub block: BlockConfig,
    pub color: ColorConfig,
    pub muted: VolumeStateConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VolumeStateConfig {
    pub color: ColorConfig,
}

impl Default for VolumeConfig {
    fn default() -> Self {
        Self {
            block: BlockConfig::default(),
            color: ColorConfig {
                text: COLOR_TEXT,
                background: COLOR_BACKGROUND,
                border: COLOR_BACKGROUND,
            },
            muted: VolumeStateConfig {
                color: ColorConfig {
                    text: COLOR_DIMMED,
                    background: COLOR_BACKGROUND,
                    border: COLOR_BACKGROUND,
                },
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(from = "shadow::BatteryConfig")]
pub struct BatteryConfig {
    pub block: BlockConfig,
    pub color: ColorConfig,
}

impl Default for BatteryConfig {
    fn default() -> Self {
        Self {
            block: BlockConfig::default(),
            color: ColorConfig {
                text: COLOR_TEXT,
                background: COLOR_BACKGROUND,
                border: COLOR_BACKGROUND,
            },
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
    use crate::color::Color;
    use serde::Deserialize;

    #[derive(Default, Deserialize)]
    #[serde(default)]
    pub(super) struct WorkspaceConfig {
        #[serde(flatten)]
        pub block: BlockConfig,
        pub active: WorkspaceStateConfig,
        pub inactive: WorkspaceStateConfig,
        pub urgent: WorkspaceStateConfig,
    }

    #[derive(Default, Deserialize)]
    #[serde(default)]
    pub(super) struct WorkspaceStateConfig {
        pub color: ColorConfig,
    }

    #[derive(Default, Deserialize)]
    #[serde(default)]
    pub(super) struct VolumeConfig {
        #[serde(flatten)]
        pub block: BlockConfig,
        pub color: ColorConfig,
        pub muted: VolumeStateConfig,
    }

    #[derive(Default, Deserialize)]
    #[serde(default)]
    pub(super) struct VolumeStateConfig {
        pub color: ColorConfig,
    }

    #[derive(Default, Deserialize)]
    #[serde(default)]
    pub(super) struct BatteryConfig {
        #[serde(flatten)]
        pub block: BlockConfig,
        pub color: ColorConfig,
    }

    #[derive(Default, Deserialize)]
    #[serde(default)]
    pub(super) struct BlockConfig {
        pub gaps: Option<[i32; 4]>,
        pub borders: Option<[i32; 4]>,
    }

    #[derive(Default, Deserialize)]
    #[serde(default)]
    pub(super) struct ColorConfig {
        pub text: Option<Color>,
        pub background: Option<Color>,
        pub border: Option<Color>,
    }

    impl WorkspaceStateConfig {
        pub(super) fn resolve(
            self,
            default: &super::WorkspaceStateConfig,
        ) -> super::WorkspaceStateConfig {
            super::WorkspaceStateConfig {
                color: self.color.resolve(&default.color),
            }
        }
    }

    impl VolumeStateConfig {
        pub(super) fn resolve(
            self,
            default: &super::VolumeStateConfig,
        ) -> super::VolumeStateConfig {
            super::VolumeStateConfig {
                color: self.color.resolve(&default.color),
            }
        }
    }

    impl BlockConfig {
        pub(super) fn resolve(self, default: &super::BlockConfig) -> super::BlockConfig {
            super::BlockConfig {
                gaps: self.gaps.unwrap_or(default.gaps),
                borders: self.borders.unwrap_or(default.borders),
            }
        }
    }

    impl ColorConfig {
        pub(super) fn resolve(self, default: &super::ColorConfig) -> super::ColorConfig {
            super::ColorConfig {
                text: self.text.unwrap_or(default.text),
                background: self.background.unwrap_or(default.background),
                border: self.border.unwrap_or(default.border),
            }
        }
    }
}

impl From<shadow::WorkspaceConfig> for WorkspaceConfig {
    fn from(shadow: shadow::WorkspaceConfig) -> Self {
        let d = WorkspaceConfig::default();
        Self {
            block: shadow.block.resolve(&d.block),
            active: shadow.active.resolve(&d.active),
            inactive: shadow.inactive.resolve(&d.inactive),
            urgent: shadow.urgent.resolve(&d.urgent),
        }
    }
}

impl From<shadow::VolumeConfig> for VolumeConfig {
    fn from(shadow: shadow::VolumeConfig) -> Self {
        let d = VolumeConfig::default();
        Self {
            block: shadow.block.resolve(&d.block),
            color: shadow.color.resolve(&d.color),
            muted: shadow.muted.resolve(&d.muted),
        }
    }
}

impl From<shadow::BatteryConfig> for BatteryConfig {
    fn from(shadow: shadow::BatteryConfig) -> Self {
        let d = BatteryConfig::default();
        Self {
            block: shadow.block.resolve(&d.block),
            color: shadow.color.resolve(&d.color),
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
        assert_eq!(actual.block.gaps, [0, 0, 0, 0]);
        assert_eq!(actual.block.borders, [0, 0, 0, 0]);
        assert_eq!(actual.active.color.text, Color::rgb(0xff, 0xff, 0xff));
        assert_eq!(actual.active.color.background, Color::rgb(0x28, 0x55, 0x77));
        assert_eq!(actual.active.color.border, Color::rgb(0x4c, 0x78, 0x99));
        assert_eq!(actual.inactive.color.text, Color::rgb(0x88, 0x88, 0x88));
        assert_eq!(
            actual.inactive.color.background,
            Color::rgb(0x22, 0x22, 0x22)
        );
        assert_eq!(actual.inactive.color.border, Color::rgb(0x33, 0x33, 0x33));
        assert_eq!(actual.urgent.color.text, Color::rgb(0xff, 0xff, 0xff));
        assert_eq!(actual.urgent.color.background, Color::rgb(0x90, 0, 0));
        assert_eq!(actual.urgent.color.border, Color::rgb(0x2f, 0x34, 0x3a));
    }

    #[test]
    fn workspace_partial_override() {
        let actual = parse_workspace_config(
            r###"
            gaps = [10, 20, 30, 40]

            [inactive.color]
            background = "#112233"
            "###,
        );
        let mut expected = WorkspaceConfig::default();
        expected.block.gaps = [10, 20, 30, 40];
        expected.inactive.color.background = Color::rgb(0x11, 0x22, 0x33);

        assert_eq!(actual, expected);
    }

    #[test]
    fn volume_defaults() {
        let actual: VolumeConfig = toml::from_str("").unwrap();

        assert_eq!(actual, VolumeConfig::default());
        assert_eq!(actual.block.gaps, [0, 0, 0, 0]);
        assert_eq!(actual.block.borders, [0, 0, 0, 0]);
        assert_eq!(actual.muted.color.text, Color::rgb(50, 50, 50));
        assert_eq!(actual.muted.color.background, Color::rgb(0, 0, 0));
        assert_eq!(actual.muted.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn volume_partial_override() {
        let actual: VolumeConfig = toml::from_str(
            r###"
            gaps = [1, 2, 3, 4]

            [muted.color]
            background = "#aabbcc"
            "###,
        )
        .unwrap();

        assert_eq!(actual.block.gaps, [1, 2, 3, 4]);
        assert_eq!(actual.block.borders, [0, 0, 0, 0]);
        assert_eq!(actual.muted.color.text, Color::rgb(50, 50, 50));
        assert_eq!(actual.muted.color.background, Color::rgb(0xaa, 0xbb, 0xcc));
        assert_eq!(actual.muted.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn battery_defaults() {
        let actual: BatteryConfig = toml::from_str("").unwrap();

        assert_eq!(actual, BatteryConfig::default());
        assert_eq!(actual.block.gaps, [0, 0, 0, 0]);
        assert_eq!(actual.block.borders, [0, 0, 0, 0]);
        assert_eq!(actual.color.text, Color::rgb(100, 100, 100));
        assert_eq!(actual.color.background, Color::rgb(0, 0, 0));
        assert_eq!(actual.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn battery_partial_override() {
        let actual: BatteryConfig = toml::from_str(
            r###"
            gaps = [1, 2, 3, 4]

            [color]
            background = "#aabbcc"
            "###,
        )
        .unwrap();

        assert_eq!(actual.block.gaps, [1, 2, 3, 4]);
        assert_eq!(actual.block.borders, [0, 0, 0, 0]);
        assert_eq!(actual.color.text, Color::rgb(100, 100, 100));
        assert_eq!(actual.color.background, Color::rgb(0xaa, 0xbb, 0xcc));
        assert_eq!(actual.color.border, Color::rgb(0, 0, 0));
    }
}
