use super::Runtime;
use perro_animation::{
    AnimationClip, AnimationObject, AnimationObjectKey, AnimationObjectTrack, AnimationTrackValue,
};
use perro_ids::{MaterialID, MeshID, TextureID};
use perro_nodes::{
    AnimationPlayer, CameraProjection, CollisionShape3D, Label3D, SceneNode, SceneNodeData,
    StaticBody3D, TextDecal3D, WaterBody3D,
    ambient_light_3d::AmbientLight3D,
    camera_3d::Camera3D,
    mesh_instance_3d::MeshInstance3D,
    mesh_instance_3d::MeshSurfaceBinding,
    multi_mesh_instance_3d::MultiMeshInstance3D,
    node_3d::Node3D,
    physics_3d::RigidBody3D,
    physics_3d::Shape3D,
    ray_light_3d::RayLight3D,
    skeleton_3d::{Bone3D, Skeleton3D},
    sky_3d::Sky3D,
    sprite_3d::Sprite3D,
};
use perro_render_bridge::{
    CameraProjectionState, Command3D, Material3D, Mesh3D, RenderCommand, RenderEvent,
    ResourceCommand, StandardMaterial3D, UiCommand,
};
use perro_resource_api::sub_apis::{MaterialAPI, MeshAPI, TextureAPI};
use perro_runtime_api::sub_apis::{AnimPlayerAPI, NodeAPI};
use perro_scene::{Node3DField, NodeField, NodeType};
use perro_structs::Transform3D;
use perro_structs::{BitMask, Color, Quaternion, Vector3};
use std::borrow::Cow;
use std::sync::Arc;

fn collect_commands(runtime: &mut Runtime) -> Vec<RenderCommand> {
    let mut out = Vec::new();
    runtime.drain_render_commands(&mut out);
    out
}

fn collect_resource_texture_request(
    runtime: &mut Runtime,
    texture: TextureID,
) -> perro_render_bridge::RenderRequestID {
    let mut commands = Vec::new();
    runtime.drain_render_commands(&mut commands);
    commands
        .into_iter()
        .find_map(|command| match command {
            RenderCommand::Resource(ResourceCommand::CreateTexture { request, id, .. })
                if id == texture =>
            {
                Some(request)
            }
            _ => None,
        })
        .expect("expected texture create command")
}

fn water_3d_command(
    commands: &[RenderCommand],
    node_id: perro_ids::NodeID,
) -> &perro_render_bridge::Water3DState {
    commands
        .iter()
        .find_map(|command| match command {
            RenderCommand::ThreeD(command) => match command.as_ref() {
                Command3D::UpsertWater { node, water } if *node == node_id => Some(water.as_ref()),
                _ => None,
            },
            _ => None,
        })
        .expect("water command should exist")
}

#[test]
fn linked_3d_water_mirrors_wake_across_overlap() {
    let mut runtime = Runtime::new();
    let water_a = NodeAPI::create::<WaterBody3D>(&mut runtime);
    let water_b = NodeAPI::create::<WaterBody3D>(&mut runtime);
    for (id, x) in [(water_a, 0.0), (water_b, 12.0)] {
        if let Some(mut node) = runtime.nodes.get_mut(id)
            && let SceneNodeData::WaterBody3D(water) = &mut node.data
        {
            water.transform.position.x = x;
            water.water.shape = perro_nodes::WaterShape::box_volume(Vector3::new(16.0, 4.0, 16.0));
            water.water.depth = 4.0;
        }
    }
    runtime
        .force_water_impacts_3d
        .push(crate::runtime::ForceWaterImpact3D {
            position: Vector3::new(8.4, 0.0, 0.0),
            force: Vector3::new(12.0, 0.0, 0.0),
            strength: 10.0,
            radius: 0.25,
            cavitation: 0.5,
        });

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);
    let water = water_3d_command(&commands, water_a);

    assert_eq!(water.links.len(), 1);
    assert_eq!(water.impacts.len(), 1);
    assert!(water.impacts[0].strength > 0.0);
    assert!(water.impacts[0].strength < 10.0);
}

#[test]
fn sprite_3d_and_label_3d_emit_projected_ui_commands() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);
    let camera = NodeAPI::create::<Camera3D>(&mut runtime);
    let sprite = NodeAPI::create::<Sprite3D>(&mut runtime);
    let label = NodeAPI::create::<Label3D>(&mut runtime);
    if let Some(mut node) = runtime.nodes.get_mut(camera)
        && let SceneNodeData::Camera3D(data) = &mut node.data
    {
        data.active = true;
    }
    if let Some(mut node) = runtime.nodes.get_mut(sprite)
        && let SceneNodeData::Sprite3D(data) = &mut node.data
    {
        data.texture = TextureID::from_parts(12, 0);
        data.transform.position = Vector3::new(0.0, 0.0, -5.0);
    }
    if let Some(mut node) = runtime.nodes.get_mut(label)
        && let SceneNodeData::Label3D(data) = &mut node.data
    {
        data.text = "Name".into();
        data.transform.position = Vector3::new(0.0, 1.0, -5.0);
    }

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);

    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::Ui(UiCommand::UpsertImage { node, texture, .. })
            if *node == sprite && *texture == TextureID::from_parts(12, 0)
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::Ui(UiCommand::UpsertLabel { node, text, wrap_width, .. })
            if *node == label && text.as_ref() == "Name" && *wrap_width == Some(80.0)
    )));
}

#[test]
fn text_decal_3d_rasterizes_text_and_emits_decal_state() {
    let mut runtime = Runtime::new();
    let text_decal = NodeAPI::create::<TextDecal3D>(&mut runtime);
    if let Some(mut node) = runtime.nodes.get_mut(text_decal)
        && let SceneNodeData::TextDecal3D(data) = &mut node.data
    {
        data.text = "Door".into();
        data.transform.position = Vector3::new(0.0, 0.0, -2.0);
        data.surface.emission_energy = 0.5;
    }

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);
    let texture = commands
        .iter()
        .find_map(|command| match command {
            RenderCommand::Resource(ResourceCommand::CreateRuntimeTexture {
                id,
                width,
                height,
                ..
            }) => {
                assert!(*width > 0);
                assert!(*height > 0);
                Some(*id)
            }
            _ => None,
        })
        .expect("expected text decal runtime texture");

    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command)
            if matches!(
                command.as_ref(),
                Command3D::SetDecal { node, decal }
                    if *node == text_decal
                        && decal.albedo_texture == texture
                        && decal.emission_texture == texture
                        && decal.modulate == Color::WHITE
            )
    )));
}

#[test]
fn sprite_3d_emits_after_async_texture_create_without_other_dirty_work() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);
    let camera = NodeAPI::create::<Camera3D>(&mut runtime);
    let sprite = NodeAPI::create::<Sprite3D>(&mut runtime);
    if let Some(mut node) = runtime.nodes.get_mut(camera)
        && let SceneNodeData::Camera3D(data) = &mut node.data
    {
        data.active = true;
    }

    let texture = runtime
        .resource_api
        .load_texture("res://textures/floating_prompt.png");
    let request = collect_resource_texture_request(&mut runtime, texture);
    if let Some(mut node) = runtime.nodes.get_mut(sprite)
        && let SceneNodeData::Sprite3D(data) = &mut node.data
    {
        data.texture = texture;
        data.transform.position = Vector3::new(0.0, 0.0, -5.0);
    }

    runtime.extract_render_3d_commands();
    assert!(
        !collect_commands(&mut runtime)
            .iter()
            .any(|command| matches!(
                command,
                RenderCommand::Ui(UiCommand::UpsertImage { node, .. }) if *node == sprite
            ))
    );

    runtime.apply_render_event(RenderEvent::TextureCreated {
        request,
        id: texture,
    });
    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);

    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::Ui(UiCommand::UpsertImage { node, texture: id, .. })
            if *node == sprite && *id == texture
    )));
}

#[test]
fn sprite_3d_and_label_3d_hide_when_mesh_blocks_center() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);
    let camera = NodeAPI::create::<Camera3D>(&mut runtime);
    if let Some(mut node) = runtime.nodes.get_mut(camera)
        && let SceneNodeData::Camera3D(data) = &mut node.data
    {
        data.active = true;
    }

    let mut blocker = MeshInstance3D::new();
    blocker.mesh = MeshID::from_parts(31, 0);
    blocker.transform.position = Vector3::new(0.0, 0.0, -2.5);
    let blocker = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(blocker)));
    runtime
        .render_3d
        .mesh_sources
        .insert(blocker, "__cube__".to_string());

    let sprite = NodeAPI::create::<Sprite3D>(&mut runtime);
    let label = NodeAPI::create::<Label3D>(&mut runtime);
    if let Some(mut node) = runtime.nodes.get_mut(sprite)
        && let SceneNodeData::Sprite3D(data) = &mut node.data
    {
        data.texture = TextureID::from_parts(12, 0);
        data.transform.position = Vector3::new(0.0, 0.0, -5.0);
    }
    if let Some(mut node) = runtime.nodes.get_mut(label)
        && let SceneNodeData::Label3D(data) = &mut node.data
    {
        data.text = "Hidden".into();
        data.transform.position = Vector3::new(0.0, 0.0, -5.0);
    }

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);

    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::Ui(UiCommand::RemoveNode { node }) if *node == sprite
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::Ui(UiCommand::RemoveNode { node }) if *node == label
    )));
    assert!(!commands.iter().any(|command| matches!(
        command,
        RenderCommand::Ui(UiCommand::UpsertImage { node, .. }) if *node == sprite
    )));
    assert!(!commands.iter().any(|command| matches!(
        command,
        RenderCommand::Ui(UiCommand::UpsertLabel { node, .. }) if *node == label
    )));
}

