use crate::State;
use wgpu::{
    Color, CommandEncoderDescriptor, LoadOp, Operations, RenderPassColorAttachment,
    RenderPassDescriptor, StoreOp, TextureViewDescriptor,
};

const CLEAR_COLOR: Color = Color {
    r: 0.004,
    g: 0.004,
    b: 0.008,
    a: 1.0,
};

pub fn render(state: &mut State) {
    let surface_texture = state
        .wgpu_resource
        .surface
        .get_current_texture()
        .expect("failed to acquire next swapchain texture");
    let texture_view = surface_texture.texture.create_view(&TextureViewDescriptor {
        format: Some(state.wgpu_resource.surface_format.add_srgb_suffix()),
        ..Default::default()
    });
    let mut encoder = state
        .wgpu_resource
        .device
        .create_command_encoder(&CommandEncoderDescriptor::default());
    encoder.begin_render_pass(&RenderPassDescriptor {
        label: Some("Clear Pass"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: &texture_view,
            depth_slice: None,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Clear(CLEAR_COLOR),
                store: StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });
    state.wgpu_resource.queue.submit([encoder.finish()]);
    state.window.pre_present_notify();
    surface_texture.present();
}
