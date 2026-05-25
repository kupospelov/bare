use super::Block;
use crate::config::{ColorConfig, TimeConfig, TimeFormatItem};
use crate::render;
use crate::{debug, error};

pub struct Group {
    pub instances: Vec<Time>,
}

impl Group {
    pub fn new() -> Self {
        Self {
            instances: Vec::new(),
        }
    }
}

pub struct Time {
    now: time::OffsetDateTime,
    config: TimeConfig,
}

impl Time {
    pub fn new(config: &TimeConfig) -> Self {
        Self {
            now: Self::now(),
            config: config.clone(),
        }
    }

    fn now() -> time::OffsetDateTime {
        let now = match time::OffsetDateTime::now_local() {
            Ok(local) => local,
            Err(error) => {
                error!("Cannot get local time: {:?}", error);
                time::OffsetDateTime::now_utc()
            }
        };
        debug!("Updated local time: {}", now);
        now
    }

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
}

impl Block for Time {
    fn layout(&self, font_size: u32, scale: i32) -> render::BlockLayout {
        let items = &self.config.format;
        let separator = super::inner_margin(font_size);
        let gaps = items.len().saturating_sub(1) as i32;
        let height: i32 = items
            .iter()
            .map(|i| Self::item_height(i, font_size) as i32)
            .sum::<i32>()
            + gaps * separator;
        let block = self.config.block.scaled(scale);
        render::BlockLayout {
            content: height,
            height: block.height.unwrap_or(height) + block.margins[0] + block.margins[2],
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

    fn update(&mut self) -> bool {
        self.now = Self::now();
        true
    }

    fn reschedule(&self) -> calloop::timer::TimeoutAction {
        let now = time::OffsetDateTime::now_utc();
        let next = 60 - (now.second() as u64);
        calloop::timer::TimeoutAction::ToInstant(
            std::time::Instant::now() + std::time::Duration::from_secs(next),
        )
    }
}
