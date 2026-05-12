use super::*;
use perro_nodes::{
    AudioMask2D, AudioMask3D, AudioPortal2D, AudioPortal3D, AudioZone2D, AudioZone3D,
    CollisionShape2D, CollisionShape3D, SceneNode, SceneNodeData, StaticBody2D, StaticBody3D,
};
use perro_resource_context::sub_apis::{Audio, Audio2D, Audio3D};
use perro_runtime_context::sub_apis::NodeAPI;
use perro_structs::{Quaternion, Transform2D, Transform3D};

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

fn spatial_options(range: f32) -> SpatialAudioOptions {
    SpatialAudioOptions {
        range,
        occlusion_mask: u32::MAX,
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
    assert!(result.volume > 0.4);
    assert_eq!(result.perceived_2d, Some(Vector2::new(5.0, 0.0)));
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
    runtime
        .resource_api
        .set_audio_listener_3d([0.0, 0.0, -5.0], [0.0, 0.0, 0.0, 1.0]);
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
    assert!(result.volume > 0.4);
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
            occlusion_mask: 0x10,
            enable_propagation: false,
            direction: Some(AudioDirection::Directional(Vector2::new(2.0, 0.0))),
        },
    ));
    runtime.update_audio_propagation(1.0);
    let sound = &runtime.audio.sounds[0];
    assert_eq!(sound.options.range, 24.0);
    assert_eq!(sound.options.occlusion_mask, 0x10);
    assert!(!sound.options.enable_propagation);
    assert_eq!(
        sound.options.direction_2d,
        AudioDirection::Directional(Vector2::new(1.0, 0.0))
    );
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
            AudioMask2D::new(),
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
fn audio_mask_3d_blocks_without_physical_collision() {
    let mut runtime = Runtime::new();
    let mask = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioMask3D(
            AudioMask3D::new(),
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
            AudioPortal2D::new(),
        )));
    let portal_exit = with_portal
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioPortal2D(
            AudioPortal2D::new(),
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
            AudioPortal2D::new(),
        )));
    let portal_exit = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioPortal2D(
            AudioPortal2D::new(),
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
            AudioPortal2D::new(),
        )));
    let portal_b = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioPortal2D(
            AudioPortal2D::new(),
        )));
    let portal_c = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioPortal2D(
            AudioPortal2D::new(),
        )));
    let portal_d = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioPortal2D(
            AudioPortal2D::new(),
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
            AudioPortal2D::new(),
        )));
    let portal_b = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioPortal2D(
            AudioPortal2D::new(),
        )));
    let portal_c = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioPortal2D(
            AudioPortal2D::new(),
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
            AudioPortal3D::new(),
        )));
    let portal_exit = with_portal
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioPortal3D(
            AudioPortal3D::new(),
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
fn audio_zone_2d_mixes_effect_when_source_enters() {
    let mut runtime = Runtime::new();
    let zone = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioZone2D(
            AudioZone2D::new(),
        )));
    if let Some(node) = runtime.nodes.get_mut(zone)
        && let SceneNodeData::AudioZone2D(zone) = &mut node.data
    {
        zone.effect.reverb_send = 0.7;
        zone.effect.echo = 0.4;
        zone.effect.dampening = 0.5;
        zone.affect_listener = false;
        zone.affect_path = false;
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
fn audio_zone_3d_mixes_effect_when_path_crosses() {
    let mut runtime = Runtime::new();
    let zone = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AudioZone3D(
            AudioZone3D::new(),
        )));
    if let Some(node) = runtime.nodes.get_mut(zone)
        && let SceneNodeData::AudioZone3D(zone) = &mut node.data
    {
        zone.effect.reverb_send = 0.6;
        zone.effect.echo = 0.3;
        zone.effect.dampening = 0.4;
        zone.affect_listener = false;
        zone.affect_emitters = false;
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
fn attached_sound_follows_and_freezes_after_remove() {
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
    assert_eq!(
        runtime.audio.sounds[0].last_2d,
        Some(Vector2::new(3.0, 0.0))
    );
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
