use perro_api::modules::NodeAPI;

use crate::Runtime;

impl NodeAPI for Runtime {
    fn create<T>(&self) -> perro_ids::NodeID {
        todo!()
    }

    fn mutate<T, F>(&self, id: perro_ids::NodeID, f: F) {
        todo!()
    }

    fn read<T, V: Clone + Default>(
        &self,
        node_id: perro_ids::NodeID,
        f: impl FnOnce(&T) -> V,
    ) -> V {
        todo!()
    }
}