#[test]
fn sprite_3d_hides_behind_mesh_with_orthographic_camera() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);
    let camera = NodeAPI::create::<Camera3D>(&mut runtime);
    if let Some(mut node) = runtime.nodes.get_mut(camera)
        && let SceneNodeData::Camera3D(data) = &mut node.data
    {
        data.active = true;
        data.projection = CameraProjection::Orthographic {
            size: 10.0,
            near: 0.1,
            far: 100.0,
        };
    }

    let mut blocker = MeshInstance3D::new();
    blocker.mesh = MeshID::from_parts(31, 0);
    blocker.transform.position = Vector3::new(2.0, 0.0, -2.5);
    let blocker = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(blocker)));
    runtime
        .render_3d
        .mesh_sources
        .insert(blocker, "__cube__".to_string());

    let sprite = NodeAPI::create::<Sprite3D>(&mut runtime);
    if let Some(mut node) = runtime.nodes.get_mut(sprite)
        && let SceneNodeData::Sprite3D(data) = &mut node.data
    {
        data.texture = TextureID::from_parts(12, 0);
        data.transform.position = Vector3::new(2.0, 0.0, -5.0);
    }

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);

    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::Ui(UiCommand::RemoveNode { node }) if *node == sprite
    )));
    assert!(!commands.iter().any(|command| matches!(
        command,
        RenderCommand::Ui(UiCommand::UpsertImage { node, .. }) if *node == sprite
    )));
}

#[test]
fn linked_3d_waters_both_collect_shared_coastline_shape() {
    let mut runtime = Runtime::new();
    let water_a = NodeAPI::create::<WaterBody3D>(&mut runtime);
    let water_b = NodeAPI::create::<WaterBody3D>(&mut runtime);
    for (id, x) in [(water_a, 0.0), (water_b, 12.0)] {
        if let Some(mut node) = runtime.nodes.get_mut(id)
            && let SceneNodeData::WaterBody3D(water) = &mut node.data
        {
            water.transform.position.x = x;
            water.water.shape = perro_nodes::WaterShape::box_volume(Vector3::new(16.0, 4.0, 16.0));
            water.water.depth = 4.0;
        }
    }
    let body = NodeAPI::create::<StaticBody3D>(&mut runtime);
    let shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, body, shape));
    if let Some(mut node) = runtime.nodes.get_mut(shape)
        && let SceneNodeData::CollisionShape3D(shape) = &mut node.data
    {
        shape.transform.position = Vector3::new(6.0, -1.0, 0.0);
        shape.shape = Shape3D::Cube {
            size: Vector3::new(2.0, 2.0, 4.0),
        };
    }

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);

    assert_eq!(
        water_3d_command(&commands, water_a).coastline_shapes.len(),
        1
    );
    assert_eq!(
        water_3d_command(&commands, water_b).coastline_shapes.len(),
        1
    );
}

#[test]
fn water_3d_impacts_use_live_body_pos_not_stale_cached_sample() {
    let mut runtime = Runtime::new();
    let water = NodeAPI::create::<WaterBody3D>(&mut runtime);
    let body = NodeAPI::create::<RigidBody3D>(&mut runtime);
    if let Some(mut node) = runtime.nodes.get_mut(body)
        && let SceneNodeData::RigidBody3D(rigid) = &mut node.data
    {
        rigid.transform.position = Vector3::new(1.5, -0.4, -0.75);
        rigid.linear_velocity = Vector3::new(0.0, -2.8, 0.0);
        rigid.mass = 4.0;
        rigid.density = 1.0;
    }
    runtime.time.elapsed = 1.0;
    runtime.apply_render_event(RenderEvent::WaterBodySamples {
        samples: Arc::from([perro_render_bridge::WaterBodySampleState {
            water,
            body,
            point: 0,
            local: [6.0, 4.0],
            height: 2.0,
            velocity: [0.0, 0.0],
            foam: 1.0,
        }]),
    });

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);
    let water = water_3d_command(&commands, water);

    assert_eq!(water.impacts.len(), 1);
    assert!((water.impacts[0].position[0] - 1.5).abs() < 0.01);
    assert!((water.impacts[0].position[1] + 0.4).abs() < 0.01);
    assert!((water.impacts[0].position[2] + 0.75).abs() < 0.01);
}

fn set_primary_material(mesh: &mut MeshInstance3D, material: MaterialID) {
    if mesh.surfaces.is_empty() {
        mesh.surfaces.push(MeshSurfaceBinding::default());
    }
    mesh.surfaces[0].material = Some(material);
}

fn set_primary_material_multi(mesh: &mut MultiMeshInstance3D, material: MaterialID) {
    if mesh.surfaces.is_empty() {
        mesh.surfaces.push(MeshSurfaceBinding::default());
    }
    mesh.surfaces[0].material = Some(material);
}

fn node3d_position_clip(xs: &[(u32, f32)]) -> AnimationClip {
    AnimationClip {
        name: Cow::Borrowed("tool"),
        fps: 1.0,
        total_frames: xs.last().map(|(frame, _)| frame + 1).unwrap_or(0),
        objects: Cow::Owned(vec![AnimationObject {
            name: Cow::Borrowed("Tool"),
            node_type: NodeType::Node3D,
        }]),
        object_tracks: Cow::Owned(vec![AnimationObjectTrack {
            object: Cow::Borrowed("Tool"),
            field: NodeField::Node3D(Node3DField::Position),
            bone_target: None,
            transform2d_mask: 0,
            transform3d_mask: perro_animation::ANIMATION_TRANSFORM_MASK_POSITION,
            interpolation: perro_animation::AnimationInterpolation::Step,
            ease: perro_animation::AnimationEase::Linear,
            keys: Cow::Owned(
                xs.iter()
                    .map(|(frame, x)| AnimationObjectKey {
                        frame: *frame,
                        mode: perro_animation::AnimationKeyMode::Closed,
                        interpolation: perro_animation::AnimationInterpolation::Step,
                        ease: perro_animation::AnimationEase::Linear,
                        value: AnimationTrackValue::Transform3D(Transform3D::new(
                            Vector3::new(*x, 0.0, 0.0),
                            Quaternion::IDENTITY,
                            Vector3::ONE,
                        )),
                    })
                    .collect(),
            ),
        }]),
        frame_events: Cow::Borrowed(&[]),
    }
}

#[test]
fn mesh_blend_options_reach_draw_command() {
    let mut runtime = Runtime::new();
    let mut mesh = MeshInstance3D::new();
    mesh.mesh = MeshID::from_parts(7, 0);
    set_primary_material(&mut mesh, MaterialID::from_parts(9, 0));
    mesh.blend.enabled = true;
    mesh.blend.screen_blending = false;
    mesh.blend.normal_blending = true;
    mesh.blend.blend_layers = BitMask::with([3]);
    mesh.blend.blend_mask = BitMask::with([2, 4]);
    mesh.blend.distance = 0.75;
    mesh.blend.min_distance = 0.125;
    mesh.blend.noise_factor = 0.5;
    mesh.blend.noise_scale = 12.0;
    let node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);
    let blend = commands
        .iter()
        .find_map(|command| match command {
            RenderCommand::ThreeD(command) => match command.as_ref() {
                Command3D::Draw {
                    node: got, blend, ..
                } if *got == node => Some(*blend),
                _ => None,
            },
            _ => None,
        })
        .expect("mesh draw command");

    assert!(blend.enabled);
    assert!(!blend.screen_blending);
    assert!(blend.normal_blending);
    assert_eq!(blend.blend_layers, BitMask::with([3]));
    assert_eq!(blend.blend_mask, BitMask::with([2, 4]));
    assert_eq!(blend.distance, 0.75);
    assert_eq!(blend.min_distance, 0.125);
    assert_eq!(blend.noise_factor, 0.5);
    assert_eq!(blend.noise_scale, 12.0);
}

#[test]
fn multimesh_blend_options_reach_dense_draw_command() {
    let mut runtime = Runtime::new();
    let mut multi = MultiMeshInstance3D::new();
    multi.mesh = MeshID::from_parts(8, 0);
    set_primary_material_multi(&mut multi, MaterialID::from_parts(10, 0));
    multi
        .instances
        .push(perro_nodes::MultiMeshInstancePose::from_pos_rot(
            Vector3::ZERO,
            Quaternion::IDENTITY,
        ));
    multi.blend.enabled = true;
    multi.blend.screen_blending = false;
    multi.blend.normal_blending = true;
    multi.blend.blend_layers = BitMask::with([5]);
    multi.blend.blend_mask = BitMask::with([1, 5]);
    multi.blend.distance = 0.25;
    let node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MultiMeshInstance3D(multi)));

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);
    let blend = commands
        .iter()
        .find_map(|command| match command {
            RenderCommand::ThreeD(command) => match command.as_ref() {
                Command3D::DrawMultiDense {
                    node: got, blend, ..
                } if *got == node => Some(*blend),
                _ => None,
            },
            _ => None,
        })
        .expect("multimesh draw command");

    assert!(blend.enabled);
    assert!(!blend.screen_blending);
    assert!(blend.normal_blending);
    assert_eq!(blend.blend_layers, BitMask::with([5]));
    assert_eq!(blend.blend_mask, BitMask::with([1, 5]));
    assert_eq!(blend.distance, 0.25);
}

