use super::Block;
use crate::config::{BlockConfig, WorkspaceConfig};
use crate::render::{self, Renderer};
use crate::state::Workspace;
use wayland_protocols::ext::workspace::v1::client::ext_workspace_handle_v1;

pub struct Workspaces {
    pub items: Vec<Workspace>,
    pub height: i32,
    config: WorkspaceConfig,
    y_start: i32,
}

impl Workspaces {
    pub fn new(height: i32, config: &WorkspaceConfig) -> Self {
        Self {
            items: Vec::new(),
            height,
            config: config.scaled(1),
            y_start: 0,
        }
    }

    pub fn handle_at(&self, y: i32) -> Option<&ext_workspace_handle_v1::ExtWorkspaceHandleV1> {
        if self.height == 0 {
            return None;
        }
        let index = (y - self.y_start) / self.height;
        if index >= 0 && (index as usize) < self.items.len() {
            Some(&self.items[index as usize].handle)
        } else {
            None
        }
    }
}

impl Block for Workspaces {
    fn layout(&self, _font_size: u32) -> render::BlockLayout {
        render::BlockLayout {
            height: self.items.len() as i32 * self.height,
            config: BlockConfig::default(),
        }
    }

    fn colors(&self) -> &crate::config::ColorConfig {
        // Ignored. Per-workspace colors are used instead.
        &self.config.inactive.color
    }

    fn render(
        &mut self,
        renderer: &mut Renderer,
        map: &mut render::Map<'_>,
        region: render::Region,
        font_size: u32,
    ) {
        self.y_start = region.y;
        let mut y = region.y;
        for ws in &self.items {
            let state = if ws.active {
                &self.config.active
            } else if ws.urgent {
                &self.config.urgent
            } else {
                &self.config.inactive
            };
            let outer = render::Region {
                x: region.x,
                y,
                w: region.w,
                h: self.height as u32,
            };
            let inner = renderer.draw_block(
                map,
                outer,
                &self.config.block,
                state.color.background,
                state.color.border,
            );
            if inner.w > 0 && inner.h > 0 {
                renderer.render_text(
                    map,
                    inner,
                    &ws.name,
                    state.color.text,
                    state.color.background,
                    font_size,
                );
            }
            y += self.height;
        }
    }

    fn set_scale(&mut self, config: &crate::config::Config, scale: i32) {
        self.height = config.bar.width as i32 * scale;
        self.config = config.workspace.scaled(scale);
    }
}
