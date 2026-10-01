use crate::blocks;
use crate::blocks::Blocks;
use crate::config::WorkspaceConfig;
use crate::debug;
use crate::render::{BlockLayout, Layout, Range};
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
    pub frame_pending: bool,
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
        font_size: u32,
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
            frame_pending: false,
            group: None,
            workspace_group: blocks::workspaces::Workspaces::new(workspace, font_size),
            layout: Layout::default(),
            buffer: None,
            dirty: None,
        }
    }

    pub fn update_block_layouts(
        &mut self,
        blocks: &Blocks,
        rasterizer: &crate::raster::Rasterizer,
        separator: u32,
    ) {
        self.layout = blocks.layout(rasterizer, self.scale, separator);
    }

    pub fn update_block_layout(
        &mut self,
        blocks: &Blocks,
        rasterizer: &crate::raster::Rasterizer,
        i: usize,
    ) -> Range {
        update_block_layout(
            self.physical_height(),
            &mut self.layout,
            i,
            blocks
                .resolve(blocks.order[i])
                .layout(rasterizer, self.scale),
        )
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
        block_range(self.physical_height(), &self.layout, i)
    }
}

fn block_range(physical_height: i32, layout: &Layout, i: usize) -> Range {
    let separator = layout.separator as i32;
    let mut y = physical_height;
    for block in &layout.blocks[..i] {
        if block.height < 1 {
            continue;
        }

        y -= block.height + separator;
    }
    let height = layout.blocks[i].height;
    y -= height;
    Range::new(y, y + height)
}

fn update_block_layout(
    physical_height: i32,
    current: &mut Layout,
    i: usize,
    layout: BlockLayout,
) -> Range {
    let range = block_range(physical_height, current, i);
    let mut hdiff = current.blocks[i].height - layout.height;
    if hdiff == 0 {
        current.blocks[i] = layout;
        return range;
    }

    // Unhiding a block also restores its separator.
    if current.blocks[i].height == 0 {
        hdiff -= current.separator as i32;
    }

    let start =
        block_range(physical_height, current, current.blocks.len() - 1).start + hdiff.min(0);
    current.blocks[i] = layout;
    Range::new(start, range.end)
}

impl Drop for Output {
    fn drop(&mut self) {
        debug!("Output {}: destroy", self.name);

        self.layer_surface.destroy();
        self.surface.destroy();
        self.output.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PHYSICAL_HEIGHT: i32 = 100;

    fn create_layout() -> Layout {
        Layout {
            separator: 5,
            blocks: vec![
                // 90..100
                BlockLayout {
                    content: 10,
                    height: 10,
                    ..Default::default()
                },
                // 65..85
                BlockLayout {
                    content: 20,
                    height: 20,
                    ..Default::default()
                },
                // 30..60
                BlockLayout {
                    content: 30,
                    height: 30,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn new_layout_with_same_height() {
        let mut current = create_layout();
        let range = update_block_layout(
            PHYSICAL_HEIGHT,
            &mut current,
            1,
            BlockLayout {
                content: 12,
                height: 20,
                ..Default::default()
            },
        );

        assert_eq!(range, Range::new(65, 85));
        assert_eq!(current.blocks[1].content, 12);
    }

    #[test]
    fn new_layout_with_greater_height() {
        let mut current = create_layout();
        let range = update_block_layout(
            PHYSICAL_HEIGHT,
            &mut current,
            1,
            BlockLayout {
                content: 25,
                height: 25,
                ..Default::default()
            },
        );

        assert_eq!(range, Range::new(25, 85));
        assert_eq!(current.blocks[1].height, 25);
    }

    #[test]
    fn new_layout_with_smaller_height() {
        let mut current = create_layout();
        let range = update_block_layout(
            PHYSICAL_HEIGHT,
            &mut current,
            1,
            BlockLayout {
                content: 15,
                height: 15,
                ..Default::default()
            },
        );

        assert_eq!(range, Range::new(30, 85));
        assert_eq!(current.blocks[1].height, 15);
    }

    #[test]
    fn new_layout_hides_block() {
        let mut current = create_layout();

        // Hide block 1 and its separator.
        let range = update_block_layout(PHYSICAL_HEIGHT, &mut current, 1, BlockLayout::default());
        assert_eq!(range, Range::new(30, 85));
        assert_eq!(
            block_range(PHYSICAL_HEIGHT, &current, 1),
            Range::new(85, 85)
        );
        assert_eq!(
            block_range(PHYSICAL_HEIGHT, &current, 2),
            Range::new(55, 85)
        );

        // Unhide block 1 and its separator.
        let range = update_block_layout(
            PHYSICAL_HEIGHT,
            &mut current,
            1,
            BlockLayout {
                content: 20,
                height: 20,
                ..Default::default()
            },
        );
        assert_eq!(range, Range::new(30, 85));
        assert_eq!(
            block_range(PHYSICAL_HEIGHT, &current, 1),
            Range::new(65, 85)
        );
        assert_eq!(
            block_range(PHYSICAL_HEIGHT, &current, 2),
            Range::new(30, 60)
        );
    }
}
