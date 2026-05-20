use super::{GC_INTERVAL_FRAMES, PerroGraphics};
use crate::backend::GraphicsBackend;
use crate::three_d::renderer::Draw3DKind;
use perro_ids::{MaterialID, MeshID, NodeID, TextureID};
use perro_render_bridge::{
    Camera3DState, CameraProjectionState, Command2D, Command3D, LODOptions3D, Material3D,
    MeshSurfaceBinding3D, PostProcessingCommand, Rect2DCommand, RenderBridge, RenderCommand,
    ResourceCommand, Sprite2DCommand, VisualAccessibilityCommand, Water2DState, Water3DState,
    WaterIdleModeState, WaterLinkState, WaterShapeState,
};
use perro_structs::{BitMask, Color, ColorBlindFilter, PostProcessEffect, PostProcessSet};
use std::sync::Arc;

fn surfaces_for(material: MaterialID) -> Arc<[MeshSurfaceBinding3D]> {
    Arc::from([MeshSurfaceBinding3D {
        material: Some(material),
        overrides: Arc::from([]),
        modulate: Color::WHITE,
    }])
}

fn rect_command() -> Rect2DCommand {
    Rect2DCommand {
        center: [0.0, 0.0],
        size: [8.0, 8.0],
        color: Color::WHITE,
        z_index: 0,
    }
}

#[test]
fn draw_frame_drains_pending_commands_in_one_pass() {
    let mut graphics = PerroGraphics::new();
    for i in 0..65 {
        graphics.submit(RenderCommand::TwoD(Command2D::UpsertRect {
            node: NodeID::from_parts(i, 0),
            rect: rect_command(),
        }));
    }

    graphics.draw_frame();

    assert!(graphics.frame.pending_commands.is_empty());
    assert_eq!(graphics.renderer_2d.retained_rects().len(), 65);
}

#[test]
fn auto_gc_runs_on_interval_not_every_frame() {
    let mut graphics = PerroGraphics::new();
    let texture = graphics
        .resources
        .create_texture("__tmp_gc_interval_texture__", false);
    graphics.resources.mark_texture_used(texture);
    graphics.resources.reset_ref_counts();

    for frame in 0..(GC_INTERVAL_FRAMES - 1) {
        graphics.submit(RenderCommand::TwoD(Command2D::UpsertRect {
            node: NodeID::from_parts(10_000 + frame, 0),
            rect: rect_command(),
        }));
        graphics.draw_frame();
    }

    assert!(graphics.resources.has_texture(texture));

    graphics.submit(RenderCommand::TwoD(Command2D::UpsertRect {
        node: NodeID::from_parts(20_000, 0),
        rect: rect_command(),
    }));
    graphics.draw_frame();

    assert!(!graphics.resources.has_texture(texture));
}

