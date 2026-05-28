use std::time::Instant;

use super::{Block, Instance};
use crate::config::{ColorConfig, TimeConfig, TimeFormatItem};
use crate::render;
use crate::state::State;
use crate::{debug, error};
use calloop::timer::{TimeoutAction, Timer};
use time::OffsetDateTime;

pub struct Group {
    pub now: OffsetDateTime,
    pub instances: Vec<Time>,
}

impl Group {
    pub fn new() -> Self {
        Self {
            now: Self::read_local(),
            instances: Vec::new(),
        }
    }

    fn read_local() -> OffsetDateTime {
        let now = match OffsetDateTime::now_local() {
            Ok(local) => local,
            Err(error) => {
                error!("Cannot get local time: {:?}", error);
                OffsetDateTime::now_utc()
            }
        };
        debug!("Read local time: {}", now);
        now
    }

    pub fn add(&mut self, config: &TimeConfig) -> Instance {
        let n = self.instances.len();
        self.instances.push(Time {
            now: self.now,
            config: config.clone(),
        });
        Instance::Time(n)
    }

    pub fn register_events(&self, handle: &calloop::LoopHandle<'_, State>) {
        if self.instances.is_empty() {
            return;
        }

        handle
            .insert_source(Timer::from_deadline(self.next_instant()), |_, _, state| {
                let now = Self::read_local();
                state.blocks.time.now = now;

                for i in 0..state.blocks.order.len() {
                    if let Instance::Time(j) = state.blocks.order[i]
                        && state.blocks.time.instances[j].update(now)
                    {
                        state.mark_all_outputs_block_dirty(i);
                    }
                }

                TimeoutAction::ToInstant(state.blocks.time.next_instant())
            })
            .expect("Failed to insert time group timer");
    }

    fn next_instant(&self) -> Instant {
        let next = 60 - (self.now.second() as u64);
        Instant::now() + std::time::Duration::from_secs(next)
    }
}

pub struct Time {
    now: time::OffsetDateTime,
    config: TimeConfig,
}

impl Time {
    fn item_text(&self, item: &TimeFormatItem) -> String {
        match item {
            TimeFormatItem::Hour => format!("{:02}", self.now.hour()),
            TimeFormatItem::Minute => format!("{:02}", self.now.minute()),
            TimeFormatItem::Day => format!("{:02}", self.now.day()),
            TimeFormatItem::Month => format!("{:02}", u8::from(self.now.month())),
            TimeFormatItem::Label(s) => s.clone(),
        }
    }

    fn item_height(item: &TimeFormatItem, font_size: u32) -> u32 {
        match item {
            TimeFormatItem::Label(s) => font_size * 2 / s.len().max(1) as u32,
            _ => font_size,
        }
    }

    fn update(&mut self, now: time::OffsetDateTime) -> bool {
        let changed = self.config.format.iter().any(|item| match item {
            TimeFormatItem::Hour => self.now.hour() != now.hour(),
            TimeFormatItem::Minute => self.now.minute() != now.minute(),
            TimeFormatItem::Day => self.now.day() != now.day(),
            TimeFormatItem::Month => self.now.month() != now.month(),
            TimeFormatItem::Label(_) => false,
        });
        self.now = now;
        changed
    }
}

impl Block for Time {
    fn layout(&self, font_size: u32, scale: i32) -> render::BlockLayout {
        let items = &self.config.format;
        let separator = super::inner_margin(font_size);
        let gaps = items.len().saturating_sub(1) as i32;
        let content: i32 = items
            .iter()
            .map(|i| Self::item_height(i, font_size) as i32)
            .sum::<i32>()
            + gaps * separator;
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
        renderer: &mut crate::render::Renderer,
        map: &mut render::Map<'_>,
        region: render::Region,
        font_size: u32,
    ) {
        let color = &self.config.color;
        let margin = super::inner_margin(font_size);
        let mut y = region.y;
        for item in &self.config.format {
            let h = Self::item_height(item, font_size);
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
            y += h as i32 + margin;
        }
    }
}
