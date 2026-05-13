use perro_graphics::{DrawFrameTiming, GraphicsBackend, PerroGraphics};
use perro_ids::{MaterialID, MeshID, NodeID, TextureID};
use perro_render_bridge::{
    Camera2DState, Camera3DState, Command2D, Command3D, DenseInstancePose3D, LODOptions3D,
    Material3D, Mesh3D, MeshSurfaceBinding3D, PointLight3DState, PostProcessingCommand,
    RayLight3DState, Rect2DCommand, RenderBridge, RenderCommand, RenderEvent, RenderRequestID,
    ResourceCommand, RuntimeMeshVertex, Sky3DState, SkyTime3DState, SpotLight3DState,
    Sprite2DCommand, Water2DState, Water3DState, WaterIdleModeState,
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

fn setup_sky(window: &Arc<Window>, cloud_density: f32) -> PerroGraphics {
    let mut graphics = base_graphics(window);
    graphics.submit(sky_command(cloud_density));
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
        },
    })));
    graphics.submit_many((0..count).map(|i| draw_overdraw_command(i, mesh, material)));
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
            size: [44.0, 44.0],
            resolution: [resolution, resolution],
            depth: 4.0,
            flow: [0.12, 0.03],
            wind: [1.0, 0.2],
            idle_mode,
            wave_speed: 1.4,
            wave_scale: 1.2,
            damping: 0.985,
            wake_strength: 1.0,
            foam_strength: 0.7,
            sample_readback_rate: 30.0,
            lod_near_distance: 128.0,
            lod_mid_distance: 384.0,
            lod_far_distance: 896.0,
            lod_min_resolution: [32, 32],
            shoreline_mask: impacts > 0,
            static_body_wakes: true,
            debug: false,
            impacts: (0..impacts)
                .map(|j| perro_render_bridge::WaterImpact2D {
                    position: [(j % 16) as f32 * 2.0, (j / 16) as f32 * 2.0],
                    velocity: [0.5, -2.5],
                    strength: 1.0 + j as f32 * 0.02,
                    radius: 2.0,
                })
                .collect::<Vec<_>>()
                .into(),
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
            size: [44.0, 44.0],
            resolution: [resolution, resolution],
            depth: 4.0,
            flow: [0.12, 0.03],
            wind: [1.0, 0.2],
            idle_mode: WaterIdleModeState::Chop,
            wave_speed: 1.4,
            wave_scale: 1.2,
            damping: 0.985,
            wake_strength: 1.0,
            foam_strength: 0.7,
            sample_readback_rate: 0.0,
            lod_near_distance: 128.0,
            lod_mid_distance: 384.0,
            lod_far_distance: 896.0,
            lod_min_resolution: [32, 32],
            shoreline_mask: impacts > 0,
            static_body_wakes: true,
            debug: false,
            impacts: (0..impacts)
                .map(|j| perro_render_bridge::WaterImpact3D {
                    position: [(j % 16) as f32 * 2.0, 0.0, (j / 16) as f32 * 2.0],
                    velocity: [0.5, -2.5, 0.0],
                    strength: 1.0 + j as f32 * 0.02,
                    radius: 2.0,
                })
                .collect::<Vec<_>>()
                .into(),
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
        meshlet_override: None,
        lod: LODOptions3D::default(),
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
        meshlet_override: None,
        lod: LODOptions3D::default(),
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
        },
    }))
}

fn sky_command(cloud_density: f32) -> RenderCommand {
    RenderCommand::ThreeD(Box::new(Command3D::SetSky {
        node: NodeID::from_parts(90_000, 0),
        sky: Box::new(Sky3DState {
            day_colors: Arc::from([[0.42, 0.7, 1.0], [0.1, 0.35, 0.8]]),
            evening_colors: Arc::from([[1.0, 0.45, 0.2], [0.25, 0.08, 0.3]]),
            night_colors: Arc::from([[0.02, 0.03, 0.08], [0.0, 0.0, 0.02]]),
            sky_angle: 0.0,
            time: SkyTime3DState {
                time_of_day: 0.5,
                paused: true,
                scale: 1.0,
            },
            cloud_size: 0.7,
            cloud_density,
            cloud_variance: 0.4,
            cloud_wind_vector: [0.2, 0.05],
            star_size: 0.7,
            star_scatter: 0.5,
            star_gleam: 0.4,
            moon_size: 0.12,
            sun_size: 0.08,
            style_blend: 1.0,
            sky_shader: None,
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
