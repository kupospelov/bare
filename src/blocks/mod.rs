pub mod battery;
pub mod cpu;
pub mod time;
pub mod volume;
pub mod wireless;
pub mod workspaces;

use crate::{
    config::{
        BatteryConfig, BlockConfig, ColorConfig, Config, CpuConfig, TimeConfig, VolumeConfig,
        WirelessConfig,
    },
    raster::Rasterizer,
    render,
    render::Layout,
};
use std::os::fd::{AsFd, BorrowedFd, RawFd};

pub trait FormatItem {
    fn label(&self) -> Option<&str>;

    fn height(&self, rasterizer: &Rasterizer, scale: i32) -> u32 {
        if let Some(s) = self.label() {
            rasterizer.get_font_size(s, scale)
        } else {
            rasterizer.get_default_font_size(scale)
        }
    }
}

pub fn content_height<B: Block + ?Sized>(block: &B, rasterizer: &Rasterizer, scale: i32) -> i32 {
    let len = block.len();
    if len < 1 {
        return 0;
    }

    let mut height = block.get(len - 1, rasterizer, scale).height;
    for i in 0..len - 1 {
        let h = block.get(i, rasterizer, scale).height;
        height += h;
        height += inner_margin(h) as u32;
    }

    height as i32
}

pub fn inner_margin(font_size: u32) -> i32 {
    font_size as i32 / 3
}

pub struct Line {
    pub height: u32,
    pub text: String,
}

#[derive(Clone, Copy)]
pub enum Instance {
    Time(usize),
    Battery(usize),
    Volume(usize),
    Wireless(usize),
    Cpu(usize),
}

pub struct Blocks {
    pub order: Vec<Instance>,
    pub time: time::Group,
    pub battery: battery::Group,
    pub volume: volume::Group,
    pub wireless: wireless::Group,
    pub cpu: cpu::Group,
}

impl Blocks {
    pub fn new(config: &Config) -> Self {
        let mut blocks = Self {
            order: Vec::with_capacity(config.bar.blocks.len()),
            time: time::Group::new(),
            battery: battery::Group::new(),
            volume: volume::Group::new(),
            wireless: wireless::Group::new(),
            cpu: cpu::Group::new(),
        };

        for (i, entry) in config.bar.blocks.iter().rev().enumerate() {
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
                    blocks.order.push(blocks.time.add(i, &cfg));
                }
                "battery" => {
                    let cfg = config
                        .battery
                        .get(name)
                        .cloned()
                        .unwrap_or_else(|| BatteryConfig::default(&config.bar.color));
                    blocks.order.push(blocks.battery.add(i, &cfg));
                }
                "volume" => {
                    let cfg = config
                        .volume
                        .get(name)
                        .cloned()
                        .unwrap_or_else(|| VolumeConfig::default(&config.bar.color));
                    blocks.order.push(blocks.volume.add(i, &cfg));
                }
                "wireless" => {
                    let cfg = config
                        .wireless
                        .get(name)
                        .cloned()
                        .unwrap_or_else(|| WirelessConfig::default(&config.bar.color));
                    blocks.order.push(blocks.wireless.add(i, &cfg));
                }
                "cpu" => {
                    let cfg = config
                        .cpu
                        .get(name)
                        .cloned()
                        .unwrap_or_else(|| CpuConfig::default(&config.bar.color));
                    blocks.order.push(blocks.cpu.add(i, &cfg));
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
            Instance::Wireless(j) => &self.wireless.instances[j],
            Instance::Cpu(j) => &self.cpu.instances[j],
        }
    }

    pub fn layout(&self, rasterizer: &Rasterizer, scale: i32, separator: u32) -> Layout {
        let font_size = rasterizer.get_default_font_size(scale);
        let separator = separator * scale as u32;
        Layout {
            font_size,
            separator,
            blocks: self
                .order
                .iter()
                .map(|r| self.resolve(*r).layout(rasterizer, scale))
                .collect(),
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
    fn layout(&self, rasterizer: &Rasterizer, scale: i32) -> render::BlockLayout {
        let content = content_height(self, rasterizer, scale);
        let block = self.block().scaled(scale);
        render::BlockLayout {
            content,
            height: block.height(content),
            config: block,
        }
    }

    /// The block config.
    fn block(&self) -> &BlockConfig;

    /// The block colors.
    fn colors(&self) -> &ColorConfig;

    /// The number of lines to render.
    fn len(&self) -> usize;

    /// The line to render.
    fn get(&self, index: usize, rasterizer: &Rasterizer, scale: i32) -> Line;
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