#[test]
fn mesh_instance_flip_x_mirrors_model_about_local_origin() {
    let mut runtime = Runtime::new();
    let mut mesh = MeshInstance3D::new();
    mesh.mesh = MeshID::from_parts(11, 0);
    set_primary_material(&mut mesh, MaterialID::from_parts(13, 0));
    mesh.flip_x = true;
    mesh.transform.position = Vector3::new(3.0, 4.0, 5.0);
    let node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);
    let model = commands
        .iter()
        .find_map(|command| match command {
            RenderCommand::ThreeD(command) => match command.as_ref() {
                Command3D::Draw {
                    node: got, model, ..
                } if *got == node => Some(*model),
                _ => None,
            },
            _ => None,
        })
        .expect("mesh draw command");

    assert_eq!(model[0], [-1.0, 0.0, 0.0, 0.0]);
    assert_eq!(model[1], [0.0, 1.0, 0.0, 0.0]);
    assert_eq!(model[2], [0.0, 0.0, 1.0, 0.0]);
    assert_eq!(model[3], [3.0, 4.0, 5.0, 1.0]);
}

#[test]
fn multimesh_flip_xy_mirrors_node_model_about_local_origin() {
    let mut runtime = Runtime::new();
    let mut multi = MultiMeshInstance3D::new();
    multi.mesh = MeshID::from_parts(12, 0);
    set_primary_material_multi(&mut multi, MaterialID::from_parts(14, 0));
    multi
        .instances
        .push(perro_nodes::MultiMeshInstancePose::from_pos_rot(
            Vector3::ZERO,
            Quaternion::IDENTITY,
        ));
    multi.flip_x = true;
    multi.flip_y = true;
    multi.transform.position = Vector3::new(1.0, 2.0, 3.0);
    let node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MultiMeshInstance3D(multi)));

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);
    let model = commands
        .iter()
        .find_map(|command| match command {
            RenderCommand::ThreeD(command) => match command.as_ref() {
                Command3D::DrawMultiDense {
                    node: got,
                    node_model,
                    ..
                } if *got == node => Some(*node_model),
                _ => None,
            },
            _ => None,
        })
        .expect("multimesh draw command");

    assert_eq!(model[0], [-1.0, 0.0, 0.0, 0.0]);
    assert_eq!(model[1], [0.0, -1.0, 0.0, 0.0]);
    assert_eq!(model[2], [0.0, 0.0, 1.0, 0.0]);
    assert_eq!(model[3], [1.0, 2.0, 3.0, 1.0]);
}

#[test]
fn mesh_instance_without_mesh_source_requests_nothing() {
    let mut runtime = Runtime::new();
    let mut mesh = MeshInstance3D::new();
    mesh.mesh = MeshID::nil();
    set_primary_material(&mut mesh, MaterialID::nil());
    runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

    runtime.extract_render_3d_commands();
    let first = collect_commands(&mut runtime);
    assert!(first.is_empty());
}

#[test]
fn mesh_instance_requests_missing_assets_once_until_events_arrive() {
    let mut runtime = Runtime::new();
    let mut mesh = MeshInstance3D::new();
    mesh.mesh = MeshID::nil();
    set_primary_material(&mut mesh, MaterialID::nil());
    let expected_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));
    runtime
        .render_3d
        .mesh_sources
        .insert(expected_node, "__cube__".to_string());

    runtime.extract_render_3d_commands();
    let first = collect_commands(&mut runtime);
    assert_eq!(first.len(), 1);
    assert!(matches!(
        &first[0],
        RenderCommand::Resource(ResourceCommand::CreateMesh { source, .. })
            if source == "__cube__"
    ));

    runtime.extract_render_3d_commands();
    let second = collect_commands(&mut runtime);
    assert!(second.is_empty());
}

#[test]
fn mesh_instance_emits_draw_after_mesh_created_and_inline_material_allocated() {
    let mut runtime = Runtime::new();
    let expected_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(
            MeshInstance3D::new(),
        )));
    runtime
        .render_3d
        .mesh_sources
        .insert(expected_node, "__cube__".to_string());

    runtime.extract_render_3d_commands();
    let first = collect_commands(&mut runtime);
    let mesh_request = match &first[0] {
        RenderCommand::Resource(ResourceCommand::CreateMesh { request, .. }) => *request,
        _ => panic!("expected mesh create request"),
    };

    let expected_mesh = MeshID::from_parts(9, 1);
    runtime.apply_render_event(RenderEvent::MeshCreated {
        request: mesh_request,
        id: expected_mesh,
        mesh: None,
    });
    runtime.extract_render_3d_commands();
    let second = collect_commands(&mut runtime);
    let expected_material = second
        .iter()
        .find_map(|command| match command {
            RenderCommand::Resource(ResourceCommand::CreateMaterial { id, .. }) => Some(*id),
            _ => None,
        })
        .expect("expected material create command");
    assert!(!expected_material.is_nil());
    let drew_expected = second.iter().any(|command| match command {
        RenderCommand::ThreeD(command) => matches!(
            command.as_ref(),
            Command3D::Draw {
                node,
                mesh,
                surfaces,
                ..
            } if *node == expected_node
                && *mesh == expected_mesh
                && surfaces
                    .first()
                    .and_then(|surface| surface.material)
                    .is_some_and(|id| id == expected_material)
        ),
        _ => false,
    });
    assert!(drew_expected);
}

#[test]
fn node_3d_effective_modulate_inherits_to_child() {
    let mut runtime = Runtime::new();
    let parent = NodeAPI::create::<Node3D>(&mut runtime);
    let child = NodeAPI::create::<MeshInstance3D>(&mut runtime);

    if let Some(mut node) = runtime.nodes.get_mut(parent)
        && let SceneNodeData::Node3D(data) = &mut node.data
    {
        data.modulate.children_modulate = Color::new(0.5, 1.0, 1.0, 1.0);
        data.modulate.self_modulate = Color::RED;
        node.add_child(child);
    }
    if let Some(mut node) = runtime.nodes.get_mut(child)
        && let SceneNodeData::MeshInstance3D(data) = &mut node.data
    {
        data.modulate.self_modulate = Color::new(1.0, 0.25, 1.0, 1.0);
        node.parent = parent;
    }

    let expected = Runtime::color_modulate(
        Color::new(0.5, 1.0, 1.0, 1.0),
        Color::new(1.0, 0.25, 1.0, 1.0),
    );
    assert_eq!(runtime.effective_self_modulate(child), expected);
    assert_eq!(runtime.effective_self_modulate(parent), Color::RED);
}

#[test]
fn effective_modulate_combines_deep_chain_roles() {
    let mut runtime = Runtime::new();
    let root = NodeAPI::create::<Node3D>(&mut runtime);
    let mid = NodeAPI::create::<Node3D>(&mut runtime);
    let leaf = NodeAPI::create::<MeshInstance3D>(&mut runtime);
    let sibling = NodeAPI::create::<MeshInstance3D>(&mut runtime);

    if let Some(mut node) = runtime.nodes.get_mut(root)
        && let SceneNodeData::Node3D(data) = &mut node.data
    {
        data.modulate.modulate = Color::new(0.8, 1.0, 1.0, 1.0);
        data.modulate.self_modulate = Color::new(1.0, 0.1, 0.1, 1.0);
        data.modulate.children_modulate = Color::new(1.0, 0.7, 1.0, 1.0);
        node.add_child(mid);
    }
    if let Some(mut node) = runtime.nodes.get_mut(mid)
        && let SceneNodeData::Node3D(data) = &mut node.data
    {
        data.modulate.modulate = Color::new(1.0, 0.9, 1.0, 1.0);
        data.modulate.self_modulate = Color::new(0.1, 1.0, 0.1, 1.0);
        data.modulate.children_modulate = Color::new(1.0, 1.0, 0.6, 1.0);
        node.parent = root;
        node.add_child(leaf);
        node.add_child(sibling);
    }
    if let Some(mut node) = runtime.nodes.get_mut(leaf)
        && let SceneNodeData::MeshInstance3D(data) = &mut node.data
    {
        data.modulate.modulate = Color::new(1.0, 1.0, 0.5, 1.0);
        data.modulate.self_modulate = Color::new(0.5, 1.0, 1.0, 1.0);
        data.modulate.children_modulate = Color::RED;
        node.parent = mid;
    }
    if let Some(mut node) = runtime.nodes.get_mut(sibling)
        && let SceneNodeData::MeshInstance3D(data) = &mut node.data
    {
        data.modulate.self_modulate = Color::new(1.0, 0.5, 1.0, 1.0);
        node.parent = mid;
    }

    assert_eq!(
        runtime.effective_self_modulate(root),
        Runtime::color_modulate(
            Color::new(0.8, 1.0, 1.0, 1.0),
            Color::new(1.0, 0.1, 0.1, 1.0)
        )
    );
    let inherited_to_mid = Runtime::color_modulate(
        Runtime::color_modulate(
            Color::new(0.8, 1.0, 1.0, 1.0),
            Color::new(1.0, 0.7, 1.0, 1.0),
        ),
        Color::new(1.0, 0.9, 1.0, 1.0),
    );
    assert_eq!(
        runtime.effective_self_modulate(mid),
        Runtime::color_modulate(inherited_to_mid, Color::new(0.1, 1.0, 0.1, 1.0))
    );
    let inherited_to_leaf =
        Runtime::color_modulate(inherited_to_mid, Color::new(1.0, 1.0, 0.6, 1.0));
    assert_eq!(
        runtime.effective_self_modulate(leaf),
        Runtime::color_modulate(
            Runtime::color_modulate(inherited_to_leaf, Color::new(1.0, 1.0, 0.5, 1.0)),
            Color::new(0.5, 1.0, 1.0, 1.0)
        )
    );
    assert_eq!(
        runtime.effective_self_modulate(sibling),
        Runtime::color_modulate(inherited_to_leaf, Color::new(1.0, 0.5, 1.0, 1.0))
    );
}

