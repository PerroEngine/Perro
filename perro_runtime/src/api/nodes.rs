use perro_api::modules::NodeAPI;

use crate::Runtime;

impl NodeAPI for Runtime {
    fn create<T>(&mut self) -> perro_ids::NodeID {
        todo!()
    }

    fn mutate<T, F>(&mut self, id: perro_ids::NodeID, f: F) {
        todo!()
    }

    fn read<T, V: Clone + Default>(
        &mut self,
        node_id: perro_ids::NodeID,
        f: impl FnOnce(&T) -> V,
    ) -> V {
        todo!()
    }
}
