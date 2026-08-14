use crate::blocks::FormatItem;
use crate::color::Color;
use crate::{debug, error, fail, warning};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

const GOOD: Color = Color::rgb(0x60, 0xb4, 0x8a);
const DEGRADED: Color = Color::rgb(0xdf, 0xaf, 0x8f);
const BAD: Color = Color::rgb(0xdc, 0xa3, 0xa3);

#[derive(Debug, Deserialize)]
#[serde(from = "toml::Table")]
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
#[serde(default, deny_unknown_fields)]
pub struct BarConfig {
    pub fonts: String,
    pub width: u32,
    #[serde(deserialize_with = "duration_from_secs")]
    pub interval: Duration,
    pub separator: u32,
    pub blocks: Vec<String>,
    pub color: ColorConfig,
}

impl Default for BarConfig {
    fn default() -> Self {
        Self {
            fonts: "Sans Bold 9".into(),
            width: 28,
            interval: Duration::from_secs(10),
            separator: 14,
            blocks: vec!["cpu.0".into(), "volume.0".into(), "time.0".into()],
            color: ColorConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceConfig {
    pub block: BlockConfig,
    pub active: StateConfig,
    pub inactive: StateConfig,
    pub urgent: StateConfig,
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
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

    fn visit(&mut self, toml: &mut Toml) {
        toml.get("margins").set(&mut self.margins);
        toml.get("borders").set(&mut self.borders);
        toml.get("height").set(&mut self.height);
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ColorConfig {
    pub text: Color,
    pub background: Color,
    pub border: Color,
}

impl Default for ColorConfig {
    fn default() -> Self {
        Self {
            text: Color::rgb(0x64, 0x64, 0x64),
            background: Color::rgb(0, 0, 0),
            border: Color::rgb(0, 0, 0),
        }
    }
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
            active: StateConfig {
                color: ColorConfig {
                    text: Color::rgb(0xff, 0xff, 0xff),
                    background: Color::rgb(0x28, 0x55, 0x77),
                    border: Color::rgb(0x4c, 0x78, 0x99),
                },
                format: Vec::new(),
            },
            inactive: StateConfig {
                color: ColorConfig {
                    text: Color::rgb(0x88, 0x88, 0x88),
                    background: Color::rgb(0x22, 0x22, 0x22),
                    border: Color::rgb(0x33, 0x33, 0x33),
                },
                format: Vec::new(),
            },
            urgent: StateConfig {
                color: ColorConfig {
                    text: Color::rgb(0xff, 0xff, 0xff),
                    background: Color::rgb(0x77, 0x28, 0x2d),
                    border: Color::rgb(0x99, 0x4c, 0x4c),
                },
                format: Vec::new(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VolumeConfig {
    pub block: BlockConfig,
    pub color: ColorConfig,
    pub muted: StateConfig<VolumeFormatItem>,
    pub format: Vec<VolumeFormatItem>,
}

impl VolumeConfig {
    pub(crate) fn default(color: &ColorConfig) -> Self {
        Self {
            block: BlockConfig::default(),
            color: color.clone(),
            muted: StateConfig {
                color: ColorConfig {
                    text: DEGRADED,
                    ..*color
                },
                format: vec![
                    VolumeFormatItem::Label("MUT".into()),
                    VolumeFormatItem::Volume,
                ],
            },
            format: vec![
                VolumeFormatItem::Label("VOL".into()),
                VolumeFormatItem::Volume,
            ],
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(from = "String")]
pub enum VolumeFormatItem {
    Volume,
    Label(String),
}

impl From<String> for VolumeFormatItem {
    fn from(s: String) -> Self {
        match s.as_str() {
            "[volume]" => Self::Volume,
            _ => Self::Label(s),
        }
    }
}

impl FormatItem for VolumeFormatItem {
    fn label(&self) -> Option<&str> {
        if let VolumeFormatItem::Label(s) = self {
            Some(s)
        } else {
            None
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
    pub charging: StateConfig<BatteryFormatItem>,
    pub full: StateConfig<BatteryFormatItem>,
    pub idle: StateConfig<BatteryFormatItem>,
    pub unknown: StateConfig<BatteryFormatItem>,
    pub low: ThresholdStateConfig<BatteryFormatItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StateConfig<T = ()> {
    pub color: ColorConfig,
    pub format: Vec<T>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThresholdStateConfig<T = ()> {
    pub state: StateConfig<T>,
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
            charging: StateConfig {
                color: ColorConfig {
                    text: GOOD,
                    ..*color
                },
                format: vec![
                    BatteryFormatItem::Label("CHR".into()),
                    BatteryFormatItem::Capacity,
                ],
            },
            full: StateConfig {
                color: color.clone(),
                format: vec![
                    BatteryFormatItem::Label("FUL".into()),
                    BatteryFormatItem::Capacity,
                ],
            },
            idle: StateConfig {
                color: color.clone(),
                format: vec![
                    BatteryFormatItem::Label("IDL".into()),
                    BatteryFormatItem::Capacity,
                ],
            },
            unknown: StateConfig {
                color: color.clone(),
                format: vec![
                    BatteryFormatItem::Label("UNK".into()),
                    BatteryFormatItem::Capacity,
                ],
            },
            low: ThresholdStateConfig {
                state: StateConfig {
                    color: ColorConfig {
                        text: BAD,
                        ..*color
                    },
                    format: vec![
                        BatteryFormatItem::Label("LOW".into()),
                        BatteryFormatItem::Capacity,
                    ],
                },
                threshold: 20,
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(from = "String")]
pub enum BatteryFormatItem {
    Capacity,
    Label(String),
}

impl From<String> for BatteryFormatItem {
    fn from(s: String) -> Self {
        match s.as_str() {
            "[capacity]" => Self::Capacity,
            _ => Self::Label(s),
        }
    }
}

impl FormatItem for BatteryFormatItem {
    fn label(&self) -> Option<&str> {
        if let BatteryFormatItem::Label(s) = self {
            Some(s)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WirelessConfig {
    pub interface: String,
    pub block: BlockConfig,
    pub color: ColorConfig,
    pub format: Vec<WirelessFormatItem>,
    pub low: ThresholdStateConfig,
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
            low: ThresholdStateConfig {
                state: StateConfig {
                    color: ColorConfig {
                        text: BAD,
                        ..*color
                    },
                    format: Vec::new(),
                },
                threshold: 50,
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(from = "String")]
pub enum WirelessFormatItem {
    Quality,
    Label(String),
}

impl From<String> for WirelessFormatItem {
    fn from(s: String) -> Self {
        match s.as_str() {
            "[quality]" => Self::Quality,
            _ => Self::Label(s),
        }
    }
}

impl FormatItem for WirelessFormatItem {
    fn label(&self) -> Option<&str> {
        if let WirelessFormatItem::Label(s) = self {
            Some(s)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimeConfig {
    pub timezone: tz::TimeZone,
    pub block: BlockConfig,
    pub color: ColorConfig,
    pub format: Vec<TimeFormatItem>,
}

impl TimeConfig {
    pub(crate) fn default(color: &ColorConfig) -> Self {
        let timezone = match tz::TimeZone::local() {
            Ok(tz) => tz,
            Err(e) => {
                warning!("Cannot get local time: {}", e);
                tz::TimeZone::utc()
            }
        };

        Self {
            timezone,
            block: BlockConfig::default(),
            color: color.clone(),
            format: vec![TimeFormatItem::Hour, TimeFormatItem::Minute],
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(from = "String")]
pub enum TimeFormatItem {
    Hour,
    Minute,
    Day,
    Month,
    Label(String),
}

impl From<String> for TimeFormatItem {
    fn from(s: String) -> Self {
        match s.as_str() {
            "[hour]" => Self::Hour,
            "[minute]" => Self::Minute,
            "[day]" => Self::Day,
            "[month]" => Self::Month,
            _ => Self::Label(s),
        }
    }
}

impl FormatItem for TimeFormatItem {
    fn label(&self) -> Option<&str> {
        if let TimeFormatItem::Label(s) = self {
            Some(s)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CpuConfig {
    pub block: BlockConfig,
    pub color: ColorConfig,
    pub format: Vec<CpuFormatItem>,
    pub high: ThresholdStateConfig,
}

impl CpuConfig {
    pub(crate) fn default(color: &ColorConfig) -> Self {
        Self {
            block: BlockConfig::default(),
            color: color.clone(),
            format: vec![CpuFormatItem::Label("CPU".into()), CpuFormatItem::Usage],
            high: ThresholdStateConfig {
                state: StateConfig {
                    color: ColorConfig {
                        text: BAD,
                        ..*color
                    },
                    format: Vec::new(),
                },
                threshold: 80,
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(from = "String")]
pub enum CpuFormatItem {
    Usage,
    Label(String),
}

impl From<String> for CpuFormatItem {
    fn from(s: String) -> Self {
        match s.as_str() {
            "[usage]" => Self::Usage,
            _ => Self::Label(s),
        }
    }
}

impl FormatItem for CpuFormatItem {
    fn label(&self) -> Option<&str> {
        if let CpuFormatItem::Label(s) = self {
            Some(s)
        } else {
            None
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bar: BarConfig::default(),
            workspace: WorkspaceConfig::default(),
            wireless: HashMap::new(),
            volume: HashMap::new(),
            battery: HashMap::new(),
            time: HashMap::new(),
            cpu: HashMap::new(),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        Self::from_dir(&default_config_path())
    }

    pub fn from_file(path: PathBuf) -> Self {
        let Some(toml) = load_toml(&path) else {
            fail!("{} not found", path.display());
        };

        toml.into()
    }

    fn from_dir(path: &Path) -> Self {
        let mut toml = load_toml(&path.join("config.toml")).unwrap_or_default();

        for p in config_paths(&path.join("conf.d")) {
            let Some(c) = load_toml(&p) else {
                error!("{} not found", p.display());
                continue;
            };

            merge_toml(&mut toml, c);
        }

        toml.into()
    }
}

pub fn default_config_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").expect("HOME not set");
            PathBuf::from(home).join(".config")
        });
    base.join("bare")
}

fn config_paths(path: &Path) -> Vec<PathBuf> {
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                return Vec::new();
            } else {
                panic!("Failed to read {}: {}", path.display(), e);
            }
        }
    };

    let mut paths = entries
        .filter_map(|entry| {
            let e = entry.expect("Failed to read entry");
            let t = e.file_type().expect("Failed to read entry type");

            if !t.is_file() {
                return None;
            }

            if let Some(ext) = e.path().extension()
                && ext == "toml"
            {
                Some(e.path())
            } else {
                None
            }
        })
        .collect::<Vec<PathBuf>>();
    paths.sort();
    paths
}

fn load_toml(path: &Path) -> Option<toml::Table> {
    let contents = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                return None;
            } else {
                panic!("Failed to load {}: {}", path.display(), e);
            }
        }
    };

    debug!("Load: {}", path.display());
    toml::from_str(&contents)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path.display(), e))
}

fn merge_toml(base: &mut toml::Table, update: toml::Table) {
    for (key, value) in update {
        match (base.get_mut(&key), value) {
            (Some(toml::Value::Table(base)), toml::Value::Table(update)) => {
                merge_toml(base, update);
            }
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

trait Table {
    fn visit<T: Visit>(self, config: &mut T);
}

impl Table for Option<toml::Table> {
    fn visit<T: Visit>(self, config: &mut T) {
        if let Some(table) = self {
            config.visit(Toml::new(table));
        }
    }
}

trait Set<T> {
    fn set(self, value: &mut T);
}

impl<T> Set<T> for Option<T> {
    fn set(self, value: &mut T) {
        if let Some(v) = self {
            *value = v;
        }
    }
}

struct Toml {
    table: toml::Table,
}

impl Toml {
    fn new(table: toml::Table) -> Self {
        Self { table }
    }

    fn get<T>(&mut self, key: &str) -> Option<T>
    where
        T: serde::de::DeserializeOwned,
    {
        self.table.remove(key).map(|value| {
            value
                .try_into()
                .unwrap_or_else(|e| panic!("invalid {}: {}", key, e))
        })
    }

    fn merge<T>(&mut self, key: &str, target: &mut HashMap<String, T>, default: T)
    where
        T: Clone + Visit,
    {
        if let Some(map) = self.get::<toml::Table>(key) {
            *target = map
                .into_iter()
                .map(|(name, value)| {
                    let table: toml::Table = value
                        .try_into()
                        .unwrap_or_else(|e| panic!("invalid {}: {}", name, e));
                    let mut instance = default.clone();
                    instance.visit(Self::new(table));
                    (name, instance)
                })
                .collect();
        }
    }

    fn empty(self) {
        if let Some(key) = self.table.keys().next() {
            panic!("unknown field {}", key);
        }
    }
}

trait Visit {
    fn visit(&mut self, toml: Toml);
}

impl Visit for ColorConfig {
    fn visit(&mut self, mut toml: Toml) {
        toml.get("text").set(&mut self.text);
        toml.get("background").set(&mut self.background);
        toml.get("border").set(&mut self.border);
        toml.empty();
    }
}

impl<T: serde::de::DeserializeOwned> Visit for StateConfig<T> {
    fn visit(&mut self, mut toml: Toml) {
        toml.get("color").visit(&mut self.color);
        toml.get("format").set(&mut self.format);
        toml.empty();
    }
}

impl<T: serde::de::DeserializeOwned> Visit for ThresholdStateConfig<T> {
    fn visit(&mut self, mut toml: Toml) {
        toml.get("threshold").set(&mut self.threshold);
        toml.get("color").visit(&mut self.state.color);
        toml.get("format").set(&mut self.state.format);
        toml.empty();
    }
}

impl Visit for WorkspaceConfig {
    fn visit(&mut self, mut toml: Toml) {
        self.block.visit(&mut toml);
        toml.get("active").visit(&mut self.active);
        toml.get("inactive").visit(&mut self.inactive);
        toml.get("urgent").visit(&mut self.urgent);
        toml.empty();
    }
}

impl Visit for VolumeConfig {
    fn visit(&mut self, mut toml: Toml) {
        self.block.visit(&mut toml);
        toml.get("format").set(&mut self.format);
        toml.get("color").visit(&mut self.color);
        toml.get("muted").visit(&mut self.muted);
        toml.empty();
    }
}

impl Visit for BatteryConfig {
    fn visit(&mut self, mut toml: Toml) {
        self.block.visit(&mut toml);
        toml.get("path").set(&mut self.path);
        toml.get("poll").set(&mut self.poll);
        toml.get("format").set(&mut self.format);
        toml.get("color").visit(&mut self.color);
        toml.get("charging").visit(&mut self.charging);
        toml.get("full").visit(&mut self.full);
        toml.get("idle").visit(&mut self.idle);
        toml.get("unknown").visit(&mut self.unknown);
        toml.get("low").visit(&mut self.low);
        toml.empty();
    }
}

impl Visit for WirelessConfig {
    fn visit(&mut self, mut toml: Toml) {
        self.block.visit(&mut toml);
        toml.get("interface").set(&mut self.interface);
        toml.get("format").set(&mut self.format);
        toml.get("color").visit(&mut self.color);
        toml.get("low").visit(&mut self.low);
        toml.empty();
    }
}

impl Visit for TimeConfig {
    fn visit(&mut self, mut toml: Toml) {
        self.block.visit(&mut toml);
        toml.get("timezone")
            .map(|tz: String| {
                tz::TimeZone::from_posix_tz(&tz)
                    .unwrap_or_else(|e| panic!("cannot read timezone {}: {}", tz, e))
            })
            .set(&mut self.timezone);
        toml.get("format").set(&mut self.format);
        toml.get("color").visit(&mut self.color);
        toml.empty();
    }
}

impl Visit for CpuConfig {
    fn visit(&mut self, mut toml: Toml) {
        self.block.visit(&mut toml);
        toml.get("format").set(&mut self.format);
        toml.get("color").visit(&mut self.color);
        toml.get("high").visit(&mut self.high);
        toml.empty();
    }
}

impl Visit for Config {
    fn visit(&mut self, mut toml: Toml) {
        toml.get("bar").set(&mut self.bar);
        toml.get("workspace").visit(&mut self.workspace);
        toml.merge("cpu", &mut self.cpu, CpuConfig::default(&self.bar.color));
        toml.merge(
            "wireless",
            &mut self.wireless,
            WirelessConfig::default(&self.bar.color),
        );
        toml.merge(
            "volume",
            &mut self.volume,
            VolumeConfig::default(&self.bar.color),
        );
        toml.merge(
            "battery",
            &mut self.battery,
            BatteryConfig::default(&self.bar.color),
        );
        toml.merge("time", &mut self.time, TimeConfig::default(&self.bar.color));
        toml.empty();
    }
}

impl From<toml::Table> for Config {
    fn from(table: toml::Table) -> Self {
        let mut config = Self::default();
        config.visit(Toml::new(table));
        config
    }
}

fn duration_from_secs<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    u64::deserialize(deserializer).map(Duration::from_secs)
}

#[cfg(test)]
mod tests {
    use crate::tests::tmp::Directory;

    use super::*;

    #[test]
    fn block_defaults() {
        let config: Config = toml::from_str("").unwrap();

        let b = config.bar;
        assert_eq!(b.width, 28);
        assert_eq!(b.interval, Duration::from_secs(10));
        assert_eq!(b.separator, 14);
        assert_eq!(b.blocks, ["cpu.0", "volume.0", "time.0"]);
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
    fn panics_on_unknown_root_field() {
        let result =
            std::panic::catch_unwind(|| toml::from_str::<Config>("unexpected = true").unwrap());

        assert!(result.is_err());
    }

    #[test]
    fn panics_on_unknown_section_fields() {
        let sections = [
            "[bar]",
            "[workspace]",
            "[cpu.0]",
            "[wireless.0]",
            "[volume.0]",
            "[battery.0]",
            "[time.0]",
            "[bar.color]",
            "[workspace.active]",
            "[cpu.0.high]",
        ];

        for section in sections {
            let input = format!("{section}\nunexpected = true");
            let result = std::panic::catch_unwind(|| toml::from_str::<Config>(&input).unwrap());

            assert!(result.is_err(), "{} accepted an unknown field", section);
        }
    }

    #[test]
    fn panics_on_invalid_nested_value() {
        let result = std::panic::catch_unwind(|| {
            toml::from_str::<Config>(
                r###"
                [cpu.0.color]
                text = 1
                "###,
            )
            .unwrap()
        });

        assert!(result.is_err());
    }

    #[test]
    fn bar_partial_override() {
        let config: Config = toml::from_str(
            r###"
            [bar]
            fonts = "Monospace 10"
            interval = 5

            [bar.color]
            background = "#aabbcc"
            "###,
        )
        .unwrap();

        let b = config.bar;
        assert_eq!(b.fonts, "Monospace 10");
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
    fn cpu_partial_override() {
        let config: Config = toml::from_str(
            r###"
            [cpu.0]
            margins = [1, 2, 3, 4]

            [cpu.0.high]
            threshold = 90
            color.text = "#123456"

            [cpu.0.color]
            background = "#aabbcc"
            "###,
        )
        .unwrap();

        let c = config.cpu.get("0").unwrap();
        assert_eq!(c.block.margins, [1, 2, 3, 4]);
        assert_eq!(c.block.borders, [0, 0, 0, 0]);
        assert_eq!(c.color.text, Color::rgb(0x64, 0x64, 0x64));
        assert_eq!(c.color.background, Color::rgb(0xaa, 0xbb, 0xcc));
        assert_eq!(c.color.border, Color::rgb(0, 0, 0));
        assert_eq!(c.high.threshold, 90);
        assert_eq!(c.high.state.color.text, Color::rgb(0x12, 0x34, 0x56));
        assert_eq!(c.high.state.color.background, Color::rgb(0, 0, 0));
        assert_eq!(c.high.state.color.border, Color::rgb(0, 0, 0));
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
        assert_eq!(
            b.charging.format,
            vec![
                BatteryFormatItem::Label("CHR".into()),
                BatteryFormatItem::Capacity,
            ]
        );
        assert_eq!(
            b.full.format,
            vec![
                BatteryFormatItem::Label("FUL".into()),
                BatteryFormatItem::Capacity,
            ]
        );
        assert_eq!(
            b.idle.format,
            vec![
                BatteryFormatItem::Label("IDL".into()),
                BatteryFormatItem::Capacity,
            ]
        );
        assert_eq!(
            b.unknown.format,
            vec![
                BatteryFormatItem::Label("UNK".into()),
                BatteryFormatItem::Capacity,
            ]
        );
        assert_eq!(
            b.low.state.format,
            vec![
                BatteryFormatItem::Label("LOW".into()),
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
        assert_eq!(w.low.threshold, 50);
        assert_eq!(w.low.state.color.text, Color::rgb(0xdc, 0xa3, 0xa3));
        assert_eq!(w.low.state.color.background, Color::rgb(0, 0, 0));
        assert_eq!(w.low.state.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn wireless_partial_override() {
        let config: Config = toml::from_str(
            r###"
            [wireless.0]
            interface = "wlp3s0"
            margins = [1, 2, 3, 4]

            [wireless.0.low]
            threshold = 40
            color.text = "#123456"

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
        assert_eq!(w.low.threshold, 40);
        assert_eq!(w.low.state.color.text, Color::rgb(0x12, 0x34, 0x56));
        assert_eq!(w.low.state.color.background, Color::rgb(0, 0, 0));
        assert_eq!(w.low.state.color.border, Color::rgb(0, 0, 0));
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
            timezone = "US/Pacific"

            [time.0.color]
            background = "#aabbcc"
            "###,
        )
        .unwrap();

        let t = config.time.get("0").unwrap();
        assert_eq!(
            t.timezone,
            tz::TimeZone::from_posix_tz("US/Pacific").unwrap()
        );
        assert_eq!(t.block.margins, [1, 2, 3, 4]);
        assert_eq!(t.block.borders, [0, 0, 0, 0]);
        assert_eq!(t.color.text, Color::rgb(0x64, 0x64, 0x64));
        assert_eq!(t.color.background, Color::rgb(0xaa, 0xbb, 0xcc));
        assert_eq!(t.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn time_defaults() {
        let config: Config = toml::from_str(
            r###"
            [time.0]
            "###,
        )
        .unwrap();

        let t = config.time.get("0").unwrap();
        assert_eq!(t.timezone, tz::TimeZone::local().unwrap());
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
        assert_eq!(
            v.muted.format,
            vec![
                VolumeFormatItem::Label("MUT".into()),
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

            [volume.0.muted]
            format = ["[volume]", "muted"]
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
        assert_eq!(
            v.muted.format,
            vec![
                VolumeFormatItem::Volume,
                VolumeFormatItem::Label("muted".into()),
            ]
        );
    }

    #[test]
    fn battery_format_parses_tokens_and_labels() {
        let config: Config = toml::from_str(
            r###"
            [battery.0]
            format = ["[capacity]", "hello"]

            [battery.0.charging]
            format = ["charging", "[capacity]"]

            [battery.0.low]
            format = ["low", "[capacity]"]
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
        assert_eq!(
            b.charging.format,
            vec![
                BatteryFormatItem::Label("charging".into()),
                BatteryFormatItem::Capacity,
            ]
        );
        assert_eq!(
            b.low.state.format,
            vec![
                BatteryFormatItem::Label("low".into()),
                BatteryFormatItem::Capacity,
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
    fn format_can_be_empty() {
        let config: Config = toml::from_str(
            r###"
            [cpu.0]
            format = []
            "###,
        )
        .unwrap();

        assert!(config.cpu.get("0").unwrap().format.is_empty());
    }

    #[test]
    fn format_rejects_non_string_items() {
        let result = std::panic::catch_unwind(|| {
            toml::from_str::<Config>(
                r###"
                [cpu.0]
                format = ["[usage]", 1]
                "###,
            )
            .unwrap()
        });

        assert!(result.is_err());
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

    #[test]
    fn multiple_configs_ignored_for_single_file() {
        let tmp = Directory::new();
        let path = tmp.path().join("loaded.toml");
        tmp.write(
            &path,
            r###"
            [bar]
            width = 20
            "###,
        );
        tmp.write(
            "conf.d/ignored.toml",
            r###"
            [bar]
            separator = 40
            "###,
        );

        let config = Config::from_file(path);

        assert_eq!(config.bar.width, 20);
        assert_eq!(config.bar.separator, 14);
    }

    #[test]
    fn multiple_configs_load_order() {
        let tmp = Directory::new();
        tmp.write(
            "config.toml",
            r###"
            [bar]
            width = 20
            blocks = ["cpu.0"]

            [bar.color]
            text = "#111111"
            background = "#222222"

            [cpu.0]
            format = ["CPU", "[usage]"]

            [volume.0]
            margins = [1, 2, 3, 4]
            "###,
        );
        tmp.write(
            "conf.d/10-first.toml",
            r###"
            [bar]
            width = 30
            blocks = ["volume.0"]

            [bar.color]
            border = "#333333"
            "###,
        );
        tmp.write(
            "conf.d/20-second.toml",
            r###"
            [bar]
            width = 40

            [bar.color]
            background = "#444444"
            "###,
        );

        let config = Config::from_dir(tmp.path());

        assert_eq!(config.bar.width, 40);
        assert_eq!(config.bar.blocks, ["volume.0"]);
        assert_eq!(config.bar.color.text, Color::rgb(0x11, 0x11, 0x11));
        assert_eq!(config.bar.color.background, Color::rgb(0x44, 0x44, 0x44));
        assert_eq!(config.bar.color.border, Color::rgb(0x33, 0x33, 0x33));
        assert_eq!(
            config.cpu.get("0").unwrap().format,
            [CpuFormatItem::Label("CPU".into()), CpuFormatItem::Usage]
        );
        assert_eq!(config.volume.get("0").unwrap().block.margins, [1, 2, 3, 4]);
        assert_eq!(
            config.volume.get("0").unwrap().color.background,
            Color::rgb(0x44, 0x44, 0x44)
        );
    }

    #[test]
    fn multiple_configs_no_main_config() {
        let tmp = Directory::new();
        tmp.write(
            "conf.d/bar.toml",
            r###"
            [bar]
            interval = 55
            "###,
        );

        let config = Config::from_dir(tmp.path());

        assert_eq!(config.bar.interval, Duration::from_secs(55));
        assert_eq!(config.bar.width, 28);
    }

    #[test]
    fn multiple_configs_only_toml() {
        let tmp = Directory::new();
        tmp.write(
            "config.toml",
            r###"
            [bar]
            width = 20
            "###,
        );
        tmp.write("conf.d/ignored.conf", "not toml");
        tmp.write("conf.d/ignored.toml/nested", "not a file");
        tmp.write(
            "conf.d/loaded.toml",
            r###"
            [bar]
            width = 30
            "###,
        );

        let config = Config::from_dir(tmp.path());

        assert_eq!(config.bar.width, 30);
    }
}
