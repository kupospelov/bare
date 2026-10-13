use super::{Block, BlockDirty, Instance, Line};
use crate::blocks::FormatItem;
use crate::config::{BatteryConfig, BatteryFormatItem, BlockConfig, ColorConfig};
use crate::raster::Rasterizer;
use crate::state::State;
use crate::{debug, error, fail, warning};
use nix::sys::socket::{
    self, AddressFamily, MsgFlags, NetlinkAddr, SockFlag, SockProtocol, SockType,
};
use std::io::ErrorKind;
use std::os::fd::{AsRawFd, OwnedFd};

pub struct Group {
    pub instances: Vec<Battery>,
}

impl Group {
    pub fn new() -> Self {
        Self {
            instances: Vec::new(),
        }
    }

    pub fn add(&mut self, id: usize, config: &BatteryConfig) -> Instance {
        let n = self.instances.len();
        self.instances.push(Battery::new(id, config));
        Instance::Battery(n)
    }

    pub fn update(&mut self, dirty: &mut Vec<BlockDirty>) {
        for instance in &mut self.instances {
            if instance.config.poll {
                let event = instance.read_event_from_path();
                if let Some(update) = instance.update_state(&event) {
                    dirty.push(update);
                }
            }
        }
    }

    pub fn register_events(&self, handle: &calloop::LoopHandle<'_, State>) {
        if self.instances.is_empty() {
            return;
        }

        let socket = open_uevent_socket().expect("Failed to open uevent socket");
        handle
            .insert_source(
                calloop::generic::Generic::new(
                    socket,
                    calloop::Interest::READ,
                    calloop::Mode::Level,
                ),
                |_, socket, state| {
                    let mut buf = [0u8; 8192];
                    loop {
                        match socket::recv(socket.as_raw_fd(), &mut buf, MsgFlags::empty()) {
                            Ok(n) => {
                                for i in 0..state.blocks.battery.instances.len() {
                                    let update = {
                                        let instance = &mut state.blocks.battery.instances[i];
                                        let Some(update) = instance.update(&buf[..n]) else {
                                            continue;
                                        };
                                        update
                                    };

                                    state.mark_all_outputs_block_dirty(update);
                                }
                            }
                            Err(nix::errno::Errno::EAGAIN) => break,
                            Err(e) => {
                                error!("Failed to read uevent: {}", e);
                                break;
                            }
                        }
                    }
                    Ok(calloop::PostAction::Continue)
                },
            )
            .expect("Failed to insert battery group fd");
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
enum BatteryState {
    // Event states.
    Unknown,
    Discharging,
    Charging,
    Full,
    Idle,

    // Calculated states.
    #[default]
    Down,
    Low,
}

impl BatteryState {
    fn from_status(status: &str) -> Self {
        match status {
            "Discharging" => Self::Discharging,
            "Charging" => Self::Charging,
            "Full" => Self::Full,
            "Not charging" => Self::Idle,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum BatteryCapacity {
    Actual,
    Charge(u64),
    Energy(u64),
}

impl BatteryCapacity {
    fn new(config: &BatteryConfig) -> Self {
        if !config.design_capacity {
            return Self::Actual;
        }

        match std::fs::read(&config.path) {
            Ok(bytes) => Self::parse(bytes.split(|&b| b == b'\n')),
            Err(_) => Self::Actual,
        }
    }

    fn key(self) -> &'static [u8] {
        match self {
            Self::Actual => b"POWER_SUPPLY_CAPACITY=",
            Self::Charge(_) => b"POWER_SUPPLY_CHARGE_NOW=",
            Self::Energy(_) => b"POWER_SUPPLY_ENERGY_NOW=",
        }
    }

    fn percentage(self, value: &str) -> Option<u8> {
        match self {
            Self::Actual => value
                .parse()
                .inspect_err(|_| error!("Cannot parse battery capacity: {}, {:?}", value, self))
                .ok(),
            Self::Charge(design) | Self::Energy(design) => {
                let now = value
                    .parse::<u64>()
                    .inspect_err(|_| error!("Cannot parse battery capacity: {}, {:?}", value, self))
                    .ok()?;
                Some(
                    (100.0 * now as f64 / design as f64)
                        .round()
                        .clamp(0.0, 100.0) as u8,
                )
            }
        }
    }

    fn parse<'a>(fields: impl Iterator<Item = &'a [u8]>) -> Self {
        for f in fields {
            let Ok(field) = std::str::from_utf8(f) else {
                continue;
            };
            let mut parts = field.splitn(2, '=');
            let capacity: fn(u64) -> Self = match parts.next() {
                Some("POWER_SUPPLY_ENERGY_FULL_DESIGN") => Self::Energy,
                Some("POWER_SUPPLY_CHARGE_FULL_DESIGN") => Self::Charge,
                _ => continue,
            };
            let Some(value) = parts.next().and_then(|value| value.parse::<u64>().ok()) else {
                warning!("Cannot read design capacity: {}", field);
                continue;
            };
            if value == 0 {
                warning!("Invalid design capacity: {}", field);
                continue;
            }
            return capacity(value);
        }
        Self::Actual
    }
}

pub struct Battery {
    id: usize,
    name: String,
    state: BatteryState,
    full: BatteryCapacity,
    capacity: u8,
    config: BatteryConfig,
}

impl Battery {
    pub fn new(id: usize, config: &BatteryConfig) -> Self {
        let Some(name) = config
            .path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
        else {
            fail!(
                "Failed to get battery name from path: {}",
                config.path.display()
            );
        };

        let mut battery = Self {
            id,
            name: name.to_owned(),
            state: BatteryState::default(),
            full: BatteryCapacity::new(config),
            capacity: 0,
            config: config.clone(),
        };
        let event = battery.read_event_from_path();
        battery.update_state(&event);
        battery
    }

    fn set_capacity(&mut self, value: u8) -> bool {
        if value == self.capacity {
            return false;
        }

        debug!("Battery {}: updated battery capacity: {}", self.name, value);
        self.capacity = value;
        true
    }

    // Make sure to update capacity first.
    fn set_state(&mut self, s: BatteryState) -> bool {
        let state = if s == BatteryState::Discharging && self.capacity <= self.config.low.threshold
        {
            BatteryState::Low
        } else {
            s
        };

        if state == self.state {
            return false;
        }

        debug!("Battery {}: updated battery state: {:?}", self.name, state);
        self.state = state;
        true
    }

    fn read_event_from_path(&self) -> Event {
        match std::fs::read(&self.config.path) {
            Ok(bytes) => {
                let fields = bytes.split(|&b| b == b'\n');
                parse_event(fields, false, self.full)
            }
            Err(e) => {
                if e.kind() != ErrorKind::NotFound {
                    // Expected for the down state.
                    error!("No event read from {}: {}", self.config.path.display(), e);
                }
                Event {
                    name: None,
                    status: None,
                    capacity: None,
                    gone: true,
                }
            }
        }
    }

    fn update(&mut self, bytes: &[u8]) -> Option<BlockDirty> {
        let event = parse_event(bytes.split(|&b| b == 0), true, self.full);
        let Some(name) = &event.name else {
            return None;
        };
        if &self.name != name {
            return None;
        }

        self.update_state(&event)
    }

    fn update_state(&mut self, event: &Event) -> Option<BlockDirty> {
        let state = self.state;
        let mut dirty = false;
        if let Some(c) = event.capacity {
            dirty |= self.set_capacity(c);
        }
        if event.gone {
            dirty |= self.set_state(BatteryState::Down)
        } else {
            if let Some(status) = &event.status {
                dirty |= self.set_state(BatteryState::from_status(status));
            }
        }
        dirty.then_some(BlockDirty {
            index: self.id,
            layout: self.state != state,
        })
    }

    fn format(&self) -> &[BatteryFormatItem] {
        match self.state {
            BatteryState::Down => &self.config.down.format,
            BatteryState::Discharging => &self.config.format,
            BatteryState::Charging => &self.config.charging.format,
            BatteryState::Full => &self.config.full.format,
            BatteryState::Idle => &self.config.idle.format,
            BatteryState::Unknown => &self.config.unknown.format,
            BatteryState::Low => &self.config.low.state.format,
        }
    }
}

struct Event {
    name: Option<String>,
    gone: bool,
    status: Option<String>,
    capacity: Option<u8>,
}

fn parse_event<'a>(
    fields: impl Iterator<Item = &'a [u8]>,
    read_name: bool,
    full: BatteryCapacity,
) -> Event {
    let mut name = None;
    let mut status = None;
    let mut capacity = None;
    let mut gone = false;
    let capacity_key = full.key();
    for f in fields {
        if read_name && let Some(v) = f.strip_prefix(b"POWER_SUPPLY_NAME=") {
            name = std::str::from_utf8(v).ok().map(str::to_owned);
        } else if let Some(v) = f.strip_prefix(b"POWER_SUPPLY_STATUS=") {
            status = std::str::from_utf8(v).ok().map(str::to_owned);
        } else if let Some(v) = f.strip_prefix(capacity_key) {
            capacity = std::str::from_utf8(v).ok();
        } else if f == b"ACTION=remove" || f == b"POWER_SUPPLY_PRESENT=0" {
            gone = true;
        }
    }
    Event {
        name,
        status,
        capacity: capacity.and_then(|value| full.percentage(value)),
        gone,
    }
}

fn open_uevent_socket() -> nix::Result<OwnedFd> {
    let fd = socket::socket(
        AddressFamily::Netlink,
        SockType::Datagram,
        SockFlag::SOCK_NONBLOCK | SockFlag::SOCK_CLOEXEC,
        SockProtocol::NetlinkKObjectUEvent,
    )?;
    socket::bind(fd.as_raw_fd(), &NetlinkAddr::new(0, 1))?;
    Ok(fd)
}

impl Block for Battery {
    fn block(&self) -> &BlockConfig {
        &self.config.block
    }

    fn colors(&self) -> &ColorConfig {
        match self.state {
            BatteryState::Down => &self.config.down.color,
            BatteryState::Discharging => &self.config.color,
            BatteryState::Charging => &self.config.charging.color,
            BatteryState::Full => &self.config.full.color,
            BatteryState::Idle => &self.config.idle.color,
            BatteryState::Unknown => &self.config.unknown.color,
            BatteryState::Low => &self.config.low.state.color,
        }
    }

    fn len(&self) -> usize {
        self.format().len()
    }

    fn get(&self, index: usize, rasterizer: &Rasterizer, scale: i32) -> Line {
        let item = &self.format()[index];
        Line {
            height: item.height(rasterizer, scale),
            text: match item {
                BatteryFormatItem::Capacity => format!("{:02}", self.capacity),
                BatteryFormatItem::Label(s) => s.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(capacity: u8, status: &str) -> Event {
        Event {
            name: None,
            gone: false,
            status: Some(status.into()),
            capacity: Some(capacity),
        }
    }

    #[test]
    fn capacity_parse() {
        let cases: &[(&[u8], BatteryCapacity)] = &[
            (b"", BatteryCapacity::Actual),
            (b"POWER_SUPPLY_CAPACITY=55", BatteryCapacity::Actual),
            (
                b"POWER_SUPPLY_CHARGE_FULL_DESIGN=120",
                BatteryCapacity::Charge(120),
            ),
            (
                b"POWER_SUPPLY_ENERGY_FULL_DESIGN=80",
                BatteryCapacity::Energy(80),
            ),
            (
                b"POWER_SUPPLY_CHARGE_FULL_DESIGN=0",
                BatteryCapacity::Actual,
            ),
            (
                b"POWER_SUPPLY_ENERGY_FULL_DESIGN=invalid",
                BatteryCapacity::Actual,
            ),
            (b"POWER_SUPPLY_CHARGE_FULL_DESIGN", BatteryCapacity::Actual),
            (
                b"POWER_SUPPLY_CAPACITY=55\n\
                  POWER_SUPPLY_CHARGE_FULL_DESIGN=\xff\n\
                  POWER_SUPPLY_CHARGE_FULL_DESIGN=invalid\n\
                  POWER_SUPPLY_CHARGE_FULL_DESIGN=0\n\
                  POWER_SUPPLY_ENERGY_FULL_DESIGN=80",
                BatteryCapacity::Energy(80),
            ),
        ];
        for &(input, expected) in cases {
            assert_eq!(
                BatteryCapacity::parse(input.split(|&b| b == b'\n')),
                expected,
                "input: {input:?}",
            );
        }
    }

    #[test]
    fn capacity_percentage() {
        let input = b"POWER_SUPPLY_CAPACITY=55\n\
                      POWER_SUPPLY_CHARGE_NOW=30\n\
                      POWER_SUPPLY_ENERGY_NOW=40";
        for (full, expected) in [
            (BatteryCapacity::Actual, 55),
            (BatteryCapacity::Charge(120), 25),
            (BatteryCapacity::Energy(80), 50),
        ] {
            let event = parse_event(input.split(|&b| b == b'\n'), false, full);
            assert_eq!(event.capacity, Some(expected));
        }
    }

    #[test]
    fn state_changes() {
        let mut config = BatteryConfig::default(&ColorConfig::default());
        config.format = vec![BatteryFormatItem::Label("discharging".into())];
        config.down.format = vec![BatteryFormatItem::Label("down".into())];
        config.charging.format = vec![BatteryFormatItem::Label("charging".into())];
        config.full.format = vec![BatteryFormatItem::Label("full".into())];
        config.idle.format = vec![BatteryFormatItem::Label("idle".into())];
        config.unknown.format = vec![BatteryFormatItem::Label("unknown".into())];
        config.low.state.format = vec![BatteryFormatItem::Label("low".into())];

        // Initialize
        let mut battery = Battery {
            id: 3,
            name: "BAT0".into(),
            state: BatteryState::Down,
            capacity: 0,
            full: BatteryCapacity::Actual,
            config,
        };

        // Down
        assert_eq!(battery.format(), [BatteryFormatItem::Label("down".into())]);
        assert_eq!(battery.colors(), &battery.config.down.color);

        // Discharging
        let dirty = battery.update_state(&event(50, "Discharging")).unwrap();
        assert_eq!(
            battery.format(),
            [BatteryFormatItem::Label("discharging".into())]
        );
        assert_eq!(battery.colors(), &battery.config.color);
        assert!(dirty.layout);

        // Capacity changes
        let dirty = battery.update_state(&event(40, "Discharging")).unwrap();
        assert!(!dirty.layout);

        // Charging
        let dirty = battery.update_state(&event(40, "Charging")).unwrap();
        assert_eq!(
            battery.format(),
            [BatteryFormatItem::Label("charging".into())]
        );
        assert_eq!(battery.colors(), &battery.config.charging.color);
        assert!(dirty.layout);

        // Low
        let dirty = battery.update_state(&event(10, "Discharging")).unwrap();
        assert_eq!(battery.format(), [BatteryFormatItem::Label("low".into())]);
        assert_eq!(battery.colors(), &battery.config.low.state.color);
        assert!(dirty.layout);

        // Full
        let dirty = battery.update_state(&event(100, "Full")).unwrap();
        assert_eq!(battery.format(), [BatteryFormatItem::Label("full".into())]);
        assert_eq!(battery.colors(), &battery.config.full.color);
        assert!(dirty.layout);

        // Idle
        let dirty = battery.update_state(&event(100, "Not charging")).unwrap();
        assert_eq!(battery.format(), [BatteryFormatItem::Label("idle".into())]);
        assert_eq!(battery.colors(), &battery.config.idle.color);
        assert!(dirty.layout);

        // Unknown
        let dirty = battery.update_state(&event(100, "invalid")).unwrap();
        assert_eq!(
            battery.format(),
            [BatteryFormatItem::Label("unknown".into())]
        );
        assert_eq!(battery.colors(), &battery.config.unknown.color);
        assert!(dirty.layout);

        // Down
        let dirty = battery
            .update_state(&Event {
                name: None,
                gone: true,
                status: None,
                capacity: None,
            })
            .unwrap();
        assert_eq!(battery.format(), [BatteryFormatItem::Label("down".into())]);
        assert_eq!(battery.colors(), &battery.config.down.color);
        assert!(dirty.layout);
    }
}
