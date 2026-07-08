use super::Runtime;
use perro_ids::TextureID;
use perro_input_api::MouseButton;
use perro_nodes::{
    AmbientLight2D, Button2D, CameraStream2D, CollisionShape2D, ImageButton2D, Label2D,
    NineSlice2D, PointLight2D, RayLight2D, SceneNode, SceneNodeData, Shape2D, SpotLight2D,
    StaticBody2D, WaterBody2D,
    camera_2d::Camera2D,
    node_2d::Node2D,
    particle_emitter_2d::ParticleEmitter2D,
    physics_2d::RigidBody2D,
    sprite_2d::{AnimatedSprite, AnimatedSprite2D, Sprite2D},
};
use perro_render_bridge::{
    CameraStreamCommand, Command2D, ParticlePath2D, RenderCommand, RenderEvent, ResourceCommand,
    UiCommand,
};
use perro_resource_api::sub_apis::{
    AnimationAPI, AnimationTreeAPI, CsvAPI, MaterialAPI, MeshAPI, TextureAPI,
};
use perro_runtime_api::sub_apis::{NodeAPI, NodeSpec};
use perro_structs::{BitMask, Color, Vector2};
use std::sync::Arc;

use crate::runtime::state::UiButtonVisualState;

fn collect_commands(runtime: &mut Runtime) -> Vec<RenderCommand> {
    let mut out = Vec::new();
    runtime.drain_render_commands(&mut out);
    out
}

fn water_2d_command(
    commands: &[RenderCommand],
    node_id: perro_ids::NodeID,
) -> &perro_render_bridge::Water2DState {
    commands
        .iter()
        .find_map(|command| match command {
            RenderCommand::TwoD(Command2D::UpsertWater { node, water }) if *node == node_id => {
                Some(water.as_ref())
            }
            _ => None,
        })
        .expect("water command should exist")
}

#[test]
fn camera_stream_2d_emits_stream_and_sprite_commands() {
    let mut runtime = Runtime::new();
    let camera = NodeAPI::create::<Camera2D>(&mut runtime);
    let stream = NodeAPI::create::<CameraStream2D>(&mut runtime);
    if let Some(node) = runtime.nodes.get_mut(stream)
        && let SceneNodeData::CameraStream2D(data) = &mut node.data
    {
        data.stream.camera = camera;
        data.stream.resolution = [320, 180].into();
    }

    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);

    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::CameraStream(CameraStreamCommand::Upsert { node, state })
            if *node == stream && state.resolution == [320, 180]
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::UpsertCameraStream { node, sprite, .. })
            if *node == stream && sprite.texture == Runtime::camera_stream_texture_id(stream)
    )));
}

#[test]
fn sprite_2d_uses_inherited_node_modulate() {
    let mut runtime = Runtime::new();
    let parent = NodeAPI::create::<Node2D>(&mut runtime);
    let child = NodeAPI::create::<Sprite2D>(&mut runtime);
    let texture = TextureID::from_parts(401, 0);

    if let Some(node) = runtime.nodes.get_mut(parent)
        && let SceneNodeData::Node2D(data) = &mut node.data
    {
        data.modulate.children_modulate = Color::new(0.5, 1.0, 1.0, 1.0);
        data.modulate.self_modulate = Color::RED;
        node.add_child(child);
    }
    if let Some(node) = runtime.nodes.get_mut(child)
        && let SceneNodeData::Sprite2D(data) = &mut node.data
    {
        data.texture = texture;
        data.modulate.self_modulate = Color::new(1.0, 0.25, 1.0, 1.0);
        node.parent = parent;
    }

    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);
    let sprite = commands
        .iter()
        .find_map(|command| match command {
            RenderCommand::TwoD(Command2D::UpsertSprite { node, sprite }) if *node == child => {
                Some(sprite)
            }
            _ => None,
        })
        .expect("sprite command");

    let expected = Runtime::color_modulate(
        Color::new(0.5, 1.0, 1.0, 1.0),
        Color::new(1.0, 0.25, 1.0, 1.0),
    );
    assert_eq!(sprite.tint, expected);
}

#[test]
fn point_light_2d_emits_cast_shadows_flag() {
    let mut runtime = Runtime::new();
    let light = NodeAPI::create::<PointLight2D>(&mut runtime);
    if let Some(node) = runtime.nodes.get_mut(light)
        && let SceneNodeData::PointLight2D(data) = &mut node.data
    {
        data.cast_shadows = true;
        data.range = 64.0;
    }

    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);

    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::SetPointLight { node, light: state })
            if *node == light && state.cast_shadows
    )));
}

#[test]
fn collision_shape_2d_emits_shadow_caster() {
    let mut runtime = Runtime::new();
    let caster = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    if let Some(node) = runtime.nodes.get_mut(caster)
        && let SceneNodeData::CollisionShape2D(data) = &mut node.data
    {
        data.shape = Shape2D::Quad {
            width: 8.0,
            height: 4.0,
        };
        data.transform.position = Vector2::new(3.0, 5.0);
        data.transform.scale = Vector2::new(2.0, 3.0);
        data.transform.rotation = 0.25;
        data.z_index = 7;
    }

    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);

    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::UpsertShadowCaster { node, caster: state })
            if *node == caster
                && state.center == [3.0, 5.0]
                && state.half_extents == [8.0, 6.0]
                && state.rotation_radians == 0.25
                && state.z_index == 7
    )));
}

