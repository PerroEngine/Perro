use perro_core::{NodeTypeDispatch, SceneNodeData};
use perro_ids::NodeID;
use std::borrow::Cow;

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

    fn get_node_name(&mut self, node_id: NodeID) -> Option<Cow<'static, str>>;

    fn set_node_name<S>(&mut self, node_id: NodeID, name: S) -> bool
    where
        S: Into<Cow<'static, str>>;

    fn get_node_parent_id(&mut self, node_id: NodeID) -> Option<NodeID>;

    fn get_node_children_ids(&mut self, node_id: NodeID) -> Option<Vec<NodeID>>;

    fn reparent(&mut self, parent_id: NodeID, child_id: NodeID) -> bool;

    fn reparent_multi<I>(&mut self, parent_id: NodeID, child_ids: I) -> usize
    where
        I: IntoIterator<Item = NodeID>;
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

    pub fn get_node_name(&mut self, node_id: NodeID) -> Option<Cow<'static, str>> {
        self.rt.get_node_name(node_id)
    }

    pub fn set_node_name<S>(&mut self, node_id: NodeID, name: S) -> bool
    where
        S: Into<Cow<'static, str>>,
    {
        self.rt.set_node_name(node_id, name)
    }

    pub fn get_node_parent_id(&mut self, node_id: NodeID) -> Option<NodeID> {
        self.rt.get_node_parent_id(node_id)
    }

    pub fn get_node_children_ids(&mut self, node_id: NodeID) -> Option<Vec<NodeID>> {
        self.rt.get_node_children_ids(node_id)
    }

    pub fn reparent(&mut self, parent_id: NodeID, child_id: NodeID) -> bool {
        self.rt.reparent(parent_id, child_id)
    }

    pub fn reparent_multi<I>(&mut self, parent_id: NodeID, child_ids: I) -> usize
    where
        I: IntoIterator<Item = NodeID>,
    {
        self.rt.reparent_multi(parent_id, child_ids)
    }
}
