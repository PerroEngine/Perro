use super::Runtime;
use perro_ids::NodeID;
use perro_input_api::InputWindow;
use perro_nodes::{InternalFixedUpdate, InternalUpdate, NodeType};
use perro_resource_api::ResourceWindow;
use perro_runtime_api::RuntimeWindow;

const NONE_POS: u32 = u32::MAX;

fn snapshot_dispatch(live: &[NodeID], scratch: &mut Vec<NodeID>) {
    scratch.clear();
    scratch.extend_from_slice(live);
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
        match ty {
            NodeType::StaticBody2D
            | NodeType::Area2D
            | NodeType::RigidBody2D
            | NodeType::CharacterBody2D
            | NodeType::WaterBody2D
            | NodeType::TileMap2D
            | NodeType::StaticBody3D
            | NodeType::Area3D
            | NodeType::RigidBody3D
            | NodeType::CharacterBody3D
            | NodeType::WaterBody3D
            | NodeType::PinJoint2D
            | NodeType::DistanceJoint2D
            | NodeType::FixedJoint2D
            | NodeType::BallJoint3D
            | NodeType::HingeJoint3D
            | NodeType::FixedJoint3D => self.invalidate_physics_query_sync(),
            _ => {}
        }

        self.unregister_physics_body(id);

        if matches!(ty, NodeType::Button2D | NodeType::ImageButton2D) {
            self.unregister_button_2d(id);
        }

        if matches!(
            ty,
            NodeType::PhysicsBoneChain2D | NodeType::PhysicsBoneChain3D
        ) {
            self.internal_updates
                .internal_fixed_dispatch_nodes
                .retain(|&node_id| node_id != id);
        }

        match ty {
            NodeType::PinJoint2D | NodeType::DistanceJoint2D | NodeType::FixedJoint2D => {
                self.internal_updates
                    .physics_joint_nodes_2d
                    .retain(|&node_id| node_id != id);
            }
            NodeType::BallJoint3D | NodeType::HingeJoint3D | NodeType::FixedJoint3D => {
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
        if !matches!(
            ty,
            NodeType::PhysicsBoneChain2D | NodeType::PhysicsBoneChain3D
        ) {
            return;
        }
        if !self
            .internal_updates
            .internal_fixed_dispatch_nodes
            .contains(&id)
        {
            self.internal_updates.internal_fixed_dispatch_nodes.push(id);
        }
    }

    fn register_physics_joint(&mut self, id: NodeID, ty: NodeType) {
        let nodes = match ty {
            NodeType::PinJoint2D | NodeType::DistanceJoint2D | NodeType::FixedJoint2D => {
                &mut self.internal_updates.physics_joint_nodes_2d
            }
            NodeType::BallJoint3D | NodeType::HingeJoint3D | NodeType::FixedJoint3D => {
                &mut self.internal_updates.physics_joint_nodes_3d
            }
            _ => return,
        };
        if !nodes.contains(&id) {
            nodes.push(id);
        }
    }

    fn register_physics_body(&mut self, id: NodeID, ty: NodeType) {
        match ty {
            NodeType::StaticBody2D
            | NodeType::Area2D
            | NodeType::RigidBody2D
            | NodeType::CharacterBody2D
            | NodeType::WaterBody2D
            | NodeType::TileMap2D => {
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
            NodeType::StaticBody3D
            | NodeType::Area3D
            | NodeType::RigidBody3D
            | NodeType::CharacterBody3D
            | NodeType::WaterBody3D => {
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
        if !matches!(ty, NodeType::Button2D | NodeType::ImageButton2D) {
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
        if self.internal_updates.internal_update_nodes.is_empty() {
            return;
        }
        let resource_api = self.resource_api.clone();
        let res = ResourceWindow::new(resource_api.as_ref());
        let input_ptr = std::ptr::addr_of!(self.input);
        // SAFETY: During callback dispatch, input is treated as immutable runtime state.
        // Engine invariant: only window/event ingestion mutates input, outside script callback execution.
        let ipt = unsafe { InputWindow::new(&*input_ptr) };
        let mut dispatch =
            std::mem::take(&mut self.internal_updates.internal_update_dispatch_scratch);
        snapshot_dispatch(&self.internal_updates.internal_update_nodes, &mut dispatch);
        for id in dispatch.iter().copied() {
            if self.nodes.get(id).is_none() || self.is_suspended_by_ui_viewport(id) {
                continue;
            }
            self.call_internal_update_node_with_context(id, &res, &ipt);
        }
        dispatch.clear();
        self.internal_updates.internal_update_dispatch_scratch = dispatch;
    }

    pub(crate) fn run_internal_fixed_update_schedule(&mut self) {
        if self
            .internal_updates
            .internal_fixed_dispatch_nodes
            .is_empty()
        {
            return;
        }
        let resource_api = self.resource_api.clone();
        let res = ResourceWindow::new(resource_api.as_ref());
        let input_ptr = std::ptr::addr_of!(self.input);
        // SAFETY: During callback dispatch, input is treated as immutable runtime state.
        // Engine invariant: only window/event ingestion mutates input, outside script callback execution.
        let ipt = unsafe { InputWindow::new(&*input_ptr) };
        let mut dispatch =
            std::mem::take(&mut self.internal_updates.internal_fixed_dispatch_scratch);
        snapshot_dispatch(
            &self.internal_updates.internal_fixed_dispatch_nodes,
            &mut dispatch,
        );
        for id in dispatch.iter().copied() {
            if self.nodes.get(id).is_none() || self.is_suspended_by_ui_viewport(id) {
                continue;
            }
            self.call_internal_fixed_update_node_with_context(id, &res, &ipt);
        }
        dispatch.clear();
        self.internal_updates.internal_fixed_dispatch_scratch = dispatch;
    }

    fn call_internal_update_node_with_context(
        &mut self,
        id: NodeID,
        res: &ResourceWindow<'_, crate::RuntimeResourceApi>,
        ipt: &InputWindow<'_, perro_input_api::InputSnapshot>,
    ) {
        if self.nodes.get(id).is_none() {
            return;
        }
        let mut ctx = RuntimeWindow::new(self);
        perro_internal_updates::internal_update_node(&mut ctx, res, ipt, id);
    }

    fn call_internal_fixed_update_node_with_context(
        &mut self,
        id: NodeID,
        res: &ResourceWindow<'_, crate::RuntimeResourceApi>,
        ipt: &InputWindow<'_, perro_input_api::InputSnapshot>,
    ) {
        if self.nodes.get(id).is_none() {
            return;
        }
        let mut ctx = RuntimeWindow::new(self);
        perro_internal_updates::internal_fixed_update_node(&mut ctx, res, ipt, id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_snapshot_keeps_order_when_live_schedule_shrinks() {
        let first = NodeID::new(1);
        let removed = NodeID::new(2);
        let last = NodeID::new(3);
        let mut live = vec![first, removed, last];
        let mut snapshot = Vec::new();
        snapshot_dispatch(&live, &mut snapshot);

        live.swap_remove(1);

        assert_eq!(snapshot, [first, removed, last]);
        assert_eq!(live, [first, last]);
    }
}
