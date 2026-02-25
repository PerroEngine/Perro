use perro_ids::{IntoTagID, NodeID, TagID};
use perro_nodes::{NodeBaseDispatch, NodeType, NodeTypeDispatch, SceneNodeData};
use std::borrow::Cow;

#[derive(Clone, Debug, Default)]
pub struct TagQuery {
    pub has: Vec<TagID>,
    pub any: Vec<TagID>,
    pub not: Vec<TagID>,
    pub is_node_types: Vec<NodeType>,
    pub base_node_types: Vec<NodeType>,
}

impl TagQuery {
    pub const fn new() -> Self {
        Self {
            has: Vec::new(),
            any: Vec::new(),
            not: Vec::new(),
            is_node_types: Vec::new(),
            base_node_types: Vec::new(),
        }
    }

    pub fn has<I, T>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: IntoTagID,
    {
        self.has
            .extend(tags.into_iter().map(IntoTagID::into_tag_id));
        self
    }

    pub fn any<I, T>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: IntoTagID,
    {
        self.any
            .extend(tags.into_iter().map(IntoTagID::into_tag_id));
        self
    }

    pub fn not<I, T>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: IntoTagID,
    {
        self.not
            .extend(tags.into_iter().map(IntoTagID::into_tag_id));
        self
    }

    pub fn is_node_types<I>(mut self, types: I) -> Self
    where
        I: IntoIterator<Item = NodeType>,
    {
        self.is_node_types.extend(types);
        self
    }

    pub fn base_node_types<I>(mut self, types: I) -> Self
    where
        I: IntoIterator<Item = NodeType>,
    {
        self.base_node_types.extend(types);
        self
    }
}

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

    fn with_node_base<T, V, F>(&mut self, id: NodeID, f: F) -> Option<V>
    where
        T: NodeBaseDispatch,
        F: FnOnce(&T) -> V;

    fn with_node_base_mut<T, V, F>(&mut self, id: NodeID, f: F) -> Option<V>
    where
        T: NodeBaseDispatch,
        F: FnOnce(&mut T) -> V;

    fn get_node_name(&mut self, node_id: NodeID) -> Option<Cow<'static, str>>;

    fn set_node_name<S>(&mut self, node_id: NodeID, name: S) -> bool
    where
        S: Into<Cow<'static, str>>;

    fn get_node_parent_id(&mut self, node_id: NodeID) -> Option<NodeID>;

    fn get_node_children_ids(&mut self, node_id: NodeID) -> Option<Vec<NodeID>>;

    fn get_node_type(&mut self, node_id: NodeID) -> Option<NodeType>;

    fn reparent(&mut self, parent_id: NodeID, child_id: NodeID) -> bool;

    fn reparent_multi<I>(&mut self, parent_id: NodeID, child_ids: I) -> usize
    where
        I: IntoIterator<Item = NodeID>;

    fn get_node_tags(&mut self, node_id: NodeID) -> Option<Vec<TagID>>;

    fn set_node_tags<T>(&mut self, node_id: NodeID, tags: Option<T>) -> bool
    where
        T: Into<Cow<'static, [TagID]>>;

    fn add_node_tag<T>(&mut self, node_id: NodeID, tag: T) -> bool
    where
        T: IntoTagID;

    fn remove_node_tag<T>(&mut self, node_id: NodeID, tag: T) -> bool
    where
        T: IntoTagID;

    fn query_nodes(&mut self, query: TagQuery) -> Vec<NodeID>;
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

    pub fn with_node_base<T, V, F>(&mut self, id: NodeID, f: F) -> Option<V>
    where
        T: NodeBaseDispatch,
        F: FnOnce(&T) -> V,
    {
        self.rt.with_node_base::<T, V, F>(id, f)
    }

    pub fn with_node_base_mut<T, V, F>(&mut self, id: NodeID, f: F) -> Option<V>
    where
        T: NodeBaseDispatch,
        F: FnOnce(&mut T) -> V,
    {
        self.rt.with_node_base_mut::<T, V, F>(id, f)
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

    pub fn get_node_type(&mut self, node_id: NodeID) -> Option<NodeType> {
        self.rt.get_node_type(node_id)
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

    pub fn get_node_tags(&mut self, node_id: NodeID) -> Option<Vec<TagID>> {
        self.rt.get_node_tags(node_id)
    }

    pub fn set_node_tags<T>(&mut self, node_id: NodeID, tags: Option<T>) -> bool
    where
        T: Into<Cow<'static, [TagID]>>,
    {
        self.rt.set_node_tags(node_id, tags)
    }

    pub fn add_node_tag<T>(&mut self, node_id: NodeID, tag: T) -> bool
    where
        T: IntoTagID,
    {
        self.rt.add_node_tag(node_id, tag)
    }

    pub fn remove_node_tag<T>(&mut self, node_id: NodeID, tag: T) -> bool
    where
        T: IntoTagID,
    {
        self.rt.remove_node_tag(node_id, tag)
    }

    pub fn query(&mut self, query: TagQuery) -> Vec<NodeID> {
        self.rt.query_nodes(query)
    }
}

#[macro_export]
macro_rules! with_node_mut {
    ($ctx:expr, $node_ty:ty, $id:expr, $f:expr) => {
        $ctx.Nodes().with_node_mut::<$node_ty, _, _>($id, $f)
    };
}

#[macro_export]
macro_rules! with_node {
    ($ctx:expr, $node_ty:ty, $id:expr, $f:expr) => {
        $ctx.Nodes().with_node::<$node_ty, _>($id, $f)
    };
}

#[macro_export]
macro_rules! with_node_base {
    ($ctx:expr, $base_ty:ty, $id:expr, $f:expr) => {
        $ctx.Nodes().with_node_base::<$base_ty, _, _>($id, $f)
    };
}

#[macro_export]
macro_rules! with_node_base_mut {
    ($ctx:expr, $base_ty:ty, $id:expr, $f:expr) => {
        $ctx.Nodes().with_node_base_mut::<$base_ty, _, _>($id, $f)
    };
}

#[macro_export]
macro_rules! create_node {
    ($ctx:expr, $node_ty:ty) => {
        $ctx.Nodes().create::<$node_ty>()
    };
}

#[macro_export]
macro_rules! get_node_name {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_node_name($id)
    };
}

