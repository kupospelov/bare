mod blocks;
mod color;
mod config;
mod damage;
mod font;
mod init;
mod log;
mod raster;
mod render;
mod state;
mod wayland;

use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;
use clap::Parser;
use init::Init;
use state::State;
use wayland_client::Connection;

#[derive(Parser)]
struct Arguments {
    #[arg(short = 'd', long = "debug")]
    debug: bool,
}

fn main() {
    let args = Arguments::parse();
    if args.debug {
        log::set(log::Level::Debug);
    } else {
        log::set(log::Level::Warning);
    }

    let conn = Connection::connect_to_env().expect("Failed to connect to Wayland compositor");

    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();

    // Bind required globals.
    let init = {
        let mut q = conn.new_event_queue();
        let _ = conn.display().get_registry(&q.handle(), ());
        let mut init = Init::new(qh.clone());
        q.roundtrip(&mut init).unwrap();
        init
    };

    // Move globals to State and create outputs.
    let mut state = State::new(config::Config::load(), qh.clone(), init);
    let _ = conn.display().get_registry(&qh, ());
    event_queue.roundtrip(&mut state).unwrap();

    let mut event_loop: EventLoop<State> =
        EventLoop::try_new().expect("Failed to create event loop");
    let handle = event_loop.handle();
    WaylandSource::new(conn.clone(), event_queue)
        .insert(handle.clone())
        .expect("Failed to insert Wayland source");
    state.register_event_sources(&handle);
    event_loop
        .run(None, &mut state, |state| state.callback(&conn))
        .expect("Failed to run event loop");
}
