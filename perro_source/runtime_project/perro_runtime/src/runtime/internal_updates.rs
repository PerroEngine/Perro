use super::Runtime;
use perro_ids::NodeID;
use perro_input_api::InputWindow;
use perro_nodes::{
    FixedUpdateHook, InternalFixedUpdate, InternalUpdate, NodeService, NodeType, UpdateHook,
};
use perro_resource_api::ResourceWindow;
use perro_runtime_api::RuntimeWindow;
use std::rc::Rc;

const NONE_POS: u32 = u32::MAX;

fn snapshot_dispatch_if_changed(
    live: &[NodeID],
    membership_epoch: u64,
    snapshot: &mut Rc<[NodeID]>,
    snapshot_epoch: &mut u64,
) -> bool {
    if *snapshot_epoch == membership_epoch {
        return false;
    }
    if snapshot.len() == live.len()
        && let Some(snapshot) = Rc::get_mut(snapshot)
    {
        snapshot.copy_from_slice(live);
    } else {
        *snapshot = Rc::from(live);
    }
    *snapshot_epoch = membership_epoch;
    true
}

impl Runtime {
    pub(crate) fn register_internal_node_schedules(&mut self, id: NodeID, ty: NodeType) {
        // node add ? shape/body chg -> physics query world stale
        self.invalidate_physics_query_sync();
        self.register_physics_body(id, ty);
        self.register_button_2d(id, ty);
        self.register_physics_joint(id, ty);
        self.register_internal_fixed_dispatch(id, ty);
        if matches!(ty.get_internal_update(), InternalUpdate::True) {
            let slot = id.index() as usize;
            if self.internal_updates.internal_update_pos.len() <= slot {
                self.internal_updates
                    .internal_update_pos
                    .resize(slot + 1, NONE_POS);
            }
            if self.internal_updates.internal_update_pos[slot] == NONE_POS {
                let pos = self.internal_updates.internal_update_nodes.len();
                self.internal_updates.internal_update_nodes.push(id);
                self.internal_updates.internal_update_pos[slot] = pos as u32;
                self.internal_updates.internal_update_membership_epoch = self
                    .internal_updates
                    .internal_update_membership_epoch
                    .wrapping_add(1);
            }
        }
        if matches!(ty.get_internal_fixed_update(), InternalFixedUpdate::True) {
            let slot = id.index() as usize;
            if self.internal_updates.internal_fixed_update_pos.len() <= slot {
                self.internal_updates
                    .internal_fixed_update_pos
                    .resize(slot + 1, NONE_POS);
            }
            if self.internal_updates.internal_fixed_update_pos[slot] == NONE_POS {
                let pos = self.internal_updates.internal_fixed_update_nodes.len();
                self.internal_updates.internal_fixed_update_nodes.push(id);
                self.internal_updates.internal_fixed_update_pos[slot] = pos as u32;
            }
        }
    }