#[test]
fn label_2d_emits_ui_label_with_world_rect() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);
    let label = NodeAPI::create::<Label2D>(&mut runtime);
    if let Some(node) = runtime.nodes.get_mut(label)
        && let SceneNodeData::Label2D(data) = &mut node.data
    {
        data.text = "HP".into();
        data.size = Vector2::new(100.0, 24.0);
        data.transform.position = Vector2::new(12.0, 8.0);
    }

    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);

    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::Ui(UiCommand::UpsertLabel { node, rect, text, .. })
            if *node == label && text.as_ref() == "HP" && rect.center == [12.0, 8.0]
    )));
}

#[test]
fn camera_stream_2d_uses_source_camera_render_mask() {
    let mut runtime = Runtime::new();
    let camera = NodeAPI::create::<Camera2D>(&mut runtime);
    let stream = NodeAPI::create::<CameraStream2D>(&mut runtime);
    let visible_sprite = NodeAPI::create::<Sprite2D>(&mut runtime);
    let masked_sprite = NodeAPI::create::<Sprite2D>(&mut runtime);
    if let Some(node) = runtime.nodes.get_mut(camera)
        && let SceneNodeData::Camera2D(data) = &mut node.data
    {
        data.render_mask = BitMask::with([2]);
    }
    if let Some(node) = runtime.nodes.get_mut(stream)
        && let SceneNodeData::CameraStream2D(data) = &mut node.data
    {
        data.stream.camera = camera;
    }
    for (node_id, layer, texture) in [
        (visible_sprite, 1, TextureID::from_parts(100, 0)),
        (masked_sprite, 2, TextureID::from_parts(101, 0)),
    ] {
        if let Some(node) = runtime.nodes.get_mut(node_id)
            && let SceneNodeData::Sprite2D(data) = &mut node.data
        {
            data.render_layers = BitMask::with([layer]);
            data.texture = texture;
        }
    }

    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);

    let stream_state = commands
        .iter()
        .find_map(|command| match command {
            RenderCommand::CameraStream(CameraStreamCommand::Upsert { node, state })
                if *node == stream =>
            {
                Some(state)
            }
            _ => None,
        })
        .expect("stream state");
    assert_eq!(stream_state.sprites_2d.len(), 1);
    assert_eq!(
        stream_state.sprites_2d[0].texture,
        TextureID::from_parts(100, 0)
    );
}

#[test]
fn disabled_camera_stream_2d_emits_remove_commands() {
    let mut runtime = Runtime::new();
    let camera = NodeAPI::create::<Camera2D>(&mut runtime);
    let stream = NodeAPI::create::<CameraStream2D>(&mut runtime);
    if let Some(node) = runtime.nodes.get_mut(stream)
        && let SceneNodeData::CameraStream2D(data) = &mut node.data
    {
        data.stream.camera = camera;
        data.stream.enabled = false;
    }

    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);

    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::CameraStream(CameraStreamCommand::RemoveNode { node }) if *node == stream
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::RemoveNode { node }) if *node == stream
    )));
}

#[test]
fn linked_2d_water_mirrors_wake_across_overlap() {
    let mut runtime = Runtime::new();
    let water_a = NodeAPI::create::<WaterBody2D>(&mut runtime);
    let water_b = NodeAPI::create::<WaterBody2D>(&mut runtime);
    for (id, x) in [(water_a, 0.0), (water_b, 12.0)] {
        if let Some(node) = runtime.nodes.get_mut(id)
            && let SceneNodeData::WaterBody2D(water) = &mut node.data
        {
            water.transform.position.x = x;
            water.water.shape = perro_nodes::WaterShape::rect(Vector2::new(16.0, 16.0));
        }
    }
    runtime
        .force_water_impacts_2d
        .push(crate::runtime::ForceWaterImpact2D {
            position: Vector2::new(8.4, 0.0),
            force: Vector2::new(12.0, 0.0),
            strength: 10.0,
            radius: 0.25,
            cavitation: 0.5,
        });

    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);
    let water = water_2d_command(&commands, water_a);

    assert_eq!(water.links.len(), 1);
    assert_eq!(water.impacts.len(), 1);
    assert!(water.impacts[0].strength > 0.0);
    assert!(water.impacts[0].strength < 10.0);
}

#[test]
fn linked_2d_waters_both_collect_shared_coastline_shape() {
    let mut runtime = Runtime::new();
    let water_a = NodeAPI::create::<WaterBody2D>(&mut runtime);
    let water_b = NodeAPI::create::<WaterBody2D>(&mut runtime);
    for (id, x) in [(water_a, 0.0), (water_b, 12.0)] {
        if let Some(node) = runtime.nodes.get_mut(id)
            && let SceneNodeData::WaterBody2D(water) = &mut node.data
        {
            water.transform.position.x = x;
            water.water.shape = perro_nodes::WaterShape::rect(Vector2::new(16.0, 16.0));
        }
    }
    let body = NodeAPI::create::<StaticBody2D>(&mut runtime);
    let shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, body, shape));
    if let Some(node) = runtime.nodes.get_mut(shape)
        && let SceneNodeData::CollisionShape2D(shape) = &mut node.data
    {
        shape.transform.position = Vector2::new(6.0, 0.0);
        shape.shape = Shape2D::Quad {
            width: 2.0,
            height: 4.0,
        };
    }

    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);

    assert_eq!(
        water_2d_command(&commands, water_a).coastline_shapes.len(),
        1
    );
    assert_eq!(
        water_2d_command(&commands, water_b).coastline_shapes.len(),
        1
    );
}

