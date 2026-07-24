use perro_graphics::{DrawFrameTiming, GraphicsBackend, PerroGraphics};
use perro_ids::{MaterialID, MeshID, NodeID, TextureID};
use perro_render_bridge::{
    Camera2DState, Camera3DState, Command2D, Command3D, DenseInstancePose3D, LODOptions3D,
    Material3D, Mesh3D, MeshBlendOptions3D, MeshSurfaceBinding3D, PointLight3DState,
    PostProcessingCommand, RayLight3DState, Rect2DCommand, RenderBridge, RenderCommand,
    RenderEvent, RenderRequestID, ResourceCommand, RuntimeMeshVertex, Sky3DState, SkyTime3DState,
    SpotLight3DState, Sprite2DCommand, Water2DState, Water3DState, WaterIdleModeState,
    WaterShapeState,
};
use perro_structs::{BitMask, PostProcessEffect, PostProcessSet};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::thread;
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

#[inline]
fn color(v: [f32; 4]) -> perro_structs::Color {
    v.into()
}

// Bench intent:
// - empty shows fixed acquire/submit/present cost.
// - retained rect/sprite/dense multimesh cases should stay close to empty.
// - post chain measures real full-frame pass cost, not CPU command cost.
// - sky_clouds cost is the sky_clear delta; keep it stable while tuning FBM count.
// Past regressions: sprites 100k hit ~9ms GPU prep; rects 100k hit ~112us CPU prep.
// Tried: branch-skipping night cloud FBM and hash-vector grad noise; both failed GPU timing here.
// Kept: cached sky noise for macro/low/high clouds; latest run cut sky_clouds gpuq ~337us -> ~269us.

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
    gpu_main: Duration,
    gpu_water: Duration,
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
        self.gpu_main += timing.gpu_timestamp_main;
        self.gpu_water += timing.gpu_timestamp_water;
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
            "{name:32} total={:>6}us wait={:>6}us gpuq={:>6}us water={:>5}us cpu_prep={:>5}us gpu2d={:>5}us gpu3d={:>5}us acquire={:>5}us encode={:>5}us submit={:>5}us post={:>5}us present={:>5}us dc2d={:>3} dc3d={:>3} inst3d={:>7}",
            Self::avg_us(self.total, frames),
            Self::avg_us(self.wait_idle, frames),
            Self::avg_us(self.gpu_main, frames),
            Self::avg_us(self.gpu_water, frames),
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

    fn append_csv(&self, path: &Path, name: &str) {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).expect("create gpu bench csv directory");
        }
        let write_header = fs::metadata(path).map_or(true, |meta| meta.len() == 0);
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("open gpu bench csv");
        if write_header {
            writeln!(
                file,
                "case,frames,total_us,wait_us,gpu_main_us,gpu_water_us,cpu_prepare_us,gpu_2d_us,gpu_3d_us,encode_us,submit_us,present_us,draw_calls_2d,draw_calls_3d,instances_3d"
            )
            .expect("write gpu bench csv header");
        }
        let frames = self.frames.max(1);
        writeln!(
            file,
            "{name},{frames},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            Self::avg_us(self.total, frames),
            Self::avg_us(self.wait_idle, frames),
            Self::avg_us(self.gpu_main, frames),
            Self::avg_us(self.gpu_water, frames),
            Self::avg_us(self.prepare_cpu, frames),
            Self::avg_us(self.gpu_prepare_2d, frames),
            Self::avg_us(self.gpu_prepare_3d, frames),
            Self::avg_us(self.encode, frames),
            Self::avg_us(self.submit, frames),
            Self::avg_us(self.present, frames),
            self.draw_calls_2d / frames,
            self.draw_calls_3d / frames,
            self.draw_instances_3d / frames,
        )
        .expect("write gpu bench csv row");
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
        let csv_path = env::var_os("PERRO_GPU_BENCH_CSV");
        let capture_ms = env::var("PERRO_GPU_CAPTURE_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        let throughput_mode = env::var_os("PERRO_GPU_BENCH_THROUGHPUT").is_some();
        for case in &self.cases {
            if let Ok(filter) = env::var("PERRO_GPU_BENCH")
                && !case.name.contains(&filter)
            {
                continue;
            }
            window.set_title(&format!("perro gpu bench - {}", case.name));
            let mut graphics = (case.setup)(&window);
            for _ in 0..WARMUP_FRAMES {
                (case.redraw)(&mut graphics);
                let _ = graphics.draw_frame_timed();
            }
            let mut sum = TimingSum::default();
            let batch_start = Instant::now();
            for _ in 0..SAMPLE_FRAMES {
                (case.redraw)(&mut graphics);
                if let Some(timing) = graphics.draw_frame_timed() {
                    sum.add(timing);
                    if !throughput_mode {
                        let wait_start = Instant::now();
                        graphics.wait_idle();
                        sum.add_wait_idle(wait_start.elapsed());
                    }
                }
            }
            if throughput_mode {
                graphics.wait_idle();
            }
            let batch_elapsed = batch_start.elapsed();
            sum.print(case.name);
            if throughput_mode {
                let fps = sum.frames as f64 / batch_elapsed.as_secs_f64().max(f64::EPSILON);
                println!(
                    "{case:32} throughput={fps:>9.1}fps batch={:>8}us",
                    batch_elapsed.as_micros(),
                    case = case.name,
                );
            }
            if let Some(path) = csv_path.as_deref() {
                sum.append_csv(Path::new(path), case.name);
            }
            if capture_ms > 0 {
                thread::sleep(Duration::from_millis(capture_ms));
            }
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
                name: "sprites_10k_same_z",
                setup: |w| setup_sprites(w, 10_000),
                redraw: redraw_2d,
            },
            BenchCase {
                name: "sprites_100k_same_z",
                setup: |w| setup_sprites(w, 100_000),
                redraw: redraw_2d,
            },
            BenchCase {
                name: "sprites_10k_unique_z",
                setup: |w| setup_sprites_unique_z(w, 10_000),
                redraw: redraw_2d,
            },
            BenchCase {
                name: "water_1_64",
                setup: |w| setup_water(w, 1, 64, 0),
                redraw: redraw_2d,
            },
            BenchCase {
                name: "water_16_64_i8",
                setup: |w| setup_water(w, 16, 64, 8),
                redraw: redraw_2d,
            },
            BenchCase {
                name: "water_64_128_i16",
                setup: |w| setup_water(w, 64, 128, 16),
                redraw: redraw_2d,
            },
            BenchCase {
                name: "water_128_256_i32",
                setup: |w| setup_water(w, 128, 256, 32),
                redraw: redraw_2d,
            },
            BenchCase {
                name: "water_sim_1_64",
                setup: |w| setup_water_sim(w, 1, 64, 0),
                redraw: redraw_3d,
            },
            BenchCase {
                name: "water_sim_16_64_i2",
                setup: |w| setup_water_sim(w, 16, 64, 2),
                redraw: redraw_3d,
            },
            BenchCase {
                name: "water_sim_64_128_i2",
                setup: |w| setup_water_sim(w, 64, 128, 2),
                redraw: redraw_3d,
            },
            BenchCase {
                name: "water_idle_calm_i2",
                setup: |w| setup_water_idle(w, WaterIdleModeState::Calm),
                redraw: redraw_2d,
            },
            BenchCase {
                name: "water_idle_sine_i2",
                setup: |w| setup_water_idle(w, WaterIdleModeState::Sine),
                redraw: redraw_2d,
            },
            BenchCase {
                name: "water_idle_chop_i2",
                setup: |w| setup_water_idle(w, WaterIdleModeState::Chop),
                redraw: redraw_2d,
            },
            BenchCase {
                name: "water_idle_storm_i2",
                setup: |w| setup_water_idle(w, WaterIdleModeState::Storm),
                redraw: redraw_2d,
            },
            BenchCase {
                name: "water_idle_river_i2",
                setup: |w| setup_water_idle(w, WaterIdleModeState::River),
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
            BenchCase {
                name: "post_bloom",
                setup: |w| {
                    setup_post(
                        w,
                        PostProcessEffect::Bloom {
                            strength: 0.7,
                            threshold: 0.8,
                            radius: 1.2,
                        },
                    )
                },
                redraw: redraw_2d,
            },
            BenchCase {
                name: "post_blur",
                setup: |w| setup_post(w, PostProcessEffect::Blur { strength: 2.0 }),
                redraw: redraw_2d,
            },
            BenchCase {
                name: "post_crt",
                setup: |w| {
                    setup_post(
                        w,
                        PostProcessEffect::Crt {
                            scanline_strength: 0.25,
                            curvature: 0.1,
                            chromatic: 0.5,
                            vignette: 0.2,
                        },
                    )
                },
                redraw: redraw_2d,
            },
            BenchCase {
                name: "sky_clear",
                setup: |w| setup_sky(w, 0.0),
                redraw: redraw_3d,
            },
            BenchCase {
                name: "sky_clouds",
                setup: |w| setup_sky(w, 0.6),
                redraw: redraw_3d,
            },
            BenchCase {
                name: "lights_point_8",
                setup: |w| setup_lit_meshes(w, 10_000, 8, 0),
                redraw: redraw_3d,
            },
            BenchCase {
                name: "lights_spot_8",
                setup: |w| setup_lit_meshes(w, 10_000, 0, 8),
                redraw: redraw_3d,
            },
            BenchCase {
                name: "overdraw_mesh_stack_2k",
                setup: |w| setup_overdraw_meshes(w, 2_000),
                redraw: redraw_3d,
            },
            BenchCase {
                name: "blend_stack_2k_smooth",
                setup: |w| setup_blend_stack(w, 2_000, 0.0),
                redraw: redraw_3d,
            },
            BenchCase {
                name: "blend_stack_2k_noise",
                setup: |w| setup_blend_stack(w, 2_000, 0.35),
                redraw: redraw_3d,
            },
            BenchCase {
                name: "blend_sphere_256_smooth",
                setup: |w| setup_blend_sphere_stack(w, 256),
                redraw: redraw_3d,
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

fn setup_sprites_unique_z(window: &Arc<Window>, count: u32) -> PerroGraphics {
    let mut graphics = base_graphics(window);
    let texture = create_texture(&mut graphics);
    graphics.submit_many((0..count).map(|i| sprite_command_z(i, texture, i as i32)));
    let _ = graphics.draw_frame_timed();
    graphics
}

fn setup_water(window: &Arc<Window>, count: u32, resolution: u32, impacts: u32) -> PerroGraphics {
    let mut graphics = base_graphics(window);
    graphics.submit_many(
        (0..count)
            .map(|i| water_command_with_idle(i, resolution, impacts, WaterIdleModeState::Chop)),
    );
    let _ = graphics.draw_frame_timed();
    graphics
}

fn setup_water_idle(window: &Arc<Window>, idle_mode: WaterIdleModeState) -> PerroGraphics {
    let mut graphics = base_graphics(window);
    graphics.submit(water_command_with_idle(0, 128, 2, idle_mode));
    let _ = graphics.draw_frame_timed();
    graphics
}

fn setup_water_sim(
    window: &Arc<Window>,
    count: u32,
    resolution: u32,
    impacts: u32,
) -> PerroGraphics {
    let mut graphics = base_graphics(window);
    graphics.submit_many((0..count).map(|i| water_sim_command(i, resolution, impacts)));
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

fn setup_post(window: &Arc<Window>, effect: PostProcessEffect) -> PerroGraphics {
    let mut graphics = setup_rects(window, 10_000);
    graphics.submit(RenderCommand::PostProcessing(
        PostProcessingCommand::SetGlobal(PostProcessSet::from_effects(vec![effect])),
    ));
    let _ = graphics.draw_frame_timed();
    graphics
}

fn setup_sky(window: &Arc<Window>, sky_variant: f32) -> PerroGraphics {
    let mut graphics = base_graphics(window);
    graphics.submit(sky_command(sky_variant));
    let _ = graphics.draw_frame_timed();
    graphics
}

fn setup_lit_meshes(
    window: &Arc<Window>,
    mesh_count: u32,
    point_count: u32,
    spot_count: u32,
) -> PerroGraphics {
    let mut graphics = base_graphics(window);
    let (mesh, material) = create_mesh_material(&mut graphics);
    graphics.submit(RenderCommand::ThreeD(Box::new(Command3D::SetRayLight {
        node: NodeID::from_parts(50_000, 0),
        light: RayLight3DState {
            direction: [-0.45, -0.85, -0.28],
            color: [1.0, 0.95, 0.9],
            intensity: 0.6,
            cast_shadows: false,
            shadow_strength: 0.82,
            shadow_depth_bias: 0.00018,
            shadow_normal_bias: 0.045,
        },
    })));
    graphics.submit_many((0..point_count).map(point_light_3d_command));
    graphics.submit_many((0..spot_count).map(spot_light_3d_command));
    graphics.submit_many((0..mesh_count).map(|i| draw_command(i, mesh, material)));
    let _ = graphics.draw_frame_timed();
    graphics
}

fn setup_overdraw_meshes(window: &Arc<Window>, count: u32) -> PerroGraphics {
    let mut graphics = base_graphics(window);
    let (mesh, material) = create_mesh_material(&mut graphics);
    graphics.submit(RenderCommand::ThreeD(Box::new(Command3D::SetRayLight {
        node: NodeID::from_parts(60_000, 0),
        light: RayLight3DState {
            direction: [-0.4, -0.8, -0.2],
            color: [1.0, 1.0, 1.0],
            intensity: 0.7,
            cast_shadows: false,
            shadow_strength: 0.82,
            shadow_depth_bias: 0.00018,
            shadow_normal_bias: 0.045,
        },
    })));
    graphics.submit_many((0..count).map(|i| draw_overdraw_command(i, mesh, material)));
    let _ = graphics.draw_frame_timed();
    graphics
}

fn setup_blend_stack(window: &Arc<Window>, count: u32, noise_factor: f32) -> PerroGraphics {
    let mut graphics = base_graphics(window);
    let (mesh, material) = create_mesh_material_with(&mut graphics, tiny_mesh());
    setup_blend_stack_scene(&mut graphics, mesh, material, count, noise_factor);
    graphics
}

fn setup_blend_sphere_stack(window: &Arc<Window>, count: u32) -> PerroGraphics {
    let mut graphics = base_graphics(window);
    let (mesh, material) = create_mesh_material_with(&mut graphics, uv_sphere_mesh(32, 16));
    setup_blend_stack_scene(&mut graphics, mesh, material, count, 0.0);
    graphics
}

fn setup_blend_stack_scene(
    graphics: &mut PerroGraphics,
    mesh: MeshID,
    material: MaterialID,
    count: u32,
    noise_factor: f32,
) {
    graphics.submit(RenderCommand::ThreeD(Box::new(Command3D::SetRayLight {
        node: NodeID::from_parts(61_000, 0),
        light: RayLight3DState {
            direction: [-0.4, -0.8, -0.2],
            color: [1.0, 1.0, 1.0],
            intensity: 0.7,
            cast_shadows: false,
            shadow_strength: 0.82,
            shadow_depth_bias: 0.00018,
            shadow_normal_bias: 0.045,
        },
    })));
    let blend = MeshBlendOptions3D {
        enabled: true,
        screen_blending: true,
        normal_blending: false,
        blend_layers: BitMask::with([1]),
        blend_mask: BitMask::with([1]),
        distance: 0.25,
        min_distance: 0.02,
        noise_factor,
        noise_scale: 8.0,
    };
    graphics.submit_many((0..count).map(|i| draw_overdraw_blend_command(i, mesh, material, blend)));
    let _ = graphics.draw_frame_timed();
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
            color: color([0.2, 0.7, 1.0, 1.0]),
            z_index: i as i32,
        },
    })
}

fn sprite_command(i: u32, texture: TextureID) -> RenderCommand {
    sprite_command_z(i, texture, 0)
}

fn sprite_command_z(i: u32, texture: TextureID, z_index: i32) -> RenderCommand {
    let [x, y] = grid2(i, 12.0);
    RenderCommand::TwoD(Command2D::UpsertSprite {
        node: NodeID::from_parts(i + 1, 0),
        sprite: Sprite2DCommand {
            texture,
            model: [[10.0, 0.0, 0.0], [0.0, 10.0, 0.0], [x, y, 1.0]],
            tint: color([1.0, 1.0, 1.0, 1.0]),
            uv_min: [0.0, 0.0],
            uv_max: [1.0, 1.0],
            uv_normalized: true,
            size: [10.0, 10.0],
            z_index,
        },
    })
}

fn water_command_with_idle(
    i: u32,
    resolution: u32,
    impacts: u32,
    idle_mode: WaterIdleModeState,
) -> RenderCommand {
    let [x, y] = grid2(i, 48.0);
    RenderCommand::TwoD(Command2D::UpsertWater {
        node: NodeID::from_parts(500_000 + i, 0),
        water: Box::new(Water2DState {
            model: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [x, y, 1.0]],
            z_index: i as i32,
            paused: false,
            simulation_time: 0.0,
            simulation_delta: 1.0 / 60.0,
            size: [44.0, 44.0],
            shape: WaterShapeState::Rect,
            resolution: [resolution, resolution],
            render_resolution: [resolution, resolution],
            depth: 4.0,
            flow: [0.12, 0.03],
            wind: [1.0, 0.2],
            idle_mode,
            wave_speed: 1.4,
            wave_scale: 1.2,
            wave_length: 18.0,
            damping: 0.985,
            wake_strength: 1.0,
            foam_strength: 0.7,
            sample_readback_rate: 30.0,
            lod_near_distance: 128.0,
            lod_mid_distance: 384.0,
            lod_far_distance: 896.0,
            lod_min_resolution: [32, 32],
            collision_layers: BitMask::with([1]),
            collision_mask: BitMask::NONE,
            deep_color: color([0.02, 0.16, 0.28, 0.86]),
            shallow_color: color([0.08, 0.46, 0.62, 0.48]),
            shallow_depth: -1.0,
            sky_bias_ratio: 0.0,
            transparency: 0.24,
            reflectivity: 0.46,
            roughness: 0.18,
            fresnel_power: 5.0,
            normal_strength: 1.15,
            ripple_scale: 1.0,
            foam_color: color([0.86, 0.96, 1.0, 1.0]),
            foam_amount: 0.72,
            crest_foam_threshold: 0.58,
            caustic_strength: 0.20,
            refraction_strength: 0.12,
            scattering_strength: 0.18,
            distance_fog_strength: 0.32,
            coastline_foam_color: color([0.9, 0.97, 1.0, 1.0]),
            coastline_foam_strength: if impacts > 0 { 0.75 } else { 0.0 },
            coastline_foam_width: 1.5,
            coastline_cutoff_softness: 0.25,
            coastline_wave_reflection: 0.45,
            coastline_wave_damping: 0.35,
            coastline_edge_noise: 0.2,
            debug: false,
            links: Arc::from([]),
            queries: Arc::from([]),
            impacts: (0..impacts)
                .map(|j| perro_render_bridge::WaterImpact2D {
                    position: [(j % 16) as f32 * 2.0, (j / 16) as f32 * 2.0],
                    velocity: [0.5, -2.5],
                    strength: 1.0 + j as f32 * 0.02,
                    radius: 2.0,
                    cavitation: 0.0,
                })
                .collect::<Vec<_>>()
                .into(),
            coastline_shapes: Arc::from([]),
        }),
    })
}