    pub(crate) fn unregister_internal_node_schedules(&mut self, id: NodeID, ty: NodeType) {
        if matches!(
            ty.service(),
            NodeService::Body2D | NodeService::Body3D | NodeService::Joint2D | NodeService::Joint3D
        ) {
            self.invalidate_physics_query_sync();
        }

        self.unregister_physics_body(id);

        if matches!(ty.service(), NodeService::Button2D) {
            self.unregister_button_2d(id);
        }

        if ty.fixed_update_hook().is_node_callback() {
            let old_len = self.internal_updates.internal_fixed_dispatch_nodes.len();
            self.internal_updates
                .internal_fixed_dispatch_nodes
                .retain(|&node_id| node_id != id);
            if self.internal_updates.internal_fixed_dispatch_nodes.len() != old_len {
                self.internal_updates.internal_fixed_membership_epoch = self
                    .internal_updates
                    .internal_fixed_membership_epoch
                    .wrapping_add(1);
            }
        }

        match ty.service() {
            NodeService::Joint2D => {
                self.internal_updates
                    .physics_joint_nodes_2d
                    .retain(|&node_id| node_id != id);
            }
            NodeService::Joint3D => {
                self.internal_updates
                    .physics_joint_nodes_3d
                    .retain(|&node_id| node_id != id);
            }
            _ => {}
        }

        let slot = id.index() as usize;

        if let Some(&raw_pos) = self.internal_updates.internal_update_pos.get(slot)
            && raw_pos != NONE_POS
        {
            let pos = raw_pos as usize;
            let last_pos = self
                .internal_updates
                .internal_update_nodes
                .len()
                .saturating_sub(1);
            self.internal_updates.internal_update_nodes.swap_remove(pos);
            self.internal_updates.internal_update_pos[slot] = NONE_POS;
            self.internal_updates.internal_update_membership_epoch = self
                .internal_updates
                .internal_update_membership_epoch
                .wrapping_add(1);
            if pos <= last_pos.saturating_sub(1)
                && let Some(moved) = self
                    .internal_updates
                    .internal_update_nodes
                    .get(pos)
                    .copied()
            {
                let moved_slot = moved.index() as usize;
                if self.internal_updates.internal_update_pos.len() <= moved_slot {
                    self.internal_updates
                        .internal_update_pos
                        .resize(moved_slot + 1, NONE_POS);
                }
                self.internal_updates.internal_update_pos[moved_slot] = pos as u32;
            }
        }

        if let Some(&raw_pos) = self.internal_updates.internal_fixed_update_pos.get(slot)
            && raw_pos != NONE_POS
        {
            let pos = raw_pos as usize;
            let last_pos = self
                .internal_updates
                .internal_fixed_update_nodes
                .len()
                .saturating_sub(1);
            self.internal_updates
                .internal_fixed_update_nodes
                .swap_remove(pos);
            self.internal_updates.internal_fixed_update_pos[slot] = NONE_POS;
            if pos <= last_pos.saturating_sub(1)
                && let Some(moved) = self
                    .internal_updates
                    .internal_fixed_update_nodes
                    .get(pos)
                    .copied()
            {
                let moved_slot = moved.index() as usize;
                if self.internal_updates.internal_fixed_update_pos.len() <= moved_slot {
                    self.internal_updates
                        .internal_fixed_update_pos
                        .resize(moved_slot + 1, NONE_POS);
                }
                self.internal_updates.internal_fixed_update_pos[moved_slot] = pos as u32;
            }
        }
    }

    pub(crate) fn clear_internal_node_schedules(&mut self) {
        self.invalidate_physics_query_sync();
        self.internal_updates.internal_update_nodes.clear();
        self.internal_updates.internal_fixed_update_nodes.clear();
        self.internal_updates.internal_fixed_dispatch_nodes.clear();
        self.internal_updates.internal_update_membership_epoch = self
            .internal_updates
            .internal_update_membership_epoch
            .wrapping_add(1);
        self.internal_updates.internal_fixed_membership_epoch = self
            .internal_updates
            .internal_fixed_membership_epoch
            .wrapping_add(1);
        self.internal_updates.internal_update_pos.clear();
        self.internal_updates.internal_fixed_update_pos.clear();
        self.internal_updates.physics_body_nodes_2d.clear();
        self.internal_updates.physics_body_nodes_3d.clear();
        self.internal_updates.physics_joint_nodes_2d.clear();
        self.internal_updates.physics_joint_nodes_3d.clear();
        self.internal_updates.physics_body_pos_2d.clear();
        self.internal_updates.physics_body_pos_3d.clear();
        self.internal_updates.button_nodes_2d.clear();
        self.internal_updates.button_pos_2d.clear();
    }

