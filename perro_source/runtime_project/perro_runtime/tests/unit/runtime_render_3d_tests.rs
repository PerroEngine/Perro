use super::Runtime;
use perro_ids::{MaterialID, MeshID};
use perro_nodes::{
    CameraProjection, SceneNode, SceneNodeData, ambient_light_3d::AmbientLight3D,
    camera_3d::Camera3D, mesh_instance_3d::MeshInstance3D, mesh_instance_3d::MeshSurfaceBinding,
    multi_mesh_instance_3d::MultiMeshInstance3D, node_3d::Node3D, physics_3d::Shape3D,
    ray_light_3d::RayLight3D, sky_3d::Sky3D,
};
use perro_render_bridge::{
    CameraProjectionState, Command3D, RenderCommand, RenderEvent, ResourceCommand,
};
use perro_structs::{Quaternion, Vector3};

fn collect_commands(runtime: &mut Runtime) -> Vec<RenderCommand> {
    let mut out = Vec::new();
    runtime.drain_render_commands(&mut out);
    out
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
fn mesh_instance_emits_draw_after_mesh_and_material_created() {
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
    let material_request = match &second[0] {
        RenderCommand::Resource(ResourceCommand::CreateMaterial { request, .. }) => *request,
        _ => panic!("expected material create request"),
    };

    let expected_material = MaterialID::from_parts(7, 4);
    runtime.apply_render_event(RenderEvent::MaterialCreated {
        request: material_request,
        id: expected_material,
    });
    runtime.extract_render_3d_commands();
    let third = collect_commands(&mut runtime);
    assert_eq!(third.len(), 1);
    assert!(matches!(
        &third[0],
        RenderCommand::ThreeD(command)
            if matches!(
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
            )
    ));
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
    assert_eq!(second.len(), 1);
    assert!(matches!(
        second[0],
        RenderCommand::Resource(ResourceCommand::CreateMaterial { .. })
    ));
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
    if let Some(parent_node) = runtime.nodes.get_mut(parent) {
        parent_node.add_child(child);
    }
    if let Some(child_node) = runtime.nodes.get_mut(child) {
        child_node.parent = parent;
    }

    let mesh = MeshID::from_parts(20, 0);
    let material = MaterialID::from_parts(21, 0);
    if let Some(node) = runtime.nodes.get_mut(child)
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

    if let Some(node) = runtime.nodes.get_mut(parent)
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
    if let Some(scene_node) = runtime.nodes.get_mut(node)
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
        (Vector3::new(1.0, 0.0, 0.0), Quaternion::IDENTITY),
        (Vector3::new(3.0, 0.0, 0.0), Quaternion::IDENTITY),
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
fn active_camera_3d_emits_set_camera_command() {
    let mut runtime = Runtime::new();
    let mut camera = Camera3D::new();
    camera.active = true;
    camera.projection = CameraProjection::Orthographic {
        size: 24.0,
        near: 0.2,
        far: 600.0,
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
fn active_ray_light_3d_emits_set_ray_light_command() {
    let mut runtime = Runtime::new();
    let mut light = RayLight3D::new();
    light.color = [0.8, 0.7, 0.6];
    light.intensity = 2.5;
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
                    if light.color == [0.8, 0.7, 0.6] && light.intensity == 2.5
            )
    )));
}

#[test]
fn active_ambient_light_3d_emits_set_ambient_light_command() {
    let mut runtime = Runtime::new();
    let mut light = AmbientLight3D::new();
    light.color = [0.25, 0.3, 0.4];
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
                    if light.color == [0.25, 0.3, 0.4] && light.intensity == 0.2
            )
    )));
}

#[test]
fn active_sky_3d_emits_set_sky_command() {
    let mut runtime = Runtime::new();
    let mut sky = Sky3D::new();
    sky.day_colors = std::borrow::Cow::Owned(vec![[0.4, 0.6, 0.9], [0.9, 0.95, 1.0]]);
    sky.evening_colors = std::borrow::Cow::Owned(vec![[0.95, 0.45, 0.22], [0.7, 0.2, 0.35]]);
    sky.night_colors = std::borrow::Cow::Owned(vec![[0.01, 0.02, 0.05], [0.04, 0.08, 0.18]]);
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

    if let Some(parent_node) = runtime.nodes.get_mut(parent) {
        parent_node.add_child(child);
    }
    if let Some(child_node) = runtime.nodes.get_mut(child) {
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
    multi.instances = vec![(Vector3::new(0.0, 0.0, 0.0), Quaternion::IDENTITY)];
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

    let mut collision = perro_nodes::CollisionShape3D::new();
    collision.debug = true;
    collision.shape = Shape3D::Cube {
        size: Vector3::new(2.0, 2.0, 2.0),
    };
    let child = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::CollisionShape3D(collision)));

    if let Some(parent_node) = runtime.nodes.get_mut(parent) {
        parent_node.add_child(child);
    }
    if let Some(child_node) = runtime.nodes.get_mut(child) {
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

    if let Some(node) = runtime.nodes.get_mut(parent)
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
#[ignore]
fn bench_quaternion_forward_hotloop_compare_scalar_vs_simd() {
    let samples = 1_000_000usize;
    let mut seed = 0x1234_5678_9ABC_DEF0u64;
    let mut data = Vec::with_capacity(samples);
    for _ in 0..samples {
        seed ^= seed >> 12;
        seed ^= seed << 25;
        seed ^= seed >> 27;
        let x = ((seed & 0xFFFF) as f32 / 65535.0) * 2.0 - 1.0;
        let y = (((seed >> 16) & 0xFFFF) as f32 / 65535.0) * 2.0 - 1.0;
        let z = (((seed >> 32) & 0xFFFF) as f32 / 65535.0) * 2.0 - 1.0;
        let w = (((seed >> 48) & 0xFFFF) as f32 / 65535.0) * 2.0 - 1.0;
        data.push(Quaternion::new(x, y, z, w));
    }

    let start_scalar = std::time::Instant::now();
    let mut acc_scalar = 0.0f32;
    for q in &data {
        let f = super::quaternion_forward_scalar_legacy(*q);
        acc_scalar += f[0] + f[1] + f[2];
    }
    let elapsed_scalar = start_scalar.elapsed();

    let start_simd = std::time::Instant::now();
    let mut acc_simd = 0.0f32;
    for q in &data {
        let f = super::quaternion_forward(*q);
        acc_simd += f[0] + f[1] + f[2];
    }
    let elapsed_simd = start_simd.elapsed();
    let speedup = elapsed_scalar.as_secs_f64() / elapsed_simd.as_secs_f64();
    println!(
        "bench_quaternion_forward_hotloop_compare_scalar_vs_simd: samples={} scalar_us={} simd_us={} speedup={:.3}x acc_scalar={} acc_simd={}",
        samples,
        elapsed_scalar.as_micros(),
        elapsed_simd.as_micros(),
        speedup,
        acc_scalar,
        acc_simd
    );
}
