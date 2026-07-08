use super::*;
use perro_nodes::{
    AudioEffectZone2D, AudioEffectZone3D, AudioMask2D, AudioMask3D, AudioPortal2D, AudioPortal3D,
    CollisionShape2D, CollisionShape3D, SceneNode, SceneNodeData, StaticBody2D, StaticBody3D,
    camera_2d::Camera2D, node_2d::Node2D,
};
use perro_resource_api::sub_apis::{Audio, Audio2D, Audio3D};
use perro_runtime_api::sub_apis::NodeAPI;
use perro_structs::{AudioInteraction, BitMask, Quaternion, Transform2D, Transform3D};

fn looped_audio() -> RuntimeAudio<'static> {
    RuntimeAudio {
        source: "res://missing.wav",
        looped: true,
        volume: 1.0,
        effects: AudioEffects::new(),
        from_start: 0.0,
        from_end: 0.0,
    }
}

// Build a StaticBody2D wall (Quad collider) centered at `pos` with the given
// half-extents. Returns the body id.
fn wall_2d(runtime: &mut Runtime, pos: Vector2, width: f32, height: f32) -> NodeID {
    let wall = NodeAPI::create::<StaticBody2D>(runtime);
    // Nearly opaque wall so sealed rooms genuinely muffle.
    if let Some(node) = runtime.nodes.get_mut(wall)
        && let SceneNodeData::StaticBody2D(body) = &mut node.data
    {
        let mut audio = AudioInteraction::new();
        audio.material.transmission = 0.02;
        audio.material.absorption = 0.85;
        audio.material.reflection = 0.1;
        body.audio_interaction = Some(audio);
    }
    let shape = NodeAPI::create::<CollisionShape2D>(runtime);
    assert!(NodeAPI::reparent(runtime, wall, shape));
    if let Some(node) = runtime.nodes.get_mut(shape)
        && let SceneNodeData::CollisionShape2D(shape) = &mut node.data
    {
        shape.shape = perro_nodes::Shape2D::Quad { width, height };
    }
    assert!(NodeAPI::set_global_transform_2d(
        runtime,
        wall,
        Transform2D::new(pos, 0.0, Vector2::ONE),
    ));
    wall
}

fn spatial_options(range: f32) -> SpatialAudioOptions {
    SpatialAudioOptions {
        range,
        audio_layer: BitMask::ALL,
        enable_propagation: true,
        direction_2d: AudioDirection::Omni,
        direction_3d: AudioDirection::Omni,
    }
}

#[test]
fn no_active_sounds_skip_propagation() {
    let mut runtime = Runtime::new();
    runtime.update_audio_propagation(1.0 / 60.0);
    assert_eq!(runtime.audio.counters.active_positional, 0);
    assert_eq!(runtime.audio.counters.raycasts, 0);
}

#[test]
fn unobstructed_sound_stays_direct() {
    let mut runtime = Runtime::new();
    assert!(runtime.play_runtime_audio_2d(
        looped_audio(),
        Vector2::new(5.0, 0.0),
        spatial_options(10.0),
    ));
    runtime.update_audio_propagation(1.0);
    let result = runtime.audio.sounds[0].last_result.expect("result");
    assert_eq!(result.occlusion, 0.0);
    // Squared falloff: half range -> quarter volume.
    assert!(result.volume > 0.2);
    assert_eq!(result.perceived_2d, Some(Vector2::new(5.0, 0.0)));
}

#[test]
fn listener_audio_options_ignore_masked_audio_layer() {
    let mut runtime = Runtime::new();
    runtime.resource_api.set_audio_listener_2d(
        [0.0, 0.0],
        0.0,
        perro_structs::AudioListenerOptions {
            audio_mask: BitMask::with([1]),
            effects: vec![perro_structs::AudioEffect {
                reverb_send: 0.7,
                echo: 0.4,
                dampening: 0.3,
            }],
        },
    );
    assert!(runtime.play_runtime_audio_2d(
        looped_audio(),
        Vector2::new(5.0, 0.0),
        SpatialAudioOptions {
            audio_layer: BitMask::with([2]),
            ..spatial_options(10.0)
        },
    ));
    runtime.update_audio_propagation(1.0);
    let result = runtime.audio.sounds[0].last_result.expect("result");
    assert_eq!(result.low_pass, 0.3);
    assert_eq!(result.echo, 0.4);
    assert_eq!(result.reverb_send, 0.7);

    let mut masked = Runtime::new();
    masked.resource_api.set_audio_listener_2d(
        [0.0, 0.0],
        0.0,
        perro_structs::AudioListenerOptions {
            audio_mask: BitMask::with([1]),
            effects: vec![perro_structs::AudioEffect {
                reverb_send: 0.7,
                echo: 0.4,
                dampening: 0.3,
            }],
        },
    );
    assert!(masked.play_runtime_audio_2d(
        looped_audio(),
        Vector2::new(5.0, 0.0),
        SpatialAudioOptions {
            audio_layer: BitMask::with([1]),
            ..spatial_options(10.0)
        },
    ));
    masked.update_audio_propagation(1.0);
    let result = masked.audio.sounds[0].last_result.expect("masked result");
    assert_eq!(result.low_pass, 0.0);
    assert_eq!(result.echo, 0.0);
    assert_eq!(result.reverb_send, 0.0);
}

#[test]
fn directional_audio_3d_front_is_louder_than_back() {
    let mut front = Runtime::new();
    assert!(front.play_runtime_audio_3d(
        looped_audio(),
        Vector3::new(0.0, 0.0, -5.0),
        SpatialAudioOptions {
            direction_3d: AudioDirection::Directional(Vector3::new(0.0, 0.0, 1.0)),
            ..spatial_options(10.0)
        },
    ));
    front.update_audio_propagation(1.0);
    let front_volume = front.audio.sounds[0].last_result.expect("front").volume;

    let mut back = Runtime::new();
    assert!(back.play_runtime_audio_3d(
        looped_audio(),
        Vector3::new(0.0, 0.0, -5.0),
        SpatialAudioOptions {
            direction_3d: AudioDirection::Directional(Vector3::new(0.0, 0.0, -1.0)),
            ..spatial_options(10.0)
        },
    ));
    back.update_audio_propagation(1.0);
    let back_volume = back.audio.sounds[0].last_result.expect("back").volume;

    assert!(front_volume > back_volume * 4.0);
}

