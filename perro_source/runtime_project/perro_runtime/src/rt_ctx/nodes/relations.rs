use super::*;

impl Runtime {
    pub(super) fn node_is_descendant_of(
        &self,
        mut id: perro_ids::NodeID,
        root: perro_ids::NodeID,
    ) -> bool {
        let mut hops = 0usize;
        while !id.is_nil() {
            if id == root {
                return true;
            }
            let Some(node) = self.nodes.get(id) else {
                return false;
            };
            id = node.get_parent();
            hops += 1;
            // Parent-cycle guard; deeper than any real scene tree.
            if hops > 100_000 {
                return false;
            }
        }
        false
    }
}
