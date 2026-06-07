use crate::color::Color;
use crate::debug;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

const GOOD: Color = Color::rgb(0x60, 0xb4, 0x8a);
const DEGRADED: Color = Color::rgb(0xdf, 0xaf, 0x8f);
const BAD: Color = Color::rgb(0xdc, 0xa3, 0xa3);

#[derive(Debug, Deserialize)]
#[serde(from = "shadow::Config")]
pub struct Config {
    pub bar: BarConfig,
    pub workspace: WorkspaceConfig,
    pub wireless: HashMap<String, WirelessConfig>,
    pub volume: HashMap<String, VolumeConfig>,
    pub battery: HashMap<String, BatteryConfig>,
    pub time: HashMap<String, TimeConfig>,
    pub cpu: HashMap<String, CpuConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(from = "shadow::BarConfig")]
pub struct BarConfig {
    pub font: String,
    pub width: u32,
    pub interval: Duration,
    pub separator: u32,
    pub blocks: Vec<String>,
    pub color: ColorConfig,
}

impl Default for BarConfig {
    fn default() -> Self {
        Self {
            font: "Sans Bold 9".into(),
            width: 28,
            interval: Duration::from_secs(10),
            separator: 14,
            blocks: vec!["volume.0".into(), "battery.0".into(), "time.0".into()],
            color: ColorConfig {
                text: Color::rgb(0x64, 0x64, 0x64),
                background: Color::rgb(0x0, 0x0, 0x0),
                border: Color::rgb(0x0, 0x0, 0x0),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
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
    pub margins: [i32; 4],
    pub borders: [i32; 4],
    pub height: i32,
}

impl BlockConfig {
    pub fn scaled(&self, scale: i32) -> Self {
        Self {
            margins: self.margins.map(|v| v * scale),
            borders: self.borders.map(|v| v * scale),
            height: self.height * scale,
        }
    }

    pub fn height(&self, min: i32) -> i32 {
        self.height.max(min) + self.margins[0] + self.margins[2]
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

    pub(crate) fn default() -> Self {
        Self {
            block: BlockConfig {
                height: 26,
                borders: [1, 1, 1, 1],
                margins: [0, 2, 2, 0],
            },
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
                    background: Color::rgb(0x77, 0x28, 0x2d),
                    border: Color::rgb(0x99, 0x4c, 0x4c),
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
                    text: DEGRADED,
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
    pub path: PathBuf,
    pub poll: bool,
    pub block: BlockConfig,
    pub color: ColorConfig,
    pub format: Vec<BatteryFormatItem>,

    // States.
    pub charging: BatteryStateConfig,
    pub full: BatteryStateConfig,
    pub idle: BatteryStateConfig,
    pub unknown: BatteryStateConfig,
    pub low: LowBatteryStateConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BatteryStateConfig {
    pub color: ColorConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LowBatteryStateConfig {
    pub state: BatteryStateConfig,
    pub threshold: u8,
}

impl BatteryConfig {
    pub(crate) fn default(color: &ColorConfig) -> Self {
        Self {
            path: "/sys/class/power_supply/BAT0/uevent".into(),
            poll: true,
            block: BlockConfig::default(),
            color: color.clone(),
            format: vec![
                BatteryFormatItem::Label("BAT".into()),
                BatteryFormatItem::Capacity,
            ],
            charging: BatteryStateConfig {
                color: ColorConfig {
                    text: GOOD,
                    ..*color
                },
            },
            full: BatteryStateConfig {
                color: color.clone(),
            },
            idle: BatteryStateConfig {
                color: color.clone(),
            },
            unknown: BatteryStateConfig {
                color: color.clone(),
            },
            low: LowBatteryStateConfig {
                state: BatteryStateConfig {
                    color: ColorConfig {
                        text: BAD,
                        ..*color
                    },
                },
                threshold: 20,
            },
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
pub struct WirelessConfig {
    pub interface: String,
    pub block: BlockConfig,
    pub color: ColorConfig,
    pub format: Vec<WirelessFormatItem>,
}

impl WirelessConfig {
    pub(crate) fn default(color: &ColorConfig) -> Self {
        Self {
            interface: "wlan0".into(),
            block: BlockConfig::default(),
            color: color.clone(),
            format: vec![
                WirelessFormatItem::Label("NET".into()),
                WirelessFormatItem::Quality,
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum WirelessFormatItem {
    Quality,
    Label(String),
}

impl WirelessFormatItem {
    pub(crate) fn parse(s: String) -> Self {
        match s.as_str() {
            "[quality]" => Self::Quality,
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

#[derive(Debug, Clone, PartialEq)]
pub struct CpuConfig {
    pub block: BlockConfig,
    pub color: ColorConfig,
    pub format: Vec<CpuFormatItem>,
}

impl CpuConfig {
    pub(crate) fn default(color: &ColorConfig) -> Self {
        Self {
            block: BlockConfig::default(),
            color: color.clone(),
            format: vec![CpuFormatItem::Label("CPU".into()), CpuFormatItem::Usage],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CpuFormatItem {
    Usage,
    Label(String),
}

impl CpuFormatItem {
    pub(crate) fn parse(s: String) -> Self {
        match s.as_str() {
            "[usage]" => Self::Usage,
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
    #[serde(default, deny_unknown_fields)]
    pub(super) struct Config {
        pub bar: BarConfig,
        pub workspace: WorkspaceConfig,
        pub wireless: HashMap<String, WirelessConfig>,
        pub volume: HashMap<String, VolumeConfig>,
        pub battery: HashMap<String, BatteryConfig>,
        pub time: HashMap<String, TimeConfig>,
        pub cpu: HashMap<String, CpuConfig>,
    }

    #[derive(Default, Deserialize)]
    #[serde(default, deny_unknown_fields)]
    pub(super) struct BarConfig {
        pub font: Option<String>,
        pub width: Option<u32>,
        pub interval: Option<u64>,
        pub separator: Option<u32>,
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
    #[serde(default, deny_unknown_fields)]
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
    #[serde(default, deny_unknown_fields)]
    pub(super) struct VolumeStateConfig {
        pub color: ColorConfig,
    }

    #[derive(Default, Deserialize)]
    #[serde(default)]
    pub(super) struct BatteryConfig {
        pub path: Option<std::path::PathBuf>,
        pub poll: Option<bool>,
        #[serde(flatten)]
        pub block: BlockConfig,
        pub color: ColorConfig,
        pub format: Option<Vec<String>>,
        pub charging: BatteryStateConfig,
        pub full: BatteryStateConfig,
        pub idle: BatteryStateConfig,
        pub unknown: BatteryStateConfig,
        pub low: LowBatteryStateConfig,
    }

    #[derive(Default, Deserialize)]
    #[serde(default, deny_unknown_fields)]
    pub(super) struct BatteryStateConfig {
        pub color: ColorConfig,
    }

    #[derive(Default, Deserialize)]
    #[serde(default, deny_unknown_fields)]
    pub(super) struct LowBatteryStateConfig {
        #[serde(flatten)]
        pub state: BatteryStateConfig,
        pub threshold: Option<u8>,
    }

    #[derive(Default, Deserialize)]
    #[serde(default)]
    pub(super) struct WirelessConfig {
        pub interface: Option<String>,
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
    pub(super) struct CpuConfig {
        #[serde(flatten)]
        pub block: BlockConfig,
        pub color: ColorConfig,
        pub format: Option<Vec<String>>,
    }

    #[derive(Default, Deserialize)]
    #[serde(default)]
    pub(super) struct BlockConfig {
        pub margins: Option<[i32; 4]>,
        pub borders: Option<[i32; 4]>,
        pub height: Option<i32>,
    }

    #[derive(Default, Deserialize)]
    #[serde(default, deny_unknown_fields)]
    pub(super) struct ColorConfig {
        pub text: Option<Color>,
        pub background: Option<Color>,
        pub border: Option<Color>,
    }

    impl WorkspaceConfig {
        pub(super) fn resolve(self, default: &super::WorkspaceConfig) -> super::WorkspaceConfig {
            super::WorkspaceConfig {
                block: self.block.resolve(&default.block),
                active: self.active.resolve(&default.active),
                inactive: self.inactive.resolve(&default.inactive),
                urgent: self.urgent.resolve(&default.urgent),
            }
        }
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

    impl BatteryStateConfig {
        pub(super) fn resolve(
            self,
            default: &super::BatteryStateConfig,
        ) -> super::BatteryStateConfig {
            super::BatteryStateConfig {
                color: self.color.resolve(&default.color),
            }
        }
    }

    impl LowBatteryStateConfig {
        pub(super) fn resolve(
            self,
            default: &super::LowBatteryStateConfig,
        ) -> super::LowBatteryStateConfig {
            super::LowBatteryStateConfig {
                state: self.state.resolve(&default.state),
                threshold: self.threshold.unwrap_or(default.threshold),
            }
        }
    }

    impl BatteryConfig {
        pub(super) fn resolve(self, default: &super::BatteryConfig) -> super::BatteryConfig {
            super::BatteryConfig {
                path: self.path.unwrap_or_else(|| default.path.clone()),
                poll: self.poll.unwrap_or(default.poll),
                block: self.block.resolve(&default.block),
                color: self.color.resolve(&default.color),
                format: self
                    .format
                    .map(|v| v.into_iter().map(super::BatteryFormatItem::parse).collect())
                    .unwrap_or_else(|| default.format.clone()),
                charging: self.charging.resolve(&default.charging),
                full: self.full.resolve(&default.full),
                idle: self.idle.resolve(&default.idle),
                unknown: self.unknown.resolve(&default.unknown),
                low: self.low.resolve(&default.low),
            }
        }
    }

    impl WirelessConfig {
        pub(super) fn resolve(self, default: &super::WirelessConfig) -> super::WirelessConfig {
            super::WirelessConfig {
                interface: self.interface.unwrap_or_else(|| default.interface.clone()),
                block: self.block.resolve(&default.block),
                color: self.color.resolve(&default.color),
                format: self
                    .format
                    .map(|v| {
                        v.into_iter()
                            .map(super::WirelessFormatItem::parse)
                            .collect()
                    })
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

    impl CpuConfig {
        pub(super) fn resolve(self, default: &super::CpuConfig) -> super::CpuConfig {
            super::CpuConfig {
                block: self.block.resolve(&default.block),
                color: self.color.resolve(&default.color),
                format: self
                    .format
                    .map(|v| v.into_iter().map(super::CpuFormatItem::parse).collect())
                    .unwrap_or_else(|| default.format.clone()),
            }
        }
    }

    impl BlockConfig {
        pub(super) fn resolve(self, default: &super::BlockConfig) -> super::BlockConfig {
            super::BlockConfig {
                margins: self.margins.unwrap_or(default.margins),
                borders: self.borders.unwrap_or(default.borders),
                height: self.height.unwrap_or(default.height),
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
        let workspace = WorkspaceConfig::default();
        let cpu = CpuConfig::default(&bar.color);
        let wireless = WirelessConfig::default(&bar.color);
        let volume = VolumeConfig::default(&bar.color);
        let battery = BatteryConfig::default(&bar.color);
        let time = TimeConfig::default(&bar.color);
        Self {
            workspace: shadow.workspace.resolve(&workspace),
            cpu: shadow
                .cpu
                .into_iter()
                .map(|(name, config)| (name, config.resolve(&cpu)))
                .collect(),
            wireless: shadow
                .wireless
                .into_iter()
                .map(|(name, config)| (name, config.resolve(&wireless)))
                .collect(),
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
            interval: shadow
                .interval
                .map(Duration::from_secs)
                .unwrap_or(d.interval),
            separator: shadow.separator.unwrap_or(d.separator),
            blocks: shadow.blocks.unwrap_or(d.blocks),
            color: shadow.color.resolve(&d.color),
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
        assert_eq!(b.width, 28);
        assert_eq!(b.interval, Duration::from_secs(10));
        assert_eq!(b.separator, 14);
        assert_eq!(b.blocks, ["volume.0", "battery.0", "time.0"]);
        assert_eq!(b.color.text, Color::rgb(0x64, 0x64, 0x64));
        assert_eq!(b.color.background, Color::rgb(0, 0, 0));
        assert_eq!(b.color.border, Color::rgb(0, 0, 0));

        let w = config.workspace;
        assert_eq!(w.block.borders, [1, 1, 1, 1]);
        assert_eq!(w.block.margins, [0, 2, 2, 0]);
        assert_eq!(w.block.height, 26);
        assert_eq!(w.active.color.text, Color::rgb(0xff, 0xff, 0xff));
        assert_eq!(w.active.color.background, Color::rgb(0x28, 0x55, 0x77));
        assert_eq!(w.active.color.border, Color::rgb(0x4c, 0x78, 0x99));
        assert_eq!(w.inactive.color.text, Color::rgb(0x88, 0x88, 0x88));
        assert_eq!(w.inactive.color.background, Color::rgb(0x22, 0x22, 0x22));
        assert_eq!(w.inactive.color.border, Color::rgb(0x33, 0x33, 0x33));
        assert_eq!(w.urgent.color.text, Color::rgb(0xff, 0xff, 0xff));
        assert_eq!(w.urgent.color.background, Color::rgb(0x77, 0x28, 0x2d));
        assert_eq!(w.urgent.color.border, Color::rgb(0x99, 0x4c, 0x4c));

        // Maps are not auto-populated.
        assert_eq!(config.cpu.len(), 0);
        assert_eq!(config.wireless.len(), 0);
        assert_eq!(config.volume.len(), 0);
        assert_eq!(config.battery.len(), 0);
        assert_eq!(config.time.len(), 0);
    }

    #[test]
    fn bar_partial_override() {
        let config: Config = toml::from_str(
            r###"
            [bar]
            interval = 5

            [bar.color]
            background = "#aabbcc"
            "###,
        )
        .unwrap();

        let b = config.bar;
        assert_eq!(b.interval, Duration::from_secs(5));
        assert_eq!(b.color.text, Color::rgb(0x64, 0x64, 0x64));
        assert_eq!(b.color.background, Color::rgb(0xaa, 0xbb, 0xcc));
        assert_eq!(b.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn workspace_partial_override() {
        let config: Config = toml::from_str(
            r###"
            [workspace]
            margins = [10, 20, 30, 40]
            height = 50

            [workspace.inactive.color]
            background = "#112233"
            "###,
        )
        .unwrap();

        let w = config.workspace;
        assert_eq!(w.block.borders, [1, 1, 1, 1]);
        assert_eq!(w.block.margins, [10, 20, 30, 40]);
        assert_eq!(w.block.height, 50);
        assert_eq!(w.inactive.color.text, Color::rgb(0x88, 0x88, 0x88));
        assert_eq!(w.inactive.color.background, Color::rgb(0x11, 0x22, 0x33));
        assert_eq!(w.inactive.color.border, Color::rgb(0x33, 0x33, 0x33));
    }

    #[test]
    fn volume_partial_override() {
        let config: Config = toml::from_str(
            r###"
            [volume.0]
            margins = [1, 2, 3, 4]

            [volume.0.muted.color]
            background = "#aabbcc"
            "###,
        )
        .unwrap();

        let v = config.volume.get("0").unwrap();
        assert_eq!(v.block.margins, [1, 2, 3, 4]);
        assert_eq!(v.block.borders, [0, 0, 0, 0]);
        assert_eq!(v.muted.color.text, Color::rgb(0xdf, 0xaf, 0x8f));
        assert_eq!(v.muted.color.background, Color::rgb(0xaa, 0xbb, 0xcc));
        assert_eq!(v.muted.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn battery_defaults() {
        let config: Config = toml::from_str(
            r###"
            [battery.0]
            "###,
        )
        .unwrap();

        let b = config.battery.get("0").unwrap();
        assert_eq!(b.path, PathBuf::from("/sys/class/power_supply/BAT0/uevent"));
        assert_eq!(b.block.height, 0);
        assert_eq!(b.block.borders, [0, 0, 0, 0]);
        assert_eq!(b.block.margins, [0, 0, 0, 0]);
        assert_eq!(b.color.text, Color::rgb(0x64, 0x64, 0x64));
        assert_eq!(b.color.background, Color::rgb(0, 0, 0));
        assert_eq!(b.color.border, Color::rgb(0, 0, 0));
        assert_eq!(
            b.format,
            vec![
                BatteryFormatItem::Label("BAT".into()),
                BatteryFormatItem::Capacity,
            ]
        );
        assert_eq!(b.low.threshold, 20);
        assert_eq!(b.low.state.color.text, Color::rgb(0xdc, 0xa3, 0xa3));
        assert_eq!(b.low.state.color.background, Color::rgb(0, 0, 0));
        assert_eq!(b.low.state.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn battery_partial_override() {
        let config: Config = toml::from_str(
            r###"
            [battery.0]
            path = "/sys/class/power_supply/BAT1/uevent"
            margins = [1, 2, 3, 4]

            [battery.0.low]
            threshold = 15
            color.text = "#123456"

            [battery.0.color]
            background = "#aabbcc"
            "###,
        )
        .unwrap();

        let b = config.battery.get("0").unwrap();
        assert_eq!(b.path, PathBuf::from("/sys/class/power_supply/BAT1/uevent"));
        assert_eq!(b.block.height, 0);
        assert_eq!(b.block.borders, [0, 0, 0, 0]);
        assert_eq!(b.block.margins, [1, 2, 3, 4]);
        assert_eq!(b.color.text, Color::rgb(0x64, 0x64, 0x64));
        assert_eq!(b.color.background, Color::rgb(0xaa, 0xbb, 0xcc));
        assert_eq!(b.color.border, Color::rgb(0, 0, 0));
        assert_eq!(b.low.threshold, 15);
        assert_eq!(b.low.state.color.text, Color::rgb(0x12, 0x34, 0x56));
        assert_eq!(b.low.state.color.background, Color::rgb(0, 0, 0));
        assert_eq!(b.low.state.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn wireless_defaults() {
        let config: Config = toml::from_str(
            r###"
            [wireless.0]
            "###,
        )
        .unwrap();

        let w = config.wireless.get("0").unwrap();
        assert_eq!(w.interface, "wlan0");
        assert_eq!(w.block.height, 0);
        assert_eq!(w.block.borders, [0, 0, 0, 0]);
        assert_eq!(w.block.margins, [0, 0, 0, 0]);
        assert_eq!(w.color.text, Color::rgb(0x64, 0x64, 0x64));
        assert_eq!(w.color.background, Color::rgb(0, 0, 0));
        assert_eq!(w.color.border, Color::rgb(0, 0, 0));
        assert_eq!(
            w.format,
            vec![
                WirelessFormatItem::Label("NET".into()),
                WirelessFormatItem::Quality,
            ]
        );
    }

    #[test]
    fn wireless_partial_override() {
        let config: Config = toml::from_str(
            r###"
            [wireless.0]
            interface = "wlp3s0"
            margins = [1, 2, 3, 4]

            [wireless.0.color]
            background = "#aabbcc"
            "###,
        )
        .unwrap();

        let w = config.wireless.get("0").unwrap();
        assert_eq!(w.interface, "wlp3s0");
        assert_eq!(w.block.height, 0);
        assert_eq!(w.block.borders, [0, 0, 0, 0]);
        assert_eq!(w.block.margins, [1, 2, 3, 4]);
        assert_eq!(w.color.text, Color::rgb(0x64, 0x64, 0x64));
        assert_eq!(w.color.background, Color::rgb(0xaa, 0xbb, 0xcc));
        assert_eq!(w.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn wireless_format_parses_tokens_and_labels() {
        let config: Config = toml::from_str(
            r###"
            [wireless.0]
            format = ["[quality]", "hello"]
            "###,
        )
        .unwrap();

        let w = config.wireless.get("0").unwrap();
        assert_eq!(
            w.format,
            vec![
                WirelessFormatItem::Quality,
                WirelessFormatItem::Label("hello".into()),
            ]
        );
    }

    #[test]
    fn time_partial_override() {
        let config: Config = toml::from_str(
            r###"
            [time.0]
            margins = [1, 2, 3, 4]

            [time.0.color]
            background = "#aabbcc"
            "###,
        )
        .unwrap();

        let t = config.time.get("0").unwrap();
        assert_eq!(t.block.margins, [1, 2, 3, 4]);
        assert_eq!(t.block.borders, [0, 0, 0, 0]);
        assert_eq!(t.color.text, Color::rgb(0x64, 0x64, 0x64));
        assert_eq!(t.color.background, Color::rgb(0xaa, 0xbb, 0xcc));
        assert_eq!(t.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn time_format_default() {
        let config: Config = toml::from_str(
            r###"
            [time.0]
            "###,
        )
        .unwrap();

        let t = config.time.get("0").unwrap();
        assert_eq!(t.format, vec![TimeFormatItem::Hour, TimeFormatItem::Minute]);
    }

    #[test]
    fn time_format_parses_tokens_and_labels() {
        let config: Config = toml::from_str(
            r###"
            [time.0]
            format = ["[hour]", "[minute]", "[day]", "[month]", "hello"]
            "###,
        )
        .unwrap();

        let t = config.time.get("0").unwrap();
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
            [volume.0]
            "###,
        )
        .unwrap();

        let v = config.volume.get("0").unwrap();
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
            [volume.0]
            format = ["[volume]", "hello"]
            "###,
        )
        .unwrap();

        let v = config.volume.get("0").unwrap();
        assert_eq!(
            v.format,
            vec![
                VolumeFormatItem::Volume,
                VolumeFormatItem::Label("hello".into()),
            ]
        );
    }

    #[test]
    fn battery_format_parses_tokens_and_labels() {
        let config: Config = toml::from_str(
            r###"
            [battery.0]
            format = ["[capacity]", "hello"]
            "###,
        )
        .unwrap();

        let b = config.battery.get("0").unwrap();
        assert_eq!(
            b.format,
            vec![
                BatteryFormatItem::Capacity,
                BatteryFormatItem::Label("hello".into()),
            ]
        );
    }

    #[test]
    fn cpu_format_parses_tokens_and_labels() {
        let config: Config = toml::from_str(
            r###"
            [cpu.0]
            format = ["[usage]", "hello"]
            "###,
        )
        .unwrap();

        let c = config.cpu.get("0").unwrap();
        assert_eq!(
            c.format,
            vec![CpuFormatItem::Usage, CpuFormatItem::Label("hello".into()),]
        );
    }

    #[test]
    fn block_height_override() {
        let config: Config = toml::from_str(
            r###"
            [time.0]
            height = 64

            [battery.0]
            margins = [1, 2, 3, 4]
            "###,
        )
        .unwrap();

        assert_eq!(config.time.get("0").unwrap().block.height, 64);
        assert_eq!(config.battery.get("0").unwrap().block.height, 0);
    }

    #[test]
    fn bar_color_propagation() {
        let config: Config = toml::from_str(
            r###"
            [bar.color]
            text = "#001122"
            background = "#334455"
            border = "#667788"

            [volume.0]
            [battery.0]
            [time.0]
            "###,
        )
        .unwrap();

        let bar_color = ColorConfig {
            text: Color::rgb(0x00, 0x11, 0x22),
            background: Color::rgb(0x33, 0x44, 0x55),
            border: Color::rgb(0x66, 0x77, 0x88),
        };
        assert_eq!(config.volume.get("0").unwrap().color, bar_color);
        assert_eq!(config.battery.get("0").unwrap().color, bar_color);
        assert_eq!(config.time.get("0").unwrap().color, bar_color);
        assert_eq!(
            config.volume.get("0").unwrap().muted.color,
            ColorConfig {
                text: Color::rgb(0xdf, 0xaf, 0x8f),
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

            [volume.0.color]
            text = "#111111"

            [time.0.color]
            background = "#222222"

            [battery.0.color]
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
            config.volume.get("0").unwrap().color,
            ColorConfig {
                text: Color::rgb(0x11, 0x11, 0x11),
                ..bar_color
            }
        );
        assert_eq!(
            config.time.get("0").unwrap().color,
            ColorConfig {
                background: Color::rgb(0x22, 0x22, 0x22),
                ..bar_color
            }
        );
        assert_eq!(
            config.battery.get("0").unwrap().color,
            ColorConfig {
                border: Color::rgb(0x33, 0x33, 0x33),
                ..bar_color
            }
        );
    }
}
