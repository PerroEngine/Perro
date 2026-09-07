use super::*;

#[path = "lod_mesh.rs"]
mod lod_mesh;
static LOD_GPU_LOOKUP_ENABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn lod_gpu_lookup(hash: u64) -> &'static [u8] {
    // CPU Mesh3D intentionally has no baked-LOD table. Leave its builtin-style
    // source unmaterialized during registration so the GPU resolves baked bytes.
    if LOD_GPU_LOOKUP_ENABLED.load(Ordering::Acquire) {
        lod_mesh::lod_test_mesh_lookup(hash)
    } else {
        &[]
    }
}

fn placed_draw(index: u32, mesh: MeshID, material: MaterialID, offset: f32) -> RenderCommand {
    let mut command = draw_command(index, mesh, material);
    if let RenderCommand::ThreeD(command) = &mut command
        && let Command3D::Draw {
            model,
            cast_shadows,
            ..
        } = command.as_mut()
    {
        *model = glam::Mat4::from_scale_rotation_translation(
            glam::Vec3::splat(0.15),
            glam::Quat::IDENTITY,
            glam::Vec3::new(
                (index % 100) as f32 * 0.25 - 12.5 + offset,
                (index / 100) as f32 * 0.25 - 12.5,
                -8.0,
            ),
        )
        .to_cols_array_2d();
        *cast_shadows = false;
    }
    command
}

pub fn setup_lod_mixed(window: &Arc<Window>) -> PerroGraphics {
    LOD_GPU_LOOKUP_ENABLED.store(false, Ordering::Release);
    let mut graphics = base_graphics(window).with_static_mesh_lookup(lod_gpu_lookup);
    let (mesh, material) = create_mesh_material(&mut graphics);
    graphics.submit(RenderCommand::Resource(Box::new(
        ResourceCommand::CreateMesh {
            request: RenderRequestID::new(9),
            id: MeshID::nil(),
            source: lod_mesh::LOD_MESH_SOURCE.into(),
            reserved: true,
        },
    )));
    let mut events = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(30);
    let lod = loop {
        graphics.draw_frame();
        graphics.drain_events(&mut events);
        let mut result = None;
        for event in events.drain(..) {
            match event {
                RenderEvent::MeshCreated { id, mesh: None, .. } => {
                    result = Some(id);
                }
                RenderEvent::Failed { reason, .. } => panic!("LOD mesh fixture: {reason}"),
                _ => {}
            }
        }
        if let Some(id) = result {
            break id;
        }
        assert!(Instant::now() < deadline, "LOD mesh fixture timeout");
        thread::yield_now();
    };
    LOD_GPU_LOOKUP_ENABLED.store(true, Ordering::Release);
    graphics.submit_many(
        (0..10_000).map(|i| placed_draw(i, if i % 100 == 0 { lod } else { mesh }, material, 0.0)),
    );
    redraw_lod_camera(&mut graphics);
    graphics.draw_frame();
    graphics
}

pub fn redraw_lod_camera(graphics: &mut PerroGraphics) {
    static TICK: AtomicU32 = AtomicU32::new(0);
    let shift = (TICK.fetch_add(1, Ordering::Relaxed) % 2) as f32 * 0.01;
    graphics.submit(RenderCommand::ThreeD(Box::new(Command3D::SetCamera {
        camera: Camera3DState {
            position: [shift, 0.0, 0.0],
            ..Camera3DState::default()
        },
    })));
}

pub fn setup_sparse_transforms(window: &Arc<Window>, count: u32) -> PerroGraphics {
    let mut graphics = base_graphics(window);
    let (mesh, material) = create_mesh_material(&mut graphics);
    RESOURCE_CASE_IDS.with(|ids| ids.set(Some((mesh, material))));
    graphics.submit_many((0..count).map(|i| placed_draw(i, mesh, material, 0.0)));
    graphics.draw_frame();
    graphics
}

pub fn redraw_sparse_transform(graphics: &mut PerroGraphics) {
    static TICK: AtomicU32 = AtomicU32::new(0);
    let shift = (TICK.fetch_add(1, Ordering::Relaxed) % 2) as f32 * 0.01;
    let (mesh, material) =
        RESOURCE_CASE_IDS.with(|ids| ids.get().expect("sparse fixture resources"));
    graphics.submit(placed_draw(500, mesh, material, shift));
}

fn ui_shape(index: u32, shade: f32) -> RenderCommand {
    use perro_render_bridge::{UiCommand, UiRectState};
    RenderCommand::Ui(Box::new(UiCommand::UpsertShape {
        node: NodeID::from_parts(700_000 + index, 0),
        rect: UiRectState {
            center: [
                (index % 50) as f32 * 24.0 - 588.0,
                (index / 50) as f32 * 30.0 - 285.0,
            ],
            size: [20.0, 20.0],
            pivot: [0.5, 0.5],
            rotation_radians: 0.0,
            z_index: index as i32,
        },
        clip_rect: [0.0, 0.0, WIDTH as f32, HEIGHT as f32],
        kind: perro_ui::UiShapeKind::Circle,
        fill: [shade, 0.5, 0.75, 1.0],
        stroke: [0.0; 4],
        stroke_width: 0.0,
    }))
}

pub fn setup_sparse_ui(window: &Arc<Window>) -> PerroGraphics {
    let mut graphics = base_graphics(window);
    graphics.submit_many((0..1_000).map(|i| ui_shape(i, 0.5)));
    graphics.draw_frame();
    graphics
}

pub fn redraw_sparse_ui(graphics: &mut PerroGraphics) {
    static TICK: AtomicU32 = AtomicU32::new(0);
    let shade = 0.5 + (TICK.fetch_add(1, Ordering::Relaxed) % 2) as f32 * 0.25;
    graphics.submit(ui_shape(500, shade));
}

pub fn redraw_all_ui(graphics: &mut PerroGraphics) {
    static TICK: AtomicU32 = AtomicU32::new(0);
    let shade = 0.5 + (TICK.fetch_add(1, Ordering::Relaxed) % 2) as f32 * 0.25;
    graphics.submit_many((0..1_000).map(|i| ui_shape(i, shade)));
}

pub fn setup_water_tiers(window: &Arc<Window>, mode: u32) -> PerroGraphics {
    use perro_structs::WaterQuality;
    let mut graphics = base_graphics(window);
    let mut cells = 0_u64;
    let mut max_cells = 0_u64;
    for i in 0..32 {
        let quality = if mode == 2 || (mode == 1 && i == 0) {
            WaterQuality::Ultra
        } else {
            WaterQuality::Low
        };
        let [x, y] = quality.sim_resolution();
        cells += u64::from(x * y);
        max_cells = max_cells.max(u64::from(x * y));
        let mut command = water_command_with_idle(i, 0, 0, WaterIdleModeState::Chop);
        if let RenderCommand::TwoD(Command2D::UpsertWater { water, .. }) = &mut command {
            water.quality = quality;
            water.sample_readback_rate = 0.0;
        }
        graphics.submit(command);
    }
    eprintln!(
        "water_work mode={mode} waters=32 active_cells={cells} rectangular_dispatch_cells={} excess_cells={}",
        max_cells * 32,
        max_cells * 32 - cells
    );
    graphics.draw_frame();
    graphics
}