#[test]
fn water_2d_impacts_use_live_body_pos_not_stale_cached_sample() {
    let mut runtime = Runtime::new();
    let water = NodeAPI::create::<WaterBody2D>(&mut runtime);
    let body = NodeAPI::create::<RigidBody2D>(&mut runtime);
    if let Some(node) = runtime.nodes.get_mut(body)
        && let SceneNodeData::RigidBody2D(rigid) = &mut node.data
    {
        rigid.transform.position = Vector2::new(1.25, -0.35);
        rigid.linear_velocity = Vector2::new(0.0, -2.5);
        rigid.mass = 4.0;
        rigid.density = 1.0;
    }
    runtime.time.elapsed = 1.0;
    runtime.apply_render_event(RenderEvent::WaterBodySamples {
        samples: Arc::from([perro_render_bridge::WaterBodySampleState {
            water,
            body,
            point: 0,
            local: [5.5, 0.0],
            height: 2.0,
            velocity: [0.0, 0.0],
            foam: 1.0,
        }]),
    });

    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);
    let water = water_2d_command(&commands, water);

    assert_eq!(water.impacts.len(), 1);
    assert!((water.impacts[0].position[0] - 1.25).abs() < 0.01);
    assert!((water.impacts[0].position[1] + 0.35).abs() < 0.01);
}

#[test]
fn particle_emitter_2d_queues_point_particles() {
    let mut runtime = Runtime::new();
    let mut emitter = ParticleEmitter2D::new();
    emitter.profile = "inline://preset = ballistic\nz = 999\nforce_z = 777".to_string();
    emitter.spawn_rate = 10.0;
    emitter.internal_simulation_time = 0.25;
    let expected_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::ParticleEmitter2D(emitter)));

    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);

    assert!(commands.iter().any(|command| {
        matches!(
            command,
            RenderCommand::TwoD(Command2D::UpsertPointParticles { node, particles })
                if *node == expected_node
                    && matches!(particles.profile.path, ParticlePath2D::Ballistic)
                    && particles.profile.force == [0.0, 0.0]
                    && particles.profile.expr_x_ops.is_none()
                    && particles.profile.expr_y_ops.is_none()
        )
    }));
}

#[test]
fn point_light_2d_emits_light_command() {
    let mut runtime = Runtime::new();
    let mut light = PointLight2D::new();
    light.transform.position.x = 24.0;
    light.transform.position.y = -6.0;
    light.color = [1.0, 0.5, 0.25];
    light.intensity = 3.0;
    light.range = 300.0;
    let expected_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::PointLight2D(light)));

    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);

    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::SetPointLight { node, light })
            if *node == expected_node
                && light.position == [24.0, -6.0]
                && light.color == [1.0, 0.5, 0.25]
                && light.intensity == 3.0
                && light.range == 300.0
    )));
}

#[test]
fn ambient_ray_and_spot_light_2d_emit_light_commands() {
    let mut runtime = Runtime::new();
    let mut ambient = AmbientLight2D::new();
    ambient.intensity = 0.25;
    let ambient_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AmbientLight2D(ambient)));

    let mut ray = RayLight2D::new();
    ray.transform.rotation = std::f32::consts::FRAC_PI_2;
    let ray_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::RayLight2D(ray)));

    let mut spot = SpotLight2D::new();
    spot.transform.position.x = 8.0;
    spot.range = 64.0;
    let spot_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::SpotLight2D(spot)));

    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);

    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::SetAmbientLight { node, light })
            if *node == ambient_node && light.intensity == 0.25
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::SetRayLight { node, light })
            if *node == ray_node && light.direction[0] > 0.99
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::SetSpotLight { node, light })
            if *node == spot_node && light.position == [8.0, 0.0] && light.range == 64.0
    )));
}

#[test]
fn inactive_point_light_2d_emits_remove_node() {
    let mut runtime = Runtime::new();
    let mut light = PointLight2D::new();
    light.active = false;
    let expected_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::PointLight2D(light)));

    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);

    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::RemoveNode { node }) if *node == expected_node
    )));
}

#[test]
fn ppart_2d_ignores_z_fields() {
    let parsed = super::parse_pparticle_source_2d(
        "x = t * 2\ny = life\nz = bad_symbol\nforce = (1, 2, 999)\nforce_z = bad",
    )
    .expect("2d ppart parses without reading z");

    assert_eq!(parsed.force, [1.0, 2.0]);
    assert!(parsed.expr_x_ops.is_some());
    assert!(parsed.expr_y_ops.is_some());
}

#[test]
fn ptileset_parses_polygon_collision_shape() {
    let parsed = perro_render_bridge::parse_ptileset_source(
        r#"
        texture = "res://tiles/world.png"
        tile_size = (16, 16)
        columns = 1
        rows = 1
        tiles = [
            { id = 1 atlas = (0, 0) collision = true collision_shape = { polygon = { points = [(0, 0), (16, 0), (8, 16)] offset = (1, -2) } } },
        ]
        "#,
    )
    .expect("tileset parses");

    let tile = parsed.tile(1).expect("tile exists");
    match &tile.collision_shape {
        super::ParsedTileCollisionShape2D::Polygon { points, offset } => {
            assert_eq!(
                points.as_ref(),
                &[
                    Vector2::new(0.0, 0.0),
                    Vector2::new(16.0, 0.0),
                    Vector2::new(8.0, 16.0)
                ]
            );
            assert_eq!(*offset, [1.0, -2.0]);
        }
        other => panic!("expected polygon shape, got {other:?}"),
    }
}

