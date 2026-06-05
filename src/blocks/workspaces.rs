use crate::config::WorkspaceConfig;
use crate::render::{self, Range, Renderer};
use crate::state::Workspace;
use crate::warning;
use wayland_protocols::ext::workspace::v1::client::ext_workspace_handle_v1;

pub struct Workspaces {
    pub items: Vec<Workspace>,
    pub height: i32,
    config: WorkspaceConfig,
}

impl Workspaces {
    pub fn new(config: &WorkspaceConfig, font_size: u32) -> Self {
        let c = config.scaled(1);
        let height = c.block.height(font_size as i32);
        Self {
            items: Vec::new(),
            height,
            config: c,
        }
    }

    pub fn height(&self) -> i32 {
        self.items.len() as i32 * self.height
    }

    pub fn handle_scroll(
        &self,
        steps: i32,
    ) -> Option<&ext_workspace_handle_v1::ExtWorkspaceHandleV1> {
        let Some(active) = self.items.iter().position(|ws| ws.active) else {
            warning!("No active workspace!");
            return None;
        };

        let next = (active as i32 + steps).clamp(0, self.items.len() as i32 - 1) as usize;
        if next == active {
            return None;
        }

        Some(&self.items[next].handle)
    }

    pub fn handle_at(&self, y: i32) -> Option<&ext_workspace_handle_v1::ExtWorkspaceHandleV1> {
        if self.height == 0 {
            return None;
        }
        let index = y / self.height;
        if index >= 0 && (index as usize) < self.items.len() {
            Some(&self.items[index as usize].handle)
        } else {
            None
        }
    }

    pub fn render(
        &mut self,
        renderer: &mut Renderer,
        map: &mut render::Map<'_>,
        region: render::Region,
        font_size: u32,
        dirty: Range,
    ) {
        let mut y = region.y;
        for ws in &self.items {
            let range = Range::new(y, y + self.height);
            if !dirty.overlaps(range) {
                y += self.height;
                continue;
            }

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

    pub fn update(&mut self, items: Vec<Workspace>) -> Option<Range> {
        let dirty = crate::damage::arrays(&self.items, &items, self.height);
        self.items = items;
        dirty
    }

    pub fn set_scale(&mut self, config: &crate::config::Config, font_size: u32, scale: i32) {
        let c = config.workspace.scaled(scale);
        self.height = c.block.height(font_size as i32 * scale);
        self.config = c;
    }
}
