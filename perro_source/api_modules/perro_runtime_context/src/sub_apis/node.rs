use perro_ids::{IntoTagID, NodeID, TagID};
use perro_nodes::{NodeBaseDispatch, NodeType, NodeTypeDispatch, SceneNodeData};
use std::borrow::Cow;

/// Query clauses used by [`query!`](macro@crate::query) to filter nodes.
///
/// Matching semantics:
/// - `has`: all listed tags must be present
/// - `any`: at least one listed tag must be present
/// - `not`: none of listed tags may be present
/// - `is_node_types`: node's concrete [`NodeType`] must match one of these
/// - `base_node_types`: node's concrete type must be `is_a` one of these base types
///
/// Across fields, clauses are combined with logical AND.
#[derive(Clone, Debug, Default)]
pub struct TagQuery {
    pub has: Vec<TagID>,
    pub any: Vec<TagID>,
    pub not: Vec<TagID>,
    pub is_node_types: Vec<NodeType>,
    pub base_node_types: Vec<NodeType>,
}

/// Converts a single tag or tag collection into `Vec<TagID>`.
///
/// Used by [`tag_add!`](macro@crate::tag_add) to support one-or-many inputs.
pub trait IntoNodeTags {
    fn into_tag_ids(self) -> Vec<TagID>;
}

impl IntoNodeTags for TagID {
    fn into_tag_ids(self) -> Vec<TagID> {
        vec![self]
    }
}

impl IntoNodeTags for &TagID {
    fn into_tag_ids(self) -> Vec<TagID> {
        vec![*self]
    }
}

impl IntoNodeTags for &str {
    fn into_tag_ids(self) -> Vec<TagID> {
        vec![self.into_tag_id()]
    }
}

impl IntoNodeTags for String {
    fn into_tag_ids(self) -> Vec<TagID> {
        vec![self.into_tag_id()]
    }
}

impl IntoNodeTags for &String {
    fn into_tag_ids(self) -> Vec<TagID> {
        vec![self.into_tag_id()]
    }
}

impl IntoNodeTags for &[TagID] {
    fn into_tag_ids(self) -> Vec<TagID> {
        self.to_vec()
    }
}

impl<const N: usize> IntoNodeTags for [TagID; N] {
    fn into_tag_ids(self) -> Vec<TagID> {
        self.into_iter().collect()
    }
}

impl<const N: usize> IntoNodeTags for &[TagID; N] {
    fn into_tag_ids(self) -> Vec<TagID> {
        self.as_slice().to_vec()
    }
}

impl IntoNodeTags for Vec<TagID> {
    fn into_tag_ids(self) -> Vec<TagID> {
        self
    }
}

impl IntoNodeTags for &[&str] {
    fn into_tag_ids(self) -> Vec<TagID> {
        self.iter().map(|tag| (*tag).into_tag_id()).collect()
    }
}

impl<const N: usize> IntoNodeTags for [&str; N] {
    fn into_tag_ids(self) -> Vec<TagID> {
        self.into_iter().map(IntoTagID::into_tag_id).collect()
    }
}

impl<const N: usize> IntoNodeTags for &[&str; N] {
    fn into_tag_ids(self) -> Vec<TagID> {
        self.as_slice()
            .iter()
            .map(|tag| (*tag).into_tag_id())
            .collect()
    }
}

impl IntoNodeTags for Vec<&str> {
    fn into_tag_ids(self) -> Vec<TagID> {
        self.into_iter().map(IntoTagID::into_tag_id).collect()
    }
}

impl IntoNodeTags for &[String] {
    fn into_tag_ids(self) -> Vec<TagID> {
        self.iter().map(IntoTagID::into_tag_id).collect()
    }
}

impl<const N: usize> IntoNodeTags for [String; N] {
    fn into_tag_ids(self) -> Vec<TagID> {
        self.into_iter().map(IntoTagID::into_tag_id).collect()
    }
}