#[test]
fn sprite_requests_texture_once_until_created() {
    let mut runtime = Runtime::new();
    let expected_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Sprite2D(Sprite2D::new())));

    runtime.extract_render_2d_commands();
    let first = collect_commands(&mut runtime);
    assert_eq!(first.len(), 1);
    let request = match &first[0] {
        RenderCommand::Resource(ResourceCommand::CreateTexture {
            request,
            id,
            source,
            reserved,
        }) => {
            assert_eq!(source, "__default__");
            assert!(!reserved);
            assert!(id.is_nil());
            *request
        }
        _ => panic!("expected CreateTexture"),
    };

    runtime.extract_render_2d_commands();
    assert!(collect_commands(&mut runtime).is_empty());

    let texture = TextureID::from_parts(3, 1);
    runtime.apply_render_event(RenderEvent::TextureCreated {
        request,
        id: texture,
    });
    runtime.extract_render_2d_commands();
    let third = collect_commands(&mut runtime);
    assert_eq!(third.len(), 1);
    assert!(matches!(
        third[0],
        RenderCommand::TwoD(Command2D::UpsertSprite { node, sprite })
        if node == expected_node && sprite.texture == texture
    ));
}

#[test]
fn texture_create_from_rgba_queues_runtime_texture() {
    let mut runtime = Runtime::new();
    let rgba = vec![255u8, 0, 0, 255, 0, 255, 0, 255];

    let texture = TextureAPI::create_texture_from_rgba(runtime.resource_api.as_ref(), 2, 1, &rgba);

    assert!(!texture.is_nil());
    let commands = collect_commands(&mut runtime);
    assert_eq!(commands.len(), 1);
    let request = match &commands[0] {
        RenderCommand::Resource(ResourceCommand::CreateRuntimeTexture {
            request,
            id,
            source,
            reserved,
            width,
            height,
            rgba: command_rgba,
        }) => {
            assert_eq!(*id, texture);
            assert!(source.starts_with("runtime://texture/"));
            assert!(!reserved);
            assert_eq!(*width, 2);
            assert_eq!(*height, 1);
            assert_eq!(command_rgba.as_ref(), rgba.as_slice());
            *request
        }
        _ => panic!("expected CreateRuntimeTexture"),
    };

    runtime.apply_render_event(RenderEvent::TextureCreated {
        request,
        id: texture,
    });
    runtime.apply_render_event(RenderEvent::TextureLoaded { id: texture });

    assert!(TextureAPI::is_texture_loaded(
        runtime.resource_api.as_ref(),
        texture
    ));
}

#[test]
fn texture_create_from_rgba_rejects_bad_len() {
    let mut runtime = Runtime::new();

    let texture =
        TextureAPI::create_texture_from_rgba(runtime.resource_api.as_ref(), 2, 1, &[255u8; 4]);

    assert!(texture.is_nil());
    assert!(collect_commands(&mut runtime).is_empty());
}

#[test]
fn texture_create_from_bytes_queues_runtime_texture_bytes() {
    let mut runtime = Runtime::new();
    let bytes = b"not a texture";

    let texture = TextureAPI::create_texture_from_bytes(runtime.resource_api.as_ref(), bytes);

    assert!(!texture.is_nil());
    let commands = collect_commands(&mut runtime);
    assert_eq!(commands.len(), 1);
    assert!(matches!(
        &commands[0],
        RenderCommand::Resource(ResourceCommand::CreateRuntimeTextureBytes { id, bytes: got, .. })
            if *id == texture && got.as_ref() == bytes
    ));
}

#[test]
fn mesh_create_from_bytes_queues_runtime_mesh_bytes() {
    let mut runtime = Runtime::new();
    let bytes = b"not a mesh";

    let mesh = MeshAPI::create_mesh_from_bytes(runtime.resource_api.as_ref(), bytes);

    assert!(!mesh.is_nil());
    let commands = collect_commands(&mut runtime);
    assert_eq!(commands.len(), 1);
    assert!(matches!(
        &commands[0],
        RenderCommand::Resource(ResourceCommand::CreateRuntimeMeshBytes { id, bytes: got, .. })
            if *id == mesh && got.as_ref() == bytes
    ));
}

#[test]
fn material_create_from_bytes_parses_pmat() {
    let runtime = Runtime::new();
    let material = MaterialAPI::create_material_from_bytes(
        runtime.resource_api.as_ref(),
        b"roughness_factor = 0.5",
    );

    assert!(!material.is_nil());
    assert!(MaterialAPI::get_material_data(runtime.resource_api.as_ref(), material).is_some());
}

#[test]
fn animation_and_tree_create_from_bytes_parse_text() {
    let runtime = Runtime::new();
    let animation_src = br#"
[Animation]
name = "Bytes"
fps = 30
[/Animation]
"#;
    let tree_src = br#"
[AnimationTree]
name = "BytesTree"
[/AnimationTree]
[AnimationSlots]
Idle
[/AnimationSlots]
[Output]
input = @Idle
[/Output]
"#;

    let animation =
        AnimationAPI::create_animation_from_bytes(runtime.resource_api.as_ref(), animation_src);
    let tree =
        AnimationTreeAPI::create_animation_tree_from_bytes(runtime.resource_api.as_ref(), tree_src);

    assert!(!animation.is_nil());
    assert!(!tree.is_nil());
    assert!(AnimationAPI::is_animation_loaded(
        runtime.resource_api.as_ref(),
        animation
    ));
    assert!(AnimationTreeAPI::is_animation_tree_loaded(
        runtime.resource_api.as_ref(),
        tree
    ));
}

