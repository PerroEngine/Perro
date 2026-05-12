use crate::Runtime;
use perro_ids::tags;
use perro_nodes::{Bone3D, BoneAttachment3D, Node2D, Node3D, SceneNode, SceneNodeData, Skeleton3D};
use perro_runtime_api::sub_apis::{NodeAPI, NodeCreationTemplate};
use perro_structs::{Quaternion, Transform2D, Transform3D, Vector2, Vector3};

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() <= 1e-4
}

#[test]
fn create_nodes_batches_parent_names_and_tags() {
    let mut runtime = Runtime::new();
    let parent_id = runtime.create::<Node2D>();
    let requests = [
        NodeCreationTemplate::new::<Node2D>()
            .name("EnemyA")
            .tags(tags!["enemy"]),
        NodeCreationTemplate::new::<Node2D>()
            .name("EnemyB")
            .tags(tags!["enemy"]),
    ];
    let ids = runtime.create_nodes(&requests, parent_id);

    assert_eq!(ids.len(), 2);
    assert_eq!(runtime.get_node_children_ids(parent_id), Some(ids.clone()));
    assert_eq!(runtime.get_node_parent_id(ids[0]), Some(parent_id));
    assert_eq!(runtime.get_node_name(ids[0]).as_deref(), Some("EnemyA"));
    assert_eq!(
        runtime.get_node_tags(ids[1]).as_deref(),
        Some([std::borrow::Cow::Borrowed("enemy")].as_slice())
    );
}

#[test]
fn create_nodes_supports_root_requests_without_metadata() {
    let mut runtime = Runtime::new();
    let ids = runtime.create_nodes(
        &[
            NodeCreationTemplate::new::<Node2D>(),
            NodeCreationTemplate::new::<Node2D>().name("RootOnly"),
        ],
        perro_ids::NodeID::nil(),
    );

    assert_eq!(ids.len(), 2);
    assert_eq!(
        runtime.get_node_parent_id(ids[0]),
        Some(perro_ids::NodeID::nil())
    );
    assert_eq!(runtime.get_node_name(ids[0]).as_deref(), Some("Node"));
    assert_eq!(runtime.get_node_name(ids[1]).as_deref(), Some("RootOnly"));
    assert_eq!(runtime.get_node_tags(ids[0]), Some(Vec::new()));
}

#[test]
fn create_nodes_handles_10k_children_and_transform_propagation() {
    let mut runtime = Runtime::new();
    let parent_id = runtime.create::<Node2D>();
    runtime
        .with_node_mut::<Node2D, _, _>(parent_id, |parent| {
            parent.transform.position = Vector2::new(12.0, 34.0);
        })
        .expect("parent exists");

    let templates = vec![NodeCreationTemplate::new::<Node2D>(); 10_000];
    let ids = runtime.create_nodes(&templates, parent_id);

    assert_eq!(ids.len(), 10_000);
    assert_eq!(runtime.nodes.len(), 10_001);
    assert_eq!(
        runtime
            .get_node_children_ids(parent_id)
            .map(|ids| ids.len()),
        Some(10_000)
    );

    runtime.propagate_pending_transform_dirty();
    runtime.refresh_dirty_global_transforms();

    let first_global = runtime
        .get_global_transform_2d(ids[0])
        .expect("first child global");
    let last_global = runtime
        .get_global_transform_2d(ids[9_999])
        .expect("last child global");
    assert_eq!(first_global.position, Vector2::new(12.0, 34.0));
    assert_eq!(last_global.position, Vector2::new(12.0, 34.0));
}

