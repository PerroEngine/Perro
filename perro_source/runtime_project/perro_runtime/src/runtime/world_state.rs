use super::Runtime;
use perro_ids::NodeID;
use perro_nodes::SceneNodeData;

impl Runtime {
    pub(crate) fn node_local_visible(data: &SceneNodeData) -> bool {
        match data {
            SceneNodeData::Node => true,
            SceneNodeData::Node2D(node) => node.visible,
            SceneNodeData::Sprite2D(node) => node.visible,
            SceneNodeData::Camera2D(node) => node.visible,
            SceneNodeData::CollisionShape2D(node) => node.visible,
            SceneNodeData::StaticBody2D(node) => node.visible,
            SceneNodeData::Area2D(node) => node.visible,
            SceneNodeData::RigidBody2D(node) => node.visible,
            SceneNodeData::Node3D(node) => node.visible,
            SceneNodeData::MeshInstance3D(node) => node.visible,
            SceneNodeData::MultiMeshInstance3D(node) => node.visible,
            SceneNodeData::CollisionShape3D(node) => node.visible,
            SceneNodeData::StaticBody3D(node) => node.visible,
            SceneNodeData::Area3D(node) => node.visible,
            SceneNodeData::RigidBody3D(node) => node.visible,
            SceneNodeData::Camera3D(node) => node.visible,
            SceneNodeData::AmbientLight3D(node) => node.visible,
            SceneNodeData::Sky3D(node) => node.visible,
            SceneNodeData::RayLight3D(node) => node.visible,
            SceneNodeData::PointLight3D(node) => node.visible,
            SceneNodeData::SpotLight3D(node) => node.visible,
            SceneNodeData::ParticleEmitter3D(node) => node.visible,
            SceneNodeData::Skeleton3D(node) => node.visible,
            SceneNodeData::UiBox(node) => node.visible,
            SceneNodeData::UiPanel(node) => node.visible,
            SceneNodeData::UiButton(node) => node.visible,
            SceneNodeData::UiLabel(node) => node.visible,
            SceneNodeData::UiLayout(node) => node.visible,
            SceneNodeData::UiHLayout(node) => node.visible,
            SceneNodeData::UiVLayout(node) => node.visible,
            SceneNodeData::UiGrid(node) => node.visible,
            SceneNodeData::AnimationPlayer(_) => true,
        }
    }

    pub(crate) fn is_effectively_visible(&self, node: NodeID) -> bool {
        if node.is_nil() {
            return false;
        }
        let mut current = node;
        let mut hops = 0usize;
        let max_hops = self.nodes.len().saturating_add(1);
        while hops < max_hops {
            let Some(scene_node) = self.nodes.get(current) else {
                return false;
            };
            if !Self::node_local_visible(&scene_node.data) {
                return false;
            }
            if scene_node.parent.is_nil() {
                return true;
            }
            current = scene_node.parent;
            hops += 1;
        }
        false
    }
}