#[test]
fn csv_load_bytes_parses_table() {
    let runtime = Runtime::new();

    let csv = CsvAPI::load_csv_bytes(runtime.resource_api.as_ref(), b"id,name\nsword,Sword\n");

    assert_eq!(csv.row_count(), 1);
    assert_eq!(
        csv.find_primary("sword").and_then(|row| row.get(1)),
        Some("Sword")
    );
}

#[test]
fn sprite_keeps_retained_texture_while_replacement_texture_is_pending() {
    let mut runtime = Runtime::new();
    let old_texture = TextureID::from_parts(31, 0);
    let mut sprite = Sprite2D::new();
    sprite.texture = old_texture;
    let node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Sprite2D(sprite)));

    runtime.extract_render_2d_commands();
    let first = collect_commands(&mut runtime);
    assert!(first.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::UpsertSprite { node: draw_node, sprite })
            if *draw_node == node && sprite.texture == old_texture
    )));

    let pending_texture = runtime
        .resource_api
        .load_texture("res://textures/tool_version_b.png");
    let pending_request = collect_commands(&mut runtime)
        .into_iter()
        .find_map(|command| match command {
            RenderCommand::Resource(ResourceCommand::CreateTexture { request, id, .. })
                if id == pending_texture =>
            {
                Some(request)
            }
            _ => None,
        })
        .expect("expected pending texture create request");
    if let Some(scene_node) = runtime.nodes.get_mut(node)
        && let SceneNodeData::Sprite2D(sprite) = &mut scene_node.data
    {
        sprite.texture = pending_texture;
    }
    runtime.mark_needs_rerender(node);

    runtime.extract_render_2d_commands();
    let pending = collect_commands(&mut runtime);
    assert!(!pending.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::RemoveNode { node: removed }) if *removed == node
    )));
    assert!(!pending.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::UpsertSprite { node: draw_node, .. }) if *draw_node == node
    )));
    assert_eq!(
        runtime
            .render_2d
            .retained_sprites
            .get(&node)
            .map(|sprite| sprite.texture),
        Some(old_texture)
    );

    runtime.apply_render_event(RenderEvent::TextureCreated {
        request: pending_request,
        id: pending_texture,
    });
    runtime.mark_needs_rerender(node);
    runtime.extract_render_2d_commands();
    let ready = collect_commands(&mut runtime);
    assert!(ready.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::UpsertSprite { node: draw_node, sprite })
            if *draw_node == node && sprite.texture == pending_texture
    )));
}

#[test]
fn animated_sprite_advances_frame_and_emits_region() {
    let mut runtime = Runtime::new();
    let mut sprite = AnimatedSprite2D::new();
    sprite.texture = TextureID::from_parts(22, 0);
    let mut animation = AnimatedSprite::new("run");
    animation.frame_size = [16.0, 16.0];
    animation.frame_count = 4;
    animation.columns = 2;
    animation.fps = 10.0;
    sprite.current_animation = "run".into();
    sprite.animations.push(animation);
    let expected_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AnimatedSprite2D(sprite)));
    runtime
        .register_internal_node_schedules(expected_node, perro_nodes::NodeType::AnimatedSprite2D);

    runtime.update(0.1);
    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);

    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::UpsertSprite { node, sprite })
            if *node == expected_node
                && sprite.uv_min == [16.0, 0.0]
                && sprite.uv_max == [32.0, 16.0]
                && sprite.size == [16.0, 16.0]
    )));
}

#[test]
fn sprite_flip_swaps_region_uv_without_changing_size() {
    let mut runtime = Runtime::new();
    let mut sprite = Sprite2D::new();
    sprite.texture = TextureID::from_parts(30, 0);
    sprite.texture_region = Some([4.0, 8.0, 16.0, 32.0]);
    sprite.flip_x = true;
    sprite.flip_y = true;
    let expected_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Sprite2D(sprite)));

    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);

    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::UpsertSprite { node, sprite })
            if *node == expected_node
                && sprite.uv_min == [20.0, 40.0]
                && sprite.uv_max == [4.0, 8.0]
                && sprite.size == [16.0, 32.0]
    )));
}

#[test]
fn create_nodes_10k_sprites_emit_render_commands() {
    let mut runtime = Runtime::new();
    let templates = vec![NodeSpec::new(Sprite2D::new()); 10_000];
    let ids = runtime.create_nodes(&templates, perro_ids::NodeID::nil());
    let texture = TextureID::from_parts(77, 0);

    for &id in &ids {
        runtime
            .with_node_mut::<Sprite2D, _, _>(id, |sprite| {
                sprite.texture = texture;
            })
            .expect("sprite exists");
    }

    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);
    let upserts = commands
        .iter()
        .filter(|command| {
            matches!(
                command,
                RenderCommand::TwoD(Command2D::UpsertSprite { sprite, .. })
                    if sprite.texture == texture
            )
        })
        .count();

    assert_eq!(ids.len(), 10_000);
    assert_eq!(upserts, 10_000);
}

#[test]
fn unchanged_sprite_skips_redundant_upsert() {
    let mut runtime = Runtime::new();
    let mut sprite = Sprite2D::new();
    sprite.texture = TextureID::from_parts(12, 0);
    let expected_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Sprite2D(sprite)));

    runtime.extract_render_2d_commands();
    let first = collect_commands(&mut runtime);
    assert_eq!(first.len(), 1);
    assert!(matches!(
        first[0],
        RenderCommand::TwoD(Command2D::UpsertSprite { node, .. }) if node == expected_node
    ));

    runtime.extract_render_2d_commands();
    let second = collect_commands(&mut runtime);
    assert!(second.is_empty());
}