#[test]
fn skeleton_bone_lookup_helpers_return_name_and_index() {
    let mut runtime = Runtime::new();

    let mut skeleton = Skeleton3D::new();
    skeleton.bones = vec![
        Bone3D {
            name: "Root".into(),
            ..Bone3D::new()
        },
        Bone3D {
            name: "Spine".into(),
            ..Bone3D::new()
        },
    ];
    let skeleton_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Skeleton3D(skeleton)));
    let node_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));

    let name = runtime.get_skeleton_bone_name(skeleton_id, 1);
    assert_eq!(name.as_deref(), Some("Spine"));
    assert_eq!(
        runtime.get_skeleton_bone_index(skeleton_id, "Root"),
        Some(0)
    );
    assert_eq!(runtime.get_skeleton_bone_name(skeleton_id, 99), None);
    assert_eq!(
        runtime.get_skeleton_bone_index(skeleton_id, "Missing"),
        None
    );
    assert_eq!(runtime.get_skeleton_bone_name(node_id, 0), None);
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
fn bone_attachment_3d_follows_skeleton_bone_global_transform() {
    let mut runtime = Runtime::new();

    let mut skeleton = Skeleton3D::new();
    skeleton.transform.position = Vector3::new(10.0, 0.0, 0.0);
    skeleton.bones = vec![
        Bone3D {
            rest: Transform3D::new(
                Vector3::new(0.0, 2.0, 0.0),
                Quaternion::IDENTITY,
                Vector3::ONE,
            ),
            pose: Transform3D::new(
                Vector3::new(0.0, 2.0, 0.0),
                Quaternion::IDENTITY,
                Vector3::ONE,
            ),
            ..Bone3D::new()
        },
        Bone3D {
            parent: 0,
            rest: Transform3D::new(
                Vector3::new(0.0, 0.0, 3.0),
                Quaternion::IDENTITY,
                Vector3::ONE,
            ),
            pose: Transform3D::new(
                Vector3::new(0.0, 0.0, 3.0),
                Quaternion::IDENTITY,
                Vector3::ONE,
            ),
            ..Bone3D::new()
        },
    ];
    let skeleton_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Skeleton3D(skeleton)));
    runtime.register_internal_node_schedules(
        skeleton_id,
        runtime.nodes.get(skeleton_id).unwrap().node_type(),
    );

    let mut attachment = BoneAttachment3D::new();
    attachment.skeleton = skeleton_id;
    attachment.bone_index = 1;
    let attachment_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::BoneAttachment3D(attachment)));
    runtime.register_internal_node_schedules(
        attachment_id,
        runtime.nodes.get(attachment_id).unwrap().node_type(),
    );
    runtime.mark_transform_dirty_recursive(skeleton_id);
    runtime.mark_transform_dirty_recursive(attachment_id);

    runtime.update(1.0 / 60.0);

    let global = runtime
        .get_global_transform_3d(attachment_id)
        .expect("attachment global must exist");
    assert!(approx(global.position.x, 10.0));
    assert!(approx(global.position.y, 2.0));
    assert!(approx(global.position.z, 3.0));
}

#[test]
fn bone_attachment_3d_child_follows_bone_global_transform() {
    let mut runtime = Runtime::new();

    let mut skeleton = Skeleton3D::new();
    skeleton.transform.position = Vector3::new(10.0, 0.0, 0.0);
    skeleton.bones = vec![Bone3D {
        rest: Transform3D::new(
            Vector3::new(0.0, 2.0, 0.0),
            Quaternion::IDENTITY,
            Vector3::ONE,
        ),
        pose: Transform3D::new(
            Vector3::new(0.0, 2.0, 0.0),
            Quaternion::IDENTITY,
            Vector3::ONE,
        ),
        ..Bone3D::new()
    }];
    let skeleton_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Skeleton3D(skeleton)));
    runtime.register_internal_node_schedules(
        skeleton_id,
        runtime.nodes.get(skeleton_id).unwrap().node_type(),
    );

    let mut attachment = BoneAttachment3D::new();
    attachment.skeleton = skeleton_id;
    attachment.bone_index = 0;
    let attachment_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::BoneAttachment3D(attachment)));
    runtime.register_internal_node_schedules(
        attachment_id,
        runtime.nodes.get(attachment_id).unwrap().node_type(),
    );

    let mut child = Node3D::new();
    child.transform.position = Vector3::new(0.0, 0.0, 5.0);
    let child_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(child)));
    if let Some(attachment_node) = runtime.nodes.get_mut(attachment_id) {
        attachment_node.add_child(child_id);
    }
    if let Some(child_node) = runtime.nodes.get_mut(child_id) {
        child_node.parent = attachment_id;
    }
    runtime.mark_transform_dirty_recursive(skeleton_id);
    runtime.mark_transform_dirty_recursive(attachment_id);

    runtime.update(1.0 / 60.0);

    let child_global = runtime
        .get_global_transform_3d(child_id)
        .expect("child global must exist");
    assert!(approx(child_global.position.x, 10.0));
    assert!(approx(child_global.position.y, 2.0));
    assert!(approx(child_global.position.z, 5.0));

    let _ = runtime.with_base_node_mut::<Skeleton3D, _, _>(skeleton_id, |skeleton| {
        skeleton.bones[0].pose.position = Vector3::new(0.0, 4.0, 0.0);
    });
    runtime.update(1.0 / 60.0);

    let child_global = runtime
        .get_global_transform_3d(child_id)
        .expect("child global must exist after bone move");
    assert!(approx(child_global.position.x, 10.0));
    assert!(approx(child_global.position.y, 4.0));
    assert!(approx(child_global.position.z, 5.0));
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