fn water_sim_command(i: u32, resolution: u32, impacts: u32) -> RenderCommand {
    let [x, y] = grid2(i, 48.0);
    RenderCommand::ThreeD(Box::new(Command3D::UpsertWater {
        node: NodeID::from_parts(600_000 + i, 0),
        water: Box::new(Water3DState {
            model: [
                [1.0, 0.0, 0.0, x],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, y],
                [0.0, 0.0, 0.0, 1.0],
            ],
            paused: false,
            simulation_time: 0.0,
            simulation_delta: 1.0 / 60.0,
            size: [44.0, 44.0],
            shape: WaterShapeState::Rect,
            resolution: [resolution, resolution],
            render_resolution: [resolution, resolution],
            depth: 4.0,
            flow: [0.12, 0.03],
            wind: [1.0, 0.2],
            idle_mode: WaterIdleModeState::Chop,
            wave_speed: 1.4,
            wave_scale: 1.2,
            wave_length: 18.0,
            damping: 0.985,
            wake_strength: 1.0,
            foam_strength: 0.7,
            sample_readback_rate: 0.0,
            lod_near_distance: 128.0,
            lod_mid_distance: 384.0,
            lod_far_distance: 896.0,
            lod_min_resolution: [32, 32],
            collision_layers: BitMask::with([1]),
            collision_mask: BitMask::NONE,
            deep_color: color([0.02, 0.16, 0.28, 0.86]),
            shallow_color: color([0.08, 0.46, 0.62, 0.48]),
            shallow_depth: -1.0,
            sky_bias_ratio: 0.35,
            transparency: 0.24,
            reflectivity: 0.46,
            roughness: 0.18,
            fresnel_power: 5.0,
            normal_strength: 1.15,
            ripple_scale: 1.0,
            foam_color: color([0.86, 0.96, 1.0, 1.0]),
            foam_amount: 0.72,
            crest_foam_threshold: 0.58,
            caustic_strength: 0.20,
            refraction_strength: 0.12,
            scattering_strength: 0.18,
            distance_fog_strength: 0.32,
            coastline_foam_color: color([0.9, 0.97, 1.0, 1.0]),
            coastline_foam_strength: if impacts > 0 { 0.75 } else { 0.0 },
            coastline_foam_width: 1.5,
            coastline_cutoff_softness: 0.25,
            coastline_wave_reflection: 0.45,
            coastline_wave_damping: 0.35,
            coastline_edge_noise: 0.2,
            debug: false,
            links: Arc::from([]),
            queries: Arc::from([]),
            impacts: (0..impacts)
                .map(|j| perro_render_bridge::WaterImpact3D {
                    position: [(j % 16) as f32 * 2.0, 0.0, (j / 16) as f32 * 2.0],
                    velocity: [0.5, -2.5, 0.0],
                    strength: 1.0 + j as f32 * 0.02,
                    radius: 2.0,
                    cavitation: 0.0,
                })
                .collect::<Vec<_>>()
                .into(),
            coastline_shapes: Arc::from([]),
        }),
    }))
}

