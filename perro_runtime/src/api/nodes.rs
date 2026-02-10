use perro_api::modules::NodeAPI;

use crate::Runtime;

impl NodeAPI for Runtime {
    fn create_node<T>(&mut self) -> perro_ids::NodeID {
        todo!()
    }

    fn mutate_node<T, F>(&mut self, id: perro_ids::NodeID, f: F) {
        todo!()
    }

    fn read_node<T, V: Clone + Default>(
        &self,
        node_id: perro_ids::NodeID,
        f: impl FnOnce(&T) -> V,
    ) -> V {
        todo!()
    }
}
