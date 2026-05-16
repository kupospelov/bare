use crate::blocks;
use crate::blocks::Block;
use crate::config::Config;
use crate::font;
use crate::raster;
use crate::render::Renderer;
use crate::wayland::output::Output;
use crate::wayland::pointer::Pointer;
use crate::{debug, warning};
use calloop::timer::TimeoutAction;
use std::collections::HashMap;
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle, WEnum,
    backend::ObjectId,
    event_created_child,
    protocol::{
        wl_buffer, wl_compositor, wl_output, wl_pointer, wl_registry, wl_seat, wl_shm, wl_surface,
    },
};
use wayland_protocols::ext::workspace::v1::client::{
    ext_workspace_group_handle_v1, ext_workspace_handle_v1, ext_workspace_manager_v1,
};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

#[derive(Clone)]
pub struct Workspace {
    pub handle: ext_workspace_handle_v1::ExtWorkspaceHandleV1,
    pub name: String,
    pub active: bool,
    pub urgent: bool,
}

#[derive(Default)]
pub struct Group {
    pub workspaces: Vec<ObjectId>,
    pub outputs: Vec<ObjectId>,
}

pub struct State {
    pub config: Config,
    pub qh: QueueHandle<State>,
    pub compositor: Option<wl_compositor::WlCompositor>,
    pub shm: Option<wl_shm::WlShm>,
    pub layer_shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    pub outputs: HashMap<ObjectId, Output>, // output id -> Output
    pub groups: HashMap<ObjectId, Group>,   // group id -> Group
    pub seat: Option<wl_seat::WlSeat>,
    pub pointer: Option<Pointer>,
    pub workspace_manager: Option<ext_workspace_manager_v1::ExtWorkspaceManagerV1>,
    pub workspace_handles: HashMap<ObjectId, Workspace>,
    pub renderer: Renderer,
    pub blocks: Vec<Box<dyn Block>>,
}

fn create_blocks(config: &Config) -> Vec<Box<dyn Block>> {
    let mut blocks: Vec<Box<dyn Block>> = Vec::new();
    blocks.push(Box::new(blocks::time::Time::new(&config.time)));
    blocks.push(Box::new(blocks::battery::Battery::new(&config.battery)));
    if let Ok(volume) = blocks::volume::Volume::new(&config.volume) {
        blocks.push(Box::new(volume));
    }

    blocks
}

impl State {
    pub fn new(config: Config, qh: QueueHandle<State>) -> Self {
        let (font, font_size) = font::load(&config.bar.font);
        let blocks = create_blocks(&config);
        Self {
            config,
            qh,
            compositor: None,
            shm: None,
            layer_shell: None,
            outputs: HashMap::new(),
            groups: HashMap::new(),
            seat: None,
            pointer: None,
            workspace_handles: HashMap::new(),
            workspace_manager: None,
            renderer: Renderer::new(raster::Rasterizer::new(font), font_size),
            blocks,
        }
    }

    fn create_output(&mut self, name: u32, output: wl_output::WlOutput) {
        let layer_shell = self.layer_shell.as_ref().unwrap();
        let compositor = self.compositor.as_ref().unwrap();
        let id = output.id();

        let surface = compositor.create_surface(&self.qh, ());
        let layer_surface = layer_shell.get_layer_surface(
            &surface,
            Some(&output),
            zwlr_layer_shell_v1::Layer::Top,
            "bare".to_string(),
            &self.qh,
            (),
        );
        layer_surface.set_anchor(
            zwlr_layer_surface_v1::Anchor::Left
                | zwlr_layer_surface_v1::Anchor::Top
                | zwlr_layer_surface_v1::Anchor::Bottom,
        );
        layer_surface.set_size(self.config.bar.width, 0);
        layer_surface.set_exclusive_zone(self.config.bar.width as i32);
        surface.commit();

        let mut output = Output::new(
            name,
            self.config.bar.width,
            &self.config.workspace,
            output,
            surface,
            layer_surface,
        );
        output.update_layout(&self.blocks, self.renderer.font_size);
        self.outputs.insert(id, output);
    }

    fn remove_output(&mut self, name: u32) {
        let id = self
            .outputs
            .iter()
            .find(|(_, o)| o.name == name)
            .map(|(id, _)| id.clone());
        let Some(id) = id else {
            return;
        };
        let output = self.outputs.remove(&id).unwrap();
        debug!("Output {}: removed", id);

        if let Some(group_id) = output.group.as_ref()
            && let Some(group) = self.groups.get_mut(group_id)
        {
            group.outputs.retain(|o| *o != id);
        }
    }

