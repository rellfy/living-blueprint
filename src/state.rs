use crate::repository::{NodeId, Repository};
use crate::resources::agent_feed::AgentFeed;
use crate::resources::agent_sessions::AgentSessions;
use crate::resources::camera::Camera;
use crate::resources::clipboard::Clipboard;
use crate::resources::pointer::Pointer;
use crate::resources::prompt::Prompt;
use crate::resources::repository_sync::RepositorySync;
use crate::resources::text::TextEngine;
use crate::resources::wgpu::WgpuResource;
use crate::systems::agent::state::AgentState;
use crate::systems::agent::update_agent;
use crate::systems::input::{
    handle_cursor_moved, handle_key, handle_mouse_button, handle_mouse_wheel, update_hover,
    viewport_size,
};
use crate::systems::prompt::update_prompt;
use crate::systems::render::render;
use crate::systems::render::state::RenderState;
use crate::systems::repository_sync::update_repository;
use crate::systems::semantic_zoom::update_semantic_zoom;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use vello::kurbo::Point;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::keyboard::ModifiersState;
use winit::window::Window;

const MAX_DELTA_TIME: f64 = 0.1;

pub struct State {
    pub window: Arc<Window>,
    pub wgpu_resource: WgpuResource,
    pub render_state: RenderState,
    pub text_engine: TextEngine,
    pub camera: Camera,
    pub pointer: Pointer,
    pub prompt: Prompt,
    pub clipboard: Clipboard,
    pub modifiers: ModifiersState,
    pub agent_sessions: AgentSessions,
    pub agent_feed: AgentFeed,
    pub agent_state: AgentState,
    pub repository_root: PathBuf,
    pub repository: Option<Repository>,
    pub repository_sync: RepositorySync,
    pub load_error: Option<String>,
    pub hovered: Option<NodeId>,
    pub start_time: Instant,
    pub last_frame_time: Instant,
    pub delta_time: f64,
}

impl State {
    pub fn new(window: Arc<Window>, repository_root: PathBuf) -> Self {
        let wgpu_resource = WgpuResource::init(window.clone());
        let render_state = RenderState::init(&wgpu_resource);
        let text_engine = TextEngine::new();
        let repository_sync =
            RepositorySync::start(repository_root.clone(), text_engine.char_advance());
        Self {
            window,
            wgpu_resource,
            render_state,
            text_engine,
            camera: Camera::new(),
            pointer: Pointer::new(),
            prompt: Prompt::new(),
            clipboard: Clipboard::new(),
            modifiers: ModifiersState::empty(),
            agent_sessions: AgentSessions::default(),
            agent_feed: AgentFeed::default(),
            agent_state: AgentState::new(&repository_root),
            repository_root,
            repository: None,
            repository_sync,
            load_error: None,
            hovered: None,
            start_time: Instant::now(),
            last_frame_time: Instant::now(),
            delta_time: 0.0,
        }
    }

    pub fn handle_window_event(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::Resized(size) => self.handle_resize(size),
            WindowEvent::RedrawRequested => self.run_frame(),
            WindowEvent::CursorMoved { position, .. } => {
                handle_cursor_moved(self, Point::new(position.x, position.y))
            }
            WindowEvent::MouseInput { state, button, .. } => {
                handle_mouse_button(self, button, state)
            }
            WindowEvent::MouseWheel { delta, .. } => handle_mouse_wheel(self, delta),
            WindowEvent::KeyboardInput { event, .. } => handle_key(self, event),
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            _ => {}
        }
    }

    fn run_frame(&mut self) {
        self.on_awake();
        update_repository(self);
        self.camera.update(self.delta_time, viewport_size(self));
        update_semantic_zoom(self);
        update_hover(self);
        update_agent(self);
        update_prompt(self);
        render(self);
    }

    fn on_awake(&mut self) {
        let now = Instant::now();
        self.delta_time = now
            .duration_since(self.last_frame_time)
            .as_secs_f64()
            .min(MAX_DELTA_TIME);
        self.last_frame_time = now;
    }

    fn handle_resize(&mut self, size: PhysicalSize<u32>) {
        self.wgpu_resource.resize(size.width, size.height);
        self.render_state
            .resize(&self.wgpu_resource.device, size.width, size.height);
    }
}
