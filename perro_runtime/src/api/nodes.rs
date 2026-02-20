use perro_api::sub_apis::NodeAPI;
use perro_core::{NodeTypeDispatch, Renderable, SceneNode, SceneNodeData};

use crate::Runtime;

impl NodeAPI for Runtime {
    fn create<T>(&mut self) -> perro_ids::NodeID
    where
        T: Default + Into<SceneNodeData>,
    {
        self.nodes.insert(SceneNode::new(T::default().into()))
    }

    fn mutate<T, F>(&mut self, id: perro_ids::NodeID, f: F)
    where
        T: NodeTypeDispatch,
        F: FnOnce(&mut T),
    {
        if id.is_nil() {
            return;
        }

        let transform_changed = {
            let Some(node) = self.nodes.get_mut(id) else {
                return;
            };

            let mut changed = false;
            let result = node.with_typed_mut::<T, _>(|typed| {
                let before = T::snapshot_transform(typed);
                f(typed);
                let after = T::snapshot_transform(typed);
                changed = before != after;
            });
            match result {
                Some(()) => {}
                None => {
                    panic!(
                        "Node {} is not of expected type {:?} (actual: {:?})",
                        id,
                        T::NODE_TYPE,
                        node.node_type()
                    );
                }
            }
            changed
        };

        if matches!(T::RENDERABLE, Renderable::True) {
            self.mark_needs_rerender(id);
        }
        if transform_changed {
            self.mark_transform_dirty_recursive(id);
        }
    }

    fn read<T, V: Clone + Default>(
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
}
