use super::Runtime;
use perro_ids::NodeID;
use perro_nodes::{Node2D, Node3D, SceneNodeData};
use perro_structs::{Color, NodeModulate};
use perro_ui::UiNode;

impl Runtime {
    #[inline]
    fn is_sub_view_data(data: &SceneNodeData) -> bool {
        matches!(
            data,
            SceneNodeData::UiSubView(_) | SceneNodeData::SubView2D(_) | SceneNodeData::SubView3D(_)
        )
    }

    fn refresh_world_membership(&self) {
        let revision = self.nodes.structural_revision();
        {
            let cache = self.world_membership.borrow();
            if cache.initialized && cache.revision == revision {
                return;
            }
        }

        let slot_count = self.nodes.slot_count();
        let mut cache = self.world_membership.borrow_mut();
        if cache.initialized && cache.revision == revision {
            return;
        }
        cache.owner_by_slot.clear();
        cache.owner_by_slot.resize(slot_count, NodeID::nil());
        cache.members.clear();

        let mut visited = vec![false; slot_count];
        let mut stack = Vec::with_capacity(self.nodes.len());
        for (id, node) in self.nodes.iter() {
            if node.parent.is_nil() || self.nodes.get(node.parent).is_none() {
                stack.push((id, NodeID::nil()));
            }
        }

        while let Some((id, owner)) = stack.pop() {
            let slot = id.index() as usize;
            if slot >= visited.len() || visited[slot] {
                continue;
            }
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            visited[slot] = true;
            cache.owner_by_slot[slot] = owner;
            cache.members.entry(owner).or_default().push(id);
            let child_owner = if Self::is_sub_view_data(&node.data) {
                id
            } else {
                owner
            };
            if let Some(children) = self.nodes.children(id) {
                stack.extend(
                    children
                        .iter()
                        .rev()
                        .copied()
                        .map(|child| (child, child_owner)),
                );
            }
        }

        // Corrupt/disconnected cycles fail into main world without hanging.
        for (id, _) in self.nodes.iter() {
            let slot = id.index() as usize;
            if slot < visited.len() && !visited[slot] {
                visited[slot] = true;
                cache.owner_by_slot[slot] = NodeID::nil();
                cache.members.entry(NodeID::nil()).or_default().push(id);
            }
        }
        cache.revision = revision;
        cache.initialized = true;
    }

    pub(crate) fn node_world(&self, node: NodeID) -> Option<NodeID> {
        self.nodes.get(node)?;
        self.refresh_world_membership();
        self.world_membership
            .borrow()
            .owner_by_slot
            .get(node.index() as usize)
            .copied()
    }

    pub(crate) fn fill_world_members(&self, world: NodeID, out: &mut Vec<NodeID>) {
        self.refresh_world_membership();
        out.clear();
        if let Some(members) = self.world_membership.borrow().members.get(&world) {
            out.extend_from_slice(members);
        }
    }