#[test]
fn attached_directional_audio_uses_node_forward() {
    let mut runtime = Runtime::new();
    runtime.resource_api.set_audio_listener_3d(
        [0.0, 0.0, -5.0],
        [0.0, 0.0, 0.0, 1.0],
        perro_structs::AudioListenerOptions::new(),
    );
    let node = NodeAPI::create::<perro_nodes::Node3D>(&mut runtime);
    assert!(NodeAPI::set_global_transform_3d(
        &mut runtime,
        node,
        Transform3D::new(Vector3::ZERO, Quaternion::IDENTITY, Vector3::ONE),
    ));
    assert!(runtime.play_runtime_audio_attached(
        None,
        looped_audio(),
        node,
        SpatialAudioOptions {
            direction_3d: AudioDirection::Directional(Vector3::new(0.0, 0.0, 1.0)),
            ..spatial_options(10.0)
        },
    ));
    runtime.update_audio_propagation(1.0);
    let result = runtime.audio.sounds[0].last_result.expect("result");
    assert!(result.volume > 0.2);
}

#[test]
fn resource_2d_and_3d_audio_enter_propagation_queue() {
    let mut runtime = Runtime::new();
    assert!(runtime.resource_api.play_audio_2d(
        None,
        Audio2D::from_audio(
            Audio::new("res://point2d.wav"),
            Vector2::new(5.0, 0.0),
            10.0
        ),
    ));
    assert!(runtime.resource_api.play_audio_3d(
        None,
        Audio3D::from_audio(
            Audio::new("res://point3d.wav"),
            Vector3::new(0.0, 0.0, -5.0),
            10.0,
        ),
    ));
    assert!(runtime.audio.sounds.is_empty());
    runtime.update_audio_propagation(1.0);
    assert_eq!(runtime.audio.sounds.len(), 2);
    assert!(
        runtime
            .audio
            .sounds
            .iter()
            .any(|sound| matches!(sound.pos, SpatialSoundPos::TwoD(_)))
    );
    assert!(
        runtime
            .audio
            .sounds
            .iter()
            .any(|sound| matches!(sound.pos, SpatialSoundPos::ThreeD(_)))
    );
}

#[test]
fn resource_point_audio_preserves_spatial_options() {
    let mut runtime = Runtime::new();
    assert!(runtime.resource_api.play_audio_2d(
        None,
        Audio2D {
            audio: Audio::new("res://point2d.wav"),
            position: Vector2::new(5.0, 0.0),
            range: 24.0,
            audio_layer: BitMask::from_bits(0x10),
            enable_propagation: false,
            direction: Some(AudioDirection::Directional(Vector2::new(2.0, 0.0))),
        },
    ));
    runtime.update_audio_propagation(1.0);
    let sound = &runtime.audio.sounds[0];
    assert_eq!(sound.options.range, 24.0);
    assert_eq!(sound.options.audio_layer, BitMask::from_bits(0x10));
    assert!(!sound.options.enable_propagation);
    assert_eq!(
        sound.options.direction_2d,
        AudioDirection::Directional(Vector2::new(1.0, 0.0))
    );
}

#[test]
fn active_camera_2d_drives_spatial_audio_listener() {
    let mut runtime = Runtime::new();
    let mut camera = Camera2D::default();
    camera.active = true;
    camera.transform.position = Vector2::new(0.0, 0.0);
    let camera_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Camera2D(camera)));
    runtime.extract_render_2d_commands();

    assert!(runtime.play_runtime_audio_2d(
        looped_audio(),
        Vector2::new(5.0, 0.0),
        spatial_options(10.0),
    ));
    runtime.update_audio_propagation(1.0);
    let first = runtime.audio.sounds[0].last_result.expect("first result");

    assert!(NodeAPI::set_global_transform_2d(
        &mut runtime,
        camera_id,
        Transform2D::new(Vector2::new(2.0, 0.0), 0.0, Vector2::ONE),
    ));
    runtime.extract_render_2d_commands();
    runtime.update_audio_propagation(1.0);
    let moved = runtime.audio.sounds[0].last_result.expect("moved result");

    assert!(moved.volume > first.volume);
    assert!(moved.pan[0] < first.pan[0]);
}

#[test]
fn attached_moving_emitter_recasts_audio_rays_from_new_position() {
    let mut runtime = Runtime::new();
    let wall = NodeAPI::create::<StaticBody2D>(&mut runtime);
    let shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, wall, shape));
    if let Some(node) = runtime.nodes.get_mut(shape)
        && let SceneNodeData::CollisionShape2D(shape) = &mut node.data
    {
        shape.shape = perro_nodes::Shape2D::Quad {
            width: 0.25,
            height: 4.0,
        };
    }
    assert!(NodeAPI::set_global_transform_2d(
        &mut runtime,
        wall,
        Transform2D::new(Vector2::new(2.5, 0.0), 0.0, Vector2::ONE),
    ));
    let emitter = NodeAPI::create::<Node2D>(&mut runtime);
    assert!(NodeAPI::set_global_transform_2d(
        &mut runtime,
        emitter,
        Transform2D::new(Vector2::new(1.0, 0.0), 0.0, Vector2::ONE),
    ));
    assert!(runtime.play_runtime_audio_attached(
        None,
        looped_audio(),
        emitter,
        spatial_options(10.0),
    ));
    runtime.update_audio_propagation(1.0);
    let near = runtime.audio.sounds[0].last_result.expect("near result");

    assert!(NodeAPI::set_global_transform_2d(
        &mut runtime,
        emitter,
        Transform2D::new(Vector2::new(5.0, 0.0), 0.0, Vector2::ONE),
    ));
    runtime.update_audio_propagation(1.0);
    let behind_wall = runtime.audio.sounds[0]
        .last_result
        .expect("behind wall result");

    assert_eq!(near.occlusion, 0.0);
    assert!(behind_wall.occlusion > near.occlusion);
    assert!(behind_wall.low_pass > near.low_pass);
    assert!(behind_wall.volume < near.volume);
}

#[test]
fn wall_between_listener_and_source_muffles() {
    let mut runtime = Runtime::new();
    let wall = NodeAPI::create::<StaticBody2D>(&mut runtime);
    let shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, wall, shape));
    assert!(NodeAPI::set_global_transform_2d(
        &mut runtime,
        wall,
        Transform2D::new(Vector2::new(2.5, 0.0), 0.0, Vector2::ONE),
    ));
    assert!(runtime.play_runtime_audio_2d(
        looped_audio(),
        Vector2::new(5.0, 0.0),
        spatial_options(10.0),
    ));
    runtime.update_audio_propagation(1.0);
    let result = runtime.audio.sounds[0].last_result.expect("result");
    assert!(result.occlusion > 0.0);
    assert!(result.low_pass > 0.0);
    assert!(result.volume < 0.5);
}

