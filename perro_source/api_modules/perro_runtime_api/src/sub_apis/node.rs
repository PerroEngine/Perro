//! Runtime node API.
//!
//! Creates, removes, reparents, tags, transforms, reads, writes, and queries
//! live scene nodes. Query helpers live beside node access because they operate
//! on the same runtime scene graph.

use perro_ids::{IntoTagID, MaterialID, MeshID, NodeID, NodeTag, TagID};
use perro_nodes::{
    Node2D, Node3D, NodeBaseDispatch, NodeType, NodeTypeDispatch, SceneNodeData, Skeleton3D, UiBox,
};
use perro_structs::{
    BitMask, IntoBitMaskLayer, Quaternion, Transform2D, Transform3D, Vector2, Vector3,
};
use std::borrow::Cow;

fn default_node_data<T>() -> SceneNodeData
where
    T: Default + Into<SceneNodeData>,
{
    T::default().into()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum QueryScope {
    #[default]
    Root,
    Subtree(NodeID),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueryExpr {
    All(Vec<QueryExpr>),
    Any(Vec<QueryExpr>),
    Not(Box<QueryExpr>),
    Name(Vec<String>),
    Tags(Vec<TagID>),
    IsType(Vec<NodeType>),
    BaseType(Vec<NodeType>),
    IsTypeMask(QueryTypeMask),
    BaseTypeMask(QueryTypeMask),
    Layers(BitMask),
    Mask(BitMask),
}

pub const QUERY_TYPE_MASK_WORDS: usize = NodeType::ALL.len().div_ceil(64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueryTypeMask {
    bits: [u64; QUERY_TYPE_MASK_WORDS],
}

impl QueryTypeMask {
    pub const NONE: Self = Self {
        bits: [0; QUERY_TYPE_MASK_WORDS],
    };

    pub const fn from_bits(bits: [u64; QUERY_TYPE_MASK_WORDS]) -> Self {
        Self { bits }
    }

    pub const fn bits(self) -> [u64; QUERY_TYPE_MASK_WORDS] {
        self.bits
    }

    pub const fn all() -> Self {
        let mut mask = Self::NONE;
        let mut index = 0;
        while index < NodeType::ALL.len() {
            mask = mask.with_type(NodeType::ALL[index]);
            index += 1;
        }
        mask
    }

    pub const fn with_type(mut self, node_type: NodeType) -> Self {
        let bit_index = node_type as usize;
        let word = bit_index / 64;
        let bit = bit_index % 64;
        if word < QUERY_TYPE_MASK_WORDS {
            self.bits[word] |= 1_u64 << bit;
        }
        self
    }

    pub const fn contains_type(self, node_type: NodeType) -> bool {
        let bit_index = node_type as usize;
        let word = bit_index / 64;
        let bit = bit_index % 64;
        word < QUERY_TYPE_MASK_WORDS && (self.bits[word] & (1_u64 << bit)) != 0
    }

    pub const fn union(self, other: Self) -> Self {
        let mut out = Self::NONE;
        let mut index = 0;
        while index < QUERY_TYPE_MASK_WORDS {
            out.bits[index] = self.bits[index] | other.bits[index];
            index += 1;
        }
        out
    }

    pub const fn intersection(self, other: Self) -> Self {
        let mut out = Self::NONE;
        let mut index = 0;
        while index < QUERY_TYPE_MASK_WORDS {
            out.bits[index] = self.bits[index] & other.bits[index];
            index += 1;
        }
        out
    }

    pub const fn complement(self) -> Self {
        let all = Self::all();
        let mut out = Self::NONE;
        let mut index = 0;
        while index < QUERY_TYPE_MASK_WORDS {
            out.bits[index] = all.bits[index] & !self.bits[index];
            index += 1;
        }
        out
    }

    pub const fn is_empty(self) -> bool {
        let mut index = 0;
        while index < QUERY_TYPE_MASK_WORDS {
            if self.bits[index] != 0 {
                return false;
            }
            index += 1;
        }
        true
    }
}

/// Query definition used by [`query!`](macro@crate::query) to filter nodes.
#[derive(Clone, Debug, Default)]
pub struct NodeQuery {
    pub expr: Option<QueryExpr>,
    pub scope: QueryScope,
}

#[derive(Clone, Copy, Debug)]
pub struct NodeQueryView<'a> {
    pub expr: &'a Option<QueryExpr>,
    pub scope: QueryScope,
}

impl<'a> NodeQueryView<'a> {
    pub const fn in_subtree(mut self, parent_id: NodeID) -> Self {
        self.scope = QueryScope::Subtree(parent_id);
        self
    }
}

/// Converts a single tag into stored node tag data.
pub trait IntoNodeTag {
    fn into_node_tag(self) -> NodeTag;
}

/// Converts a single tag or tag collection into stored node tag data.
///
/// Used by [`tag_add!`](macro@crate::tag_add) to support one-or-many inputs.
pub trait IntoNodeTags {
    fn into_node_tags(self) -> Vec<NodeTag>;
}

/// Data used by [`create_nodes!`](macro@crate::create_nodes) for batch node creation.
#[derive(Clone, Debug)]
pub struct NodeCreationTemplate {
    pub node_type: NodeType,
    pub name: Option<Cow<'static, str>>,
    pub tags: Vec<NodeTag>,
    factory: fn() -> SceneNodeData,
}

impl NodeCreationTemplate {
    /// Creates a request for a default node of `T`.
    pub fn new<T>() -> Self
    where
        T: Default + Into<SceneNodeData> + NodeTypeDispatch,
    {
        Self {
            node_type: T::NODE_TYPE,
            name: None,
            tags: Vec::new(),
            factory: default_node_data::<T>,
        }
    }

    /// Sets display name.
    pub fn name<S>(mut self, name: S) -> Self
    where
        S: Into<Cow<'static, str>>,
    {
        self.name = Some(name.into());
        self
    }

    /// Sets tags.
    pub fn tags<T>(mut self, tags: T) -> Self
    where
        T: IntoNodeTags,
    {
        self.tags = tags.into_node_tags();
        self
    }

    /// Builds node payload.
    pub fn scene_node_data(&self) -> SceneNodeData {
        (self.factory)()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChildSelector {
    Index(usize),
    Name(Cow<'static, str>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshSurfaceHit3D {
    /// Instance index for `MultiMeshInstance3D` (always `0` for `MeshInstance3D`).
    pub instance_index: u32,
    /// Surface index on the resolved mesh.
    pub surface_index: u32,
    /// Material bound on the resolved surface.
    pub material: Option<MaterialID>,
    /// Nearest point on the surface in global space.
    pub global_point: Vector3,
    /// Nearest point on the surface in mesh-local space.
    pub local_point: Vector3,
    /// Surface normal at the hit in global space.
    pub global_normal: Vector3,
    /// Surface normal at the hit in mesh-local space.
    pub local_normal: Vector3,
    /// Distance from query point to nearest surface point.
    pub distance: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshSurfaceRay3D {
    pub origin: Vector3,
    pub direction: Vector3,
    pub max_distance: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshMaterialRegion3D {
    pub instance_index: u32,
    pub surface_index: u32,
    pub material: Option<MaterialID>,
    pub triangle_count: u32,
    pub center_global: Vector3,
    pub center_local: Vector3,
    pub aabb_min_global: Vector3,
    pub aabb_max_global: Vector3,
    pub aabb_min_local: Vector3,
    pub aabb_max_local: Vector3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshDataSurfaceHit3D {
    pub surface_index: u32,
    pub local_point: Vector3,
    pub local_normal: Vector3,
    pub distance: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshDataSurfaceRegion3D {
    pub surface_index: u32,
    pub triangle_count: u32,
    pub center_local: Vector3,
    pub aabb_min_local: Vector3,
    pub aabb_max_local: Vector3,
}

pub trait IntoChildSelector {
    fn into_child_selector(self) -> ChildSelector;
}

impl IntoChildSelector for usize {
    fn into_child_selector(self) -> ChildSelector {
        ChildSelector::Index(self)
    }
}

impl IntoChildSelector for &str {
    fn into_child_selector(self) -> ChildSelector {
        ChildSelector::Name(Cow::Owned(self.to_string()))
    }
}

impl IntoChildSelector for String {
    fn into_child_selector(self) -> ChildSelector {
        ChildSelector::Name(Cow::Owned(self))
    }
}

impl IntoChildSelector for &String {
    fn into_child_selector(self) -> ChildSelector {
        ChildSelector::Name(Cow::Owned(self.clone()))
    }
}

impl IntoChildSelector for Cow<'static, str> {
    fn into_child_selector(self) -> ChildSelector {
        ChildSelector::Name(self)
    }
}

impl IntoChildSelector for &Cow<'static, str> {
    fn into_child_selector(self) -> ChildSelector {
        ChildSelector::Name(self.clone())
    }
}

impl IntoNodeTag for NodeTag {
    fn into_node_tag(self) -> NodeTag {
        self
    }
}

impl IntoNodeTag for &NodeTag {
    fn into_node_tag(self) -> NodeTag {
        self.clone()
    }
}

impl IntoNodeTag for TagID {
    fn into_node_tag(self) -> NodeTag {
        NodeTag {
            id: self,
            name: Cow::Borrowed(""),
        }
    }
}

impl IntoNodeTag for &TagID {
    fn into_node_tag(self) -> NodeTag {
        (*self).into_node_tag()
    }
}

impl IntoNodeTag for &str {
    fn into_node_tag(self) -> NodeTag {
        NodeTag::new(self.to_string())
    }
}

impl IntoNodeTag for String {
    fn into_node_tag(self) -> NodeTag {
        NodeTag::new(self)
    }
}

impl IntoNodeTag for &String {
    fn into_node_tag(self) -> NodeTag {
        NodeTag::new(self.clone())
    }
}

impl IntoNodeTags for NodeTag {
    fn into_node_tags(self) -> Vec<NodeTag> {
        vec![self]
    }
}

impl IntoNodeTags for &NodeTag {
    fn into_node_tags(self) -> Vec<NodeTag> {
        vec![self.clone()]
    }
}

impl IntoNodeTags for TagID {
    fn into_node_tags(self) -> Vec<NodeTag> {
        vec![self.into_node_tag()]
    }
}

impl IntoNodeTags for &TagID {
    fn into_node_tags(self) -> Vec<NodeTag> {
        vec![self.into_node_tag()]
    }
}

impl IntoNodeTags for &str {
    fn into_node_tags(self) -> Vec<NodeTag> {
        vec![self.into_node_tag()]
    }
}

impl IntoNodeTags for String {
    fn into_node_tags(self) -> Vec<NodeTag> {
        vec![self.into_node_tag()]
    }
}

impl IntoNodeTags for &String {
    fn into_node_tags(self) -> Vec<NodeTag> {
        vec![self.into_node_tag()]
    }
}

impl IntoNodeTags for &[TagID] {
    fn into_node_tags(self) -> Vec<NodeTag> {
        self.iter().map(IntoNodeTag::into_node_tag).collect()
    }
}

impl<const N: usize> IntoNodeTags for [TagID; N] {
    fn into_node_tags(self) -> Vec<NodeTag> {
        self.into_iter().map(IntoNodeTag::into_node_tag).collect()
    }
}

impl<const N: usize> IntoNodeTags for &[TagID; N] {
    fn into_node_tags(self) -> Vec<NodeTag> {
        self.as_slice()
            .iter()
            .map(IntoNodeTag::into_node_tag)
            .collect()
    }
}

impl IntoNodeTags for Vec<TagID> {
    fn into_node_tags(self) -> Vec<NodeTag> {
        self.into_iter().map(IntoNodeTag::into_node_tag).collect()
    }
}

impl IntoNodeTags for &[NodeTag] {
    fn into_node_tags(self) -> Vec<NodeTag> {
        self.to_vec()
    }
}

impl<const N: usize> IntoNodeTags for [NodeTag; N] {
    fn into_node_tags(self) -> Vec<NodeTag> {
        self.into_iter().collect()
    }
}

impl<const N: usize> IntoNodeTags for &[NodeTag; N] {
    fn into_node_tags(self) -> Vec<NodeTag> {
        self.as_slice().to_vec()
    }
}

impl IntoNodeTags for Vec<NodeTag> {
    fn into_node_tags(self) -> Vec<NodeTag> {
        self
    }
}

impl IntoNodeTags for &[&str] {
    fn into_node_tags(self) -> Vec<NodeTag> {
        self.iter().map(|tag| (*tag).into_node_tag()).collect()
    }
}

impl<const N: usize> IntoNodeTags for [&str; N] {
    fn into_node_tags(self) -> Vec<NodeTag> {
        self.into_iter().map(IntoNodeTag::into_node_tag).collect()
    }
}

impl<const N: usize> IntoNodeTags for &[&str; N] {
    fn into_node_tags(self) -> Vec<NodeTag> {
        self.as_slice()
            .iter()
            .map(|tag| (*tag).into_node_tag())
            .collect()
    }
}

impl IntoNodeTags for Vec<&str> {
    fn into_node_tags(self) -> Vec<NodeTag> {
        self.into_iter().map(IntoNodeTag::into_node_tag).collect()
    }
}

impl IntoNodeTags for &[String] {
    fn into_node_tags(self) -> Vec<NodeTag> {
        self.iter().map(IntoNodeTag::into_node_tag).collect()
    }
}

impl<const N: usize> IntoNodeTags for [String; N] {
    fn into_node_tags(self) -> Vec<NodeTag> {
        self.into_iter().map(IntoNodeTag::into_node_tag).collect()
    }
}

impl<const N: usize> IntoNodeTags for &[String; N] {
    fn into_node_tags(self) -> Vec<NodeTag> {
        self.as_slice()
            .iter()
            .map(IntoNodeTag::into_node_tag)
            .collect()
    }
}

impl IntoNodeTags for Vec<String> {
    fn into_node_tags(self) -> Vec<NodeTag> {
        self.into_iter().map(IntoNodeTag::into_node_tag).collect()
    }
}

impl NodeQuery {
    /// Creates an empty query (matches all nodes).
    pub const fn new() -> Self {
        Self {
            expr: None,
            scope: QueryScope::Root,
        }
    }

    fn and_expr(mut self, expr: QueryExpr) -> Self {
        self.expr = Some(match self.expr {
            None => expr,
            Some(existing) => QueryExpr::All(vec![existing, expr]),
        });
        self
    }

    /// Adds names to the query as a single OR group.
    pub fn name<I, T>(self, names: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        self.and_expr(QueryExpr::Name(names.into_iter().map(Into::into).collect()))
    }

    /// Adds tags as a single OR-group.
    pub fn tags<I, T>(self, tags: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: IntoTagID,
    {
        self.and_expr(QueryExpr::Tags(
            tags.into_iter().map(IntoTagID::into_tag_id).collect(),
        ))
    }

    /// Adds exact type filters.
    ///
    /// Match succeeds if node's concrete type is any one of these.
    pub fn node_type<I>(self, types: I) -> Self
    where
        I: IntoIterator<Item = NodeType>,
    {
        self.and_expr(QueryExpr::IsType(types.into_iter().collect()))
    }

    /// Adds base/inclusive type filters.
    ///
    /// Match succeeds if node's concrete type `is_a` any one of these.
    pub fn base_type<I>(self, types: I) -> Self
    where
        I: IntoIterator<Item = NodeType>,
    {
        self.and_expr(QueryExpr::BaseType(types.into_iter().collect()))
    }

    /// Adds render layer filters for 2D/3D nodes.
    ///
    /// Match succeeds when node render layers intersect any requested layer.
    pub fn layers<I, L>(self, layers: I) -> Self
    where
        I: IntoIterator<Item = L>,
        L: IntoBitMaskLayer,
    {
        self.and_expr(QueryExpr::Layers(BitMask::from_layers(layers)))
    }

    /// Adds render layer mask filters for 2D/3D nodes.
    ///
    /// Match succeeds when node render layers do not intersect any masked layer.
    pub fn mask<I, L>(self, layers: I) -> Self
    where
        I: IntoIterator<Item = L>,
        L: IntoBitMaskLayer,
    {
        self.and_expr(QueryExpr::Mask(BitMask::from_layers(layers)))
    }

    /// Adds an explicit expression tree.
    pub fn where_expr(self, expr: QueryExpr) -> Self {
        self.and_expr(expr)
    }

    /// Restricts query traversal to a subtree.
    pub fn in_subtree(mut self, parent_id: NodeID) -> Self {
        self.scope = QueryScope::Subtree(parent_id);
        self
    }

    pub const fn as_view(&self) -> NodeQueryView<'_> {
        NodeQueryView {
            expr: &self.expr,
            scope: self.scope,
        }
    }
}

#[doc(hidden)]
pub const fn __query_type_mask(types: &[NodeType]) -> QueryTypeMask {
    let mut mask = QueryTypeMask::NONE;
    let mut index = 0;
    while index < types.len() {
        mask = mask.with_type(types[index]);
        index += 1;
    }
    mask
}

#[doc(hidden)]
pub const fn __query_all_type_mask() -> QueryTypeMask {
    QueryTypeMask::all()
}

#[doc(hidden)]
pub const fn __query_base_type_mask(base_types: &[NodeType]) -> QueryTypeMask {
    if base_types.is_empty() {
        return __query_all_type_mask();
    }

    let mut mask = QueryTypeMask::NONE;
    let mut type_index = 0;
    while type_index < NodeType::ALL.len() {
        let node_type = NodeType::ALL[type_index];
        let mut base_index = 0;
        while base_index < base_types.len() {
            if node_type.is_a(base_types[base_index]) {
                mask = mask.with_type(node_type);
                break;
            }
            base_index += 1;
        }
        type_index += 1;
    }
    mask
}

pub trait NodeAPI {
    /// Creates a new node with default value of `T`.
    fn create<T>(&mut self) -> NodeID
    where
        T: Default + Into<SceneNodeData>;

    /// Creates many nodes and optionally attaches them under one parent.
    ///
    /// Returns created IDs in request order.
    fn create_nodes(&mut self, requests: &[NodeCreationTemplate], parent_id: NodeID)
    -> Vec<NodeID>;

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

    /// Returns skeleton bone name by index.
    fn get_skeleton_bone_name(
        &mut self,
        skeleton_id: NodeID,
        bone_index: usize,
    ) -> Option<Cow<'static, str>> {
        self.with_node::<Skeleton3D, _>(skeleton_id, |skeleton| {
            skeleton
                .bone_name(bone_index)
                .map(|name| Cow::Owned(name.to_string()))
        })
    }

    /// Returns first skeleton bone index matching name.
    fn get_skeleton_bone_index<S>(&mut self, skeleton_id: NodeID, bone_name: S) -> Option<usize>
    where
        S: AsRef<str>,
    {
        self.with_node::<Skeleton3D, _>(skeleton_id, |skeleton| {
            skeleton.bone_index(bone_name.as_ref())
        })
    }

    /// Sets UI rotation in radians. Works on `UiBox` and descendants.
    fn set_ui_rotation(&mut self, node_id: NodeID, rotation: f32) -> bool {
        self.with_base_node_mut::<UiBox, _, _>(node_id, |node| {
            node.transform.rotation = rotation;
        })
        .is_some()
    }

    /// Binds a UI text node's main text field to a localization key.
    ///
    /// Works on `UiLabel.text`, `UiTextBox.text`, and `UiTextBlock.text`.
    fn bind_locale_text<S>(&mut self, node_id: NodeID, key: S) -> bool
    where
        S: AsRef<str>;

    /// Binds a text-edit node's placeholder field to a localization key.
    ///
    /// Works on `UiTextBox.placeholder` and `UiTextBlock.placeholder`.
    fn bind_locale_placeholder<S>(&mut self, node_id: NodeID, key: S) -> bool
    where
        S: AsRef<str>;

    /// Returns parent node id if node exists.
    fn get_node_parent_id(&mut self, node_id: NodeID) -> Option<NodeID>;

    /// Returns children ids if node exists.
    fn get_node_children_ids(&mut self, node_id: NodeID) -> Option<Vec<NodeID>>;

    /// Returns direct children ids. Invalid parent returns empty vec.
    fn get_children(&mut self, node_id: NodeID) -> Vec<NodeID> {
        self.get_node_children_ids(node_id).unwrap_or_default()
    }

    /// Returns direct child by index.
    fn get_child_at(&mut self, parent_id: NodeID, index: usize) -> Option<NodeID> {
        self.get_node_children_ids(parent_id)
            .and_then(|children| children.into_iter().nth(index))
    }

    /// Returns first direct child matching name.
    fn get_child_by_name<S>(&mut self, parent_id: NodeID, name: S) -> Option<NodeID>
    where
        S: AsRef<str>,
    {
        let target = name.as_ref();
        for child_id in self.get_children(parent_id) {
            if let Some(child_name) = self.get_node_name(child_id)
                && child_name.as_ref() == target
            {
                return Some(child_id);
            }
        }
        None
    }

    /// Returns all direct children matching name.
    fn get_children_by_name<S>(&mut self, parent_id: NodeID, name: S) -> Vec<NodeID>
    where
        S: AsRef<str>,
    {
        let target = name.as_ref();
        let mut out = Vec::new();
        for child_id in self.get_children(parent_id) {
            if let Some(child_name) = self.get_node_name(child_id)
                && child_name.as_ref() == target
            {
                out.push(child_id);
            }
        }
        out
    }

    /// Returns direct child selected by index or name.
    fn get_child<T>(&mut self, parent_id: NodeID, selector: T) -> Option<NodeID>
    where
        T: IntoChildSelector,
    {
        match selector.into_child_selector() {
            ChildSelector::Index(index) => self.get_child_at(parent_id, index),
            ChildSelector::Name(name) => self.get_child_by_name(parent_id, name),
        }
    }

    /// Returns concrete runtime node type if node exists.
    fn get_node_type(&mut self, node_id: NodeID) -> Option<NodeType>;

    /// Reparents a child under parent. `parent_id = nil` detaches to root.
    fn reparent(&mut self, parent_id: NodeID, child_id: NodeID) -> bool;

    /// Marks one node + all descendants dirty for render extraction this frame.
    fn force_rerender(&mut self, root_id: NodeID) -> bool;

    /// Marks one node dirty for render extraction this frame.
    fn mark_needs_rerender(&mut self, node_id: NodeID) -> bool;

    /// Returns true when a MeshInstance3D/MultiMeshInstance3D has a retained draw
    /// using loaded mesh and material resources.
    fn is_mesh_instance_ready(&mut self, node_id: NodeID) -> bool;

    /// Batch reparent. Returns count of successful operations.
    fn reparent_multi<I>(&mut self, parent_id: NodeID, child_ids: I) -> usize
    where
        I: IntoIterator<Item = NodeID>;

    /// Removes a node from the scene graph.
    fn remove_node(&mut self, node_id: NodeID) -> bool;

    /// Returns node tag names if node exists.
    fn get_node_tags(&mut self, node_id: NodeID) -> Option<Vec<Cow<'static, str>>>;

    /// Sets node tags (`Some`) or clears all tags (`None`).
    fn tag_set<T>(&mut self, node_id: NodeID, tags: Option<T>) -> bool
    where
        T: IntoNodeTags;

    /// Adds one tag to node (idempotent).
    fn add_node_tag<T>(&mut self, node_id: NodeID, tag: T) -> bool
    where
        T: IntoNodeTag;

    /// Removes one tag from node.
    fn remove_node_tag<T>(&mut self, node_id: NodeID, tag: T) -> bool
    where
        T: IntoTagID;

    /// Executes a node query and returns matching node IDs.
    fn query_nodes(&mut self, query: NodeQueryView<'_>) -> Vec<NodeID>;

    /// Executes a node query and returns the first matching node ID.
    fn query_first_node(&mut self, query: NodeQueryView<'_>) -> Option<NodeID> {
        self.query_nodes(query).into_iter().next()
    }

    /// Returns the current global transform for a 2D spatial node.
    fn get_global_transform_2d(&mut self, node_id: NodeID) -> Option<Transform2D>;

    /// Returns the current global transform for a 3D spatial node.
    fn get_global_transform_3d(&mut self, node_id: NodeID) -> Option<Transform3D>;

    /// Sets a 2D node's local transform so its resulting global transform matches `global`.
    fn set_global_transform_2d(&mut self, node_id: NodeID, global: Transform2D) -> bool;

    /// Sets a 3D node's local transform so its resulting global transform matches `global`.
    fn set_global_transform_3d(&mut self, node_id: NodeID, global: Transform3D) -> bool;

    /// Converts a point from node-local 2D space to global 2D space.
    fn to_global_point_2d(&mut self, node_id: NodeID, local: Vector2) -> Option<Vector2>;

    /// Converts a point from global 2D space to node-local 2D space.
    fn to_local_point_2d(&mut self, node_id: NodeID, global: Vector2) -> Option<Vector2>;

    /// Converts a point from node-local 3D space to global 3D space.
    fn to_global_point_3d(&mut self, node_id: NodeID, local: Vector3) -> Option<Vector3>;

    /// Converts a point from global 3D space to node-local 3D space.
    fn to_local_point_3d(&mut self, node_id: NodeID, global: Vector3) -> Option<Vector3>;

    /// Converts a local 2D transform (relative to `node_id`) into global space.
    fn to_global_transform_2d(
        &mut self,
        node_id: NodeID,
        local: Transform2D,
    ) -> Option<Transform2D>;

    /// Converts a global 2D transform into local space relative to `node_id`.
    fn to_local_transform_2d(
        &mut self,
        node_id: NodeID,
        global: Transform2D,
    ) -> Option<Transform2D>;

    /// Converts a local 3D transform (relative to `node_id`) into global space.
    fn to_global_transform_3d(
        &mut self,
        node_id: NodeID,
        local: Transform3D,
    ) -> Option<Transform3D>;

    /// Converts a global 3D transform into local space relative to `node_id`.
    fn to_local_transform_3d(
        &mut self,
        node_id: NodeID,
        global: Transform3D,
    ) -> Option<Transform3D>;

    /// Finds mesh-instance surface nearest to global-space point for a 3D mesh node.
    ///
    /// Returns `None` when:
    /// - node does not exist
    /// - node is not a mesh-bearing 3D node
    /// - mesh source cannot be resolved/decoded
    fn mesh_instance_surface_at_global_point(
        &mut self,
        node_id: NodeID,
        global_point: Vector3,
    ) -> Option<MeshSurfaceHit3D>;

    /// Finds the first mesh surface hit along a global-space ray for a 3D mesh node.
    ///
    /// `ray_direction` does not need to be normalized.
    /// Returns `None` when:
    /// - node does not exist
    /// - node is not a mesh-bearing 3D node
    /// - mesh source cannot be resolved/decoded
    /// - ray misses all triangles within `max_distance`
    fn mesh_instance_surface_on_global_ray(
        &mut self,
        node_id: NodeID,
        ray_origin: Vector3,
        ray_direction: Vector3,
        max_distance: f32,
    ) -> Option<MeshSurfaceHit3D>;

    /// Finds mesh surface hits for many global-space rays against the same mesh node.
    ///
    /// Reuses node lookup, mesh decode/cache lookup, node global transform, and instance data
    /// across all rays. `resolve_material=false` skips material lookup and leaves hit material
    /// as `None`, useful when scripts only need `surface_index`.
    fn mesh_instance_surfaces_on_global_rays(
        &mut self,
        node_id: NodeID,
        rays: &[MeshSurfaceRay3D],
        resolve_material: bool,
    ) -> Vec<Option<MeshSurfaceHit3D>>;

    /// Returns regions (one per matching surface) where `material` exists on a mesh node.
    ///
    /// Region bounds/centers are coarse geometric summaries for gameplay queries.
    fn mesh_instance_material_regions(
        &mut self,
        node_id: NodeID,
        material: MaterialID,
    ) -> Vec<MeshMaterialRegion3D>;

    /// Finds raw mesh-data surface nearest to mesh-local point.
    ///
    /// Uses mesh data directly, with no node transform, instances, global values, or material resolve.
    fn mesh_data_surface_at_local_point(
        &mut self,
        mesh_id: MeshID,
        local_point: Vector3,
    ) -> Option<MeshDataSurfaceHit3D>;

    /// Finds raw mesh-data surface hit on mesh-local ray.
    ///
    /// Uses mesh data directly, with no node transform, instances, global values, or material resolve.
    fn mesh_data_surface_on_local_ray(
        &mut self,
        mesh_id: MeshID,
        ray_origin_local: Vector3,
        ray_direction_local: Vector3,
        max_distance: f32,
    ) -> Option<MeshDataSurfaceHit3D>;

    /// Returns regions for one raw mesh-data surface index.
    fn mesh_data_surface_regions(
        &mut self,
        mesh_id: MeshID,
        surface_index: u32,
    ) -> Vec<MeshDataSurfaceRegion3D>;
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

    pub fn create_nodes(
        &mut self,
        requests: &[NodeCreationTemplate],
        parent_id: NodeID,
    ) -> Vec<NodeID> {
        self.rt.create_nodes(requests, parent_id)
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

    pub fn get_skeleton_bone_name(
        &mut self,
        skeleton_id: NodeID,
        bone_index: usize,
    ) -> Option<Cow<'static, str>> {
        self.rt.get_skeleton_bone_name(skeleton_id, bone_index)
    }

    pub fn get_skeleton_bone_index<S>(&mut self, skeleton_id: NodeID, bone_name: S) -> Option<usize>
    where
        S: AsRef<str>,
    {
        self.rt.get_skeleton_bone_index(skeleton_id, bone_name)
    }

    pub fn set_ui_rotation(&mut self, node_id: NodeID, rotation: f32) -> bool {
        self.rt.set_ui_rotation(node_id, rotation)
    }

    pub fn bind_locale_text<S>(&mut self, node_id: NodeID, key: S) -> bool
    where
        S: AsRef<str>,
    {
        self.rt.bind_locale_text(node_id, key)
    }

    pub fn bind_locale_placeholder<S>(&mut self, node_id: NodeID, key: S) -> bool
    where
        S: AsRef<str>,
    {
        self.rt.bind_locale_placeholder(node_id, key)
    }

    pub fn get_node_parent_id(&mut self, node_id: NodeID) -> Option<NodeID> {
        self.rt.get_node_parent_id(node_id)
    }

    pub fn get_node_children_ids(&mut self, node_id: NodeID) -> Option<Vec<NodeID>> {
        self.rt.get_node_children_ids(node_id)
    }

    pub fn get_children(&mut self, node_id: NodeID) -> Vec<NodeID> {
        self.get_node_children_ids(node_id).unwrap_or_default()
    }

    pub fn get_child_at(&mut self, parent_id: NodeID, index: usize) -> Option<NodeID> {
        self.get_children(parent_id).into_iter().nth(index)
    }

    pub fn get_child_by_name<S>(&mut self, parent_id: NodeID, name: S) -> Option<NodeID>
    where
        S: AsRef<str>,
    {
        let target = name.as_ref();
        for child_id in self.get_children(parent_id) {
            if let Some(child_name) = self.get_node_name(child_id)
                && child_name.as_ref() == target
            {
                return Some(child_id);
            }
        }
        None
    }

    pub fn get_children_by_name<S>(&mut self, parent_id: NodeID, name: S) -> Vec<NodeID>
    where
        S: AsRef<str>,
    {
        let target = name.as_ref();
        let mut out = Vec::new();
        for child_id in self.get_children(parent_id) {
            if let Some(child_name) = self.get_node_name(child_id)
                && child_name.as_ref() == target
            {
                out.push(child_id);
            }
        }
        out
    }

    pub fn get_child<T>(&mut self, parent_id: NodeID, selector: T) -> Option<NodeID>
    where
        T: IntoChildSelector,
    {
        match selector.into_child_selector() {
            ChildSelector::Index(index) => self.get_child_at(parent_id, index),
            ChildSelector::Name(name) => self.get_child_by_name(parent_id, name),
        }
    }

    pub fn get_node_type(&mut self, node_id: NodeID) -> Option<NodeType> {
        self.rt.get_node_type(node_id)
    }

    pub fn reparent(&mut self, parent_id: NodeID, child_id: NodeID) -> bool {
        self.rt.reparent(parent_id, child_id)
    }

    pub fn force_rerender(&mut self, root_id: NodeID) -> bool {
        self.rt.force_rerender(root_id)
    }

    pub fn mark_needs_rerender(&mut self, node_id: NodeID) -> bool {
        self.rt.mark_needs_rerender(node_id)
    }

    pub fn is_mesh_instance_ready(&mut self, node_id: NodeID) -> bool {
        self.rt.is_mesh_instance_ready(node_id)
    }

    pub fn reparent_multi<I>(&mut self, parent_id: NodeID, child_ids: I) -> usize
    where
        I: IntoIterator<Item = NodeID>,
    {
        self.rt.reparent_multi(parent_id, child_ids)
    }

    pub fn remove_node(&mut self, node_id: NodeID) -> bool {
        self.rt.remove_node(node_id)
    }

    pub fn get_node_tags(&mut self, node_id: NodeID) -> Option<Vec<Cow<'static, str>>> {
        self.rt.get_node_tags(node_id)
    }

    pub fn tag_set<T>(&mut self, node_id: NodeID, tags: Option<T>) -> bool
    where
        T: IntoNodeTags,
    {
        self.rt.tag_set(node_id, tags)
    }

    pub fn add_node_tag<T>(&mut self, node_id: NodeID, tag: T) -> bool
    where
        T: IntoNodeTag,
    {
        self.rt.add_node_tag(node_id, tag)
    }

    pub fn add_node_tags<T>(&mut self, node_id: NodeID, tags: T) -> bool
    where
        T: IntoNodeTags,
    {
        let node_tags = tags.into_node_tags();
        if node_tags.is_empty() {
            return true;
        }

        for tag in node_tags {
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

    pub fn get_global_transform_2d(&mut self, node_id: NodeID) -> Option<Transform2D> {
        self.rt.get_global_transform_2d(node_id)
    }

    pub fn get_global_transform_3d(&mut self, node_id: NodeID) -> Option<Transform3D> {
        self.rt.get_global_transform_3d(node_id)
    }

    pub fn get_local_transform_2d(&mut self, node_id: NodeID) -> Option<Transform2D> {
        self.with_base_node::<Node2D, _, _>(node_id, |node| node.transform)
    }

    pub fn get_local_transform_3d(&mut self, node_id: NodeID) -> Option<Transform3D> {
        self.with_base_node::<Node3D, _, _>(node_id, |node| node.transform)
    }

    pub fn set_local_transform_2d(&mut self, node_id: NodeID, transform: Transform2D) -> bool {
        self.with_base_node_mut::<Node2D, _, _>(node_id, |node| {
            node.transform = transform;
        })
        .is_some()
    }

    pub fn set_local_transform_3d(&mut self, node_id: NodeID, transform: Transform3D) -> bool {
        self.with_base_node_mut::<Node3D, _, _>(node_id, |node| {
            node.transform = transform;
        })
        .is_some()
    }

    pub fn set_global_transform_2d(&mut self, node_id: NodeID, global: Transform2D) -> bool {
        self.rt.set_global_transform_2d(node_id, global)
    }

    pub fn set_global_transform_3d(&mut self, node_id: NodeID, global: Transform3D) -> bool {
        self.rt.set_global_transform_3d(node_id, global)
    }

    pub fn get_local_pos_2d(&mut self, node_id: NodeID) -> Option<Vector2> {
        self.get_local_transform_2d(node_id)
            .map(|transform| transform.position)
    }

    pub fn get_local_pos_3d(&mut self, node_id: NodeID) -> Option<Vector3> {
        self.get_local_transform_3d(node_id)
            .map(|transform| transform.position)
    }

    pub fn set_local_pos_2d(&mut self, node_id: NodeID, pos: Vector2) -> bool {
        self.with_base_node_mut::<Node2D, _, _>(node_id, |node| {
            node.transform.position = pos;
        })
        .is_some()
    }

    pub fn set_local_pos_3d(&mut self, node_id: NodeID, pos: Vector3) -> bool {
        self.with_base_node_mut::<Node3D, _, _>(node_id, |node| {
            node.transform.position = pos;
        })
        .is_some()
    }

    pub fn get_global_pos_2d(&mut self, node_id: NodeID) -> Option<Vector2> {
        self.get_global_transform_2d(node_id)
            .map(|transform| transform.position)
    }

    pub fn get_global_pos_3d(&mut self, node_id: NodeID) -> Option<Vector3> {
        self.get_global_transform_3d(node_id)
            .map(|transform| transform.position)
    }

    pub fn set_global_pos_2d(&mut self, node_id: NodeID, pos: Vector2) -> bool {
        let Some(mut transform) = self.get_global_transform_2d(node_id) else {
            return false;
        };
        transform.position = pos;
        self.set_global_transform_2d(node_id, transform)
    }

    pub fn set_global_pos_3d(&mut self, node_id: NodeID, pos: Vector3) -> bool {
        let Some(mut transform) = self.get_global_transform_3d(node_id) else {
            return false;
        };
        transform.position = pos;
        self.set_global_transform_3d(node_id, transform)
    }

    pub fn get_local_rot_2d(&mut self, node_id: NodeID) -> Option<f32> {
        self.get_local_transform_2d(node_id)
            .map(|transform| transform.rotation)
    }

    pub fn get_local_rot_3d(&mut self, node_id: NodeID) -> Option<Quaternion> {
        self.get_local_transform_3d(node_id)
            .map(|transform| transform.rotation)
    }

    pub fn set_local_rot_2d(&mut self, node_id: NodeID, rot: f32) -> bool {
        self.with_base_node_mut::<Node2D, _, _>(node_id, |node| {
            node.transform.rotation = rot;
        })
        .is_some()
    }

    pub fn set_local_rot_3d(&mut self, node_id: NodeID, rot: Quaternion) -> bool {
        self.with_base_node_mut::<Node3D, _, _>(node_id, |node| {
            node.transform.rotation = rot;
        })
        .is_some()
    }

    pub fn get_global_rot_2d(&mut self, node_id: NodeID) -> Option<f32> {
        self.get_global_transform_2d(node_id)
            .map(|transform| transform.rotation)
    }

    pub fn get_global_rot_3d(&mut self, node_id: NodeID) -> Option<Quaternion> {
        self.get_global_transform_3d(node_id)
            .map(|transform| transform.rotation)
    }

    pub fn set_global_rot_2d(&mut self, node_id: NodeID, rot: f32) -> bool {
        let Some(mut transform) = self.get_global_transform_2d(node_id) else {
            return false;
        };
        transform.rotation = rot;
        self.set_global_transform_2d(node_id, transform)
    }

    pub fn set_global_rot_3d(&mut self, node_id: NodeID, rot: Quaternion) -> bool {
        let Some(mut transform) = self.get_global_transform_3d(node_id) else {
            return false;
        };
        transform.rotation = rot;
        self.set_global_transform_3d(node_id, transform)
    }

    pub fn get_local_scale_2d(&mut self, node_id: NodeID) -> Option<Vector2> {
        self.get_local_transform_2d(node_id)
            .map(|transform| transform.scale)
    }

    pub fn get_local_scale_3d(&mut self, node_id: NodeID) -> Option<Vector3> {
        self.get_local_transform_3d(node_id)
            .map(|transform| transform.scale)
    }

    pub fn set_local_scale_2d(&mut self, node_id: NodeID, scale: Vector2) -> bool {
        self.with_base_node_mut::<Node2D, _, _>(node_id, |node| {
            node.transform.scale = scale;
        })
        .is_some()
    }

    pub fn set_local_scale_3d(&mut self, node_id: NodeID, scale: Vector3) -> bool {
        self.with_base_node_mut::<Node3D, _, _>(node_id, |node| {
            node.transform.scale = scale;
        })
        .is_some()
    }

    pub fn get_global_scale_2d(&mut self, node_id: NodeID) -> Option<Vector2> {
        self.get_global_transform_2d(node_id)
            .map(|transform| transform.scale)
    }

    pub fn get_global_scale_3d(&mut self, node_id: NodeID) -> Option<Vector3> {
        self.get_global_transform_3d(node_id)
            .map(|transform| transform.scale)
    }

    pub fn set_global_scale_2d(&mut self, node_id: NodeID, scale: Vector2) -> bool {
        let Some(mut transform) = self.get_global_transform_2d(node_id) else {
            return false;
        };
        transform.scale = scale;
        self.set_global_transform_2d(node_id, transform)
    }

    pub fn set_global_scale_3d(&mut self, node_id: NodeID, scale: Vector3) -> bool {
        let Some(mut transform) = self.get_global_transform_3d(node_id) else {
            return false;
        };
        transform.scale = scale;
        self.set_global_transform_3d(node_id, transform)
    }

    pub fn to_global_point_2d(&mut self, node_id: NodeID, local: Vector2) -> Option<Vector2> {
        self.rt.to_global_point_2d(node_id, local)
    }

    pub fn to_local_point_2d(&mut self, node_id: NodeID, global: Vector2) -> Option<Vector2> {
        self.rt.to_local_point_2d(node_id, global)
    }

    pub fn to_global_point_3d(&mut self, node_id: NodeID, local: Vector3) -> Option<Vector3> {
        self.rt.to_global_point_3d(node_id, local)
    }

    pub fn to_local_point_3d(&mut self, node_id: NodeID, global: Vector3) -> Option<Vector3> {
        self.rt.to_local_point_3d(node_id, global)
    }

    pub fn to_global_transform_2d(
        &mut self,
        node_id: NodeID,
        local: Transform2D,
    ) -> Option<Transform2D> {
        self.rt.to_global_transform_2d(node_id, local)
    }

    pub fn to_local_transform_2d(
        &mut self,
        node_id: NodeID,
        global: Transform2D,
    ) -> Option<Transform2D> {
        self.rt.to_local_transform_2d(node_id, global)
    }

    pub fn to_global_transform_3d(
        &mut self,
        node_id: NodeID,
        local: Transform3D,
    ) -> Option<Transform3D> {
        self.rt.to_global_transform_3d(node_id, local)
    }

    pub fn to_local_transform_3d(
        &mut self,
        node_id: NodeID,
        global: Transform3D,
    ) -> Option<Transform3D> {
        self.rt.to_local_transform_3d(node_id, global)
    }

    pub fn mesh_instance_surface_at_global_point(
        &mut self,
        node_id: NodeID,
        global_point: Vector3,
    ) -> Option<MeshSurfaceHit3D> {
        self.rt
            .mesh_instance_surface_at_global_point(node_id, global_point)
    }

    pub fn mesh_instance_surface_on_global_ray(
        &mut self,
        node_id: NodeID,
        ray_origin: Vector3,
        ray_direction: Vector3,
        max_distance: f32,
    ) -> Option<MeshSurfaceHit3D> {
        self.rt.mesh_instance_surface_on_global_ray(
            node_id,
            ray_origin,
            ray_direction,
            max_distance,
        )
    }

    pub fn mesh_instance_surfaces_on_global_rays(
        &mut self,
        node_id: NodeID,
        rays: &[MeshSurfaceRay3D],
        resolve_material: bool,
    ) -> Vec<Option<MeshSurfaceHit3D>> {
        self.rt
            .mesh_instance_surfaces_on_global_rays(node_id, rays, resolve_material)
    }

    pub fn mesh_instance_material_regions(
        &mut self,
        node_id: NodeID,
        material: MaterialID,
    ) -> Vec<MeshMaterialRegion3D> {
        self.rt.mesh_instance_material_regions(node_id, material)
    }

    pub fn mesh_data_surface_at_local_point(
        &mut self,
        mesh_id: MeshID,
        local_point: Vector3,
    ) -> Option<MeshDataSurfaceHit3D> {
        self.rt
            .mesh_data_surface_at_local_point(mesh_id, local_point)
    }

    pub fn mesh_data_surface_on_local_ray(
        &mut self,
        mesh_id: MeshID,
        ray_origin_local: Vector3,
        ray_direction_local: Vector3,
        max_distance: f32,
    ) -> Option<MeshDataSurfaceHit3D> {
        self.rt.mesh_data_surface_on_local_ray(
            mesh_id,
            ray_origin_local,
            ray_direction_local,
            max_distance,
        )
    }

    pub fn mesh_data_surface_regions(
        &mut self,
        mesh_id: MeshID,
        surface_index: u32,
    ) -> Vec<MeshDataSurfaceRegion3D> {
        self.rt.mesh_data_surface_regions(mesh_id, surface_index)
    }
}

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

pub struct MeshQueryModule<'rt, R: NodeAPI + ?Sized> {
    rt: &'rt mut R,
}

impl<'rt, R: NodeAPI + ?Sized> MeshQueryModule<'rt, R> {
    pub fn new(rt: &'rt mut R) -> Self {
        Self { rt }
    }

    pub fn instance_surface_at_global_point(
        &mut self,
        node_id: NodeID,
        global_point: Vector3,
    ) -> Option<MeshSurfaceHit3D> {
        self.rt
            .mesh_instance_surface_at_global_point(node_id, global_point)
    }

    pub fn instance_surface_on_global_ray(
        &mut self,
        node_id: NodeID,
        ray_origin: Vector3,
        ray_direction: Vector3,
        max_distance: f32,
    ) -> Option<MeshSurfaceHit3D> {
        self.rt.mesh_instance_surface_on_global_ray(
            node_id,
            ray_origin,
            ray_direction,
            max_distance,
        )
    }

    pub fn instance_surfaces_on_global_rays(
        &mut self,
        node_id: NodeID,
        rays: &[MeshSurfaceRay3D],
        resolve_material: bool,
    ) -> Vec<Option<MeshSurfaceHit3D>> {
        self.rt
            .mesh_instance_surfaces_on_global_rays(node_id, rays, resolve_material)
    }

    pub fn instance_material_regions(
        &mut self,
        node_id: NodeID,
        material: MaterialID,
    ) -> Vec<MeshMaterialRegion3D> {
        self.rt.mesh_instance_material_regions(node_id, material)
    }

    pub fn data_surface_at_local_point(
        &mut self,
        mesh_id: MeshID,
        local_point: Vector3,
    ) -> Option<MeshDataSurfaceHit3D> {
        self.rt
            .mesh_data_surface_at_local_point(mesh_id, local_point)
    }

    pub fn data_surface_on_local_ray(
        &mut self,
        mesh_id: MeshID,
        ray_origin_local: Vector3,
        ray_direction_local: Vector3,
        max_distance: f32,
    ) -> Option<MeshDataSurfaceHit3D> {
        self.rt.mesh_data_surface_on_local_ray(
            mesh_id,
            ray_origin_local,
            ray_direction_local,
            max_distance,
        )
    }

    pub fn data_surface_regions(
        &mut self,
        mesh_id: MeshID,
        surface_index: u32,
    ) -> Vec<MeshDataSurfaceRegion3D> {
        self.rt.mesh_data_surface_regions(mesh_id, surface_index)
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
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
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
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
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
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
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
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
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
/// Usage:
/// - `create_node!(ctx, ConcreteType) -> NodeID`
/// - `create_node!(ctx, ConcreteType, name) -> NodeID`
/// - `create_node!(ctx, ConcreteType, name, tags) -> NodeID`
/// - `create_node!(ctx, ConcreteType, name, tags, parent_id) -> NodeID`
/// - `create_nodes!(ctx, requests, parent_id) -> Vec<NodeID>`
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `ConcreteType`: ie Node2D, MeshInstance3D, Sprite2D
/// - `name` (optional): `&str`, `String`, or `Cow<'static, str>`
/// - `tags` (optional): usually from `tags![...]`, or string/id tag collections
/// - `parent_id` (optional): `NodeID`
#[macro_export]
macro_rules! create_node {
    ($ctx:expr, $node_ty:ty) => {
        $ctx.Nodes().create::<$node_ty>()
    };
    ($ctx:expr, $node_ty:ty, $name:expr) => {{
        let __id = $ctx.Nodes().create::<$node_ty>();
        let _ = $ctx.Nodes().set_node_name(__id, $name);
        __id
    }};
    ($ctx:expr, $node_ty:ty, $name:expr, $tags:expr) => {{
        let __id = $ctx.Nodes().create::<$node_ty>();
        let _ = $ctx.Nodes().set_node_name(__id, $name);
        let _ = $ctx.Nodes().tag_set(__id, Some($tags));
        __id
    }};
    ($ctx:expr, $node_ty:ty, $name:expr, $tags:expr, $parent:expr) => {{
        let __id = $ctx.Nodes().create::<$node_ty>();
        let _ = $ctx.Nodes().set_node_name(__id, $name);
        let _ = $ctx.Nodes().tag_set(__id, Some($tags));
        let _ = $ctx.Nodes().reparent($parent, __id);
        __id
    }};
}

/// Creates a batch request for [`create_nodes!`](macro@crate::create_nodes).
/// Usage:
/// - `node_template!(ConcreteType)`
/// - `node_template!(ConcreteType, name)`
/// - `node_template!(ConcreteType, name, tags)`
#[macro_export]
macro_rules! node_template {
    ($node_ty:ty) => {
        $crate::sub_apis::NodeCreationTemplate::new::<$node_ty>()
    };
    ($node_ty:ty, $name:expr) => {
        $crate::sub_apis::NodeCreationTemplate::new::<$node_ty>().name($name)
    };
    ($node_ty:ty, $name:expr, $tags:expr) => {
        $crate::sub_apis::NodeCreationTemplate::new::<$node_ty>()
            .name($name)
            .tags($tags)
    };
}

/// Creates many nodes from [`NodeCreationTemplate`](crate::sub_apis::NodeCreationTemplate).
/// Usage:
/// - `create_nodes!(ctx, requests) -> Vec<NodeID>`
/// - `create_nodes!(ctx, requests, parent_id) -> Vec<NodeID>`
#[macro_export]
macro_rules! create_nodes {
    ($ctx:expr, $requests:expr) => {
        $ctx.Nodes()
            .create_nodes(&$requests, $crate::perro_ids::NodeID::nil())
    };
    ($ctx:expr, $requests:expr, $parent:expr) => {
        $ctx.Nodes().create_nodes(&$requests, $parent)
    };
}

/// SceneNode metadata macros.
///
/// These macros expose node identity/relationship/metadata access:
/// - name (`get_node_name!`, `set_node_name!`)
/// - hierarchy (`get_node_parent_id!`, `get_node_children_ids!`)
/// - runtime typing (`get_node_type!`)
/// - tags (`get_node_tags!`, `tag_set!`, `tag_add!`, `tag_remove!`)
/// - global transform helpers (`get_global_transform_*`, `set_global_transform_*`, `to_*`)
///
/// Gets node display name.
/// Usage: `get_node_name!(ctx, node_id) -> Option<Cow<'static, str>>`.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
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
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `node_id`: `NodeID`
/// - `name`: `&str`, `String`, or `Cow<'static, str>`
#[macro_export]
macro_rules! set_node_name {
    ($ctx:expr, $id:expr, $name:expr) => {
        $ctx.Nodes().set_node_name($id, $name)
    };
}

/// Gets skeleton bone name by index.
/// Usage: `get_skeleton_bone_name!(ctx, skeleton_id, bone_index) -> Option<Cow<'static, str>>`.
#[macro_export]
macro_rules! get_skeleton_bone_name {
    ($ctx:expr, $id:expr, $index:expr) => {
        $ctx.Nodes().get_skeleton_bone_name($id, $index)
    };
}

/// Gets first skeleton bone index by name.
/// Usage: `get_skeleton_bone_index!(ctx, skeleton_id, bone_name) -> Option<usize>`.
#[macro_export]
macro_rules! get_skeleton_bone_index {
    ($ctx:expr, $id:expr, $name:expr) => {
        $ctx.Nodes().get_skeleton_bone_index($id, $name)
    };
}

#[macro_export]
macro_rules! set_ui_rotation {
    ($ctx:expr, $id:expr, $rotation:expr) => {
        $ctx.Nodes().set_ui_rotation($id, $rotation)
    };
}

#[macro_export]
macro_rules! bind_locale_text {
    ($ctx:expr, $id:expr, $key:expr) => {
        $ctx.Nodes().bind_locale_text($id, $key)
    };
}

#[macro_export]
macro_rules! bind_locale_placeholder {
    ($ctx:expr, $id:expr, $key:expr) => {
        $ctx.Nodes().bind_locale_placeholder($id, $key)
    };
}

/// Gets node parent id.
/// Usage: `get_node_parent_id!(ctx, node_id) -> Option<NodeID>`.
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
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
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `node_id`: `NodeID`
#[macro_export]
macro_rules! get_node_children_ids {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_node_children_ids($id)
    };
}

/// Gets direct children ids; invalid parent returns empty vec.
/// Usage: `get_children!(ctx, parent_id) -> Vec<NodeID>`.
#[macro_export]
macro_rules! get_children {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_children($id)
    };
}

/// Gets one direct child by index or name, or many by name.
/// Usage:
/// - `get_child!(ctx, parent_id, 0usize) -> Option<NodeID>`
/// - `get_child!(ctx, parent_id, "Player") -> Option<NodeID>`
/// - `get_child!(ctx, parent_id, all["Enemy"]) -> Vec<NodeID>`
#[macro_export]
macro_rules! get_child {
    ($ctx:expr, $id:expr, all[$name:expr] $(,)?) => {
        $ctx.Nodes().get_children_by_name($id, $name)
    };
    ($ctx:expr, $id:expr, $selector:expr $(,)?) => {
        $ctx.Nodes().get_child($id, $selector)
    };
}

/// Gets concrete runtime node type.
/// Usage: `get_node_type!(ctx, node_id) -> Option<NodeType>`.
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
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
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `parent_id`: `NodeID` (`NodeID::nil()` detaches child)
/// - `child_id`: `NodeID`
#[macro_export]
macro_rules! reparent {
    ($ctx:expr, $parent:expr, $child:expr) => {
        $ctx.Nodes().reparent($parent, $child)
    };
}

/// Marks node subtree dirty for render extraction this frame.
/// Usage: `force_rerender!(ctx, root_id) -> bool`.
#[macro_export]
macro_rules! force_rerender {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().force_rerender($id)
    };
}

/// Checks whether a MeshInstance3D/MultiMeshInstance3D has a ready retained draw.
/// Usage: `is_mesh_instance_ready!(ctx, node_id) -> bool`.
#[macro_export]
macro_rules! is_mesh_instance_ready {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().is_mesh_instance_ready($id)
    };
}

/// Batch reparent.
/// Usage: `reparent_multi!(ctx, parent_id, child_ids_iter) -> usize`.
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `parent_id`: `NodeID` (`NodeID::nil()` detaches)
/// - `child_ids_iter`: iterator of `NodeID`
#[macro_export]
macro_rules! reparent_multi {
    ($ctx:expr, $parent:expr, $child_ids:expr) => {
        $ctx.Nodes().reparent_multi($parent, $child_ids)
    };
}

/// Removes a node from the scene graph.
/// Usage: `remove_node!(ctx, node_id) -> bool`.
#[macro_export]
macro_rules! remove_node {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().remove_node($id)
    };
}

/// Gets global transform for a 2D spatial node.
/// Usage: `get_global_transform_2d!(ctx, node_id) -> Option<Transform2D>`.
#[macro_export]
macro_rules! get_global_transform_2d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_global_transform_2d($id)
    };
}

/// Gets global transform for a 3D spatial node.
/// Usage: `get_global_transform_3d!(ctx, node_id) -> Option<Transform3D>`.
#[macro_export]
macro_rules! get_global_transform_3d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_global_transform_3d($id)
    };
}

