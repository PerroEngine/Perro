use super::Runtime;
use perro_ids::TextureID;
use perro_nodes::{
    AmbientLight2D, CollisionShape2D, PointLight2D, RayLight2D, SceneNode, SceneNodeData, Shape2D,
    SpotLight2D, StaticBody2D, WaterBody2D,
    camera_2d::Camera2D,
    node_2d::Node2D,
    particle_emitter_2d::ParticleEmitter2D,
    sprite_2d::{AnimatedSprite, AnimatedSprite2D, Sprite2D},
};
use perro_render_bridge::{Command2D, ParticlePath2D, RenderCommand, RenderEvent, ResourceCommand};
use perro_runtime_api::sub_apis::{NodeAPI, NodeCreationTemplate};
use perro_structs::{BitMask, Vector2};

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
fn create_nodes_10k_sprites_emit_render_commands() {
    let mut runtime = Runtime::new();
    let templates = vec![NodeCreationTemplate::new::<Sprite2D>(); 10_000];
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
    assert_eq!(second.len(), 1);
    assert!(matches!(
        second[0],
        RenderCommand::TwoD(Command2D::RemoveNode { node }) if node == expected_node
    ));
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