#[test]
fn thin_collider_transmits_more_than_thick_collider() {
    let mut thin = Runtime::new();
    let wall = NodeAPI::create::<StaticBody2D>(&mut thin);
    let shape = NodeAPI::create::<CollisionShape2D>(&mut thin);
    assert!(NodeAPI::reparent(&mut thin, wall, shape));
    if let Some(node) = thin.nodes.get_mut(shape)
        && let SceneNodeData::CollisionShape2D(shape) = &mut node.data
    {
        shape.shape = perro_nodes::Shape2D::Quad {
            width: 0.1,
            height: 1.0,
        };
    }
    assert!(NodeAPI::set_global_transform_2d(
        &mut thin,
        wall,
        Transform2D::new(Vector2::new(2.5, 0.0), 0.0, Vector2::ONE),
    ));
    assert!(thin.play_runtime_audio_2d(
        looped_audio(),
        Vector2::new(5.0, 0.0),
        spatial_options(10.0),
    ));
    thin.update_audio_propagation(1.0);
    let thin_volume = thin.audio.sounds[0].last_result.expect("result").volume;

    let mut thick = Runtime::new();
    let wall = NodeAPI::create::<StaticBody2D>(&mut thick);
    let shape = NodeAPI::create::<CollisionShape2D>(&mut thick);
    assert!(NodeAPI::reparent(&mut thick, wall, shape));
    if let Some(node) = thick.nodes.get_mut(shape)
        && let SceneNodeData::CollisionShape2D(shape) = &mut node.data
    {
        shape.shape = perro_nodes::Shape2D::Quad {
            width: 4.0,
            height: 1.0,
        };
    }
    assert!(NodeAPI::set_global_transform_2d(
        &mut thick,
        wall,
        Transform2D::new(Vector2::new(2.5, 0.0), 0.0, Vector2::ONE),
    ));
    assert!(thick.play_runtime_audio_2d(
        looped_audio(),
        Vector2::new(5.0, 0.0),
        spatial_options(10.0),
    ));
    thick.update_audio_propagation(1.0);
    let thick_volume = thick.audio.sounds[0].last_result.expect("result").volume;
    assert!(thin_volume > thick_volume);
}

#[test]
fn corner_path_changes_perceived_direction() {
    let mut runtime = Runtime::new();
    let wall = NodeAPI::create::<StaticBody2D>(&mut runtime);
    let shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, wall, shape));
    assert!(NodeAPI::set_global_transform_2d(
        &mut runtime,
        wall,
        Transform2D::new(Vector2::new(2.5, 0.0), 0.0, Vector2::ONE),
    ));
    assert!(runtime.play_runtime_audio_2d(
        looped_audio(),
        Vector2::new(5.0, 0.0),
        spatial_options(10.0),
    ));
    runtime.update_audio_propagation(1.0);
    let result = runtime.audio.sounds[0].last_result.expect("result");
    assert_ne!(result.perceived_2d, Some(Vector2::new(5.0, 0.0)));
}

#[test]
fn audio_mask_blocks_without_physical_collision() {
    let mut runtime = Runtime::new();
    let mask = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioMask2D(
            AudioMask2D::default(),
        )));
    let shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, mask, shape));
    assert!(NodeAPI::set_global_transform_2d(
        &mut runtime,
        mask,
        Transform2D::new(Vector2::new(2.5, 0.0), 0.0, Vector2::ONE),
    ));
    assert!(runtime.play_runtime_audio_2d(
        looped_audio(),
        Vector2::new(5.0, 0.0),
        spatial_options(10.0),
    ));
    runtime.update_audio_propagation(1.0);
    let result = runtime.audio.sounds[0].last_result.expect("result");
    assert!(result.occlusion > 0.0);
}

#[test]
fn audio_mask_ignores_masked_audio_layer() {
    let mut runtime = Runtime::new();
    let mask = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioMask2D(
            AudioMask2D::default(),
        )));
    if let Some(node) = runtime.nodes.get_mut(mask)
        && let SceneNodeData::AudioMask2D(mask) = &mut node.data
    {
        mask.material.audio_mask = BitMask::with([2]);
    }
    let shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, mask, shape));
    assert!(NodeAPI::set_global_transform_2d(
        &mut runtime,
        mask,
        Transform2D::new(Vector2::new(2.5, 0.0), 0.0, Vector2::ONE),
    ));
    assert!(runtime.play_runtime_audio_2d(
        looped_audio(),
        Vector2::new(5.0, 0.0),
        SpatialAudioOptions {
            audio_layer: BitMask::with([2]),
            ..spatial_options(10.0)
        },
    ));
    runtime.update_audio_propagation(1.0);
    let result = runtime.audio.sounds[0].last_result.expect("result");
    assert_eq!(result.occlusion, 0.0);
    assert_eq!(result.perceived_2d, Some(Vector2::new(5.0, 0.0)));
}

#[test]
fn audio_mask_3d_blocks_without_physical_collision() {
    let mut runtime = Runtime::new();
    let mask = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioMask3D(
            AudioMask3D::default(),
        )));
    let shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, mask, shape));
    if let Some(node) = runtime.nodes.get_mut(shape)
        && let SceneNodeData::CollisionShape3D(shape) = &mut node.data
    {
        shape.shape = perro_nodes::Shape3D::Cube {
            size: Vector3::new(1.0, 2.0, 2.0),
        };
    }
    assert!(NodeAPI::set_global_transform_3d(
        &mut runtime,
        mask,
        Transform3D::new(
            Vector3::new(2.5, 0.0, 0.0),
            Quaternion::IDENTITY,
            Vector3::ONE
        ),
    ));
    assert!(runtime.play_runtime_audio_3d(
        looped_audio(),
        Vector3::new(5.0, 0.0, 0.0),
        spatial_options(10.0),
    ));
    runtime.update_audio_propagation(1.0);
    let result = runtime.audio.sounds[0].last_result.expect("result");
    assert!(result.occlusion > 0.0);
    assert!(result.low_pass > 0.0);
}

