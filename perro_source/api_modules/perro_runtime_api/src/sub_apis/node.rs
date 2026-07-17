//! Runtime node API.
//!
//! Creates, removes, reparents, tags, transforms, reads, writes, and queries
//! live scene nodes. Query helpers live beside node access because they operate
//! on the same runtime scene graph.

use perro_ids::{IntoTagID, MaterialID, MeshID, NodeID, NodeTag, ScriptMemberID, TagID};
use perro_nodes::{
    Node2D, Node3D, NodeBaseDispatch, NodeType, NodeTypeDispatch, SceneNodeData, Skeleton3D, UiNode,
};
use perro_resource_api::ResPathSource;
use perro_structs::{
    BitMask, IntoBitMaskLayer, Quaternion, Transform2D, Transform3D, Vector2, Vector3,
};
use perro_variant::Variant;
use std::borrow::Cow;
use std::sync::Arc;

use super::scene::IntoScenePath;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum QueryScope {
    #[default]
    Root,
    Subtree(NodeID),
}

#[derive(Clone, Debug, PartialEq)]
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
    Within(QueryBounds),
}

impl QueryExpr {
    /// Returns `true` when the expression tree contains a spatial clause.
    pub fn has_spatial(&self) -> bool {
        let (has_2d, has_3d) = self.spatial_dims();
        has_2d || has_3d
    }

    /// Returns which spatial dimensions the expression tree filters on, as
    /// `(has_2d, has_3d)`. Lets the runtime skip position snapshots for
    /// dimensions the query never tests.
    pub fn spatial_dims(&self) -> (bool, bool) {
        match self {
            Self::Within(QueryBounds::Box2D { .. }) => (true, false),
            Self::Within(QueryBounds::Box3D { .. }) => (false, true),
            Self::All(children) | Self::Any(children) => {
                children.iter().fold((false, false), |(d2, d3), child| {
                    let (c2, c3) = child.spatial_dims();
                    (d2 || c2, d3 || c3)
                })
            }
            Self::Not(inner) => inner.spatial_dims(),
            _ => (false, false),
        }
    }
}

/// Axis-aligned box bounds for [`QueryExpr::Within`], in global space.
///
/// `origin` is the box center; `size` is the full extent along each axis.
/// 2D bounds match only 2D nodes; 3D bounds match only 3D nodes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum QueryBounds {
    Box2D { origin: Vector2, size: Vector2 },
    Box3D { origin: Vector3, size: Vector3 },
}

impl QueryBounds {
    pub fn contains_2d(&self, position: Vector2) -> bool {
        match self {
            Self::Box2D { origin, size } => {
                (position.x - origin.x).abs() <= size.x * 0.5
                    && (position.y - origin.y).abs() <= size.y * 0.5
            }
            Self::Box3D { .. } => false,
        }
    }

    pub fn contains_3d(&self, position: Vector3) -> bool {
        match self {
            Self::Box2D { .. } => false,
            Self::Box3D { origin, size } => {
                (position.x - origin.x).abs() <= size.x * 0.5
                    && (position.y - origin.y).abs() <= size.y * 0.5
                    && (position.z - origin.z).abs() <= size.z * 0.5
            }
        }
    }
}

/// Converts an `(origin, size)` vector pair into [`QueryBounds`].
///
/// Implemented for [`Vector2`] (2D box) and [`Vector3`] (3D box).
pub trait IntoQueryBounds {
    fn into_query_bounds(origin: Self, size: Self) -> QueryBounds;
}

impl IntoQueryBounds for Vector2 {
    fn into_query_bounds(origin: Self, size: Self) -> QueryBounds {
        QueryBounds::Box2D { origin, size }
    }
}

impl IntoQueryBounds for Vector3 {
    fn into_query_bounds(origin: Self, size: Self) -> QueryBounds {
        QueryBounds::Box3D { origin, size }
    }
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

/// One concrete node creation request inside a recursive node collection.
#[derive(Clone, Debug)]
pub struct NodeSpec {
    pub data: SceneNodeData,
    pub name: Option<Cow<'static, str>>,
    pub tags: Vec<NodeTag>,
    pub script: Option<NodeScriptSpec>,
    pub parent: Option<usize>,
}

impl NodeSpec {
    pub fn new<T>(data: T) -> Self
    where
        T: Into<SceneNodeData>,
    {
        Self {
            data: data.into(),
            name: None,
            tags: Vec::new(),
            script: None,
            parent: None,
        }
    }

