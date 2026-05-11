use super::Block;
use crate::config::WorkspaceConfig;
use crate::render::{
    self, COLOR_ACTIVE, COLOR_INACTIVE, COLOR_URGENT, COLOR_WORKSPACE_ACTIVE_BG,
    COLOR_WORKSPACE_ACTIVE_BR, Renderer,
};
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

            let state = if ws.active {
                &self.config.active
            } else if ws.urgent {
                &self.config.urgent
            } else {
                &self.config.inactive
            };
            let outer = render::Region {
                x: state.gaps[3],
                y: y + state.gaps[0],
                w: (map.width as i32 - state.gaps[3] - state.gaps[1]).max(0) as u32,
                h: (self.height - state.gaps[0] - state.gaps[2]).max(0) as u32,
            };
            let inner = render::Region {
                x: outer.x + state.borders[3],
                y: outer.y + state.borders[0],
                w: (outer.w as i32 - state.borders[3] - state.borders[1]).max(0) as u32,
                h: (outer.h as i32 - state.borders[0] - state.borders[2]).max(0) as u32,
            };
            if outer.w > 0 && outer.h > 0 {
                render_border(
                    renderer,
                    map,
                    outer,
                    state.borders,
                    COLOR_WORKSPACE_ACTIVE_BR,
                );
            }

            if inner.w > 0 && inner.h > 0 {
                if ws_bg_color != bg_color {
                    renderer.fill_rect(map, inner, ws_bg_color);
                }
                renderer.render_text(map, inner, &ws.name, text_color, ws_bg_color, font_size);
            }

            y += self.height;
        }
    }

    fn set_scale(&mut self, config: &crate::config::Config, scale: i32) {
        self.height = config.bar.width as i32 * scale;
        self.config = config.workspace.scaled(scale);
    }
}

fn render_border(
    renderer: &Renderer,
    map: &mut render::Map<'_>,
    outer: render::Region,
    borders: [i32; 4],
    color: [u8; 4],
) {
    // top
    if borders[0] > 0 {
        renderer.fill_rect(
            map,
            render::Region {
                x: outer.x,
                y: outer.y,
                w: outer.w,
                h: borders[0] as u32,
            },
            color,
        );
    }

    // right
    if borders[1] > 0 {
        renderer.fill_rect(
            map,
            render::Region {
                x: outer.x + outer.w as i32 - borders[1],
                y: outer.y,
                w: borders[1] as u32,
                h: outer.h,
            },
            color,
        );
    }

    // bottom
    if borders[2] > 0 {
        renderer.fill_rect(
            map,
            render::Region {
                x: outer.x,
                y: outer.y + outer.h as i32 - borders[2],
                w: outer.w,
                h: borders[2] as u32,
            },
            color,
        );
    }

    // left
    if borders[3] > 0 {
        renderer.fill_rect(
            map,
            render::Region {
                x: outer.x,
                y: outer.y,
                w: borders[3] as u32,
                h: outer.h,
            },
            color,
        );
    }
}