#[test]
fn audio_debug_rays_color_direct_vs_wall_diffusion() {
    let mut direct = Runtime::new();
    direct.set_audio_debug_rays(true);
    assert!(direct.play_runtime_audio_3d(
        looped_audio(),
        Vector3::new(5.0, 0.0, 0.0),
        spatial_options(10.0),
    ));
    direct.update_audio_propagation(1.0);
    let mut commands = Vec::new();
    direct.drain_render_commands(&mut commands);
    assert!(commands.iter().any(|cmd| {
        matches!(
            cmd,
            perro_render_bridge::RenderCommand::ThreeD(command)
                if matches!(
                    command.as_ref(),
                    perro_render_bridge::Command3D::DrawDebugLine3D { color, .. }
                        if color[1] > color[2]
                )
        )
    }));
    assert!(commands.iter().all(|cmd| {
        !matches!(
            cmd,
            perro_render_bridge::RenderCommand::ThreeD(command)
                if matches!(
                    command.as_ref(),
                    perro_render_bridge::Command3D::DrawDebugLine3D { node, .. }
                        | perro_render_bridge::Command3D::DrawDebugPoint3D { node, .. }
                        if node.is_nil()
                )
        )
    }));

    let mut wall = Runtime::new();
    wall.set_audio_debug_rays(true);
    let mask = wall.nodes.insert(SceneNode::new(SceneNodeData::AudioMask3D(
        AudioMask3D::default(),
    )));
    if let Some(node) = wall.nodes.get_mut(mask)
        && let SceneNodeData::AudioMask3D(mask) = &mut node.data
    {
        mask.material.transmission = 0.2;
        mask.material.reflection = 0.45;
        mask.material.absorption = 0.6;
    }
    let shape = NodeAPI::create::<CollisionShape3D>(&mut wall);
    assert!(NodeAPI::reparent(&mut wall, mask, shape));
    if let Some(node) = wall.nodes.get_mut(shape)
        && let SceneNodeData::CollisionShape3D(shape) = &mut node.data
    {
        shape.shape = perro_nodes::Shape3D::Cube {
            size: Vector3::new(1.0, 2.0, 2.0),
        };
    }
    assert!(NodeAPI::set_global_transform_3d(
        &mut wall,
        mask,
        Transform3D::new(
            Vector3::new(2.5, 0.0, 0.0),
            Quaternion::IDENTITY,
            Vector3::ONE
        ),
    ));
    assert!(wall.play_runtime_audio_3d(
        looped_audio(),
        Vector3::new(5.0, 0.0, 0.0),
        spatial_options(10.0),
    ));
    wall.update_audio_propagation(1.0);
    commands.clear();
    wall.drain_render_commands(&mut commands);
    assert!(commands.iter().any(|cmd| {
        matches!(
            cmd,
            perro_render_bridge::RenderCommand::ThreeD(command)
                if matches!(
                    command.as_ref(),
                    perro_render_bridge::Command3D::DrawDebugLine3D { color, thickness, .. }
                        if color[2] > color[1] && *thickness > 0.0
                )
        )
    }));
    assert!(commands.iter().any(|cmd| {
        matches!(
            cmd,
            perro_render_bridge::RenderCommand::ThreeD(command)
                if matches!(
                    command.as_ref(),
                    perro_render_bridge::Command3D::DrawDebugPoint3D { color, .. }
                        if color[2] > color[1]
                )
        )
    }));
}

#[test]
fn reflection_loses_strength_per_bounce_and_stops_at_cutoff() {
    let mut runtime = Runtime::new();
    runtime.audio.config.energy_cutoff = 0.02;
    runtime.audio.config.max_bounces_2d = 4;
    let four_bounces = runtime.bounce_energy(0.5, runtime.audio.config.max_bounces_2d);
    runtime.audio.config.max_bounces_2d = 1;
    let one_bounce = runtime.bounce_energy(0.5, runtime.audio.config.max_bounces_2d);
    assert!(four_bounces > one_bounce);
    runtime.audio.config.energy_cutoff = 0.6;
    assert_eq!(runtime.bounce_energy(0.5, 4), 0.0);
}

#[test]
fn physics_body_reflects_spatial_audio_from_geometry() {
    let mut runtime = Runtime::new();
    let floor = NodeAPI::create::<StaticBody2D>(&mut runtime);
    if let Some(node) = runtime.nodes.get_mut(floor)
        && let SceneNodeData::StaticBody2D(body) = &mut node.data
    {
        let mut audio = AudioInteraction::new();
        audio.material.absorption = 0.0;
        audio.material.reflection = 1.0;
        audio.material.low_pass_strength = 0.1;
        body.audio_interaction = Some(audio);
    }
    let shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, floor, shape));
    if let Some(node) = runtime.nodes.get_mut(shape)
        && let SceneNodeData::CollisionShape2D(shape) = &mut node.data
    {
        shape.shape = perro_nodes::Shape2D::Quad {
            width: 4.0,
            height: 0.1,
        };
    }
    assert!(NodeAPI::set_global_transform_2d(
        &mut runtime,
        floor,
        Transform2D::new(Vector2::new(2.0, -2.0), 0.0, Vector2::ONE),
    ));
    assert!(runtime.play_runtime_audio_2d(
        looped_audio(),
        Vector2::new(0.0, 0.0),
        spatial_options(8.0),
    ));
    runtime.resource_api.set_audio_listener_2d(
        [4.0, 0.0],
        0.0,
        perro_structs::AudioListenerOptions::new(),
    );
    runtime.update_audio_propagation(1.0);
    let result = runtime.audio.sounds[0].last_result.expect("result");
    assert!(result.reflection > 0.5);
    assert!(result.echo > 0.15);
}

#[test]
fn bounce_audio_effect_zone_reflects_instead_of_pass_through() {
    let mut runtime = Runtime::new();
    let zone = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioEffectZone2D(
            AudioEffectZone2D::default(),
        )));
    if let Some(node) = runtime.nodes.get_mut(zone)
        && let SceneNodeData::AudioEffectZone2D(zone) = &mut node.data
    {
        zone.bounce = true;
        zone.effects[0].reverb_send = 0.6;
        zone.effects[0].echo = 0.8;
        zone.effects[0].dampening = 0.2;
    }
    let shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, zone, shape));
    if let Some(node) = runtime.nodes.get_mut(shape)
        && let SceneNodeData::CollisionShape2D(shape) = &mut node.data
    {
        shape.shape = perro_nodes::Shape2D::Quad {
            width: 4.0,
            height: 0.1,
        };
    }
    assert!(NodeAPI::set_global_transform_2d(
        &mut runtime,
        zone,
        Transform2D::new(Vector2::new(2.0, -2.0), 0.0, Vector2::ONE),
    ));
    assert!(runtime.play_runtime_audio_2d(
        looped_audio(),
        Vector2::new(0.0, 0.0),
        spatial_options(8.0),
    ));
    runtime.resource_api.set_audio_listener_2d(
        [4.0, 0.0],
        0.0,
        perro_structs::AudioListenerOptions::new(),
    );
    runtime.update_audio_propagation(1.0);
    let result = runtime.audio.sounds[0].last_result.expect("result");
    assert!(result.reflection >= 0.8);
    assert!(result.echo >= 0.8);
    assert!(result.reverb_send >= 0.6);
}

