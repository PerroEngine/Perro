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

#[test]
fn removed_node_cache_does_not_leak_to_reused_slot_3d() {
    let mut runtime = Runtime::new();

    let mut original = Node3D::new();
    original.transform.position = Vector3::new(3.0, 4.0, 5.0);
    let original_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(original)));
    runtime.mark_transform_dirty_recursive(original_id);
    let original_global = runtime
        .get_global_transform_3d(original_id)
        .expect("original global must exist");
    assert!(approx(original_global.position.x, 3.0));
    assert!(runtime.remove_node(original_id));

    let mut reused = None;
    for i in 0..2048 {
        let mut node = Node3D::new();
        node.transform.position = Vector3::new(-11.0, i as f32, 7.0);
        let id = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::Node3D(node)));
        if id.index() == original_id.index() && id.generation() != original_id.generation() {
            reused = Some((id, i as f32));
            break;
        }
    }

    let (reused_id, expected_y) = reused.expect("slot must be reused with new generation");
    runtime.mark_transform_dirty_recursive(reused_id);
    let reused_global = runtime
        .get_global_transform_3d(reused_id)
        .expect("reused global must exist");
    assert!(approx(reused_global.position.x, -11.0));
    assert!(approx(reused_global.position.y, expected_y));
    assert!(approx(reused_global.position.z, 7.0));
}

#[test]
fn removed_node_cache_does_not_leak_to_reused_slot_2d() {
    let mut runtime = Runtime::new();

    let mut original = Node2D::new();
    original.transform.position = Vector2::new(8.0, 9.0);
    let original_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node2D(original)));
    runtime.mark_transform_dirty_recursive(original_id);
    let original_global = runtime
        .get_global_transform_2d(original_id)
        .expect("original global must exist");
    assert!(approx(original_global.position.x, 8.0));
    assert!(runtime.remove_node(original_id));

    let mut reused = None;
    for i in 0..2048 {
        let mut node = Node2D::new();
        node.transform.position = Vector2::new(-2.0, i as f32 * 2.0);
        let id = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::Node2D(node)));
        if id.index() == original_id.index() && id.generation() != original_id.generation() {
            reused = Some((id, i as f32 * 2.0));
            break;
        }
    }

    let (reused_id, expected_y) = reused.expect("slot must be reused with new generation");
    runtime.mark_transform_dirty_recursive(reused_id);
    let reused_global = runtime
        .get_global_transform_2d(reused_id)
        .expect("reused global must exist");
    assert!(approx(reused_global.position.x, -2.0));
    assert!(approx(reused_global.position.y, expected_y));
}

#[test]
fn recompute_overwrites_old_cached_global_transform_3d() {
    let mut runtime = Runtime::new();
    let mut node = Node3D::new();
    node.transform.position = Vector3::new(1.0, 2.0, 3.0);
    let id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(node)));

    runtime.mark_transform_dirty_recursive(id);
    let first = runtime
        .get_global_transform_3d(id)
        .expect("first global must exist");
    assert!(approx(first.position.x, 1.0));

    let _ = runtime.with_base_node_mut::<Node3D, _, _>(id, |node| {
        node.transform.position = Vector3::new(9.0, 8.0, 7.0);
    });
    runtime.mark_transform_dirty_recursive(id);

    let second = runtime
        .get_global_transform_3d(id)
        .expect("second global must exist");
    assert!(approx(second.position.x, 9.0));
    assert!(approx(second.position.y, 8.0));
    assert!(approx(second.position.z, 7.0));
}
