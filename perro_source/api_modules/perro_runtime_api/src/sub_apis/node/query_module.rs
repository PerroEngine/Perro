use super::*;

pub struct NodeQueryModule<'rt, R: NodeAPI + ?Sized> {
    rt: &'rt mut R,
}

impl<'rt, R: NodeAPI + ?Sized> NodeQueryModule<'rt, R> {
    pub fn new(rt: &'rt mut R) -> Self {
        Self { rt }
    }

    pub fn query(&mut self, query: &NodeQuery) -> Vec<NodeID> {
        self.query_view(query.as_view())
    }

    /// Executes a node query and returns owned node ids as an iterator.
    ///
    /// This still allocates the same `Vec<NodeID>` as [`Self::query`], then
    /// returns its `IntoIter`.
    pub fn query_iter(&mut self, query: &NodeQuery) -> std::vec::IntoIter<NodeID> {
        self.query(query).into_iter()
    }

    /// Executes a node query and returns the first matching node id.
    pub fn query_first(&mut self, query: &NodeQuery) -> Option<NodeID> {
        self.query_view_first(query.as_view())
    }

    #[doc(hidden)]
    pub fn query_view(&mut self, query: NodeQueryView<'_>) -> Vec<NodeID> {
        self.rt.query_nodes(query)
    }

    #[doc(hidden)]
    pub fn query_view_iter(&mut self, query: NodeQueryView<'_>) -> std::vec::IntoIter<NodeID> {
        self.query_view(query).into_iter()
    }

    #[doc(hidden)]
    pub fn query_view_first(&mut self, query: NodeQueryView<'_>) -> Option<NodeID> {
        self.rt.query_first_node(query)
    }
}
