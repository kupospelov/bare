use nix::sys::statvfs::statvfs;

use super::{Block, BlockDirty, FormatItem, Instance, Line};
use crate::config::{BlockConfig, ColorConfig, DriveConfig, DriveFormatItem};
use crate::raster::Rasterizer;
use crate::{debug, error};

fn calc_usage(total: u64, free: u64, available: u64) -> u8 {
    let used = u128::from(total.saturating_sub(free));
    let capacity = used + u128::from(available);
    if capacity == 0 {
        return 0;
    }

    (100 * used).div_ceil(capacity) as u8
}

pub struct Group {
    pub instances: Vec<Drive>,
}

impl Group {
    pub fn new() -> Self {
        Self {
            instances: Vec::new(),
        }
    }

    pub fn add(&mut self, id: usize, config: &DriveConfig) -> Instance {
        let n = self.instances.len();
        self.instances.push(Drive::new(id, config));
        Instance::Drive(n)
    }

    pub fn update(&mut self, dirty: &mut Vec<BlockDirty>) {
        for instance in &mut self.instances {
            if let Some(update) = instance.update() {
                dirty.push(update);
            }
        }
    }
}

pub struct Drive {
    id: usize,
    config: DriveConfig,
    usage: u8,
}

impl Drive {
    pub fn new(id: usize, config: &DriveConfig) -> Self {
        Self {
            id,
            config: config.clone(),
            usage: 0,
        }
    }

    fn update(&mut self) -> Option<BlockDirty> {
        let stats = match statvfs(&self.config.path) {
            Ok(stats) => stats,
            Err(err) => {
                error!(
                    "Cannot read filesystem usage for {}: {}",
                    self.config.path.display(),
                    err
                );
                return None;
            }
        };

        let usage = calc_usage(
            stats.blocks(),
            stats.blocks_free(),
            stats.blocks_available(),
        );
        self.update_usage(usage)
    }

    fn update_usage(&mut self, usage: u8) -> Option<BlockDirty> {
        if usage == self.usage {
            return None;
        }

        debug!(
            "Updated drive usage for {}: {}",
            self.config.path.display(),
            usage
        );
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

    fn format(&self) -> &[DriveFormatItem] {
        if self.is_high() {
            &self.config.high.state.format
        } else {
            &self.config.format
        }
    }
}

impl Block for Drive {
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
                DriveFormatItem::Usage => format!("{:02}", self.usage),
                DriveFormatItem::Label(s) => s.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_percentage() {
        assert_eq!(calc_usage(1000, 300, 200), 78);
        assert_eq!(calc_usage(1000, 999, 999), 1);
        assert_eq!(calc_usage(1000, 1000, 900), 0);
        assert_eq!(calc_usage(1000, 0, 0), 100);
        assert_eq!(calc_usage(1000, 100, 0), 100);
        assert_eq!(calc_usage(0, 0, 0), 0);
        assert_eq!(calc_usage(1000, 1000, 0), 0);
        assert_eq!(calc_usage(100, 200, 50), 0);
        assert_eq!(calc_usage(u64::MAX, 0, 0), 100);
        assert_eq!(calc_usage(u64::MAX, u64::MAX, 0), 0);
    }

    #[test]
    fn state_changes() {
        let mut config = DriveConfig::default(&ColorConfig::default());
        config.format = vec![DriveFormatItem::Label("normal".into())];
        config.high.state.format = vec![
            DriveFormatItem::Label("high".into()),
            DriveFormatItem::Usage,
        ];
        let mut drive = Drive::new(3, &config);

        // Initialize
        let dirty = drive.update_usage(80).unwrap();
        assert_eq!(drive.format(), config.format);
        assert_eq!(drive.colors(), &config.color);
        assert_eq!(
            dirty,
            BlockDirty {
                index: 3,
                layout: false
            }
        );

        // High
        let dirty = drive.update_usage(81).unwrap();
        assert_eq!(drive.format(), config.high.state.format);
        assert_eq!(drive.colors(), &config.high.state.color);
        assert_eq!(
            dirty,
            BlockDirty {
                index: 3,
                layout: true
            }
        );

        // Usage changes
        let dirty = drive.update_usage(90).unwrap();
        assert_eq!(drive.format(), config.high.state.format);
        assert_eq!(drive.colors(), &config.high.state.color);
        assert_eq!(
            dirty,
            BlockDirty {
                index: 3,
                layout: false
            }
        );

        // Normal
        let dirty = drive.update_usage(80).unwrap();
        assert_eq!(drive.format(), config.format);
        assert_eq!(drive.colors(), &config.color);
        assert_eq!(
            dirty,
            BlockDirty {
                index: 3,
                layout: true
            }
        );
    }
}
