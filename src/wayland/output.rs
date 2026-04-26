use crate::blocks;
use crate::debug;
use crate::wayland::buffer::Buffer;
use wayland_client::{
    backend::ObjectId,
    protocol::{wl_output, wl_surface},
};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1;

pub struct Output {
    pub name: u32,
    pub output: wl_output::WlOutput,
    pub surface: wl_surface::WlSurface,
    pub layer_surface: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    pub width: u32,
    pub height: u32,
    pub scale: i32,
    pub configured: bool,
    pub group: Option<ObjectId>,
    pub workspace_group: blocks::workspaces::Workspaces,
    pub buffer: Option<Buffer>,
    pub render: bool,
}

impl Output {
    pub fn new(
        name: u32,
        width: u32,
        output: wl_output::WlOutput,
        surface: wl_surface::WlSurface,
        layer_surface: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    ) -> Self {
        Self {
            name,
            output,
            surface,
            layer_surface,
            width: width,
            height: 0,
            scale: 1,
            configured: false,
            group: None,
            workspace_group: blocks::workspaces::Workspaces::new(),
            buffer: None,
            render: false,
        }
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        debug!("Output {}: destroy", self.name);

        self.layer_surface.destroy();
        self.surface.destroy();
        self.output.release();
    }
}