fn water_2d_state() -> Water2DState {
    Water2DState {
        model: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        z_index: 0,
        paused: false,
        simulation_time: 0.0,
        simulation_delta: 1.0 / 60.0,
        size: [32.0, 32.0],
        shape: WaterShapeState::Rect,
        resolution: [64, 64],
        render_resolution: [128, 128],
        depth: 4.0,
        flow: [0.0, 0.0],
        wind: [1.0, 0.0],
        idle_mode: WaterIdleModeState::Calm,
        wave_speed: 1.0,
        wave_scale: 1.0,
        wave_length: 18.0,
        damping: 0.985,
        wake_strength: 1.35,
        foam_strength: 0.9,
        sample_readback_rate: 30.0,
        lod_near_distance: 128.0,
        lod_mid_distance: 384.0,
        lod_far_distance: 896.0,
        lod_min_resolution: [32, 32],
        collision_layers: BitMask::with([1]),
        collision_mask: BitMask::NONE,
        deep_color: Color::new(0.02, 0.16, 0.28, 0.94),
        shallow_color: Color::new(0.08, 0.46, 0.62, 0.74),
        shallow_depth: -1.0,
        sky_bias_ratio: 0.0,
        transparency: 0.24,
        reflectivity: 0.46,
        roughness: 0.18,
        fresnel_power: 5.0,
        normal_strength: 1.15,
        ripple_scale: 1.0,
        foam_color: Color::new(0.86, 0.96, 1.0, 1.0),
        foam_amount: 0.72,
        crest_foam_threshold: 0.58,
        caustic_strength: 0.20,
        refraction_strength: 0.12,
        scattering_strength: 0.18,
        distance_fog_strength: 0.32,
        coastline_foam_color: Color::new(0.9, 0.97, 1.0, 1.0),
        coastline_foam_strength: 0.75,
        coastline_foam_width: 1.5,
        coastline_cutoff_softness: 0.25,
        coastline_wave_reflection: 0.45,
        coastline_wave_damping: 0.35,
        coastline_edge_noise: 0.2,
        debug: false,
        links: Arc::from([WaterLinkState {
            other: NodeID::from_parts(31, 0),
            overlap_min: [-2.0, -1.0],
            overlap_max: [2.0, 1.0],
            blend_width: 1.0,
            wave_transfer: 0.75,
            flow_transfer: 0.5,
        }]),
        queries: Arc::from([]),
        impacts: Arc::from([]),
        coastline_shapes: Arc::from([]),
    }
}

fn water_3d_state() -> Water3DState {
    Water3DState {
        model: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
        paused: false,
        simulation_time: 0.0,
        simulation_delta: 1.0 / 60.0,
        size: [32.0, 32.0],
        shape: WaterShapeState::Rect,
        resolution: [64, 64],
        render_resolution: [128, 128],
        depth: 4.0,
        flow: [0.0, 0.0],
        wind: [1.0, 0.0],
        idle_mode: WaterIdleModeState::Calm,
        wave_speed: 1.0,
        wave_scale: 1.0,
        wave_length: 18.0,
        damping: 0.985,
        wake_strength: 1.35,
        foam_strength: 0.9,
        sample_readback_rate: 30.0,
        lod_near_distance: 128.0,
        lod_mid_distance: 384.0,
        lod_far_distance: 896.0,
        lod_min_resolution: [32, 32],
        collision_layers: BitMask::with([1]),
        collision_mask: BitMask::NONE,
        deep_color: Color::new(0.02, 0.16, 0.28, 0.94),
        shallow_color: Color::new(0.08, 0.46, 0.62, 0.74),
        shallow_depth: -1.0,
        sky_bias_ratio: 0.0,
        transparency: 0.24,
        reflectivity: 0.46,
        roughness: 0.18,
        fresnel_power: 5.0,
        normal_strength: 1.15,
        ripple_scale: 1.0,
        foam_color: Color::new(0.86, 0.96, 1.0, 1.0),
        foam_amount: 0.72,
        crest_foam_threshold: 0.58,
        caustic_strength: 0.20,
        refraction_strength: 0.12,
        scattering_strength: 0.18,
        distance_fog_strength: 0.32,
        coastline_foam_color: Color::new(0.9, 0.97, 1.0, 1.0),
        coastline_foam_strength: 0.75,
        coastline_foam_width: 1.5,
        coastline_cutoff_softness: 0.25,
        coastline_wave_reflection: 0.45,
        coastline_wave_damping: 0.35,
        coastline_edge_noise: 0.2,
        debug: false,
        links: Arc::from([WaterLinkState {
            other: NodeID::from_parts(32, 0),
            overlap_min: [-2.0, -1.0],
            overlap_max: [2.0, 1.0],
            blend_width: 1.0,
            wave_transfer: 0.75,
            flow_transfer: 0.5,
        }]),
        queries: Arc::from([]),
        impacts: Arc::from([]),
        coastline_shapes: Arc::from([]),
    }
}