    fn register_internal_fixed_dispatch(&mut self, id: NodeID, ty: NodeType) {
        if !ty.fixed_update_hook().is_node_callback() {
            return;
        }
        if !self
            .internal_updates
            .internal_fixed_dispatch_nodes
            .contains(&id)
        {
            self.internal_updates.internal_fixed_dispatch_nodes.push(id);
            self.internal_updates.internal_fixed_membership_epoch = self
                .internal_updates
                .internal_fixed_membership_epoch
                .wrapping_add(1);
        }
    }

    fn register_physics_joint(&mut self, id: NodeID, ty: NodeType) {
        let nodes = match ty.service() {
            NodeService::Joint2D => &mut self.internal_updates.physics_joint_nodes_2d,
            NodeService::Joint3D => &mut self.internal_updates.physics_joint_nodes_3d,
            _ => return,
        };
        if !nodes.contains(&id) {
            nodes.push(id);
        }
    }

    fn register_physics_body(&mut self, id: NodeID, ty: NodeType) {
        match ty.service() {
            NodeService::Body2D => {
                let slot = id.index() as usize;
                if self.internal_updates.physics_body_pos_2d.len() <= slot {
                    self.internal_updates
                        .physics_body_pos_2d
                        .resize(slot + 1, NONE_POS);
                }
                if self.internal_updates.physics_body_pos_2d[slot] == NONE_POS {
                    let pos = self.internal_updates.physics_body_nodes_2d.len();
                    self.internal_updates.physics_body_nodes_2d.push(id);
                    self.internal_updates.physics_body_pos_2d[slot] = pos as u32;
                }
            }
            NodeService::Body3D => {
                let slot = id.index() as usize;
                if self.internal_updates.physics_body_pos_3d.len() <= slot {
                    self.internal_updates
                        .physics_body_pos_3d
                        .resize(slot + 1, NONE_POS);
                }
                if self.internal_updates.physics_body_pos_3d[slot] == NONE_POS {
                    let pos = self.internal_updates.physics_body_nodes_3d.len();
                    self.internal_updates.physics_body_nodes_3d.push(id);
                    self.internal_updates.physics_body_pos_3d[slot] = pos as u32;
                }
            }
            _ => {}
        }
    }

    fn register_button_2d(&mut self, id: NodeID, ty: NodeType) {
        if !matches!(ty.service(), NodeService::Button2D) {
            return;
        }
        let slot = id.index() as usize;
        if self.internal_updates.button_pos_2d.len() <= slot {
            self.internal_updates
                .button_pos_2d
                .resize(slot + 1, NONE_POS);
        }
        if self.internal_updates.button_pos_2d[slot] == NONE_POS {
            let pos = self.internal_updates.button_nodes_2d.len();
            self.internal_updates.button_nodes_2d.push(id);
            self.internal_updates.button_pos_2d[slot] = pos as u32;
        }
    }

    fn unregister_button_2d(&mut self, id: NodeID) {
        let slot = id.index() as usize;
        if let Some(&raw_pos) = self.internal_updates.button_pos_2d.get(slot)
            && raw_pos != NONE_POS
        {
            let pos = raw_pos as usize;
            self.internal_updates.button_nodes_2d.swap_remove(pos);
            self.internal_updates.button_pos_2d[slot] = NONE_POS;
            if let Some(moved) = self.internal_updates.button_nodes_2d.get(pos).copied() {
                let moved_slot = moved.index() as usize;
                if self.internal_updates.button_pos_2d.len() <= moved_slot {
                    self.internal_updates
                        .button_pos_2d
                        .resize(moved_slot + 1, NONE_POS);
                }
                self.internal_updates.button_pos_2d[moved_slot] = pos as u32;
            }
        }
    }

