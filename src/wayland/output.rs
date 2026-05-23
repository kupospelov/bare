use crate::blocks;
use crate::blocks::Block;
use crate::config::WorkspaceConfig;
use crate::debug;
use crate::render::{Layout, Range};
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
    pub layout: Layout,
    pub buffer: Option<Buffer>,
    pub dirty: Option<Range>,
}

impl Output {
    pub fn new(
        name: u32,
        width: u32,
        workspace: &WorkspaceConfig,
        output: wl_output::WlOutput,
        surface: wl_surface::WlSurface,
        layer_surface: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    ) -> Self {
        Self {
            name,
            output,
            surface,
            layer_surface,
            width,
            height: 0,
            scale: 1,
            configured: false,
            group: None,
            workspace_group: blocks::workspaces::Workspaces::new(width as i32, workspace),
            layout: Layout::default(),
            buffer: None,
            dirty: None,
        }
    }

    pub fn update_layout(&mut self, blocks: &[Box<dyn Block>], font_size: u32, separator: u32) {
        let font_size = font_size * self.scale as u32;
        let separator = separator * self.scale as u32;
        self.layout = Layout {
            font_size,
            separator,
            blocks: blocks
                .iter()
                .map(|b| b.layout(font_size, self.scale))
                .collect(),
        };
    }

    pub fn physical_height(&self) -> i32 {
        self.height as i32 * self.scale
    }

    pub fn mark_dirty(&mut self, range: Range) {
        self.dirty = Some(match self.dirty {
            Some(d) => d.union(range),
            None => range,
        });
    }

    pub fn mark_full_dirty(&mut self) {
        debug!("Mark full bar dirty");
        self.mark_dirty(Range::new(0, self.physical_height()));
    }

    pub fn block_range(&self, i: usize) -> Range {
        let separator = self.layout.separator as i32;
        let mut y = self.physical_height();
        for j in 0..i {
            y -= self.layout.blocks[j].height + separator;
        }
        let height = self.layout.blocks[i].height;
        y -= height;
        Range::new(y, y + height)
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
