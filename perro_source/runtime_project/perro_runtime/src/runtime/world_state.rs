use super::Runtime;
use perro_ids::NodeID;
use perro_nodes::SceneNodeData;
use perro_terrain::{ChunkCoord, TerrainData};

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
            SceneNodeData::CollisionShape3D(node) => node.visible,
            SceneNodeData::StaticBody3D(node) => node.visible,
            SceneNodeData::Area3D(node) => node.visible,
            SceneNodeData::RigidBody3D(node) => node.visible,
            SceneNodeData::TerrainInstance3D(node) => node.visible,
            SceneNodeData::Camera3D(node) => node.visible,
            SceneNodeData::AmbientLight3D(node) => node.visible,
            SceneNodeData::Sky3D(node) => node.visible,
            SceneNodeData::RayLight3D(node) => node.visible,
            SceneNodeData::PointLight3D(node) => node.visible,
            SceneNodeData::SpotLight3D(node) => node.visible,
            SceneNodeData::ParticleEmitter3D(node) => node.visible,
            SceneNodeData::Skeleton3D(node) => node.visible,
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

    pub(crate) fn default_terrain_data() -> TerrainData {
        let mut terrain = TerrainData::new(64.0);
        let _ = terrain.ensure_chunk(ChunkCoord::new(0, 0));
        terrain
    }

    pub(crate) fn ensure_terrain_instance_data(&mut self, node: NodeID) -> bool {
        let Some(current_id) = self
            .nodes
            .get(node)
            .and_then(|scene_node| match &scene_node.data {
                SceneNodeData::TerrainInstance3D(terrain) => Some(terrain.terrain),
                _ => None,
            })
        else {
            return false;
        };

        if !current_id.is_nil() {
            let store = self
                .terrain_store
                .lock()
                .expect("terrain store mutex poisoned");
            if store.get(current_id).is_some() {
                return true;
            }
        }

        let id = self
            .terrain_store
            .lock()
            .expect("terrain store mutex poisoned")
            .insert(Self::default_terrain_data());
        if let Some(scene_node) = self.nodes.get_mut(node)
            && let SceneNodeData::TerrainInstance3D(terrain) = &mut scene_node.data
        {
            terrain.terrain = id;
            return true;
        }

        false
    }
}
