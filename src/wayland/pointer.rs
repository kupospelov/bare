use crate::wayland::output::Output;
use crate::{debug, warning};
use std::collections::HashMap;
use wayland_client::{Proxy, backend::ObjectId, protocol::wl_pointer};
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1;

const SCROLL_THRESHOLD: f64 = 15.0;

pub struct Pointer {
    pub handle: wl_pointer::WlPointer,
    pub cursor_shape_device: Option<wp_cursor_shape_device_v1::WpCursorShapeDeviceV1>,
    pub y: i32,
    pub scroll_accumulator: f64,
    pub surface: Option<ObjectId>,
}

impl Pointer {
    pub fn scroll(&mut self, value: f64) -> i32 {
        self.scroll_accumulator += value;

        let steps = (self.scroll_accumulator / SCROLL_THRESHOLD) as i32;
        if steps == 0 {
            return 0;
        };

        self.scroll_accumulator -= steps as f64 * SCROLL_THRESHOLD;
        steps
    }

    pub fn get_output<'a>(&self, outputs: &'a HashMap<ObjectId, Output>) -> Option<&'a Output> {
        let Some(surface_id) = self.surface.as_ref() else {
            warning!("Pointer {} has no surface", self.handle.id());
            return None;
        };

        let Some(output) = outputs.values().find(|o| o.surface.id() == *surface_id) else {
            warning!("Surface {} is not attached to an output", *surface_id);
            return None;
        };

        Some(output)
    }
}

impl Drop for Pointer {
    fn drop(&mut self) {
        debug!("Pointer {}: destroy", self.handle.id());

        if let Some(d) = self.cursor_shape_device.as_ref() {
            d.destroy();
        }
        self.handle.release();
    }
}