#[test]
fn parent_modulate_change_reemits_child_sprite_with_effective_tint() {
    let mut runtime = Runtime::new();
    let parent = NodeAPI::create::<Node2D>(&mut runtime);
    let child = NodeAPI::create::<Sprite2D>(&mut runtime);
    let texture = TextureID::from_parts(402, 0);

    if let Some(node) = runtime.nodes.get_mut(parent) {
        node.add_child(child);
    }
    if let Some(node) = runtime.nodes.get_mut(child)
        && let SceneNodeData::Sprite2D(sprite) = &mut node.data
    {
        sprite.texture = texture;
        node.parent = parent;
    }

    runtime.extract_render_2d_commands();
    let first = collect_commands(&mut runtime);
    assert!(first.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::UpsertSprite { node, sprite })
            if *node == child && sprite.tint == Color::WHITE
    )));

    runtime.clear_dirty_flags();
    NodeAPI::with_base_node_mut::<Node2D, _, _>(&mut runtime, parent, |node| {
        node.modulate.children_modulate = Color::new(0.25, 0.5, 1.0, 1.0);
    });
    runtime.extract_render_2d_commands();
    let second = collect_commands(&mut runtime);

    assert!(second.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::UpsertSprite { node, sprite })
            if *node == child && sprite.tint == Color::new(0.25, 0.5, 1.0, 1.0)
    )));
}

#[test]
fn sprite_becoming_invisible_emits_remove_node() {
    let mut runtime = Runtime::new();
    let mut sprite = Sprite2D::new();
    sprite.texture = TextureID::from_parts(7, 0);
    let expected_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Sprite2D(sprite)));

    runtime.extract_render_2d_commands();
    let first = collect_commands(&mut runtime);
    assert_eq!(first.len(), 1);
    assert!(matches!(
        first[0],
        RenderCommand::TwoD(Command2D::UpsertSprite { node, .. }) if node == expected_node
    ));

    let node = runtime
        .nodes
        .get_mut(expected_node)
        .expect("sprite node must exist");
    if let SceneNodeData::Sprite2D(sprite) = &mut node.data {
        sprite.visible = false;
    }
    runtime.mark_needs_rerender(expected_node);

    runtime.extract_render_2d_commands();
    let second = collect_commands(&mut runtime);
    assert!(second.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::RemoveNode { node }) if *node == expected_node
    )));
    assert!(second.iter().any(|command| matches!(
        command,
        RenderCommand::Resource(ResourceCommand::SetSceneResourceRefs { textures, .. })
            if textures.iter().any(|(texture, nodes)| {
                *texture == TextureID::from_parts(7, 0) && nodes == &vec![expected_node]
            })
    )));
}

#[test]
fn removed_water_2d_emits_remove_node() {
    let mut runtime = Runtime::new();
    let water = NodeAPI::create::<WaterBody2D>(&mut runtime);

    runtime.extract_render_2d_commands();
    let first = collect_commands(&mut runtime);
    assert!(first.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::UpsertWater { node, .. }) if *node == water
    )));

    assert!(NodeAPI::remove_node(&mut runtime, water));
    let second = collect_commands(&mut runtime);
    assert!(second.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::RemoveNode { node }) if *node == water
    )));
}

#[test]
fn physics_pause_updates_water_2d_state() {
    let mut runtime = Runtime::new();
    let water = NodeAPI::create::<WaterBody2D>(&mut runtime);

    runtime.extract_render_2d_commands();
    let first = collect_commands(&mut runtime);
    assert!(!water_2d_command(&first, water).paused);
    runtime.clear_dirty_flags();

    runtime.extract_render_2d_commands();
    assert!(collect_commands(&mut runtime).is_empty());

    runtime.set_physics_paused(true);
    runtime.extract_render_2d_commands();
    let paused = collect_commands(&mut runtime);
    assert!(water_2d_command(&paused, water).paused);
}

#[test]
fn unchanged_camera_2d_skips_redundant_set_camera() {
    let mut runtime = Runtime::new();
    let mut camera = Camera2D::new();
    camera.active = true;
    camera.zoom = 1.5;
    runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Camera2D(camera)));

    runtime.extract_render_2d_commands();
    let first = collect_commands(&mut runtime);
    assert!(
        first
            .iter()
            .any(|cmd| matches!(cmd, RenderCommand::TwoD(Command2D::SetCamera { .. })))
    );

    runtime.extract_render_2d_commands();
    let second = collect_commands(&mut runtime);
    assert!(second.is_empty());
}

#[test]
fn active_camera_2d_emits_set_camera_command() {
    let mut runtime = Runtime::new();
    let mut camera = Camera2D::new();
    camera.active = true;
    camera.zoom = 2.0;
    camera.transform.position.x = 128.0;
    camera.transform.position.y = -32.0;
    camera.transform.rotation = 0.5;
    runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Camera2D(camera)));

    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::SetCamera { camera })
        if camera.position == [128.0, -32.0]
            && camera.rotation_radians == 0.5
            && camera.zoom == 2.0
    )));
}