/// Gets local transform for a 2D spatial node.
/// Usage: `get_local_transform_2d!(ctx, node_id) -> Option<Transform2D>`.
#[macro_export]
macro_rules! get_local_transform_2d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_local_transform_2d($id)
    };
}

/// Gets local transform for a 3D spatial node.
/// Usage: `get_local_transform_3d!(ctx, node_id) -> Option<Transform3D>`.
#[macro_export]
macro_rules! get_local_transform_3d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_local_transform_3d($id)
    };
}

/// Sets global transform for a 2D spatial node.
/// Usage: `set_global_transform_2d!(ctx, node_id, transform) -> bool`.
#[macro_export]
macro_rules! set_global_transform_2d {
    ($ctx:expr, $id:expr, $transform:expr) => {
        $ctx.Nodes().set_global_transform_2d($id, $transform)
    };
}

/// Sets global transform for a 3D spatial node.
/// Usage: `set_global_transform_3d!(ctx, node_id, transform) -> bool`.
#[macro_export]
macro_rules! set_global_transform_3d {
    ($ctx:expr, $id:expr, $transform:expr) => {
        $ctx.Nodes().set_global_transform_3d($id, $transform)
    };
}

/// Sets local transform for a 2D spatial node.
/// Usage: `set_local_transform_2d!(ctx, node_id, transform) -> bool`.
#[macro_export]
macro_rules! set_local_transform_2d {
    ($ctx:expr, $id:expr, $transform:expr) => {
        $ctx.Nodes().set_local_transform_2d($id, $transform)
    };
}