    pub fn name<S>(mut self, name: S) -> Self
    where
        S: Into<Cow<'static, str>>,
    {
        self.name = Some(name.into());
        self
    }

    pub fn tags<T>(mut self, tags: T) -> Self
    where
        T: IntoNodeTags,
    {
        self.tags = tags.into_node_tags();
        self
    }

    pub fn script<S>(mut self, script: S) -> Self
    where
        S: IntoNodeScriptSpec,
    {
        self.script = Some(script.into_node_script_spec());
        self
    }

    pub const fn parent(mut self, parent: Option<usize>) -> Self {
        self.parent = parent;
        self
    }
}

#[derive(Clone, Debug)]
pub struct NodeScriptSpec {
    pub path: Cow<'static, str>,
    pub vars: Vec<(ScriptMemberID, NodeScriptVar)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NodeScriptVar {
    Value(Variant),
    NodeRef(usize),
}

impl NodeScriptSpec {
    pub fn new<P>(path: P) -> Self
    where
        P: ResPathSource,
    {
        Self {
            path: Cow::Owned(path.as_res_path_str().to_string()),
            vars: Vec::new(),
        }
    }

    pub fn vars(mut self, vars: Vec<(ScriptMemberID, Variant)>) -> Self {
        self.vars = vars
            .into_iter()
            .map(|(member, value)| (member, NodeScriptVar::Value(value)))
            .collect();
        self
    }

    pub fn raw_vars(mut self, vars: Vec<(ScriptMemberID, NodeScriptVar)>) -> Self {
        self.vars = vars;
        self
    }
}

pub trait IntoNodeScriptSpec {
    fn into_node_script_spec(self) -> NodeScriptSpec;
}

impl IntoNodeScriptSpec for NodeScriptSpec {
    fn into_node_script_spec(self) -> NodeScriptSpec {
        self
    }
}

impl<P> IntoNodeScriptSpec for P
where
    P: ResPathSource,
{
    fn into_node_script_spec(self) -> NodeScriptSpec {
        NodeScriptSpec::new(self)
    }
}

#[derive(Clone)]
pub struct NodeRootPatch {
    node_type: NodeType,
    apply: Arc<dyn Fn(&mut SceneNodeData) -> bool>,
}

impl std::fmt::Debug for NodeRootPatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeRootPatch")
            .field("node_type", &self.node_type)
            .finish_non_exhaustive()
    }
}

impl NodeRootPatch {
    pub fn new<T, F>(apply: F) -> Self
    where
        T: NodeTypeDispatch + 'static,
        F: Fn(&mut T) + 'static,
    {
        Self {
            node_type: T::NODE_TYPE,
            apply: Arc::new(move |data| T::with_mut(data, |node| apply(node)).is_some()),
        }
    }

    pub const fn node_type(&self) -> NodeType {
        self.node_type
    }

    pub fn apply(&self, data: &mut SceneNodeData) -> bool {
        (self.apply)(data)
    }
}

/// One scene load request inside a recursive node collection.
#[derive(Clone, Debug)]
pub struct NodeSceneSpec {
    pub path: Cow<'static, str>,
    pub name: Option<Cow<'static, str>>,
    pub tags: Vec<NodeTag>,
    pub script: Option<NodeScriptSpec>,
    pub patches: Vec<NodeRootPatch>,
    pub parent: Option<usize>,
}

impl NodeSceneSpec {
    pub fn new<P>(path: P) -> Self
    where
        P: IntoScenePath,
    {
        Self {
            path: path.into_scene_path(),
            name: None,
            tags: Vec::new(),
            script: None,
            patches: Vec::new(),
            parent: None,
        }
    }

    pub fn name<S>(mut self, name: S) -> Self
    where
        S: Into<Cow<'static, str>>,
    {
        self.name = Some(name.into());
        self
    }

