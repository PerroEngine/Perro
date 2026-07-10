use perro_graphics::{DrawFrameTiming, GraphicsBackend, PerroGraphics};
use perro_ids::{MaterialID, MeshID, NodeID, TextureID};
use perro_render_bridge::{
    Camera2DState, Camera3DState, CameraStream3DState, CameraStreamCommand,
    CameraStreamDraw3DState, CameraStreamLighting3DState, CameraStreamSourceState,
    CameraStreamState, Command2D, Command3D, LODOptions3D, Material3D, Mesh3D, MeshBlendOptions3D,
    MeshSurfaceBinding3D, RenderBridge, RenderCommand, RenderEvent, RenderRequestID,
    ResourceCommand, RuntimeMeshVertex, Sprite2DCommand,
};
use perro_structs::Color;
use std::env;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
const WARMUP_FRAMES: usize = 8;
const SAMPLE_FRAMES: usize = 60;

// webcam-style bench state: a bare `redraw` fn can't capture, so the displayed
// stream texture id + resolution set during setup live here for each case.
std::thread_local! {
    static WEBCAM_STREAM: std::cell::Cell<(u64, u32, u32)> = const { std::cell::Cell::new((0, 0, 0)) };
}

#[derive(Default)]
struct TimingSum {
    total: Duration,
    process: Duration,
    prepare_cpu: Duration,
    gpu_prepare_2d: Duration,
    gpu_prepare_3d: Duration,
    encode: Duration,
    submit: Duration,
    post: Duration,
    gpu_main: Duration,
    gpu_water: Duration,
    wait_idle: Duration,
    draw_calls_2d: u64,
    draw_calls_3d: u64,
    draw_instances_3d: u64,
    frames: u64,
}

impl TimingSum {
    fn add(&mut self, timing: DrawFrameTiming) {
        self.total += timing.total;
        self.process += timing.process_commands;
        self.prepare_cpu += timing.prepare_cpu;
        self.gpu_prepare_2d += timing.gpu_prepare_2d;
        self.gpu_prepare_3d += timing.gpu_prepare_3d;
        self.encode += timing.gpu_encode_main;
        self.submit += timing.gpu_submit_main;
        self.post += timing.gpu_post_process;
        self.gpu_main += timing.gpu_timestamp_main;
        self.gpu_water += timing.gpu_timestamp_water;
        self.draw_calls_2d += u64::from(timing.draw_calls_2d);
        self.draw_calls_3d += u64::from(timing.draw_calls_3d);
        self.draw_instances_3d += u64::from(timing.draw_instances_3d);
        self.frames += 1;
    }

    fn add_wait_idle(&mut self, wait_idle: Duration) {
        self.wait_idle += wait_idle;
    }

    fn avg_us(value: Duration, frames: u64) -> u128 {
        value.as_micros() / u128::from(frames.max(1))
    }

    fn print(&self, name: &str) {
        let frames = self.frames.max(1);
        println!(
            "{name:34} total={:>6}us wait={:>6}us gpuq={:>6}us water={:>5}us cpu={:>5}us gpu2d={:>5}us gpu3d={:>5}us encode={:>5}us submit={:>5}us post={:>5}us dc2d={:>3} dc3d={:>3} inst3d={:>6}",
            Self::avg_us(self.total, frames),
            Self::avg_us(self.wait_idle, frames),
            Self::avg_us(self.gpu_main, frames),
            Self::avg_us(self.gpu_water, frames),
            Self::avg_us(self.prepare_cpu, frames),
            Self::avg_us(self.gpu_prepare_2d, frames),
            Self::avg_us(self.gpu_prepare_3d, frames),
            Self::avg_us(self.encode, frames),
            Self::avg_us(self.submit, frames),
            Self::avg_us(self.post, frames),
            self.draw_calls_2d / frames,
            self.draw_calls_3d / frames,
            self.draw_instances_3d / frames,
        );
    }
}

struct BenchCase {
    name: &'static str,
    setup: fn(&Arc<Window>) -> PerroGraphics,
    redraw: fn(&mut PerroGraphics),
}