#[test]
fn mesh_instance_redraws_when_script_assigned_pending_mesh_load_finishes() {
    let mut runtime = Runtime::new();
    let pending_mesh = MeshAPI::load_mesh(
        runtime.resource_api.as_ref(),
        "res://avatars/face/noses.glb:mesh[3]",
    );
    let mesh_request = collect_commands(&mut runtime)
        .into_iter()
        .find_map(|command| match command {
            RenderCommand::Resource(ResourceCommand::CreateMesh { request, id, .. })
                if id == pending_mesh =>
            {
                Some(request)
            }
            _ => None,
        })
        .expect("expected mesh create command");

    let node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(
            MeshInstance3D::new(),
        )));
    NodeAPI::with_node_mut::<MeshInstance3D, _, _>(&mut runtime, node, |mesh| {
        mesh.mesh = pending_mesh;
    });

    runtime.extract_render_3d_commands();
    assert!(collect_commands(&mut runtime).is_empty());

    runtime.apply_render_event(RenderEvent::MeshCreated {
        request: mesh_request,
        id: pending_mesh,
        mesh: Some(Mesh3D {
            vertices: Vec::new(),
            indices: Vec::new(),
            surface_ranges: Vec::new(),
            blend_shapes: Vec::new(),
        }),
    });
    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);

    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command)
            if matches!(
                command.as_ref(),
                Command3D::Draw { node: draw_node, mesh, .. }
                    if *draw_node == node && *mesh == pending_mesh
            )
    )));
}

#[test]
fn mesh_instance_ready_waits_for_mesh_and_material_backend_ack() {
    let mut runtime = Runtime::new();
    let node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(
            MeshInstance3D::new(),
        )));
    runtime
        .render_3d
        .mesh_sources
        .insert(node, "__cube__".to_string());

    runtime.extract_render_3d_commands();
    let first = collect_commands(&mut runtime);
    let mesh_request = match first.first() {
        Some(RenderCommand::Resource(ResourceCommand::CreateMesh { request, .. })) => *request,
        _ => panic!("expected mesh create request"),
    };
    assert!(!NodeAPI::is_mesh_instance_ready(&mut runtime, node));

    let mesh = MeshID::from_parts(99, 0);
    runtime.apply_render_event(RenderEvent::MeshCreated {
        request: mesh_request,
        id: mesh,
        mesh: Some(Mesh3D {
            vertices: Vec::new(),
            indices: Vec::new(),
            surface_ranges: Vec::new(),
            blend_shapes: Vec::new(),
        }),
    });
    runtime.extract_render_3d_commands();
    let second = collect_commands(&mut runtime);
    let (material_request, material) = second
        .iter()
        .find_map(|command| match command {
            RenderCommand::Resource(ResourceCommand::CreateMaterial { request, id, .. }) => {
                Some((*request, *id))
            }
            _ => None,
        })
        .expect("expected material create command");
    assert!(!NodeAPI::is_mesh_instance_ready(&mut runtime, node));

    runtime.apply_render_event(RenderEvent::MaterialCreated {
        request: material_request,
        id: material,
    });
    assert!(NodeAPI::is_mesh_instance_ready(&mut runtime, node));
}

#[test]
fn mesh_instance_ready_ignores_default_nil_mesh_and_material() {
    let mut runtime = Runtime::new();
    let node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(
            MeshInstance3D::new(),
        )));

    assert!(NodeAPI::is_mesh_instance_ready(&mut runtime, node));
}

#[test]
fn mesh_instance_ready_ignores_nil_surface_material() {
    let mut runtime = Runtime::new();
    let mesh_id = MeshAPI::create_mesh_data(
        runtime.resource_api.as_ref(),
        Mesh3D {
            vertices: Vec::new(),
            indices: Vec::new(),
            surface_ranges: Vec::new(),
            blend_shapes: Vec::new(),
        },
    );
    let request = collect_commands(&mut runtime)
        .into_iter()
        .find_map(|command| match command {
            RenderCommand::Resource(ResourceCommand::CreateRuntimeMesh { request, .. }) => {
                Some(request)
            }
            _ => None,
        })
        .expect("expected runtime mesh create command");
    runtime.apply_render_event(RenderEvent::MeshCreated {
        request,
        id: mesh_id,
        mesh: None,
    });
    let mut mesh = MeshInstance3D::new();
    mesh.mesh = mesh_id;
    set_primary_material(&mut mesh, MaterialID::nil());
    let node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

    assert!(NodeAPI::is_mesh_instance_ready(&mut runtime, node));
}

#[test]
fn mesh_instance_can_request_mesh_and_material_in_separate_frames() {
    let mut runtime = Runtime::new();
    let inserted = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(
            MeshInstance3D::new(),
        )));
    runtime
        .render_3d
        .mesh_sources
        .insert(inserted, "__cube__".to_string());

    runtime.extract_render_3d_commands();
    let first = collect_commands(&mut runtime);
    let mesh_request = match first.first() {
        Some(RenderCommand::Resource(ResourceCommand::CreateMesh { request, .. })) => *request,
        _ => panic!("expected mesh create request"),
    };

    runtime.apply_render_event(RenderEvent::MeshCreated {
        request: mesh_request,
        id: MeshID::from_parts(10, 0),
        mesh: None,
    });
    runtime.extract_render_3d_commands();
    let second = collect_commands(&mut runtime);
    assert!(second.iter().any(|command| matches!(
        command,
        RenderCommand::Resource(ResourceCommand::CreateMaterial { .. })
    )));
    assert!(second.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command)
            if matches!(command.as_ref(), Command3D::Draw { node, .. } if *node == inserted)
    )));
}

#[test]
fn mesh_instances_share_default_material() {
    let mut runtime = Runtime::new();
    let first_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(
            MeshInstance3D::new(),
        )));
    let second_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(
            MeshInstance3D::new(),
        )));
    runtime
        .render_3d
        .mesh_sources
        .insert(first_node, "__cube__".to_string());
    runtime
        .render_3d
        .mesh_sources
        .insert(second_node, "__cube__".to_string());

    runtime.extract_render_3d_commands();
    let first = collect_commands(&mut runtime);
    for (request, mesh) in first
        .iter()
        .filter_map(|command| match command {
            RenderCommand::Resource(ResourceCommand::CreateMesh { request, .. }) => Some(*request),
            _ => None,
        })
        .zip([MeshID::from_parts(20, 0), MeshID::from_parts(21, 0)])
    {
        runtime.apply_render_event(RenderEvent::MeshCreated {
            request,
            id: mesh,
            mesh: None,
        });
    }

    runtime.extract_render_3d_commands();
    let second = collect_commands(&mut runtime);
    let default_materials: Vec<MaterialID> = second
        .iter()
        .filter_map(|command| match command {
            RenderCommand::Resource(ResourceCommand::CreateMaterial { id, source, .. })
                if source.is_none() =>
            {
                Some(*id)
            }
            _ => None,
        })
        .collect();
    assert_eq!(default_materials.len(), 1);
    let default_material = default_materials[0];
    let draws_using_default = second
        .iter()
        .filter(|command| {
            matches!(
                command,
                RenderCommand::ThreeD(command)
                    if matches!(
                        command.as_ref(),
                        Command3D::Draw { surfaces, .. }
                            if surfaces
                                .first()
                                .and_then(|surface| surface.material)
                                == Some(default_material)
                    )
            )
        })
        .count();
    assert_eq!(draws_using_default, 2);
}