/// Sets local transform for a 3D spatial node.
/// Usage: `set_local_transform_3d!(ctx, node_id, transform) -> bool`.
#[macro_export]
macro_rules! set_local_transform_3d {
    ($ctx:expr, $id:expr, $transform:expr) => {
        $ctx.Nodes().set_local_transform_3d($id, $transform)
    };
}

/// Gets local position for a 2D spatial node.
/// Usage: `get_local_pos_2d!(ctx, node_id) -> Option<Vector2>`.
#[macro_export]
macro_rules! get_local_pos_2d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_local_pos_2d($id)
    };
}

/// Gets local position for a 3D spatial node.
/// Usage: `get_local_pos_3d!(ctx, node_id) -> Option<Vector3>`.
#[macro_export]
macro_rules! get_local_pos_3d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_local_pos_3d($id)
    };
}

/// Sets local position for a 2D spatial node.
/// Usage: `set_local_pos_2d!(ctx, node_id, pos) -> bool`.
#[macro_export]
macro_rules! set_local_pos_2d {
    ($ctx:expr, $id:expr, $pos:expr) => {
        $ctx.Nodes().set_local_pos_2d($id, $pos)
    };
}

/// Sets local position for a 3D spatial node.
/// Usage: `set_local_pos_3d!(ctx, node_id, pos) -> bool`.
#[macro_export]
macro_rules! set_local_pos_3d {
    ($ctx:expr, $id:expr, $pos:expr) => {
        $ctx.Nodes().set_local_pos_3d($id, $pos)
    };
}

