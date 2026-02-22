use perro_core::{NodeTypeDispatch, SceneNode, SceneNodeData};
use perro_ids::NodeID;

pub trait NodeAPI {
    fn create<T>(&mut self) -> NodeID
    where
        T: Default + Into<SceneNodeData>;

    fn with_node_mut<T, V, F>(&mut self, id: NodeID, f: F) -> Option<V>
    where
        T: NodeTypeDispatch,
        F: FnOnce(&mut T) -> V;

    fn with_node<T, V: Clone + Default>(&mut self, node_id: NodeID, f: impl FnOnce(&T) -> V) -> V
    where
        T: NodeTypeDispatch;

    fn with_node_meta_mut<F>(&mut self, id: NodeID, f: F)
    where
        F: FnOnce(&mut SceneNode);

    fn with_node_meta<V: Clone + Default>(
        &mut self,
        node_id: NodeID,
        f: impl FnOnce(&SceneNode) -> V,
    ) -> V;
}

pub struct NodeModule<'rt, R: NodeAPI + ?Sized> {
    rt: &'rt mut R,
}

impl<'rt, R: NodeAPI + ?Sized> NodeModule<'rt, R> {
    pub fn new(rt: &'rt mut R) -> Self {
        Self { rt }
    }

    pub fn create<T>(&mut self) -> NodeID
    where
        T: Default + Into<SceneNodeData>,
    {
        self.rt.create::<T>()
    }

    pub fn with_node_mut<T, V, F>(&mut self, id: NodeID, f: F) -> Option<V>
    where
        T: NodeTypeDispatch,
        F: FnOnce(&mut T) -> V,
    {
        self.rt.with_node_mut::<T, V, F>(id, f)
    }

    pub fn with_node<T, V: Clone + Default>(
        &mut self,
        node_id: NodeID,
        f: impl FnOnce(&T) -> V,
    ) -> V
    where
        T: NodeTypeDispatch,
    {
        self.rt.with_node::<T, V>(node_id, f)
    }

    pub fn with_node_meta_mut<F>(&mut self, id: NodeID, f: F)
    where
        F: FnOnce(&mut SceneNode),
    {
        self.rt.with_node_meta_mut(id, f);
    }

    pub fn with_node_meta<V: Clone + Default>(
        &mut self,
        node_id: NodeID,
        f: impl FnOnce(&SceneNode) -> V,
    ) -> V {
        self.rt.with_node_meta(node_id, f)
    }
}
