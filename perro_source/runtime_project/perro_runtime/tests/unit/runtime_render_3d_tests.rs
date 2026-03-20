use super::Runtime;
use perro_ids::{MaterialID, MeshID};
use perro_nodes::{
    CameraProjection, SceneNode, SceneNodeData, ambient_light_3d::AmbientLight3D,
    camera_3d::Camera3D, mesh_instance_3d::MeshInstance3D, node_3d::Node3D,
    ray_light_3d::RayLight3D, terrain_instance_3d::TerrainInstance3D,
};
use perro_render_bridge::{
    CameraProjectionState, Command3D, RenderCommand, RenderEvent, ResourceCommand,
};

fn collect_commands(runtime: &mut Runtime) -> Vec<RenderCommand> {
    let mut out = Vec::new();
    runtime.drain_render_commands(&mut out);
    out
}

#[test]
fn mesh_instance_without_mesh_source_requests_nothing() {
    let mut runtime = Runtime::new();
    let mut mesh = MeshInstance3D::new();
    mesh.mesh = MeshID::nil();
    mesh.material = MaterialID::nil();
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
    mesh.material = MaterialID::nil();
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
                    material,
                    ..
                } if *node == expected_node && *mesh == expected_mesh && *material == expected_material
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
        mesh_instance.material = material;
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
        mesh_instance.material = material;
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
fn terrain_instance_emits_runtime_chunk_mesh_commands() {
    let mut runtime = Runtime::new();
    let node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::TerrainInstance3D(
            TerrainInstance3D::new(),
        )));

    runtime.extract_render_3d_commands();
    let assigned_id = runtime
        .nodes
        .get(node)
        .and_then(|node| match &node.data {
            SceneNodeData::TerrainInstance3D(terrain) => Some(terrain.terrain),
            _ => None,
        })
        .expect("expected terrain node");
    assert!(!assigned_id.is_nil());
    assert!(
        runtime
            .terrain_store
            .lock()
            .expect("terrain store mutex poisoned")
            .get(assigned_id)
            .is_some()
    );

    let first = collect_commands(&mut runtime);
    assert!(first.iter().any(|command| matches!(
        command,
        RenderCommand::Resource(ResourceCommand::CreateRuntimeMesh { source, .. })
            if source.starts_with("__terrain_runtime__/")
    )));
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
fn mesh_under_parent_uses_global_transform() {
    let mut runtime = Runtime::new();

    let mut parent_node = Node3D::new();
    parent_node.transform.position.x = 15.0;
    let parent = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(parent_node)));

    let mut mesh = MeshInstance3D::new();
    mesh.mesh = MeshID::from_parts(41, 0);
    mesh.material = MaterialID::from_parts(42, 0);
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
