use crate::State;
use crate::systems::render::background::draw_background;
use crate::systems::render::edges::{collect_linked_nodes, draw_edges};
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
    pub delta_time: f64,
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
        draw_edges(&mut render_state.scene, repository, &frame, state.hovered);
        MapPainter {
            scene: &mut render_state.scene,
            cache: &mut render_state.cache,
            text_engine: &mut state.text_engine,
            repository,
            frame: &frame,
            hovered: state.hovered,
            linked: &collect_linked_nodes(repository, state.hovered),
        }
        .draw();
    }
    draw_orb(&mut render_state.scene, &state.agent_session.orb, &frame);
    let mut overlay = OverlayPainter {
        scene: &mut render_state.scene,
        text_engine: &mut state.text_engine,
        frame: &frame,
    };
    overlay.draw_feed(&mut state.agent_session.feed, &state.prompt);
    overlay.draw_prompt(&mut state.prompt);
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
        delta_time: state.delta_time,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::{Detail, Repository};
    use crate::resources::camera::Camera;
    use crate::resources::text::TextEngine;
    use crate::systems::render::cache::RenderCache;
    use std::collections::HashSet;
    use std::num::NonZeroUsize;
    use std::path::Path;
    use vello::kurbo::{Point, Size};
    use vello::{AaSupport, Renderer, RendererOptions, Scene};
    use wgpu::{
        BackendOptions, Backends, DeviceDescriptor, Extent3d, Instance, InstanceDescriptor,
        InstanceFlags, MemoryBudgetThresholds, RequestAdapterOptions, TextureDescriptor,
        TextureDimension, TextureFormat, TextureUsages,
    };

    #[test]
    #[ignore = "requires a GPU"]
    fn renders_repository_offscreen_at_several_zoom_levels() {
        let (device, queue, mut renderer, texture) = create_gpu();
        let view = texture.create_view(&Default::default());
        let viewport = Size::new(1280.0, 800.0);
        let mut text_engine = TextEngine::new();
        let mut repository = Repository::load(
            Path::new(env!("CARGO_MANIFEST_DIR")),
            text_engine.char_advance(),
        )
        .unwrap();
        repository.set_all_details(Detail::Expanded);
        repository.animate(10.0);
        let main_rect = repository
            .node(repository.find_by_path(Path::new("src/main.rs")).unwrap())
            .rect;
        let mut cache = RenderCache::default();
        let mut camera = Camera::new();
        for (index, (rect, zoom_factor)) in [
            (repository.bounds(), 1.0),
            (main_rect, 1.0),
            (main_rect, 0.15),
        ]
        .into_iter()
        .enumerate()
        {
            camera.jump_to(rect, viewport);
            camera.zoom_at(Point::new(640.0, 400.0), zoom_factor, viewport);
            for _ in 0..60 {
                camera.update(1.0 / 60.0, viewport);
            }
            let frame = FrameContext {
                viewport,
                transform: camera.transform(viewport),
                zoom: camera.zoom(),
                visible: camera.visible_world_rect(viewport),
                scale_factor: 1.0,
                elapsed_seconds: 0.0,
                delta_time: 1.0,
            };
            let mut scene = Scene::new();
            cache.begin_frame();
            draw_background(&mut scene, &frame);
            draw_edges(&mut scene, &repository, &frame, None);
            MapPainter {
                scene: &mut scene,
                cache: &mut cache,
                text_engine: &mut text_engine,
                repository: &repository,
                frame: &frame,
                hovered: Some(repository.root_id()),
                linked: &HashSet::new(),
            }
            .draw();
            let mut prompt = crate::resources::prompt::Prompt::new();
            prompt.insert("explain how the semantic");
            prompt.break_line();
            prompt.insert("zoom levels are chosen");
            prompt.move_caret(crate::resources::prompt::Motion::WordLeft, false);
            prompt.move_caret(crate::resources::prompt::Motion::WordLeft, true);
            prompt.visibility = 1.0;
            let mut session = crate::resources::agent_session::AgentSession::new();
            session.status = crate::resources::agent_session::AgentStatus::Working;
            session.feed.push(
                crate::resources::agent_feed::FeedKind::Prompt,
                "explain the zoom levels",
            );
            session.feed.push(
                crate::resources::agent_feed::FeedKind::ToolUse,
                "Read src/systems/render/map.rs",
            );
            session.feed.push(
                crate::resources::agent_feed::FeedKind::Message,
                &"The map picks a level from the on-screen line height of each file. ".repeat(4),
            );
            session
                .feed
                .push(crate::resources::agent_feed::FeedKind::Success, "done");
            for entry in &mut session.feed.entries {
                entry.age_seconds = 5.0;
            }
            session.orb.visibility = 1.0;
            session.orb.position = main_rect.center();
            draw_orb(&mut scene, &session.orb, &frame);
            let mut overlay = OverlayPainter {
                scene: &mut scene,
                text_engine: &mut text_engine,
                frame: &frame,
            };
            overlay.draw_feed(&mut session.feed, &prompt);
            overlay.draw_prompt(&mut prompt);
            let params = RenderParams {
                base_color: BACKGROUND,
                width: 1280,
                height: 800,
                antialiasing_method: AaConfig::Area,
            };
            renderer
                .render_to_texture(&device, &queue, &scene, &view, &params)
                .unwrap();
            if let Ok(directory) = std::env::var("LIVING_BLUEPRINT_SNAPSHOT_DIR") {
                write_snapshot(
                    &device,
                    &queue,
                    &texture,
                    &format!("{directory}/snapshot_{index}.ppm"),
                );
            }
        }
    }

    #[test]
    #[ignore = "requires a GPU"]
    fn renders_semantic_zoom_levels_offscreen() {
        let (device, queue, mut renderer, texture) = create_gpu();
        let view = texture.create_view(&Default::default());
        let viewport = Size::new(1280.0, 800.0);
        let mut text_engine = TextEngine::new();
        let mut repository = Repository::load(
            Path::new(env!("CARGO_MANIFEST_DIR")),
            text_engine.char_advance(),
        )
        .unwrap();
        repository.set_all_details(Detail::Expanded);
        let root = repository.root_id();
        let camera_id = repository
            .find_by_path(Path::new("src/resources/camera.rs"))
            .unwrap();
        let mut cache = RenderCache::default();
        let mut camera = Camera::new();
        let root_steps = [1, 1, 1, 1, 1, 1];
        for (index, compress_steps) in root_steps.into_iter().enumerate() {
            for _ in 0..compress_steps {
                repository.compress(root);
            }
            repository.animate(10.0);
            let focus = match index {
                1 => repository.node(camera_id).rect.inflate(400.0, 200.0),
                _ => repository.bounds(),
            };
            camera.jump_to(focus, viewport);
            let frame = FrameContext {
                viewport,
                transform: camera.transform(viewport),
                zoom: camera.zoom(),
                visible: camera.visible_world_rect(viewport),
                scale_factor: 1.0,
                elapsed_seconds: 0.0,
                delta_time: 1.0,
            };
            let mut scene = Scene::new();
            cache.begin_frame();
            draw_background(&mut scene, &frame);
            draw_edges(&mut scene, &repository, &frame, Some(camera_id));
            MapPainter {
                scene: &mut scene,
                cache: &mut cache,
                text_engine: &mut text_engine,
                repository: &repository,
                frame: &frame,
                hovered: Some(camera_id),
                linked: &collect_linked_nodes(&repository, Some(camera_id)),
            }
            .draw();
            let params = RenderParams {
                base_color: BACKGROUND,
                width: 1280,
                height: 800,
                antialiasing_method: AaConfig::Area,
            };
            renderer
                .render_to_texture(&device, &queue, &scene, &view, &params)
                .unwrap();
            if let Ok(directory) = std::env::var("LIVING_BLUEPRINT_SNAPSHOT_DIR") {
                write_snapshot(
                    &device,
                    &queue,
                    &texture,
                    &format!("{directory}/semantic_{index}.ppm"),
                );
            }
        }
    }

    fn create_gpu() -> (wgpu::Device, wgpu::Queue, Renderer, wgpu::Texture) {
        let instance = Instance::new(InstanceDescriptor {
            display: None,
            backends: Backends::PRIMARY,
            flags: InstanceFlags::default(),
            memory_budget_thresholds: MemoryBudgetThresholds::default(),
            backend_options: BackendOptions::default(),
        });
        let adapter =
            pollster::block_on(instance.request_adapter(&RequestAdapterOptions::default()))
                .unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&DeviceDescriptor::default())).unwrap();
        let mut renderer = Renderer::new(
            &device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: AaSupport::area_only(),
                num_init_threads: NonZeroUsize::new(1),
                pipeline_cache: None,
            },
        )
        .unwrap();
        let texture = device.create_texture(&TextureDescriptor {
            label: None,
            size: Extent3d {
                width: 1280,
                height: 800,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            usage: TextureUsages::STORAGE_BINDING
                | TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_SRC,
            format: TextureFormat::Rgba8Unorm,
            view_formats: &[],
        });
        (device, queue, renderer, texture)
    }

    fn write_snapshot(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        path: &str,
    ) {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 1280 * 800 * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(1280 * 4),
                    rows_per_image: None,
                },
            },
            Extent3d {
                width: 1280,
                height: 800,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);
        buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let pixels = buffer.slice(..).get_mapped_range().unwrap();
        let mut bytes = b"P6\n1280 800\n255\n".to_vec();
        for pixel in pixels.chunks(4) {
            bytes.extend_from_slice(&pixel[..3]);
        }
        std::fs::write(path, bytes).unwrap();
    }
}
