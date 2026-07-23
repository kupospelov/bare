use crate::blocks::Blocks;
use crate::config::Config;
use crate::init::Init;
use crate::render::Renderer;
use crate::wayland::output::Output;
use crate::wayland::pointer::Pointer;
use crate::{debug, warning};
use calloop::signals::{Signal, Signals};
use calloop::timer::{TimeoutAction, Timer};
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
use wayland_protocols::wp::cursor_shape::v1::client::{
    wp_cursor_shape_device_v1, wp_cursor_shape_manager_v1,
};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

#[derive(Clone)]
pub struct Workspace {
    pub handle: ext_workspace_handle_v1::ExtWorkspaceHandleV1,
    pub name: String,
    pub active: bool,
    pub urgent: bool,
}

impl PartialEq for Workspace {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.active == other.active && self.urgent == other.urgent
    }
}

#[derive(Default)]
pub struct Group {
    pub workspaces: Vec<ObjectId>,
    pub outputs: Vec<ObjectId>,
}

pub struct State {
    pub config: Config,
    pub qh: QueueHandle<State>,
    pub compositor: wl_compositor::WlCompositor,
    pub shm: wl_shm::WlShm,
    pub layer_shell: zwlr_layer_shell_v1::ZwlrLayerShellV1,
    pub outputs: HashMap<ObjectId, Output>, // output id -> Output
    pub groups: HashMap<ObjectId, Group>,   // group id -> Group
    pub seat: Option<wl_seat::WlSeat>,
    pub cursor_shape_manager: Option<wp_cursor_shape_manager_v1::WpCursorShapeManagerV1>,
    pub pointer: Option<Pointer>,
    pub workspace_manager: ext_workspace_manager_v1::ExtWorkspaceManagerV1,
    pub workspace_handles: HashMap<ObjectId, Workspace>,
    pub renderer: Renderer,
    pub blocks: Blocks,
}

impl State {
    pub fn new(config: Config, qh: QueueHandle<State>, init: Init) -> Self {
        let renderer = Renderer::new(&config);
        let blocks = Blocks::new(&config);
        Self {
            config,
            qh,
            compositor: init.compositor.expect("Missing wl_compositor"),
            shm: init.shm.expect("Missing wl_shm"),
            layer_shell: init.layer_shell.expect("Missing zwlr_layer_shell_v1"),
            workspace_manager: init.workspace_manager.expect("Missing ext-workspace-v1"),
            outputs: HashMap::new(),
            groups: HashMap::new(),
            seat: None,
            cursor_shape_manager: None,
            pointer: None,
            workspace_handles: HashMap::new(),
            renderer,
            blocks,
        }
    }

    fn create_output(&mut self, name: u32, output: wl_output::WlOutput) {
        let layer_shell = &self.layer_shell;
        let compositor = &self.compositor;
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
            self.renderer.font_size,
            output,
            surface,
            layer_surface,
        );
        output.update_layout(
            &self.blocks,
            &self.renderer.rasterizer,
            self.config.bar.separator,
        );
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

    pub(crate) fn mark_all_outputs_block_dirty(&mut self, block_idx: usize) {
        for output in self.outputs.values_mut() {
            let range = output.block_range(block_idx);
            let output_id = output.output.id();

            debug!(
                "Output {}: mark block {} dirty: {}",
                output_id, block_idx, range
            );
            output.mark_dirty(range);
        }
    }