#[test]
fn audio_portal_improves_corner_opening_path() {
    let mut without_portal = Runtime::new();
    let wall = NodeAPI::create::<StaticBody2D>(&mut without_portal);
    let shape = NodeAPI::create::<CollisionShape2D>(&mut without_portal);
    assert!(NodeAPI::reparent(&mut without_portal, wall, shape));
    assert!(NodeAPI::set_global_transform_2d(
        &mut without_portal,
        wall,
        Transform2D::new(Vector2::new(2.5, 0.0), 0.0, Vector2::ONE),
    ));
    assert!(without_portal.play_runtime_audio_2d(
        looped_audio(),
        Vector2::new(5.0, 0.0),
        spatial_options(10.0),
    ));
    without_portal.update_audio_propagation(1.0);
    let blocked = without_portal.audio.sounds[0].last_result.expect("result");

    let mut with_portal = Runtime::new();
    let wall = NodeAPI::create::<StaticBody2D>(&mut with_portal);
    let shape = NodeAPI::create::<CollisionShape2D>(&mut with_portal);
    assert!(NodeAPI::reparent(&mut with_portal, wall, shape));
    assert!(NodeAPI::set_global_transform_2d(
        &mut with_portal,
        wall,
        Transform2D::new(Vector2::new(2.5, 0.0), 0.0, Vector2::ONE),
    ));
    let portal = with_portal
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioPortal2D(
            AudioPortal2D::default(),
        )));
    let portal_exit = with_portal
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioPortal2D(
            AudioPortal2D::default(),
        )));
    if let Some(node) = with_portal.nodes.get_mut(portal)
        && let SceneNodeData::AudioPortal2D(portal) = &mut node.data
    {
        portal.targets.push(portal_exit);
    }
    if let Some(node) = with_portal.nodes.get_mut(portal_exit)
        && let SceneNodeData::AudioPortal2D(exit) = &mut node.data
    {
        exit.targets.push(portal);
    }
    let portal_shape = NodeAPI::create::<CollisionShape2D>(&mut with_portal);
    let portal_exit_shape = NodeAPI::create::<CollisionShape2D>(&mut with_portal);
    assert!(NodeAPI::reparent(&mut with_portal, portal, portal_shape));
    assert!(NodeAPI::reparent(
        &mut with_portal,
        portal_exit,
        portal_exit_shape
    ));
    assert!(NodeAPI::set_global_transform_2d(
        &mut with_portal,
        portal,
        Transform2D::new(Vector2::new(2.5, 0.0), 0.0, Vector2::ONE),
    ));
    assert!(NodeAPI::set_global_transform_2d(
        &mut with_portal,
        portal_exit,
        Transform2D::new(Vector2::new(0.5, 0.0), 0.0, Vector2::ONE),
    ));
    assert!(with_portal.play_runtime_audio_2d(
        looped_audio(),
        Vector2::new(5.0, 0.0),
        spatial_options(10.0),
    ));
    with_portal.update_audio_propagation(1.0);
    let opened = with_portal.audio.sounds[0].last_result.expect("result");
    assert!(opened.volume > blocked.volume);
    assert!(opened.low_pass < blocked.low_pass);
    assert_eq!(opened.perceived_2d, Some(Vector2::new(1.0, 0.0)));
}

#[test]
fn audio_portal_2d_transforms_exit_direction_with_rotation() {
    let mut runtime = Runtime::new();
    let portal = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioPortal2D(
            AudioPortal2D::default(),
        )));
    let portal_exit = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioPortal2D(
            AudioPortal2D::default(),
        )));
    if let Some(node) = runtime.nodes.get_mut(portal)
        && let SceneNodeData::AudioPortal2D(portal) = &mut node.data
    {
        portal.targets.push(portal_exit);
    }
    let portal_shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, portal, portal_shape));
    assert!(NodeAPI::set_global_transform_2d(
        &mut runtime,
        portal,
        Transform2D::new(Vector2::new(4.0, 0.0), 0.0, Vector2::ONE),
    ));
    assert!(NodeAPI::set_global_transform_2d(
        &mut runtime,
        portal_exit,
        Transform2D::new(
            Vector2::new(0.0, -0.5),
            -std::f32::consts::FRAC_PI_2,
            Vector2::ONE
        ),
    ));
    assert!(runtime.play_runtime_audio_2d(
        looped_audio(),
        Vector2::new(5.0, 0.0),
        spatial_options(10.0),
    ));
    runtime.update_audio_propagation(1.0);
    let opened = runtime.audio.sounds[0].last_result.expect("result");
    let perceived = opened.perceived_2d.expect("perceived");
    assert!(perceived.distance_to(Vector2::new(0.0, -1.0)) < 0.0001);
}

#[test]
fn audio_portal_2d_chains_multiple_hops() {
    let mut runtime = Runtime::new();
    let portal_a = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioPortal2D(
            AudioPortal2D::default(),
        )));
    let portal_b = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioPortal2D(
            AudioPortal2D::default(),
        )));
    let portal_c = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioPortal2D(
            AudioPortal2D::default(),
        )));
    let portal_d = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioPortal2D(
            AudioPortal2D::default(),
        )));
    if let Some(node) = runtime.nodes.get_mut(portal_a)
        && let SceneNodeData::AudioPortal2D(portal) = &mut node.data
    {
        portal.targets.push(portal_b);
    }
    if let Some(node) = runtime.nodes.get_mut(portal_c)
        && let SceneNodeData::AudioPortal2D(portal) = &mut node.data
    {
        portal.targets.push(portal_d);
    }
    let shape_a = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    let shape_c = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, portal_a, shape_a));
    assert!(NodeAPI::reparent(&mut runtime, portal_c, shape_c));
    assert!(NodeAPI::set_global_transform_2d(
        &mut runtime,
        portal_a,
        Transform2D::new(Vector2::new(4.0, 0.0), 0.0, Vector2::ONE),
    ));
    assert!(NodeAPI::set_global_transform_2d(
        &mut runtime,
        portal_b,
        Transform2D::new(Vector2::new(2.5, 0.0), 0.0, Vector2::ONE),
    ));
    assert!(NodeAPI::set_global_transform_2d(
        &mut runtime,
        portal_c,
        Transform2D::new(Vector2::new(2.0, 0.0), 0.0, Vector2::ONE),
    ));
    assert!(NodeAPI::set_global_transform_2d(
        &mut runtime,
        portal_d,
        Transform2D::new(Vector2::new(0.5, 0.0), 0.0, Vector2::ONE),
    ));
    assert!(runtime.play_runtime_audio_2d(
        looped_audio(),
        Vector2::new(5.0, 0.0),
        spatial_options(10.0),
    ));
    runtime.update_audio_propagation(1.0);
    let result = runtime.audio.sounds[0].last_result.expect("result");
    assert_eq!(result.perceived_2d, Some(Vector2::new(1.0, 0.0)));
}