#[test]
fn water_upsert_retains_and_remove_clears_state() {
    let mut graphics = PerroGraphics::new();
    let water_2d = NodeID::from_parts(21, 0);
    let water_3d = NodeID::from_parts(22, 0);

    graphics.submit(RenderCommand::TwoD(Command2D::UpsertWater {
        node: water_2d,
        water: Box::new(water_2d_state()),
    }));
    graphics.submit(RenderCommand::ThreeD(Box::new(Command3D::UpsertWater {
        node: water_3d,
        water: Box::new(water_3d_state()),
    })));
    graphics.draw_frame();

    assert_eq!(graphics.renderer_2d.retained_water_count(), 1);
    assert_eq!(graphics.renderer_3d.retained_waters_sorted().len(), 1);
    assert_eq!(
        graphics
            .renderer_2d
            .retained_waters()
            .next()
            .expect("2d water should be retained")
            .1
            .links
            .len(),
        1
    );
    assert_eq!(
        graphics.renderer_3d.retained_waters_sorted()[0]
            .1
            .links
            .len(),
        1
    );

    graphics.submit(RenderCommand::TwoD(Command2D::RemoveNode {
        node: water_2d,
    }));
    graphics.submit(RenderCommand::ThreeD(Box::new(Command3D::RemoveNode {
        node: water_3d,
    })));
    graphics.draw_frame();

    assert_eq!(graphics.renderer_2d.retained_water_count(), 0);
    assert_eq!(graphics.renderer_3d.retained_waters_sorted().len(), 0);
}

#[test]
fn sprite_texture_upsert_is_accepted_after_texture_creation() {
    let mut graphics = PerroGraphics::new();
    let request = perro_render_bridge::RenderRequestID::new(99);
    let node = NodeID::from_parts(1, 0);

    graphics.submit(RenderCommand::Resource(ResourceCommand::CreateTexture {
        request,
        id: TextureID::nil(),
        source: "__default__".to_string(),
        reserved: false,
    }));
    graphics.draw_frame();

    let mut events = Vec::new();
    graphics.drain_events(&mut events);
    let created = events
        .into_iter()
        .find_map(|event| match event {
            perro_render_bridge::RenderEvent::TextureCreated { id, .. } => Some(id),
            _ => None,
        })
        .expect("texture creation event should exist");

    graphics.submit(RenderCommand::TwoD(Command2D::UpsertSprite {
        node,
        sprite: Sprite2DCommand {
            texture: created,
            model: [[1.0, 0.0, 10.0], [0.0, 1.0, 5.0], [0.0, 0.0, 1.0]],
            tint: Color::WHITE,
            uv_min: [0.0, 0.0],
            uv_max: [0.0, 0.0],
            size: [0.0, 0.0],
            z_index: 2,
        },
    }));
    graphics.draw_frame();

    assert_eq!(
        graphics.renderer_2d.retained_sprite(node),
        Some(Sprite2DCommand {
            texture: created,
            model: [[1.0, 0.0, 10.0], [0.0, 1.0, 5.0], [0.0, 0.0, 1.0]],
            tint: Color::WHITE,
            uv_min: [0.0, 0.0],
            uv_max: [0.0, 0.0],
            size: [0.0, 0.0],
            z_index: 2,
        })
    );
}

