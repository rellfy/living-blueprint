use crate::resources::wgpu::WgpuResource;
use std::num::NonZeroUsize;
use vello::{AaSupport, Renderer, RendererOptions, Scene};
use wgpu::util::TextureBlitter;
use wgpu::{
    Device, Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    TextureView, TextureViewDescriptor,
};

pub struct RenderState {
    pub renderer: Renderer,
    pub scene: Scene,
    pub target_view: TextureView,
    pub blitter: TextureBlitter,
}

impl RenderState {
    pub fn init(wgpu_resource: &WgpuResource) -> Self {
        let device = &wgpu_resource.device;
        let renderer = Renderer::new(
            device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: AaSupport::area_only(),
                num_init_threads: NonZeroUsize::new(1),
                pipeline_cache: None,
            },
        )
        .expect("failed to create vello renderer");
        let config = &wgpu_resource.surface_config;
        Self {
            renderer,
            scene: Scene::new(),
            target_view: create_target_view(device, config.width, config.height),
            blitter: TextureBlitter::new(device, config.format),
        }
    }

    pub fn resize(&mut self, device: &Device, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.target_view = create_target_view(device, width, height);
    }
}

fn create_target_view(device: &Device, width: u32, height: u32) -> TextureView {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("Vello Target"),
        size: Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
        format: TextureFormat::Rgba8Unorm,
        view_formats: &[],
    });
    texture.create_view(&TextureViewDescriptor::default())
}