impl<const N: usize> IntoNodeTags for &[String; N] {
    fn into_tag_ids(self) -> Vec<TagID> {
        self.as_slice().iter().map(IntoTagID::into_tag_id).collect()
    }
}

impl IntoNodeTags for Vec<String> {
    fn into_tag_ids(self) -> Vec<TagID> {
        self.into_iter().map(IntoTagID::into_tag_id).collect()
    }
}

impl TagQuery {
    /// Creates an empty query (matches all nodes).
    pub const fn new() -> Self {
        Self {
            has: Vec::new(),
            any: Vec::new(),
            not: Vec::new(),
            is_node_types: Vec::new(),
            base_node_types: Vec::new(),
        }
    }

    /// Adds tags to the `has` clause.
    ///
    /// Each tag is converted via [`IntoTagID`].
    pub fn has<I, T>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: IntoTagID,
    {
        self.has
            .extend(tags.into_iter().map(IntoTagID::into_tag_id));
        self
    }

    /// Adds tags to the `any` clause.
    ///
    /// Each tag is converted via [`IntoTagID`].
    pub fn any<I, T>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: IntoTagID,
    {
        self.any
            .extend(tags.into_iter().map(IntoTagID::into_tag_id));
        self
    }

    /// Adds tags to the `not` clause.
    ///
    /// Each tag is converted via [`IntoTagID`].
    pub fn not<I, T>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: IntoTagID,
    {
        self.not
            .extend(tags.into_iter().map(IntoTagID::into_tag_id));
        self
    }

    /// Adds exact type filters.
    ///
    /// Match succeeds if node's concrete type is any one of these.
    pub fn is_node_types<I>(mut self, types: I) -> Self
    where
        I: IntoIterator<Item = NodeType>,
    {
        self.is_node_types.extend(types);
        self
    }

    /// Adds base/inclusive type filters.
    ///
    /// Match succeeds if node's concrete type `is_a` any one of these.
    pub fn base_node_types<I>(mut self, types: I) -> Self
    where
        I: IntoIterator<Item = NodeType>,
    {
        self.base_node_types.extend(types);
        self
    }
}

pub trait NodeAPI {
    /// Creates a new node with default value of `T`.
    fn create<T>(&mut self) -> NodeID
    where
        T: Default + Into<SceneNodeData>;

    /// Runs closure against an exact concrete node type.
    ///
    /// Returns `None` if `id` is invalid or node type does not exactly match `T`.
    fn with_node_mut<T, V, F>(&mut self, id: NodeID, f: F) -> Option<V>
    where
        T: NodeTypeDispatch,
        F: FnOnce(&mut T) -> V;

    /// Reads from an exact concrete node type.
    ///
    /// Returns `V::default()` if `id` is invalid or node type does not exactly match `T`.
    fn with_node<T, V: Clone + Default>(&mut self, node_id: NodeID, f: impl FnOnce(&T) -> V) -> V
    where
        T: NodeTypeDispatch;

    /// Runs closure against a base type (`T`) with runtime ancestry check.
    ///
    /// This allows descendant concrete types to be treated as a shared base type.
    fn with_base_node<T, V, F>(&mut self, id: NodeID, f: F) -> Option<V>
    where
        T: NodeBaseDispatch,
        F: FnOnce(&T) -> V;

    /// Mutable variant of [`NodeAPI::with_base_node`].
    fn with_base_node_mut<T, V, F>(&mut self, id: NodeID, f: F) -> Option<V>
    where
        T: NodeBaseDispatch,
        F: FnOnce(&mut T) -> V;

    /// Returns node display name if node exists.
    fn get_node_name(&mut self, node_id: NodeID) -> Option<Cow<'static, str>>;

    /// Sets node display name; returns `true` on success.
    fn set_node_name<S>(&mut self, node_id: NodeID, name: S) -> bool
    where
        S: Into<Cow<'static, str>>;