#[test]
fn audio_portal_skip_is_per_ray_branch() {
    let mut runtime = Runtime::new();
    let portal_a = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioPortal2D(
            AudioPortal2D::default(),
        )));
    let portal_b = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioPortal2D(
            AudioPortal2D::default(),
        )));
    let portal_c = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioPortal2D(
            AudioPortal2D::default(),
        )));
    if let Some(node) = runtime.nodes.get_mut(portal_a)
        && let SceneNodeData::AudioPortal2D(portal) = &mut node.data
    {
        portal.targets.push(portal_b);
    }
    if let Some(node) = runtime.nodes.get_mut(portal_b)
        && let SceneNodeData::AudioPortal2D(portal) = &mut node.data
    {
        portal.targets.push(portal_c);
    }
    let shape_a = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    let shape_b = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, portal_a, shape_a));
    assert!(NodeAPI::reparent(&mut runtime, portal_b, shape_b));
    assert!(NodeAPI::set_global_transform_2d(
        &mut runtime,
        portal_a,
        Transform2D::new(Vector2::new(4.0, 0.0), 0.0, Vector2::ONE),
    ));
    assert!(NodeAPI::set_global_transform_2d(
        &mut runtime,
        portal_b,
        Transform2D::new(Vector2::new(2.5, 0.0), 0.0, Vector2::ONE),
    ));
    assert!(NodeAPI::set_global_transform_2d(
        &mut runtime,
        portal_c,
        Transform2D::new(Vector2::new(0.5, 0.0), 0.0, Vector2::ONE),
    ));
    assert!(runtime.play_runtime_audio_2d(
        looped_audio(),
        Vector2::new(5.0, 0.0),
        spatial_options(10.0),
    ));
    assert!(runtime.play_runtime_audio_2d(
        looped_audio(),
        Vector2::new(3.5, 0.0),
        spatial_options(10.0),
    ));
    runtime.update_audio_propagation(1.0);

    let mut perceived: Vec<Vector2> = runtime
        .audio
        .sounds
        .iter()
        .filter_map(|sound| sound.last_result.and_then(|result| result.perceived_2d))
        .collect();
    perceived.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
    assert_eq!(
        perceived,
        vec![Vector2::new(1.0, 0.0), Vector2::new(3.0, 0.0)]
    );
}

#[test]
fn audio_portal_3d_transports_through_connected_exit() {
    let mut without_portal = Runtime::new();
    let wall = NodeAPI::create::<StaticBody3D>(&mut without_portal);
    let shape = NodeAPI::create::<CollisionShape3D>(&mut without_portal);
    assert!(NodeAPI::reparent(&mut without_portal, wall, shape));
    assert!(NodeAPI::set_global_transform_3d(
        &mut without_portal,
        wall,
        Transform3D::new(
            Vector3::new(0.0, 0.0, -2.5),
            Quaternion::IDENTITY,
            Vector3::ONE
        ),
    ));
    assert!(without_portal.play_runtime_audio_3d(
        looped_audio(),
        Vector3::new(0.0, 0.0, -5.0),
        spatial_options(10.0),
    ));
    without_portal.update_audio_propagation(1.0);
    let blocked = without_portal.audio.sounds[0].last_result.expect("result");

    let mut with_portal = Runtime::new();
    let wall = NodeAPI::create::<StaticBody3D>(&mut with_portal);
    let shape = NodeAPI::create::<CollisionShape3D>(&mut with_portal);
    assert!(NodeAPI::reparent(&mut with_portal, wall, shape));
    assert!(NodeAPI::set_global_transform_3d(
        &mut with_portal,
        wall,
        Transform3D::new(
            Vector3::new(0.0, 0.0, -2.5),
            Quaternion::IDENTITY,
            Vector3::ONE
        ),
    ));
    let portal = with_portal
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioPortal3D(
            AudioPortal3D::default(),
        )));
    let portal_exit = with_portal
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioPortal3D(
            AudioPortal3D::default(),
        )));
    if let Some(node) = with_portal.nodes.get_mut(portal)
        && let SceneNodeData::AudioPortal3D(portal) = &mut node.data
    {
        portal.targets.push(portal_exit);
    }
    let portal_shape = NodeAPI::create::<CollisionShape3D>(&mut with_portal);
    assert!(NodeAPI::reparent(&mut with_portal, portal, portal_shape));
    assert!(NodeAPI::set_global_transform_3d(
        &mut with_portal,
        portal,
        Transform3D::new(
            Vector3::new(0.0, 0.0, -2.5),
            Quaternion::IDENTITY,
            Vector3::ONE
        ),
    ));
    assert!(NodeAPI::set_global_transform_3d(
        &mut with_portal,
        portal_exit,
        Transform3D::new(
            Vector3::new(0.0, 0.0, -0.5),
            Quaternion::IDENTITY,
            Vector3::ONE
        ),
    ));
    assert!(with_portal.play_runtime_audio_3d(
        looped_audio(),
        Vector3::new(0.0, 0.0, -5.0),
        spatial_options(10.0),
    ));
    with_portal.update_audio_propagation(1.0);
    let opened = with_portal.audio.sounds[0].last_result.expect("result");
    assert!(opened.volume > blocked.volume);
    assert!(opened.low_pass < blocked.low_pass);
    assert_eq!(opened.perceived_3d, Some(Vector3::new(0.0, 0.0, -1.0)));
}