    pub fn tags<T>(mut self, tags: T) -> Self
    where
        T: IntoNodeTags,
    {
        self.tags = tags.into_node_tags();
        self
    }

    pub fn script<S>(mut self, script: S) -> Self
    where
        S: IntoNodeScriptSpec,
    {
        self.script = Some(script.into_node_script_spec());
        self
    }

    pub fn patch(mut self, patch: NodeRootPatch) -> Self {
        self.patches.push(patch);
        self
    }

    pub fn patches(mut self, patches: Vec<NodeRootPatch>) -> Self {
        self.patches = patches;
        self
    }

    pub const fn parent(mut self, parent: Option<usize>) -> Self {
        self.parent = parent;
        self
    }
}

#[derive(Clone, Debug)]
pub enum NodeCollectionEntry {
    Node(usize),
    Scene(usize),
}

/// Flat node collection emitted by [`node_collection!`](macro@crate::node_collection).
#[derive(Clone, Debug, Default)]
pub struct NodeCollection {
    pub specs: Vec<NodeSpec>,
    pub scenes: Vec<NodeSceneSpec>,
    pub entries: Vec<NodeCollectionEntry>,
    pub root: Option<usize>,
}

impl NodeCollection {
    pub const fn new() -> Self {
        Self {
            specs: Vec::new(),
            scenes: Vec::new(),
            entries: Vec::new(),
            root: None,
        }
    }

    pub fn push(&mut self, spec: NodeSpec) -> usize {
        let index = self.entries.len();
        self.specs.push(spec);
        self.entries
            .push(NodeCollectionEntry::Node(self.specs.len() - 1));
        index
    }

    pub fn push_scene(&mut self, scene: NodeSceneSpec) -> usize {
        let index = self.entries.len();
        self.scenes.push(scene);
        self.entries
            .push(NodeCollectionEntry::Scene(self.scenes.len() - 1));
        index
    }

    pub const fn set_root(&mut self, root: usize) {
        self.root = Some(root);
    }

    pub fn as_specs(&self) -> &[NodeSpec] {
        &self.specs
    }

    pub fn is_specs_only(&self) -> bool {
        self.scenes.is_empty()
    }

    pub fn extend(&mut self, collection: impl IntoNodeCollection, parent: Option<usize>) -> usize {
        let mut collection = collection.into_node_collection();
        let entry_offset = self.entries.len();
        let spec_offset = self.specs.len();
        let scene_offset = self.scenes.len();
        let mut root = collection.root.map(|root| entry_offset + root);
        for spec in &mut collection.specs {
            spec.parent = spec.parent.map(|parent| entry_offset + parent).or(parent);
        }
        for scene in &mut collection.scenes {
            scene.parent = scene.parent.map(|parent| entry_offset + parent).or(parent);
        }
        for entry in collection.entries {
            let entry_parent = match entry {
                NodeCollectionEntry::Node(index) => collection.specs[index].parent,
                NodeCollectionEntry::Scene(index) => collection.scenes[index].parent,
            };
            if root.is_none() && entry_parent == parent {
                root = Some(self.entries.len());
            }
            match entry {
                NodeCollectionEntry::Node(index) => {
                    self.specs.push(collection.specs[index].clone());
                    self.entries
                        .push(NodeCollectionEntry::Node(spec_offset + index));
                }
                NodeCollectionEntry::Scene(index) => {
                    self.scenes.push(collection.scenes[index].clone());
                    self.entries
                        .push(NodeCollectionEntry::Scene(scene_offset + index));
                }
            }
        }
        root.unwrap_or(entry_offset)
    }
}

pub trait IntoNodeCollection {
    fn into_node_collection(self) -> NodeCollection;
}

impl IntoNodeCollection for NodeCollection {
    fn into_node_collection(self) -> NodeCollection {
        self
    }
}

impl IntoNodeCollection for &NodeCollection {
    fn into_node_collection(self) -> NodeCollection {
        self.clone()
    }
}

impl IntoNodeCollection for Vec<NodeSpec> {
    fn into_node_collection(self) -> NodeCollection {
        let entries = (0..self.len()).map(NodeCollectionEntry::Node).collect();
        NodeCollection {
            specs: self,
            scenes: Vec::new(),
            entries,
            root: None,
        }
    }
}