    /// Returns parent node id if node exists.
    fn get_node_parent_id(&mut self, node_id: NodeID) -> Option<NodeID>;

    /// Returns children ids if node exists.
    fn get_node_children_ids(&mut self, node_id: NodeID) -> Option<Vec<NodeID>>;

    /// Returns concrete runtime node type if node exists.
    fn get_node_type(&mut self, node_id: NodeID) -> Option<NodeType>;

    /// Reparents a child under parent. `parent_id = nil` detaches to root.
    fn reparent(&mut self, parent_id: NodeID, child_id: NodeID) -> bool;

    /// Batch reparent. Returns count of successful operations.
    fn reparent_multi<I>(&mut self, parent_id: NodeID, child_ids: I) -> usize
    where
        I: IntoIterator<Item = NodeID>;

    /// Returns node tags if node exists.
    fn get_node_tags(&mut self, node_id: NodeID) -> Option<Vec<TagID>>;

    /// Sets node tags (`Some`) or clears all tags (`None`).
    ///
    /// `T` supports borrowed static slices or owned vectors through `Cow`.
    fn set_node_tags<T>(&mut self, node_id: NodeID, tags: Option<T>) -> bool
    where
        T: Into<Cow<'static, [TagID]>>;

    /// Adds one tag to node (idempotent).
    fn add_node_tag<T>(&mut self, node_id: NodeID, tag: T) -> bool
    where
        T: IntoTagID;

    /// Removes one tag from node.
    fn remove_node_tag<T>(&mut self, node_id: NodeID, tag: T) -> bool
    where
        T: IntoTagID;

    /// Executes a node query and returns matching node IDs.
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

    pub fn with_base_node<T, V, F>(&mut self, id: NodeID, f: F) -> Option<V>
    where
        T: NodeBaseDispatch,
        F: FnOnce(&T) -> V,
    {
        self.rt.with_base_node::<T, V, F>(id, f)
    }

