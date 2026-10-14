use crate::resources::camera::Camera;
use crate::resources::wgpu::WgpuResource;
use crate::systems::render::render;
use crate::systems::render::state::RenderState;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::window::Window;

pub struct State {
    pub window: Arc<Window>,
    pub wgpu_resource: WgpuResource,
    pub render_state: RenderState,
    pub camera: Camera,
    pub repository_root: PathBuf,
    pub last_frame_time: Instant,
    pub delta_time: f64,
}

impl State {
    pub fn new(window: Arc<Window>, repository_root: PathBuf) -> Self {
        let wgpu_resource = WgpuResource::init(window.clone());
        let render_state = RenderState::init(&wgpu_resource);
        Self {
            window,
            wgpu_resource,
            render_state,
            camera: Camera::new(),
            repository_root,
            last_frame_time: Instant::now(),
            delta_time: 0.0,
        }
    }

    pub fn handle_window_event(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::Resized(size) => self.handle_resize(size),
            WindowEvent::RedrawRequested => self.run_frame(),
            _ => {}
        }
    }

    fn run_frame(&mut self) {
        self.on_awake();
        render(self);
    }

    fn on_awake(&mut self) {
        let now = Instant::now();
        self.delta_time = now.duration_since(self.last_frame_time).as_secs_f64();
        self.last_frame_time = now;
    }

    fn handle_resize(&mut self, size: PhysicalSize<u32>) {
        self.wgpu_resource.resize(size.width, size.height);
        self.render_state
            .resize(&self.wgpu_resource.device, size.width, size.height);
    }
}
