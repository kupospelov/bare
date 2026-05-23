pub mod battery;
pub mod time;
pub mod volume;
pub mod workspaces;

use crate::config::{BatteryConfig, ColorConfig, Config, TimeConfig, VolumeConfig};
use std::os::fd::{AsFd, BorrowedFd, RawFd};

pub fn inner_margin(font_size: u32) -> i32 {
    font_size as i32 / 5
}

pub fn new(config: &Config) -> Vec<Box<dyn Block>> {
    let mut blocks: Vec<Box<dyn Block>> = Vec::with_capacity(config.bar.blocks.len());
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
                blocks.push(Box::new(time::Time::new(&cfg)));
            }
            "battery" => {
                let cfg = config
                    .battery
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| BatteryConfig::default(&config.bar.color));
                blocks.push(Box::new(battery::Battery::new(&cfg)));
            }
            "volume" => {
                let cfg = config
                    .volume
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| VolumeConfig::default(&config.bar.color));
                let volume = volume::Volume::new(&cfg)
                    .unwrap_or_else(|e| panic!("Failed to construct volume.{}: {}", name, e));
                blocks.push(Box::new(volume));
            }
            _ => panic!(
                "Unknown block type '{}' in bar.blocks entry '{}'",
                kind, entry
            ),
        }
    }
    blocks
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

    /// Update internal state. Returns true if the display needs to be redrawn.
    fn update(&mut self) -> bool {
        false
    }

    /// Returns the next timeout action. Called once on registration and again after each timer
    /// fire. Return `Drop` if this block never needs timer-driven updates.
    fn reschedule(&self) -> calloop::timer::TimeoutAction {
        calloop::timer::TimeoutAction::Drop
    }

    /// Calloop event source to register, if this block is fd-driven.
    fn fd(&self) -> Option<calloop::generic::Generic<Fd>> {
        None
    }

    /// Drain pending events from the fd. Returns true if a redraw is needed.
    fn on_fd(&mut self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_blocks(entries: &[&str]) -> Config {
        let mut config = Config::default();
        config.bar.blocks = entries.iter().map(|s| (*s).to_string()).collect();
        config
    }

    #[test]
    #[should_panic(expected = "Invalid bar.blocks entry 'noname'")]
    fn missing_separator_panics() {
        new(&config_with_blocks(&["noname"]));
    }

    #[test]
    #[should_panic(expected = "Unknown block type 'unknown' in bar.blocks entry 'unknown.default'")]
    fn unknown_kind_panics() {
        new(&config_with_blocks(&["unknown.default"]));
    }
}
