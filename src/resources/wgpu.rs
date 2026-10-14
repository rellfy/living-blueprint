use std::sync::Arc;
use wgpu::{
    BackendOptions, Backends, Device, DeviceDescriptor, Instance, InstanceDescriptor,
    InstanceFlags, Limits, MemoryBudgetThresholds, PowerPreference, PresentMode, Queue,
    RequestAdapterOptions, Surface, SurfaceConfiguration, TextureFormat, TextureUsages,
};
use winit::window::Window;

pub struct WgpuResource {
    pub device: Device,
    pub queue: Queue,
    pub surface: Surface<'static>,
    pub surface_config: SurfaceConfiguration,
}

impl WgpuResource {
    pub fn init(window: Arc<Window>) -> Self {
        let instance = Instance::new(InstanceDescriptor {
            display: None,
            backends: Backends::from_env().unwrap_or(Backends::PRIMARY),
            flags: InstanceFlags::from_build_config().with_env(),
            memory_budget_thresholds: MemoryBudgetThresholds::default(),
            backend_options: BackendOptions::from_env_or_default(),
        });
        let size = window.inner_size();
        let surface = instance.create_surface(window).unwrap();
        let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .expect("no compatible GPU adapter found");
        let (device, queue) = pollster::block_on(adapter.request_device(&DeviceDescriptor {
            required_limits: Limits::default(),
            ..Default::default()
        }))
        .expect("failed to create GPU device");
        let mut surface_config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .expect("surface is not supported by the adapter");
        let capabilities = surface.get_capabilities(&adapter);
        surface_config.format = pick_surface_format(&capabilities.formats);
        surface_config.usage = TextureUsages::RENDER_ATTACHMENT;
        surface_config.present_mode = PresentMode::AutoVsync;
        surface.configure(&device, &surface_config);
        Self {
            device,
            queue,
            surface,
            surface_config,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
    }

    pub fn reconfigure(&self) {
        self.surface.configure(&self.device, &self.surface_config);
    }
}

fn pick_surface_format(formats: &[TextureFormat]) -> TextureFormat {
    formats
        .iter()
        .copied()
        .find(|format| {
            matches!(
                format,
                TextureFormat::Rgba8Unorm | TextureFormat::Bgra8Unorm
            )
        })
        .unwrap_or(formats[0])
}
