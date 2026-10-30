use std::fs;

use super::{Block, BlockDirty, Instance, Line};
use crate::blocks::FormatItem;
use crate::config::{BlockConfig, ColorConfig, MemoryConfig, MemoryFormatItem};
use crate::raster::Rasterizer;
use crate::{debug, error};

#[derive(Default)]
struct Meminfo {
    total: u64,
    available: u64,
}

impl Meminfo {
    fn parse(meminfo: &str) -> Self {
        let mut m = Self::default();
        for line in meminfo.lines() {
            let mut fields = line.split_whitespace();
            let Some(key) = fields.next() else {
                continue;
            };
            let field = match key {
                "MemTotal:" => &mut m.total,
                "MemAvailable:" => &mut m.available,
                _ => continue,
            };
            let Some(value) = fields.next() else {
                error!("{} missing value", key);
                continue;
            };
            let Ok(parsed) = value.parse::<u64>() else {
                error!("{} cannot parse {}", key, value);
                continue;
            };
            *field = parsed;
        }
        m
    }

    fn calc_usage(&self) -> u8 {
        if self.total == 0 {
            return 0;
        }

        let used = self.total.saturating_sub(self.available);
        ((100 * used + self.total / 2) / self.total) as u8
    }
}

pub struct Group {
    pub instances: Vec<Memory>,
}

impl Group {
    pub fn new() -> Self {
        Self {
            instances: Vec::new(),
        }
    }

    pub fn add(&mut self, id: usize, config: &MemoryConfig) -> Instance {
        let n = self.instances.len();
        self.instances.push(Memory::new(id, config));
        Instance::Memory(n)
    }

    pub fn update(&mut self, dirty: &mut Vec<BlockDirty>) {
        let Ok(meminfo) = fs::read_to_string("/proc/meminfo") else {
            error!("Cannot read /proc/meminfo");
            return;
        };

        let usage = Meminfo::parse(&meminfo).calc_usage();
        for instance in &mut self.instances {
            if let Some(update) = instance.update(usage) {
                dirty.push(update);
            }
        }
    }
}

pub struct Memory {
    id: usize,
    config: MemoryConfig,
    usage: u8,
}

impl Memory {
    pub fn new(id: usize, config: &MemoryConfig) -> Self {
        Self {
            id,
            config: config.clone(),
            usage: 0,
        }
    }

    fn update(&mut self, usage: u8) -> Option<BlockDirty> {
        if usage == self.usage {
            return None;
        }

        debug!("Updated memory usage: {:?}", usage);
        let high = self.is_high();
        self.usage = usage;
        Some(BlockDirty {
            index: self.id,
            layout: high != self.is_high(),
        })
    }

    fn is_high(&self) -> bool {
        self.usage > self.config.high.threshold
    }

    fn format(&self) -> &[MemoryFormatItem] {
        if self.is_high() {
            &self.config.high.state.format
        } else {
            &self.config.format
        }
    }
}

impl Block for Memory {
    fn block(&self) -> &BlockConfig {
        &self.config.block
    }

    fn colors(&self) -> &ColorConfig {
        if self.is_high() {
            &self.config.high.state.color
        } else {
            &self.config.color
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
                MemoryFormatItem::Usage => format!("{:02}", self.usage),
                MemoryFormatItem::Label(s) => s.clone(),
            },
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meminfo_parse() {
        let meminfo =
            Meminfo::parse("\nMemTotal: 1000 kB\nMemFree: 100 kB\n\nMemAvailable: 345 kB\n");
        assert_eq!(meminfo.total, 1000);
        assert_eq!(meminfo.available, 345);
        assert_eq!(meminfo.calc_usage(), 66);
    }
}