    pub fn with_base_node_mut<T, V, F>(&mut self, id: NodeID, f: F) -> Option<V>
    where
        T: NodeBaseDispatch,
        F: FnOnce(&mut T) -> V,
    {
        self.rt.with_base_node_mut::<T, V, F>(id, f)
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

    pub fn add_node_tags<T>(&mut self, node_id: NodeID, tags: T) -> bool
    where
        T: IntoNodeTags,
    {
        let tag_ids = tags.into_tag_ids();
        if tag_ids.is_empty() {
            return true;
        }

        for tag in tag_ids {
            if !self.rt.add_node_tag(node_id, tag) {
                return false;
            }
        }
        true
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

/// Node access macros.
///
/// These macros expose typed node access via closure-scoped borrows.
///
/// Exact-type mutable node access.
/// Usage: `with_node_mut!(ctx, ConcreteType, node_id, |node| { ... }) -> Option<V>`.
/// Internals:
/// - The runtime looks up `node_id`, verifies exact type equality with `ConcreteType`,
///   then invokes your closure while holding a short-lived mutable borrow.
/// - The borrow cannot escape the closure, which keeps access compile-time safe.
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
/// - `ConcreteType`: concrete node struct type (exact match only)
/// - `node_id`: `NodeID`
/// - closure arg: `&mut ConcreteType`
#[macro_export]
macro_rules! with_node_mut {
    ($ctx:expr, $node_ty:ty, $id:expr, $f:expr) => {
        $ctx.Nodes().with_node_mut::<$node_ty, _, _>($id, $f)
    };
}

/// Exact-type read node access.
/// Usage: `with_node!(ctx, ConcreteType, node_id, |node| -> V { ... }) -> V`.
/// Internals:
/// - The runtime does an exact concrete-type check, then calls the closure with `&ConcreteType`.
/// - The read borrow is scoped to the closure call and cannot outlive it.
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
/// - `ConcreteType`: concrete node struct type (exact match only)
/// - `node_id`: `NodeID`
/// - closure arg: `&ConcreteType`
#[macro_export]
macro_rules! with_node {
    ($ctx:expr, $node_ty:ty, $id:expr, $f:expr) => {
        $ctx.Nodes().with_node::<$node_ty, _>($id, $f)
    };
}

/// Base/inheritance-aware read node access.
/// Usage: `with_base_node!(ctx, BaseType, node_id, |base| { ... }) -> Option<V>`.
/// Internals:
/// - The runtime checks `node.node_type().is_a(BaseType)`, then dispatches the closure as `&BaseType`.
/// - This keeps one runtime check while still giving typed field/method access in the closure body.
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
/// - `BaseType`: base node struct type (descendants allowed)
/// - `node_id`: `NodeID`
/// - closure arg: `&BaseType`
#[macro_export]
macro_rules! with_base_node {
    ($ctx:expr, $base_ty:ty, $id:expr, $f:expr) => {
        $ctx.Nodes().with_base_node::<$base_ty, _, _>($id, $f)
    };
}

/// Base/inheritance-aware mutable node access.
/// Usage: `with_base_node_mut!(ctx, BaseType, node_id, |base| { ... }) -> Option<V>`.
/// Internals:
/// - Same `is_a` runtime check as `with_base_node!`, then executes your closure with `&mut BaseType`.
/// - Mutable borrow is closure-scoped so references cannot escape.
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
/// - `BaseType`: base node struct type (descendants allowed)
/// - `node_id`: `NodeID`
/// - closure arg: `&mut BaseType`
#[macro_export]
macro_rules! with_base_node_mut {
    ($ctx:expr, $base_ty:ty, $id:expr, $f:expr) => {
        $ctx.Nodes().with_base_node_mut::<$base_ty, _, _>($id, $f)
    };
}

/// Creates a node from default concrete type.
/// Usage: `create_node!(ctx, ConcreteType) -> NodeID`.
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
/// - `ConcreteType`: node struct type implementing `Default`
#[macro_export]
macro_rules! create_node {
    ($ctx:expr, $node_ty:ty) => {
        $ctx.Nodes().create::<$node_ty>()
    };
}

/// SceneNode metadata macros.
///
/// These macros expose node identity/relationship/metadata access:
/// - name (`get_node_name!`, `set_node_name!`)
/// - hierarchy (`get_node_parent_id!`, `get_node_children_ids!`)
/// - runtime typing (`get_node_type!`)
/// - tags (`get_node_tags!`, `set_node_tags!`, `tag_add!`, `tag_remove!`)
/// Gets node display name.
/// Usage: `get_node_name!(ctx, node_id) -> Option<Cow<'static, str>>`.
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
/// - `node_id`: `NodeID`
#[macro_export]
macro_rules! get_node_name {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_node_name($id)
    };
}

/// Sets node display name.
/// Usage: `set_node_name!(ctx, node_id, name) -> bool`.
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
/// - `node_id`: `NodeID`
/// - `name`: `&str`, `String`, or `Cow<'static, str>`
#[macro_export]
macro_rules! set_node_name {
    ($ctx:expr, $id:expr, $name:expr) => {
        $ctx.Nodes().set_node_name($id, $name)
    };
}

/// Gets node parent id.
/// Usage: `get_node_parent_id!(ctx, node_id) -> Option<NodeID>`.
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
/// - `node_id`: `NodeID`
#[macro_export]
macro_rules! get_node_parent_id {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_node_parent_id($id)
    };
}

/// Gets children ids for a node.
/// Usage: `get_node_children_ids!(ctx, node_id) -> Option<Vec<NodeID>>`.
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
/// - `node_id`: `NodeID`
#[macro_export]
macro_rules! get_node_children_ids {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_node_children_ids($id)
    };
}

