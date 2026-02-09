use perro_ids::NodeID;

pub trait NodeAPI {
    fn create_node<T>(&mut self) -> NodeID;

    fn mutate_node<T, F>(&mut self, id: NodeID, f: F);

    fn read_node<T, V: Clone + Default>(&self, node_id: NodeID, f: impl FnOnce(&T) -> V) -> V;
}

pub struct NodeModule<'rt, R: NodeAPI + ?Sized> {
    rt: &'rt mut R,
}

impl<'rt, R: NodeAPI + ?Sized> NodeModule<'rt, R> {
    pub fn new(rt: &'rt mut R) -> Self {
        Self { rt }
    }

    pub fn create_node<T>(&mut self) -> NodeID {
        self.rt.create_node::<T>()
    }

    pub fn mutate_node<T, F>(&mut self, id: NodeID, f: F) {
        self.rt.mutate_node::<T, F>(id, f);
    }
}
