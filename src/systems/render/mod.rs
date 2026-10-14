use crate::State;
use vello::peniko::Color;
use vello::{AaConfig, RenderParams};
use wgpu::{
    CommandEncoderDescriptor, CurrentSurfaceTexture, SurfaceTexture, TextureViewDescriptor,
};

pub mod state;

pub const BACKGROUND_COLOR: Color = Color::from_rgb8(22, 64, 122);

pub fn render(state: &mut State) {
    let Some(surface_texture) = acquire_surface_texture(state) else {
        return;
    };
    let config = &state.wgpu_resource.surface_config;
    let params = RenderParams {
        base_color: BACKGROUND_COLOR,
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
