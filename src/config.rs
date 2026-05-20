use crate::color::Color;
use crate::debug;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
#[serde(from = "shadow::Config")]
pub struct Config {
    pub bar: BarConfig,
    pub workspace: WorkspaceConfig,
    pub volume: HashMap<String, VolumeConfig>,
    pub battery: HashMap<String, BatteryConfig>,
    pub time: HashMap<String, TimeConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(from = "shadow::BarConfig")]
pub struct BarConfig {
    pub font: String,
    pub width: u32,
    pub blocks: Vec<String>,
    pub color: ColorConfig,
}

impl Default for BarConfig {
    fn default() -> Self {
        Self {
            font: "Sans Bold".into(),
            width: 28,
            blocks: vec![
                "volume.default".into(),
                "battery.default".into(),
                "time.default".into(),
            ],
            color: ColorConfig {
                text: Color::rgb(0x64, 0x64, 0x64),
                background: Color::rgb(0x0, 0x0, 0x0),
                border: Color::rgb(0x0, 0x0, 0x0),
            },
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

#[derive(Debug, Clone, PartialEq)]
pub struct VolumeConfig {
    pub block: BlockConfig,
    pub color: ColorConfig,
    pub muted: VolumeStateConfig,
    pub format: Vec<VolumeFormatItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VolumeStateConfig {
    pub color: ColorConfig,
}

impl VolumeConfig {
    pub(crate) fn default(color: &ColorConfig) -> Self {
        Self {
            block: BlockConfig::default(),
            color: color.clone(),
            muted: VolumeStateConfig {
                color: ColorConfig {
                    text: Color::rgb(0x32, 0x32, 0x32),
                    ..*color
                },
            },
            format: vec![
                VolumeFormatItem::Label("VOL".into()),
                VolumeFormatItem::Volume,
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum VolumeFormatItem {
    Volume,
    Label(String),
}

impl VolumeFormatItem {
    pub(crate) fn parse(s: String) -> Self {
        match s.as_str() {
            "[volume]" => Self::Volume,
            _ => Self::Label(s),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BatteryConfig {
    pub block: BlockConfig,
    pub color: ColorConfig,
    pub format: Vec<BatteryFormatItem>,
}

impl BatteryConfig {
    pub(crate) fn default(color: &ColorConfig) -> Self {
        Self {
            block: BlockConfig::default(),
            color: color.clone(),
            format: vec![
                BatteryFormatItem::Label("BAT".into()),
                BatteryFormatItem::Capacity,
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BatteryFormatItem {
    Capacity,
    Label(String),
}

impl BatteryFormatItem {
    pub(crate) fn parse(s: String) -> Self {
        match s.as_str() {
            "[capacity]" => Self::Capacity,
            _ => Self::Label(s),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimeConfig {
    pub block: BlockConfig,
    pub color: ColorConfig,
    pub format: Vec<TimeFormatItem>,
}

impl TimeConfig {
    pub(crate) fn default(color: &ColorConfig) -> Self {
        Self {
            block: BlockConfig::default(),
            color: color.clone(),
            format: vec![TimeFormatItem::Hour, TimeFormatItem::Minute],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TimeFormatItem {
    Hour,
    Minute,
    Day,
    Month,
    Label(String),
}

impl TimeFormatItem {
    pub(crate) fn parse(s: String) -> Self {
        match s.as_str() {
            "[hour]" => Self::Hour,
            "[minute]" => Self::Minute,
            "[day]" => Self::Day,
            "[month]" => Self::Month,
            _ => Self::Label(s),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::from(shadow::Config::default())
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
    use std::collections::HashMap;

    #[derive(Default, Deserialize)]
    #[serde(default)]
    pub(super) struct Config {
        pub bar: BarConfig,
        pub workspace: WorkspaceConfig,
        pub volume: HashMap<String, VolumeConfig>,
        pub battery: HashMap<String, BatteryConfig>,
        pub time: HashMap<String, TimeConfig>,
    }

    #[derive(Default, Deserialize)]
    #[serde(default)]
    pub(super) struct BarConfig {
        pub font: Option<String>,
        pub width: Option<u32>,
        pub blocks: Option<Vec<String>>,
        pub color: ColorConfig,
    }

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
        pub format: Option<Vec<String>>,
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
        pub format: Option<Vec<String>>,
    }

    #[derive(Default, Deserialize)]
    #[serde(default)]
    pub(super) struct TimeConfig {
        #[serde(flatten)]
        pub block: BlockConfig,
        pub color: ColorConfig,
        pub format: Option<Vec<String>>,
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

    impl VolumeConfig {
        pub(super) fn resolve(self, default: &super::VolumeConfig) -> super::VolumeConfig {
            super::VolumeConfig {
                block: self.block.resolve(&default.block),
                color: self.color.resolve(&default.color),
                muted: self.muted.resolve(&default.muted),
                format: self
                    .format
                    .map(|v| v.into_iter().map(super::VolumeFormatItem::parse).collect())
                    .unwrap_or_else(|| default.format.clone()),
            }
        }
    }

    impl BatteryConfig {
        pub(super) fn resolve(self, default: &super::BatteryConfig) -> super::BatteryConfig {
            super::BatteryConfig {
                block: self.block.resolve(&default.block),
                color: self.color.resolve(&default.color),
                format: self
                    .format
                    .map(|v| v.into_iter().map(super::BatteryFormatItem::parse).collect())
                    .unwrap_or_else(|| default.format.clone()),
            }
        }
    }

    impl TimeConfig {
        pub(super) fn resolve(self, default: &super::TimeConfig) -> super::TimeConfig {
            super::TimeConfig {
                block: self.block.resolve(&default.block),
                color: self.color.resolve(&default.color),
                format: self
                    .format
                    .map(|v| v.into_iter().map(super::TimeFormatItem::parse).collect())
                    .unwrap_or_else(|| default.format.clone()),
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

impl From<shadow::Config> for Config {
    fn from(shadow: shadow::Config) -> Self {
        let bar = BarConfig::from(shadow.bar);
        let volume = VolumeConfig::default(&bar.color);
        let battery = BatteryConfig::default(&bar.color);
        let time = TimeConfig::default(&bar.color);
        Self {
            workspace: WorkspaceConfig::from(shadow.workspace),
            volume: shadow
                .volume
                .into_iter()
                .map(|(name, config)| (name, config.resolve(&volume)))
                .collect(),
            battery: shadow
                .battery
                .into_iter()
                .map(|(name, config)| (name, config.resolve(&battery)))
                .collect(),
            time: shadow
                .time
                .into_iter()
                .map(|(name, config)| (name, config.resolve(&time)))
                .collect(),
            bar,
        }
    }
}

impl From<shadow::BarConfig> for BarConfig {
    fn from(shadow: shadow::BarConfig) -> Self {
        let d = BarConfig::default();
        Self {
            font: shadow.font.unwrap_or(d.font),
            width: shadow.width.unwrap_or(d.width),
            blocks: shadow.blocks.unwrap_or(d.blocks),
            color: shadow.color.resolve(&d.color),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_defaults() {
        let config: Config = toml::from_str("").unwrap();

        let b = config.bar;
        assert_eq!(
            b.blocks,
            ["volume.default", "battery.default", "time.default"]
        );
        assert_eq!(b.color.text, Color::rgb(0x64, 0x64, 0x64));
        assert_eq!(b.color.background, Color::rgb(0, 0, 0));
        assert_eq!(b.color.border, Color::rgb(0, 0, 0));

        let w = config.workspace;
        assert_eq!(w.block.gaps, [0, 0, 0, 0]);
        assert_eq!(w.block.borders, [0, 0, 0, 0]);
        assert_eq!(w.active.color.text, Color::rgb(0xff, 0xff, 0xff));
        assert_eq!(w.active.color.background, Color::rgb(0x28, 0x55, 0x77));
        assert_eq!(w.active.color.border, Color::rgb(0x4c, 0x78, 0x99));
        assert_eq!(w.inactive.color.text, Color::rgb(0x88, 0x88, 0x88));
        assert_eq!(w.inactive.color.background, Color::rgb(0x22, 0x22, 0x22));
        assert_eq!(w.inactive.color.border, Color::rgb(0x33, 0x33, 0x33));
        assert_eq!(w.urgent.color.text, Color::rgb(0xff, 0xff, 0xff));
        assert_eq!(w.urgent.color.background, Color::rgb(0x90, 0, 0));
        assert_eq!(w.urgent.color.border, Color::rgb(0x2f, 0x34, 0x3a));

        // Maps are not auto-populated.
        assert_eq!(config.volume.len(), 0);
        assert_eq!(config.battery.len(), 0);
        assert_eq!(config.time.len(), 0);
    }

    #[test]
    fn bar_partial_override() {
        let config: Config = toml::from_str(
            r###"
            [bar.color]
            background = "#aabbcc"
            "###,
        )
        .unwrap();

        let b = config.bar;
        assert_eq!(b.color.text, Color::rgb(0x64, 0x64, 0x64));
        assert_eq!(b.color.background, Color::rgb(0xaa, 0xbb, 0xcc));
        assert_eq!(b.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn workspace_partial_override() {
        let config: Config = toml::from_str(
            r###"
            [workspace]
            gaps = [10, 20, 30, 40]

            [workspace.inactive.color]
            background = "#112233"
            "###,
        )
        .unwrap();

        let w = config.workspace;
        assert_eq!(w.block.gaps, [10, 20, 30, 40]);
        assert_eq!(w.block.borders, [0, 0, 0, 0]);
        assert_eq!(w.inactive.color.text, Color::rgb(0x88, 0x88, 0x88));
        assert_eq!(w.inactive.color.background, Color::rgb(0x11, 0x22, 0x33));
        assert_eq!(w.inactive.color.border, Color::rgb(0x33, 0x33, 0x33));
    }

    #[test]
    fn volume_partial_override() {
        let config: Config = toml::from_str(
            r###"
            [volume.default]
            gaps = [1, 2, 3, 4]

            [volume.default.muted.color]
            background = "#aabbcc"
            "###,
        )
        .unwrap();

        let v = config.volume.get("default").unwrap();
        assert_eq!(v.block.gaps, [1, 2, 3, 4]);
        assert_eq!(v.block.borders, [0, 0, 0, 0]);
        assert_eq!(v.muted.color.text, Color::rgb(0x32, 0x32, 0x32));
        assert_eq!(v.muted.color.background, Color::rgb(0xaa, 0xbb, 0xcc));
        assert_eq!(v.muted.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn battery_partial_override() {
        let config: Config = toml::from_str(
            r###"
            [battery.default]
            gaps = [1, 2, 3, 4]

            [battery.default.color]
            background = "#aabbcc"
            "###,
        )
        .unwrap();

        let b = config.battery.get("default").unwrap();
        assert_eq!(b.block.gaps, [1, 2, 3, 4]);
        assert_eq!(b.block.borders, [0, 0, 0, 0]);
        assert_eq!(b.color.text, Color::rgb(0x64, 0x64, 0x64));
        assert_eq!(b.color.background, Color::rgb(0xaa, 0xbb, 0xcc));
        assert_eq!(b.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn time_partial_override() {
        let config: Config = toml::from_str(
            r###"
            [time.default]
            gaps = [1, 2, 3, 4]

            [time.default.color]
            background = "#aabbcc"
            "###,
        )
        .unwrap();

        let t = config.time.get("default").unwrap();
        assert_eq!(t.block.gaps, [1, 2, 3, 4]);
        assert_eq!(t.block.borders, [0, 0, 0, 0]);
        assert_eq!(t.color.text, Color::rgb(0x64, 0x64, 0x64));
        assert_eq!(t.color.background, Color::rgb(0xaa, 0xbb, 0xcc));
        assert_eq!(t.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn time_format_default() {
        let config: Config = toml::from_str(
            r###"
            [time.default]
            "###,
        )
        .unwrap();

        let t = config.time.get("default").unwrap();
        assert_eq!(t.format, vec![TimeFormatItem::Hour, TimeFormatItem::Minute]);
    }

    #[test]
    fn time_format_parses_tokens_and_labels() {
        let config: Config = toml::from_str(
            r###"
            [time.default]
            format = ["[hour]", "[minute]", "[day]", "[month]", "hello"]
            "###,
        )
        .unwrap();

        let t = config.time.get("default").unwrap();
        assert_eq!(
            t.format,
            vec![
                TimeFormatItem::Hour,
                TimeFormatItem::Minute,
                TimeFormatItem::Day,
                TimeFormatItem::Month,
                TimeFormatItem::Label("hello".into()),
            ]
        );
    }

    #[test]
    fn volume_format_default() {
        let config: Config = toml::from_str(
            r###"
            [volume.default]
            "###,
        )
        .unwrap();

        let v = config.volume.get("default").unwrap();
        assert_eq!(
            v.format,
            vec![
                VolumeFormatItem::Label("VOL".into()),
                VolumeFormatItem::Volume,
            ]
        );
    }

    #[test]
    fn volume_format_parses_tokens_and_labels() {
        let config: Config = toml::from_str(
            r###"
            [volume.default]
            format = ["[volume]", "hello"]
            "###,
        )
        .unwrap();

        let v = config.volume.get("default").unwrap();
        assert_eq!(
            v.format,
            vec![
                VolumeFormatItem::Volume,
                VolumeFormatItem::Label("hello".into()),
            ]
        );
    }

    #[test]
    fn battery_format_default() {
        let config: Config = toml::from_str(
            r###"
            [battery.default]
            "###,
        )
        .unwrap();

        let b = config.battery.get("default").unwrap();
        assert_eq!(
            b.format,
            vec![
                BatteryFormatItem::Label("BAT".into()),
                BatteryFormatItem::Capacity,
            ]
        );
    }

    #[test]
    fn battery_format_parses_tokens_and_labels() {
        let config: Config = toml::from_str(
            r###"
            [battery.default]
            format = ["[capacity]", "hello"]
            "###,
        )
        .unwrap();

        let b = config.battery.get("default").unwrap();
        assert_eq!(
            b.format,
            vec![
                BatteryFormatItem::Capacity,
                BatteryFormatItem::Label("hello".into()),
            ]
        );
    }

    #[test]
    fn bar_color_propagation() {
        let config: Config = toml::from_str(
            r###"
            [bar.color]
            text = "#001122"
            background = "#334455"
            border = "#667788"

            [volume.default]
            [battery.default]
            [time.default]
            "###,
        )
        .unwrap();

        let bar_color = ColorConfig {
            text: Color::rgb(0x00, 0x11, 0x22),
            background: Color::rgb(0x33, 0x44, 0x55),
            border: Color::rgb(0x66, 0x77, 0x88),
        };
        assert_eq!(config.volume.get("default").unwrap().color, bar_color);
        assert_eq!(config.battery.get("default").unwrap().color, bar_color);
        assert_eq!(config.time.get("default").unwrap().color, bar_color);
        assert_eq!(
            config.volume.get("default").unwrap().muted.color,
            ColorConfig {
                text: Color::rgb(0x32, 0x32, 0x32),
                ..bar_color
            }
        );
    }

    #[test]
    fn bar_color_propagation_partial_override() {
        let config: Config = toml::from_str(
            r###"
            [bar.color]
            text = "#001122"
            background = "#334455"
            border = "#667788"

            [volume.default.color]
            text = "#111111"

            [time.default.color]
            background = "#222222"

            [battery.default.color]
            border = "#333333"
            "###,
        )
        .unwrap();

        let bar_color = ColorConfig {
            text: Color::rgb(0x00, 0x11, 0x22),
            background: Color::rgb(0x33, 0x44, 0x55),
            border: Color::rgb(0x66, 0x77, 0x88),
        };
        assert_eq!(
            config.volume.get("default").unwrap().color,
            ColorConfig {
                text: Color::rgb(0x11, 0x11, 0x11),
                ..bar_color
            }
        );
        assert_eq!(
            config.time.get("default").unwrap().color,
            ColorConfig {
                background: Color::rgb(0x22, 0x22, 0x22),
                ..bar_color
            }
        );
        assert_eq!(
            config.battery.get("default").unwrap().color,
            ColorConfig {
                border: Color::rgb(0x33, 0x33, 0x33),
                ..bar_color
            }
        );
    }
}
