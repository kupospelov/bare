use crate::debug;
use wayland_client::{Proxy, backend::ObjectId, protocol::wl_pointer};
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1;

pub struct Pointer {
    pub handle: wl_pointer::WlPointer,
    pub cursor_shape_device: Option<wp_cursor_shape_device_v1::WpCursorShapeDeviceV1>,
    pub y: i32,
    pub surface: Option<ObjectId>,
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
