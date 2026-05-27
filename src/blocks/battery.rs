use super::{Block, Instance};
use crate::config::{BatteryConfig, BatteryFormatItem, ColorConfig};
use crate::render;
use crate::state::State;
use crate::{debug, error};
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

    pub fn add(&mut self, config: &BatteryConfig) -> Instance {
        let n = self.instances.len();
        self.instances.push(Battery::new(config));
        Instance::Battery(n)
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
                                let event = parse_event(buf[..n].split(|&b| b == 0));
                                for i in 0..state.blocks.order.len() {
                                    if let Instance::Battery(j) = state.blocks.order[i]
                                        && state.blocks.battery.instances[j].update(&event)
                                    {
                                        state.mark_all_outputs_block_dirty(i);
                                    }
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

pub struct Battery {
    pub capacity: String,
    name: String,
    config: BatteryConfig,
}

impl Battery {
    pub fn new(config: &BatteryConfig) -> Self {
        let mut battery = Self {
            capacity: String::new(),
            name: String::new(),
            config: config.clone(),
        };
        match std::fs::read(&battery.config.path) {
            Ok(bytes) => {
                let event = parse_event(bytes.split(|&b| b == b'\n'));
                battery.name = event.name.expect("No POWER_SUPPLY_NAME in the uevent file");
                battery.set_capacity(
                    event
                        .capacity
                        .expect("No POWER_SUPPLY_CAPACITY in the uevent file"),
                );
            }
            Err(e) => error!("Failed to read uevent file: {}", e),
        }
        battery
    }

    fn set_capacity(&mut self, c: String) -> bool {
        if c == self.capacity {
            return false;
        }
        debug!("Updated battery capacity: {}", c);
        self.capacity = c;
        true
    }

    fn item_text(&self, item: &BatteryFormatItem) -> String {
        match item {
            BatteryFormatItem::Capacity => {
                if self.capacity.is_empty() {
                    "??".into()
                } else {
                    self.capacity.clone()
                }
            }
            BatteryFormatItem::Label(s) => s.clone(),
        }
    }

    fn item_height(item: &BatteryFormatItem, font_size: u32) -> u32 {
        match item {
            BatteryFormatItem::Capacity => font_size,
            BatteryFormatItem::Label(s) => font_size * 2 / s.len().max(1) as u32,
        }
    }

    fn update(&mut self, event: &Event) -> bool {
        let Some(name) = &event.name else {
            return false;
        };
        if &self.name != name {
            return false;
        }
        let Some(c) = &event.capacity else {
            debug!("Battery {}: no reported capacity", name);
            return false;
        };
        self.set_capacity(c.clone())
    }
}

struct Event {
    name: Option<String>,
    capacity: Option<String>,
}

fn parse_event<'a>(fields: impl Iterator<Item = &'a [u8]>) -> Event {
    let mut name = None;
    let mut capacity = None;
    for f in fields {
        if let Some(v) = f.strip_prefix(b"POWER_SUPPLY_NAME=") {
            name = std::str::from_utf8(v).ok().map(str::to_owned);
        } else if let Some(v) = f.strip_prefix(b"POWER_SUPPLY_CAPACITY=") {
            capacity = std::str::from_utf8(v).ok().map(str::to_owned);
        }
    }
    Event { name, capacity }
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
    fn layout(&self, font_size: u32, scale: i32) -> render::BlockLayout {
        let items = &self.config.format;
        let separator = super::inner_margin(font_size);
        let gaps = items.len().saturating_sub(1) as i32;
        let height: i32 = items
            .iter()
            .map(|i| Self::item_height(i, font_size) as i32)
            .sum::<i32>()
            + gaps * separator;
        let block = self.config.block.scaled(scale);
        render::BlockLayout {
            content: height,
            height: block.height.unwrap_or(height) + block.margins[0] + block.margins[2],
            config: block,
        }
    }

    fn colors(&self) -> &ColorConfig {
        &self.config.color
    }

    fn render(
        &mut self,
        renderer: &mut crate::render::Renderer,
        map: &mut render::Map<'_>,
        region: render::Region,
        font_size: u32,
    ) {
        let color = &self.config.color;
        let margin = super::inner_margin(font_size);
        let mut y = region.y;
        for item in &self.config.format {
            let h = Self::item_height(item, font_size);
            let text = self.item_text(item);
            renderer.render_text(
                map,
                render::Region {
                    x: region.x,
                    y,
                    w: region.w,
                    h,
                },
                &text,
                color.text,
                color.background,
                h,
            );
            y += h as i32 + margin;
        }
    }
}