#[test]
fn draw_3d_updates_retained_state_per_node() {
    let mut graphics = PerroGraphics::new();
    let node_a = NodeID::from_parts(10, 0);
    let node_b = NodeID::from_parts(11, 0);

    graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMesh {
        request: perro_render_bridge::RenderRequestID::new(1001),
        id: MeshID::nil(),
        source: "__cube__".to_string(),
        reserved: false,
    }));
    graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMaterial {
        request: perro_render_bridge::RenderRequestID::new(1002),
        id: MaterialID::nil(),
        material: Material3D::default(),
        source: None,
        reserved: false,
    }));
    graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMesh {
        request: perro_render_bridge::RenderRequestID::new(1003),
        id: MeshID::nil(),
        source: "__sphere__".to_string(),
        reserved: false,
    }));
    graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMaterial {
        request: perro_render_bridge::RenderRequestID::new(1004),
        id: MaterialID::nil(),
        material: Material3D::default(),
        source: None,
        reserved: false,
    }));
    graphics.draw_frame();

    let mut events = Vec::new();
    graphics.drain_events(&mut events);
    let mut created_meshes = Vec::new();
    let mut created_materials = Vec::new();
    for event in events {
        match event {
            perro_render_bridge::RenderEvent::MeshCreated { id, .. } => created_meshes.push(id),
            perro_render_bridge::RenderEvent::MaterialCreated { id, .. } => {
                created_materials.push(id)
            }
            _ => {}
        }
    }
    assert_eq!(created_meshes.len(), 2);
    assert_eq!(created_materials.len(), 2);

    let model_a = [
        [1.0, 0.0, 0.0, 2.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let model_b = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 3.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];

    graphics.submit(RenderCommand::ThreeD(Box::new(Command3D::Draw {
        mesh: created_meshes[0],
        surfaces: surfaces_for(created_materials[0]),
        node: node_a,
        model: model_a,
        skeleton: None,
        meshlet_override: None,
        lod: LODOptions3D::default(),
        blend: Default::default(),
    })));
    graphics.submit(RenderCommand::ThreeD(Box::new(Command3D::Draw {
        mesh: created_meshes[1],
        surfaces: surfaces_for(created_materials[1]),
        node: node_b,
        model: model_b,
        skeleton: None,
        meshlet_override: None,
        lod: LODOptions3D::default(),
        blend: Default::default(),
    })));
    graphics.draw_frame();

    assert_eq!(graphics.renderer_3d.retained_draw_count(), 2);
    assert_eq!(
        graphics.renderer_3d.retained_draw(node_a),
        Some(crate::three_d::renderer::Draw3DInstance {
            node: node_a,
            kind: Draw3DKind::Mesh(created_meshes[0]),
            surfaces: surfaces_for(created_materials[0]),
            instance_mats: Arc::from([model_a]),
            skeleton: None,
            dense_multimesh: None,
            meshlet_override: None,
            lod: LODOptions3D::default(),
            blend: Default::default(),
            debug_color: None,
        })
    );
    assert_eq!(
        graphics.renderer_3d.retained_draw(node_b),
        Some(crate::three_d::renderer::Draw3DInstance {
            node: node_b,
            kind: Draw3DKind::Mesh(created_meshes[1]),
            surfaces: surfaces_for(created_materials[1]),
            instance_mats: Arc::from([model_b]),
            skeleton: None,
            dense_multimesh: None,
            meshlet_override: None,
            lod: LODOptions3D::default(),
            blend: Default::default(),
            debug_color: None,
        })
    );
}