    fn mark_all_outputs_dirty(&mut self) {
        for output in self.outputs.values_mut() {
            output.render = true;
        }
    }

    pub fn callback(&mut self, conn: &Connection) {
        let outputs_to_render: Vec<ObjectId> = self
            .outputs
            .iter()
            .filter(|(_, o)| o.configured && o.render)
            .map(|(id, _)| id.clone())
            .collect();
        if !outputs_to_render.is_empty() {
            let shm = self.shm.as_ref().unwrap().clone();
            let qh = self.qh.clone();
            for id in outputs_to_render {
                if let Some(output) = self.outputs.get_mut(&id) {
                    self.renderer
                        .render(&id, output, &shm, &qh, &mut self.blocks);
                }
            }
        }
        let _ = conn.flush();
    }

    pub fn register_event_sources(&self, handle: &calloop::LoopHandle<'_, State>) {
        for i in 0..self.blocks.len() {
            if let Some(fd) = self.blocks[i].fd() {
                handle
                    .insert_source(fd, move |_readiness, _fd, state| {
                        if state.blocks[i].on_fd() {
                            state.mark_all_outputs_dirty();
                        }
                        Ok(calloop::PostAction::Continue)
                    })
                    .expect("Failed to insert block fd source");
            }

            let timer = match self.blocks[i].reschedule() {
                TimeoutAction::ToInstant(i) => calloop::timer::Timer::from_deadline(i),
                TimeoutAction::ToDuration(d) => calloop::timer::Timer::from_duration(d),
                TimeoutAction::Drop => continue,
            };
            handle
                .insert_source(timer, move |_, _, state| {
                    if state.blocks[i].update() {
                        state.mark_all_outputs_dirty();
                    }
                    state.blocks[i].reschedule()
                })
                .expect("Failed to insert block timer");
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name, interface, ..
            } => match &interface[..] {
                "wl_compositor" => {
                    state.compositor =
                        Some(registry.bind::<wl_compositor::WlCompositor, _, _>(name, 3, qh, ()));
                }
                "wl_shm" => {
                    state.shm = Some(registry.bind::<wl_shm::WlShm, _, _>(name, 1, qh, ()));
                }
                "zwlr_layer_shell_v1" => {
                    state.layer_shell = Some(
                        registry.bind::<zwlr_layer_shell_v1::ZwlrLayerShellV1, _, _>(
                            name,
                            1,
                            qh,
                            (),
                        ),
                    );
                }
                "ext_workspace_manager_v1" => {
                    state.workspace_manager = Some(
                        registry.bind::<ext_workspace_manager_v1::ExtWorkspaceManagerV1, _, _>(
                            name,
                            1,
                            qh,
                            (),
                        ),
                    );
                }
                "wl_output" => {
                    let output = registry.bind::<wl_output::WlOutput, _, _>(name, 3, qh, ());
                    state.create_output(name, output);
                }
                "wl_seat" => {
                    state.seat = Some(registry.bind::<wl_seat::WlSeat, _, _>(name, 3, qh, ()));
                }
                _ => {}
            },
            wl_registry::Event::GlobalRemove { name } => {
                state.remove_output(name);
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities { capabilities } = event {
            let caps = match capabilities {
                WEnum::Value(c) => c,
                WEnum::Unknown(u) => wl_seat::Capability::from_bits_truncate(u),
            };
            let has_pointer = caps.contains(wl_seat::Capability::Pointer);
            match (has_pointer, state.pointer.as_ref()) {
                (true, None) => {
                    state.pointer = Some(Pointer {
                        handle: seat.get_pointer(qh, ()),
                        y: 0,
                        surface: None,
                    });
                }
                (false, Some(_)) => {
                    state.pointer = None;
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for State {
    fn event(
        state: &mut Self,
        _: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        conn: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            // TODO: Handle cursor shape.
            wl_pointer::Event::Enter {
                surface, surface_y, ..
            } => {
                if let Some(p) = state.pointer.as_mut() {
                    p.surface = Some(surface.id());
                    p.y = surface_y as i32;
                }
            }
            wl_pointer::Event::Leave { .. } => {
                if let Some(p) = state.pointer.as_mut() {
                    p.surface = None;
                }
            }
            wl_pointer::Event::Motion { surface_y, .. } => {
                if let Some(p) = state.pointer.as_mut() {
                    p.y = surface_y as i32;
                }
            }
            wl_pointer::Event::Button {
                state: WEnum::Value(wl_pointer::ButtonState::Pressed),
                ..
            } => {
                let pointer = if let Some(pointer) = state.pointer.as_ref() {
                    pointer
                } else {
                    warning!("No pointer found for mouse button pressed event");
                    return;
                };

                let surface_id = if let Some(surface) = pointer.surface.as_ref() {
                    surface
                } else {
                    warning!("No surface found for mouse button pressed event");
                    return;
                };

                debug!("Mouse button press at y = {}", pointer.y);
                let mut target_output_id = None;
                for (output_id, output) in &state.outputs {
                    if output.surface.id() == *surface_id {
                        target_output_id = Some(output_id.clone());
                        break;
                    }
                }

                if let Some(output_id) = target_output_id
                    && let Some(output) = state.outputs.get(&output_id)
                {
                    let physical_y = pointer.y * output.scale;
                    if let Some(handle) = output.workspace_group.handle_at(physical_y) {
                        handle.activate();
                        debug!("Activated handle at y = {}", physical_y);

                        if let Some(mgr) = state.workspace_manager.as_ref() {
                            mgr.commit();
                        }
                        let _ = conn.flush();
                    }
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_output::WlOutput, ()> for State {
    fn event(
        state: &mut Self,
        output: &wl_output::WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let id = output.id();

        match event {
            wl_output::Event::Scale { factor } => {
                if let Some(o) = state.outputs.get_mut(&output.id()) {
                    debug!("Output {}: scale change: {}", id, factor);

                    o.scale = factor;
                    o.workspace_group.set_scale(&state.config, factor);
                    o.update_layout(&state.blocks, state.renderer.font_size);
                }
            }
            wl_output::Event::Done => {
                if let Some(o) = state.outputs.get_mut(&output.id()) {
                    debug!("Output {}: done", id);

                    o.render = true;
                }
            }
            _ => {}
        }
    }
}

macro_rules! impl_empty_dispatch {
    ($($t:ty),*) => {
        $(
            impl Dispatch<$t, ()> for State {
                fn event(_: &mut Self, _: &$t, _: <$t as wayland_client::Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
            }
        )*
    };
}

impl_empty_dispatch!(
    wl_compositor::WlCompositor,
    wl_shm::WlShm,
    wl_surface::WlSurface,
    zwlr_layer_shell_v1::ZwlrLayerShellV1,
    wayland_client::protocol::wl_shm_pool::WlShmPool
);

impl Dispatch<wl_buffer::WlBuffer, (ObjectId, usize)> for State {
    fn event(
        state: &mut Self,
        _: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        (output_id, idx): &(ObjectId, usize),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event
            && let Some(output) = state.outputs.get_mut(output_id)
        {
            debug!("Output {}: buffer {} released", output_id, *idx);

            if let Some(buf) = output.buffer.as_mut() {
                buf.released[*idx] = true;
            }
        }
    }
}

impl Dispatch<ext_workspace_group_handle_v1::ExtWorkspaceGroupHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        handle: &ext_workspace_group_handle_v1::ExtWorkspaceGroupHandleV1,
        event: ext_workspace_group_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let id = handle.id();

        use ext_workspace_group_handle_v1::Event;
        match event {
            Event::OutputEnter { output } => {
                let output_id = output.id();
                debug!("Group {} entered output {}", id, output_id);

                let group = state.groups.entry(id.clone()).or_default();
                group.outputs.push(output_id.clone());
                if let Some(output) = state.outputs.get_mut(&output_id) {
                    output.group = Some(id.clone());
                }
            }
            Event::OutputLeave { output } => {
                let output_id = output.id();
                debug!("Group {} left output {}", id, output_id);

                if let Some(group) = state.groups.get_mut(&id) {
                    group.outputs.retain(|id| *id != output_id);
                }
                if let Some(output) = state.outputs.get_mut(&output_id) {
                    output.group = None;
                }
            }
            Event::WorkspaceEnter { workspace } => {
                let workspace_id = workspace.id();
                debug!("Workspace {}: entered {}", workspace_id, id);

                state
                    .groups
                    .entry(id.clone())
                    .or_default()
                    .workspaces
                    .push(workspace_id);
            }
            Event::WorkspaceLeave { workspace } => {
                let workspace_id = workspace.id();
                debug!("Workspace {}: left {}", workspace_id, id);

                if let Some(group) = state.groups.get_mut(&id) {
                    group.workspaces.retain(|id| *id != workspace_id);
                }
            }
            Event::Removed => {
                debug!("Group {}: removed", id);

                state.groups.remove(&id);
                for output in state.outputs.values_mut() {
                    if output.group.as_ref() == Some(&id) {
                        output.group = None;
                    }
                }
            }
            _ => {}
        }
    }
}

fn rebuild_workspaces(state: &mut State) {
    let output_ids: Vec<ObjectId> = state.outputs.keys().cloned().collect();
    for output_id in output_ids {
        let ws = {
            let output = &state.outputs[&output_id];
            output
                .group
                .as_ref()
                .and_then(|gid| state.groups.get(gid))
                .map(|group| {
                    let mut ws: Vec<Workspace> = group
                        .workspaces
                        .iter()
                        .filter_map(|ws_id| state.workspace_handles.get(ws_id).cloned())
                        .collect();
                    ws.sort_by(|a, b| a.name.cmp(&b.name));
                    ws
                })
        };
        if let Some(ws) = ws {
            state
                .outputs
                .get_mut(&output_id)
                .unwrap()
                .workspace_group
                .items = ws;
        }
    }
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for State {
    fn event(
        state: &mut Self,
        layer_surface: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_layer_surface_v1::Event::Configure {
            serial,
            width,
            height,
        } = event
        {
            let id = layer_surface.id();
            debug!("Layer surface {} configured", id);

            layer_surface.ack_configure(serial);
            for output in state.outputs.values_mut() {
                if output.layer_surface.id() == id {
                    output.width = width;
                    output.height = height;
                    output.configured = true;
                    output.surface.commit();
                    output.render = true;
                    break;
                }
            }
        }
    }
}

impl Dispatch<ext_workspace_manager_v1::ExtWorkspaceManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        handle: &ext_workspace_manager_v1::ExtWorkspaceManagerV1,
        event: ext_workspace_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_workspace_manager_v1::Event::Done = event {
            let id = handle.id();
            debug!("Workspace manager {} done", id);

            // TODO: Only mark affected outputs.
            rebuild_workspaces(state);
            state.mark_all_outputs_dirty();
        }
    }

    event_created_child!(State, ext_workspace_manager_v1::ExtWorkspaceManagerV1, [
        ext_workspace_manager_v1::EVT_WORKSPACE_GROUP_OPCODE => (wayland_protocols::ext::workspace::v1::client::ext_workspace_group_handle_v1::ExtWorkspaceGroupHandleV1, ()),
        ext_workspace_manager_v1::EVT_WORKSPACE_OPCODE => (ext_workspace_handle_v1::ExtWorkspaceHandleV1, ()),
    ]);
}

impl Dispatch<ext_workspace_handle_v1::ExtWorkspaceHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        handle: &ext_workspace_handle_v1::ExtWorkspaceHandleV1,
        event: ext_workspace_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let id = handle.id();
        state
            .workspace_handles
            .entry(id.clone())
            .or_insert(Workspace {
                handle: handle.clone(),
                name: "?".to_string(),
                active: false,
                urgent: false,
            });
        match event {
            ext_workspace_handle_v1::Event::Name { name } => {
                debug!("Workspace {}: name = {}", id, name);

                state.workspace_handles.get_mut(&id).unwrap().name = name;
            }
            ext_workspace_handle_v1::Event::State { state: ws_state } => {
                let ws = state.workspace_handles.get_mut(&id).unwrap();
                if let WEnum::Value(flags) = ws_state {
                    ws.active = flags.contains(ext_workspace_handle_v1::State::Active);
                    ws.urgent = flags.contains(ext_workspace_handle_v1::State::Urgent);
                } else {
                    ws.active = false;
                    ws.urgent = false;
                }

                debug!(
                    "Workspace {}: active = {}, urgent = {}",
                    id, ws.active, ws.urgent
                );
            }
            ext_workspace_handle_v1::Event::Removed => {
                debug!("Workspace {}: removed", id);

                state.workspace_handles.remove(&id);
            }
            _ => {}
        }
    }
}