#[macro_export]
macro_rules! set_node_name {
    ($ctx:expr, $id:expr, $name:expr) => {
        $ctx.Nodes().set_node_name($id, $name)
    };
}

#[macro_export]
macro_rules! get_node_parent_id {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_node_parent_id($id)
    };
}

#[macro_export]
macro_rules! get_node_children_ids {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_node_children_ids($id)
    };
}

#[macro_export]
macro_rules! get_node_type {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_node_type($id)
    };
}

#[macro_export]
macro_rules! reparent {
    ($ctx:expr, $parent:expr, $child:expr) => {
        $ctx.Nodes().reparent($parent, $child)
    };
}

#[macro_export]
macro_rules! reparent_multi {
    ($ctx:expr, $parent:expr, $child_ids:expr) => {
        $ctx.Nodes().reparent_multi($parent, $child_ids)
    };
}

#[macro_export]
macro_rules! get_node_tags {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_node_tags($id)
    };
}

#[macro_export]
macro_rules! set_node_tags {
    ($ctx:expr, $id:expr, $tags:expr) => {
        $ctx.Nodes().set_node_tags($id, Some($tags))
    };
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes()
            .set_node_tags::<&'static [::perro_ids::TagID]>($id, None)
    };
}

#[macro_export]
macro_rules! add_node_tag {
    ($ctx:expr, $id:expr, $tag:expr) => {
        $ctx.Nodes().add_node_tag($id, $tag)
    };
}

#[macro_export]
macro_rules! remove_node_tag {
    ($ctx:expr, $id:expr, $tag:expr) => {
        $ctx.Nodes().remove_node_tag($id, $tag)
    };
}

#[macro_export]
macro_rules! tag_add {
    ($ctx:expr, $id:expr, $tag:expr) => {
        $ctx.Nodes().add_node_tag($id, $tag)
    };
}

#[macro_export]
macro_rules! tag_remove {
    ($ctx:expr, $id:expr, $tag:expr) => {
        $ctx.Nodes().remove_node_tag($id, $tag)
    };
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes()
            .set_node_tags::<&'static [::perro_ids::TagID]>($id, None)
    };
}

#[macro_export]
macro_rules! query {
    ($ctx:expr $(, $kind:ident [$($arg:tt)*] )* $(,)?) => {{
        let mut __query = $crate::sub_apis::TagQuery::new();
        $(
            __query = $crate::query!(@apply __query, $kind, [$($arg)*]);
        )*
        $ctx.Nodes().query(__query)
    }};
    (@apply $query:expr, has, [$($tag:expr),* $(,)?]) => {
        $query.has(vec![$(::perro_ids::IntoTagID::into_tag_id($tag)),*])
    };
    (@apply $query:expr, any, [$($tag:expr),* $(,)?]) => {
        $query.any(vec![$(::perro_ids::IntoTagID::into_tag_id($tag)),*])
    };
    (@apply $query:expr, not, [$($tag:expr),* $(,)?]) => {
        $query.not(vec![$(::perro_ids::IntoTagID::into_tag_id($tag)),*])
    };
    (@apply $query:expr, is, [$($ty:ident),* $(,)?]) => {
        $query.is_node_types(vec![$(::perro_nodes::NodeType::$ty),*])
    };
    (@apply $query:expr, is, [$($ty:path),* $(,)?]) => {
        $query.is_node_types(vec![$($ty),*])
    };
    (@apply $query:expr, is_type, [$($ty:ident),* $(,)?]) => {
        $query.is_node_types(vec![$(::perro_nodes::NodeType::$ty),*])
    };
    (@apply $query:expr, is_type, [$($ty:path),* $(,)?]) => {
        $query.is_node_types(vec![$($ty),*])
    };
    (@apply $query:expr, base, [$($ty:ident),* $(,)?]) => {
        $query.base_node_types(vec![$(::perro_nodes::NodeType::$ty),*])
    };
    (@apply $query:expr, base, [$($ty:path),* $(,)?]) => {
        $query.base_node_types(vec![$($ty),*])
    };
    (@apply $query:expr, base_type, [$($ty:ident),* $(,)?]) => {
        $query.base_node_types(vec![$(::perro_nodes::NodeType::$ty),*])
    };
    (@apply $query:expr, base_type, [$($ty:path),* $(,)?]) => {
        $query.base_node_types(vec![$($ty),*])
    };
}