#[test]
fn draw_multi_3d_retains_all_instance_mats() {
    let mut graphics = PerroGraphics::new();
    let node = NodeID::from_parts(12, 0);

    graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMesh {
        request: perro_render_bridge::RenderRequestID::new(1201),
        id: MeshID::nil(),
        source: "__cube__".to_string(),
        reserved: false,
    }));
    graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMaterial {
        request: perro_render_bridge::RenderRequestID::new(1202),
        id: MaterialID::nil(),
        material: Material3D::default(),
        source: None,
        reserved: false,
    }));
    graphics.draw_frame();

    let mut events = Vec::new();
    graphics.drain_events(&mut events);
    let mut mesh_id = MeshID::nil();
    let mut material_id = MaterialID::nil();
    for event in events {
        match event {
            perro_render_bridge::RenderEvent::MeshCreated { id, .. } => mesh_id = id,
            perro_render_bridge::RenderEvent::MaterialCreated { id, .. } => material_id = id,
            _ => {}
        }
    }
    assert!(!mesh_id.is_nil());
    assert!(!material_id.is_nil());

    let instance_mats: Arc<[[[f32; 4]; 4]]> = Arc::from(
        vec![
            [
                [1.0, 0.0, 0.0, 1.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            [
                [1.0, 0.0, 0.0, 2.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            [
                [1.0, 0.0, 0.0, 3.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        ]
        .into_boxed_slice(),
    );
    graphics.submit(RenderCommand::ThreeD(Box::new(Command3D::DrawMulti {
        mesh: mesh_id,
        surfaces: surfaces_for(material_id),
        node,
        instance_mats: instance_mats.clone(),
        skeleton: None,
        meshlet_override: None,
        lod: LODOptions3D::default(),
        blend: Default::default(),
    })));
    graphics.draw_frame();

    assert_eq!(
        graphics.renderer_3d.retained_draw(node),
        Some(crate::three_d::renderer::Draw3DInstance {
            node,
            kind: Draw3DKind::Mesh(mesh_id),
            surfaces: surfaces_for(material_id),
            instance_mats,
            skeleton: None,
            dense_multimesh: None,
            meshlet_override: None,
            lod: LODOptions3D::default(),
            blend: Default::default(),
            debug_color: None,
        })
    );
}

#[test]
fn rejected_3d_draw_keeps_previous_retained_binding() {
    let mut graphics = PerroGraphics::new();
    let node = NodeID::from_parts(20, 0);

    graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMesh {
        request: perro_render_bridge::RenderRequestID::new(2001),
        id: MeshID::nil(),
        source: "__cube__".to_string(),
        reserved: false,
    }));
    graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMaterial {
        request: perro_render_bridge::RenderRequestID::new(2002),
        id: MaterialID::nil(),
        material: Material3D::default(),
        source: None,
        reserved: false,
    }));
    graphics.draw_frame();

    let mut events = Vec::new();
    graphics.drain_events(&mut events);
    let mut mesh_id = MeshID::nil();
    let mut material_id = MaterialID::nil();
    for event in events {
        match event {
            perro_render_bridge::RenderEvent::MeshCreated { id, .. } => mesh_id = id,
            perro_render_bridge::RenderEvent::MaterialCreated { id, .. } => material_id = id,
            _ => {}
        }
    }
    assert!(!mesh_id.is_nil());
    assert!(!material_id.is_nil());

    let first_model = [
        [1.0, 0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    graphics.submit(RenderCommand::ThreeD(Box::new(Command3D::Draw {
        mesh: mesh_id,
        surfaces: surfaces_for(material_id),
        node,
        model: first_model,
        skeleton: None,
        meshlet_override: None,
        lod: LODOptions3D::default(),
        blend: Default::default(),
    })));
    graphics.draw_frame();
    assert_eq!(
        graphics.renderer_3d.retained_draw(node),
        Some(crate::three_d::renderer::Draw3DInstance {
            node,
            kind: Draw3DKind::Mesh(mesh_id),
            surfaces: surfaces_for(material_id),
            instance_mats: Arc::from([first_model]),
            skeleton: None,
            dense_multimesh: None,
            meshlet_override: None,
            lod: LODOptions3D::default(),
            blend: Default::default(),
            debug_color: None,
        })
    );

    let missing_mesh = MeshID::from_parts(999_999, 0);
    let second_model = [
        [1.0, 0.0, 0.0, 2.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    graphics.submit(RenderCommand::ThreeD(Box::new(Command3D::Draw {
        mesh: missing_mesh,
        surfaces: surfaces_for(material_id),
        node,
        model: second_model,
        skeleton: None,
        meshlet_override: None,
        lod: LODOptions3D::default(),
        blend: Default::default(),
    })));
    graphics.draw_frame();

    assert_eq!(
        graphics.renderer_3d.retained_draw(node),
        Some(crate::three_d::renderer::Draw3DInstance {
            node,
            kind: Draw3DKind::Mesh(mesh_id),
            surfaces: surfaces_for(material_id),
            instance_mats: Arc::from([second_model]),
            skeleton: None,
            dense_multimesh: None,
            meshlet_override: None,
            lod: LODOptions3D::default(),
            blend: Default::default(),
            debug_color: None,
        })
    );
}

#[test]
fn rejected_3d_material_swap_keeps_previous_material_binding() {
    let mut graphics = PerroGraphics::new();
    let node = NodeID::from_parts(21, 0);

    graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMesh {
        request: perro_render_bridge::RenderRequestID::new(2101),
        id: MeshID::nil(),
        source: "__cube__".to_string(),
        reserved: false,
    }));
    graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMaterial {
        request: perro_render_bridge::RenderRequestID::new(2102),
        id: MaterialID::nil(),
        material: Material3D::default(),
        source: None,
        reserved: false,
    }));
    graphics.draw_frame();

    let mut events = Vec::new();
    graphics.drain_events(&mut events);
    let mut mesh_id = MeshID::nil();
    let mut material_id = MaterialID::nil();
    for event in events {
        match event {
            perro_render_bridge::RenderEvent::MeshCreated { id, .. } => mesh_id = id,
            perro_render_bridge::RenderEvent::MaterialCreated { id, .. } => material_id = id,
            _ => {}
        }
    }
    assert!(!mesh_id.is_nil());
    assert!(!material_id.is_nil());

    let first_model = [
        [1.0, 0.0, 0.0, 0.5],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    graphics.submit(RenderCommand::ThreeD(Box::new(Command3D::Draw {
        mesh: mesh_id,
        surfaces: surfaces_for(material_id),
        node,
        model: first_model,
        skeleton: None,
        meshlet_override: None,
        lod: LODOptions3D::default(),
        blend: Default::default(),
    })));
    graphics.draw_frame();

    let missing_material = MaterialID::from_parts(999_998, 0);
    let second_model = [
        [1.0, 0.0, 0.0, 1.5],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    graphics.submit(RenderCommand::ThreeD(Box::new(Command3D::Draw {
        mesh: mesh_id,
        surfaces: surfaces_for(missing_material),
        node,
        model: second_model,
        skeleton: None,
        meshlet_override: None,
        lod: LODOptions3D::default(),
        blend: Default::default(),
    })));
    graphics.draw_frame();

    assert_eq!(
        graphics.renderer_3d.retained_draw(node),
        Some(crate::three_d::renderer::Draw3DInstance {
            node,
            kind: Draw3DKind::Mesh(mesh_id),
            surfaces: surfaces_for(material_id),
            instance_mats: Arc::from([second_model]),
            skeleton: None,
            dense_multimesh: None,
            meshlet_override: None,
            lod: LODOptions3D::default(),
            blend: Default::default(),
            debug_color: None,
        })
    );
}

#[test]
fn set_camera_3d_updates_retained_camera_state() {
    let mut graphics = PerroGraphics::new();
    graphics.submit(RenderCommand::ThreeD(Box::new(Command3D::SetCamera {
        camera: Camera3DState {
            position: [1.0, 2.0, 3.0],
            rotation: [0.0, 0.5, 0.0, 0.8660254],
            projection: CameraProjectionState::Perspective {
                fov_y_degrees: 48.0,
                near: 0.2,
                far: 900.0,
            },
            render_mask: perro_structs::BitMask::NONE,
            post_processing: Arc::from([]),
            audio_options: perro_structs::AudioListenerOptions::new(),
        },
    })));
    graphics.draw_frame();

    assert_eq!(
        graphics.renderer_3d.camera(),
        Camera3DState {
            position: [1.0, 2.0, 3.0],
            rotation: [0.0, 0.5, 0.0, 0.8660254],
            projection: CameraProjectionState::Perspective {
                fov_y_degrees: 48.0,
                near: 0.2,
                far: 900.0,
            },
            render_mask: perro_structs::BitMask::NONE,
            post_processing: Arc::from([]),
            audio_options: perro_structs::AudioListenerOptions::new(),
        }
    );
}

#[test]
fn rejected_sprite_texture_does_not_update_retained_binding() {
    let mut graphics = PerroGraphics::new();
    let node = NodeID::from_parts(2, 0);
    let missing = TextureID::from_parts(999, 0);

    graphics.submit(RenderCommand::TwoD(Command2D::UpsertSprite {
        node,
        sprite: Sprite2DCommand {
            texture: missing,
            model: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            tint: Color::WHITE,
            uv_min: [0.0, 0.0],
            uv_max: [0.0, 0.0],
            size: [0.0, 0.0],
            z_index: 0,
        },
    }));
    graphics.draw_frame();

    assert_eq!(graphics.renderer_2d.retained_sprite(node), None);
}

#[test]
fn rejected_sprite_texture_swap_keeps_previous_texture_binding() {
    let mut graphics = PerroGraphics::new();
    let request = perro_render_bridge::RenderRequestID::new(3001);
    let node = NodeID::from_parts(3, 0);

    graphics.submit(RenderCommand::Resource(ResourceCommand::CreateTexture {
        request,
        id: TextureID::nil(),
        source: "__default__".to_string(),
        reserved: false,
    }));
    graphics.draw_frame();

    let mut events = Vec::new();
    graphics.drain_events(&mut events);
    let texture = events
        .into_iter()
        .find_map(|event| match event {
            perro_render_bridge::RenderEvent::TextureCreated { id, .. } => Some(id),
            _ => None,
        })
        .expect("texture creation event should exist");

    let first_model = [[1.0, 0.0, 2.0], [0.0, 1.0, 3.0], [0.0, 0.0, 1.0]];
    graphics.submit(RenderCommand::TwoD(Command2D::UpsertSprite {
        node,
        sprite: Sprite2DCommand {
            texture,
            model: first_model,
            tint: Color::WHITE,
            uv_min: [0.0, 0.0],
            uv_max: [0.0, 0.0],
            size: [0.0, 0.0],
            z_index: 1,
        },
    }));
    graphics.draw_frame();

    let missing_texture = TextureID::from_parts(999_997, 0);
    let second_model = [[1.0, 0.0, 9.0], [0.0, 1.0, 4.0], [0.0, 0.0, 1.0]];
    graphics.submit(RenderCommand::TwoD(Command2D::UpsertSprite {
        node,
        sprite: Sprite2DCommand {
            texture: missing_texture,
            model: second_model,
            tint: Color::WHITE,
            uv_min: [0.0, 0.0],
            uv_max: [0.0, 0.0],
            size: [0.0, 0.0],
            z_index: 7,
        },
    }));
    graphics.draw_frame();

    assert_eq!(
        graphics.renderer_2d.retained_sprite(node),
        Some(Sprite2DCommand {
            texture,
            model: second_model,
            tint: Color::WHITE,
            uv_min: [0.0, 0.0],
            uv_max: [0.0, 0.0],
            size: [0.0, 0.0],
            z_index: 7,
        })
    );
}

#[test]
fn retained_sprite_instances_count_texture_refs_per_node() {
    let mut graphics = PerroGraphics::new();
    let texture = graphics
        .resources
        .create_texture("__tmp_ref_sprite__", false);
    let first = NodeID::from_parts(91, 0);
    let second = NodeID::from_parts(92, 0);

    for node in [first, second] {
        graphics.submit(RenderCommand::TwoD(Command2D::UpsertSprite {
            node,
            sprite: Sprite2DCommand {
                texture,
                model: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                tint: Color::WHITE,
                uv_min: [0.0, 0.0],
                uv_max: [1.0, 1.0],
                size: [16.0, 16.0],
                z_index: 0,
            },
        }));
    }
    graphics.draw_frame();

    assert_eq!(graphics.resources.texture_ref_count(texture), 2);
}

#[test]
fn retained_mesh_instances_count_mesh_refs_per_node() {
    let mut graphics = PerroGraphics::new();
    let mesh = graphics
        .resources
        .create_mesh("res://meshes/ref_count.glb", false);
    let material = graphics
        .resources
        .create_material(Material3D::default(), None, false);
    let surfaces = surfaces_for(material);

    for node in [NodeID::from_parts(101, 0), NodeID::from_parts(102, 0)] {
        graphics.submit(RenderCommand::ThreeD(Box::new(Command3D::Draw {
            node,
            mesh,
            surfaces: Arc::clone(&surfaces),
            model: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            skeleton: None,
            meshlet_override: None,
            lod: LODOptions3D::default(),
            blend: Default::default(),
        })));
    }
    graphics.draw_frame();

    assert_eq!(graphics.resources.mesh_ref_count(mesh), 2);
    assert_eq!(graphics.resources.material_ref_count(material), 2);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn stale_async_texture_result_after_drop_does_not_emit_loaded() {
    let mut graphics = PerroGraphics::new();
    let id = graphics
        .resources
        .create_texture("__tmp_async_drop_texture__", false);
    graphics.pending_async_texture_loads.insert(id);
    assert!(graphics.resources.drop_texture(id));

    graphics
        .async_texture_load_tx
        .send(super::AsyncTextureLoadResult {
            id,
            texture: Some(super::DecodedTextureRgba {
                rgba: vec![255, 255, 255, 255],
                width: 1,
                height: 1,
            }),
        })
        .unwrap();

    graphics.draw_frame();

    let mut events = Vec::new();
    graphics.drain_events(&mut events);
    assert!(!events
        .iter()
        .any(|event| matches!(event, perro_render_bridge::RenderEvent::TextureLoaded { id: got } if *got == id)));
    assert!(!graphics.resources.has_texture(id));
    assert!(graphics.resources.decoded_texture_data(id).is_none());
}

#[test]
fn accessibility_command_updates_global_accessibility_state() {
    let mut graphics = PerroGraphics::new();
    graphics.submit(RenderCommand::VisualAccessibility(
        VisualAccessibilityCommand::EnableColorBlind {
            mode: ColorBlindFilter::Deuteran,
            strength: 0.75,
        },
    ));
    graphics.draw_frame();

    let filter = graphics
        .accessibility
        .color_blind
        .expect("color blind filter should be enabled");
    assert_eq!(filter.filter, ColorBlindFilter::Deuteran);
    assert_eq!(filter.strength, 0.75);

    graphics.submit(RenderCommand::VisualAccessibility(
        VisualAccessibilityCommand::DisableColorBlind,
    ));
    graphics.draw_frame();
    assert_eq!(graphics.accessibility.color_blind, None);
}

#[test]
fn post_processing_commands_update_global_post_processing_state() {
    let mut graphics = PerroGraphics::new();
    graphics.submit(RenderCommand::PostProcessing(
        PostProcessingCommand::AddGlobalNamed {
            name: "crt".into(),
            effect: PostProcessEffect::Crt {
                scanline_strength: 0.25,
                curvature: 0.1,
                chromatic: 0.5,
                vignette: 0.2,
            },
        },
    ));
    graphics.submit(RenderCommand::PostProcessing(
        PostProcessingCommand::AddGlobalUnnamed(PostProcessEffect::Bloom {
            strength: 0.7,
            threshold: 0.8,
            radius: 1.2,
        }),
    ));
    graphics.draw_frame();

    assert_eq!(graphics.global_post_processing.len(), 2);
    assert!(matches!(
        graphics.global_post_processing.get("crt"),
        Some(PostProcessEffect::Crt { .. })
    ));

    graphics.submit(RenderCommand::PostProcessing(
        PostProcessingCommand::RemoveGlobalByName("crt".into()),
    ));
    graphics.submit(RenderCommand::PostProcessing(
        PostProcessingCommand::RemoveGlobalByIndex(0),
    ));
    graphics.draw_frame();
    assert!(graphics.global_post_processing.is_empty());

    let set = PostProcessSet::from_effects(vec![PostProcessEffect::Blur { strength: 2.0 }]);
    graphics.submit(RenderCommand::PostProcessing(
        PostProcessingCommand::SetGlobal(set),
    ));
    graphics.draw_frame();
    assert_eq!(graphics.global_post_processing.len(), 1);

    graphics.submit(RenderCommand::PostProcessing(
        PostProcessingCommand::ClearGlobal,
    ));
    graphics.draw_frame();
    assert!(graphics.global_post_processing.is_empty());
}