fn draw_command(i: u32, mesh: MeshID, material: MaterialID) -> RenderCommand {
    RenderCommand::ThreeD(Box::new(Command3D::Draw {
        mesh,
        surfaces: surface(material),
        node: NodeID::from_parts(i + 1, 0),
        model: model_3d(i),
        skeleton: None,
        blend_shape_weights: Arc::from([]),
        meshlet_override: None,
        lod: LODOptions3D::default(),
        blend: MeshBlendOptions3D::default(),
        cast_shadows: true,
        receive_shadows: true,
    }))
}

fn draw_overdraw_command(i: u32, mesh: MeshID, material: MaterialID) -> RenderCommand {
    RenderCommand::ThreeD(Box::new(Command3D::Draw {
        mesh,
        surfaces: surface(material),
        node: NodeID::from_parts(i + 1, 1),
        model: [
            [16.0, 0.0, 0.0, 0.0],
            [0.0, 16.0, 0.0, 0.0],
            [0.0, 0.0, 16.0, 0.02 * i as f32],
            [0.0, 0.0, 0.0, 1.0],
        ],
        skeleton: None,
        blend_shape_weights: Arc::from([]),
        meshlet_override: None,
        lod: LODOptions3D::default(),
        blend: MeshBlendOptions3D::default(),
        cast_shadows: true,
        receive_shadows: true,
    }))
}

