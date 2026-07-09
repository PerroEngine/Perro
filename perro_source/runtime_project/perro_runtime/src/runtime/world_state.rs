use super::Runtime;
use perro_ids::NodeID;
use perro_nodes::{Node2D, Node3D, SceneNodeData};
use perro_structs::{Color, NodeModulate};
use perro_ui::UiNode;

impl Runtime {
    pub(crate) fn node_local_visible(data: &SceneNodeData) -> bool {
        match data {
            SceneNodeData::Node => true,
            SceneNodeData::Node2D(node) => node.visible,
            SceneNodeData::Button2D(node) => node.visible,
            SceneNodeData::ImageButton2D(node) => node.visible,
            SceneNodeData::Sprite2D(node) => node.visible,
            SceneNodeData::Label2D(node) => node.visible,
            SceneNodeData::NineSlice2D(node) => node.visible,
            SceneNodeData::AnimatedSprite2D(node) => node.visible,
            SceneNodeData::VideoPlayer2D(node) => node.visible,
            SceneNodeData::ParticleEmitter2D(node) => node.visible,
            SceneNodeData::WaterBody2D(node) => node.base.visible,
            SceneNodeData::AmbientLight2D(node) => node.visible,
            SceneNodeData::RayLight2D(node) => node.visible,
            SceneNodeData::PointLight2D(node) => node.visible,
            SceneNodeData::SpotLight2D(node) => node.visible,
            SceneNodeData::TileMap2D(node) => node.visible,
            SceneNodeData::Skeleton2D(node) => node.visible,
            SceneNodeData::BoneAttachment2D(node) => node.visible,
            SceneNodeData::IKTarget2D(node) => node.visible,
            SceneNodeData::PhysicsBoneChain2D(node) => node.visible,
            SceneNodeData::BoneCollider2D(node) => node.visible,
            SceneNodeData::Camera2D(node) => node.visible,
            SceneNodeData::CameraStream2D(node) => node.visible,
            SceneNodeData::CollisionShape2D(node) => node.visible,
            SceneNodeData::StaticBody2D(node) => node.visible,
            SceneNodeData::Area2D(node) => node.visible,
            SceneNodeData::RigidBody2D(node) => node.visible,
            SceneNodeData::CharacterBody2D(node) => node.visible,
            SceneNodeData::PhysicsForceEmitter2D(node) => node.visible,
            SceneNodeData::PinJoint2D(node) => node.visible,
            SceneNodeData::DistanceJoint2D(node) => node.visible,
            SceneNodeData::FixedJoint2D(node) => node.visible,
            SceneNodeData::AudioMask2D(node) => node.visible,
            SceneNodeData::AudioEffectZone2D(node) => node.visible,
            SceneNodeData::AudioPortal2D(node) => node.visible,
            SceneNodeData::Node3D(node) => node.visible,
            SceneNodeData::MeshInstance3D(node) => node.visible,
            SceneNodeData::MultiMeshInstance3D(node) => node.visible,
            SceneNodeData::CollisionShape3D(node) => node.visible,
            SceneNodeData::StaticBody3D(node) => node.visible,
            SceneNodeData::Area3D(node) => node.visible,
            SceneNodeData::RigidBody3D(node) => node.visible,
            SceneNodeData::CharacterBody3D(node) => node.visible,
            SceneNodeData::PhysicsForceEmitter3D(node) => node.visible,
            SceneNodeData::BallJoint3D(node) => node.visible,
            SceneNodeData::HingeJoint3D(node) => node.visible,
            SceneNodeData::FixedJoint3D(node) => node.visible,
            SceneNodeData::Camera3D(node) => node.visible,
            SceneNodeData::CameraStream3D(node) => node.visible,
            SceneNodeData::AmbientLight3D(node) => node.visible,
            SceneNodeData::Sky3D(node) => node.visible,
            SceneNodeData::RayLight3D(node) => node.visible,
            SceneNodeData::PointLight3D(node) => node.visible,
            SceneNodeData::SpotLight3D(node) => node.visible,
            SceneNodeData::ParticleEmitter3D(node) => node.visible,
            SceneNodeData::WaterBody3D(node) => node.base.visible,
            SceneNodeData::Decal3D(node) => node.base.visible,
            SceneNodeData::TextDecal3D(node) => node.base.visible,
            SceneNodeData::Sprite3D(node) => node.visible,
            SceneNodeData::VideoPlayer3D(node) => node.visible,
            SceneNodeData::Label3D(node) => node.visible,
            SceneNodeData::Skeleton3D(node) => node.visible,
            SceneNodeData::BoneAttachment3D(node) => node.visible,
            SceneNodeData::IKTarget3D(node) => node.visible,
            SceneNodeData::PhysicsBoneChain3D(node) => node.visible,
            SceneNodeData::BoneCollider3D(node) => node.visible,
            SceneNodeData::AudioMask3D(node) => node.visible,
            SceneNodeData::AudioEffectZone3D(node) => node.visible,
            SceneNodeData::AudioPortal3D(node) => node.visible,
            SceneNodeData::UiNode(node) => node.visible,
            SceneNodeData::UiCameraStream(node) => node.visible,
            SceneNodeData::UiPanel(node) => node.visible,
            SceneNodeData::UiShape(node) => node.visible,
            SceneNodeData::UiButton(node) => node.visible,
            SceneNodeData::UiDropdown(node) => node.visible,
            SceneNodeData::UiCheckbox(node) => node.visible,
            SceneNodeData::UiColorPicker(node) => node.visible,
            SceneNodeData::UiImage(node) => node.visible,
            SceneNodeData::UiVideoPlayer(node) => node.visible,
            SceneNodeData::UiImageButton(node) => node.visible,
            SceneNodeData::UiNineSlice(node) => node.visible,
            SceneNodeData::UiAnimatedImage(node) => node.visible,
            SceneNodeData::UiLabel(node) => node.visible,
            SceneNodeData::UiTextBox(node) => node.inner.base.visible,
            SceneNodeData::UiTextBlock(node) => node.inner.base.visible,
            SceneNodeData::UiScrollContainer(node) => node.visible,
            SceneNodeData::UiLayout(node) => node.visible,
            SceneNodeData::UiHLayout(node) => node.visible,
            SceneNodeData::UiVLayout(node) => node.visible,
            SceneNodeData::UiGrid(node) => node.visible,
            SceneNodeData::UiTreeList(node) => node.visible,
            SceneNodeData::AnimationPlayer(_) => true,
            SceneNodeData::AnimationTree(_) => true,
            SceneNodeData::Webcam(node) => node.enabled,
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

    pub(crate) fn color_modulate(a: Color, b: Color) -> Color {
        if a == Color::WHITE {
            return b;
        }
        if b == Color::WHITE {
            return a;
        }
        Color::from_rgba([a.r() * b.r(), a.g() * b.g(), a.b() * b.b(), a.a() * b.a()])
    }

    pub(crate) fn color_modulate_rgba(color: [f32; 4], modulate: Color) -> [f32; 4] {
        if modulate == Color::WHITE {
            return color;
        }
        [
            color[0] * modulate.r(),
            color[1] * modulate.g(),
            color[2] * modulate.b(),
            color[3] * modulate.a(),
        ]
    }

    pub(crate) fn color_modulate_rgb(color: Color, modulate: Color) -> [f32; 3] {
        if modulate == Color::WHITE {
            return color.to_rgb();
        }
        [
            color.r() * modulate.r(),
            color.g() * modulate.g(),
            color.b() * modulate.b(),
        ]
    }

    pub(crate) fn effective_self_modulate(&self, node: NodeID) -> Color {
        if node.is_nil() {
            return Color::WHITE;
        }
        let mut chain = Vec::new();
        let mut current = node;
        let mut hops = 0usize;
        let max_hops = self.nodes.len().saturating_add(1);
        while hops < max_hops {
            let Some(scene_node) = self.nodes.get(current) else {
                break;
            };
            chain.push(current);
            if scene_node.parent.is_nil() {
                break;
            }
            current = scene_node.parent;
            hops += 1;
        }

        let mut inherited = Color::WHITE;
        for id in chain.iter().rev().copied() {
            let Some(local) = self.local_node_modulate(id) else {
                continue;
            };
            if id == node {
                return Self::color_modulate(
                    Self::color_modulate(inherited, local.modulate),
                    local.self_modulate,
                );
            }
            inherited = Self::color_modulate(
                Self::color_modulate(inherited, local.modulate),
                local.children_modulate,
            );
        }
        inherited
    }

    fn local_node_modulate(&self, node: NodeID) -> Option<NodeModulate> {
        let scene_node = self.nodes.get(node)?;
        scene_node
            .with_base_ref::<Node2D, _>(|node| node.modulate)
            .or_else(|| scene_node.with_base_ref::<Node3D, _>(|node| node.modulate))
            .or_else(|| scene_node.with_base_ref::<UiNode, _>(|node| node.modulate))
    }
}
