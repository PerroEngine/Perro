use super::Runtime;
use ahash::AHashMap;
use perro_ids::NodeID;
use perro_nodes::{Node2D, Node3D, SceneNodeData};
use perro_render_bridge::{
    CameraStreamDraw3DState, CameraStreamState, Light2DState, PointParticles2DState,
    PointParticles3DState, SkeletonPalette, Sprite2DCommand, Water2DState, Water3DState,
    empty_arc_slice,
};
use perro_structs::{Color, NodeModulate, PostProcessEffect};
use perro_ui::UiNode;
use std::sync::Arc;

/// Last emitted lane `Arc`s 4 one stream/sub-view node.
///
/// A stream refresh rebuilds each lane into scratch; when the rebuilt slice
/// equals the retained one, the retained `Arc` is handed back so downstream
/// `Arc::ptr_eq` fast paths hit and the graphics-side upsert compare collapses
/// to a pointer check instead of two deep walks of the whole state.
pub(crate) struct StreamRetainedLanes {
    pub(crate) post_processing: Arc<[PostProcessEffect]>,
    pub(crate) sprites_2d: Arc<[Sprite2DCommand]>,
    pub(crate) lights_2d: Arc<[Light2DState]>,
    pub(crate) point_particles_2d: Arc<[(NodeID, PointParticles2DState)]>,
    pub(crate) waters_2d: Arc<[(NodeID, Water2DState)]>,
    pub(crate) draws_3d: Arc<[CameraStreamDraw3DState]>,
    pub(crate) point_particles_3d: Arc<[(NodeID, PointParticles3DState)]>,
    pub(crate) waters_3d: Arc<[(NodeID, Water3DState)]>,
}

impl Default for StreamRetainedLanes {
    fn default() -> Self {
        Self {
            post_processing: empty_arc_slice(),
            sprites_2d: empty_arc_slice(),
            lights_2d: empty_arc_slice(),
            point_particles_2d: empty_arc_slice(),
            waters_2d: empty_arc_slice(),
            draws_3d: empty_arc_slice(),
            point_particles_3d: empty_arc_slice(),
            waters_3d: empty_arc_slice(),
        }
    }
}

/// Bucketed auto-resolution 4 one sub-view node (logic + consts live in
/// `render::bridge::stream_state`).
///
/// A UI size animation moves a rect by sub-pixel steps every frame; an exact
/// auto resolution turned each of those into a full target-chain recreate (up
/// to 4 textures + external bindings + tonemap bind-cache drop). The long axis
/// snaps UP to a bucket (short axis follows the aspect), and a smaller size
/// only lands aft a hold, so a wobbling rect keeps one target.
#[derive(Default)]
pub(crate) struct AutoResolutionState {
    /// last emitted target size (0 = none yet).
    pub(crate) emitted: [u32; 2],
    /// consecutive refreshes whose wanted size stayed under `emitted`.
    pub(crate) shrink_streak: u32,
}

/// Cross-refresh retention 4 camera-stream / sub-view extraction output.
///
/// Value-based: retention never decides WHETHER a stream refreshes (the
/// dirty-world set stays the only trigger) — only whether a refresh hands back
/// the previously emitted `Arc` instead of allocating an equal fresh one.
/// Entries drop with their node in `note_removed_render_node`.
#[derive(Default)]
pub(crate) struct StreamRetention {
    /// per stream/sub-view node: last emitted lane `Arc`s.
    pub(crate) lanes: AHashMap<NodeID, StreamRetainedLanes>,
    /// per stream/sub-view node: last upserted whole-state `Arc`. An equal
    /// rebuild re-sends this exact `Arc` so the gpu-side upsert hits
    /// `Arc::ptr_eq` and skips its deep compare + re-render mark.
    pub(crate) states: AHashMap<NodeID, Arc<CameraStreamState>>,
    /// per skeleton node: (node change stamp @ build, palette). The palette
    /// reads only the `Skeleton3D` node's own data, so the arena's per-node
    /// change stamp is an exact invalidation signal: same stamp = same bones =
    /// same palette, reuse w/o rebuild (mirrors the main pass, which only
    /// rebuilds palettes 4 traversed = dirty nodes).
    pub(crate) skeleton_palettes: AHashMap<NodeID, (u64, SkeletonPalette)>,
    /// per sub-view node w/ an auto (0) resolution axis: bucket + shrink-hold
    /// state so size animations stop recreating the stream target chain.
    pub(crate) auto_resolutions: AHashMap<NodeID, AutoResolutionState>,
}

