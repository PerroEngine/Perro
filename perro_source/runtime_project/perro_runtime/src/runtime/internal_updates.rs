use super::Runtime;
use perro_ids::NodeID;
use perro_input::InputWindow;
use perro_nodes::{InternalFixedUpdate, InternalUpdate, NodeType};
use perro_resource_context::ResourceWindow;
use perro_runtime_context::RuntimeWindow;

const NONE_POS: u32 = u32::MAX;

impl Runtime {
    pub(crate) fn rebuild_internal_node_schedules(&mut self) {
        self.internal_updates.internal_update_nodes.clear();
        self.internal_updates.internal_fixed_update_nodes.clear();
        self.internal_updates.internal_update_pos.clear();
        self.internal_updates.internal_fixed_update_pos.clear();
        let mut pairs = Vec::new();
        for (id, node) in self.nodes.iter() {
            pairs.push((id, node.node_type()));
        }
        for (id, ty) in pairs {
            self.register_internal_node_schedules(id, ty);
        }
    }

    pub(crate) fn register_internal_node_schedules(&mut self, id: NodeID, ty: NodeType) {
        self.register_physics_body(id, ty);
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

    pub(crate) fn unregister_internal_node_schedules(&mut self, id: NodeID) {
        self.unregister_physics_body(id);
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
        self.internal_updates.internal_update_nodes.clear();
        self.internal_updates.internal_fixed_update_nodes.clear();
        self.internal_updates.internal_update_pos.clear();
        self.internal_updates.internal_fixed_update_pos.clear();
        self.internal_updates.physics_body_nodes_2d.clear();
        self.internal_updates.physics_body_nodes_3d.clear();
        self.internal_updates.physics_body_pos_2d.clear();
        self.internal_updates.physics_body_pos_3d.clear();
    }

    fn register_physics_body(&mut self, id: NodeID, ty: NodeType) {
        match ty {
            NodeType::StaticBody2D | NodeType::Area2D | NodeType::RigidBody2D => {
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
            NodeType::StaticBody3D | NodeType::Area3D | NodeType::RigidBody3D => {
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

    pub(crate) fn rebuild_node_tag_index(&mut self) {
        self.node_index.node_tag_index.clear();
        for (id, node) in self.nodes.iter() {
            for &tag in node.tags_slice() {
                self.node_index
                    .node_tag_index
                    .entry(tag)
                    .or_default()
                    .insert(id);
            }
        }
    }

    pub(crate) fn run_internal_update_schedule(&mut self) {
        let resource_api = self.resource_api.clone();
        let res = ResourceWindow::new(resource_api.as_ref());
        let input_ptr = std::ptr::addr_of!(self.input);
        // SAFETY: During callback dispatch, input is treated as immutable runtime state.
        // Engine invariant: only window/event ingestion mutates input, outside script callback execution.
        let ipt = unsafe { InputWindow::new(&*input_ptr) };
        let count = self.internal_updates.internal_update_nodes.len();
        for i in 0..count {
            let id = self.internal_updates.internal_update_nodes[i];
            if self.nodes.get(id).is_none() {
                continue;
            }
            self.call_internal_update_node_with_context(id, &res, &ipt);
        }
    }

    pub(crate) fn run_internal_fixed_update_schedule(&mut self) {
        let resource_api = self.resource_api.clone();
        let res = ResourceWindow::new(resource_api.as_ref());
        let input_ptr = std::ptr::addr_of!(self.input);
        // SAFETY: During callback dispatch, input is treated as immutable runtime state.
        // Engine invariant: only window/event ingestion mutates input, outside script callback execution.
        let ipt = unsafe { InputWindow::new(&*input_ptr) };
        let count = self.internal_updates.internal_fixed_update_nodes.len();
        for i in 0..count {
            let id = self.internal_updates.internal_fixed_update_nodes[i];
            if self.nodes.get(id).is_none() {
                continue;
            }
            self.call_internal_fixed_update_node_with_context(id, &res, &ipt);
        }
    }

    fn call_internal_update_node_with_context(
        &mut self,
        id: NodeID,
        res: &ResourceWindow<'_, crate::RuntimeResourceApi>,
        ipt: &InputWindow<'_, perro_input::InputSnapshot>,
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
        ipt: &InputWindow<'_, perro_input::InputSnapshot>,
    ) {
        if self.nodes.get(id).is_none() {
            return;
        }
        let mut ctx = RuntimeWindow::new(self);
        perro_internal_updates::internal_fixed_update_node(&mut ctx, res, ipt, id);
    }
}
