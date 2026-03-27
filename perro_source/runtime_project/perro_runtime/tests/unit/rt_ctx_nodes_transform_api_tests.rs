use crate::Runtime;
use perro_nodes::{Node2D, Node3D, SceneNode, SceneNodeData};
use perro_runtime_context::sub_apis::NodeAPI;
use perro_structs::{Quaternion, Transform2D, Transform3D, Vector2, Vector3};

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() <= 1e-4
}

#[test]
fn get_set_global_transform_3d_works_under_scaled_parent() {
    let mut runtime = Runtime::new();

    let mut parent = Node3D::new();
    parent.transform.position = Vector3::new(0.0, 1.0, 0.0);
    parent.transform.scale = Vector3::new(15.0, 15.0, 15.0);
    let parent_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(parent)));

    let child_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));

    if let Some(parent_node) = runtime.nodes.get_mut(parent_id) {
        parent_node.add_child(child_id);
    }
    if let Some(child_node) = runtime.nodes.get_mut(child_id) {
        child_node.parent = parent_id;
    }
    runtime.mark_transform_dirty_recursive(parent_id);

    let desired = Transform3D::new(
        Vector3::new(0.0, 0.0, 0.0),
        Quaternion::IDENTITY,
        Vector3::ONE,
    );
    assert!(runtime.set_global_transform_3d(child_id, desired));

    let child_global = runtime
        .get_global_transform_3d(child_id)
        .expect("child global must exist");
    assert!(approx(child_global.position.x, 0.0));
    assert!(approx(child_global.position.y, 0.0));
    assert!(approx(child_global.position.z, 0.0));

    let child_local = runtime
        .with_base_node::<Node3D, _, _>(child_id, |node| node.transform)
        .expect("child local must exist");
    assert!(approx(child_local.position.x, 0.0));
    assert!(approx(child_local.position.y, -1.0 / 15.0));
    assert!(approx(child_local.position.z, 0.0));
}

#[test]
fn to_global_and_to_local_points_3d_roundtrip() {
    let mut runtime = Runtime::new();

    let mut parent = Node3D::new();
    parent.transform.position = Vector3::new(0.0, 1.0, 0.0);
    parent.transform.scale = Vector3::new(15.0, 15.0, 15.0);
    let parent_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(parent)));

    let mut child = Node3D::new();
    child.transform.position = Vector3::new(0.0, -1.0, 0.0);
    let child_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(child)));

    if let Some(parent_node) = runtime.nodes.get_mut(parent_id) {
        parent_node.add_child(child_id);
    }
    if let Some(child_node) = runtime.nodes.get_mut(child_id) {
        child_node.parent = parent_id;
    }
    runtime.mark_transform_dirty_recursive(parent_id);

    let world = runtime
        .to_global_point_3d(child_id, Vector3::ZERO)
        .expect("global point must exist");
    assert!(approx(world.x, 0.0));
    assert!(approx(world.y, -14.0));
    assert!(approx(world.z, 0.0));

    let local = runtime
        .to_local_point_3d(child_id, world)
        .expect("local point must exist");
    assert!(approx(local.x, 0.0));
    assert!(approx(local.y, 0.0));
    assert!(approx(local.z, 0.0));
}

#[test]
fn get_set_global_transform_2d_and_point_conversion() {
    let mut runtime = Runtime::new();

    let mut parent = Node2D::new();
    parent.transform.position = Vector2::new(10.0, 0.0);
    parent.transform.scale = Vector2::new(2.0, 2.0);
    let parent_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node2D(parent)));

    let child_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node2D(Node2D::new())));

    if let Some(parent_node) = runtime.nodes.get_mut(parent_id) {
        parent_node.add_child(child_id);
    }
    if let Some(child_node) = runtime.nodes.get_mut(child_id) {
        child_node.parent = parent_id;
    }
    runtime.mark_transform_dirty_recursive(parent_id);

    let desired = Transform2D::new(Vector2::new(16.0, 0.0), 0.0, Vector2::ONE);
    assert!(runtime.set_global_transform_2d(child_id, desired));

    let child_global = runtime
        .get_global_transform_2d(child_id)
        .expect("child global must exist");
    assert!(approx(child_global.position.x, 16.0));
    assert!(approx(child_global.position.y, 0.0));

    let world = runtime
        .to_global_point_2d(child_id, Vector2::new(1.0, 0.0))
        .expect("global point must exist");
    assert!(approx(world.x, 17.0));
    assert!(approx(world.y, 0.0));

    let local = runtime
        .to_local_point_2d(child_id, world)
        .expect("local point must exist");
    assert!(approx(local.x, 1.0));
    assert!(approx(local.y, 0.0));
}

#[test]
fn reparent_preserves_child_global_transform_3d() {
    let mut runtime = Runtime::new();

    let mut parent_a = Node3D::new();
    parent_a.transform.position = Vector3::new(10.0, 0.0, 0.0);
    let parent_a_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(parent_a)));

    let mut parent_b = Node3D::new();
    parent_b.transform.position = Vector3::new(-5.0, 0.0, 0.0);
    let parent_b_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(parent_b)));

    let mut child = Node3D::new();
    child.transform.position = Vector3::new(2.0, 0.0, 0.0);
    let child_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(child)));

    if let Some(parent) = runtime.nodes.get_mut(parent_a_id) {
        parent.add_child(child_id);
    }
    if let Some(child) = runtime.nodes.get_mut(child_id) {
        child.parent = parent_a_id;
    }
    runtime.mark_transform_dirty_recursive(parent_a_id);

    let before = runtime
        .get_global_transform_3d(child_id)
        .expect("child global before reparent must exist");
    assert!(runtime.reparent(parent_b_id, child_id));

    let after = runtime
        .get_global_transform_3d(child_id)
        .expect("child global after reparent must exist");
    assert!(approx(before.position.x, after.position.x));
    assert!(approx(before.position.y, after.position.y));
    assert!(approx(before.position.z, after.position.z));

    let local = runtime
        .with_base_node::<Node3D, _, _>(child_id, |node| node.transform)
        .expect("child local must exist");
    assert!(approx(local.position.x, 17.0));
}

#[test]
fn remove_node_removes_entire_subtree() {
    let mut runtime = Runtime::new();

    let root_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    let child_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    let grandchild_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));

    if let Some(root) = runtime.nodes.get_mut(root_id) {
        root.add_child(child_id);
    }
    if let Some(child) = runtime.nodes.get_mut(child_id) {
        child.parent = root_id;
        child.add_child(grandchild_id);
    }
    if let Some(grandchild) = runtime.nodes.get_mut(grandchild_id) {
        grandchild.parent = child_id;
    }

    assert!(runtime.remove_node(root_id));
    assert!(runtime.nodes.get(root_id).is_none());
    assert!(runtime.nodes.get(child_id).is_none());
    assert!(runtime.nodes.get(grandchild_id).is_none());
    assert!(!runtime.remove_node(root_id));
}
