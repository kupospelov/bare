use super::Block;
use crate::config::Config;
use crate::render::{
    self, COLOR_ACTIVE, COLOR_INACTIVE, COLOR_URGENT, COLOR_WORKSPACE_ACTIVE_BG, Renderer,
};
use crate::state::Workspace;
use wayland_protocols::ext::workspace::v1::client::ext_workspace_handle_v1;

pub struct Workspaces {
    pub items: Vec<Workspace>,
    pub height: i32,
    pub gaps: [i32; 4],
    y_start: i32,
}

impl Workspaces {
    pub fn new(height: i32, gaps: [i32; 4]) -> Self {
        Self {
            items: Vec::new(),
            height,
            gaps,
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
        map: &mut render::Map<'_>,
        y: i32,
        font_size: u32,
        bg_color: [u8; 4],
    ) {
        let mut y = y;
        self.y_start = y;
        let [top, right, bottom, left] = self.gaps;

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

            let inner = render::Region {
                x: left,
                y: y + top,
                w: (map.width as i32 - left - right).max(0) as u32,
                h: (self.height - top - bottom).max(0) as u32,
            };

            if inner.w > 0 && inner.h > 0 {
                if ws_bg_color != bg_color {
                    renderer.fill_rect(map, inner, ws_bg_color);
                }
                renderer.render_text(map, inner, &ws.name, text_color, ws_bg_color, font_size);
            }

            y += self.height;
        }
    }

    fn set_scale(&mut self, config: &Config, scale: i32) {
        self.height = config.bar.width as i32 * scale;
        self.gaps = config.workspaces.gaps.map(|v| v as i32 * scale);
    }
}
