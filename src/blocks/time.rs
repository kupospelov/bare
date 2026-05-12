use super::Block;
use crate::color::Color;
use crate::render;
use crate::{debug, error};

pub struct Time {
    pub hours: String,
    pub minutes: String,
}

impl Default for Time {
    fn default() -> Self {
        Self::new()
    }
}

impl Time {
    pub fn new() -> Self {
        let (hours, minutes) = Self::get_time();
        Self { hours, minutes }
    }

    fn get_time() -> (String, String) {
        let now = match time::OffsetDateTime::now_local() {
            Ok(local) => local,
            Err(error) => {
                error!("Cannot get local time: {:?}", error);
                time::OffsetDateTime::now_utc()
            }
        };

        debug!("Updated local time: {}", now);
        (format!("{:02}", now.hour()), format!("{:02}", now.minute()))
    }
}

impl Block for Time {
    fn height(&self, font_size: u32) -> i32 {
        font_size as i32 * 2 + super::inner_margin(font_size)
    }

    fn render(
        &mut self,
        renderer: &mut crate::render::Renderer,
        map: &mut render::Map<'_>,
        y: i32,
        font_size: u32,
        bg_color: Color,
    ) {
        let margin = super::inner_margin(font_size);
        renderer.render_text(
            map,
            render::Region {
                x: 0,
                y,
                w: map.width,
                h: font_size,
            },
            &self.hours,
            render::COLOR_INACTIVE,
            bg_color,
            font_size,
        );
        renderer.render_text(
            map,
            render::Region {
                x: 0,
                y: y + font_size as i32 + margin,
                w: map.width,
                h: font_size,
            },
            &self.minutes,
            render::COLOR_INACTIVE,
            bg_color,
            font_size,
        );
    }

    fn update(&mut self) -> bool {
        let (hours, minutes) = Self::get_time();
        self.hours = hours;
        self.minutes = minutes;
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