fn draw_overdraw_blend_command(
    i: u32,
    mesh: MeshID,
    material: MaterialID,
    blend: MeshBlendOptions3D,
) -> RenderCommand {
    RenderCommand::ThreeD(Box::new(Command3D::Draw {
        mesh,
        surfaces: surface(material),
        node: NodeID::from_parts(100_000 + i, 1),
        model: [
            [16.0, 0.0, 0.0, 0.0],
            [0.0, 16.0, 0.0, 0.0],
            [0.0, 0.0, 16.0, 0.02 * i as f32],
            [0.0, 0.0, 0.0, 1.0],
        ],
        skeleton: None,
        blend_shape_weights: Arc::from([]),
        meshlet_override: None,
        lod: LODOptions3D::default(),
        blend,
        cast_shadows: true,
        receive_shadows: true,
    }))
}

fn point_light_3d_command(i: u32) -> RenderCommand {
    RenderCommand::ThreeD(Box::new(Command3D::SetPointLight {
        node: NodeID::from_parts(70_000 + i, 0),
        light: PointLight3DState {
            position: [(i % 4) as f32 * 2.0 - 4.0, 3.0, (i / 4) as f32 * 2.0 - 4.0],
            color: [1.0, 0.8, 0.55],
            intensity: 12.0,
            range: 8.0,
            cast_shadows: false,
            shadow_strength: 0.82,
            shadow_depth_bias: 0.00018,
            shadow_normal_bias: 0.045,
        },
    }))
}