impl StreamRetention {
    pub(crate) fn note_removed_node(&mut self, node: NodeID) {
        self.lanes.remove(&node);
        self.states.remove(&node);
        self.skeleton_palettes.remove(&node);
        self.auto_resolutions.remove(&node);
    }
}

/// Per-epoch memo for the O(depth) ancestor walks in
/// [`Runtime::is_effectively_visible`] and
/// [`Runtime::effective_self_modulate`].
///
/// Slot-indexed stamp arrays: a slot whose stamp equals `current` holds a
/// valid value for this epoch. The epoch is the arena `mutation_revision`
/// (every visibility / modulate / structural change bumps it, so the memo can
/// never serve a stale ancestor chain); within one render/extraction pass the
/// revision is stable, which is where the repeated walks happen.
#[derive(Default)]
pub(crate) struct VisibilityModulateMemo {
    revision: u64,
    initialized: bool,
    current: u32,
    vis_stamp: Vec<u32>,
    /// generation of the id the stamp was written for; a hit requires both the
    /// stamp and the generation to match so stale ids can't read a reused
    /// slot's value.
    vis_generation: Vec<u32>,
    vis_value: Vec<u8>,
    modulate_stamp: Vec<u32>,
    modulate_generation: Vec<u32>,
    /// memoized fold of `modulate * children_modulate` from this node up.
    modulate_value: Vec<Color>,
    vis_chain_scratch: Vec<(NodeID, bool)>,
    modulate_chain_scratch: Vec<(NodeID, Color)>,
}

impl VisibilityModulateMemo {
    fn refresh(&mut self, revision: u64, slot_count: usize) {
        if !self.initialized || self.revision != revision {
            self.initialized = true;
            self.revision = revision;
            self.current = self.current.wrapping_add(1);
            if self.current == 0 {
                // wrapped: reset stamps so stale 0 entries don't read as hits.
                self.vis_stamp.iter_mut().for_each(|s| *s = 0);
                self.modulate_stamp.iter_mut().for_each(|s| *s = 0);
                self.current = 1;
            }
        }
        if self.vis_stamp.len() < slot_count {
            self.vis_stamp.resize(slot_count, 0);
            self.vis_generation.resize(slot_count, 0);
            self.vis_value.resize(slot_count, 0);
        }
        if self.modulate_stamp.len() < slot_count {
            self.modulate_stamp.resize(slot_count, 0);
            self.modulate_generation.resize(slot_count, 0);
            self.modulate_value.resize(slot_count, Color::WHITE);
        }
    }

    #[inline]
    fn vis_hit(&self, id: NodeID) -> Option<bool> {
        let index = id.index() as usize;
        if index < self.vis_stamp.len()
            && self.vis_stamp[index] == self.current
            && self.vis_generation[index] == id.generation()
        {
            Some(self.vis_value[index] != 0)
        } else {
            None
        }
    }

    #[inline]
    fn modulate_hit(&self, id: NodeID) -> Option<Color> {
        let index = id.index() as usize;
        if index < self.modulate_stamp.len()
            && self.modulate_stamp[index] == self.current
            && self.modulate_generation[index] == id.generation()
        {
            Some(self.modulate_value[index])
        } else {
            None
        }
    }
}

impl Runtime {
    #[inline]
    fn is_sub_view_data(data: &SceneNodeData) -> bool {
        matches!(
            data,
            SceneNodeData::UiSubView(_) | SceneNodeData::SubView2D(_) | SceneNodeData::SubView3D(_)
        )
    }

