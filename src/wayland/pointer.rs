use crate::debug;
use wayland_client::{Proxy, backend::ObjectId, protocol::wl_pointer};

pub struct Pointer {
    pub handle: wl_pointer::WlPointer,
    pub y: i32,
    pub surface: Option<ObjectId>,
}

impl Drop for Pointer {
    fn drop(&mut self) {
        debug!("Pointer {}: destroy", self.handle.id());

        self.handle.release();
    }
}