/// Gets global position for a 2D spatial node.
/// Usage: `get_global_pos_2d!(ctx, node_id) -> Option<Vector2>`.
#[macro_export]
macro_rules! get_global_pos_2d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_global_pos_2d($id)
    };
}

/// Gets global position for a 3D spatial node.
/// Usage: `get_global_pos_3d!(ctx, node_id) -> Option<Vector3>`.
#[macro_export]
macro_rules! get_global_pos_3d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_global_pos_3d($id)
    };
}

/// Sets global position for a 2D spatial node.
/// Usage: `set_global_pos_2d!(ctx, node_id, pos) -> bool`.
#[macro_export]
macro_rules! set_global_pos_2d {
    ($ctx:expr, $id:expr, $pos:expr) => {
        $ctx.Nodes().set_global_pos_2d($id, $pos)
    };
}

/// Sets global position for a 3D spatial node.
/// Usage: `set_global_pos_3d!(ctx, node_id, pos) -> bool`.
#[macro_export]
macro_rules! set_global_pos_3d {
    ($ctx:expr, $id:expr, $pos:expr) => {
        $ctx.Nodes().set_global_pos_3d($id, $pos)
    };
}

/// Gets local rotation for a 2D spatial node.
/// Usage: `get_local_rot_2d!(ctx, node_id) -> Option<f32>`.
#[macro_export]
macro_rules! get_local_rot_2d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_local_rot_2d($id)
    };
}

