pub mod battery;
pub mod time;
pub mod volume;
pub mod workspaces;

use crate::config::{BatteryConfig, ColorConfig, Config, TimeConfig, VolumeConfig};
use std::os::fd::{AsFd, BorrowedFd, RawFd};

pub fn inner_margin(font_size: u32) -> i32 {
    font_size as i32 / 5
}

#[derive(Clone, Copy)]
pub enum Instance {
    Time(usize),
    Battery(usize),
    Volume(usize),
}

pub struct Blocks {
    pub order: Vec<Instance>,
    pub time: time::Group,
    pub battery: battery::Group,
    pub volume: volume::Group,
}

impl Blocks {
    pub fn new(config: &Config) -> Self {
        let mut blocks = Self {
            order: Vec::with_capacity(config.bar.blocks.len()),
            time: time::Group::new(),
            battery: battery::Group::new(),
            volume: volume::Group::new(),
        };

        for entry in config.bar.blocks.iter().rev() {
            let (kind, name) = entry.split_once('.').unwrap_or_else(|| {
                panic!(
                    "Invalid bar.blocks entry '{}': expected '<type>.<name>'",
                    entry
                )
            });
            match kind {
                "time" => {
                    let cfg = config
                        .time
                        .get(name)
                        .cloned()
                        .unwrap_or_else(|| TimeConfig::default(&config.bar.color));
                    blocks.order.push(blocks.time.add(&cfg));
                }
                "battery" => {
                    let cfg = config
                        .battery
                        .get(name)
                        .cloned()
                        .unwrap_or_else(|| BatteryConfig::default(&config.bar.color));
                    blocks.order.push(blocks.battery.add(&cfg));
                }
                "volume" => {
                    let cfg = config
                        .volume
                        .get(name)
                        .cloned()
                        .unwrap_or_else(|| VolumeConfig::default(&config.bar.color));
                    blocks.order.push(blocks.volume.add(&cfg));
                }
                _ => panic!(
                    "Unknown block type '{}' in bar.blocks entry '{}'",
                    kind, entry
                ),
            }
        }
        blocks
    }

    pub fn resolve(&self, r: Instance) -> &dyn Block {
        match r {
            Instance::Time(j) => &self.time.instances[j],
            Instance::Battery(j) => &self.battery.instances[j],
            Instance::Volume(j) => &self.volume.instances[j],
        }
    }

    pub fn resolve_mut(&mut self, r: Instance) -> &mut dyn Block {
        match r {
            Instance::Time(j) => &mut self.time.instances[j],
            Instance::Battery(j) => &mut self.battery.instances[j],
            Instance::Volume(j) => &mut self.volume.instances[j],
        }
    }
}

pub struct Fd(pub RawFd);

impl AsFd for Fd {
    fn as_fd(&self) -> BorrowedFd<'_> {
        unsafe { BorrowedFd::borrow_raw(self.0) }
    }
}

pub trait Block {
    /// The block layout.
    fn layout(&self, font_size: u32, scale: i32) -> crate::render::BlockLayout;

    /// The block colors.
    fn colors(&self) -> &ColorConfig;

    /// Render into the region of `mapping`.
    fn render(
        &mut self,
        renderer: &mut crate::render::Renderer,
        map: &mut crate::render::Map<'_>,
        region: crate::render::Region,
        font_size: u32,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_blocks(entries: &[&str]) -> Config {
        crate::log::set(crate::log::Level::Error);
        let mut config = Config::default();
        config.bar.blocks = entries.iter().map(|s| (*s).to_string()).collect();
        config
    }

    #[test]
    #[should_panic(expected = "Invalid bar.blocks entry 'noname'")]
    fn missing_separator_panics() {
        Blocks::new(&config_with_blocks(&["noname"]));
    }

    #[test]
    #[should_panic(expected = "Unknown block type 'unknown' in bar.blocks entry 'unknown.default'")]
    fn unknown_kind_panics() {
        Blocks::new(&config_with_blocks(&["unknown.default"]));
    }
}
