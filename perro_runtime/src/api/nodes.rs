use perro_context::sub_apis::NodeAPI;
use perro_core::{NodeTypeDispatch, Renderable, SceneNode, SceneNodeData};

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
}
