use super::Block;
use crate::render::{
    COLOR_ACTIVE, COLOR_INACTIVE, COLOR_URGENT, COLOR_WORKSPACE_ACTIVE_BG, Renderer,
};
use crate::state::Workspace;
use wayland_protocols::ext::workspace::v1::client::ext_workspace_handle_v1;

pub struct Workspaces {
    pub items: Vec<Workspace>,
    pub height: i32,
    y_start: i32,
}

impl Workspaces {
    pub fn new(height: i32) -> Self {
        Self {
            items: Vec::new(),
            height,
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
    fn height(&self, _font_size: u32) -> i32 {
        self.items.len() as i32 * self.height
    }

    fn render(
        &mut self,
        renderer: &mut Renderer,
        mapping: &mut [u8],
        width: u32,
        height: u32,
        y: i32,
        font_size: u32,
        bg_color: [u8; 4],
    ) {
        let mut y = y;
        self.y_start = y;

        for ws in &self.items {
            let text_color = if ws.active {
                COLOR_ACTIVE
            } else if ws.urgent {
                COLOR_URGENT
            } else {
                COLOR_INACTIVE
            };
            let ws_bg_color = if ws.active {
                COLOR_WORKSPACE_ACTIVE_BG
            } else {
                bg_color
            };

            if ws_bg_color != bg_color {
                renderer.fill_rect(mapping, width, height, y, self.height, ws_bg_color);
            }

            let text_y = y + (self.height - font_size as i32).max(0) / 2;
            renderer.render_text(
                mapping,
                width,
                height,
                text_y,
                &ws.name,
                text_color,
                ws_bg_color,
                font_size,
            );

            y += self.height;
        }
    }
}
