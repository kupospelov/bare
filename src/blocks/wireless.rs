use super::{Block, BlockDirty, Instance, Line};
use crate::blocks::FormatItem;
use crate::config::{BlockConfig, ColorConfig, WirelessConfig, WirelessFormatItem};
use crate::raster::Rasterizer;
use crate::state::State;
use crate::{debug, error, fail};
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

    pub fn add(&mut self, id: usize, config: &WirelessConfig) -> Instance {
        let n = self.instances.len();
        self.instances.push(Wireless::new(id, config));
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
                                let mut dirty = Vec::new();
                                state.blocks.wireless.update(&mut dirty);
                                for update in dirty {
                                    state.mark_all_outputs_block_dirty(update);
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
    }

    pub fn update(&mut self, dirty: &mut Vec<BlockDirty>) {
        let Some(socket) = &mut self.socket else {
            return;
        };

        for instance in &mut self.instances {
            let signal = match socket.get_station_info(instance.interface) {
                Ok(stations) => stations.first().and_then(|s| s.signal),
                Err(e) => {
                    error!("Error reading station signal: {}", e);
                    continue;
                }
            };
            if let Some(update) = instance.update(signal.map(dbm_to_quality)) {
                dirty.push(update);
            }
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

fn dbm_to_quality(dbm: i8) -> u8 {
    const SIGNAL_MIN_DBM: f32 = -90.0;
    const SIGNAL_MAX_DBM: f32 = -20.0;
    let percent =
        100.0 - 70.0 * ((SIGNAL_MAX_DBM - dbm as f32) / (SIGNAL_MAX_DBM - SIGNAL_MIN_DBM));
    percent.round().clamp(0.0, 100.0) as u8
}

pub struct Wireless {
    id: usize,
    config: WirelessConfig,
    interface: i32,
    quality: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum WirelessState {
    Down,
    Low,
    Normal,
}

impl Wireless {
    pub fn new(id: usize, config: &WirelessConfig) -> Self {
        let interface = match if_nametoindex(config.interface.as_str()) {
            Ok(index) => index as i32,
            Err(e) => fail!(
                "Failed to get interface index for {}: {}",
                config.interface,
                e
            ),
        };
        Self {
            id,
            config: config.clone(),
            interface,
            quality: None,
        }
    }

    fn update(&mut self, quality: Option<u8>) -> Option<BlockDirty> {
        if quality == self.quality {
            return None;
        }

        debug!("Updated wireless quality: {:?}", quality);
        let state = self.state();
        self.quality = quality;
        Some(BlockDirty {
            index: self.id,
            layout: state != self.state(),
        })
    }

    fn state(&self) -> WirelessState {
        match self.quality {
            None => WirelessState::Down,
            Some(q) if q <= self.config.low.threshold => WirelessState::Low,
            Some(_) => WirelessState::Normal,
        }
    }

    fn format(&self) -> &[WirelessFormatItem] {
        match self.state() {
            WirelessState::Down => &self.config.down.format,
            WirelessState::Low => &self.config.low.state.format,
            WirelessState::Normal => &self.config.format,
        }
    }
}

impl Block for Wireless {
    fn block(&self) -> &BlockConfig {
        &self.config.block
    }

    fn colors(&self) -> &ColorConfig {
        match self.state() {
            WirelessState::Down => &self.config.down.color,
            WirelessState::Low => &self.config.low.state.color,
            WirelessState::Normal => &self.config.color,
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
                WirelessFormatItem::Quality => match self.quality {
                    Some(q) => q.to_string(),
                    None => "...".into(),
                },
                WirelessFormatItem::Label(s) => s.clone(),
            },
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

    #[test]
    fn state_changes() {
        let mut config = WirelessConfig::default(&ColorConfig::default());
        config.format = vec![WirelessFormatItem::Label("normal".into())];
        config.down.format = vec![WirelessFormatItem::Label("down".into())];
        config.low.state.format = vec![
            WirelessFormatItem::Label("low".into()),
            WirelessFormatItem::Quality,
        ];
        let mut wireless = Wireless {
            id: 3,
            config: config.clone(),
            interface: 0,
            quality: None,
        };

        // Down
        assert_eq!(wireless.format(), config.down.format);
        assert_eq!(wireless.colors(), &config.down.color);

        // Initialize
        let dirty = wireless.update(Some(40)).unwrap();
        assert_eq!(wireless.format(), config.low.state.format);
        assert_eq!(wireless.colors(), &config.low.state.color);
        assert_eq!(
            dirty,
            BlockDirty {
                index: 3,
                layout: true
            }
        );

        // Normal
        let dirty = wireless.update(Some(80)).unwrap();
        assert_eq!(wireless.format(), config.format);
        assert_eq!(wireless.colors(), &config.color);
        assert_eq!(
            dirty,
            BlockDirty {
                index: 3,
                layout: true
            }
        );

        // Quality changes
        let dirty = wireless.update(Some(90)).unwrap();
        assert_eq!(wireless.format(), config.format);
        assert_eq!(
            dirty,
            BlockDirty {
                index: 3,
                layout: false
            }
        );

        // Down
        let dirty = wireless.update(None).unwrap();
        assert_eq!(wireless.format(), config.down.format);
        assert_eq!(wireless.colors(), &config.down.color);
        assert_eq!(
            dirty,
            BlockDirty {
                index: 3,
                layout: true
            }
        );

        // Low
        let dirty = wireless.update(Some(30)).unwrap();
        assert_eq!(wireless.format(), config.low.state.format);
        assert_eq!(
            dirty,
            BlockDirty {
                index: 3,
                layout: true
            }
        );
    }
}