struct App {
    window: Option<Arc<Window>>,
    cases: Vec<BenchCase>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attrs = WindowAttributes::default()
                .with_title("perro camera stream bench")
                .with_inner_size(PhysicalSize::new(WIDTH, HEIGHT))
                .with_visible(true);
            self.window = Some(Arc::new(event_loop.create_window(attrs).expect("window")));
        }

        let window = self.window.as_ref().expect("window").clone();
        for case in &self.cases {
            if let Ok(filter) = env::var("PERRO_CAMERA_STREAM_BENCH")
                && !case.name.contains(&filter)
            {
                continue;
            }
            let mut graphics = (case.setup)(&window);
            for _ in 0..WARMUP_FRAMES {
                (case.redraw)(&mut graphics);
                let _ = graphics.draw_frame_timed();
            }
            let mut sum = TimingSum::default();
            for _ in 0..SAMPLE_FRAMES {
                (case.redraw)(&mut graphics);
                if let Some(timing) = graphics.draw_frame_timed() {
                    sum.add(timing);
                    let wait = Instant::now();
                    graphics.wait_idle();
                    sum.add_wait_idle(wait.elapsed());
                }
            }
            sum.print(case.name);
        }
        event_loop.exit();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        if matches!(event, WindowEvent::CloseRequested) {
            event_loop.exit();
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    let mut app = App {
        window: None,
        cases: vec![
            BenchCase {
                name: "stream2d_1_512_sprites1k",
                setup: |w| setup_stream_2d(w, 1, 512, 1_000),
                redraw: redraw_2d,
            },
            BenchCase {
                name: "stream2d_4_512_sprites1k",
                setup: |w| setup_stream_2d(w, 4, 512, 1_000),
                redraw: redraw_2d,
            },
            BenchCase {
                name: "stream2d_1_1024_sprites10k",
                setup: |w| setup_stream_2d(w, 1, 1024, 10_000),
                redraw: redraw_2d,
            },
            BenchCase {
                name: "stream3d_1_512_meshes1k",
                setup: |w| setup_stream_3d(w, 1, 512, 1_000),
                redraw: redraw_3d,
            },
            BenchCase {
                name: "stream3d_4_512_meshes1k",
                setup: |w| setup_stream_3d(w, 4, 512, 1_000),
                redraw: redraw_3d,
            },
            BenchCase {
                name: "stream3d_1_1024_meshes10k",
                setup: |w| setup_stream_3d(w, 1, 1024, 10_000),
                redraw: redraw_3d,
            },
            // webcam CPU-upload path: per-frame WriteTextureRgba to a displayed
            // stream texture (exercises findings 1-3, not covered by the
            // render-target stream cases above).
            BenchCase {
                name: "webcam_512_sprites1k",
                setup: |w| setup_stream_webcam(w, 512, 1_000),
                redraw: redraw_webcam,
            },
            BenchCase {
                name: "webcam_1024_sprites10k",
                setup: |w| setup_stream_webcam(w, 1024, 10_000),
                redraw: redraw_webcam,
            },
        ],
    };
    event_loop.run_app(&mut app).expect("run app");
}

fn base_graphics(window: &Arc<Window>) -> PerroGraphics {
    let mut graphics = PerroGraphics::new()
        .with_vsync(false)
        .with_msaa(false)
        .with_occlusion_culling(perro_graphics::OcclusionCullingMode::Off);
    graphics.attach_window(window.clone());
    graphics.resize(WIDTH, HEIGHT);
    graphics
}

fn setup_stream_2d(
    window: &Arc<Window>,
    stream_count: u32,
    resolution: u32,
    sprite_count: u32,
) -> PerroGraphics {
    let mut graphics = base_graphics(window);
    let texture = create_texture(&mut graphics);
    let sprites: Arc<[Sprite2DCommand]> = (0..sprite_count)
        .map(|i| sprite_state(i, texture))
        .collect::<Vec<_>>()
        .into();
    for i in 0..stream_count {
        let node = NodeID::from_parts(100_000 + i, 0);
        let output = TextureID::from_parts(200_000 + i, 0);
        let stream = camera_stream_2d_state(output, resolution, sprites.clone());
        graphics.submit(RenderCommand::CameraStream(CameraStreamCommand::Upsert {
            node,
            state: Box::new(stream.clone()),
        }));
        graphics.submit(RenderCommand::TwoD(Command2D::UpsertCameraStream {
            node,
            stream: Box::new(stream),
            sprite: stream_display_sprite(i, output, resolution),
        }));
    }
    let _ = graphics.draw_frame_timed();
    graphics
}