/// Gets concrete runtime node type.
/// Usage: `get_node_type!(ctx, node_id) -> Option<NodeType>`.
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
/// - `node_id`: `NodeID`
#[macro_export]
macro_rules! get_node_type {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_node_type($id)
    };
}

/// Reparents a child under parent (`parent = nil` detaches).
/// Usage: `reparent!(ctx, parent_id, child_id) -> bool`.
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
/// - `parent_id`: `NodeID` (`NodeID::nil()` detaches child)
/// - `child_id`: `NodeID`
#[macro_export]
macro_rules! reparent {
    ($ctx:expr, $parent:expr, $child:expr) => {
        $ctx.Nodes().reparent($parent, $child)
    };
}

/// Batch reparent.
/// Usage: `reparent_multi!(ctx, parent_id, child_ids_iter) -> usize`.
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
/// - `parent_id`: `NodeID` (`NodeID::nil()` detaches)
/// - `child_ids_iter`: iterator of `NodeID`
#[macro_export]
macro_rules! reparent_multi {
    ($ctx:expr, $parent:expr, $child_ids:expr) => {
        $ctx.Nodes().reparent_multi($parent, $child_ids)
    };
}

/// Gets node tags.
/// Usage: `get_node_tags!(ctx, node_id) -> Option<Vec<TagID>>`.
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
/// - `node_id`: `NodeID`
#[macro_export]
macro_rules! get_node_tags {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_node_tags($id)
    };
}

/// Sets or clears node tags.
/// Usage:
/// - `set_node_tags!(ctx, node_id, tags)` where `tags` is `Cow<'static, [TagID]>` compatible.
/// - `set_node_tags!(ctx, node_id)` clears all tags.
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
/// - `node_id`: `NodeID`
/// - `tags`: usually from `tags![...]`, or `&[TagID]`, `[TagID; N]`, `Vec<TagID>`
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

/// Adds one or more tags to a node.
/// Usage:
/// - `tag_add!(ctx, node_id, "enemy")`
/// - `tag_add!(ctx, node_id, tags!["enemy", "alive"])`
/// - `tag_add!(ctx, node_id, ["enemy", "alive"])`
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
/// - `node_id`: `NodeID`
/// - tags: `TagID`, `&str`, `String`, slices/arrays/vectors of those
#[macro_export]
macro_rules! tag_add {
    ($ctx:expr, $id:expr, $tags:expr) => {
        $ctx.Nodes().add_node_tags($id, $tags)
    };
}

/// Removes tag(s) from node.
/// Usage:
/// - `tag_remove!(ctx, node_id, tag) -> bool`
/// - `tag_remove!(ctx, node_id)` clears all tags.
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
/// - `node_id`: `NodeID`
/// - `tag` (3-arg form): `TagID`, `&str`, or `String`
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

/// Executes a node query and returns `Vec<NodeID>`.
///
/// Syntax:
/// - `query!(ctx, CLAUSE[...], CLAUSE[...], ...)`
/// - Each clause always uses bracket form: `CLAUSE[comma-separated items]`.
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
/// - clause values:
///   - tag clauses (`has/any/not`) accept `TagID`, `&str`, `String`
///   - type clauses (`is/base`) accept `NodeType` variants
///
/// Clauses:
/// - `has[...]` all tags must match
/// - `any[...]` at least one tag must match
/// - `not[...]` tags must not match
/// - `is_type[...]` exact concrete type OR list
/// - `base_type[...]` inclusive base type OR list (`is_a`)
///
/// Aliases:
/// - `is[...]` == `is_type[...]`
/// - `base[...]` == `base_type[...]`
///
/// Across clauses = AND.
///
/// Examples:
/// - `query!(ctx, has["enemy", "alive"])`
/// - `query!(ctx, any["flying", "boss"], not["dead"])`
/// - `query!(ctx, is[MeshInstance3D, Light3D])`
/// - `query!(ctx, base[Node3D], has["visible"])`
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
