use std::time::Duration;

use super::{Block, Instance};
use crate::config::{ColorConfig, WirelessConfig, WirelessFormatItem};
use crate::render;
use crate::state::State;
use crate::{debug, error};
use calloop::timer::{TimeoutAction, Timer};
use neli_wifi::Socket;
use nix::net::if_::if_nametoindex;
use nix::sys::socket::{
    self, AddressFamily, MsgFlags, NetlinkAddr, SockFlag, SockProtocol, SockType,
};
use std::os::fd::AsRawFd;

pub struct Group {
    pub instances: Vec<Wireless>,
    socket: Option<Socket>,
}

impl Group {
    pub fn new() -> Self {
        Self {
            instances: Vec::new(),
            socket: None,
        }
    }

    pub fn add(&mut self, config: &WirelessConfig, id: usize) -> Instance {
        let n = self.instances.len();
        self.instances.push(Wireless::new(config, id));
        Instance::Wireless(n)
    }

    pub fn register_events(&mut self, handle: &calloop::LoopHandle<'_, State>) {
        if self.instances.is_empty() {
            return;
        }

        // The socket to get Wi-Fi station info.
        self.socket = match Socket::connect() {
            Ok(s) => Some(s),
            Err(e) => {
                error!("Failed to open nl80211 socket: {}", e);
                return;
            }
        };

        // The socket to get notified about link/unlink events.
        let socket = open_netlink_socket().expect("Failed to open netlink socket");
        handle
            .insert_source(
                calloop::generic::Generic::new(
                    socket,
                    calloop::Interest::READ,
                    calloop::Mode::Level,
                ),
                move |_, socket, state| {
                    let mut buf = [0u8; 8192];
                    loop {
                        match nix::sys::socket::recv(
                            socket.as_raw_fd(),
                            &mut buf,
                            MsgFlags::empty(),
                        ) {
                            Ok(e) => {
                                debug!("Read a netlink event {}", e);

                                for i in 0..state.blocks.wireless.instances.len() {
                                    update_instance(state, i);
                                }
                            }
                            Err(nix::errno::Errno::EAGAIN) => break,
                            Err(e) => {
                                error!("Failed to read netlink: {}", e);
                                break;
                            }
                        }
                    }
                    Ok(calloop::PostAction::Continue)
                },
            )
            .expect("Failed to insert netlink source");

        for j in 0..self.instances.len() {
            let interval = Duration::from_secs(self.instances[j].config.interval);
            handle
                .insert_source(Timer::immediate(), move |_, _, state| {
                    update_instance(state, j);

                    TimeoutAction::ToDuration(interval)
                })
                .expect("Failed to insert wireless instance timer");
        }
    }
}

fn open_netlink_socket() -> nix::Result<std::os::fd::OwnedFd> {
    let fd = socket::socket(
        AddressFamily::Netlink,
        SockType::Datagram,
        SockFlag::SOCK_NONBLOCK | SockFlag::SOCK_CLOEXEC,
        SockProtocol::NetlinkRoute,
    )?;
    socket::bind(
        fd.as_raw_fd(),
        &NetlinkAddr::new(0, nix::libc::RTMGRP_LINK as u32),
    )?;
    Ok(fd)
}

fn update_instance(state: &mut State, j: usize) {
    let socket = state.blocks.wireless.socket.as_mut().unwrap();
    let id = {
        let instance = &mut state.blocks.wireless.instances[j];
        let signal = match socket.get_station_info(instance.interface) {
            Ok(stations) => stations.first().and_then(|s| s.signal),
            Err(e) => {
                debug!("Error reading station signal: {}", e);
                return;
            }
        };
        if !instance.update(signal) {
            return;
        }
        instance.id
    };

    state.mark_all_outputs_block_dirty(id);
}

fn dbm_to_quality(dbm: i8) -> u8 {
    const SIGNAL_MIN_DBM: f32 = -90.0;
    const SIGNAL_MAX_DBM: f32 = -20.0;
    let percent =
        100.0 - 70.0 * ((SIGNAL_MAX_DBM - dbm as f32) / (SIGNAL_MAX_DBM - SIGNAL_MIN_DBM));
    percent.round().clamp(0.0, 100.0) as u8
}

pub struct Wireless {
    config: WirelessConfig,
    id: usize,
    interface: i32,
    signal: Option<i8>,
}

impl Wireless {
    pub fn new(config: &WirelessConfig, id: usize) -> Self {
        let interface =
            if_nametoindex(config.interface.as_str()).expect("Failed to resolve interface") as i32;
        Self {
            config: config.clone(),
            id,
            interface,
            signal: None,
        }
    }

    fn update(&mut self, signal: Option<i8>) -> bool {
        if signal == self.signal {
            return false;
        }

        debug!("Updated wireless signal: {:?} dBm", signal);
        self.signal = signal;
        true
    }

    fn item_text(&self, item: &WirelessFormatItem) -> String {
        match item {
            WirelessFormatItem::Quality => match self.signal {
                Some(s) => dbm_to_quality(s).to_string(),
                None => "??".into(),
            },
            WirelessFormatItem::Label(s) => s.clone(),
        }
    }

    fn item_height(item: &WirelessFormatItem, font_size: u32) -> u32 {
        match item {
            WirelessFormatItem::Quality => font_size,
            WirelessFormatItem::Label(s) => font_size * 2 / s.len().max(1) as u32,
        }
    }
}

impl Block for Wireless {
    fn layout(&self, font_size: u32, scale: i32) -> render::BlockLayout {
        let items = &self.config.format;
        let separator = super::inner_margin(font_size);
        let gaps = items.len().saturating_sub(1) as i32;
        let content: i32 = items
            .iter()
            .map(|i| Self::item_height(i, font_size) as i32)
            .sum::<i32>()
            + gaps * separator;
        let block = self.config.block.scaled(scale);
        render::BlockLayout {
            content,
            height: block.height(content),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dbm_to_quality_clamps_and_scales() {
        assert_eq!(dbm_to_quality(-10), 100);
        assert_eq!(dbm_to_quality(-20), 100);
        assert_eq!(dbm_to_quality(-54), 66);
        assert_eq!(dbm_to_quality(-90), 30);
        assert_eq!(dbm_to_quality(-120), 0);
    }
}
