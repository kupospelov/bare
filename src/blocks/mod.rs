pub mod battery;
pub mod time;
pub mod workspaces;

use crate::color::Color;
use crate::config::Config;

pub fn inner_margin(font_size: u32) -> i32 {
    font_size as i32 / 5
}

pub trait Block {
    /// Physical pixel height of this block. Called by the renderer for layout before rendering.
    fn height(&self, font_size: u32) -> i32;

    /// Render into `mapping`. `y` is the physical-pixel bottom anchor (lower y = higher on
    /// screen).
    fn render(
        &mut self,
        renderer: &mut crate::render::Renderer,
        map: &mut crate::render::Map<'_>,
        y: i32,
        font_size: u32,
        bg_color: Color,
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

    /// React to an output scale change.
    fn set_scale(&mut self, _config: &Config, _scale: i32) {}
}