    #[inline]
    fn is_stream_node_data(data: &SceneNodeData) -> bool {
        matches!(
            data,
            SceneNodeData::UiSubView(_)
                | SceneNodeData::SubView2D(_)
                | SceneNodeData::SubView3D(_)
                | SceneNodeData::UiCameraStream(_)
                | SceneNodeData::CameraStream2D(_)
                | SceneNodeData::CameraStream3D(_)
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
        cache.members_shared.clear();
        cache.stream_nodes.clear();
        cache.sub_view_count = 0;

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
            if Self::is_stream_node_data(&node.data) {
                cache.stream_nodes.push(id);
            }
            let child_owner = if Self::is_sub_view_data(&node.data) {
                cache.sub_view_count += 1;
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
        for (id, node) in self.nodes.iter() {
            let slot = id.index() as usize;
            if slot < visited.len() && !visited[slot] {
                visited[slot] = true;
                cache.owner_by_slot[slot] = NodeID::nil();
                cache.members.entry(NodeID::nil()).or_default().push(id);
                if Self::is_stream_node_data(&node.data) {
                    cache.stream_nodes.push(id);
                }
                // count here too: over-count only disables the fast path.
                if Self::is_sub_view_data(&node.data) {
                    cache.sub_view_count += 1;
                }
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

    /// Shared view of a world's direct member list. Built once per world per
    /// structural revision; later reads are a refcount clone, so per-frame
    /// callers can iterate while mutating `self` without copying the list.
    pub(crate) fn world_members_arc(&self, world: NodeID) -> std::sync::Arc<[NodeID]> {
        self.refresh_world_membership();
        let mut cache = self.world_membership.borrow_mut();
        if let Some(shared) = cache.members_shared.get(&world) {
            return shared.clone();
        }
        let shared: std::sync::Arc<[NodeID]> = match cache.members.get(&world) {
            Some(members) if !members.is_empty() => std::sync::Arc::from(members.as_slice()),
            _ => perro_render_bridge::empty_arc_slice(),
        };
        cache.members_shared.insert(world, shared.clone());
        shared
    }

    /// Every stream/sub-view node in the arena (UiCameraStream, UiSubView,
    /// CameraStream2D/3D, SubView2D/3D), any world.
    pub(crate) fn fill_stream_nodes(&self, out: &mut Vec<NodeID>) {
        self.refresh_world_membership();
        out.clear();
        out.extend_from_slice(&self.world_membership.borrow().stream_nodes);
    }

    /// Worlds holding >=1 dirty node this pass, plus each dirty sub-view
    /// world's owner chain (the owner re-collects to re-upsert nested stream
    /// state). `NodeID::nil()` (main world) enters only from its own dirty
    /// members: interior-only sub-view churn never rebuilds main-world
    /// watchers, since nested content reaches them through a stable texture
    /// id. Call after transform-dirty propagation.
    pub(crate) fn collect_dirty_worlds(&self, out: &mut ahash::AHashSet<NodeID>) {
        out.clear();
        for &raw_index in self.dirty.dirty_indices() {
            let Some((id, _)) = self.nodes.slot_get(raw_index as usize) else {
                continue;
            };
            let Some(mut world) = self.node_world(id) else {
                continue;
            };
            while out.insert(world) && !world.is_nil() {
                match self.node_world(world) {
                    Some(owner) if !owner.is_nil() => world = owner,
                    _ => break,
                }
            }
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
        // memoized ancestor walk: per-frame loops query this per node, so the
        // naive O(depth) walk per query is O(n * depth) per pass. Walk up only
        // until a slot already stamped this epoch, then fold results back down
        // and stamp the whole chain.
        let mut memo = self.vis_memo.borrow_mut();
        let memo = &mut *memo;
        memo.refresh(self.nodes.mutation_revision(), self.nodes.slot_count());
        if let Some(hit) = memo.vis_hit(node) {
            return hit;
        }

        let mut chain = std::mem::take(&mut memo.vis_chain_scratch);
        chain.clear();
        let mut current = node;
        let mut hops = 0usize;
        let max_hops = self.nodes.len().saturating_add(1);
        let base = loop {
            if hops >= max_hops {
                // cycle: fail closed like the legacy walk, skip memo writes so
                // a corrupt chain can't stamp bogus values.
                chain.clear();
                memo.vis_chain_scratch = chain;
                return false;
            }
            if let Some(hit) = memo.vis_hit(current) {
                break hit;
            }
            let Some(scene_node) = self.nodes.get(current) else {
                // missing / stale ancestor: everything below it is invisible.
                break false;
            };
            chain.push((current, Self::node_local_visible(&scene_node.data)));
            if scene_node.parent.is_nil() {
                break true;
            }
            current = scene_node.parent;
            hops += 1;
        };

        let stamp = memo.current;
        let mut acc = base;
        for &(id, local_visible) in chain.iter().rev() {
            acc = acc && local_visible;
            let index = id.index() as usize;
            memo.vis_stamp[index] = stamp;
            memo.vis_generation[index] = id.generation();
            memo.vis_value[index] = acc as u8;
        }
        let result = if chain.is_empty() { base } else { acc };
        chain.clear();
        memo.vis_chain_scratch = chain;
        result
    }

    /// True when the arena holds >=1 SubView2D/3D / UiSubView node. Scenes w/o
    /// sub-views (the common case) skip the owner lookup on every dispatch.
    fn has_sub_views(&self) -> bool {
        self.nodes.has_sub_views()
    }

    pub(crate) fn sub_view_ancestor(&self, node: NodeID) -> Option<NodeID> {
        if !self.has_sub_views() {
            return None;
        }
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
        // so the product does not depend on root->node ordering. node contributes
        // `modulate * self_modulate`; strict ancestors contribute
        // `modulate * children_modulate` — that ancestor fold is memoized per
        // slot in `ancestor_children_modulate`.
        let Some(scene_node) = self.nodes.get(node) else {
            return Color::WHITE;
        };
        let own = match self.local_node_modulate(node) {
            Some(local) => Self::color_modulate(local.modulate, local.self_modulate),
            None => Color::WHITE,
        };
        let ancestors = self.ancestor_children_modulate(scene_node.parent);
        Self::color_modulate(ancestors, own)
    }

    /// Memoized fold of `modulate * children_modulate` from `node` up through
    /// its ancestors (inclusive). `WHITE` for nil / missing nodes, matching the
    /// legacy walk that stopped contributing at a broken parent link.
    fn ancestor_children_modulate(&self, node: NodeID) -> Color {
        if node.is_nil() {
            return Color::WHITE;
        }
        let mut memo = self.vis_memo.borrow_mut();
        let memo = &mut *memo;
        memo.refresh(self.nodes.mutation_revision(), self.nodes.slot_count());
        if let Some(hit) = memo.modulate_hit(node) {
            return hit;
        }

        let mut chain = std::mem::take(&mut memo.modulate_chain_scratch);
        chain.clear();
        let mut current = node;
        let mut hops = 0usize;
        let max_hops = self.nodes.len().saturating_add(1);
        let base = loop {
            if hops >= max_hops {
                // cycle: stop contributing like the legacy bounded walk, skip
                // memo writes so a corrupt chain can't stamp bogus values.
                chain.clear();
                memo.modulate_chain_scratch = chain;
                return Color::WHITE;
            }
            if let Some(hit) = memo.modulate_hit(current) {
                break hit;
            }
            let Some(scene_node) = self.nodes.get(current) else {
                break Color::WHITE;
            };
            let contribution = match self.local_node_modulate(current) {
                Some(local) => Self::color_modulate(local.modulate, local.children_modulate),
                None => Color::WHITE,
            };
            chain.push((current, contribution));
            if scene_node.parent.is_nil() {
                break Color::WHITE;
            }
            current = scene_node.parent;
            hops += 1;
        };

        let stamp = memo.current;
        let mut acc = base;
        for &(id, contribution) in chain.iter().rev() {
            acc = Self::color_modulate(acc, contribution);
            let index = id.index() as usize;
            memo.modulate_stamp[index] = stamp;
            memo.modulate_generation[index] = id.generation();
            memo.modulate_value[index] = acc;
        }
        let result = if chain.is_empty() { base } else { acc };
        chain.clear();
        memo.modulate_chain_scratch = chain;
        result
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
    use perro_runtime_api::sub_apis::{NodeAPI, NodeSpec};

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

    #[test]
    fn flat_creates_do_not_rebuild_world_membership() {
        let mut runtime = Runtime::new();
        assert_eq!(runtime.node_world(NodeID::nil()), None);
        runtime.refresh_world_membership();
        let cached_revision = runtime.world_membership.borrow().revision;

        for _ in 0..1_000 {
            let _ = NodeAPI::create::<Node3D>(&mut runtime);
        }

        assert_eq!(runtime.world_membership.borrow().revision, cached_revision);
        assert!(!runtime.nodes.has_sub_views());
        runtime.refresh_world_membership();
        assert_eq!(
            runtime.world_membership.borrow().revision,
            runtime.nodes.structural_revision()
        );
    }

    #[test]
    fn sub_view_presence_tracks_remove_and_slot_reuse() {
        let mut runtime = Runtime::new();
        let view = NodeAPI::create::<SubView3D>(&mut runtime);
        assert!(runtime.nodes.has_sub_views());
        assert!(NodeAPI::remove_node(&mut runtime, view));
        assert!(!runtime.nodes.has_sub_views());

        let _ = NodeAPI::create::<Node3D>(&mut runtime);
        assert!(!runtime.nodes.has_sub_views());
    }

    #[test]
    fn batch_build_preserves_nested_sub_view_ownership() {
        let mut runtime = Runtime::new();
        let specs = [
            NodeSpec::new(SubView3D::default()),
            NodeSpec::new(SubView3D::default()).parent(Some(0)),
            NodeSpec::new(Node3D::new()).parent(Some(1)),
        ];
        let ids = NodeAPI::create_nodes(&mut runtime, &specs, NodeID::nil());

        assert_eq!(runtime.node_world(ids[0]), Some(NodeID::nil()));
        assert_eq!(runtime.node_world(ids[1]), Some(ids[0]));
        assert_eq!(runtime.node_world(ids[2]), Some(ids[1]));
        assert!(runtime.dirty.is_node_dirty(ids[0]));
        assert!(runtime.dirty.is_node_dirty(ids[1]));
        assert!(runtime.dirty.is_node_dirty(ids[2]));
    }
}