#[test]
fn button_2d_emits_world_rect_command() {
    let mut runtime = Runtime::new();
    let button = NodeAPI::create::<Button2D>(&mut runtime);
    if let Some(node) = runtime.nodes.get_mut(button)
        && let SceneNodeData::Button2D(data) = &mut node.data
    {
        data.size = Vector2::new(80.0, 32.0);
        data.transform.position = Vector2::new(12.0, -8.0);
    }

    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);

    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::UpsertRect { node, rect })
            if *node == button && rect.center == [12.0, -8.0] && rect.size == [80.0, 32.0]
    )));
}

#[test]
fn image_button_2d_emits_world_sprite_command_with_state_tint() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);
    let button = NodeAPI::create::<ImageButton2D>(&mut runtime);
    if let Some(node) = runtime.nodes.get_mut(button)
        && let SceneNodeData::ImageButton2D(data) = &mut node.data
    {
        data.texture = TextureID::from_parts(44, 0);
        data.size = Vector2::new(96.0, 48.0);
        data.hover_tint = perro_structs::Color::new(0.2, 0.4, 0.6, 1.0);
    }
    runtime.begin_input_frame();
    runtime.set_mouse_position(400.0, 300.0);

    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);

    assert_eq!(
        runtime.render_ui.button_states.get(&button).copied(),
        Some(UiButtonVisualState::Hover)
    );
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::UpsertSprite { node, sprite })
            if *node == button
                && sprite.texture == TextureID::from_parts(44, 0)
                && sprite.size == [96.0, 48.0]
                && sprite.uv_min == [0.0, 0.0]
                && sprite.uv_max == [0.0, 0.0]
                && sprite.tint == perro_structs::Color::new(0.2, 0.4, 0.6, 1.0)
    )));
}

#[test]
fn button_2d_mouse_click_uses_world_hitbox() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);
    let button = NodeAPI::create::<Button2D>(&mut runtime);

    runtime.begin_input_frame();
    runtime.set_mouse_position(400.0, 300.0);
    runtime.set_mouse_button_state(MouseButton::Left, true);
    runtime.extract_render_2d_commands();

    assert_eq!(
        runtime.render_ui.button_states.get(&button).copied(),
        Some(UiButtonVisualState::Pressed)
    );

    runtime.begin_input_frame();
    runtime.set_mouse_position(400.0, 300.0);
    runtime.set_mouse_button_state(MouseButton::Left, false);
    runtime.extract_render_2d_commands();

    assert_eq!(
        runtime.render_ui.button_states.get(&button).copied(),
        Some(UiButtonVisualState::Hover)
    );
}

#[test]
fn button_2d_hover_requests_default_pointer_cursor_and_unhover_restores_default() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);
    let _button = NodeAPI::create::<Button2D>(&mut runtime);

    runtime.begin_input_frame();
    runtime.set_mouse_position(400.0, 300.0);
    runtime.extract_render_2d_commands();

    assert_eq!(
        runtime.take_cursor_icon_request(),
        Some(perro_ui::CursorIcon::Pointer)
    );

    runtime.begin_input_frame();
    runtime.set_mouse_position(700.0, 500.0);
    runtime.extract_render_2d_commands();

    assert_eq!(
        runtime.take_cursor_icon_request(),
        Some(perro_ui::CursorIcon::Default)
    );
}

#[test]
fn image_button_2d_hover_uses_cursor_icon_override() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);
    let button = NodeAPI::create::<ImageButton2D>(&mut runtime);
    if let Some(node) = runtime.nodes.get_mut(button)
        && let SceneNodeData::ImageButton2D(data) = &mut node.data
    {
        data.cursor_icon = perro_ui::CursorIcon::Grab;
    }

    runtime.begin_input_frame();
    runtime.set_mouse_position(400.0, 300.0);
    runtime.extract_render_2d_commands();

    assert_eq!(
        runtime.take_cursor_icon_request(),
        Some(perro_ui::CursorIcon::Grab)
    );
}

#[test]
fn image_button_2d_cursor_survives_full_render_extraction() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);
    let _button = NodeAPI::create::<ImageButton2D>(&mut runtime);

    runtime.begin_input_frame();
    runtime.set_mouse_position(400.0, 300.0);
    let mut commands = Vec::new();
    runtime.extract_render_snapshot_commands(&mut commands);

    assert_eq!(
        runtime.take_cursor_icon_request(),
        Some(perro_ui::CursorIcon::Pointer)
    );
}

#[test]
fn nine_slice_2d_emits_nine_sprite_tilemap() {
    let mut runtime = Runtime::new();
    let node = NodeAPI::create::<NineSlice2D>(&mut runtime);
    if let Some(scene_node) = runtime.nodes.get_mut(node)
        && let SceneNodeData::NineSlice2D(nine) = &mut scene_node.data
    {
        nine.texture = TextureID::from_parts(55, 0);
        nine.size = Vector2::new(100.0, 60.0);
        nine.margins = [10.0, 8.0, 12.0, 6.0];
        nine.texture_region = Some([0.0, 0.0, 50.0, 30.0]);
    }

    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);

    let sprites = commands
        .iter()
        .find_map(|command| match command {
            RenderCommand::TwoD(Command2D::UpsertTileMap { node: n, tilemap }) if *n == node => {
                Some(tilemap.sprites.as_ref())
            }
            _ => None,
        })
        .expect("nine slice tilemap");
    assert_eq!(sprites.len(), 9);
    assert!(sprites.iter().any(|sprite| sprite.size == [78.0, 46.0]));
    assert!(
        sprites
            .iter()
            .all(|sprite| sprite.texture == TextureID::from_parts(55, 0))
    );
}