#[test]
fn mesh_instances_share_identical_inline_material() {
    let mut runtime = Runtime::new();
    let first_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(
            MeshInstance3D::new(),
        )));
    let second_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(
            MeshInstance3D::new(),
        )));
    runtime
        .render_3d
        .mesh_sources
        .insert(first_node, "__cube__".to_string());
    runtime
        .render_3d
        .mesh_sources
        .insert(second_node, "__cube__".to_string());
    let standard = StandardMaterial3D {
        base_color_factor: [0.2, 0.4, 0.8, 1.0],
        ..Default::default()
    };
    let material = Material3D::Standard(standard);
    runtime
        .render_3d
        .material_surface_overrides
        .insert(first_node, vec![Some(material.clone())]);
    runtime
        .render_3d
        .material_surface_overrides
        .insert(second_node, vec![Some(material)]);

    runtime.extract_render_3d_commands();
    let first = collect_commands(&mut runtime);
    for (request, mesh) in first
        .iter()
        .filter_map(|command| match command {
            RenderCommand::Resource(ResourceCommand::CreateMesh { request, .. }) => Some(*request),
            _ => None,
        })
        .zip([MeshID::from_parts(22, 0), MeshID::from_parts(23, 0)])
    {
        runtime.apply_render_event(RenderEvent::MeshCreated {
            request,
            id: mesh,
            mesh: None,
        });
    }

    runtime.extract_render_3d_commands();
    let second = collect_commands(&mut runtime);
    let inline_materials: Vec<MaterialID> = second
        .iter()
        .filter_map(|command| match command {
            RenderCommand::Resource(ResourceCommand::CreateMaterial { id, source, .. })
                if source.is_none() =>
            {
                Some(*id)
            }
            _ => None,
        })
        .collect();
    assert_eq!(inline_materials.len(), 1);
}

#[test]
fn mesh_instance_keeps_retained_mesh_while_replacement_mesh_is_pending() {
    let mut runtime = Runtime::new();
    let old_mesh = MeshID::from_parts(41, 0);
    let old_material = MaterialID::from_parts(42, 0);
    let mut mesh = MeshInstance3D::new();
    mesh.mesh = old_mesh;
    set_primary_material(&mut mesh, old_material);
    let node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

    runtime.extract_render_3d_commands();
    let first = collect_commands(&mut runtime);
    assert!(first.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command)
            if matches!(
                command.as_ref(),
                Command3D::Draw { node: draw_node, mesh, .. }
                    if *draw_node == node && *mesh == old_mesh
            )
    )));

    let pending_mesh = runtime
        .resource_api
        .load_mesh("res://meshes/tool_version_b.glb:mesh[0]");
    let pending_request = collect_commands(&mut runtime)
        .into_iter()
        .find_map(|command| match command {
            RenderCommand::Resource(ResourceCommand::CreateMesh { request, id, .. })
                if id == pending_mesh =>
            {
                Some(request)
            }
            _ => None,
        })
        .expect("expected pending mesh create request");
    if let Some(mut scene_node) = runtime.nodes.get_mut(node)
        && let SceneNodeData::MeshInstance3D(mesh) = &mut scene_node.data
    {
        mesh.mesh = pending_mesh;
    }
    runtime.mark_needs_rerender(node);

    runtime.extract_render_3d_commands();
    let pending = collect_commands(&mut runtime);
    assert!(!pending.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command)
            if matches!(command.as_ref(), Command3D::RemoveNode { node: removed } if *removed == node)
    )));
    assert!(!pending.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command)
            if matches!(command.as_ref(), Command3D::Draw { node: draw_node, .. } if *draw_node == node)
    )));
    assert_eq!(
        runtime
            .render_3d
            .retained_mesh_draws
            .get(&node)
            .map(|draw| draw.mesh),
        Some(old_mesh)
    );

    runtime.apply_render_event(RenderEvent::MeshCreated {
        request: pending_request,
        id: pending_mesh,
        mesh: None,
    });
    runtime.mark_needs_rerender(node);
    runtime.extract_render_3d_commands();
    let ready = collect_commands(&mut runtime);
    assert!(ready.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command)
            if matches!(
                command.as_ref(),
                Command3D::Draw { node: draw_node, mesh, .. }
                    if *draw_node == node && *mesh == pending_mesh
            )
    )));
}

#[test]
fn mesh_instance_keeps_retained_material_while_replacement_material_is_pending() {
    let mut runtime = Runtime::new();
    let mesh_id = MeshID::from_parts(51, 0);
    let old_material = MaterialID::from_parts(52, 0);
    let mut mesh = MeshInstance3D::new();
    mesh.mesh = mesh_id;
    set_primary_material(&mut mesh, old_material);
    let node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

    runtime.extract_render_3d_commands();
    let first = collect_commands(&mut runtime);
    assert!(first.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command)
            if matches!(
                command.as_ref(),
                Command3D::Draw { node: draw_node, surfaces, .. }
                    if *draw_node == node
                        && surfaces
                            .first()
                            .and_then(|surface| surface.material)
                            .is_some_and(|material| material == old_material)
            )
    )));

    let pending_material = runtime
        .resource_api
        .load_material_source("res://materials/tool_version_b.pmat");
    let pending_request =
        collect_commands(&mut runtime)
            .into_iter()
            .find_map(|command| match command {
                RenderCommand::Resource(ResourceCommand::CreateMaterial {
                    request, id, ..
                }) if id == pending_material => Some(request),
                _ => None,
            })
            .expect("expected pending material create request");
    if let Some(mut scene_node) = runtime.nodes.get_mut(node)
        && let SceneNodeData::MeshInstance3D(mesh) = &mut scene_node.data
    {
        set_primary_material(mesh, pending_material);
    }
    runtime.mark_needs_rerender(node);

    runtime.extract_render_3d_commands();
    let pending = collect_commands(&mut runtime);
    assert!(!pending.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command)
            if matches!(command.as_ref(), Command3D::RemoveNode { node: removed } if *removed == node)
    )));
    assert!(!pending.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command)
            if matches!(command.as_ref(), Command3D::Draw { node: draw_node, .. } if *draw_node == node)
    )));
    assert_eq!(
        runtime
            .render_3d
            .retained_mesh_draws
            .get(&node)
            .and_then(|draw| draw.surfaces.first())
            .and_then(|surface| surface.material),
        Some(old_material)
    );

    runtime.apply_render_event(RenderEvent::MaterialCreated {
        request: pending_request,
        id: pending_material,
    });
    runtime.mark_needs_rerender(node);
    runtime.extract_render_3d_commands();
    let ready = collect_commands(&mut runtime);
    assert!(ready.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command)
            if matches!(
                command.as_ref(),
                Command3D::Draw { node: draw_node, surfaces, .. }
                    if *draw_node == node
                        && surfaces
                            .first()
                            .and_then(|surface| surface.material)
                            .is_some_and(|material| material == pending_material)
            )
    )));
}

#[test]
fn material_loaded_event_reemits_mesh_draw_using_material() {
    let mut runtime = Runtime::new();
    let mesh_id = MeshID::from_parts(61, 0);
    let material_id = MaterialID::from_parts(62, 0);
    let mut mesh = MeshInstance3D::new();
    mesh.mesh = mesh_id;
    set_primary_material(&mut mesh, material_id);
    let node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

    runtime.extract_render_3d_commands();
    let first = collect_commands(&mut runtime);
    assert!(first.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command)
            if matches!(command.as_ref(), Command3D::Draw { node: draw_node, .. } if *draw_node == node)
    )));

    runtime.apply_render_event(RenderEvent::MaterialLoaded { id: material_id });
    runtime.extract_render_3d_commands();
    let second = collect_commands(&mut runtime);
    assert!(second.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command)
            if matches!(
                command.as_ref(),
                Command3D::Draw { node: draw_node, surfaces, .. }
                    if *draw_node == node
                        && surfaces
                            .first()
                            .and_then(|surface| surface.material)
                            .is_some_and(|material| material == material_id)
            )
    )));
}

#[test]
fn animation_player_keeps_old_clip_while_replacement_clip_is_pending() {
    let mut runtime = Runtime::new();
    let target = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    let player = NodeAPI::create::<AnimationPlayer>(&mut runtime);
    let old_clip = runtime
        .resource_api
        .test_create_animation(node3d_position_clip(&[(0, 1.0), (1, 2.0), (2, 3.0)]), true);
    let pending_clip = runtime
        .resource_api
        .test_create_animation(node3d_position_clip(&[(0, 100.0), (1, 200.0)]), false);

    assert!(runtime.animation_set_clip(player, old_clip));
    assert!(runtime.animation_bind(player, "Tool", target));
    runtime.update(1.0);
    let x_after_old = runtime
        .nodes
        .get(target)
        .and_then(|node| match &node.data {
            SceneNodeData::Node3D(node) => Some(node.transform.position.x),
            _ => None,
        })
        .expect("target node");
    assert_eq!(x_after_old, 1.0);

    assert!(runtime.animation_set_clip(player, pending_clip));
    assert!(runtime.animation_seek_frame(player, 1));
    runtime.update(1.0);
    let x_while_pending = runtime
        .nodes
        .get(target)
        .and_then(|node| match &node.data {
            SceneNodeData::Node3D(node) => Some(node.transform.position.x),
            _ => None,
        })
        .expect("target node");
    assert_eq!(x_while_pending, 2.0);

    runtime
        .resource_api
        .test_mark_animation_loaded(pending_clip);
    runtime.update(1.0);
    let x_after_ready = runtime
        .nodes
        .get(target)
        .and_then(|node| match &node.data {
            SceneNodeData::Node3D(node) => Some(node.transform.position.x),
            _ => None,
        })
        .expect("target node");
    assert!(x_after_ready >= 100.0);
}

