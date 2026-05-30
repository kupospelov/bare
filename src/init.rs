use crate::state::State;
use wayland_client::{
    Connection, Dispatch, QueueHandle,
    protocol::{wl_compositor, wl_registry, wl_shm},
};
use wayland_protocols::ext::workspace::v1::client::ext_workspace_manager_v1;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1;

/// Registry dispatch target to resolve required globals for State.
pub struct Init {
    qh: QueueHandle<State>,
    pub compositor: Option<wl_compositor::WlCompositor>,
    pub shm: Option<wl_shm::WlShm>,
    pub layer_shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    pub workspace_manager: Option<ext_workspace_manager_v1::ExtWorkspaceManagerV1>,
}

impl Init {
    pub fn new(qh: QueueHandle<State>) -> Self {
        Self {
            qh,
            compositor: None,
            shm: None,
            layer_shell: None,
            workspace_manager: None,
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for Init {
    fn event(
        init: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name, interface, ..
        } = event
        {
            let qh = &init.qh;
            match &interface[..] {
                "wl_compositor" => {
                    init.compositor = Some(registry.bind(name, 4, qh, ()));
                }
                "wl_shm" => {
                    init.shm = Some(registry.bind(name, 1, qh, ()));
                }
                "zwlr_layer_shell_v1" => {
                    init.layer_shell = Some(registry.bind(name, 1, qh, ()));
                }
                "ext_workspace_manager_v1" => {
                    init.workspace_manager = Some(registry.bind(name, 1, qh, ()));
                }
                _ => {}
            }
        }
    }
}
