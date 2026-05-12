use super::Block;
use crate::color::Color;
use crate::debug;
use crate::render;

const BATTERY_PATH: &str = "/sys/class/power_supply/BAT0/capacity";
const BATTERY_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);

pub struct Battery {
    pub capacity: String,
}

impl Default for Battery {
    fn default() -> Self {
        Self::new()
    }
}

impl Battery {
    pub fn new() -> Self {
        let mut battery = Self {
            capacity: String::new(),
        };
        battery.update();
        battery
    }
}

impl Block for Battery {
    fn height(&self, font_size: u32) -> i32 {
        font_size as i32 + super::inner_margin(font_size) + (font_size * 2 / 3) as i32
    }

    fn render(
        &mut self,
        renderer: &mut crate::render::Renderer,
        map: &mut render::Map<'_>,
        y: i32,
        font_size: u32,
        bg_color: Color,
    ) {
        let capacity = if self.capacity.is_empty() {
            "??"
        } else {
            &self.capacity
        };

        let margin = super::inner_margin(font_size);
        let label_size = font_size * 2 / 3;
        renderer.render_text(
            map,
            render::Region {
                x: 0,
                y,
                w: map.width,
                h: label_size,
            },
            "BAT",
            render::COLOR_INACTIVE,
            bg_color,
            label_size,
        );
        renderer.render_text(
            map,
            render::Region {
                x: 0,
                y: y + label_size as i32 + margin,
                w: map.width,
                h: font_size,
            },
            capacity,
            render::COLOR_INACTIVE,
            bg_color,
            font_size,
        );
    }

    fn update(&mut self) -> bool {
        if let Ok(capacity) = std::fs::read_to_string(BATTERY_PATH) {
            let c = capacity.trim().to_string();
            if c != self.capacity {
                debug!("Updated battery capacity: {}", c);

                self.capacity = c;
                return true;
            }
        }
        false
    }

    fn reschedule(&self) -> calloop::timer::TimeoutAction {
        calloop::timer::TimeoutAction::ToDuration(BATTERY_INTERVAL)
    }
}
