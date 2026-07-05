use super::{Block, Instance, Line};
use crate::blocks::FormatItem;
use crate::config::{BatteryConfig, BatteryFormatItem, BlockConfig, ColorConfig};
use crate::raster::Rasterizer;
use crate::state::State;
use crate::{debug, error, fail};
use nix::sys::socket::{
    self, AddressFamily, MsgFlags, NetlinkAddr, SockFlag, SockProtocol, SockType,
};
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

    pub fn update(&mut self, dirty: &mut Vec<usize>) {
        for instance in &mut self.instances {
            if instance.config.poll
                && let Some(event) = instance.read_event_from_path(false)
                && instance.update_state(&event)
            {
                dirty.push(instance.id);
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
                                let event = parse_event(buf[..n].split(|&b| b == 0), true);
                                for i in 0..state.blocks.battery.instances.len() {
                                    let id = {
                                        let instance = &mut state.blocks.battery.instances[i];
                                        if !instance.update(&event) {
                                            continue;
                                        }
                                        instance.id
                                    };

                                    state.mark_all_outputs_block_dirty(id);
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
    #[default]
    Unknown,
    Discharging,
    Charging,
    Full,
    Idle,

    // Calculated states.
    Low,
}

impl BatteryState {
    fn from_status(status: &str) -> Self {
        match status {
            "Discharging" => Self::Discharging,
            "Charging" => Self::Charging,
            "Full" => Self::Full,
            "Not charging" => Self::Idle,
            s => {
                debug!("Unknown battery status: {}", s);
                Self::Unknown
            }
        }
    }
}

pub struct Battery {
    id: usize,
    name: String,
    state: BatteryState,
    capacity: u8,
    config: BatteryConfig,
}

impl Battery {
    pub fn new(id: usize, config: &BatteryConfig) -> Self {
        let mut battery = Self {
            id,
            name: String::new(),
            state: BatteryState::default(),
            capacity: 0,
            config: config.clone(),
        };
        if let Some(event) = battery.read_event_from_path(true) {
            let Some(name) = event.name.as_ref() else {
                fail!("Failed to read battery name");
            };

            battery.name = name.clone();
            battery.update_state(&event);
        }
        battery
    }

    fn set_capacity(&mut self, c: String) -> bool {
        let Ok(value) = c.parse() else {
            error!(
                "Battery {}: cannot parse battery capacity: {}",
                self.name, c
            );
            return false;
        };

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

    fn read_event_from_path(&mut self, read_name: bool) -> Option<Event> {
        match std::fs::read(&self.config.path) {
            Ok(bytes) => Some(parse_event(bytes.split(|&b| b == b'\n'), read_name)),
            Err(e) => {
                error!("No event read from {}: {}", self.config.path.display(), e);
                None
            }
        }
    }

    fn update(&mut self, event: &Event) -> bool {
        let Some(name) = &event.name else {
            return false;
        };
        if &self.name != name {
            return false;
        }

        self.update_state(event)
    }

    fn update_state(&mut self, event: &Event) -> bool {
        let mut dirty = false;
        if let Some(c) = &event.capacity {
            dirty |= self.set_capacity(c.clone());
        } else {
            debug!("Battery {}: no reported capacity", self.name);
        }
        if let Some(status) = &event.status {
            dirty |= self.set_state(BatteryState::from_status(status));
        } else {
            debug!("Battery {}: no reported status", self.name);
        }
        dirty
    }
}

struct Event {
    name: Option<String>,
    status: Option<String>,
    capacity: Option<String>,
}

fn parse_event<'a>(fields: impl Iterator<Item = &'a [u8]>, read_name: bool) -> Event {
    let mut name = None;
    let mut status = None;
    let mut capacity = None;
    for f in fields {
        if read_name && let Some(v) = f.strip_prefix(b"POWER_SUPPLY_NAME=") {
            name = std::str::from_utf8(v).ok().map(str::to_owned);
        } else if let Some(v) = f.strip_prefix(b"POWER_SUPPLY_STATUS=") {
            status = std::str::from_utf8(v).ok().map(str::to_owned);
        } else if let Some(v) = f.strip_prefix(b"POWER_SUPPLY_CAPACITY=") {
            capacity = std::str::from_utf8(v).ok().map(str::to_owned);
        }
    }
    Event {
        name,
        status,
        capacity,
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
            BatteryState::Discharging => &self.config.color,
            BatteryState::Charging => &self.config.charging.color,
            BatteryState::Full => &self.config.full.color,
            BatteryState::Idle => &self.config.idle.color,
            BatteryState::Unknown => &self.config.unknown.color,
            BatteryState::Low => &self.config.low.state.color,
        }
    }

    fn len(&self) -> usize {
        self.config.format.len()
    }

    fn get(&self, index: usize, rasterizer: &Rasterizer, scale: i32) -> Line {
        let item = &self.config.format[index];
        Line {
            height: item.height(rasterizer, scale),
            text: match item {
                BatteryFormatItem::Capacity => format!("{:02}", self.capacity),
                BatteryFormatItem::Label(s) => s.clone(),
            },
        }
    }
}
