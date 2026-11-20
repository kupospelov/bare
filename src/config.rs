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

#[derive(Debug)]
pub struct Config {
    pub bar: BarConfig,
    pub workspace: WorkspaceConfig,
}

#[derive(Debug, Default)]
pub struct InstanceConfig {
    pub wireless: HashMap<String, WirelessConfig>,
    pub volume: HashMap<String, VolumeConfig>,
    pub battery: HashMap<String, BatteryConfig>,
    pub time: HashMap<String, TimeConfig>,
    pub cpu: HashMap<String, CpuConfig>,
    pub memory: HashMap<String, MemoryConfig>,
    pub drive: HashMap<String, DriveConfig>,
    pub file: HashMap<String, FileConfig>,
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
    pub size: SizeConfig,
    pub active: StateConfig,
    pub inactive: StateConfig,
    pub urgent: StateConfig,
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
pub struct BlockConfig {
    pub color: ColorConfig,

    #[serde(flatten)]
    pub size: SizeConfig,
}

impl BlockConfig {
    fn visit(&mut self, toml: &mut Toml) {
        toml.get("color").visit(&mut self.color);
        self.size.visit(toml);
    }
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
pub struct SizeConfig {
    pub margins: [i32; 4],
    pub borders: [i32; 4],
    pub corners: [u32; 4],
    pub height: i32,
}

impl SizeConfig {
    pub fn scaled(&self, scale: i32) -> Self {
        Self {
            margins: self.margins.map(|v| v * scale),
            borders: self.borders.map(|v| v * scale),
            corners: self.corners.map(|v| v * scale as u32),
            height: self.height * scale,
        }
    }

    pub fn height(&self, min: i32) -> i32 {
        self.height.max(min) + self.margins[0] + self.margins[2]
    }

    fn visit(&mut self, toml: &mut Toml) {
        toml.get("margins").set(&mut self.margins);
        toml.get("borders").set(&mut self.borders);
        toml.get("corners").set(&mut self.corners);
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
            size: self.size.scaled(scale),
            active: self.active.clone(),
            inactive: self.inactive.clone(),
            urgent: self.urgent.clone(),
        }
    }