/// Gets local rotation for a 3D spatial node.
/// Usage: `get_local_rot_3d!(ctx, node_id) -> Option<Quaternion>`.
#[macro_export]
macro_rules! get_local_rot_3d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_local_rot_3d($id)
    };
}

/// Sets local rotation for a 2D spatial node.
/// Usage: `set_local_rot_2d!(ctx, node_id, rot) -> bool`.
#[macro_export]
macro_rules! set_local_rot_2d {
    ($ctx:expr, $id:expr, $rot:expr) => {
        $ctx.Nodes().set_local_rot_2d($id, $rot)
    };
}

/// Sets local rotation for a 3D spatial node.
/// Usage: `set_local_rot_3d!(ctx, node_id, rot) -> bool`.
#[macro_export]
macro_rules! set_local_rot_3d {
    ($ctx:expr, $id:expr, $rot:expr) => {
        $ctx.Nodes().set_local_rot_3d($id, $rot)
    };
}

/// Gets global rotation for a 2D spatial node.
/// Usage: `get_global_rot_2d!(ctx, node_id) -> Option<f32>`.
#[macro_export]
macro_rules! get_global_rot_2d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_global_rot_2d($id)
    };
}

/// Gets global rotation for a 3D spatial node.
/// Usage: `get_global_rot_3d!(ctx, node_id) -> Option<Quaternion>`.
#[macro_export]
macro_rules! get_global_rot_3d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_global_rot_3d($id)
    };
}