fn setup_stream_3d(
    window: &Arc<Window>,
    stream_count: u32,
    resolution: u32,
    mesh_count: u32,
) -> PerroGraphics {
    let mut graphics = base_graphics(window);
    let (mesh, material) = create_mesh_material(&mut graphics);
    let draws: Arc<[CameraStreamDraw3DState]> = (0..mesh_count)
        .map(|i| stream_draw_state(i, mesh, material))
        .collect::<Vec<_>>()
        .into();
    for i in 0..stream_count {
        let node = NodeID::from_parts(300_000 + i, 0);
        let output = TextureID::from_parts(400_000 + i, 0);
        let stream = camera_stream_3d_state(output, resolution, draws.clone());
        graphics.submit(RenderCommand::CameraStream(CameraStreamCommand::Upsert {
            node,
            state: Box::new(stream.clone()),
        }));
        graphics.submit(RenderCommand::ThreeD(Box::new(
            Command3D::UpsertCameraStream {
                node,
                stream: Box::new(stream),
                quad: CameraStream3DState {
                    model: quad_model(i),
                    size: [1.5, 1.0],
                    tint: Color::WHITE,
                },
            },
        )));
    }
    let _ = graphics.draw_frame_timed();
    graphics
}

fn setup_stream_webcam(
    window: &Arc<Window>,
    resolution: u32,
    sprite_count: u32,
) -> PerroGraphics {
    let mut graphics = base_graphics(window);
    let texture = TextureID::from_parts(900_001, 1);
    graphics.submit(RenderCommand::Resource(
        ResourceCommand::CreateExternalTexture {
            request: RenderRequestID::new(9),
            id: texture,
            source: "webcam://bench".to_string(),
            reserved: true,
            width: resolution,
            height: resolution,
        },
    ));
    for i in 0..sprite_count {
        graphics.submit(RenderCommand::TwoD(Command2D::UpsertSprite {
            node: NodeID::from_parts(600_000 + i, 0),
            sprite: sprite_state(i, texture),
        }));
    }
    // prime the first frame so the displayed sprite texture builds; subsequent
    // redraws hit the persistent-texture in-place upload path.
    graphics.submit(RenderCommand::Resource(ResourceCommand::WriteTextureRgba {
        id: texture,
        width: resolution,
        height: resolution,
        rgba: webcam_frame_bytes(resolution, resolution).into(),
    }));
    let _ = graphics.draw_frame_timed();
    WEBCAM_STREAM.with(|state| state.set((texture.as_u64(), resolution, resolution)));
    graphics
}

fn redraw_webcam(graphics: &mut PerroGraphics) {
    let (raw, width, height) = WEBCAM_STREAM.with(|state| state.get());
    graphics.submit(RenderCommand::Resource(ResourceCommand::WriteTextureRgba {
        id: TextureID::from_u64(raw),
        width,
        height,
        rgba: webcam_frame_bytes(width, height).into(),
    }));
}

fn webcam_frame_bytes(width: u32, height: u32) -> Vec<u8> {
    vec![0u8; width as usize * height as usize * 4]
}

fn redraw_2d(graphics: &mut PerroGraphics) {
    graphics.submit(RenderCommand::TwoD(Command2D::SetCamera {
        camera: Camera2DState::default(),
    }));
}

fn redraw_3d(graphics: &mut PerroGraphics) {
    graphics.submit(RenderCommand::ThreeD(Box::new(Command3D::SetCamera {
        camera: Camera3DState::default(),
    })));
}

fn camera_stream_2d_state(
    output_texture: TextureID,
    resolution: u32,
    sprites_2d: Arc<[Sprite2DCommand]>,
) -> CameraStreamState {
    CameraStreamState {
        source: CameraStreamSourceState::TwoD(Camera2DState::default()),
        resolution: [resolution, resolution],
        aspect_ratio: 1.0,
        post_processing: Arc::from([]),
        output_texture,
        sprites_2d,
        lights_2d: Arc::from([]),
        point_particles_2d: Arc::from([]),
        waters_2d: Arc::from([]),
        draws_3d: Arc::from([]),
        lighting_3d: CameraStreamLighting3DState::default(),
        point_particles_3d: Arc::from([]),
        waters_3d: Arc::from([]),
    }
}

fn camera_stream_3d_state(
    output_texture: TextureID,
    resolution: u32,
    draws_3d: Arc<[CameraStreamDraw3DState]>,
) -> CameraStreamState {
    CameraStreamState {
        source: CameraStreamSourceState::ThreeD(Camera3DState::default()),
        resolution: [resolution, resolution],
        aspect_ratio: 1.0,
        post_processing: Arc::from([]),
        output_texture,
        sprites_2d: Arc::from([]),
        lights_2d: Arc::from([]),
        point_particles_2d: Arc::from([]),
        waters_2d: Arc::from([]),
        draws_3d,
        lighting_3d: CameraStreamLighting3DState::default(),
        point_particles_3d: Arc::from([]),
        waters_3d: Arc::from([]),
    }
}