    fn unregister_physics_body(&mut self, id: NodeID) {
        let slot = id.index() as usize;

        if let Some(&raw_pos) = self.internal_updates.physics_body_pos_2d.get(slot)
            && raw_pos != NONE_POS
        {
            let pos = raw_pos as usize;
            let last_pos = self
                .internal_updates
                .physics_body_nodes_2d
                .len()
                .saturating_sub(1);
            self.internal_updates.physics_body_nodes_2d.swap_remove(pos);
            self.internal_updates.physics_body_pos_2d[slot] = NONE_POS;
            if pos <= last_pos.saturating_sub(1)
                && let Some(moved) = self
                    .internal_updates
                    .physics_body_nodes_2d
                    .get(pos)
                    .copied()
            {
                let moved_slot = moved.index() as usize;
                if self.internal_updates.physics_body_pos_2d.len() <= moved_slot {
                    self.internal_updates
                        .physics_body_pos_2d
                        .resize(moved_slot + 1, NONE_POS);
                }
                self.internal_updates.physics_body_pos_2d[moved_slot] = pos as u32;
            }
        }

        if let Some(&raw_pos) = self.internal_updates.physics_body_pos_3d.get(slot)
            && raw_pos != NONE_POS
        {
            let pos = raw_pos as usize;
            let last_pos = self
                .internal_updates
                .physics_body_nodes_3d
                .len()
                .saturating_sub(1);
            self.internal_updates.physics_body_nodes_3d.swap_remove(pos);
            self.internal_updates.physics_body_pos_3d[slot] = NONE_POS;
            if pos <= last_pos.saturating_sub(1)
                && let Some(moved) = self
                    .internal_updates
                    .physics_body_nodes_3d
                    .get(pos)
                    .copied()
            {
                let moved_slot = moved.index() as usize;
                if self.internal_updates.physics_body_pos_3d.len() <= moved_slot {
                    self.internal_updates
                        .physics_body_pos_3d
                        .resize(moved_slot + 1, NONE_POS);
                }
                self.internal_updates.physics_body_pos_3d[moved_slot] = pos as u32;
            }
        }
    }

    pub(crate) fn run_internal_update_schedule(&mut self) {
        let membership_epoch = self.internal_updates.internal_update_membership_epoch;
        if snapshot_dispatch_if_changed(
            &self.internal_updates.internal_update_nodes,
            membership_epoch,
            &mut self.internal_updates.internal_update_dispatch_scratch,
            &mut self.internal_updates.internal_update_dispatch_epoch,
        ) {
            #[cfg(any(test, feature = "bench", feature = "profile"))]
            {
                self.internal_updates.internal_update_snapshot_copies += 1;
            }
        }
        let dispatch = Rc::clone(&self.internal_updates.internal_update_dispatch_scratch);
        if dispatch.is_empty() {
            return;
        }
        let resource_api = self.resource_api.clone();
        let res = ResourceWindow::new(resource_api.as_ref());
        let input_ptr = std::ptr::addr_of!(self.input);
        // SAFETY: During callback dispatch, input is treated as immutable runtime state.
        // Engine invariant: only window/event ingestion mutates input, outside script callback execution.
        let ipt = unsafe { InputWindow::new(&*input_ptr) };
        for id in dispatch.iter().copied() {
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            let hook = node.node_type().update_hook();
            if self.is_suspended_by_sub_view(id) {
                continue;
            }
            self.call_internal_update_node_with_context(id, hook, &res, &ipt);
        }
    }

    pub(crate) fn run_internal_fixed_update_schedule(&mut self) {
        let membership_epoch = self.internal_updates.internal_fixed_membership_epoch;
        if snapshot_dispatch_if_changed(
            &self.internal_updates.internal_fixed_dispatch_nodes,
            membership_epoch,
            &mut self.internal_updates.internal_fixed_dispatch_scratch,
            &mut self.internal_updates.internal_fixed_dispatch_epoch,
        ) {
            #[cfg(any(test, feature = "bench", feature = "profile"))]
            {
                self.internal_updates.internal_fixed_snapshot_copies += 1;
            }
        }
        let dispatch = Rc::clone(&self.internal_updates.internal_fixed_dispatch_scratch);
        if dispatch.is_empty() {
            return;
        }
        for id in dispatch.iter().copied() {
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            let hook = node.node_type().fixed_update_hook();
            if self.is_suspended_by_sub_view(id) {
                continue;
            }
            self.call_internal_fixed_update_node_with_context(id, hook);
        }
    }