#[test]
fn deactivating_last_camera_2d_resets_renderer_camera() {
    let mut runtime = Runtime::new();
    let mut camera = Camera2D::new();
    camera.active = true;
    camera.transform.position.x = 128.0;
    let camera_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Camera2D(camera)));

    runtime.extract_render_2d_commands();
    let _ = collect_commands(&mut runtime);

    runtime
        .with_node_mut::<Camera2D, _, _>(camera_node, |camera| {
            camera.active = false;
        })
        .expect("camera exists");
    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);

    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::SetCamera { camera })
            if camera.position == [0.0, 0.0] && camera.zoom == 1.0
    )));
}

#[test]
fn camera_2d_render_mask_filters_sprites() {
    let mut runtime = Runtime::new();
    let mut camera = Camera2D::new();
    camera.active = true;
    camera.render_mask = BitMask::with([3]);
    runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Camera2D(camera)));

    let mut sprite = Sprite2D::new();
    sprite.texture = TextureID::from_parts(91, 0);
    sprite.render_layers = BitMask::with([3]);
    let sprite_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Sprite2D(sprite)));

    runtime.extract_render_2d_commands();
    let first = collect_commands(&mut runtime);
    assert!(!first.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::UpsertSprite { node, .. }) if *node == sprite_node
    )));

    if let Some(node) = runtime.nodes.get_mut(sprite_node)
        && let SceneNodeData::Sprite2D(sprite) = &mut node.data
    {
        sprite.render_layers = BitMask::with([2]);
    }
    runtime.mark_needs_rerender(sprite_node);

    runtime.extract_render_2d_commands();
    let second = collect_commands(&mut runtime);
    assert!(second.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::UpsertSprite { node, .. }) if *node == sprite_node
    )));
}

#[test]
fn camera_2d_move_does_not_rewalk_sprite_render_layers() {
    let mut runtime = Runtime::new();
    let mut camera = Camera2D::new();
    camera.active = true;
    let camera_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Camera2D(camera)));

    let mut sprite = Sprite2D::new();
    sprite.texture = TextureID::from_parts(94, 0);
    let sprite_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Sprite2D(sprite)));

    runtime.extract_render_2d_commands();
    let _ = collect_commands(&mut runtime);

    if let Some(node) = runtime.nodes.get_mut(camera_node)
        && let SceneNodeData::Camera2D(camera) = &mut node.data
    {
        camera.transform.position.x = 10.0;
    }
    runtime.mark_transform_dirty_recursive(camera_node);

    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, RenderCommand::TwoD(Command2D::SetCamera { .. })))
    );
    assert!(!commands.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::UpsertSprite { node, .. }) if *node == sprite_node
    )));
}

#[test]
fn active_camera_2d_change_via_node_api_forces_full_rescan() {
    let mut runtime = Runtime::new();

    let mut camera_a = Camera2D::new();
    camera_a.active = true;
    camera_a.render_mask = BitMask::with([1]);
    let camera_a = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Camera2D(camera_a)));

    let mut camera_b = Camera2D::new();
    camera_b.active = false;
    camera_b.render_mask = BitMask::with([2]);
    let camera_b = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Camera2D(camera_b)));

    let mut sprite = Sprite2D::new();
    sprite.texture = TextureID::from_parts(95, 0);
    sprite.render_layers = BitMask::with([1]);
    let sprite_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Sprite2D(sprite)));

    runtime.extract_render_2d_commands();
    let first = collect_commands(&mut runtime);
    assert!(!first.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::UpsertSprite { node, .. }) if *node == sprite_node
    )));

    runtime
        .with_node_mut::<Camera2D, _, _>(camera_a, |camera| {
            camera.active = false;
        })
        .expect("cam a");
    runtime
        .with_node_mut::<Camera2D, _, _>(camera_b, |camera| {
            camera.active = true;
        })
        .expect("cam b");

    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, RenderCommand::TwoD(Command2D::SetCamera { .. })))
    );
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::UpsertSprite { node, .. }) if *node == sprite_node
    )));
}

#[test]
fn sprite_under_parent_uses_global_transform() {
    let mut runtime = Runtime::new();

    let mut parent_node = Node2D::new();
    parent_node.transform.position.x = 15.0;
    let parent = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node2D(parent_node)));

    let mut sprite = Sprite2D::new();
    sprite.texture = TextureID::from_parts(8, 0);
    sprite.transform.position.x = 1.0;
    let child = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Sprite2D(sprite)));

    if let Some(parent_node) = runtime.nodes.get_mut(parent) {
        parent_node.add_child(child);
    }
    if let Some(child_node) = runtime.nodes.get_mut(child) {
        child_node.parent = parent;
    }
    runtime.mark_transform_dirty_recursive(parent);

    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::UpsertSprite { node, sprite })
            if *node == child
                && sprite.model[2][0] == 16.0
                && sprite.model[2][1] == 0.0
    )));
}

#[test]
fn force_rerender_marks_subtree_dirty() {
    let mut runtime = Runtime::new();

    let parent = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node2D(Node2D::new())));

    let mut sprite = Sprite2D::new();
    sprite.texture = TextureID::from_parts(14, 0);
    let child = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Sprite2D(sprite)));

    runtime
        .nodes
        .get_mut(parent)
        .expect("parent exists")
        .add_child(child);
    runtime.nodes.get_mut(child).expect("child exists").parent = parent;

    runtime.clear_dirty_flags();
    runtime.force_rerender(parent);
    assert_eq!(runtime.dirty_node_count(), 2);
}
