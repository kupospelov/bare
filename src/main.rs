mod blocks;
mod color;
mod config;
mod damage;
mod font;
mod log;
mod raster;
mod render;
mod state;
mod wayland;

use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;
use state::State;
use wayland_client::Connection;

const LOG_LEVEL: log::Level = log::Level::Debug;

fn main() {
    let conn = Connection::connect_to_env().expect("Failed to connect to Wayland compositor");
    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();
    let _ = conn.display().get_registry(&qh, ());

    let mut event_loop: EventLoop<State> =
        EventLoop::try_new().expect("Failed to create event loop");
    let handle = event_loop.handle();

    let mut state = State::new(config::Config::load(), qh.clone());
    event_queue.roundtrip(&mut state).unwrap();
    let _workspace_manager = state
        .workspace_manager
        .as_ref()
        .expect("Compositor doesn't support ext-workspace-v1");

    WaylandSource::new(conn.clone(), event_queue)
        .insert(handle.clone())
        .expect("Failed to insert Wayland source");
    state.register_event_sources(&handle);
    event_loop
        .run(None, &mut state, |state| state.callback(&conn))
        .expect("Failed to run event loop");
}