/// Sets global rotation for a 2D spatial node.
/// Usage: `set_global_rot_2d!(ctx, node_id, rot) -> bool`.
#[macro_export]
macro_rules! set_global_rot_2d {
    ($ctx:expr, $id:expr, $rot:expr) => {
        $ctx.Nodes().set_global_rot_2d($id, $rot)
    };
}

/// Sets global rotation for a 3D spatial node.
/// Usage: `set_global_rot_3d!(ctx, node_id, rot) -> bool`.
#[macro_export]
macro_rules! set_global_rot_3d {
    ($ctx:expr, $id:expr, $rot:expr) => {
        $ctx.Nodes().set_global_rot_3d($id, $rot)
    };
}

/// Gets local scale for a 2D spatial node.
/// Usage: `get_local_scale_2d!(ctx, node_id) -> Option<Vector2>`.
#[macro_export]
macro_rules! get_local_scale_2d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_local_scale_2d($id)
    };
}

/// Gets local scale for a 3D spatial node.
/// Usage: `get_local_scale_3d!(ctx, node_id) -> Option<Vector3>`.
#[macro_export]
macro_rules! get_local_scale_3d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_local_scale_3d($id)
    };
}

/// Sets local scale for a 2D spatial node.
/// Usage: `set_local_scale_2d!(ctx, node_id, scale) -> bool`.
#[macro_export]
macro_rules! set_local_scale_2d {
    ($ctx:expr, $id:expr, $scale:expr) => {
        $ctx.Nodes().set_local_scale_2d($id, $scale)
    };
}

