use crate::mouse::MouseScrollDeltaExt;
use crate::resources::camera::Camera;
use crate::resources::wgpu::WgpuResource;
use crate::resources::window::WindowResource;
use crate::systems::render::render;
use std::sync::Arc;
use std::time::Instant;
use winit::dpi::PhysicalSize;
use winit::event::{Event, MouseScrollDelta, WindowEvent};
use winit::event_loop::ControlFlow;
use winit::window::Window;

pub mod mouse;
pub mod resources;
pub mod systems;

const ZOOM_STEP: f32 = 1.1;

pub struct State {
    window: Arc<Window>,
    wgpu_resource: WgpuResource,
    camera: Camera,
    last_frame_time: Instant,
    delta_time: f32,
}

impl State {
    pub fn new(wgpu_resource: WgpuResource, window: Arc<Window>, camera: Camera) -> Self {
        Self {
            window,
            wgpu_resource,
            camera,
            last_frame_time: Instant::now(),
            delta_time: 0.0,
        }
    }

    /// Runs at the beginning of a new frame.
    fn on_awake(&mut self) {
        let now = Instant::now();
        self.delta_time = now.duration_since(self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;
    }

    fn process_mouse_scroll_event(&mut self, delta: MouseScrollDelta) {
        let factor = match delta.is_up() {
            true => ZOOM_STEP,
            false => 1.0 / ZOOM_STEP,
        };
        self.camera.zoom_by(factor);
    }

    fn handle_window_resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.wgpu_resource.surface_config.width = size.width;
        self.wgpu_resource.surface_config.height = size.height;
        self.wgpu_resource.surface.configure(
            &self.wgpu_resource.device,
            &self.wgpu_resource.surface_config,
        );
    }
}

fn main() {
    env_logger::init();
    let (window, event_loop) = WindowResource::init().take();
    let wgpu_resource = WgpuResource::init(window.clone());
    let mut state = State::new(wgpu_resource, window.clone(), Camera::new());
    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll);
            let Event::WindowEvent { event, .. } = event else {
                return;
            };
            match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::Resized(size) => state.handle_window_resize(size),
                WindowEvent::MouseWheel { delta, .. } => state.process_mouse_scroll_event(delta),
                WindowEvent::RedrawRequested => {
                    state.on_awake();
                    render(&mut state);
                    state.window.request_redraw();
                }
                _ => {}
            }
        })
        .unwrap();
}
