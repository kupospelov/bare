use std::fs;

use super::{Block, Instance};
use crate::blocks::FormatItem;
use crate::config::{ColorConfig, CpuConfig, CpuFormatItem};
use crate::raster::Rasterizer;
use crate::render;
use crate::render::Renderer;
use crate::{debug, error};

struct Event {
    idle: u64,
    total: u64,
}

fn read_proc_stat() -> Option<Event> {
    let stat = fs::read_to_string("/proc/stat").ok()?;
    let Some(line) = stat.lines().next() else {
        error!("No first line in /proc/stat");
        return None;
    };

    let parts: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|s| s.parse().ok())
        .collect();
    if parts.len() < 4 {
        error!("Unexpected number of fields in /proc/stat");
        return None;
    }

    Some(Event {
        idle: parts[3],
        total: parts[..4].iter().sum(),
    })
}

pub struct Group {
    pub instances: Vec<Cpu>,
}

impl Group {
    pub fn new() -> Self {
        Self {
            instances: Vec::new(),
        }
    }

    pub fn add(&mut self, id: usize, config: &CpuConfig) -> Instance {
        let n = self.instances.len();
        self.instances.push(Cpu::new(id, config));
        Instance::Cpu(n)
    }

    pub fn update(&mut self, dirty: &mut Vec<usize>) {
        let Some(event) = read_proc_stat() else {
            return;
        };

        for instance in &mut self.instances {
            if !instance.update(&event) {
                continue;
            }

            dirty.push(instance.id);
        }
    }
}

pub struct Cpu {
    id: usize,
    config: CpuConfig,
    idle: u64,
    total: u64,
    usage: u8,
}

impl Cpu {
    pub fn new(id: usize, config: &CpuConfig) -> Self {
        Self {
            id,
            config: config.clone(),
            idle: 0,
            total: 0,
            usage: 0,
        }
    }

    fn update(&mut self, event: &Event) -> bool {
        let diff_idle = event.idle.saturating_sub(self.idle) as f64;
        let diff_total = event.total.saturating_sub(self.total) as f64;
        let value = 100.0 * diff_idle / diff_total;
        let usage = 100 - value.round().clamp(0.0, 100.0) as u8;

        self.idle = event.idle;
        self.total = event.total;
        if usage != self.usage {
            debug!("Updated CPU usage: {:?}", usage);
            self.usage = usage;
            true
        } else {
            false
        }
    }

    fn item_text(&self, item: &CpuFormatItem) -> String {
        match item {
            CpuFormatItem::Usage => format!("{:02}", self.usage),
            CpuFormatItem::Label(s) => s.clone(),
        }
    }
}

impl Block for Cpu {
    fn layout(&self, rasterizer: &Rasterizer, scale: i32) -> render::BlockLayout {
        let content = super::content_height(&self.config.format, rasterizer, scale);
        let block = self.config.block.scaled(scale);
        render::BlockLayout {
            content,
            height: block.height(content),
            config: block,
        }
    }

    fn colors(&self) -> &ColorConfig {
        if self.usage > self.config.high.threshold {
            &self.config.high.state.color
        } else {
            &self.config.color
        }
    }

    fn render(
        &mut self,
        renderer: &mut Renderer,
        map: &mut render::Map<'_>,
        region: render::Region,
        scale: i32,
    ) {
        let color = self.colors();
        let mut y = region.y;
        for item in &self.config.format {
            let h = item.height(&renderer.rasterizer, scale);
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
            y += h as i32 + super::inner_margin(h);
        }
    }
}