#[test]
fn audio_effect_zone_2d_mixes_effect_when_source_enters() {
    let mut runtime = Runtime::new();
    let zone = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioEffectZone2D(
            AudioEffectZone2D::default(),
        )));
    if let Some(node) = runtime.nodes.get_mut(zone)
        && let SceneNodeData::AudioEffectZone2D(zone) = &mut node.data
    {
        zone.effects[0].reverb_send = 0.7;
        zone.effects[0].echo = 0.4;
        zone.effects[0].dampening = 0.5;
    }
    let shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, zone, shape));
    if let Some(node) = runtime.nodes.get_mut(shape)
        && let SceneNodeData::CollisionShape2D(shape) = &mut node.data
    {
        shape.shape = perro_nodes::Shape2D::Quad {
            width: 4.0,
            height: 4.0,
        };
    }
    assert!(NodeAPI::set_global_transform_2d(
        &mut runtime,
        shape,
        Transform2D::new(Vector2::new(5.0, 0.0), 0.0, Vector2::ONE),
    ));
    assert!(runtime.play_runtime_audio_2d(
        looped_audio(),
        Vector2::new(5.0, 0.0),
        spatial_options(10.0),
    ));
    runtime.update_audio_propagation(1.0);
    let result = runtime.audio.sounds[0].last_result.expect("result");
    assert!(result.reverb_send >= 0.7);
    assert!(result.reflection >= 0.4);
    assert!(result.low_pass >= 0.5);
    assert!(result.volume < 0.5);
}

#[test]
fn audio_effect_zone_ignores_masked_audio_layer() {
    let mut runtime = Runtime::new();
    let zone = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioEffectZone2D(
            AudioEffectZone2D::default(),
        )));
    if let Some(node) = runtime.nodes.get_mut(zone)
        && let SceneNodeData::AudioEffectZone2D(zone) = &mut node.data
    {
        zone.audio_mask = BitMask::with([2]);
        zone.effects[0].reverb_send = 0.7;
        zone.effects[0].echo = 0.4;
        zone.effects[0].dampening = 0.5;
    }
    let shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, zone, shape));
    if let Some(node) = runtime.nodes.get_mut(shape)
        && let SceneNodeData::CollisionShape2D(shape) = &mut node.data
    {
        shape.shape = perro_nodes::Shape2D::Quad {
            width: 4.0,
            height: 4.0,
        };
    }
    assert!(NodeAPI::set_global_transform_2d(
        &mut runtime,
        shape,
        Transform2D::new(Vector2::new(5.0, 0.0), 0.0, Vector2::ONE),
    ));
    assert!(runtime.play_runtime_audio_2d(
        looped_audio(),
        Vector2::new(5.0, 0.0),
        SpatialAudioOptions {
            audio_layer: BitMask::with([2]),
            ..spatial_options(10.0)
        },
    ));
    runtime.update_audio_propagation(1.0);
    let result = runtime.audio.sounds[0].last_result.expect("result");
    assert_eq!(result.reverb_send, 0.0);
    assert_eq!(result.reflection, 0.0);
    assert_eq!(result.low_pass, 0.0);
}

#[test]
fn audio_effect_zone_3d_mixes_effect_when_path_crosses() {
    let mut runtime = Runtime::new();
    let zone = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioEffectZone3D(
            AudioEffectZone3D::default(),
        )));
    if let Some(node) = runtime.nodes.get_mut(zone)
        && let SceneNodeData::AudioEffectZone3D(zone) = &mut node.data
    {
        zone.effects[0].reverb_send = 0.6;
        zone.effects[0].echo = 0.3;
        zone.effects[0].dampening = 0.4;
    }
    let shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, zone, shape));
    if let Some(node) = runtime.nodes.get_mut(shape)
        && let SceneNodeData::CollisionShape3D(shape) = &mut node.data
    {
        shape.shape = perro_nodes::Shape3D::Cube {
            size: Vector3::new(2.0, 2.0, 2.0),
        };
    }
    assert!(NodeAPI::set_global_transform_3d(
        &mut runtime,
        shape,
        Transform3D::new(
            Vector3::new(0.0, 0.0, -2.5),
            Quaternion::IDENTITY,
            Vector3::ONE
        ),
    ));
    assert!(runtime.play_runtime_audio_3d(
        looped_audio(),
        Vector3::new(0.0, 0.0, -5.0),
        spatial_options(10.0),
    ));
    runtime.update_audio_propagation(1.0);
    let result = runtime.audio.sounds[0].last_result.expect("result");
    assert!(result.reverb_send >= 0.6);
    assert!(result.reflection >= 0.3);
    assert!(result.low_pass >= 0.4);
    assert!(result.volume < 0.5);
}

#[test]
fn attached_sound_removed_with_node() {
    let mut runtime = Runtime::new();
    let node = NodeAPI::create::<perro_nodes::Node2D>(&mut runtime);
    assert!(
        runtime.play_runtime_audio_attached(None, looped_audio(), node, spatial_options(10.0),)
    );
    assert!(NodeAPI::set_global_transform_2d(
        &mut runtime,
        node,
        Transform2D::new(Vector2::new(3.0, 0.0), 0.0, Vector2::ONE),
    ));
    runtime.update_audio_propagation(1.0);
    assert_eq!(
        runtime.audio.sounds[0].last_2d,
        Some(Vector2::new(3.0, 0.0))
    );
    assert!(NodeAPI::remove_node(&mut runtime, node));
    runtime.update_audio_propagation(1.0);
    assert!(runtime.audio.sounds.is_empty());
}

#[test]
fn removed_attached_source_clears_3d_audio_debug_rays() {
    let mut runtime = Runtime::new();
    runtime.set_audio_debug_rays(true);
    runtime.resource_api.set_audio_listener_3d(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
        perro_structs::AudioListenerOptions::new(),
    );
    let node = NodeAPI::create::<perro_nodes::Node3D>(&mut runtime);
    assert!(NodeAPI::set_global_transform_3d(
        &mut runtime,
        node,
        Transform3D::new(
            Vector3::new(0.0, 0.0, -5.0),
            Quaternion::IDENTITY,
            Vector3::ONE
        ),
    ));
    assert!(
        runtime.play_runtime_audio_attached(None, looped_audio(), node, spatial_options(10.0),)
    );
    runtime.update_audio_propagation(1.0);
    let mut commands = Vec::new();
    runtime.drain_render_commands(&mut commands);
    assert!(commands.iter().any(|cmd| {
        matches!(
            cmd,
            perro_render_bridge::RenderCommand::ThreeD(command)
                if matches!(command.as_ref(), perro_render_bridge::Command3D::DrawDebugLine3D { .. })
        )
    }));

    assert!(NodeAPI::remove_node(&mut runtime, node));
    commands.clear();
    runtime.drain_render_commands(&mut commands);
    assert!(commands.iter().any(|cmd| {
        matches!(
            cmd,
            perro_render_bridge::RenderCommand::ThreeD(command)
                if matches!(command.as_ref(), perro_render_bridge::Command3D::RemoveNode { .. })
        )
    }));
}

