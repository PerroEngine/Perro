use perro_ids::NodeID;

pub trait NodeAPI {
    fn create<T>(&mut self) -> NodeID;

    fn mutate<T, F>(&mut self, id: NodeID, f: F);

    fn read<T, V: Clone + Default>(&mut self, node_id: NodeID, f: impl FnOnce(&T) -> V) -> V;
}

pub struct NodeModule<'rt, R: NodeAPI + ?Sized> {
    rt: &'rt mut R,
}

impl<'rt, R: NodeAPI + ?Sized> NodeModule<'rt, R> {
    pub fn new(rt: &'rt mut R) -> Self {
        Self { rt }
    }

    pub fn create<T>(&mut self) -> NodeID {
        self.rt.create::<T>()
    }

    pub fn mutate<T, F>(&mut self, id: NodeID, f: F) {
        self.rt.mutate::<T, F>(id, f);
    }

    pub fn read<T, V: Clone + Default>(&mut self, node_id: NodeID, f: impl FnOnce(&T) -> V) -> V {
        self.rt.read::<T, V>(node_id, f)
    }
}