    #[cfg(feature = "bench")]
    pub fn bench_internal_schedule_snapshot_copies(&self) -> (u64, u64) {
        (
            self.internal_updates.internal_update_snapshot_copies,
            self.internal_updates.internal_fixed_snapshot_copies,
        )
    }

    #[cfg(feature = "bench")]
    pub fn bench_reset_internal_schedule_snapshot_copies(&mut self) {
        self.internal_updates.internal_update_snapshot_copies = 0;
        self.internal_updates.internal_fixed_snapshot_copies = 0;
    }

    fn call_internal_update_node_with_context(
        &mut self,
        id: NodeID,
        hook: UpdateHook,
        res: &ResourceWindow<'_, crate::RuntimeResourceApi>,
        ipt: &InputWindow<'_, perro_input_api::InputSnapshot>,
    ) {
        // Caller validates the generational ID before entering this callback.
        self.active_runtime_nodes.push(id);
        let mut ctx = RuntimeWindow::new(self);
        perro_internal_updates::dispatch_update(hook, &mut ctx, res, ipt, id);
        let _ = self.active_runtime_nodes.pop();
    }

    fn call_internal_fixed_update_node_with_context(&mut self, id: NodeID, hook: FixedUpdateHook) {
        // Caller validates the generational ID before entering this callback.
        self.active_runtime_nodes.push(id);
        let mut ctx = RuntimeWindow::new(self);
        perro_internal_updates::dispatch_fixed_update(hook, &mut ctx, id);
        let _ = self.active_runtime_nodes.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_bone_batch_keeps_name_index_masks_and_missing_targets() {
        use perro_nodes::{AnimationTree, Bone3D, Skeleton3D};
        use perro_resource_api::sub_apis::{AnimationAPI, AnimationTreeAPI};
        use perro_runtime_api::sub_apis::NodeAPI;
        use perro_structs::{Transform3D, Vector3};
        let mut runtime = Runtime::new();
        let skeleton = NodeAPI::create::<Skeleton3D>(&mut runtime);
        let tree = NodeAPI::create::<AnimationTree>(&mut runtime);
        let rest = Transform3D {
            position: Vector3::new(3.0, 4.0, 5.0),
            scale: Vector3::new(2.0, 2.0, 2.0),
            ..Transform3D::IDENTITY
        };
        let _ = runtime.with_node_mut::<Skeleton3D, _, _>(skeleton, |node| {
            node.bones = vec![
                Bone3D {
                    name: "Head".into(),
                    rest,
                    pose: rest,
                    ..Bone3D::new()
                },
                Bone3D::new(),
            ];
        });
        let clip = runtime.resource_api.create_animation_from_bytes(
            br#"
[Animation]
name="Batch"
fps=60
[/Animation]
[Objects]
Rig=Skeleton3D
[/Objects]
[Frame0]
@Rig {
    bone["Head"].position=(9,8,7)
    bone[1].scale=(3,4,5)
    bone[99].position=(1,1,1)
}
[/Frame0]
"#,
        );
        let asset = runtime.resource_api.create_animation_tree_from_bytes(
            br#"
[AnimationTree]
name="BatchTree"
[/AnimationTree]
[AnimationSlots]
Base
[/AnimationSlots]
[Output]
input=@Base
[/Output]
"#,
        );
        assert!(!clip.is_nil() && !asset.is_nil());
        let _ = runtime.with_node_mut::<AnimationTree, _, _>(tree, |node| {
            node.set_tree(asset);
            node.set_clip_by_index(0, clip);
            node.set_slot_binding(0, "Rig", skeleton);
        });
        for _ in 0..3 {
            runtime.update(1.0 / 60.0);
            runtime
                .with_node::<Skeleton3D, _>(skeleton, |node| {
                    assert_eq!(node.bones[0].pose.position, Vector3::new(9.0, 8.0, 7.0));
                    assert_eq!(node.bones[0].pose.scale, rest.scale);
                    assert_eq!(node.bones[0].rest, rest);
                    assert_eq!(node.bones[1].pose.scale, Vector3::new(3.0, 4.0, 5.0));
                    assert_eq!(node.bones[1].pose.position, Vector3::ZERO);
                })
                .expect("skeleton remains live");
        }
    }

    #[test]
    fn dispatch_snapshot_keeps_same_pass_order_when_callback_changes_membership() {
        let first = NodeID::new(1);
        let removed = NodeID::new(2);
        let last = NodeID::new(3);
        let added = NodeID::new(4);
        let mut live = vec![first, removed, last];
        let mut snapshot = Rc::from(Vec::<NodeID>::new());
        let mut membership_epoch = 1;
        let mut snapshot_epoch = 0;
        assert!(snapshot_dispatch_if_changed(
            &live,
            membership_epoch,
            &mut snapshot,
            &mut snapshot_epoch,
        ));

        let dispatch_len = snapshot.len();
        let mut seen = Vec::new();
        for index in 0..dispatch_len {
            let id = snapshot[index];
            seen.push(id);
            if id == first {
                live.swap_remove(1);
                live.push(added);
                membership_epoch += 1;
            }
        }

        assert_eq!(seen, [first, removed, last]);
        assert_eq!(live, [first, last, added]);
        assert!(snapshot_dispatch_if_changed(
            &live,
            membership_epoch,
            &mut snapshot,
            &mut snapshot_epoch,
        ));
        assert_eq!(snapshot.as_ref(), live);
    }

    #[test]
    fn dispatch_snapshot_skips_copy_until_membership_epoch_changes() {
        let live = vec![NodeID::new(1), NodeID::new(2)];
        let mut snapshot = Rc::from(Vec::<NodeID>::new());
        let mut snapshot_epoch = 0;

        assert!(snapshot_dispatch_if_changed(
            &live,
            1,
            &mut snapshot,
            &mut snapshot_epoch,
        ));
        let ptr = snapshot.as_ptr();
        assert!(!snapshot_dispatch_if_changed(
            &live,
            1,
            &mut snapshot,
            &mut snapshot_epoch,
        ));
        assert_eq!(snapshot.as_ptr(), ptr);
    }

    #[test]
    fn dispatch_snapshot_reuses_unique_equal_length_allocation() {
        let first = [NodeID::new(1), NodeID::new(2)];
        let second = [NodeID::new(3), NodeID::new(4)];
        let mut snapshot = Rc::from(Vec::<NodeID>::new());
        let mut snapshot_epoch = 0;
        assert!(snapshot_dispatch_if_changed(
            &first,
            1,
            &mut snapshot,
            &mut snapshot_epoch,
        ));
        let ptr = snapshot.as_ptr();

        assert!(snapshot_dispatch_if_changed(
            &second,
            2,
            &mut snapshot,
            &mut snapshot_epoch,
        ));

        assert_eq!(snapshot.as_ptr(), ptr);
        assert_eq!(snapshot.as_ref(), second);
    }

    #[test]
    fn nested_pass_membership_changes_keep_outer_snapshot_frozen() {
        let first = NodeID::new(1);
        let removed = NodeID::new(2);
        let added = NodeID::new(3);
        let mut live = vec![first, removed];
        let mut cached = Rc::from(Vec::<NodeID>::new());
        let mut snapshot_epoch = 0;
        assert!(snapshot_dispatch_if_changed(
            &live,
            1,
            &mut cached,
            &mut snapshot_epoch,
        ));
        let outer = Rc::clone(&cached);

        live.clear();
        live.push(added);
        assert!(snapshot_dispatch_if_changed(
            &live,
            2,
            &mut cached,
            &mut snapshot_epoch,
        ));
        let nested = Rc::clone(&cached);

        assert_eq!(outer.as_ref(), [first, removed]);
        assert_eq!(nested.as_ref(), [added]);

        live.clear();
        assert!(snapshot_dispatch_if_changed(
            &live,
            3,
            &mut cached,
            &mut snapshot_epoch,
        ));
        assert!(cached.is_empty());
        assert_eq!(outer.as_ref(), [first, removed]);
        assert_eq!(nested.as_ref(), [added]);
    }

    #[test]
    fn update_and_fixed_snapshots_copy_once_per_membership_epoch() {
        let mut runtime = Runtime::new();
        let update = NodeID::new(1);
        let fixed = NodeID::new(2);
        runtime.register_internal_node_schedules(update, NodeType::AnimatedSprite2D);
        runtime.register_internal_node_schedules(fixed, NodeType::PhysicsBoneChain2D);

        runtime.run_internal_update_schedule();
        runtime.run_internal_fixed_update_schedule();
        runtime.run_internal_update_schedule();
        runtime.run_internal_fixed_update_schedule();

        assert_eq!(runtime.internal_updates.internal_update_snapshot_copies, 1);
        assert_eq!(runtime.internal_updates.internal_fixed_snapshot_copies, 1);

        runtime.register_internal_node_schedules(update, NodeType::AnimatedSprite2D);
        runtime.register_internal_node_schedules(fixed, NodeType::PhysicsBoneChain2D);
        runtime.run_internal_update_schedule();
        runtime.run_internal_fixed_update_schedule();

        assert_eq!(runtime.internal_updates.internal_update_snapshot_copies, 1);
        assert_eq!(runtime.internal_updates.internal_fixed_snapshot_copies, 1);

        runtime.unregister_internal_node_schedules(update, NodeType::AnimatedSprite2D);
        runtime.unregister_internal_node_schedules(fixed, NodeType::PhysicsBoneChain2D);
        runtime.run_internal_update_schedule();
        runtime.run_internal_fixed_update_schedule();

        assert_eq!(runtime.internal_updates.internal_update_snapshot_copies, 2);
        assert_eq!(runtime.internal_updates.internal_fixed_snapshot_copies, 2);
    }

    #[test]
    fn clear_and_reused_ids_refresh_both_snapshots() {
        let mut runtime = Runtime::new();
        let first_update = NodeID::from_parts(7, 1);
        let first_fixed = NodeID::from_parts(8, 1);
        runtime.register_internal_node_schedules(first_update, NodeType::AnimatedSprite2D);
        runtime.register_internal_node_schedules(first_fixed, NodeType::PhysicsBoneChain2D);
        runtime.run_internal_update_schedule();
        runtime.run_internal_fixed_update_schedule();

        runtime.clear_internal_node_schedules();
        runtime.run_internal_update_schedule();
        runtime.run_internal_fixed_update_schedule();
        assert!(
            runtime
                .internal_updates
                .internal_update_dispatch_scratch
                .is_empty()
        );
        assert!(
            runtime
                .internal_updates
                .internal_fixed_dispatch_scratch
                .is_empty()
        );

        let reused_update = NodeID::from_parts(7, 2);
        let reused_fixed = NodeID::from_parts(8, 2);
        runtime.register_internal_node_schedules(reused_update, NodeType::AnimatedSprite2D);
        runtime.register_internal_node_schedules(reused_fixed, NodeType::PhysicsBoneChain2D);
        runtime.run_internal_update_schedule();
        runtime.run_internal_fixed_update_schedule();

        assert_eq!(
            runtime
                .internal_updates
                .internal_update_dispatch_scratch
                .as_ref(),
            [reused_update]
        );
        assert_eq!(
            runtime
                .internal_updates
                .internal_fixed_dispatch_scratch
                .as_ref(),
            [reused_fixed]
        );
    }
}