#[test]
fn mesh_under_invisible_parent_emits_remove_node() {
    let mut runtime = Runtime::new();
    let parent = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    let child = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(
            MeshInstance3D::new(),
        )));
    if let Some(mut parent_node) = runtime.nodes.get_mut(parent) {
        parent_node.add_child(child);
    }
    if let Some(mut child_node) = runtime.nodes.get_mut(child) {
        child_node.parent = parent;
    }

    let mesh = MeshID::from_parts(20, 0);
    let material = MaterialID::from_parts(21, 0);
    if let Some(mut node) = runtime.nodes.get_mut(child)
        && let SceneNodeData::MeshInstance3D(mesh_instance) = &mut node.data
    {
        mesh_instance.mesh = mesh;
        set_primary_material(mesh_instance, material);
    }

    runtime.extract_render_3d_commands();
    let first = collect_commands(&mut runtime);
    assert!(first.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command_3d)
            if matches!(command_3d.as_ref(), Command3D::Draw { node, .. } if *node == child)
    )));

    if let Some(mut node) = runtime.nodes.get_mut(parent)
        && let SceneNodeData::Node3D(parent_node) = &mut node.data
    {
        parent_node.visible = false;
    }
    runtime.mark_needs_rerender(parent);
    runtime.extract_render_3d_commands();
    let second = collect_commands(&mut runtime);
    assert!(second.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command_3d)
            if matches!(command_3d.as_ref(), Command3D::RemoveNode { node } if *node == child)
    )));
    assert_eq!(runtime.scene_mesh_refs_cache.get(&mesh), Some(&vec![child]));
    assert_eq!(
        runtime.scene_material_refs_cache.get(&material),
        Some(&vec![child])
    );
}

#[test]
fn removed_water_3d_emits_remove_node() {
    let mut runtime = Runtime::new();
    let water = NodeAPI::create::<WaterBody3D>(&mut runtime);

    runtime.extract_render_3d_commands();
    let first = collect_commands(&mut runtime);
    assert!(first.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command_3d)
            if matches!(command_3d.as_ref(), Command3D::UpsertWater { node, .. } if *node == water)
    )));

    assert!(NodeAPI::remove_node(&mut runtime, water));
    let second = collect_commands(&mut runtime);
    assert!(second.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command_3d)
            if matches!(command_3d.as_ref(), Command3D::RemoveNode { node } if *node == water)
    )));
}

#[test]
fn physics_pause_keeps_water_3d_visual_state_live() {
    let mut runtime = Runtime::new();
    let water = NodeAPI::create::<WaterBody3D>(&mut runtime);

    runtime.extract_render_3d_commands();
    let first = collect_commands(&mut runtime);
    assert!(!water_3d_command(&first, water).paused);
    runtime.clear_dirty_flags();

    runtime.extract_render_3d_commands();
    assert!(collect_commands(&mut runtime).is_empty());

    runtime.set_physics_paused(true);
    runtime.extract_render_3d_commands();
    let paused = collect_commands(&mut runtime);
    assert!(!water_3d_command(&paused, water).paused);
}

#[test]
fn unchanged_mesh_instance_emits_draw() {
    let mut runtime = Runtime::new();
    let node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(
            MeshInstance3D::new(),
        )));
    let mesh = MeshID::from_parts(30, 0);
    let material = MaterialID::from_parts(31, 0);
    if let Some(mut scene_node) = runtime.nodes.get_mut(node)
        && let SceneNodeData::MeshInstance3D(mesh_instance) = &mut scene_node.data
    {
        mesh_instance.mesh = mesh;
        set_primary_material(mesh_instance, material);
    }

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command_3d)
            if matches!(
                command_3d.as_ref(),
                Command3D::Draw {
                    node: draw_node, ..
                } if *draw_node == node
            )
    )));
}

#[test]
fn multi_mesh_instance_emits_draw_multi_with_instance_mats() {
    let mut runtime = Runtime::new();
    let mut multi = MultiMeshInstance3D::new();
    multi.mesh = MeshID::from_parts(330, 0);
    set_primary_material_multi(&mut multi, MaterialID::from_parts(331, 0));

    multi.instance_scale = 1.0;
    multi.instances = vec![
        perro_nodes::MultiMeshInstancePose::from_pos_rot(
            Vector3::new(1.0, 0.0, 0.0),
            Quaternion::IDENTITY,
        ),
        perro_nodes::MultiMeshInstancePose::from_pos_rot(
            Vector3::new(3.0, 0.0, 0.0),
            Quaternion::IDENTITY,
        ),
    ];

    let node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MultiMeshInstance3D(multi)));

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);
    assert!(commands.iter().any(|command| {
        matches!(
            command,
            RenderCommand::ThreeD(command_3d)
                if matches!(
                    command_3d.as_ref(),
                    Command3D::DrawMulti {
                        node: draw_node,
                        instance_mats,
                        ..
                    } if *draw_node == node
                        && instance_mats.len() == 2
                        && instance_mats[0][3][0] == 1.0
                        && instance_mats[1][3][0] == 3.0
                )
                || matches!(
                    command_3d.as_ref(),
                    Command3D::DrawMultiDense {
                        node: draw_node,
                        instances,
                        ..
                    } if *draw_node == node
                        && instances.len() == 2
                        && instances[0].position[0] == 1.0
                        && instances[1].position[0] == 3.0
                )
        )
    }));
}

#[test]
fn multi_mesh_instance_default_scale_is_one() {
    let multi = MultiMeshInstance3D::default();
    assert_eq!(multi.instance_scale, 1.0);
}

#[test]
fn multi_mesh_instance_passes_instance_scale_to_dense_draw() {
    let mut runtime = Runtime::new();
    let mut multi = MultiMeshInstance3D::new();
    multi.mesh = MeshID::from_parts(332, 0);
    set_primary_material_multi(&mut multi, MaterialID::from_parts(333, 0));
    multi.instances = vec![perro_nodes::MultiMeshInstancePose::new(Transform3D::new(
        Vector3::new(1.0, 2.0, 3.0),
        Quaternion::IDENTITY,
        Vector3::new(2.0, 3.0, 4.0),
    ))];

    let node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MultiMeshInstance3D(multi)));

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command_3d)
            if matches!(
                command_3d.as_ref(),
                Command3D::DrawMultiDense {
                    node: draw_node,
                    instances,
                    ..
                } if *draw_node == node && instances[0].scale == [2.0, 3.0, 4.0]
            )
    )));
}

#[test]
fn skinned_mesh_palette_uses_bone_pose_not_rest() {
    let mut runtime = Runtime::new();

    let mut skeleton = Skeleton3D::default();
    skeleton.bones = vec![Bone3D {
        rest: Transform3D::IDENTITY,
        pose: Transform3D::new(
            Vector3::new(2.0, 0.0, 0.0),
            Quaternion::IDENTITY,
            Vector3::ONE,
        ),
        inv_bind: Transform3D::IDENTITY,
        ..Bone3D::new()
    }];
    // Populate the derived inv-bind lane like a real scene load so the palette
    // builder takes the cached (non-fallback) path.
    skeleton.refresh_inv_bind_cache();
    let skeleton_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Skeleton3D(skeleton)));

    let mut mesh = MeshInstance3D::new();
    mesh.mesh = MeshID::from_parts(340, 0);
    mesh.skeleton = skeleton_id;
    set_primary_material(&mut mesh, MaterialID::from_parts(341, 0));
    runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command_3d)
            if matches!(
                command_3d.as_ref(),
                Command3D::Draw {
                    skeleton: Some(palette),
                    ..
                    // Palette rows are affine (row-major); translation.x is row0[3].
                } if palette.matrices.first().is_some_and(|m| m[0][3] == 2.0)
            )
    )));
}

#[test]
fn dirty_skeleton_refreshes_sibling_skinned_mesh_draw() {
    let mut runtime = Runtime::new();

    let mut skeleton = Skeleton3D::default();
    skeleton.bones = vec![Bone3D {
        pose: Transform3D::IDENTITY,
        ..Bone3D::new()
    }];
    let skeleton_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Skeleton3D(skeleton)));

    let mut mesh = MeshInstance3D::new();
    mesh.mesh = MeshID::from_parts(350, 0);
    mesh.skeleton = skeleton_id;
    set_primary_material(&mut mesh, MaterialID::from_parts(351, 0));
    let mesh_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

    runtime.extract_render_3d_commands();
    let _ = collect_commands(&mut runtime);

    if let Some(mut node) = runtime.nodes.get_mut(skeleton_id)
        && let SceneNodeData::Skeleton3D(skeleton) = &mut node.data
    {
        skeleton.bones[0].pose.position.x = 3.0;
    }
    runtime.mark_needs_rerender(skeleton_id);

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command_3d)
            if matches!(
                command_3d.as_ref(),
                Command3D::Draw {
                    node,
                    skeleton: Some(palette),
                    ..
                } if *node == mesh_id
                    && palette.matrices.first().is_some_and(|m| m[0][3] == 3.0)
            )
    )));
}