fn stream_display_sprite(i: u32, texture: TextureID, resolution: u32) -> Sprite2DCommand {
    Sprite2DCommand {
        texture,
        model: [
            [resolution as f32 * 0.25, 0.0, 0.0],
            [0.0, resolution as f32 * 0.25, 0.0],
            [160.0 + i as f32 * 180.0, 160.0, 1.0],
        ],
        tint: Color::WHITE,
        uv_min: [0.0, 0.0],
        uv_max: [1.0, 1.0],
        size: [resolution as f32, resolution as f32],
        z_index: 10_000 + i as i32,
    }
}

fn sprite_state(i: u32, texture: TextureID) -> Sprite2DCommand {
    Sprite2DCommand {
        texture,
        model: [
            [12.0, 0.0, 0.0],
            [0.0, 12.0, 0.0],
            [(i % 256) as f32 * 10.0, (i / 256) as f32 * 10.0, 1.0],
        ],
        tint: Color::WHITE,
        uv_min: [0.0, 0.0],
        uv_max: [1.0, 1.0],
        size: [12.0, 12.0],
        z_index: i as i32,
    }
}

fn stream_draw_state(i: u32, mesh: MeshID, material: MaterialID) -> CameraStreamDraw3DState {
    CameraStreamDraw3DState::Draw {
        mesh,
        surfaces: surface(material),
        node: NodeID::from_parts(500_000 + i, 0),
        model: model_3d(i),
        skeleton: None,
        meshlet_override: None,
        lod: LODOptions3D::default(),
        blend: MeshBlendOptions3D::default(),
    }
}

fn quad_model(i: u32) -> [[f32; 4]; 4] {
    [
        [1.0, 0.0, 0.0, i as f32 * 1.8 - 2.7],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, -4.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn model_3d(i: u32) -> [[f32; 4]; 4] {
    [
        [1.0, 0.0, 0.0, (i % 128) as f32 * 0.35 - 16.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, (i / 128) as f32 * 0.35 - 8.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn surface(material: MaterialID) -> Arc<[MeshSurfaceBinding3D]> {
    Arc::from([MeshSurfaceBinding3D {
        material: Some(material),
        overrides: Arc::from([]),
        modulate: Color::WHITE,
    }])
}

fn create_texture(graphics: &mut PerroGraphics) -> TextureID {
    graphics.submit(RenderCommand::Resource(ResourceCommand::CreateTexture {
        request: RenderRequestID::new(1),
        id: TextureID::nil(),
        source: "__bench_texture__".to_string(),
        reserved: true,
    }));
    graphics.draw_frame();
    let mut events = Vec::new();
    graphics.drain_events(&mut events);
    events
        .into_iter()
        .find_map(|event| match event {
            RenderEvent::TextureCreated { id, .. } => Some(id),
            _ => None,
        })
        .expect("texture event")
}

fn create_mesh_material(graphics: &mut PerroGraphics) -> (MeshID, MaterialID) {
    graphics.submit_many([
        RenderCommand::Resource(ResourceCommand::CreateRuntimeMesh {
            request: RenderRequestID::new(2),
            id: MeshID::nil(),
            source: "__bench_mesh__".to_string(),
            mesh: tiny_mesh(),
            reserved: true,
        }),
        RenderCommand::Resource(ResourceCommand::CreateMaterial {
            request: RenderRequestID::new(3),
            id: MaterialID::nil(),
            material: Material3D::default(),
            source: Some("__bench_material__".to_string()),
            reserved: true,
        }),
    ]);
    graphics.draw_frame();
    let mut events = Vec::new();
    graphics.drain_events(&mut events);
    let mut mesh = None;
    let mut material = None;
    for event in events {
        match event {
            RenderEvent::MeshCreated { id, .. } => mesh = Some(id),
            RenderEvent::MaterialCreated { id, .. } => material = Some(id),
            _ => {}
        }
    }
    (mesh.expect("mesh event"), material.expect("material event"))
}

fn tiny_mesh() -> Mesh3D {
    Mesh3D {
        vertices: vec![
            RuntimeMeshVertex {
                position: [-0.05, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 0.0],
                paint_uv: [0.0, 0.0],
                joints: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0].into(),
            },
            RuntimeMeshVertex {
                position: [0.05, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [1.0, 0.0],
                paint_uv: [1.0, 0.0],
                joints: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0].into(),
            },
            RuntimeMeshVertex {
                position: [0.0, 0.1, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.5, 1.0],
                paint_uv: [0.5, 1.0],
                joints: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0].into(),
            },
        ],
        indices: vec![0, 1, 2],
        surface_ranges: vec![],
        blend_shapes: vec![],
    }
}