fn spot_light_3d_command(i: u32) -> RenderCommand {
    RenderCommand::ThreeD(Box::new(Command3D::SetSpotLight {
        node: NodeID::from_parts(80_000 + i, 0),
        light: SpotLight3DState {
            position: [(i % 4) as f32 * 2.0 - 4.0, 6.0, (i / 4) as f32 * 2.0 - 4.0],
            direction: [0.0, -1.0, 0.0],
            color: [0.7, 0.85, 1.0],
            intensity: 18.0,
            range: 10.0,
            inner_angle_radians: 0.45,
            outer_angle_radians: 0.9,
            cast_shadows: false,
            shadow_strength: 0.82,
            shadow_depth_bias: 0.00018,
            shadow_normal_bias: 0.045,
        },
    }))
}

fn sky_command(_sky_variant: f32) -> RenderCommand {
    RenderCommand::ThreeD(Box::new(Command3D::SetSky {
        node: NodeID::from_parts(90_000, 0),
        sky: Box::new(Sky3DState {
            day_colors: Arc::from([[0.42, 0.7, 1.0], [0.1, 0.35, 0.8]]),
            evening_colors: Arc::from([[1.0, 0.45, 0.2], [0.25, 0.08, 0.3]]),
            night_colors: Arc::from([[0.02, 0.03, 0.08], [0.0, 0.0, 0.02]]),
            horizon_colors: Arc::from([[0.55, 0.57, 0.60], [0.35, 0.36, 0.38]]),
            time: SkyTime3DState {
                time_of_day: 0.5,
                paused: true,
                scale: 1.0,
            },
            shaders: Arc::from([]),
            environment: None,
        }),
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
            scale: [1.0, 1.0, 1.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            has_blend_shape_weight_override: false,
            blend_shape_weights: Arc::from([]),
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
        blend_shape_weights: Arc::from([]),
        meshlet_override: None,
        lod: LODOptions3D::default(),
        blend: MeshBlendOptions3D::default(),
        cast_shadows: true,
        receive_shadows: true,
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
        modulate: perro_structs::Color::WHITE,
    }])
}

fn tiny_mesh() -> Mesh3D {
    Mesh3D {
        vertices: vec![
            RuntimeMeshVertex {
                position: [0.0, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 0.0],
                paint_uv: [0.0, 0.0],
                joints: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0].into(),
            },
            RuntimeMeshVertex {
                position: [1.0, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [1.0, 0.0],
                paint_uv: [1.0, 0.0],
                joints: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0].into(),
            },
            RuntimeMeshVertex {
                position: [0.0, 1.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 1.0],
                paint_uv: [0.0, 1.0],
                joints: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0].into(),
            },
        ],
        indices: vec![0, 1, 2],
        surface_ranges: vec![],
        blend_shapes: vec![],
    }
}