#[test]
fn active_camera_3d_emits_set_camera_command() {
    let mut runtime = Runtime::new();
    let mut camera = Camera3D {
        active: true,
        projection: CameraProjection::Orthographic {
            size: 24.0,
            near: 0.2,
            far: 600.0,
        },
        ..Default::default()
    };
    camera.transform.position.x = 6.0;
    camera.transform.position.y = 7.0;
    camera.transform.position.z = 8.0;
    camera.transform.rotation.x = 0.1;
    camera.transform.rotation.y = 0.2;
    camera.transform.rotation.z = 0.3;
    camera.transform.rotation.w = 0.9;
    runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Camera3D(camera)));

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command_3d)
            if matches!(
                command_3d.as_ref(),
                Command3D::SetCamera { camera }
                    if camera.position == [6.0, 7.0, 8.0]
                        && camera.rotation == [0.1, 0.2, 0.3, 0.9]
                        && matches!(
                            camera.projection,
                            CameraProjectionState::Orthographic { size, near, far }
                                if size == 24.0 && near == 0.2 && far == 600.0
                        )
            )
    )));
}

#[test]
fn deactivating_last_camera_3d_resets_renderer_camera() {
    let mut runtime = Runtime::new();
    let mut camera = Camera3D {
        active: true,
        ..Default::default()
    };
    camera.transform.position.x = 12.0;
    let camera_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Camera3D(camera)));

    runtime.extract_render_3d_commands();
    let _ = collect_commands(&mut runtime);

    NodeAPI::with_node_mut::<Camera3D, _, _>(&mut runtime, camera_node, |camera| {
        camera.active = false;
    });
    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);

    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command_3d)
            if matches!(
                command_3d.as_ref(),
                Command3D::SetCamera { camera } if camera.position == [0.0, 0.0, 6.0]
            )
    )));
}

#[test]
fn newly_activated_camera_3d_wins_over_higher_slot_old_camera() {
    let mut runtime = Runtime::new();
    let dummy = NodeAPI::create::<Node3D>(&mut runtime);
    let old_camera = NodeAPI::create::<Camera3D>(&mut runtime);
    NodeAPI::with_node_mut::<Camera3D, _, _>(&mut runtime, old_camera, |camera| {
        camera.active = true;
        camera.transform.position.x = 1.0;
    });
    let _ = NodeAPI::remove_node(&mut runtime, dummy);
    let new_camera = NodeAPI::create::<Camera3D>(&mut runtime);
    NodeAPI::with_node_mut::<Camera3D, _, _>(&mut runtime, new_camera, |camera| {
        camera.active = true;
        camera.transform.position.x = 9.0;
    });

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);

    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command_3d)
            if matches!(
                command_3d.as_ref(),
                Command3D::SetCamera { camera } if camera.position[0] == 9.0
            )
    )));
}

#[test]
fn camera_3d_render_mask_filters_meshes() {
    let mut runtime = Runtime::new();
    let camera = Camera3D {
        active: true,
        render_mask: BitMask::with([2]),
        ..Default::default()
    };
    let camera_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Camera3D(camera)));

    let mut mesh = MeshInstance3D::new();
    mesh.mesh = MeshID::from_parts(92, 0);
    mesh.render_layers = BitMask::with([2]);
    set_primary_material(&mut mesh, MaterialID::from_parts(93, 0));
    let mesh_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

    runtime.extract_render_3d_commands();
    let first = collect_commands(&mut runtime);
    assert!(!first.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command_3d)
            if matches!(command_3d.as_ref(), Command3D::Draw { node, .. } if *node == mesh_node)
    )));

    if let Some(mut node) = runtime.nodes.get_mut(camera_node)
        && let SceneNodeData::Camera3D(camera) = &mut node.data
    {
        camera.render_mask = BitMask::with([1]);
    }
    runtime.mark_needs_rerender(camera_node);

    runtime.extract_render_3d_commands();
    let second = collect_commands(&mut runtime);
    assert!(second.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command_3d)
            if matches!(command_3d.as_ref(), Command3D::Draw { node, .. } if *node == mesh_node)
    )));
}

#[test]
fn camera_3d_move_does_not_rewalk_mesh_render_layers() {
    let mut runtime = Runtime::new();
    let camera = Camera3D {
        active: true,
        ..Default::default()
    };
    let camera_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Camera3D(camera)));

    let mut mesh = MeshInstance3D::new();
    mesh.mesh = MeshID::from_parts(95, 0);
    set_primary_material(&mut mesh, MaterialID::from_parts(96, 0));
    let mesh_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

    runtime.extract_render_3d_commands();
    let _ = collect_commands(&mut runtime);

    if let Some(mut node) = runtime.nodes.get_mut(camera_node)
        && let SceneNodeData::Camera3D(camera) = &mut node.data
    {
        camera.transform.position.x = 10.0;
    }
    runtime.mark_transform_dirty_recursive(camera_node);

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command_3d)
            if matches!(command_3d.as_ref(), Command3D::SetCamera { .. })
    )));
    assert!(!commands.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command_3d)
            if matches!(command_3d.as_ref(), Command3D::Draw { node, .. } if *node == mesh_node)
    )));
}

#[test]
fn active_ray_light_3d_emits_set_ray_light_command() {
    let mut runtime = Runtime::new();
    let mut light = RayLight3D::new();
    light.color = Color::new(0.8, 0.7, 0.6, 1.0);
    light.intensity = 2.5;
    light.shadow_strength = 0.55;
    light.shadow_depth_bias = 0.001;
    light.shadow_normal_bias = 0.12;
    light.active = true;
    runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::RayLight3D(light)));

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command_3d)
            if matches!(
                command_3d.as_ref(),
                Command3D::SetRayLight { light, .. }
                    if light.color == Color::new(0.8, 0.7, 0.6, 1.0).to_rgb()
                        && light.intensity == 2.5
                        && light.shadow_strength == 0.55
                        && light.shadow_depth_bias == 0.001
                        && light.shadow_normal_bias == 0.12
            )
    )));
}

#[test]
fn active_ambient_light_3d_emits_set_ambient_light_command() {
    let mut runtime = Runtime::new();
    let mut light = AmbientLight3D::new();
    light.color = Color::new(0.25, 0.3, 0.4, 1.0);
    light.intensity = 0.2;
    light.active = true;
    runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::AmbientLight3D(light)));

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command_3d)
            if matches!(
                command_3d.as_ref(),
                Command3D::SetAmbientLight { light, .. }
                    if light.color == Color::new(0.25, 0.3, 0.4, 1.0).to_rgb()
                        && light.intensity == 0.2
            )
    )));
}

#[test]
fn active_sky_3d_emits_set_sky_command() {
    let mut runtime = Runtime::new();
    let mut sky = Sky3D::default();
    sky.palette.day_colors = vec![[0.4, 0.6, 0.9], [0.9, 0.95, 1.0]];
    sky.palette.evening_colors = vec![[0.95, 0.45, 0.22], [0.7, 0.2, 0.35]];
    sky.palette.night_colors = vec![[0.01, 0.02, 0.05], [0.04, 0.08, 0.18]];
    sky.time.time_of_day = 0.67;
    sky.time.paused = true;
    sky.time.scale = 0.25;
    sky.active = true;
    runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Sky3D(sky)));

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command_3d)
            if matches!(
                command_3d.as_ref(),
                Command3D::SetSky { sky, .. }
                    if sky.time.time_of_day == 0.67
                        && sky.time.paused
                        && sky.time.scale == 0.25
                        && sky.day_colors.len() == 2
                        && sky.evening_colors.len() == 2
                        && sky.night_colors.len() == 2
            )
    )));
}

#[test]
fn unchanged_sky_3d_does_not_reemit_set_sky_command() {
    let mut runtime = Runtime::new();
    let mut sky = Sky3D::default();
    sky.palette.day_colors = vec![[0.4, 0.6, 0.9], [0.9, 0.95, 1.0]];
    sky.active = true;
    let node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Sky3D(sky)));

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, RenderCommand::ThreeD(command_3d) if matches!(command_3d.as_ref(), Command3D::SetSky { .. })))
    );

    // Re-mark the node dirty (via a transform touch) without changing any
    // sky data, so the retained-state comparison runs again on revisit.
    runtime.mark_transform_dirty_recursive(node);
    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, RenderCommand::ThreeD(command_3d) if matches!(command_3d.as_ref(), Command3D::SetSky { .. })))
    );
}

#[test]
fn sky_3d_state_matches_compares_all_fields() {
    let mut sky = Sky3D::default();
    sky.palette.day_colors = vec![[0.1, 0.2, 0.3]];
    sky.palette.evening_colors = vec![[0.4, 0.5, 0.6]];
    sky.palette.night_colors = vec![[0.7, 0.8, 0.9]];
    sky.palette.horizon_colors = vec![[0.2, 0.2, 0.2]];
    sky.time.time_of_day = 0.5;
    sky.time.paused = false;
    sky.time.scale = 1.0;
    sky.shaders
        .push(perro_nodes::sky_3d::SkyShaderPass::new("shader_a"));

    let retained = super::Sky3DState {
        day_colors: Arc::from(sky.palette.day_colors.as_slice()),
        evening_colors: Arc::from(sky.palette.evening_colors.as_slice()),
        night_colors: Arc::from(sky.palette.night_colors.as_slice()),
        horizon_colors: Arc::from(sky.palette.horizon_colors.as_slice()),
        time: super::SkyTime3DState {
            time_of_day: sky.time.time_of_day,
            paused: sky.time.paused,
            scale: sky.time.scale,
        },
        shaders: Arc::from(
            sky.shaders
                .iter()
                .map(|shader| super::SkyShaderPass3DState {
                    path: shader.path.clone(),
                    params: Arc::from(shader.params.as_slice()),
                })
                .collect::<Vec<_>>(),
        ),
    };

    assert!(super::sky_3d_state_matches(&retained, &sky));

    sky.time.time_of_day = 0.75;
    assert!(!super::sky_3d_state_matches(&retained, &sky));
    sky.time.time_of_day = 0.5;

    sky.shaders
        .push(perro_nodes::sky_3d::SkyShaderPass::new("shader_b"));
    assert!(!super::sky_3d_state_matches(&retained, &sky));
}

