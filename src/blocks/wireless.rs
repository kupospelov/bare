use super::{Block, Instance, Line};
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
                                for id in dirty {
                                    state.mark_all_outputs_block_dirty(id);
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

    pub fn update(&mut self, dirty: &mut Vec<usize>) {
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
            if !instance.update(signal.map(dbm_to_quality)) {
                continue;
            }

            dirty.push(instance.id);
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

    fn update(&mut self, quality: Option<u8>) -> bool {
        if quality == self.quality {
            return false;
        }

        debug!("Updated wireless quality: {:?}", quality);
        self.quality = quality;
        true
    }
}

impl Block for Wireless {
    fn block(&self) -> &BlockConfig {
        &self.config.block
    }

    fn colors(&self) -> &ColorConfig {
        match self.quality {
            Some(q) if q > self.config.low.threshold => &self.config.color,
            _ => &self.config.low.state.color,
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
}
