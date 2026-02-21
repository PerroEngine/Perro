use perro_core::{NodeTypeDispatch, SceneNode, SceneNodeData};
use perro_ids::NodeID;

pub trait NodeAPI {
    fn create<T>(&mut self) -> NodeID
    where
        T: Default + Into<SceneNodeData>;

    fn mutate<T, F>(&mut self, id: NodeID, f: F)
    where
        T: NodeTypeDispatch,
        F: FnOnce(&mut T);

    fn read<T, V: Clone + Default>(&mut self, node_id: NodeID, f: impl FnOnce(&T) -> V) -> V
    where
        T: NodeTypeDispatch;

    fn mutate_meta<F>(&mut self, id: NodeID, f: F)
    where
        F: FnOnce(&mut SceneNode);

    fn read_meta<V: Clone + Default>(
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

    pub fn mutate<T, F>(&mut self, id: NodeID, f: F)
    where
        T: NodeTypeDispatch,
        F: FnOnce(&mut T),
    {
        self.rt.mutate::<T, F>(id, f);
    }

    pub fn read<T, V: Clone + Default>(&mut self, node_id: NodeID, f: impl FnOnce(&T) -> V) -> V
    where
        T: NodeTypeDispatch,
    {
        self.rt.read::<T, V>(node_id, f)
    }

    pub fn mutate_meta<F>(&mut self, id: NodeID, f: F)
    where
        F: FnOnce(&mut SceneNode),
    {
        self.rt.mutate_meta(id, f);
    }

    pub fn read_meta<V: Clone + Default>(
        &mut self,
        node_id: NodeID,
        f: impl FnOnce(&SceneNode) -> V,
    ) -> V {
        self.rt.read_meta(node_id, f)
    }
}