fn uv_sphere_mesh(slices: u32, stacks: u32) -> Mesh3D {
    let slices = slices.max(3);
    let stacks = stacks.max(2);
    let mut vertices = Vec::with_capacity(((slices + 1) * (stacks + 1)) as usize);
    let mut indices = Vec::with_capacity((slices * stacks * 6) as usize);
    for y in 0..=stacks {
        let v = y as f32 / stacks as f32;
        let theta = v * std::f32::consts::PI;
        let sin_theta = theta.sin();
        let cos_theta = theta.cos();
        for x in 0..=slices {
            let u = x as f32 / slices as f32;
            let phi = u * std::f32::consts::TAU;
            let normal = [sin_theta * phi.cos(), cos_theta, sin_theta * phi.sin()];
            vertices.push(RuntimeMeshVertex {
                position: [normal[0] * 0.5, normal[1] * 0.5, normal[2] * 0.5],
                normal,
                uv: [u, v],
                paint_uv: [u, v],
                joints: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0].into(),
            });
        }
    }
    let row = slices + 1;
    for y in 0..stacks {
        for x in 0..slices {
            let a = y * row + x;
            let b = a + row;
            indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    Mesh3D {
        vertices,
        indices,
        surface_ranges: vec![],
        blend_shapes: vec![],
    }
}

fn create_texture(graphics: &mut PerroGraphics) -> TextureID {
    graphics.submit(RenderCommand::Resource(
        ResourceCommand::CreateRuntimeTexture {
            request: RenderRequestID::new(1),
            id: TextureID::nil(),
            source: "runtime://gpu-frame-bench".to_string(),
            reserved: true,
            width: 1,
            height: 1,
            rgba: Arc::from([255, 255, 255, 255]),
        },
    ));
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
    create_mesh_material_with(graphics, tiny_mesh())
}

fn create_mesh_material_with(
    graphics: &mut PerroGraphics,
    mesh_data: Mesh3D,
) -> (MeshID, MaterialID) {
    graphics.submit_many([
        RenderCommand::Resource(ResourceCommand::CreateRuntimeMesh {
            request: RenderRequestID::new(2),
            id: MeshID::nil(),
            source: "__bench_mesh__".to_string(),
            reserved: true,
            mesh: mesh_data,
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
