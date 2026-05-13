use perro_graphics::{DrawFrameTiming, GraphicsBackend, PerroGraphics};
use perro_ids::{MaterialID, MeshID, NodeID, TextureID};
use perro_render_bridge::{
    Camera2DState, Camera3DState, Command2D, Command3D, DenseInstancePose3D, LODOptions3D,
    Material3D, Mesh3D, MeshSurfaceBinding3D, PostProcessingCommand, Rect2DCommand, RenderBridge,
    RenderCommand, RenderEvent, RenderRequestID, ResourceCommand, RuntimeMeshVertex,
    Sprite2DCommand,
};
use perro_structs::{PostProcessEffect, PostProcessSet};
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

#[derive(Default)]
struct TimingSum {
    total: Duration,
    process: Duration,
    prepare_cpu: Duration,
    gpu_prepare_2d: Duration,
    gpu_prepare_3d: Duration,
    acquire: Duration,
    encode: Duration,
    submit: Duration,
    post: Duration,
    present: Duration,
    wait_idle: Duration,
    draw_calls_2d: u64,
    draw_calls_3d: u64,
    draw_instances_3d: u64,
    presented: u64,
    frames: u64,
}

impl TimingSum {
    fn add(&mut self, timing: DrawFrameTiming) {
        self.total += timing.total;
        self.process += timing.process_commands;
        self.prepare_cpu += timing.prepare_cpu;
        self.gpu_prepare_2d += timing.gpu_prepare_2d;
        self.gpu_prepare_3d += timing.gpu_prepare_3d;
        self.acquire += timing.gpu_acquire;
        self.encode += timing.gpu_encode_main;
        self.submit += timing.gpu_submit_main;
        self.post += timing.gpu_post_process;
        self.present += timing.gpu_present;
        self.draw_calls_2d += u64::from(timing.draw_calls_2d);
        self.draw_calls_3d += u64::from(timing.draw_calls_3d);
        self.draw_instances_3d += u64::from(timing.draw_instances_3d);
        self.presented += u64::from(!timing.idle_clear);
        self.frames += 1;
    }

    fn add_wait_idle(&mut self, wait_idle: Duration) {
        self.wait_idle += wait_idle;
    }

    fn avg_us(value: Duration, frames: u64) -> u128 {
        if frames == 0 {
            return 0;
        }
        value.as_micros() / u128::from(frames)
    }