impl IntoNodeCollection for &[NodeSpec] {
    fn into_node_collection(self) -> NodeCollection {
        self.to_vec().into_node_collection()
    }
}

impl<const N: usize> IntoNodeCollection for [NodeSpec; N] {
    fn into_node_collection(self) -> NodeCollection {
        self.into_iter().collect::<Vec<_>>().into_node_collection()
    }
}

impl<const N: usize> IntoNodeCollection for &[NodeSpec; N] {
    fn into_node_collection(self) -> NodeCollection {
        self.to_vec().into_node_collection()
    }
}

pub enum NodeCreateBatch<'a> {
    Specs(&'a [NodeSpec]),
    Collection(&'a NodeCollection),
    OwnedSpecs(Vec<NodeSpec>),
    OwnedCollection(NodeCollection),
}

pub trait IntoNodeCreateBatch<'a> {
    fn into_node_create_batch(self) -> NodeCreateBatch<'a>;
}

impl<'a> IntoNodeCreateBatch<'a> for &'a [NodeSpec] {
    fn into_node_create_batch(self) -> NodeCreateBatch<'a> {
        NodeCreateBatch::Specs(self)
    }
}

impl<'a> IntoNodeCreateBatch<'a> for &'a Vec<NodeSpec> {
    fn into_node_create_batch(self) -> NodeCreateBatch<'a> {
        NodeCreateBatch::Specs(self.as_slice())
    }
}

impl<'a> IntoNodeCreateBatch<'a> for Vec<NodeSpec> {
    fn into_node_create_batch(self) -> NodeCreateBatch<'a> {
        NodeCreateBatch::OwnedSpecs(self)
    }
}

impl<'a, const N: usize> IntoNodeCreateBatch<'a> for &'a [NodeSpec; N] {
    fn into_node_create_batch(self) -> NodeCreateBatch<'a> {
        NodeCreateBatch::Specs(self.as_slice())
    }
}

impl<'a> IntoNodeCreateBatch<'a> for &'a NodeCollection {
    fn into_node_create_batch(self) -> NodeCreateBatch<'a> {
        NodeCreateBatch::Collection(self)
    }
}

impl<'a> IntoNodeCreateBatch<'a> for NodeCollection {
    fn into_node_create_batch(self) -> NodeCreateBatch<'a> {
        NodeCreateBatch::OwnedCollection(self)
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
    /// Triangle index in the decoded mesh query triangle list.
    pub triangle_index: u32,
    /// Triangle weights `(a, b, c)` at the hit.
    pub barycentric: Vector3,
    /// Interpolated primary texture coordinate.
    pub uv0: Vector2,
    /// Interpolated UV1, or UV0 when the mesh has no UV1.
    pub paint_uv: Vector2,
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

    /// Adds a spatial filter: node global position must lie inside an
    /// axis-aligned box.
    ///
    /// `origin` is the box center in global space; `size` is the full box
    /// extent. Pass [`Vector2`]s to match 2D nodes or [`Vector3`]s to match
    /// 3D nodes; nodes of the other dimensionality (and non-spatial nodes)
    /// never match.
    pub fn within<V>(self, origin: V, size: V) -> Self
    where
        V: IntoQueryBounds,
    {
        self.and_expr(QueryExpr::Within(V::into_query_bounds(origin, size)))
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

/// Depth-first collection of `root` plus all descendants using a caller-supplied
/// child lookup. Shared by [`NodeAPI::subtree_node_ids`]; broken out so the walk
/// is unit-testable without a full runtime.
///
/// Returns an empty vec when `root` is nil. Nil children are skipped.
pub fn collect_subtree_ids(
    root: NodeID,
    mut children_of: impl FnMut(NodeID) -> Vec<NodeID>,
) -> Vec<NodeID> {
    if root.is_nil() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        out.push(id);
        for child in children_of(id) {
            if !child.is_nil() {
                stack.push(child);
            }
        }
    }
    out
}

mod api;
pub use api::*;
mod node_module;
pub use node_module::*;
mod query_module;
pub use query_module::*;
mod mesh_module;
pub use mesh_module::*;
