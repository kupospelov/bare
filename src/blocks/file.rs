use super::{Block, BlockDirty, FormatItem, Instance, Line};
use crate::config::{BlockConfig, ColorConfig, FileConfig};
use crate::raster::Rasterizer;

pub struct Group {
    pub instances: Vec<File>,
}

impl Group {
    pub fn new() -> Self {
        Self {
            instances: Vec::new(),
        }
    }

    pub fn add(&mut self, id: usize, config: &FileConfig) -> Instance {
        let n = self.instances.len();
        self.instances.push(File::new(id, config));
        Instance::File(n)
    }

    pub fn update(&mut self, dirty: &mut Vec<BlockDirty>) {
        for instance in &mut self.instances {
            if let Some(update) = instance.update() {
                dirty.push(update);
            }
        }
    }
}

pub struct File {
    id: usize,
    config: FileConfig,
    exists: bool,
}

impl File {
    pub fn new(id: usize, config: &FileConfig) -> Self {
        Self {
            id,
            config: config.clone(),
            exists: config.path.exists(),
        }
    }

    fn update(&mut self) -> Option<BlockDirty> {
        let exists = self.config.path.exists();
        if exists == self.exists {
            return None;
        }

        self.exists = exists;
        Some(BlockDirty {
            index: self.id,
            layout: true,
        })
    }

    fn format(&self) -> &[String] {
        if self.exists {
            &self.config.format
        } else {
            &self.config.down.format
        }
    }
}

impl Block for File {
    fn block(&self) -> &BlockConfig {
        &self.config.block
    }

    fn colors(&self) -> &ColorConfig {
        if self.exists {
            &self.config.color
        } else {
            &self.config.down.color
        }
    }

    fn len(&self) -> usize {
        self.format().len()
    }

    fn get(&self, index: usize, rasterizer: &Rasterizer, scale: i32) -> Line {
        let item = &self.format()[index];
        Line {
            height: item.height(rasterizer, scale),
            text: item.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::tmp::Directory;
    use std::fs;

    #[test]
    fn state_changes() {
        let tmp = Directory::new();
        let config = FileConfig {
            path: tmp.path().join("flag"),
            ..FileConfig::default(&ColorConfig::default())
        };

        let mut file = File::new(3, &config);
        let dirty = Some(BlockDirty {
            index: 3,
            layout: true,
        });
        assert!(!file.exists);
        assert_eq!(file.update(), None);

        tmp.write("flag", "contents");
        assert_eq!(file.update(), dirty);
        assert!(file.exists);
        assert_eq!(file.update(), None);

        fs::remove_file(&config.path).unwrap();
        assert_eq!(file.update(), dirty);
        assert!(!file.exists);
        assert_eq!(file.update(), None);
    }
}
