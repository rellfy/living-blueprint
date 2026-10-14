use crate::app::App;
use std::env;
use std::path::PathBuf;
use winit::event_loop::EventLoop;

pub mod agent;
pub mod analysis;
pub mod app;
pub mod resources;
pub mod state;
pub mod systems;

pub use crate::state::State;

fn main() {
    env_logger::init();
    let repository_root = read_repository_root();
    let event_loop = EventLoop::new().expect("failed to create event loop");
    let mut app = App::new(repository_root);
    event_loop.run_app(&mut app).expect("event loop failed");
}

fn read_repository_root() -> PathBuf {
    let path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().expect("failed to read current directory"));
    path.canonicalize().unwrap_or(path)
}