/// Sets local scale for a 3D spatial node.
/// Usage: `set_local_scale_3d!(ctx, node_id, scale) -> bool`.
#[macro_export]
macro_rules! set_local_scale_3d {
    ($ctx:expr, $id:expr, $scale:expr) => {
        $ctx.Nodes().set_local_scale_3d($id, $scale)
    };
}

/// Gets global scale for a 2D spatial node.
/// Usage: `get_global_scale_2d!(ctx, node_id) -> Option<Vector2>`.
#[macro_export]
macro_rules! get_global_scale_2d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_global_scale_2d($id)
    };
}

/// Gets global scale for a 3D spatial node.
/// Usage: `get_global_scale_3d!(ctx, node_id) -> Option<Vector3>`.
#[macro_export]
macro_rules! get_global_scale_3d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_global_scale_3d($id)
    };
}

/// Sets global scale for a 2D spatial node.
/// Usage: `set_global_scale_2d!(ctx, node_id, scale) -> bool`.
#[macro_export]
macro_rules! set_global_scale_2d {
    ($ctx:expr, $id:expr, $scale:expr) => {
        $ctx.Nodes().set_global_scale_2d($id, $scale)
    };
}

/// Sets global scale for a 3D spatial node.
/// Usage: `set_global_scale_3d!(ctx, node_id, scale) -> bool`.
#[macro_export]
macro_rules! set_global_scale_3d {
    ($ctx:expr, $id:expr, $scale:expr) => {
        $ctx.Nodes().set_global_scale_3d($id, $scale)
    };
}

/// Converts local 2D point to global point.
/// Usage: `to_global_point_2d!(ctx, node_id, local_point) -> Option<Vector2>`.
#[macro_export]
macro_rules! to_global_point_2d {
    ($ctx:expr, $id:expr, $point:expr) => {
        $ctx.Nodes().to_global_point_2d($id, $point)
    };
}

/// Converts global 2D point to local point.
/// Usage: `to_local_point_2d!(ctx, node_id, global_point) -> Option<Vector2>`.
#[macro_export]
macro_rules! to_local_point_2d {
    ($ctx:expr, $id:expr, $point:expr) => {
        $ctx.Nodes().to_local_point_2d($id, $point)
    };
}

/// Converts local 3D point to global point.
/// Usage: `to_global_point_3d!(ctx, node_id, local_point) -> Option<Vector3>`.
#[macro_export]
macro_rules! to_global_point_3d {
    ($ctx:expr, $id:expr, $point:expr) => {
        $ctx.Nodes().to_global_point_3d($id, $point)
    };
}

/// Converts global 3D point to local point.
/// Usage: `to_local_point_3d!(ctx, node_id, global_point) -> Option<Vector3>`.
#[macro_export]
macro_rules! to_local_point_3d {
    ($ctx:expr, $id:expr, $point:expr) => {
        $ctx.Nodes().to_local_point_3d($id, $point)
    };
}

/// Converts local 2D transform to global transform.
/// Usage: `to_global_transform_2d!(ctx, node_id, local_transform) -> Option<Transform2D>`.
#[macro_export]
macro_rules! to_global_transform_2d {
    ($ctx:expr, $id:expr, $transform:expr) => {
        $ctx.Nodes().to_global_transform_2d($id, $transform)
    };
}

/// Converts global 2D transform to local transform.
/// Usage: `to_local_transform_2d!(ctx, node_id, global_transform) -> Option<Transform2D>`.
#[macro_export]
macro_rules! to_local_transform_2d {
    ($ctx:expr, $id:expr, $transform:expr) => {
        $ctx.Nodes().to_local_transform_2d($id, $transform)
    };
}

/// Converts local 3D transform to global transform.
/// Usage: `to_global_transform_3d!(ctx, node_id, local_transform) -> Option<Transform3D>`.
#[macro_export]
macro_rules! to_global_transform_3d {
    ($ctx:expr, $id:expr, $transform:expr) => {
        $ctx.Nodes().to_global_transform_3d($id, $transform)
    };
}

/// Converts global 3D transform to local transform.
/// Usage: `to_local_transform_3d!(ctx, node_id, global_transform) -> Option<Transform3D>`.
#[macro_export]
macro_rules! to_local_transform_3d {
    ($ctx:expr, $id:expr, $transform:expr) => {
        $ctx.Nodes().to_local_transform_3d($id, $transform)
    };
}

/// Finds nearest mesh surface at a global-space point for a mesh instance node.
/// Usage: `mesh_instance_surface_at_global_point_3d!(ctx, node_id, global_point) -> Option<MeshSurfaceHit3D>`.
#[macro_export]
macro_rules! mesh_instance_surface_at_global_point_3d {
    ($ctx:expr, $id:expr, $point:expr) => {
        $ctx.MeshQuery()
            .instance_surface_at_global_point($id, $point)
    };
}

/// Finds first mesh surface hit along a global-space ray for a mesh instance node.
/// Usage:
/// `mesh_instance_surface_on_global_ray_3d!(ctx, node_id, ray_origin, ray_direction, max_distance) -> Option<MeshSurfaceHit3D>`.
#[macro_export]
macro_rules! mesh_instance_surface_on_global_ray_3d {
    ($ctx:expr, $id:expr, $origin:expr, $direction:expr, $max_distance:expr) => {
        $ctx.MeshQuery()
            .instance_surface_on_global_ray($id, $origin, $direction, $max_distance)
    };
}

/// Finds mesh surface hits for many global-space rays against one mesh instance node.
/// Usage:
/// `mesh_instance_surfaces_on_global_rays_3d!(ctx, node_id, rays, resolve_material) -> Vec<Option<MeshSurfaceHit3D>>`.
#[macro_export]
macro_rules! mesh_instance_surfaces_on_global_rays_3d {
    ($ctx:expr, $id:expr, $rays:expr, $resolve_material:expr) => {
        $ctx.MeshQuery()
            .instance_surfaces_on_global_rays($id, $rays, $resolve_material)
    };
}

/// Returns mesh instance regions that use the target material.
/// Usage: `mesh_instance_material_regions_3d!(ctx, node_id, material_id) -> Vec<MeshMaterialRegion3D>`.
#[macro_export]
macro_rules! mesh_instance_material_regions_3d {
    ($ctx:expr, $id:expr, $material:expr) => {
        $ctx.MeshQuery().instance_material_regions($id, $material)
    };
}

/// Finds nearest raw mesh-data surface at a mesh-local point.
#[macro_export]
macro_rules! mesh_data_surface_at_local_point_3d {
    ($ctx:expr, $mesh_id:expr, $point_local:expr) => {
        $ctx.MeshQuery()
            .data_surface_at_local_point($mesh_id, $point_local)
    };
}

/// Finds raw mesh-data surface hit on a mesh-local ray.
#[macro_export]
macro_rules! mesh_data_surface_on_local_ray_3d {
    ($ctx:expr, $mesh_id:expr, $origin_local:expr, $direction_local:expr, $max_distance:expr) => {
        $ctx.MeshQuery().data_surface_on_local_ray(
            $mesh_id,
            $origin_local,
            $direction_local,
            $max_distance,
        )
    };
}

/// Returns raw mesh-data regions for one surface index.
#[macro_export]
macro_rules! mesh_data_surface_regions_3d {
    ($ctx:expr, $mesh_id:expr, $surface_index:expr) => {
        $ctx.MeshQuery()
            .data_surface_regions($mesh_id, $surface_index)
    };
}

/// Gets node tags.
/// Usage: `get_node_tags!(ctx, node_id) -> Option<Vec<Cow<'static, str>>>`.
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `node_id`: `NodeID`
#[macro_export]
macro_rules! get_node_tags {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_node_tags($id)
    };
}

/// Sets or clears node tags.
/// Usage:
/// - `tag_set!(ctx, node_id, tags)` where `tags` converts into node tag data.
/// - `tag_set!(ctx, node_id)` clears all tags.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `node_id`: `NodeID`
/// - `tags`: usually from `tags![...]`, or string/id tag collections
#[macro_export]
macro_rules! tag_set {
    ($ctx:expr, $id:expr, $tags:expr) => {
        $ctx.Nodes().tag_set($id, Some($tags))
    };
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes()
            .tag_set::<&'static [$crate::perro_ids::TagID]>($id, None)
    };
}

/// Adds one or more tags to a node.
/// Usage:
/// - `tag_add!(ctx, node_id, "enemy")`
/// - `tag_add!(ctx, node_id, tags!["enemy", "alive"])`
/// - `tag_add!(ctx, node_id, ["enemy", "alive"])`
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
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
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `node_id`: `NodeID`
/// - `tag` (3-arg form): `TagID`, `&str`, or `String`
#[macro_export]
macro_rules! tag_remove {
    ($ctx:expr, $id:expr, $tag:expr) => {
        $ctx.Nodes().remove_node_tag($id, $tag)
    };
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes()
            .tag_set::<&'static [$crate::perro_ids::TagID]>($id, None)
    };
}

/// Builds a query expression without executing it.
#[macro_export]
macro_rules! query_expr {
    ($kind:ident $args:tt $(,)?) => {
        $crate::query!(@expr $kind $args)
    };
}

/// Builds a reusable node query without executing it.
#[macro_export]
macro_rules! query_builder {
    ($kind:ident $args:tt, in_subtree($parent:expr) $(,)?) => {{
        let __expr = $crate::query_expr!($kind $args);
        $crate::sub_apis::NodeQuery::new()
            .where_expr(__expr)
            .in_subtree($parent)
    }};
    ($kind:ident $args:tt $(,)?) => {{
        let __expr = $crate::query_expr!($kind $args);
        $crate::sub_apis::NodeQuery::new().where_expr(__expr)
    }};
}