    pub(crate) fn node_local_visible(data: &SceneNodeData) -> bool {
        match data {
            SceneNodeData::Node => true,
            SceneNodeData::Node2D(node) => node.visible,
            SceneNodeData::Button2D(node) => node.visible,
            SceneNodeData::ImageButton2D(node) => node.visible,
            SceneNodeData::NineSliceButton2D(node) => node.visible,
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
            SceneNodeData::SubView2D(node) => node.visible,
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
            SceneNodeData::SubView3D(node) => node.visible,
            SceneNodeData::AmbientLight3D(node) => node.visible,
            SceneNodeData::Sky3D(node) => node.visible,
            SceneNodeData::RayLight3D(node) => node.visible,
            SceneNodeData::PointLight3D(node) => node.visible,
            SceneNodeData::SpotLight3D(node) => node.visible,
            SceneNodeData::ParticleEmitter3D(node) => node.visible,
            SceneNodeData::WaterBody3D(node) => node.base.visible,
            SceneNodeData::Decal3D(node) => node.base.visible,
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
            SceneNodeData::UiSubView(node) => node.visible,
            SceneNodeData::UiCameraStream(node) => node.visible,
            SceneNodeData::UiPanel(node) => node.visible,
            SceneNodeData::UiProgressBar(node) => node.visible,
            SceneNodeData::UiShape(node) => node.visible,
            SceneNodeData::UiButton(node) => node.visible,
            SceneNodeData::UiDropdown(node) => node.visible,
            SceneNodeData::UiCheckbox(node) => node.visible,
            SceneNodeData::UiColorPicker(node) => node.visible,
            SceneNodeData::UiImage(node) => node.visible,
            SceneNodeData::UiVideoPlayer(node) => node.visible,
            SceneNodeData::UiImageButton(node) => node.visible,
            SceneNodeData::UiNineSliceButton(node) => node.visible,
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

    pub(crate) fn sub_view_ancestor(&self, node: NodeID) -> Option<NodeID> {
        self.node_world(node).filter(|world| !world.is_nil())
    }

    pub(crate) fn is_under_sub_view(&self, node: NodeID) -> bool {
        self.sub_view_ancestor(node).is_some()
    }

    pub(crate) fn is_suspended_by_sub_view(&self, node: NodeID) -> bool {
        let Some(mut viewport_id) = self.sub_view_ancestor(node) else {
            return false;
        };
        let mut hops = 0usize;
        while !viewport_id.is_nil() && hops <= self.nodes.len() {
            let Some(viewport_node) = self.nodes.get(viewport_id) else {
                return true;
            };
            let suspended = matches!(
                &viewport_node.data,
                SceneNodeData::UiSubView(viewport)
                    if viewport.suspend_when_hidden
                        && (!viewport.enabled || !self.is_effectively_visible(viewport_id))
            ) || matches!(
                &viewport_node.data,
                SceneNodeData::SubView2D(viewport)
                    if viewport.sub_view.suspend_when_hidden
                        && (!viewport.sub_view.enabled
                            || !self.is_effectively_visible(viewport_id))
            ) || matches!(
                &viewport_node.data,
                SceneNodeData::SubView3D(viewport)
                    if viewport.sub_view.suspend_when_hidden
                        && (!viewport.sub_view.enabled
                            || !self.is_effectively_visible(viewport_id))
            );
            if suspended {
                return true;
            }
            viewport_id = self.node_world(viewport_id).unwrap_or(NodeID::nil());
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
        // color_modulate is a component-wise multiply: commutative + associative,
        // so the product does not depend on root->node ordering. Fold upward in a
        // single pass with no buffer. node contributes its own self_modulate;
        // strict ancestors contribute children_modulate. modulate applies to both.
        let mut acc = Color::WHITE;
        let mut current = node;
        let mut hops = 0usize;
        let max_hops = self.nodes.len().saturating_add(1);
        while hops < max_hops {
            let Some(scene_node) = self.nodes.get(current) else {
                break;
            };
            let parent = scene_node.parent;
            if let Some(local) = self.local_node_modulate(current) {
                let own = if current == node {
                    local.self_modulate
                } else {
                    local.children_modulate
                };
                acc = Self::color_modulate(Self::color_modulate(acc, local.modulate), own);
            }
            if parent.is_nil() {
                break;
            }
            current = parent;
            hops += 1;
        }
        acc
    }

    fn local_node_modulate(&self, node: NodeID) -> Option<NodeModulate> {
        let scene_node = self.nodes.get(node)?;
        scene_node
            .with_base_ref::<Node2D, _>(|node| node.modulate)
            .or_else(|| scene_node.with_base_ref::<Node3D, _>(|node| node.modulate))
            .or_else(|| scene_node.with_base_ref::<UiNode, _>(|node| node.modulate))
    }
}

#[cfg(test)]
mod world_membership_tests {
    use super::*;
    use perro_nodes::{Node3D, SubView3D};
    use perro_runtime_api::sub_apis::NodeAPI;

    #[test]
    fn nearest_sub_view_owner_tracks_nested_reparent() {
        let mut runtime = Runtime::new();
        let outer = NodeAPI::create::<SubView3D>(&mut runtime);
        let inner = NodeAPI::create::<SubView3D>(&mut runtime);
        let child = NodeAPI::create::<Node3D>(&mut runtime);

        assert_eq!(runtime.node_world(child), Some(NodeID::nil()));
        assert!(runtime.reparent(outer, inner));
        assert!(runtime.reparent(inner, child));
        assert_eq!(runtime.node_world(inner), Some(outer));
        assert_eq!(runtime.node_world(child), Some(inner));

        assert!(runtime.reparent(NodeID::nil(), child));
        assert_eq!(runtime.node_world(child), Some(NodeID::nil()));
    }

    #[test]
    fn hidden_outer_sub_view_suspends_nested_world() {
        let mut runtime = Runtime::new();
        let outer = NodeAPI::create::<SubView3D>(&mut runtime);
        let inner = NodeAPI::create::<SubView3D>(&mut runtime);
        let child = NodeAPI::create::<Node3D>(&mut runtime);
        assert!(runtime.reparent(outer, inner));
        assert!(runtime.reparent(inner, child));
        if let Some(mut node) = runtime.nodes.get_mut(outer)
            && let SceneNodeData::SubView3D(view) = &mut node.data
        {
            view.visible = false;
        }
        assert!(runtime.is_suspended_by_sub_view(child));
    }

    #[test]
    fn main_world_member_scan_excludes_large_sub_view_subtree() {
        let mut runtime = Runtime::new();
        let view = NodeAPI::create::<SubView3D>(&mut runtime);
        let mut children = Vec::new();
        for _ in 0..1_000 {
            let child = NodeAPI::create::<Node3D>(&mut runtime);
            assert!(runtime.reparent(view, child));
            children.push(child);
        }
        let mut main = Vec::new();
        runtime.fill_world_members(NodeID::nil(), &mut main);
        assert_eq!(main, vec![view]);
        let mut local = Vec::new();
        runtime.fill_world_members(view, &mut local);
        assert_eq!(local.len(), children.len());
    }
}