    fn print(&self, name: &str) {
        let frames = self.frames.max(1);
        println!(
            "{name:32} total={:>6}us wait={:>6}us cpu_prep={:>5}us gpu2d={:>5}us gpu3d={:>5}us acquire={:>5}us encode={:>5}us submit={:>5}us post={:>5}us present={:>5}us dc2d={:>3} dc3d={:>3} inst3d={:>7}",
            Self::avg_us(self.total, frames),
            Self::avg_us(self.wait_idle, frames),
            Self::avg_us(self.prepare_cpu, frames),
            Self::avg_us(self.gpu_prepare_2d, frames),
            Self::avg_us(self.gpu_prepare_3d, frames),
            Self::avg_us(self.acquire, frames),
            Self::avg_us(self.encode, frames),
            Self::avg_us(self.submit, frames),
            Self::avg_us(self.post, frames),
            Self::avg_us(self.present, frames),
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

struct GpuBenchApp {
    window: Option<Arc<Window>>,
    cases: Vec<BenchCase>,
}

impl ApplicationHandler for GpuBenchApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attrs = WindowAttributes::default()
                .with_title("perro gpu bench")
                .with_inner_size(PhysicalSize::new(WIDTH, HEIGHT))
                .with_visible(true);
            let window = Arc::new(event_loop.create_window(attrs).expect("window"));
            self.window = Some(window);
        }

        let window = self.window.as_ref().expect("window").clone();
        for case in &self.cases {
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
                    let wait_start = Instant::now();
                    graphics.wait_idle();
                    sum.add_wait_idle(wait_start.elapsed());
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
    let mut app = GpuBenchApp {
        window: None,
        cases: vec![
            BenchCase {
                name: "empty",
                setup: setup_empty,
                redraw: redraw_2d,
            },
            BenchCase {
                name: "rects_10k",
                setup: |w| setup_rects(w, 10_000),
                redraw: redraw_2d,
            },
            BenchCase {
                name: "rects_100k",
                setup: |w| setup_rects(w, 100_000),
                redraw: redraw_2d,
            },
            BenchCase {
                name: "sprites_10k",
                setup: |w| setup_sprites(w, 10_000),
                redraw: redraw_2d,
            },
            BenchCase {
                name: "sprites_100k",
                setup: |w| setup_sprites(w, 100_000),
                redraw: redraw_2d,
            },
            BenchCase {
                name: "meshes_10k",
                setup: |w| setup_meshes(w, 10_000),
                redraw: redraw_3d,
            },
            BenchCase {
                name: "multimesh_dense_100k",
                setup: |w| setup_multimesh_dense(w, 100_000),
                redraw: redraw_3d,
            },
            BenchCase {
                name: "rects_post_8",
                setup: |w| {
                    let mut graphics = setup_rects(w, 10_000);
                    graphics.submit(RenderCommand::PostProcessing(
                        PostProcessingCommand::SetGlobal(post_chain()),
                    ));
                    let _ = graphics.draw_frame_timed();
                    graphics
                },
                redraw: redraw_2d,
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

fn setup_empty(window: &Arc<Window>) -> PerroGraphics {
    base_graphics(window)
}

fn setup_rects(window: &Arc<Window>, count: u32) -> PerroGraphics {
    let mut graphics = base_graphics(window);
    graphics.submit_many((0..count).map(rect_command));
    let _ = graphics.draw_frame_timed();
    graphics
}

fn setup_sprites(window: &Arc<Window>, count: u32) -> PerroGraphics {
    let mut graphics = base_graphics(window);
    let texture = create_texture(&mut graphics);
    graphics.submit_many((0..count).map(|i| sprite_command(i, texture)));
    let _ = graphics.draw_frame_timed();
    graphics
}

fn setup_meshes(window: &Arc<Window>, count: u32) -> PerroGraphics {
    let mut graphics = base_graphics(window);
    let (mesh, material) = create_mesh_material(&mut graphics);
    graphics.submit_many((0..count).map(|i| draw_command(i, mesh, material)));
    let _ = graphics.draw_frame_timed();
    graphics
}

fn setup_multimesh_dense(window: &Arc<Window>, count: u32) -> PerroGraphics {
    let mut graphics = base_graphics(window);
    let (mesh, material) = create_mesh_material(&mut graphics);
    graphics.submit(draw_multi_dense_command(count, mesh, material));
    let _ = graphics.draw_frame_timed();
    graphics
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

fn rect_command(i: u32) -> RenderCommand {
    RenderCommand::TwoD(Command2D::UpsertRect {
        node: NodeID::from_parts(i + 1, 0),
        rect: Rect2DCommand {
            center: grid2(i, 12.0),
            size: [10.0, 10.0],
            color: [0.2, 0.7, 1.0, 1.0],
            z_index: i as i32,
        },
    })
}

fn sprite_command(i: u32, texture: TextureID) -> RenderCommand {
    let [x, y] = grid2(i, 12.0);
    RenderCommand::TwoD(Command2D::UpsertSprite {
        node: NodeID::from_parts(i + 1, 0),
        sprite: Sprite2DCommand {
            texture,
            model: [[10.0, 0.0, 0.0], [0.0, 10.0, 0.0], [x, y, 1.0]],
            tint: [1.0, 1.0, 1.0, 1.0],
            uv_min: [0.0, 0.0],
            uv_max: [1.0, 1.0],
            size: [10.0, 10.0],
            z_index: i as i32,
        },
    })
}

fn draw_command(i: u32, mesh: MeshID, material: MaterialID) -> RenderCommand {
    RenderCommand::ThreeD(Box::new(Command3D::Draw {
        mesh,
        surfaces: surface(material),
        node: NodeID::from_parts(i + 1, 0),
        model: model_3d(i),
        skeleton: None,
        meshlet_override: None,
        lod: LODOptions3D::default(),
    }))
}

fn draw_multi_dense_command(count: u32, mesh: MeshID, material: MaterialID) -> RenderCommand {
    let instances: Arc<[DenseInstancePose3D]> = (0..count)
        .map(|i| DenseInstancePose3D {
            position: [
                (i % 256) as f32 * 0.08 - 10.0,
                0.0,
                (i / 256) as f32 * 0.08 - 10.0,
            ],
            rotation: [0.0, 0.0, 0.0, 1.0],
        })
        .collect::<Vec<_>>()
        .into();
    RenderCommand::ThreeD(Box::new(Command3D::DrawMultiDense {
        mesh,
        surfaces: surface(material),
        node: NodeID::from_parts(1, 0),
        node_model: identity_4(),
        instance_scale: 0.05,
        instances,
        meshlet_override: None,
        lod: LODOptions3D::default(),
    }))
}

fn grid2(i: u32, step: f32) -> [f32; 2] {
    let cols = (WIDTH as f32 / step).floor().max(1.0) as u32;
    [
        (i % cols) as f32 * step - WIDTH as f32 * 0.5,
        (i / cols % 80) as f32 * step - HEIGHT as f32 * 0.5,
    ]
}

fn model_3d(i: u32) -> [[f32; 4]; 4] {
    [
        [0.05, 0.0, 0.0, (i % 256) as f32 * 0.08 - 10.0],
        [0.0, 0.05, 0.0, 0.0],
        [0.0, 0.0, 0.05, (i / 256) as f32 * 0.08 - 10.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn identity_4() -> [[f32; 4]; 4] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn surface(material: MaterialID) -> Arc<[MeshSurfaceBinding3D]> {
    Arc::from([MeshSurfaceBinding3D {
        material: Some(material),
        overrides: Arc::from([]),
        modulate: [1.0, 1.0, 1.0, 1.0],
    }])
}

fn tiny_mesh() -> Mesh3D {
    Mesh3D {
        vertices: vec![
            RuntimeMeshVertex {
                position: [0.0, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 0.0],
                joints: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
            },
            RuntimeMeshVertex {
                position: [1.0, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [1.0, 0.0],
                joints: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
            },
            RuntimeMeshVertex {
                position: [0.0, 1.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 1.0],
                joints: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
            },
        ],
        indices: vec![0, 1, 2],
        surface_ranges: vec![],
    }
}

fn create_texture(graphics: &mut PerroGraphics) -> TextureID {
    graphics.submit(RenderCommand::Resource(ResourceCommand::CreateTexture {
        request: RenderRequestID::new(1),
        id: TextureID::nil(),
        source: "__default__".to_string(),
        reserved: true,
    }));
    let _ = graphics.draw_frame_timed();
    let mut events = Vec::new();
    graphics.drain_events(&mut events);
    events
        .into_iter()
        .find_map(|event| match event {
            RenderEvent::TextureCreated { id, .. } => Some(id),
            _ => None,
        })
        .expect("texture")
}

fn create_mesh_material(graphics: &mut PerroGraphics) -> (MeshID, MaterialID) {
    graphics.submit_many([
        RenderCommand::Resource(ResourceCommand::CreateRuntimeMesh {
            request: RenderRequestID::new(2),
            id: MeshID::nil(),
            source: "__bench_mesh__".to_string(),
            reserved: true,
            mesh: tiny_mesh(),
        }),
        RenderCommand::Resource(ResourceCommand::CreateMaterial {
            request: RenderRequestID::new(3),
            id: MaterialID::nil(),
            material: Material3D::default(),
            source: Some("__bench_material__".to_string()),
            reserved: true,
        }),
    ]);
    let _ = graphics.draw_frame_timed();
    let mut events = Vec::new();
    graphics.drain_events(&mut events);
    let mut mesh = MeshID::nil();
    let mut material = MaterialID::nil();
    for event in events {
        match event {
            RenderEvent::MeshCreated { id, .. } => mesh = id,
            RenderEvent::MaterialCreated { id, .. } => material = id,
            _ => {}
        }
    }
    (mesh, material)
}

fn post_chain() -> PostProcessSet {
    PostProcessSet::from_effects(vec![
        PostProcessEffect::Bloom {
            strength: 0.7,
            threshold: 0.8,
            radius: 1.2,
        },
        PostProcessEffect::Crt {
            scanline_strength: 0.25,
            curvature: 0.1,
            chromatic: 0.5,
            vignette: 0.2,
        },
        PostProcessEffect::ColorGrade {
            exposure: 0.1,
            contrast: 1.2,
            brightness: -0.05,
            saturation: 1.3,
            gamma: 0.95,
            temperature: 0.2,
            tint: -0.1,
            hue_shift: 0.05,
            vibrance: 0.4,
            lift: [0.01, 0.02, 0.03],
            gain: [1.1, 1.05, 1.0],
            offset: [-0.01, -0.02, -0.03],
        },
    ])
}
