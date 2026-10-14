use crate::State;
use crate::systems::render::background::draw_background;
use crate::systems::render::edges::draw_edges;
use crate::systems::render::map::MapPainter;
use crate::systems::render::orb::draw_orb;
use crate::systems::render::overlay::OverlayPainter;
use crate::systems::render::palette::BACKGROUND;
use vello::kurbo::{Affine, Rect, Size};
use vello::{AaConfig, RenderParams};
use wgpu::{
    CommandEncoderDescriptor, CurrentSurfaceTexture, SurfaceTexture, TextureViewDescriptor,
};

pub mod background;
pub mod cache;
pub mod edges;
pub mod map;
pub mod orb;
pub mod overlay;
pub mod palette;
pub mod state;
pub mod text;

const LOADING_MESSAGE: &str = "Reading repository…";

pub struct FrameContext {
    pub viewport: Size,
    pub transform: Affine,
    pub zoom: f64,
    pub visible: Rect,
    pub scale_factor: f64,
    pub elapsed_seconds: f64,
}

pub fn render(state: &mut State) {
    compose_scene(state);
    let Some(surface_texture) = acquire_surface_texture(state) else {
        return;
    };
    submit_scene(state, surface_texture);
}

pub fn smoothstep(edge_start: f64, edge_end: f64, value: f64) -> f64 {
    let t = ((value - edge_start) / (edge_end - edge_start)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn check_overlaps(a: Rect, b: Rect) -> bool {
    a.x0 <= b.x1 && a.x1 >= b.x0 && a.y0 <= b.y1 && a.y1 >= b.y0
}

fn compose_scene(state: &mut State) {
    let frame = create_frame_context(state);
    let render_state = &mut state.render_state;
    render_state.scene.reset();
    render_state.cache.begin_frame();
    draw_background(&mut render_state.scene, &frame);
    if let Some(repository) = &state.repository {
        MapPainter {
            scene: &mut render_state.scene,
            cache: &mut render_state.cache,
            text_engine: &mut state.text_engine,
            repository,
            frame: &frame,
            hovered: state.hovered,
        }
        .draw();
        draw_edges(&mut render_state.scene, repository, &frame, state.hovered);
    }
    draw_orb(&mut render_state.scene, &state.agent_session.orb, &frame);
    let mut overlay = OverlayPainter {
        scene: &mut render_state.scene,
        text_engine: &mut state.text_engine,
        frame: &frame,
    };
    overlay.draw_status(&state.agent_session);
    overlay.draw_prompt(&state.prompt);
    let notice = match (&state.load_error, state.repository.is_none()) {
        (Some(error), _) => Some(error.clone()),
        (None, true) => Some(LOADING_MESSAGE.to_string()),
        (None, false) => None,
    };
    if let Some(notice) = notice {
        let alpha = 0.6 + 0.4 * (frame.elapsed_seconds * 3.0).sin() as f32;
        overlay.draw_notice(&notice, alpha);
    }
}

fn create_frame_context(state: &State) -> FrameContext {
    let config = &state.wgpu_resource.surface_config;
    let viewport = Size::new(config.width as f64, config.height as f64);
    FrameContext {
        viewport,
        transform: state.camera.transform(viewport),
        zoom: state.camera.zoom(),
        visible: state.camera.visible_world_rect(viewport),
        scale_factor: state.window.scale_factor(),
        elapsed_seconds: state.start_time.elapsed().as_secs_f64(),
    }
}

fn submit_scene(state: &mut State, surface_texture: SurfaceTexture) {
    let config = &state.wgpu_resource.surface_config;
    let params = RenderParams {
        base_color: BACKGROUND,
        width: config.width,
        height: config.height,
        antialiasing_method: AaConfig::Area,
    };
    let render_state = &mut state.render_state;
    render_state
        .renderer
        .render_to_texture(
            &state.wgpu_resource.device,
            &state.wgpu_resource.queue,
            &render_state.scene,
            &render_state.target_view,
            &params,
        )
        .expect("failed to render scene");
    let surface_view = surface_texture
        .texture
        .create_view(&TextureViewDescriptor::default());
    let mut encoder = state
        .wgpu_resource
        .device
        .create_command_encoder(&CommandEncoderDescriptor::default());
    render_state.blitter.copy(
        &state.wgpu_resource.device,
        &mut encoder,
        &render_state.target_view,
        &surface_view,
    );
    state.wgpu_resource.queue.submit([encoder.finish()]);
    state.window.pre_present_notify();
    state.wgpu_resource.queue.present(surface_texture);
}

fn acquire_surface_texture(state: &mut State) -> Option<SurfaceTexture> {
    match state.wgpu_resource.surface.get_current_texture() {
        CurrentSurfaceTexture::Success(texture) => Some(texture),
        CurrentSurfaceTexture::Suboptimal(texture) => Some(texture),
        CurrentSurfaceTexture::Outdated | CurrentSurfaceTexture::Lost => {
            state.wgpu_resource.reconfigure();
            None
        }
        _ => None,
    }
}
