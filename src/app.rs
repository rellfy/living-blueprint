use crate::State;
use std::path::PathBuf;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::{Window, WindowId};

const WINDOW_TITLE: &str = "Living Blueprint";
const WINDOW_WIDTH: u32 = 1440;
const WINDOW_HEIGHT: u32 = 900;

pub struct App {
    repository_root: PathBuf,
    state: Option<State>,
}

impl App {
    pub fn new(repository_root: PathBuf) -> Self {
        Self {
            repository_root,
            state: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        event_loop.set_control_flow(ControlFlow::Poll);
        let window = Arc::new(create_window(event_loop));
        self.state = Some(State::new(window, self.repository_root.clone()));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        if matches!(event, WindowEvent::CloseRequested) {
            event_loop.exit();
            return;
        }
        state.handle_window_event(event);
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let Some(state) = self.state.as_ref() else {
            return;
        };
        state.window.request_redraw();
    }
}

fn create_window(event_loop: &ActiveEventLoop) -> Window {
    let attributes = Window::default_attributes()
        .with_title(WINDOW_TITLE)
        .with_inner_size(LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT));
    event_loop
        .create_window(attributes)
        .expect("failed to create window")
}
