use perro_context::sub_apis::NodeAPI;
use perro_core::{NodeTypeDispatch, Renderable, SceneNode, SceneNodeData};
use std::borrow::Cow;

use crate::Runtime;

impl NodeAPI for Runtime {
    fn create<T>(&mut self) -> perro_ids::NodeID
    where
        T: Default + Into<SceneNodeData>,
    {
        self.nodes.insert(SceneNode::new(T::default().into()))
    }

    fn with_node_mut<T, V, F>(&mut self, id: perro_ids::NodeID, f: F) -> Option<V>
    where
        T: NodeTypeDispatch,
        F: FnOnce(&mut T) -> V,
    {
        if id.is_nil() {
            return None;
        }

        let (transform_changed, value) = {
            let Some(node) = self.nodes.get_mut(id) else {
                return None;
            };

            let mut changed = false;
            let mut value = None;
            let result = node.with_typed_mut::<T, _>(|typed| {
                let before = T::snapshot_transform(typed);
                value = Some(f(typed));
                let after = T::snapshot_transform(typed);
                changed = before != after;
            });
            if result.is_none() {
                return None;
            }
            (changed, value)
        };

        if matches!(T::RENDERABLE, Renderable::True) {
            self.mark_needs_rerender(id);
        }
        if transform_changed {
            self.mark_transform_dirty_recursive(id);
        }
        value
    }

    fn with_node<T, V: Clone + Default>(
        &mut self,
        node_id: perro_ids::NodeID,
        f: impl FnOnce(&T) -> V,
    ) -> V
    where
        T: NodeTypeDispatch,
    {
        if node_id.is_nil() {
            return V::default();
        }

        let Some(node) = self.nodes.get(node_id) else {
            return V::default();
        };

        node.with_typed_ref::<T, _>(f).unwrap_or_default()
    }

    fn with_node_meta_mut<F>(&mut self, id: perro_ids::NodeID, f: F)
    where
        F: FnOnce(&mut SceneNode),
    {
        if id.is_nil() {
            return;
        }
        let Some(node) = self.nodes.get_mut(id) else {
            return;
        };
        f(node);
    }

    fn with_node_meta<V: Clone + Default>(
        &mut self,
        node_id: perro_ids::NodeID,
        f: impl FnOnce(&SceneNode) -> V,
    ) -> V {
        if node_id.is_nil() {
            return V::default();
        }
        let Some(node) = self.nodes.get(node_id) else {
            return V::default();
        };
        f(node)
    }

    fn get_node_name(&mut self, node_id: perro_ids::NodeID) -> Option<Cow<'static, str>> {
        self.nodes.get(node_id).map(|node| node.name.clone())
    }

    fn set_node_name<S>(&mut self, node_id: perro_ids::NodeID, name: S) -> bool
    where
        S: Into<Cow<'static, str>>,
    {
        let Some(node) = self.nodes.get_mut(node_id) else {
            return false;
        };
        node.set_name(name);
        true
    }

    fn get_node_parent_id(&mut self, node_id: perro_ids::NodeID) -> Option<perro_ids::NodeID> {
        self.nodes.get(node_id).map(|node| node.get_parent_id())
    }

    fn get_node_children_ids(&mut self, node_id: perro_ids::NodeID) -> Option<Vec<perro_ids::NodeID>> {
        self.nodes
            .get(node_id)
            .map(|node| node.get_children_ids().to_vec())
    }

    fn reparent(&mut self, parent_id: perro_ids::NodeID, child_id: perro_ids::NodeID) -> bool {
        if child_id.is_nil() {
            return false;
        }
        if !parent_id.is_nil() && self.nodes.get(parent_id).is_none() {
            return false;
        }

        let old_parent = match self.nodes.get(child_id) {
            Some(node) => node.get_parent_id(),
            None => return false,
        };

        if old_parent == parent_id {
            return true;
        }

        if !old_parent.is_nil() {
            if let Some(parent) = self.nodes.get_mut(old_parent) {
                parent.remove_child(child_id);
            }
        }

        if let Some(child) = self.nodes.get_mut(child_id) {
            child.parent = parent_id;
        } else {
            return false;
        }

        if !parent_id.is_nil() {
            if let Some(parent) = self.nodes.get_mut(parent_id) {
                if !parent.get_children_ids().contains(&child_id) {
                    parent.add_child(child_id);
                }
            } else {
                return false;
            }
        }

        self.mark_transform_dirty_recursive(child_id);
        true
    }

    fn reparent_multi<I>(&mut self, parent_id: perro_ids::NodeID, child_ids: I) -> usize
    where
        I: IntoIterator<Item = perro_ids::NodeID>,
    {
        let mut updated = 0_usize;
        for child_id in child_ids {
            if self.reparent(parent_id, child_id) {
                updated += 1;
            }
        }
        updated
    }
}