/// Executes a node query and returns `Vec<NodeID>`.
///
/// Preferred syntax:
/// - `query!(ctx, all(name[...], tags[...], ...))`
/// - `query!(ctx, any(...))`
/// - `query!(ctx, not(...))`
/// - Optional scope: `query!(ctx, all(...), in_subtree(parent_id))`
///
/// Predicate groups:
/// - `name[...]` OR-list of names
/// - `tags[...]` list of tags; interpretation comes from wrapper:
///   `all(tags[...])`, `any(tags[...])`, or `not(tags[...])`
/// - `node_type[...]`
/// - `base_type[...]`
/// - `layers[...]` render layer allow-list for 2D/3D nodes
/// - `mask[...]` render layer deny-list for 2D/3D nodes
///
/// Boolean combinators:
/// - `all(expr, expr, ...)`
/// - `any(expr, expr, ...)`
/// - `not(expr)`
#[macro_export]
///   R is the return type of the underlying API method call this macro expands to.
macro_rules! query {
    ($ctx:expr, tags[$($tag:tt)*], in_subtree($parent:expr) $(,)?) => {{
        let _ = &$ctx;
        let _ = &$parent;
        compile_error!("tags[...] must be wrapped by all(...), any(...), or not(...)");
    }};
    ($ctx:expr, tags[$($tag:tt)*] $(,)?) => {{
        let _ = &$ctx;
        compile_error!("tags[...] must be wrapped by all(...), any(...), or not(...)");
    }};
    ($ctx:expr, $kind:ident $args:tt, in_subtree($parent:expr) $(,)?) => {{
        let __expr = $crate::query!(@expr $kind $args);
        let __query = $crate::sub_apis::NodeQuery::new()
            .where_expr(__expr)
            .in_subtree($parent);
        $ctx.NodeQuery().query(&__query)
    }};
    ($ctx:expr, $kind:ident $args:tt $(,)?) => {{
        let __expr = $crate::query!(@expr $kind $args);
        let __query = $crate::sub_apis::NodeQuery::new().where_expr(__expr);
        $ctx.NodeQuery().query(&__query)
    }};
    ($ctx:expr, $query:expr, in_subtree($parent:expr) $(,)?) => {{
        let __query = $query;
        let __query_view = (&__query).as_view().in_subtree($parent);
        $ctx.NodeQuery().query_view(__query_view)
    }};
    ($ctx:expr, $query:expr $(,)?) => {{
        let __query = $query;
        let __query_view = (&__query).as_view();
        $ctx.NodeQuery().query_view(__query_view)
    }};

    (@expr all($($kind:ident $args:tt),* $(,)?)) => {
        $crate::sub_apis::QueryExpr::All(vec![$($crate::query!(@expr $kind $args)),*])
    };
    (@expr any($($kind:ident $args:tt),* $(,)?)) => {
        $crate::sub_apis::QueryExpr::Any(vec![$($crate::query!(@expr $kind $args)),*])
    };
    (@expr not($kind:ident $args:tt)) => {
        $crate::sub_apis::QueryExpr::Not(Box::new($crate::query!(@expr $kind $args)))
    };

    (@expr name[$($name:expr),* $(,)?]) => {
        $crate::sub_apis::QueryExpr::Name(vec![$(($name).to_string()),*])
    };

    (@expr tags[$($tag:literal),* $(,)?]) => {
        $crate::sub_apis::QueryExpr::Tags(vec![$(const { $crate::perro_ids::TagID::from_string($tag) }),*])
    };

    (@expr tags[$($tag:expr),* $(,)?]) => {
        $crate::sub_apis::QueryExpr::Tags(vec![$($crate::perro_ids::IntoTagID::into_tag_id($tag)),*])
    };

    (@expr node_type[$($ty:ident),* $(,)?]) => {
        $crate::sub_apis::QueryExpr::IsTypeMask(const {
            $crate::sub_apis::__query_type_mask(&[$($crate::perro_nodes::NodeType::$ty),*])
        })
    };
    (@expr node_type[$($ty:path),* $(,)?]) => {
        $crate::sub_apis::QueryExpr::IsTypeMask(const {
            $crate::sub_apis::__query_type_mask(&[$($ty),*])
        })
    };
    (@expr base_type[$($ty:ident),* $(,)?]) => {
        $crate::sub_apis::QueryExpr::BaseTypeMask(const {
            $crate::sub_apis::__query_base_type_mask(&[$($crate::perro_nodes::NodeType::$ty),*])
        })
    };
    (@expr base_type[$($ty:path),* $(,)?]) => {
        $crate::sub_apis::QueryExpr::BaseTypeMask(const {
            $crate::sub_apis::__query_base_type_mask(&[$($ty),*])
        })
    };

    (@expr layers[] ) => {
        $crate::sub_apis::QueryExpr::Layers($crate::perro_structs::BitMask::NONE)
    };
    (@expr layers[$($layer:literal),* $(,)?]) => {
        $crate::sub_apis::QueryExpr::Layers(const {
            $crate::perro_structs::BitMask::with([$($layer),*])
        })
    };
    (@expr layers[$($layer:expr),* $(,)?]) => {
        $crate::sub_apis::QueryExpr::Layers(
            $crate::perro_structs::BitMask::from_layers([$($layer),*])
        )
    };
    (@expr mask[] ) => {
        $crate::sub_apis::QueryExpr::Mask($crate::perro_structs::BitMask::NONE)
    };
    (@expr mask[$($layer:literal),* $(,)?]) => {
        $crate::sub_apis::QueryExpr::Mask(const {
            $crate::perro_structs::BitMask::with([$($layer),*])
        })
    };
    (@expr mask[$($layer:expr),* $(,)?]) => {
        $crate::sub_apis::QueryExpr::Mask(
            $crate::perro_structs::BitMask::from_layers([$($layer),*])
        )
    };
}

/// Executes a node query and returns owned `NodeID`s as an iterator.
///
/// This has the same syntax as [`query!`](macro@crate::query). It still uses
/// the runtime's owned query result internally, then returns `Vec::into_iter()`.
#[macro_export]
macro_rules! query_iter {
    ($ctx:expr, $kind:ident $args:tt, in_subtree($parent:expr) $(,)?) => {{
        $crate::query!($ctx, $kind $args, in_subtree($parent)).into_iter()
    }};
    ($ctx:expr, $kind:ident $args:tt $(,)?) => {{
        $crate::query!($ctx, $kind $args).into_iter()
    }};
    ($ctx:expr, $query:expr, in_subtree($parent:expr) $(,)?) => {{
        let __query = $query;
        let __query_view = (&__query).as_view().in_subtree($parent);
        $ctx.NodeQuery().query_view_iter(__query_view)
    }};
    ($ctx:expr, $query:expr $(,)?) => {{
        let __query = $query;
        $ctx.NodeQuery().query_iter(&__query)
    }};
}

/// Executes a node query and runs a closure once for each matching `NodeID`.
///
/// This has the same query syntax as [`query!`](macro@crate::query).
#[macro_export]
macro_rules! query_each {
    ($ctx:expr, $kind:ident $args:tt, in_subtree($parent:expr), $f:expr $(,)?) => {{
        for __node_id in $crate::query_iter!($ctx, $kind $args, in_subtree($parent)) {
            $f(__node_id);
        }
    }};
    ($ctx:expr, $kind:ident $args:tt, $f:expr $(,)?) => {{
        for __node_id in $crate::query_iter!($ctx, $kind $args) {
            $f(__node_id);
        }
    }};
    ($ctx:expr, $query:expr, in_subtree($parent:expr), $f:expr $(,)?) => {{
        for __node_id in $crate::query_iter!($ctx, $query, in_subtree($parent)) {
            $f(__node_id);
        }
    }};
    ($ctx:expr, $query:expr, $f:expr $(,)?) => {{
        for __node_id in $crate::query_iter!($ctx, $query) {
            $f(__node_id);
        }
    }};
}

/// Executes a node query and maps each matching `NodeID` into a collected `Vec`.
///
/// This has the same query syntax as [`query!`](macro@crate::query).
#[macro_export]
macro_rules! query_map {
    ($ctx:expr, $kind:ident $args:tt, in_subtree($parent:expr), $f:expr $(,)?) => {{
        $crate::query_iter!($ctx, $kind $args, in_subtree($parent))
            .map($f)
            .collect::<Vec<_>>()
    }};
    ($ctx:expr, $kind:ident $args:tt, $f:expr $(,)?) => {{
        $crate::query_iter!($ctx, $kind $args)
            .map($f)
            .collect::<Vec<_>>()
    }};
    ($ctx:expr, $query:expr, in_subtree($parent:expr), $f:expr $(,)?) => {{
        $crate::query_iter!($ctx, $query, in_subtree($parent))
            .map($f)
            .collect::<Vec<_>>()
    }};
    ($ctx:expr, $query:expr, $f:expr $(,)?) => {{
        $crate::query_iter!($ctx, $query)
            .map($f)
            .collect::<Vec<_>>()
    }};
}

/// Executes a node query and returns the first result as owned `NodeID`.
///
/// Usage:
/// - `query_first!(ctx, all(name["Enemy1"])) -> Option<NodeID>`
/// - `query_first!(ctx, all(tags["enemy"]), in_subtree(parent_id)) -> Option<NodeID>`
#[macro_export]
///   R is the return type of the underlying API method call this macro expands to.
macro_rules! query_first {
    ($ctx:expr, $kind:ident $args:tt, in_subtree($parent:expr) $(,)?) => {{
        let __expr = $crate::query!(@expr $kind $args);
        let __query = $crate::sub_apis::NodeQuery::new()
            .where_expr(__expr)
            .in_subtree($parent);
        $ctx.NodeQuery().query_first(&__query)
    }};
    ($ctx:expr, $kind:ident $args:tt $(,)?) => {{
        let __expr = $crate::query!(@expr $kind $args);
        let __query = $crate::sub_apis::NodeQuery::new().where_expr(__expr);
        $ctx.NodeQuery().query_first(&__query)
    }};
    ($ctx:expr, $query:expr, in_subtree($parent:expr) $(,)?) => {{
        let __query = $query;
        let __query_view = (&__query).as_view().in_subtree($parent);
        $ctx.NodeQuery().query_view_first(__query_view)
    }};
    ($ctx:expr, $query:expr $(,)?) => {{
        let __query = $query;
        $ctx.NodeQuery().query_first(&__query)
    }};
}