    pub(crate) fn default() -> Self {
        Self {
            size: SizeConfig {
                height: 26,
                borders: [1; 4],
                corners: [0; 4],
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
    pub properties: HashMap<String, String>,
    pub down: StateConfig<VolumeFormatItem>,
    pub idle: StateConfig<VolumeFormatItem>,
    pub muted: StateConfig<VolumeFormatItem>,
    pub format: Vec<VolumeFormatItem>,
}

impl VolumeConfig {
    pub(crate) fn default(color: &ColorConfig) -> Self {
        let format = vec![
            VolumeFormatItem::Label("VOL".into()),
            VolumeFormatItem::Volume,
        ];

        Self {
            block: BlockConfig {
                color: color.clone(),
                ..BlockConfig::default()
            },
            properties: HashMap::new(),
            down: StateConfig {
                color: ColorConfig {
                    text: BAD,
                    ..*color
                },
                format: format.clone(),
            },
            idle: StateConfig {
                color: color.clone(),
                format: format.clone(),
            },
            muted: StateConfig {
                color: ColorConfig {
                    text: DEGRADED,
                    ..*color
                },
                format: format.clone(),
            },
            format,
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
    pub design_capacity: bool,
    pub block: BlockConfig,
    pub format: Vec<BatteryFormatItem>,

    // States.
    pub down: StateConfig<BatteryFormatItem>,
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
        let format = vec![
            BatteryFormatItem::Label("BAT".into()),
            BatteryFormatItem::Capacity,
        ];

        Self {
            path: "/sys/class/power_supply/BAT0/uevent".into(),
            poll: true,
            design_capacity: false,
            block: BlockConfig {
                color: color.clone(),
                ..BlockConfig::default()
            },
            format: format.clone(),
            down: StateConfig {
                color: ColorConfig {
                    text: BAD,
                    ..*color
                },
                format: format.clone(),
            },
            charging: StateConfig {
                color: ColorConfig {
                    text: GOOD,
                    ..*color
                },
                format: format.clone(),
            },
            full: StateConfig {
                color: color.clone(),
                format: format.clone(),
            },
            idle: StateConfig {
                color: color.clone(),
                format: format.clone(),
            },
            unknown: StateConfig {
                color: color.clone(),
                format: format.clone(),
            },
            low: ThresholdStateConfig {
                state: StateConfig {
                    color: ColorConfig {
                        text: BAD,
                        ..*color
                    },
                    format,
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
    pub format: Vec<WirelessFormatItem>,
    pub down: StateConfig<WirelessFormatItem>,
    pub low: ThresholdStateConfig<WirelessFormatItem>,
}

impl WirelessConfig {
    pub(crate) fn default(color: &ColorConfig) -> Self {
        let format = vec![
            WirelessFormatItem::Label("NET".into()),
            WirelessFormatItem::Quality,
        ];

        Self {
            interface: "wlan0".into(),
            block: BlockConfig {
                color: color.clone(),
                ..BlockConfig::default()
            },
            format: format.clone(),
            down: StateConfig {
                color: ColorConfig {
                    text: BAD,
                    ..*color
                },
                format: format.clone(),
            },
            low: ThresholdStateConfig {
                state: StateConfig {
                    color: ColorConfig {
                        text: BAD,
                        ..*color
                    },
                    format,
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
    pub weekdays: [String; 7],
    pub block: BlockConfig,
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
            weekdays: ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"].map(String::from),
            block: BlockConfig {
                color: color.clone(),
                ..BlockConfig::default()
            },
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
    Weekday,
    Label(String),
}

impl From<String> for TimeFormatItem {
    fn from(s: String) -> Self {
        match s.as_str() {
            "[hour]" => Self::Hour,
            "[minute]" => Self::Minute,
            "[day]" => Self::Day,
            "[month]" => Self::Month,
            "[weekday]" => Self::Weekday,
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
    pub format: Vec<CpuFormatItem>,
    pub high: ThresholdStateConfig<CpuFormatItem>,
}

impl CpuConfig {
    pub(crate) fn default(color: &ColorConfig) -> Self {
        let format = vec![CpuFormatItem::Label("CPU".into()), CpuFormatItem::Usage];

        Self {
            block: BlockConfig {
                color: color.clone(),
                ..BlockConfig::default()
            },
            format: format.clone(),
            high: ThresholdStateConfig {
                state: StateConfig {
                    color: ColorConfig {
                        text: BAD,
                        ..*color
                    },
                    format,
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

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryConfig {
    pub block: BlockConfig,
    pub format: Vec<MemoryFormatItem>,
    pub high: ThresholdStateConfig<MemoryFormatItem>,
}

impl MemoryConfig {
    pub(crate) fn default(color: &ColorConfig) -> Self {
        let format = vec![
            MemoryFormatItem::Label("MEM".into()),
            MemoryFormatItem::Usage,
        ];

        Self {
            block: BlockConfig {
                color: color.clone(),
                ..BlockConfig::default()
            },
            format: format.clone(),
            high: ThresholdStateConfig {
                state: StateConfig {
                    color: ColorConfig {
                        text: BAD,
                        ..*color
                    },
                    format,
                },
                threshold: 80,
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(from = "String")]
pub enum MemoryFormatItem {
    Usage,
    Label(String),
}

impl From<String> for MemoryFormatItem {
    fn from(s: String) -> Self {
        match s.as_str() {
            "[usage]" => Self::Usage,
            _ => Self::Label(s),
        }
    }
}

impl FormatItem for MemoryFormatItem {
    fn label(&self) -> Option<&str> {
        if let MemoryFormatItem::Label(s) = self {
            Some(s)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DriveConfig {
    pub path: PathBuf,
    pub block: BlockConfig,
    pub format: Vec<DriveFormatItem>,
    pub high: ThresholdStateConfig<DriveFormatItem>,
}

impl DriveConfig {
    pub(crate) fn default(color: &ColorConfig) -> Self {
        let format = vec![DriveFormatItem::Label("DRV".into()), DriveFormatItem::Usage];

        Self {
            path: PathBuf::from("/"),
            block: BlockConfig {
                color: color.clone(),
                ..BlockConfig::default()
            },
            format: format.clone(),
            high: ThresholdStateConfig {
                state: StateConfig {
                    color: ColorConfig {
                        text: BAD,
                        ..*color
                    },
                    format,
                },
                threshold: 80,
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(from = "String")]
pub enum DriveFormatItem {
    Usage,
    Label(String),
}

impl From<String> for DriveFormatItem {
    fn from(s: String) -> Self {
        match s.as_str() {
            "[usage]" => Self::Usage,
            _ => Self::Label(s),
        }
    }
}

impl FormatItem for DriveFormatItem {
    fn label(&self) -> Option<&str> {
        if let DriveFormatItem::Label(s) = self {
            Some(s)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileConfig {
    pub path: PathBuf,
    pub block: BlockConfig,
    pub format: Vec<String>,
    pub down: StateConfig<String>,
}

impl FileConfig {
    pub(crate) fn default(color: &ColorConfig) -> Self {
        let format = vec!["PTH".into()];
        Self {
            path: PathBuf::new(),
            block: BlockConfig {
                color: color.clone(),
                ..BlockConfig::default()
            },
            format: format.clone(),
            down: StateConfig {
                color: ColorConfig {
                    text: BAD,
                    ..*color
                },
                format,
            },
        }
    }
}

impl FormatItem for String {
    fn label(&self) -> Option<&str> {
        Some(self)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bar: BarConfig::default(),
            workspace: WorkspaceConfig::default(),
        }
    }
}

impl Config {
    pub fn load(&mut self) -> InstanceConfig {
        self.load_dir(&default_config_path())
    }

    pub fn load_file(&mut self, path: PathBuf) -> InstanceConfig {
        let Some(toml) = load_toml(&path) else {
            fail!("{} not found", path.display());
        };

        self.parse(toml)
    }

    fn load_dir(&mut self, path: &Path) -> InstanceConfig {
        let mut toml = load_toml(&path.join("config.toml")).unwrap_or_default();

        for p in config_paths(&path.join("conf.d")) {
            let Some(c) = load_toml(&p) else {
                error!("{} not found", p.display());
                continue;
            };

            merge_toml(&mut toml, c);
        }

        self.parse(toml)
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
        self.size.visit(&mut toml);
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
        self.down.format.clone_from(&self.format);
        self.idle.format.clone_from(&self.format);
        self.muted.format.clone_from(&self.format);

        toml.get("properties").set(&mut self.properties);
        toml.get("down").visit(&mut self.down);
        toml.get("idle").visit(&mut self.idle);
        toml.get("muted").visit(&mut self.muted);
        toml.empty();
    }
}

impl Visit for BatteryConfig {
    fn visit(&mut self, mut toml: Toml) {
        self.block.visit(&mut toml);

        toml.get("format").set(&mut self.format);
        self.down.format.clone_from(&self.format);
        self.charging.format.clone_from(&self.format);
        self.full.format.clone_from(&self.format);
        self.idle.format.clone_from(&self.format);
        self.unknown.format.clone_from(&self.format);
        self.low.state.format.clone_from(&self.format);

        toml.get("path").set(&mut self.path);
        toml.get("poll").set(&mut self.poll);
        toml.get("design_capacity").set(&mut self.design_capacity);
        toml.get("down").visit(&mut self.down);
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

        toml.get("format").set(&mut self.format);
        self.down.format.clone_from(&self.format);
        self.low.state.format.clone_from(&self.format);

        toml.get("interface").set(&mut self.interface);
        toml.get("down").visit(&mut self.down);
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
        toml.get("weekdays").set(&mut self.weekdays);
        toml.get("format").set(&mut self.format);
        toml.empty();
    }
}

impl Visit for CpuConfig {
    fn visit(&mut self, mut toml: Toml) {
        self.block.visit(&mut toml);

        toml.get("format").set(&mut self.format);
        self.high.state.format.clone_from(&self.format);

        toml.get("high").visit(&mut self.high);
        toml.empty();
    }
}

impl Visit for MemoryConfig {
    fn visit(&mut self, mut toml: Toml) {
        self.block.visit(&mut toml);

        toml.get("format").set(&mut self.format);
        self.high.state.format.clone_from(&self.format);

        toml.get("high").visit(&mut self.high);
        toml.empty();
    }
}

impl Visit for DriveConfig {
    fn visit(&mut self, mut toml: Toml) {
        self.block.visit(&mut toml);
        toml.get("path").set(&mut self.path);

        toml.get("format").set(&mut self.format);
        self.high.state.format.clone_from(&self.format);

        toml.get("high").visit(&mut self.high);
        toml.empty();
    }
}

impl Visit for FileConfig {
    fn visit(&mut self, mut toml: Toml) {
        self.block.visit(&mut toml);

        toml.get("path").set(&mut self.path);
        assert!(
            !self.path.as_os_str().is_empty(),
            "file.path must be non-empty"
        );
        toml.get("format").set(&mut self.format);
        self.down.format.clone_from(&self.format);
        toml.get("down").visit(&mut self.down);
        toml.empty();
    }
}

impl Config {
    pub fn parse(&mut self, table: toml::Table) -> InstanceConfig {
        let mut toml = Toml::new(table);
        let mut instances = InstanceConfig::default();
        toml.get("bar").set(&mut self.bar);
        toml.get("workspace").visit(&mut self.workspace);
        toml.merge(
            "cpu",
            &mut instances.cpu,
            CpuConfig::default(&self.bar.color),
        );
        toml.merge(
            "memory",
            &mut instances.memory,
            MemoryConfig::default(&self.bar.color),
        );
        toml.merge(
            "drive",
            &mut instances.drive,
            DriveConfig::default(&self.bar.color),
        );
        toml.merge(
            "wireless",
            &mut instances.wireless,
            WirelessConfig::default(&self.bar.color),
        );
        toml.merge(
            "volume",
            &mut instances.volume,
            VolumeConfig::default(&self.bar.color),
        );
        toml.merge(
            "battery",
            &mut instances.battery,
            BatteryConfig::default(&self.bar.color),
        );
        toml.merge(
            "time",
            &mut instances.time,
            TimeConfig::default(&self.bar.color),
        );
        toml.merge(
            "file",
            &mut instances.file,
            FileConfig::default(&self.bar.color),
        );
        toml.empty();
        instances
    }
}

fn duration_from_secs<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    u64::deserialize(deserializer).map(Duration::from_secs)
}

#[cfg(test)]
pub(crate) mod tests {
    use crate::tests::tmp::Directory;

    use super::*;

    pub(crate) fn parse_config(input: &str) -> Config {
        let mut config = Config::default();
        parse_instances(&mut config, input);
        config
    }

    pub(crate) fn parse_instances(config: &mut Config, input: &str) -> InstanceConfig {
        config.parse(toml::from_str(input).unwrap())
    }

    #[test]
    fn block_defaults() {
        let mut config = Config::default();
        let instances = parse_instances(&mut config, "");

        let b = config.bar;
        assert_eq!(b.width, 28);
        assert_eq!(b.interval, Duration::from_secs(10));
        assert_eq!(b.separator, 14);
        assert_eq!(b.blocks, ["cpu.0", "volume.0", "time.0"]);
        assert_eq!(b.color.text, Color::rgb(0x64, 0x64, 0x64));
        assert_eq!(b.color.background, Color::rgb(0, 0, 0));
        assert_eq!(b.color.border, Color::rgb(0, 0, 0));

        let w = config.workspace;
        assert_eq!(w.size.borders, [1, 1, 1, 1]);
        assert_eq!(w.size.margins, [0, 2, 2, 0]);
        assert_eq!(w.size.corners, [0, 0, 0, 0]);
        assert_eq!(w.size.height, 26);
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
        assert_eq!(instances.cpu.len(), 0);
        assert_eq!(instances.memory.len(), 0);
        assert_eq!(instances.drive.len(), 0);
        assert_eq!(instances.wireless.len(), 0);
        assert_eq!(instances.volume.len(), 0);
        assert_eq!(instances.battery.len(), 0);
        assert_eq!(instances.time.len(), 0);
    }

    #[test]
    fn panics_on_unknown_root_field() {
        let result = std::panic::catch_unwind(|| parse_config("unexpected = true"));

        assert!(result.is_err());
    }

    #[test]
    fn panics_on_unknown_section_fields() {
        let sections = [
            "[bar]",
            "[workspace]",
            "[cpu.0]",
            "[memory.0]",
            "[drive.0]",
            "[wireless.0]",
            "[volume.0]",
            "[battery.0]",
            "[time.0]",
            "[bar.color]",
            "[workspace.active]",
            "[cpu.0.high]",
            "[memory.0.high]",
        ];

        for section in sections {
            let input = format!("{section}\nunexpected = true");
            let result = std::panic::catch_unwind(|| parse_config(&input));

            assert!(result.is_err(), "{} accepted an unknown field", section);
        }
    }

    #[test]
    fn panics_on_invalid_nested_value() {
        let result = std::panic::catch_unwind(|| {
            parse_config(
                r###"
                [cpu.0.color]
                text = 1
                "###,
            )
        });

        assert!(result.is_err());
    }

    #[test]
    fn bar_partial_override() {
        let config = parse_config(
            r###"
            [bar]
            fonts = "Monospace 10"
            interval = 5

            [bar.color]
            background = "#aabbcc"
            "###,
        );

        let b = config.bar;
        assert_eq!(b.fonts, "Monospace 10");
        assert_eq!(b.interval, Duration::from_secs(5));
        assert_eq!(b.color.text, Color::rgb(0x64, 0x64, 0x64));
        assert_eq!(b.color.background, Color::rgb(0xaa, 0xbb, 0xcc));
        assert_eq!(b.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn workspace_partial_override() {
        let config = parse_config(
            r###"
            [workspace]
            margins = [10, 20, 30, 40]
            height = 50

            [workspace.inactive.color]
            background = "#112233"
            "###,
        );

        let w = config.workspace;
        assert_eq!(w.size.borders, [1, 1, 1, 1]);
        assert_eq!(w.size.margins, [10, 20, 30, 40]);
        assert_eq!(w.size.height, 50);
        assert_eq!(w.inactive.color.text, Color::rgb(0x88, 0x88, 0x88));
        assert_eq!(w.inactive.color.background, Color::rgb(0x11, 0x22, 0x33));
        assert_eq!(w.inactive.color.border, Color::rgb(0x33, 0x33, 0x33));
    }

    #[test]
    fn cpu_partial_override() {
        let mut config = Config::default();
        let instances = parse_instances(
            &mut config,
            r###"
            [cpu.0]
            margins = [1, 2, 3, 4]

            [cpu.0.high]
            threshold = 90
            color.text = "#123456"

            [cpu.0.color]
            background = "#aabbcc"
            "###,
        );

        let c = instances.cpu.get("0").unwrap();
        assert_eq!(c.block.size.margins, [1, 2, 3, 4]);
        assert_eq!(c.block.size.borders, [0, 0, 0, 0]);
        assert_eq!(c.block.color.text, Color::rgb(0x64, 0x64, 0x64));
        assert_eq!(c.block.color.background, Color::rgb(0xaa, 0xbb, 0xcc));
        assert_eq!(c.block.color.border, Color::rgb(0, 0, 0));
        assert_eq!(c.high.threshold, 90);
        assert_eq!(c.high.state.color.text, Color::rgb(0x12, 0x34, 0x56));
        assert_eq!(c.high.state.color.background, Color::rgb(0, 0, 0));
        assert_eq!(c.high.state.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn memory_defaults() {
        let mut config = Config::default();
        let instances = parse_instances(&mut config, "[memory.0]");

        let m = instances.memory.get("0").unwrap();
        assert_eq!(m.block.size.height, 0);
        assert_eq!(m.block.size.borders, [0, 0, 0, 0]);
        assert_eq!(m.block.size.margins, [0, 0, 0, 0]);
        assert_eq!(m.block.color, config.bar.color);
        assert_eq!(
            m.format,
            [
                MemoryFormatItem::Label("MEM".into()),
                MemoryFormatItem::Usage
            ]
        );
        assert_eq!(m.high.threshold, 80);
        assert_eq!(m.high.state.format, m.format);
        assert_eq!(m.high.state.color.text, Color::rgb(0xdc, 0xa3, 0xa3));
        assert_eq!(m.high.state.color.background, m.block.color.background);
        assert_eq!(m.high.state.color.border, m.block.color.border);
    }

    #[test]
    fn memory_partial_override() {
        let mut config = Config::default();
        let instances = parse_instances(
            &mut config,
            r###"
            [memory.0]
            margins = [1, 2, 3, 4]

            [memory.0.high]
            threshold = 90
            color.text = "#123456"

            [memory.0.color]
            background = "#aabbcc"
            "###,
        );

        let c = instances.memory.get("0").unwrap();
        assert_eq!(c.block.size.margins, [1, 2, 3, 4]);
        assert_eq!(c.block.size.borders, [0, 0, 0, 0]);
        assert_eq!(c.block.color.text, Color::rgb(0x64, 0x64, 0x64));
        assert_eq!(c.block.color.background, Color::rgb(0xaa, 0xbb, 0xcc));
        assert_eq!(c.block.color.border, Color::rgb(0, 0, 0));
        assert_eq!(c.high.threshold, 90);
        assert_eq!(c.high.state.color.text, Color::rgb(0x12, 0x34, 0x56));
        assert_eq!(c.high.state.color.background, Color::rgb(0, 0, 0));
        assert_eq!(c.high.state.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn drive_defaults() {
        let mut config = Config::default();
        let instances = parse_instances(&mut config, "[drive.0]");

        let c = instances.drive.get("0").unwrap();
        assert_eq!(c.path, PathBuf::from("/"));
        assert_eq!(c.block.size.height, 0);
        assert_eq!(c.block.size.borders, [0, 0, 0, 0]);
        assert_eq!(c.block.size.margins, [0, 0, 0, 0]);
        assert_eq!(c.block.color, config.bar.color);
        assert_eq!(
            c.format,
            [DriveFormatItem::Label("DRV".into()), DriveFormatItem::Usage]
        );
        assert_eq!(c.high.threshold, 80);
        assert_eq!(c.high.state.format, c.format);
        assert_eq!(c.high.state.color.text, Color::rgb(0xdc, 0xa3, 0xa3));
        assert_eq!(c.high.state.color.background, c.block.color.background);
        assert_eq!(c.high.state.color.border, c.block.color.border);
    }

    #[test]
    fn drive_partial_override() {
        let mut config = Config::default();
        let instances = parse_instances(
            &mut config,
            r###"
            [drive.0]
            path = "/home"
            margins = [1, 2, 3, 4]

            [drive.0.high]
            threshold = 90
            color.text = "#123456"

            [drive.0.color]
            background = "#aabbcc"
            "###,
        );

        let c = instances.drive.get("0").unwrap();
        assert_eq!(c.path, PathBuf::from("/home"));
        assert_eq!(c.block.size.margins, [1, 2, 3, 4]);
        assert_eq!(c.block.size.borders, [0, 0, 0, 0]);
        assert_eq!(c.block.color.text, Color::rgb(0x64, 0x64, 0x64));
        assert_eq!(c.block.color.background, Color::rgb(0xaa, 0xbb, 0xcc));
        assert_eq!(c.block.color.border, Color::rgb(0, 0, 0));
        assert_eq!(c.high.threshold, 90);
        assert_eq!(c.high.state.color.text, Color::rgb(0x12, 0x34, 0x56));
        assert_eq!(c.high.state.color.background, Color::rgb(0, 0, 0));
        assert_eq!(c.high.state.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn volume_partial_override() {
        let mut config = Config::default();
        let instances = parse_instances(
            &mut config,
            r###"
            [volume.0]
            margins = [1, 2, 3, 4]

            [volume.0.muted.color]
            background = "#aabbcc"
            "###,
        );

        let v = instances.volume.get("0").unwrap();
        assert_eq!(v.block.size.margins, [1, 2, 3, 4]);
        assert_eq!(v.block.size.borders, [0, 0, 0, 0]);
        assert_eq!(v.muted.color.text, Color::rgb(0xdf, 0xaf, 0x8f));
        assert_eq!(v.muted.color.background, Color::rgb(0xaa, 0xbb, 0xcc));
        assert_eq!(v.muted.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn battery_defaults() {
        let mut config = Config::default();
        let instances = parse_instances(
            &mut config,
            r###"
            [battery.0]
            "###,
        );

        let b = instances.battery.get("0").unwrap();
        assert_eq!(b.path, PathBuf::from("/sys/class/power_supply/BAT0/uevent"));
        assert_eq!(b.block.size.height, 0);
        assert_eq!(b.block.size.borders, [0, 0, 0, 0]);
        assert_eq!(b.block.size.margins, [0, 0, 0, 0]);
        assert_eq!(b.block.color.text, Color::rgb(0x64, 0x64, 0x64));
        assert_eq!(b.block.color.background, Color::rgb(0, 0, 0));
        assert_eq!(b.block.color.border, Color::rgb(0, 0, 0));

        assert_eq!(
            b.format,
            vec![
                BatteryFormatItem::Label("BAT".into()),
                BatteryFormatItem::Capacity,
            ]
        );
        assert_eq!(b.down.format, b.format);
        assert_eq!(b.charging.format, b.format);
        assert_eq!(b.full.format, b.format);
        assert_eq!(b.idle.format, b.format);
        assert_eq!(b.unknown.format, b.format);
        assert_eq!(b.low.state.format, b.format);

        assert_eq!(b.down.color.text, Color::rgb(0xdc, 0xa3, 0xa3));
        assert_eq!(b.down.color.background, Color::rgb(0, 0, 0));
        assert_eq!(b.down.color.border, Color::rgb(0, 0, 0));
        assert_eq!(b.low.threshold, 20);
        assert_eq!(b.low.state.color.text, Color::rgb(0xdc, 0xa3, 0xa3));
        assert_eq!(b.low.state.color.background, Color::rgb(0, 0, 0));
        assert_eq!(b.low.state.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn battery_partial_override() {
        let mut config = Config::default();
        let instances = parse_instances(
            &mut config,
            r###"
            [battery.0]
            path = "/sys/class/power_supply/BAT1/uevent"
            design_capacity = true
            margins = [1, 2, 3, 4]

            [battery.0.low]
            threshold = 15
            color.text = "#123456"

            [battery.0.down]
            format = ["OFF"]
            color.text = "#abcdef"

            [battery.0.color]
            background = "#aabbcc"
            "###,
        );

        let b = instances.battery.get("0").unwrap();
        assert_eq!(b.path, PathBuf::from("/sys/class/power_supply/BAT1/uevent"));
        assert!(b.design_capacity);
        assert_eq!(b.block.size.height, 0);
        assert_eq!(b.block.size.borders, [0, 0, 0, 0]);
        assert_eq!(b.block.size.margins, [1, 2, 3, 4]);
        assert_eq!(b.block.color.text, Color::rgb(0x64, 0x64, 0x64));
        assert_eq!(b.block.color.background, Color::rgb(0xaa, 0xbb, 0xcc));
        assert_eq!(b.block.color.border, Color::rgb(0, 0, 0));
        assert_eq!(b.down.format, vec![BatteryFormatItem::Label("OFF".into())]);
        assert_eq!(b.down.color.text, Color::rgb(0xab, 0xcd, 0xef));
        assert_eq!(b.down.color.background, Color::rgb(0, 0, 0));
        assert_eq!(b.down.color.border, Color::rgb(0, 0, 0));
        assert_eq!(b.low.threshold, 15);
        assert_eq!(b.low.state.color.text, Color::rgb(0x12, 0x34, 0x56));
        assert_eq!(b.low.state.color.background, Color::rgb(0, 0, 0));
        assert_eq!(b.low.state.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn wireless_defaults() {
        let mut config = Config::default();
        let instances = parse_instances(
            &mut config,
            r###"
            [wireless.0]
            "###,
        );

        let w = instances.wireless.get("0").unwrap();
        assert_eq!(w.interface, "wlan0");
        assert_eq!(w.block.size.height, 0);
        assert_eq!(w.block.size.borders, [0, 0, 0, 0]);
        assert_eq!(w.block.size.margins, [0, 0, 0, 0]);
        assert_eq!(w.block.color.text, Color::rgb(0x64, 0x64, 0x64));
        assert_eq!(w.block.color.background, Color::rgb(0, 0, 0));
        assert_eq!(w.block.color.border, Color::rgb(0, 0, 0));

        assert_eq!(
            w.format,
            vec![
                WirelessFormatItem::Label("NET".into()),
                WirelessFormatItem::Quality,
            ]
        );
        assert_eq!(w.down.format, w.format);
        assert_eq!(w.low.state.format, w.format);

        assert_eq!(w.down.color.text, Color::rgb(0xdc, 0xa3, 0xa3));
        assert_eq!(w.down.color.background, Color::rgb(0, 0, 0));
        assert_eq!(w.down.color.border, Color::rgb(0, 0, 0));
        assert_eq!(w.low.threshold, 50);
        assert_eq!(w.low.state.color.text, Color::rgb(0xdc, 0xa3, 0xa3));
        assert_eq!(w.low.state.color.background, Color::rgb(0, 0, 0));
        assert_eq!(w.low.state.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn wireless_partial_override() {
        let mut config = Config::default();
        let instances = parse_instances(
            &mut config,
            r###"
            [wireless.0]
            interface = "wlp3s0"
            margins = [1, 2, 3, 4]

            [wireless.0.low]
            threshold = 40
            color.text = "#123456"

            [wireless.0.down]
            format = ["OFF"]
            color.text = "#abcdef"

            [wireless.0.color]
            background = "#aabbcc"
            "###,
        );

        let w = instances.wireless.get("0").unwrap();
        assert_eq!(w.interface, "wlp3s0");
        assert_eq!(w.block.size.height, 0);
        assert_eq!(w.block.size.borders, [0, 0, 0, 0]);
        assert_eq!(w.block.size.margins, [1, 2, 3, 4]);
        assert_eq!(w.block.color.text, Color::rgb(0x64, 0x64, 0x64));
        assert_eq!(w.block.color.background, Color::rgb(0xaa, 0xbb, 0xcc));
        assert_eq!(w.block.color.border, Color::rgb(0, 0, 0));
        assert_eq!(w.down.format, vec![WirelessFormatItem::Label("OFF".into())]);
        assert_eq!(w.down.color.text, Color::rgb(0xab, 0xcd, 0xef));
        assert_eq!(w.down.color.background, Color::rgb(0, 0, 0));
        assert_eq!(w.down.color.border, Color::rgb(0, 0, 0));
        assert_eq!(w.low.threshold, 40);
        assert_eq!(w.low.state.color.text, Color::rgb(0x12, 0x34, 0x56));
        assert_eq!(w.low.state.color.background, Color::rgb(0, 0, 0));
        assert_eq!(w.low.state.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn time_partial_override() {
        let mut config = Config::default();
        let instances = parse_instances(
            &mut config,
            r###"
            [time.0]
            margins = [1, 2, 3, 4]
            timezone = "America/Los_Angeles"
            weekdays = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"]

            [time.0.color]
            background = "#aabbcc"
            "###,
        );

        let t = instances.time.get("0").unwrap();
        assert_eq!(
            t.timezone,
            tz::TimeZone::from_posix_tz("America/Los_Angeles").unwrap()
        );
        assert_eq!(t.block.size.margins, [1, 2, 3, 4]);
        assert_eq!(t.weekdays, ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"]);
        assert_eq!(t.block.size.borders, [0, 0, 0, 0]);
        assert_eq!(t.block.color.text, Color::rgb(0x64, 0x64, 0x64));
        assert_eq!(t.block.color.background, Color::rgb(0xaa, 0xbb, 0xcc));
        assert_eq!(t.block.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn time_defaults() {
        let mut config = Config::default();
        let instances = parse_instances(
            &mut config,
            r###"
            [time.0]
            "###,
        );

        let t = instances.time.get("0").unwrap();
        assert_eq!(t.timezone, tz::TimeZone::local().unwrap());
        assert_eq!(
            t.weekdays,
            ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"]
        );
        assert_eq!(t.format, vec![TimeFormatItem::Hour, TimeFormatItem::Minute]);
    }

    #[test]
    fn time_weekdays_rejects_wrong_length() {
        for weekdays in [
            "['Su', 'Mo', 'Tu', 'We', 'Th', 'Fr']",
            "['Su', 'Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa', 'Su']",
        ] {
            let input = format!("[time.0]\nweekdays = {weekdays}");
            let result = std::panic::catch_unwind(|| parse_config(&input));
            assert!(result.is_err(), "accepted invalid weekdays: {weekdays}");
        }
    }

    #[test]
    fn time_format_parses_tokens_and_labels() {
        let mut config = Config::default();
        let instances = parse_instances(
            &mut config,
            r###"
            [time.0]
            format = ["[hour]", "[minute]", "[day]", "[month]", "[weekday]", "hello"]
            "###,
        );

        let t = instances.time.get("0").unwrap();
        assert_eq!(
            t.format,
            vec![
                TimeFormatItem::Hour,
                TimeFormatItem::Minute,
                TimeFormatItem::Day,
                TimeFormatItem::Month,
                TimeFormatItem::Weekday,
                TimeFormatItem::Label("hello".into()),
            ]
        );
    }

    #[test]
    fn volume_format_default() {
        let mut config = Config::default();
        let instances = parse_instances(
            &mut config,
            r###"
            [volume.0]
            "###,
        );

        let v = instances.volume.get("0").unwrap();
        assert_eq!(
            v.format,
            vec![
                VolumeFormatItem::Label("VOL".into()),
                VolumeFormatItem::Volume,
            ]
        );
        assert_eq!(v.muted.format, v.format);
        assert_eq!(v.down.format, v.format);
        assert_eq!(v.idle.format, v.format);
        assert_eq!(v.idle.color, v.block.color);
    }

    #[test]
    fn volume_state_format() {
        let mut config = Config::default();
        let instances = parse_instances(
            &mut config,
            r###"
            [volume.0]
            properties = { "device.api" = "bluez5" }
            format = ["[volume]", "hello0"]

            [volume.1]
            properties = { "device.api" = "alsa", "device.bus" = "pci" }
            format = ["[volume]", "hello1"]

            [volume.2]
            format = ["[volume]", "hello2"]

            [volume.0.muted.color]
            text = "#123456"

            [volume.0.down.color]
            text = "#abcdef"

            [volume.0.idle.color]
            text = "#123456"

            [volume.1.idle]
            format = []

            [volume.2.idle]
            format = ["OFF"]

            [volume.1.down]
            format = []

            [volume.2.down]
            format = ["OFF"]

            [volume.1.muted]
            format = []

            [volume.2.muted]
            format = ["[volume]", "muted2"]
            "###,
        );

        // Inherited
        let v0 = instances.volume.get("0").unwrap();
        assert_eq!(
            v0.properties,
            HashMap::from([("device.api".into(), "bluez5".into())])
        );
        assert_eq!(v0.muted.format, v0.format);
        assert_eq!(v0.down.format, v0.format);
        assert_eq!(v0.idle.format, v0.format);
        assert_eq!(v0.idle.color.text, Color::rgb(0x12, 0x34, 0x56));

        // Empty override
        let v1 = instances.volume.get("1").unwrap();
        assert_eq!(
            v1.properties,
            HashMap::from([
                ("device.api".into(), "alsa".into()),
                ("device.bus".into(), "pci".into()),
            ])
        );
        assert_eq!(v1.muted.format, vec![]);
        assert_eq!(v1.down.format, vec![]);
        assert_eq!(v1.idle.format, vec![]);

        // Override
        let v2 = instances.volume.get("2").unwrap();
        assert!(v2.properties.is_empty());
        assert_eq!(v2.down.format, vec![VolumeFormatItem::Label("OFF".into())]);
        assert_eq!(v2.idle.format, vec![VolumeFormatItem::Label("OFF".into())]);
        assert_eq!(
            v2.muted.format,
            vec![
                VolumeFormatItem::Volume,
                VolumeFormatItem::Label("muted2".into()),
            ]
        );
    }

    #[test]
    fn battery_state_format() {
        let mut config = Config::default();
        let instances = parse_instances(
            &mut config,
            r###"
            [battery.0]
            format = ["[capacity]", "hello0"]

            [battery.1]
            format = ["[capacity]", "hello1"]

            [battery.2]
            format = ["[capacity]", "hello2"]

            [battery.0.low]
            threshold = 30

            [battery.1.down]
            format = []

            [battery.1.charging]
            format = []

            [battery.2.down]
            format = ["down2", "[capacity]"]

            [battery.2.charging]
            format = ["charging2", "[capacity]"]

            [battery.2.full]
            format = ["full2", "[capacity]"]

            [battery.2.idle]
            format = ["idle2", "[capacity]"]

            [battery.2.unknown]
            format = ["unknown2", "[capacity]"]

            [battery.2.low]
            format = ["low2", "[capacity]"]
            "###,
        );

        // Inherited
        let b0 = instances.battery.get("0").unwrap();
        assert_eq!(b0.down.format, b0.format);
        assert_eq!(b0.charging.format, b0.format);
        assert_eq!(b0.full.format, b0.format);
        assert_eq!(b0.idle.format, b0.format);
        assert_eq!(b0.unknown.format, b0.format);
        assert_eq!(b0.low.state.format, b0.format);

        // Empty override
        let b1 = instances.battery.get("1").unwrap();
        assert_eq!(b1.down.format, vec![]);
        assert_eq!(b1.charging.format, vec![]);

        // Overrides
        let b2 = instances.battery.get("2").unwrap();
        let format = |label: &str| {
            vec![
                BatteryFormatItem::Label(label.into()),
                BatteryFormatItem::Capacity,
            ]
        };
        assert_eq!(b2.down.format, format("down2"));
        assert_eq!(b2.charging.format, format("charging2"));
        assert_eq!(b2.full.format, format("full2"));
        assert_eq!(b2.idle.format, format("idle2"));
        assert_eq!(b2.unknown.format, format("unknown2"));
        assert_eq!(b2.low.state.format, format("low2"));
    }

    #[test]
    fn wireless_state_format() {
        let mut config = Config::default();
        let instances = parse_instances(
            &mut config,
            r###"
            [wireless.0]
            format = ["[quality]", "hello0"]

            [wireless.1]
            format = ["[quality]", "hello1"]

            [wireless.2]
            format = ["[quality]", "hello2"]

            [wireless.0.low]
            threshold = 40

            [wireless.1.down]
            format = []

            [wireless.1.low]
            format = []

            [wireless.2.down]
            format = ["down2"]

            [wireless.2.low]
            format = ["low2", "[quality]"]
            "###,
        );

        // Inherited
        let w0 = instances.wireless.get("0").unwrap();
        assert_eq!(w0.down.format, w0.format);
        assert_eq!(w0.low.state.format, w0.format);

        // Empty override
        let w1 = instances.wireless.get("1").unwrap();
        assert_eq!(w1.down.format, vec![]);
        assert_eq!(w1.low.state.format, vec![]);

        // Override
        let w2 = instances.wireless.get("2").unwrap();
        assert_eq!(
            w2.down.format,
            vec![WirelessFormatItem::Label("down2".into())]
        );
        assert_eq!(
            w2.low.state.format,
            vec![
                WirelessFormatItem::Label("low2".into()),
                WirelessFormatItem::Quality,
            ]
        );
    }

    #[test]
    fn cpu_state_format() {
        let mut config = Config::default();
        let instances = parse_instances(
            &mut config,
            r###"
            [cpu.0]
            format = ["[usage]", "hello0"]

            [cpu.1]
            format = ["[usage]", "hello1"]

            [cpu.2]
            format = ["[usage]", "hello2"]

            [cpu.0.high]
            threshold = 90

            [cpu.1.high]
            format = []

            [cpu.2.high]
            format = ["high2", "[usage]"]
            "###,
        );

        // Inherited
        let c0 = instances.cpu.get("0").unwrap();
        assert_eq!(c0.high.state.format, c0.format);

        // Empty override
        let c1 = instances.cpu.get("1").unwrap();
        assert_eq!(c1.high.state.format, vec![]);

        // Override
        let c2 = instances.cpu.get("2").unwrap();
        assert_eq!(
            c2.high.state.format,
            vec![CpuFormatItem::Label("high2".into()), CpuFormatItem::Usage,]
        );
    }

    #[test]
    fn memory_state_format() {
        let mut config = Config::default();
        let instances = parse_instances(
            &mut config,
            r###"
            [memory.0]
            format = ["[usage]", "hello0"]

            [memory.1]
            format = ["[usage]", "hello1"]

            [memory.2]
            format = ["[usage]", "hello2"]

            [memory.0.high]
            threshold = 90

            [memory.1.high]
            format = []

            [memory.2.high]
            format = ["high2", "[usage]"]
            "###,
        );

        // Inherited
        let c0 = instances.memory.get("0").unwrap();
        assert_eq!(c0.high.state.format, c0.format);

        // Empty override
        let c1 = instances.memory.get("1").unwrap();
        assert_eq!(c1.high.state.format, vec![]);

        // Override
        let c2 = instances.memory.get("2").unwrap();
        assert_eq!(
            c2.high.state.format,
            vec![
                MemoryFormatItem::Label("high2".into()),
                MemoryFormatItem::Usage,
            ]
        );
    }

    #[test]
    fn drive_state_format() {
        let mut config = Config::default();
        let instances = parse_instances(
            &mut config,
            r###"
            [drive.0]
            format = ["[usage]", "hello0"]

            [drive.1]
            format = ["[usage]", "hello1"]

            [drive.2]
            format = ["[usage]", "hello2"]

            [drive.0.high]
            threshold = 90

            [drive.1.high]
            format = []

            [drive.2.high]
            format = ["high2", "[usage]"]
            "###,
        );

        // Inherited
        let c0 = instances.drive.get("0").unwrap();
        assert_eq!(c0.high.state.format, c0.format);

        // Empty override
        let c1 = instances.drive.get("1").unwrap();
        assert_eq!(c1.high.state.format, vec![]);

        // Override
        let c2 = instances.drive.get("2").unwrap();
        assert_eq!(
            c2.high.state.format,
            vec![
                DriveFormatItem::Label("high2".into()),
                DriveFormatItem::Usage,
            ]
        );
    }

    #[test]
    fn file_defaults() {
        let mut config = Config::default();
        let instances = parse_instances(
            &mut config,
            r###"
            [file.0]
            path = "/tmp/flag"
            "###,
        );

        let f = instances.file.get("0").unwrap();
        assert_eq!(f.path, PathBuf::from("/tmp/flag"));
        assert_eq!(f.block.size.height, 0);
        assert_eq!(f.block.size.borders, [0, 0, 0, 0]);
        assert_eq!(f.block.size.margins, [0, 0, 0, 0]);
        assert_eq!(f.block.color.text, Color::rgb(0x64, 0x64, 0x64));
        assert_eq!(f.block.color.background, Color::rgb(0, 0, 0));
        assert_eq!(f.block.color.border, Color::rgb(0, 0, 0));
        assert_eq!(f.format, ["PTH"]);
        assert_eq!(f.down.format, f.format);
        assert_eq!(f.down.color.text, Color::rgb(0xdc, 0xa3, 0xa3));
        assert_eq!(f.down.color.background, Color::rgb(0, 0, 0));
        assert_eq!(f.down.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    fn file_partial_override() {
        let mut config = Config::default();
        let instances = parse_instances(
            &mut config,
            r###"
            [file.0]
            path = "relative/path"
            margins = [1, 2, 3, 4]
            format = ["[literal]", "ON"]

            [file.0.down]
            format = ["OFF"]
            color.text = "#abcdef"

            [file.0.color]
            background = "#aabbcc"
            "###,
        );

        let f = instances.file.get("0").unwrap();
        assert_eq!(f.path, PathBuf::from("relative/path"));
        assert_eq!(f.block.size.height, 0);
        assert_eq!(f.block.size.borders, [0, 0, 0, 0]);
        assert_eq!(f.block.size.margins, [1, 2, 3, 4]);
        assert_eq!(f.block.color.text, Color::rgb(0x64, 0x64, 0x64));
        assert_eq!(f.block.color.background, Color::rgb(0xaa, 0xbb, 0xcc));
        assert_eq!(f.block.color.border, Color::rgb(0, 0, 0));
        assert_eq!(f.format, ["[literal]", "ON"]);
        assert_eq!(f.down.format, ["OFF"]);
        assert_eq!(f.down.color.text, Color::rgb(0xab, 0xcd, 0xef));
        assert_eq!(f.down.color.background, Color::rgb(0, 0, 0));
        assert_eq!(f.down.color.border, Color::rgb(0, 0, 0));
    }

    #[test]
    #[should_panic(expected = "file.path must be non-empty")]
    fn file_missing_path_rejected() {
        parse_config("[file.0]");
    }

    #[test]
    #[should_panic(expected = "file.path must be non-empty")]
    fn file_empty_path_rejected() {
        parse_config("[file.0]\npath = ''");
    }

    #[test]
    fn format_rejects_non_string_items() {
        let result = std::panic::catch_unwind(|| {
            parse_config(
                r###"
                [cpu.0]
                format = ["[usage]", 1]
                "###,
            )
        });

        assert!(result.is_err());
    }

    #[test]
    fn block_height_override() {
        let mut config = Config::default();
        let instances = parse_instances(
            &mut config,
            r###"
            [time.0]
            height = 64

            [battery.0]
            margins = [1, 2, 3, 4]
            "###,
        );

        assert_eq!(instances.time.get("0").unwrap().block.size.height, 64);
        assert_eq!(instances.battery.get("0").unwrap().block.size.height, 0);
    }

    #[test]
    fn bar_color_propagation() {
        let mut config = Config::default();
        let instances = parse_instances(
            &mut config,
            r###"
            [bar.color]
            text = "#001122"
            background = "#334455"
            border = "#667788"

            [volume.0]
            [battery.0]
            [time.0]
            "###,
        );

        let bar_color = ColorConfig {
            text: Color::rgb(0x00, 0x11, 0x22),
            background: Color::rgb(0x33, 0x44, 0x55),
            border: Color::rgb(0x66, 0x77, 0x88),
        };
        assert_eq!(instances.volume.get("0").unwrap().block.color, bar_color);
        assert_eq!(instances.volume.get("0").unwrap().idle.color, bar_color);
        assert_eq!(instances.battery.get("0").unwrap().block.color, bar_color);
        assert_eq!(instances.time.get("0").unwrap().block.color, bar_color);
        assert_eq!(
            instances.volume.get("0").unwrap().muted.color,
            ColorConfig {
                text: Color::rgb(0xdf, 0xaf, 0x8f),
                ..bar_color
            }
        );
    }

    #[test]
    fn bar_color_propagation_partial_override() {
        let mut config = Config::default();
        let instances = parse_instances(
            &mut config,
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
        );

        let bar_color = ColorConfig {
            text: Color::rgb(0x00, 0x11, 0x22),
            background: Color::rgb(0x33, 0x44, 0x55),
            border: Color::rgb(0x66, 0x77, 0x88),
        };
        assert_eq!(
            instances.volume.get("0").unwrap().block.color,
            ColorConfig {
                text: Color::rgb(0x11, 0x11, 0x11),
                ..bar_color
            }
        );
        assert_eq!(
            instances.time.get("0").unwrap().block.color,
            ColorConfig {
                background: Color::rgb(0x22, 0x22, 0x22),
                ..bar_color
            }
        );
        assert_eq!(
            instances.battery.get("0").unwrap().block.color,
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

        let mut config = Config::default();
        config.load_file(path);

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

        let mut config = Config::default();
        let instances = config.load_dir(tmp.path());

        assert_eq!(config.bar.width, 40);
        assert_eq!(config.bar.blocks, ["volume.0"]);
        assert_eq!(config.bar.color.text, Color::rgb(0x11, 0x11, 0x11));
        assert_eq!(config.bar.color.background, Color::rgb(0x44, 0x44, 0x44));
        assert_eq!(config.bar.color.border, Color::rgb(0x33, 0x33, 0x33));
        assert_eq!(
            instances.cpu.get("0").unwrap().format,
            [CpuFormatItem::Label("CPU".into()), CpuFormatItem::Usage]
        );
        assert_eq!(
            instances.volume.get("0").unwrap().block.size.margins,
            [1, 2, 3, 4]
        );
        assert_eq!(
            instances.volume.get("0").unwrap().block.color.background,
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

        let mut config = Config::default();
        config.load_dir(tmp.path());

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

        let mut config = Config::default();
        config.load_dir(tmp.path());

        assert_eq!(config.bar.width, 30);
    }
}