#[test]
fn stop_attached_matches_node_and_source() {
    let mut runtime = Runtime::new();
    let a = NodeAPI::create::<perro_nodes::Node2D>(&mut runtime);
    let b = NodeAPI::create::<perro_nodes::Node2D>(&mut runtime);
    assert!(runtime.play_runtime_audio_attached(None, looped_audio(), a, spatial_options(10.0)));
    assert!(runtime.play_runtime_audio_attached(None, looped_audio(), b, spatial_options(10.0)));
    assert!(runtime.stop_runtime_audio_attached(a, "res://missing.wav"));
    assert_eq!(runtime.audio.sounds.len(), 1);
    assert!(matches!(runtime.audio.sounds[0].pos, SpatialSoundPos::Attached(id) if id == b));
}

// Build a box room of StaticBody2D walls centered on the origin (listener).
// `right_gap` leaves the right wall (facing +x) open around y=0 so audio can
// reconcile out through it.
fn box_room_2d(runtime: &mut Runtime, half: f32, right_gap: bool) {
    let t = 0.4;
    // Top + bottom + left walls: full span.
    wall_2d(runtime, Vector2::new(0.0, half), half * 2.0, t);
    wall_2d(runtime, Vector2::new(0.0, -half), half * 2.0, t);
    wall_2d(runtime, Vector2::new(-half, 0.0), t, half * 2.0);
    if right_gap {
        // Split right wall into two, leaving a gap of ~2 units around y=0.
        let seg = (half - 1.0) * 0.5;
        wall_2d(runtime, Vector2::new(half, 1.0 + seg), t, seg * 2.0);
        wall_2d(runtime, Vector2::new(half, -(1.0 + seg)), t, seg * 2.0);
    } else {
        wall_2d(runtime, Vector2::new(half, 0.0), t, half * 2.0);
    }
}

#[test]
fn reconcile_window_2d_opens_muffled_room() {
    let emitter = Vector2::new(12.0, 3.0);
    // Sealed room: emitter fully boxed off from the listener.
    let mut sealed = Runtime::new();
    box_room_2d(&mut sealed, 4.0, false);
    assert!(sealed.play_runtime_audio_2d(looped_audio(), emitter, spatial_options(16.0)));
    sealed.update_audio_propagation(1.0);
    let sealed_result = sealed.audio.sounds[0].last_result.expect("sealed");

    // Same room with a window in the wall facing the emitter. Emitter sits
    // off-axis so the DIRECT line is blocked by the wall segment (forcing the
    // reconciler to engage), but audio can still escape through the window.
    let mut open = Runtime::new();
    box_room_2d(&mut open, 4.0, true);
    assert!(open.play_runtime_audio_2d(looped_audio(), emitter, spatial_options(16.0)));
    open.update_audio_propagation(1.0);
    let open_result = open.audio.sounds[0].last_result.expect("open");

    // Window path is significantly louder than the sealed room.
    assert!(
        open_result.volume > sealed_result.volume * 1.5,
        "open {} vs sealed {}",
        open_result.volume,
        sealed_result.volume
    );
    // Reconciler found a virtual source; occlusion is lower than sealed.
    assert!(open.audio.sounds[0].aperture_2d.is_some());
    assert!(open_result.occlusion < sealed_result.occlusion);
    // Perceived comes from the reconciled path, not the raw source position.
    let perceived = open_result.perceived_2d.expect("perceived");
    assert_ne!(perceived, emitter);
}

#[test]
fn reconcile_sealed_room_stays_muffled() {
    let mut runtime = Runtime::new();
    box_room_2d(&mut runtime, 4.0, false);
    assert!(runtime.play_runtime_audio_2d(
        looped_audio(),
        Vector2::new(12.0, 3.0),
        spatial_options(16.0),
    ));
    runtime.update_audio_propagation(1.0);
    let result = runtime.audio.sounds[0].last_result.expect("result");
    assert!(result.occlusion > 0.3, "occlusion {}", result.occlusion);
    assert!(result.volume < 0.2, "volume {}", result.volume);
}

#[test]
fn reconcile_aperture_cache_cuts_second_tick_rays() {
    let mut runtime = Runtime::new();
    box_room_2d(&mut runtime, 4.0, true);
    assert!(runtime.play_runtime_audio_2d(
        looped_audio(),
        Vector2::new(12.0, 3.0),
        spatial_options(16.0),
    ));
    runtime.update_audio_propagation(1.0);
    let first_rays = runtime.audio.counters.raycasts;
    assert!(
        runtime.audio.sounds[0].aperture_2d.is_some(),
        "reconciler should have found an aperture"
    );
    // Second tick verifies the cached aperture with a couple of cheap rays
    // instead of re-running the full bidirectional fan.
    runtime.update_audio_propagation(1.0);
    let second_rays = runtime.audio.counters.raycasts;
    assert!(
        second_rays < first_rays,
        "cached tick {} should cast fewer rays than search tick {}",
        second_rays,
        first_rays
    );
}

#[test]
fn reconcile_openness_hysteresis_does_not_oscillate() {
    // A wall the emitter sits just behind: the openness probes straddle its
    // edge. Flip the wall in/out on alternate ticks and confirm the stored
    // openness (hence volume) does not swing hard each tick.
    let mut runtime = Runtime::new();
    let wall = wall_2d(&mut runtime, Vector2::new(2.5, 0.0), 0.25, 4.0);
    assert!(runtime.play_runtime_audio_2d(
        looped_audio(),
        Vector2::new(5.0, 0.0),
        spatial_options(10.0),
    ));
    // Prime the field with the wall present.
    runtime.update_audio_propagation(1.0);
    let mut volumes = Vec::new();
    for tick in 0..6 {
        // Toggle a probe's blocked state by nudging the wall aside and back.
        let x = if tick % 2 == 0 { 2.5 } else { 40.0 };
        assert!(NodeAPI::set_global_transform_2d(
            &mut runtime,
            wall,
            Transform2D::new(Vector2::new(x, 0.0), 0.0, Vector2::ONE),
        ));
        runtime.update_audio_propagation(1.0);
        volumes.push(runtime.audio.sounds[0].last_result.expect("result").volume);
    }
    // Consecutive volumes must not swing by more than a bounded step: the
    // hysteresis + result smoothing damps the alternating probe flip.
    for pair in volumes.windows(2) {
        let swing = (pair[1] - pair[0]).abs();
        assert!(
            swing < 0.4,
            "volume swing {} too large: {:?}",
            swing,
            volumes
        );
    }
}
