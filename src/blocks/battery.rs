use super::{Block, Fd};
use crate::config::{BatteryConfig, BatteryFormatItem, ColorConfig};
use crate::render;
use crate::{debug, error};
use nix::sys::socket::{
    self, AddressFamily, MsgFlags, NetlinkAddr, SockFlag, SockProtocol, SockType,
};
use std::os::fd::{AsRawFd, OwnedFd};

const BATTERY_UEVENT_PATH: &str = "/sys/class/power_supply/BAT0/uevent";

pub struct Battery {
    pub capacity: String,
    socket: OwnedFd,
    config: BatteryConfig,
}

impl Battery {
    pub fn new(config: &BatteryConfig) -> Self {
        let socket = open_uevent_socket().expect("Failed to open uevent socket");
        let mut battery = Self {
            capacity: String::new(),
            socket,
            config: config.clone(),
        };
        match std::fs::read(BATTERY_UEVENT_PATH) {
            Ok(bytes) => {
                if let Some(c) = parse_capacity(bytes.split(|&b| b == b'\n')) {
                    battery.set_capacity(c);
                }
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

    fn drain(&mut self) -> bool {
        let mut buf = [0u8; 8192];
        let mut redraw = false;
        loop {
            match socket::recv(self.socket.as_raw_fd(), &mut buf, MsgFlags::empty()) {
                Ok(n) => {
                    if let Some(c) = parse_capacity(buf[..n].split(|&b| b == 0)) {
                        redraw |= self.set_capacity(c);
                    }
                }
                Err(nix::errno::Errno::EAGAIN) => break,
                Err(e) => {
                    error!("Failed to read uevent: {}", e);
                    break;
                }
            }
        }
        redraw
    }
}

fn parse_capacity<'a>(fields: impl Iterator<Item = &'a [u8]>) -> Option<String> {
    for f in fields {
        if let Some(v) = f.strip_prefix(b"POWER_SUPPLY_CAPACITY=") {
            return std::str::from_utf8(v).ok().map(str::to_owned);
        }
    }

    None
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
    fn layout(&self, font_size: u32) -> render::BlockLayout {
        let margin = super::inner_margin(font_size);
        let items = &self.config.format;
        let gaps = items.len().saturating_sub(1) as i32;
        let height: i32 = items
            .iter()
            .map(|i| Self::item_height(i, font_size) as i32)
            .sum::<i32>()
            + gaps * margin;
        render::BlockLayout {
            height,
            config: self.config.block.clone(),
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

    fn fd(&self) -> Option<calloop::generic::Generic<Fd>> {
        Some(calloop::generic::Generic::new(
            Fd(self.socket.as_raw_fd()),
            calloop::Interest::READ,
            calloop::Mode::Level,
        ))
    }

    fn on_fd(&mut self) -> bool {
        self.drain()
    }
}
