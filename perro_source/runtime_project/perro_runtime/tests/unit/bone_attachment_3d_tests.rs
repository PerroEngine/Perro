use perro_ids::NodeID;
use perro_nodes::{Bone3D, BoneAttachment3D, Node3D, SceneNode, SceneNodeData, Skeleton3D};
use perro_runtime::runtime::Runtime;
use perro_structs::{Transform3D, Vector3};

#[test]
fn bone_attachment_create_and_setters() {
    let mut node = BoneAttachment3D::new();
    let skeleton = NodeID::from_u64(99);
    node.set_skeleton(Some(skeleton));
    node.set_bone_index(2);
    node.set_enabled(false);
    assert_eq!(node.skeleton(), Some(skeleton));
    assert_eq!(node.bone_index(), 2);
    assert!(!node.enabled());
}

#[test]
fn bone_attachment_runtime_updates_transform() {
    let mut runtime = Runtime::new();

    let mut skeleton = Skeleton3D::new();
    skeleton.bones.push(Bone3D { rest: Transform3D { position: Vector3::new(3.0, 4.0, 5.0), ..Transform3D::IDENTITY }, ..Bone3D::new()});
    let skeleton_id = runtime.nodes.insert(SceneNode::new(SceneNodeData::Skeleton3D(skeleton)));

    let mut attachment = BoneAttachment3D::new();
    attachment.set_skeleton(Some(skeleton_id));
    attachment.set_bone_index(0);
    let attachment_id = runtime.nodes.insert(SceneNode::new(SceneNodeData::BoneAttachment3D(attachment)));

    runtime.update_bone_attachments();

    let t = runtime.with_base_node::<Node3D, _, _>(attachment_id, |node| node.transform).unwrap();
    assert_eq!(t.position, Vector3::new(3.0, 4.0, 5.0));
}

#[test]
fn bone_attachment_invalid_index_or_disabled_safe() {
    let mut runtime = Runtime::new();
    let skeleton_id = runtime.nodes.insert(SceneNode::new(SceneNodeData::Skeleton3D(Skeleton3D::new())));
    let mut attachment = BoneAttachment3D::new();
    attachment.set_skeleton(Some(skeleton_id));
    attachment.set_bone_index(100);
    let attachment_id = runtime.nodes.insert(SceneNode::new(SceneNodeData::BoneAttachment3D(attachment.clone())));
    runtime.update_bone_attachments();

    let mut disabled = attachment;
    disabled.set_enabled(false);
    let _disabled_id = runtime.nodes.insert(SceneNode::new(SceneNodeData::BoneAttachment3D(disabled)));
    runtime.update_bone_attachments();

    assert!(runtime.nodes.get(attachment_id).is_some());
}