    pub fn callback(&mut self, conn: &Connection) {
        let outputs_to_render: Vec<ObjectId> = self
            .outputs
            .iter()
            .filter(|(_, o)| o.configured)
            .map(|(id, _)| id.clone())
            .collect();
        if !outputs_to_render.is_empty() {
            let shm = self.shm.clone();
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

    pub fn register_event_sources(&mut self, handle: &calloop::LoopHandle<'_, State>) {
        // Event-based updates.
        self.blocks.time.register_events(handle);
        self.blocks.battery.register_events(handle);
        self.blocks.volume.register_events(handle);
        self.blocks.wireless.register_events(handle);

        // Interval-based updates.
        handle
            .insert_source(Timer::immediate(), move |_, _, state| {
                state.update_interval_blocks();
                TimeoutAction::ToDuration(state.config.bar.interval)
            })
            .expect("Failed to insert interval timer");

        // Signal-based updates.
        let signal_handle = handle.clone();
        handle
            .insert_source(
                Signals::new(&[Signal::SIGUSR1]).expect("Failed to create signal source"),
                move |event, _, state| {
                    debug!("Received {:?}", event.signal());
                    if event.signal() == Signal::SIGUSR1 {
                        state.update_event_blocks(&signal_handle);
                        state.update_interval_blocks();
                    }
                },
            )
            .expect("Failed to insert signal source");
    }

    fn update_event_blocks(&mut self, handle: &calloop::LoopHandle<'_, State>) {
        self.blocks.time.register_events(handle);
        self.blocks.volume.register_events(handle);
    }

    fn update_interval_blocks(&mut self) {
        let mut dirty = Vec::new();
        self.blocks.cpu.update(&mut dirty);
        self.blocks.wireless.update(&mut dirty);
        self.blocks.battery.update(&mut dirty);
        for id in dirty {
            self.mark_all_outputs_block_dirty(id);
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
                "wl_output" => {
                    let output = registry.bind::<wl_output::WlOutput, _, _>(name, 3, qh, ());
                    state.create_output(name, output);
                }
                "wl_seat" => {
                    state.seat = Some(registry.bind::<wl_seat::WlSeat, _, _>(name, 3, qh, ()));
                }
                "wp_cursor_shape_manager_v1" => {
                    state.cursor_shape_manager = Some(
                        registry.bind::<wp_cursor_shape_manager_v1::WpCursorShapeManagerV1, _, _>(
                            name,
                            1,
                            qh,
                            (),
                        ),
                    );
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
                    let handle = seat.get_pointer(qh, ());
                    let cursor_shape_device = if let Some(m) = state.cursor_shape_manager.as_ref() {
                        Some(m.get_pointer(&handle, qh, ()))
                    } else {
                        debug!("No cursor shape manager");
                        None
                    };
                    state.pointer = Some(Pointer {
                        handle,
                        cursor_shape_device,
                        y: 0,
                        scroll_accumulator: 0.0,
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
            wl_pointer::Event::Enter {
                serial,
                surface,
                surface_y,
                ..
            } => {
                if let Some(p) = state.pointer.as_mut() {
                    p.surface = Some(surface.id());
                    p.y = surface_y as i32;
                    if let Some(d) = p.cursor_shape_device.as_ref() {
                        d.set_shape(serial, wp_cursor_shape_device_v1::Shape::Default);
                    }
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
                let Some(pointer) = state.pointer.as_ref() else {
                    warning!("No pointer found for mouse button pressed event");
                    return;
                };

                debug!("Mouse button press at y = {}", pointer.y);
                if let Some(output) = pointer.get_output(&state.outputs) {
                    let physical_y = pointer.y * output.scale;
                    if let Some(handle) = output.workspace_group.handle_at(physical_y) {
                        debug!("Activating handle at y = {}", physical_y);
                        handle.activate();
                        state.workspace_manager.commit();
                        let _ = conn.flush();
                    }
                }
            }
            wl_pointer::Event::Axis {
                axis: WEnum::Value(wl_pointer::Axis::VerticalScroll),
                value,
                ..
            } => {
                let Some(pointer) = state.pointer.as_mut() else {
                    warning!("No pointer found for vertical scroll event");
                    return;
                };

                let steps = pointer.scroll(value);
                if steps == 0 {
                    return;
                }

                if let Some(output) = pointer.get_output(&state.outputs)
                    && let Some(handle) = output.workspace_group.handle_scroll(steps)
                {
                    debug!("Switching workspace on axis event {}", steps);
                    handle.activate();
                    state.workspace_manager.commit();
                    let _ = conn.flush();
                }
            }
            wl_pointer::Event::AxisStop {
                axis: WEnum::Value(wl_pointer::Axis::VerticalScroll),
                ..
            } => {
                let Some(pointer) = state.pointer.as_mut() else {
                    return;
                };

                debug!("Resetting scroll accumulator");
                pointer.scroll_accumulator = 0.0;
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
                    o.workspace_group
                        .set_scale(&state.config, state.renderer.font_size, factor);
                    o.update_layout(
                        &state.blocks,
                        &state.renderer.rasterizer,
                        state.config.bar.separator,
                    );
                }
            }
            wl_output::Event::Done => {
                if let Some(o) = state.outputs.get_mut(&output.id()) {
                    debug!("Output {}: done", id);

                    o.mark_full_dirty();
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
    wayland_client::protocol::wl_shm_pool::WlShmPool,
    wp_cursor_shape_manager_v1::WpCursorShapeManagerV1,
    wp_cursor_shape_device_v1::WpCursorShapeDeviceV1
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
            let output = state.outputs.get_mut(&output_id).unwrap();
            if let Some(range) = output.workspace_group.update(ws) {
                debug!("Output {}: mark workspaces dirty: {}", output_id, range);
                output.mark_dirty(range);
            }
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
                    output.mark_full_dirty();
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

            rebuild_workspaces(state);
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
