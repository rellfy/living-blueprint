use std::sync::Arc;
use winit::dpi::LogicalSize;
use winit::event_loop::EventLoop;
use winit::window::Window;

const WINDOW_TITLE: &str = "Living Blueprint";
const WINDOW_WIDTH: u32 = 1280;
const WINDOW_HEIGHT: u32 = 720;

pub struct WindowResource {
    pub window: Arc<Window>,
    pub event_loop: EventLoop<()>,
}

impl WindowResource {
    pub fn init() -> Self {
        let event_loop = EventLoop::new().unwrap();
        let attributes = Window::default_attributes()
            .with_title(WINDOW_TITLE)
            .with_inner_size(LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT));
        let window = Arc::new(event_loop.create_window(attributes).unwrap());
        Self { window, event_loop }
    }

    pub fn take(self) -> (Arc<Window>, EventLoop<()>) {
        (self.window, self.event_loop)
    }
}