#[test]
fn mesh_under_parent_uses_global_transform() {
    let mut runtime = Runtime::new();

    let mut parent_node = Node3D::new();
    parent_node.transform.position.x = 15.0;
    let parent = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(parent_node)));

    let mut mesh = MeshInstance3D::new();
    mesh.mesh = MeshID::from_parts(41, 0);
    set_primary_material(&mut mesh, MaterialID::from_parts(42, 0));
    mesh.transform.position.x = 1.0;
    let child = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

    if let Some(mut parent_node) = runtime.nodes.get_mut(parent) {
        parent_node.add_child(child);
    }
    if let Some(mut child_node) = runtime.nodes.get_mut(child) {
        child_node.parent = parent;
    }
    runtime.mark_transform_dirty_recursive(parent);

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command_3d)
            if matches!(
                command_3d.as_ref(),
                Command3D::Draw { node, model, .. }
                    if *node == child
                        && model[3][0] == 16.0
                        && model[3][1] == 0.0
                        && model[3][2] == 0.0
            )
    )));
}

#[test]
fn mesh_instance_passes_meshlet_override_to_draw_command() {
    let mut runtime = Runtime::new();
    let mut mesh = MeshInstance3D::new();
    mesh.mesh = MeshID::from_parts(420, 0);
    mesh.meshlet_override = Some(false);
    set_primary_material(&mut mesh, MaterialID::from_parts(421, 0));
    let node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::ThreeD(command_3d)
            if matches!(
                command_3d.as_ref(),
                Command3D::Draw {
                    node: draw_node,
                    meshlet_override,
                    ..
                } if *draw_node == node && *meshlet_override == Some(false)
            )
    )));
}

#[test]
fn multi_mesh_instance_passes_meshlet_override_to_draw_command() {
    let mut runtime = Runtime::new();
    let mut multi = MultiMeshInstance3D::new();
    multi.mesh = MeshID::from_parts(430, 0);
    multi.meshlet_override = Some(true);
    set_primary_material_multi(&mut multi, MaterialID::from_parts(431, 0));
    multi.instances = vec![perro_nodes::MultiMeshInstancePose::from_pos_rot(
        Vector3::new(0.0, 0.0, 0.0),
        Quaternion::IDENTITY,
    )];
    let node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::MultiMeshInstance3D(multi)));

    runtime.extract_render_3d_commands();
    let commands = collect_commands(&mut runtime);
    assert!(commands.iter().any(|command| {
        matches!(
            command,
            RenderCommand::ThreeD(command_3d)
                if matches!(
                    command_3d.as_ref(),
                    Command3D::DrawMulti {
                        node: draw_node,
                        meshlet_override,
                        ..
                    } if *draw_node == node && *meshlet_override == Some(true)
                )
                || matches!(
                    command_3d.as_ref(),
                    Command3D::DrawMultiDense {
                        node: draw_node,
                        meshlet_override,
                        ..
                    } if *draw_node == node && *meshlet_override == Some(true)
                )
        )
    }));
}

#[test]
fn collision_shape_debug_rebuilds_when_parent_moves() {
    let mut runtime = Runtime::new();

    let mut parent_node = Node3D::new();
    parent_node.transform.position.x = 2.0;
    let parent = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(parent_node)));

    let collision = perro_nodes::CollisionShape3D {
        debug: true,
        shape: Shape3D::Cube {
            size: Vector3::new(2.0, 2.0, 2.0),
        },
        ..Default::default()
    };
    let child = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::CollisionShape3D(collision)));

    if let Some(mut parent_node) = runtime.nodes.get_mut(parent) {
        parent_node.add_child(child);
    }
    if let Some(mut child_node) = runtime.nodes.get_mut(child) {
        child_node.parent = parent;
    }
    runtime.mark_transform_dirty_recursive(parent);

    runtime.extract_render_3d_commands();
    let first = collect_commands(&mut runtime);
    let first_x = first
        .iter()
        .find_map(|command| match command {
            RenderCommand::ThreeD(command_3d) => match command_3d.as_ref() {
                Command3D::DrawDebugLine3D { start, .. } => Some(start[0]),
                _ => None,
            },
            _ => None,
        })
        .expect("expected collision debug line draw");

    if let Some(mut node) = runtime.nodes.get_mut(parent)
        && let SceneNodeData::Node3D(parent_node) = &mut node.data
    {
        parent_node.transform.position.x = 8.0;
    }
    runtime.mark_transform_dirty_recursive(parent);

    runtime.extract_render_3d_commands();
    let second = collect_commands(&mut runtime);
    let second_x = second
        .iter()
        .find_map(|command| match command {
            RenderCommand::ThreeD(command_3d) => match command_3d.as_ref() {
                Command3D::DrawDebugLine3D { start, .. } => Some(start[0]),
                _ => None,
            },
            _ => None,
        })
        .expect("expected collision debug line draw after move");

    assert_ne!(first_x, second_x);
}

#[test]
fn sliding_window_max_covers_edges_and_plateaus() {
    let input = [0u8, 10, 0, 0, 200, 0, 0, 0, 50];
    let mut out = vec![0u8; input.len()];
    super::sliding_window_max(&input, 2, &mut out);
    assert_eq!(out, vec![10, 10, 200, 200, 200, 200, 200, 50, 50]);
}

#[test]
fn dilate_mask_grows_square_neighborhood() {
    let mut mask = vec![0u8; 25];
    mask[12] = 255; // center of 5x5
    let out = super::dilate_mask(&mask, 5, 5, 1);
    let expected: Vec<u8> = (0..25)
        .map(|i| {
            let (x, y) = (i % 5, i / 5);
            if (1..=3).contains(&x) && (1..=3).contains(&y) {
                255
            } else {
                0
            }
        })
        .collect();
    assert_eq!(out, expected);
}

#[test]
fn text_decal_raster_background_carries_fill_rgb() {
    // Transparent texels must carry the fill color so linear filtering never
    // blends toward black (the old dark-fringe artifact).
    let params = super::TextDecalRasterParams {
        node: perro_ids::NodeID::from_u64(1),
        text: "",
        size: Vector3::new(2.0, 0.5, 0.25),
        font_size: 32.0,
        h_align: perro_ui::UiTextAlign::Center,
        v_align: perro_ui::UiTextAlign::Center,
        texture_resolution: 64,
        color: Color::new(1.0, 0.5, 0.0, 1.0),
        outline_width: 0.0,
        outline_color: Color::BLACK,
    };
    let (rgba, width, height) = super::raster_text_decal(&params);
    assert!(width > 0 && height > 0);
    for pixel in rgba.chunks_exact(4) {
        assert_eq!(pixel[0], 255);
        assert_eq!(pixel[1], 128);
        assert_eq!(pixel[2], 0);
        assert_eq!(pixel[3], 0);
    }
}

#[test]
fn text_decal_outline_grows_coverage_and_tints_border() {
    let base = super::TextDecalRasterParams {
        node: perro_ids::NodeID::from_u64(1),
        text: "A",
        size: Vector3::new(1.0, 1.0, 0.25),
        font_size: 48.0,
        h_align: perro_ui::UiTextAlign::Center,
        v_align: perro_ui::UiTextAlign::Center,
        texture_resolution: 64,
        color: Color::WHITE,
        outline_width: 0.0,
        outline_color: Color::BLACK,
    };
    let (plain, _, _) = super::raster_text_decal(&base);
    let outlined_params = super::TextDecalRasterParams {
        outline_width: 4.0,
        ..base
    };
    let (outlined, _, _) = super::raster_text_decal(&outlined_params);
    let plain_coverage = plain.chunks_exact(4).filter(|px| px[3] > 0).count();
    let outlined_coverage = outlined.chunks_exact(4).filter(|px| px[3] > 0).count();
    assert!(plain_coverage > 0, "glyph raster produced no coverage");
    assert!(
        outlined_coverage > plain_coverage,
        "outline must dilate coverage ({outlined_coverage} <= {plain_coverage})"
    );
    // Pure-outline texels (opaque in outlined, transparent in plain) take the
    // outline color.
    let border_black = plain
        .chunks_exact(4)
        .zip(outlined.chunks_exact(4))
        .filter(|(p, o)| p[3] == 0 && o[3] == 255)
        .all(|(_, o)| o[0] == 0 && o[1] == 0 && o[2] == 0);
    assert!(border_black, "outline-only texels must use outline color");
}
