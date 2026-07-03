use std::time::Duration;

use super::{Block, Instance};
use crate::blocks::FormatItem;
use crate::config::{ColorConfig, TimeConfig, TimeFormatItem};
use crate::map::Map;
use crate::raster::Rasterizer;
use crate::render;
use crate::render::Renderer;
use crate::state::State;
use crate::{debug, error, fail};
use calloop::timer::{TimeoutAction, Timer};
use tz::{DateTime, UtcDateTime};

pub struct Group {
    pub instances: Vec<Time>,
}

impl Group {
    pub fn new() -> Self {
        Self {
            instances: Vec::new(),
        }
    }

    pub fn add(&mut self, id: usize, config: &TimeConfig) -> Instance {
        let n = self.instances.len();
        let now = match UtcDateTime::UNIX_EPOCH.project(config.timezone.as_ref()) {
            Ok(t) => t,
            Err(e) => {
                fail!("Failed to project time to timezone: {}", e);
            }
        };
        self.instances.push(Time {
            id,
            now,
            config: config.clone(),
        });
        Instance::Time(n)
    }

    pub fn register_events(&self, handle: &calloop::LoopHandle<'_, State>) {
        if self.instances.is_empty() {
            return;
        }

        handle
            .insert_source(Timer::immediate(), |instant, _, state| {
                let utc = match UtcDateTime::now() {
                    Ok(t) => {
                        debug!("Read UTC time: {}", t);
                        t
                    }
                    Err(e) => {
                        error!("Cannot read UTC time: {}", e);
                        return TimeoutAction::Drop;
                    }
                };

                for i in 0..state.blocks.time.instances.len() {
                    let id = {
                        let instance = &mut state.blocks.time.instances[i];

                        if !instance.update(utc) {
                            continue;
                        }
                        instance.id
                    };

                    state.mark_all_outputs_block_dirty(id);
                }

                TimeoutAction::ToInstant(instant + Duration::from_secs(60 - utc.second() as u64))
            })
            .expect("Failed to insert time group timer");
    }
}

pub struct Time {
    id: usize,
    now: DateTime,
    config: TimeConfig,
}

impl Time {
    fn item_text(&self, item: &TimeFormatItem) -> String {
        match item {
            TimeFormatItem::Hour => format!("{:02}", self.now.hour()),
            TimeFormatItem::Minute => format!("{:02}", self.now.minute()),
            TimeFormatItem::Day => format!("{:02}", self.now.month_day()),
            TimeFormatItem::Month => format!("{:02}", self.now.month()),
            TimeFormatItem::Label(s) => s.clone(),
        }
    }

    fn update(&mut self, utc: UtcDateTime) -> bool {
        let now = match utc.project(self.config.timezone.as_ref()) {
            Ok(t) => t,
            Err(e) => {
                error!("Failed to project time to timezone: {}", e);
                return false;
            }
        };

        let changed = self.config.format.iter().any(|item| match item {
            TimeFormatItem::Hour => self.now.hour() != now.hour(),
            TimeFormatItem::Minute => self.now.minute() != now.minute(),
            TimeFormatItem::Day => self.now.month_day() != now.month_day(),
            TimeFormatItem::Month => self.now.month() != now.month(),
            TimeFormatItem::Label(_) => false,
        });
        self.now = now;
        changed
    }
}

impl Block for Time {
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
        &self.config.color
    }

    fn render(
        &mut self,
        renderer: &mut Renderer,
        map: &mut dyn Map,
        region: render::Region,
        scale: i32,
    ) {
        let color = &self.config.color;
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
