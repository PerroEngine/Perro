use crate::ids::NodeID;
use cow_map::CowMap;
use serde_json::Value;
use std::borrow::Cow;
use std::fmt::Debug;
use std::{any::Any, collections::HashMap};

use serde::{Deserialize, Serialize};

/// Enum for specifying whether a node needs internal fixed updates
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedUpdate {
    True,
    False,
}

impl FixedUpdate {
    pub fn as_bool(self) -> bool {
        matches!(self, FixedUpdate::True)
    }
}

/// Enum for specifying whether a node needs internal render updates
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderUpdate {
    True,
    False,
}

impl RenderUpdate {
    pub fn as_bool(self) -> bool {
        matches!(self, RenderUpdate::True)
    }
}

/// Enum for specifying whether a node is renderable
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Renderable {
    True,
    False,
}

impl Renderable {
    pub fn as_bool(self) -> bool {
        matches!(self, Renderable::True)
    }
}

/// Trait for inner node types that need internal fixed updates (e.g., Area2D for physics)
/// Nodes implement this trait to opt into internal fixed updates.
///
/// To use this trait:
/// 1. Implement `NodeWithInternalFixedUpdate` for your node type
/// 2. The macro-generated BaseNode impl will automatically call your trait method
///
/// Example for Node (impl lives in this crate; doc test skipped to avoid orphan rule):
/// ```ignore
/// impl NodeWithInternalFixedUpdate for Node {
///     fn internal_fixed_update(&mut self, api: &mut ScriptApi) {
///         // Your internal fixed update logic here
///     }
/// }
/// ```
pub trait NodeWithInternalFixedUpdate: BaseNode {
    /// Called during the fixed update phase
    /// This runs at the XPS rate from the project manifest
    fn internal_fixed_update(&mut self, api: &mut crate::scripting::api::ScriptApi);
}

/// Trait for inner node types that need internal render updates (e.g., UINode for UI interactions)
/// Nodes implement this trait to opt into internal render updates.
///
/// To use this trait:
/// 1. Implement `NodeWithInternalRenderUpdate` for your node type
/// 2. The macro-generated BaseNode impl will automatically call your trait method
///
/// Example for UINode (impl lives in this crate; doc test skipped to avoid orphan rule):
/// ```ignore
/// impl NodeWithInternalRenderUpdate for UINode {
///     fn internal_render_update(&mut self, api: &mut ScriptApi) {
///         // Your internal render update logic here (runs every frame)
///     }
/// }
/// ```
pub trait NodeWithInternalRenderUpdate: BaseNode {
    /// Called during the render phase (every frame)
    /// This runs at the render rate to match visual updates
    fn internal_render_update(&mut self, api: &mut crate::scripting::api::ScriptApi);
}

/// Base trait implemented by all engine node types.
/// Provides unified access and manipulation for all node variants stored in `SceneNode`.
pub trait BaseNode: Any + Debug + Send {
    fn get_id(&self) -> NodeID;
    fn set_id(&mut self, id: NodeID);

    fn get_name(&self) -> &str;
    fn set_name(&mut self, name: impl Into<Cow<'static, str>>);
    fn get_is_root_of(&self) -> Option<&str>;
    fn get_parent(&self) -> Option<crate::nodes::node::ParentType>;

    /// Returns a slice of child node IDs (borrowed static slice or owned vec).
    fn get_children(&self) -> &[NodeID];

    fn get_type(&self) -> NodeType;
    fn get_script_path(&self) -> Option<&str>;

    fn set_parent(&mut self, parent: Option<crate::nodes::node::ParentType>);
    fn add_child(&mut self, child: NodeID);
    fn remove_child(&mut self, c: &NodeID);
    fn set_script_path(&mut self, path: &str);

    fn get_script_exp_vars(&self) -> Option<HashMap<String, Value>>;
    fn set_script_exp_vars(&mut self, vars: Option<HashMap<String, Value>>);
    /// Raw script_exp_vars (for codegen/remap). Returns None if empty.
    fn get_script_exp_vars_raw(
        &self,
    ) -> Option<&CowMap<&'static str, crate::nodes::node::ScriptExpVarValue>>;
    /// Mutable raw script_exp_vars (for remap NodeRef(scene_key) → NodeRef(runtime_id)).
    fn get_script_exp_vars_raw_mut(
        &mut self,
    ) -> Option<&mut CowMap<&'static str, crate::nodes::node::ScriptExpVarValue>>;

    /// Raw metadata (for codegen). Returns None if empty.
    fn get_metadata_raw(&self) -> Option<&CowMap<&'static str, crate::nodes::node::MetadataValue>>;
    fn get_metadata_raw_mut(
        &mut self,
    ) -> Option<&mut CowMap<&'static str, crate::nodes::node::MetadataValue>>;

    /// Check if this node is renderable (actually rendered to screen)
    /// Only renderable nodes should be added to needs_rerender
    fn is_renderable(&self) -> bool;

    fn get_children_mut(&mut self) -> &mut Vec<NodeID>;

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn clear_children(&mut self) {
        self.get_children_mut().clear();
    }
    fn clear_parent(&mut self) {
        self.set_parent(None);
    }

    /// Internal fixed update - called during fixed update phase for nodes that need it
    /// Default implementation does nothing.
    /// Nodes that implement NodeWithInternalFixedUpdate will have their trait method called
    /// automatically in SceneNode::internal_fixed_update.
    fn internal_fixed_update(&mut self, _api: &mut crate::scripting::api::ScriptApi) {
        // Default empty implementation
    }

    /// Returns true if this node needs internal fixed updates
    /// Default implementation returns false.
    /// Nodes that implement NodeWithInternalFixedUpdate should override this to return true.
    fn needs_internal_fixed_update(&self) -> bool {
        false
    }

    /// Internal render update - called during render phase for nodes that need it
    /// Default implementation does nothing.
    /// Nodes that implement NodeWithInternalRenderUpdate will have their trait method called
    /// automatically in SceneNode::internal_render_update.
    fn internal_render_update(&mut self, _api: &mut crate::scripting::api::ScriptApi) {
        // Default empty implementation
    }

    /// Returns true if this node needs internal render updates
    /// Default implementation returns false.
    /// Nodes that implement NodeWithInternalRenderUpdate should override this to return true.
    fn needs_internal_render_update(&self) -> bool {
        false
    }

    /// Mark transform as dirty for Node2D nodes (no-op for other node types)
    /// This is called after deserialization to ensure transforms are recalculated
    fn mark_transform_dirty_if_node2d(&mut self) {
        // Default implementation does nothing - only Node2D nodes override this
    }

    /// Mark transform as dirty for Node3D nodes (no-op for other node types)
    fn mark_transform_dirty_if_node3d(&mut self) {
        // Default implementation does nothing - only Node3D nodes override this
    }

    /// Get the creation timestamp (Unix time in seconds as u64)
    /// Used for tie-breaking when z_index values are the same (newer nodes render above older)
    fn get_created_timestamp(&self) -> u64;
}

/// Used to unwrap `SceneNode` variants back into their concrete types.
pub trait IntoInner<T> {
    fn into_inner(self) -> T;
}

/// Trait for converting a concrete node type into a SceneNode enum
/// This is implemented automatically for all node types via the define_nodes! macro
/// Note: SceneNode must be defined before this trait (it's defined in the macro)
pub trait ToSceneNode {
    fn to_scene_node(self) -> SceneNode;
}

/// Sealed trait for types that can be extracted from SceneNode via match dispatch
/// This allows compile-time optimization of type extraction
/// Implemented automatically for all node types via the define_nodes! macro
pub trait NodeTypeDispatch: 'static {
    /// Extract a reference to this type from a SceneNode if it matches
    /// Returns None if the SceneNode is not of this type
    fn extract_ref(node: &SceneNode) -> Option<&Self>;

    /// Extract a mutable reference to this type from a SceneNode if it matches
    /// Returns None if the SceneNode is not of this type
    fn extract_mut(node: &mut SceneNode) -> Option<&mut Self>;
}

/// Common macro implementing `BaseNode` for each concrete node type.
/// This version supports `Option<Vec<NodeID>>` for `children`.
#[macro_export]
macro_rules! impl_scene_node {
    ($ty:ty, $variant:ident, $needs_internal:expr, $needs_render:expr, $is_renderable:expr) => {
        impl crate::nodes::node_registry::BaseNode for $ty {
            fn get_id(&self) -> NodeID {
                self.id
            }
            fn set_id(&mut self, id: NodeID) {
                self.id = id;
            }

            fn get_name(&self) -> &str {
                &self.name
            }
            fn set_name(&mut self, name: impl Into<Cow<'static, str>>) {
                self.name = name.into();
            }
            fn get_is_root_of(&self) -> Option<&str> {
                self.is_root_of.as_deref()
            }
            fn get_parent(&self) -> Option<crate::nodes::node::ParentType> {
                self.parent.clone()
            }

            fn get_children(&self) -> &[NodeID] {
                match &self.children {
                    None => &[],
                    Some(std::borrow::Cow::Borrowed(s)) => s,
                    Some(std::borrow::Cow::Owned(v)) => v.as_slice(),
                }
            }

            fn get_type(&self) -> NodeType {
                // All node types now use NodeType - return it directly
                self.ty
            }

            fn get_script_path(&self) -> Option<&str> {
                self.script_path.as_deref() // This works for both Cow and Option<String>
            }

            fn set_parent(&mut self, p: Option<crate::nodes::node::ParentType>) {
                self.parent = p;
            }

            fn add_child(&mut self, c: NodeID) {
                self.get_children_mut().push(c);
            }

            fn remove_child(&mut self, c: &NodeID) {
                self.get_children_mut().retain(|x| x != c);
            }

            fn set_script_path(&mut self, path: &str) {
                self.script_path = Some(std::borrow::Cow::Owned(path.to_string()));
            }

            fn get_children_mut(&mut self) -> &mut Vec<NodeID> {
                if self.children.is_none() {
                    self.children = Some(std::borrow::Cow::Owned(Vec::new()));
                }
                let cow = self.children.as_mut().unwrap();
                if let std::borrow::Cow::Borrowed(s) = cow {
                    *cow = std::borrow::Cow::Owned(s.to_vec());
                }
                match self.children.as_mut().unwrap() {
                    std::borrow::Cow::Owned(v) => v,
                    _ => unreachable!(),
                }
            }

            fn get_script_exp_vars(&self) -> Option<HashMap<String, Value>> {
                self.script_exp_vars.as_ref().map(|m| {
                    m.iter()
                        .map(|(k, v)| (k.to_string(), v.to_json_value()))
                        .collect()
                })
            }

            fn set_script_exp_vars(&mut self, vars: Option<HashMap<String, Value>>) {
                self.script_exp_vars = vars.map(|m| {
                    CowMap::from(
                        m.into_iter()
                            .map(|(k, v)| {
                                (
                                    &*Box::leak(k.into_boxed_str()),
                                    crate::nodes::node::ScriptExpVarValue::from_json_value(&v),
                                )
                            })
                            .collect::<HashMap<_, _>>(),
                    )
                });
            }

            fn get_script_exp_vars_raw(
                &self,
            ) -> Option<&CowMap<&'static str, crate::nodes::node::ScriptExpVarValue>> {
                self.script_exp_vars.as_ref()
            }

            fn get_script_exp_vars_raw_mut(
                &mut self,
            ) -> Option<&mut CowMap<&'static str, crate::nodes::node::ScriptExpVarValue>> {
                self.script_exp_vars.as_mut()
            }

            fn get_metadata_raw(
                &self,
            ) -> Option<&CowMap<&'static str, crate::nodes::node::MetadataValue>> {
                self.metadata.as_ref()
            }
            fn get_metadata_raw_mut(
                &mut self,
            ) -> Option<&mut CowMap<&'static str, crate::nodes::node::MetadataValue>> {
                self.metadata.as_mut()
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }

            // Generate internal_fixed_update based on the flag
            fn internal_fixed_update(&mut self, api: &mut crate::scripting::api::ScriptApi) {
                // If the node needs internal fixed update, call its method
                // The node must have an `internal_fixed_update` method in its impl block
                if $needs_internal.as_bool() {
                    self.internal_fixed_update(api);
                }
            }

            fn needs_internal_fixed_update(&self) -> bool {
                $needs_internal.as_bool()
            }

            // Generate internal_render_update based on the flag
            fn internal_render_update(&mut self, api: &mut crate::scripting::api::ScriptApi) {
                // If the node needs internal render update, call its method
                // The node must have an `internal_render_update` method in its impl block
                if $needs_render.as_bool() {
                    self.internal_render_update(api);
                }
            }

            fn needs_internal_render_update(&self) -> bool {
                $needs_render.as_bool()
            }

            fn get_created_timestamp(&self) -> u64 {
                // Access created_timestamp directly (works for Node and Node2D types via Deref)
                // Same pattern as get_id() and get_name() - they access directly, not through base
                self.created_timestamp
            }

            fn is_renderable(&self) -> bool {
                $is_renderable.as_bool()
            }
        }

        impl $ty {
            /// Converts this specific node into a generic `SceneNode` enum variant.
            pub fn to_scene_node(self) -> crate::nodes::node_registry::SceneNode {
                crate::nodes::node_registry::SceneNode::$variant(self)
            }
        }

        impl crate::nodes::node_registry::ToSceneNode for $ty {
            fn to_scene_node(self) -> crate::nodes::node_registry::SceneNode {
                crate::nodes::node_registry::SceneNode::$variant(self)
            }
        }

        impl crate::nodes::node_registry::IntoInner<$ty>
            for crate::nodes::node_registry::SceneNode
        {
            fn into_inner(self) -> $ty {
                match self {
                    crate::nodes::node_registry::SceneNode::$variant(inner) => inner,
                    _ => panic!(
                        "Cannot extract {} from SceneNode variant {:?}",
                        stringify!($ty),
                        self
                    ),
                }
            }
        }

        // Implement NodeTypeDispatch for optimized type extraction
        impl crate::nodes::node_registry::NodeTypeDispatch for $ty {
            #[inline(always)]
            fn extract_ref(node: &crate::nodes::node_registry::SceneNode) -> Option<&Self> {
                match node {
                    crate::nodes::node_registry::SceneNode::$variant(inner) => Some(inner),
                    _ => None,
                }
            }

            #[inline(always)]
            fn extract_mut(node: &mut crate::nodes::node_registry::SceneNode) -> Option<&mut Self> {
                match node {
                    crate::nodes::node_registry::SceneNode::$variant(inner) => Some(inner),
                    _ => None,
                }
            }
        }
    };
}

/// Declares all node types and generates `NodeType` + `SceneNode` enums.
/// Also implements the `BaseNode` trait for `SceneNode` by delegating to its inner value.
///
/// Syntax: `NodeName(FixedUpdate::True/False, RenderUpdate::True/False, Renderable::True/False) => path::to::NodeType`
/// where `FixedUpdate::True` means the node needs internal fixed updates (runs at XPS rate)
/// and `RenderUpdate::True` means the node needs internal render updates (runs every frame)
/// and `Renderable::True` means the node is actually rendered to screen
/// If FixedUpdate::True, the node must have an `internal_fixed_update` method in its impl block
/// If RenderUpdate::True, the node must have an `internal_render_update` method in its impl block
#[macro_export]
macro_rules! define_nodes {
    ( $( $variant:ident($needs_internal:expr, $needs_render:expr, $is_renderable:expr) => $ty:path ),+ $(,)? ) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "PascalCase")]
        pub enum NodeType { $( $variant, )+ }

        impl Default for NodeType {
            fn default() -> Self {
                NodeType::Node
            }
        }

        impl std::fmt::Display for NodeType {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $( NodeType::$variant => write!(f, "{}", stringify!($variant)), )+
                }
            }
        }

        impl std::str::FromStr for NodeType {
            type Err = String;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $( stringify!($variant) => Ok(NodeType::$variant), )+
                    _ => Err(format!("Unknown node type: {}", s)),
                }
            }
        }

        impl NodeType {
            /// Get the string representation as a static slice
            pub fn type_name(&self) -> &'static str {
                match self {
                    $( NodeType::$variant => stringify!($variant), )+
                }
            }

            /// Check if this node type is renderable (actually rendered to screen)
            pub fn is_renderable(&self) -> bool {
                match self {
                    $( NodeType::$variant => $is_renderable.as_bool(), )+
                }
            }
        }

        #[derive(Debug, Clone, Serialize)]
        #[serde(tag = "type")]
        pub enum SceneNode {
            $( $variant($ty), )+
        }

        impl<'de> serde::Deserialize<'de> for SceneNode {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                use serde_json::Value;
                use serde::de::Error;

                let value = Value::deserialize(deserializer)?;

                let type_str = value.get("type")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| D::Error::missing_field("type"))?;

                match type_str {
                    $(
                        stringify!($variant) => {
                            let inner: $ty = serde_json::from_value(value)
                                .map_err(D::Error::custom)?;
                            Ok(SceneNode::$variant(inner))
                        },
                    )+
                    _ => Err(D::Error::unknown_variant(
                        type_str,
                        &[$(stringify!($variant)),+]
                    )),
                }
            }
        }

        impl crate::nodes::node_registry::BaseNode for SceneNode {
            fn get_id(&self) -> NodeID {
                match self { $( SceneNode::$variant(n) => n.get_id(), )+ }
            }

            fn set_id(&mut self, id: NodeID) {
                match self { $( SceneNode::$variant(n) => n.set_id(id), )+ }
            }

            fn get_name(&self) -> &str {
                match self { $( SceneNode::$variant(n) => n.get_name(), )+ }
            }

            fn set_name(&mut self, name: impl Into<Cow<'static, str>>) {
                match self { $( SceneNode::$variant(n) => n.set_name(name), )+ }
            }

            fn get_is_root_of(&self) -> Option<&str> {
                match self { $( SceneNode::$variant(n) => n.get_is_root_of(), )+ }
            }

            fn get_parent(&self) -> Option<crate::nodes::node::ParentType> {
                match self { $( SceneNode::$variant(n) => n.get_parent(), )+ }
            }

            fn get_children(&self) -> &[NodeID] {
                match self { $( SceneNode::$variant(n) => n.get_children(), )+ }
            }

            fn get_type(&self) -> NodeType {
                match self { $( SceneNode::$variant(n) => n.get_type(), )+ }
            }

            fn get_script_path(&self) -> Option<&str> {
                match self { $( SceneNode::$variant(n) => n.get_script_path(), )+ }
            }

            fn set_parent(&mut self, parent: Option<crate::nodes::node::ParentType>) {
                match self { $( SceneNode::$variant(n) => n.set_parent(parent), )+ }
            }

            fn add_child(&mut self, child: NodeID) {
                match self { $( SceneNode::$variant(n) => n.add_child(child), )+ }
            }

            fn remove_child(&mut self, c: &NodeID) {
                match self { $( SceneNode::$variant(n) => n.remove_child(c), )+ }
            }

            fn set_script_path(&mut self, path: &str) {
                match self { $( SceneNode::$variant(n) => n.set_script_path(path), )+ }
            }

            fn get_script_exp_vars(&self) -> Option<HashMap<String, Value>> {
                match self { $( SceneNode::$variant(n) => n.get_script_exp_vars(), )+ }
            }

            fn set_script_exp_vars(&mut self, vars: Option<HashMap<String, Value>>) {
                match self { $( SceneNode::$variant(n) => n.set_script_exp_vars(vars), )+ }
            }

            fn get_script_exp_vars_raw(&self) -> Option<&CowMap<&'static str, crate::nodes::node::ScriptExpVarValue>> {
                match self { $( SceneNode::$variant(n) => n.get_script_exp_vars_raw(), )+ }
            }

            fn get_script_exp_vars_raw_mut(&mut self) -> Option<&mut CowMap<&'static str, crate::nodes::node::ScriptExpVarValue>> {
                match self { $( SceneNode::$variant(n) => n.get_script_exp_vars_raw_mut(), )+ }
            }

            fn get_metadata_raw(&self) -> Option<&CowMap<&'static str, crate::nodes::node::MetadataValue>> {
                match self { $( SceneNode::$variant(n) => n.get_metadata_raw(), )+ }
            }
            fn get_metadata_raw_mut(&mut self) -> Option<&mut CowMap<&'static str, crate::nodes::node::MetadataValue>> {
                match self { $( SceneNode::$variant(n) => n.get_metadata_raw_mut(), )+ }
            }

            fn get_children_mut(&mut self) -> &mut Vec<NodeID> {
                match self { $( SceneNode::$variant(n) => n.get_children_mut(), )+ }
            }

            fn as_any(&self) -> &dyn std::any::Any {
                match self { $( SceneNode::$variant(n) => n.as_any(), )+ }
            }

            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                match self { $( SceneNode::$variant(n) => n.as_any_mut(), )+ }
            }

            fn internal_fixed_update(&mut self, api: &mut crate::scripting::api::ScriptApi) {
                match self {
                    $(
                        SceneNode::$variant(n) => {
                            // Call BaseNode::internal_fixed_update - if the type implements NodeWithInternalFixedUpdate
                            // and used the macro, this will call the trait method
                            <$ty as crate::nodes::node_registry::BaseNode>::internal_fixed_update(n, api);
                        }
                    )+
                }
            }

            fn needs_internal_fixed_update(&self) -> bool {
                match self { $( SceneNode::$variant(n) => n.needs_internal_fixed_update(), )+ }
            }

            fn internal_render_update(&mut self, api: &mut crate::scripting::api::ScriptApi) {
                match self {
                    $(
                        SceneNode::$variant(n) => {
                            // Call BaseNode::internal_render_update - if the type implements NodeWithInternalRenderUpdate
                            // and used the macro, this will call the trait method
                            <$ty as crate::nodes::node_registry::BaseNode>::internal_render_update(n, api);
                        }
                    )+
                }
            }

            fn needs_internal_render_update(&self) -> bool {
                match self { $( SceneNode::$variant(n) => n.needs_internal_render_update(), )+ }
            }

            fn mark_transform_dirty_if_node2d(&mut self) {
                if let Some(node2d) = self.as_node2d_mut() {
                    node2d.transform_dirty = true;
                }
            }

            fn mark_transform_dirty_if_node3d(&mut self) {
                if let Some(node3d) = self.as_node3d_mut() {
                    node3d.transform_dirty = true;
                }
            }

            fn get_created_timestamp(&self) -> u64 {
                match self {
                    $( SceneNode::$variant(n) => n.get_created_timestamp(), )+
                }
            }

            fn is_renderable(&self) -> bool {
                match self {
                    $( SceneNode::$variant(_) => $is_renderable.as_bool(), )+
                }
            }
        }

        // Helper methods to extract Node2D references
        impl SceneNode {
            /// Get a mutable reference to the Node2D if this is a Node2D-based node
            pub fn as_node2d_mut(&mut self) -> Option<&mut crate::nodes::_2d::node_2d::Node2D> {
                match self {
                    SceneNode::Node2D(n2d) => Some(n2d),
                    SceneNode::Sprite2D(sprite) => Some(&mut sprite.base),
                    SceneNode::Area2D(area) => Some(&mut area.base),
                    SceneNode::CollisionShape2D(cs) => Some(&mut cs.base),
                    SceneNode::ShapeInstance2D(shape) => Some(&mut shape.base),
                    SceneNode::Camera2D(cam) => Some(&mut cam.base),
                    _ => None,
                }
            }

            /// Get a reference to the Node2D if this is a Node2D-based node
            pub fn as_node2d(&self) -> Option<&crate::nodes::_2d::node_2d::Node2D> {
                match self {
                    SceneNode::Node2D(n2d) => Some(n2d),
                    SceneNode::Sprite2D(sprite) => Some(&sprite.base),
                    SceneNode::Area2D(area) => Some(&area.base),
                    SceneNode::CollisionShape2D(cs) => Some(&cs.base),
                    SceneNode::ShapeInstance2D(shape) => Some(&shape.base),
                    SceneNode::Camera2D(cam) => Some(&cam.base),
                    _ => None,
                }
            }

            /// Get the local transform if this is a Node2D-based node
            /// Uses Deref to access transform through Node2D
            pub fn get_node2d_transform(&self) -> Option<crate::structs2d::Transform2D> {
                self.as_node2d().map(|node2d| node2d.transform)
            }

            /// Get a mutable reference to the Node3D if this is a Node3D-based node
            pub fn as_node3d_mut(&mut self) -> Option<&mut crate::nodes::_3d::node_3d::Node3D> {
                match self {
                    SceneNode::Node3D(n3d) => Some(n3d),
                    SceneNode::MeshInstance3D(mesh) => Some(&mut mesh.base),
                    SceneNode::Camera3D(cam) => Some(&mut cam.base),
                    SceneNode::DirectionalLight3D(light) => Some(&mut light.base),
                    SceneNode::OmniLight3D(light) => Some(&mut light.base),
                    SceneNode::SpotLight3D(light) => Some(&mut light.base),
                    _ => None,
                }
            }

            /// Get a reference to the Node3D if this is a Node3D-based node
            pub fn as_node3d(&self) -> Option<&crate::nodes::_3d::node_3d::Node3D> {
                match self {
                    SceneNode::Node3D(n3d) => Some(n3d),
                    SceneNode::MeshInstance3D(mesh) => Some(&mesh.base),
                    SceneNode::Camera3D(cam) => Some(&cam.base),
                    SceneNode::DirectionalLight3D(light) => Some(&light.base),
                    SceneNode::OmniLight3D(light) => Some(&light.base),
                    SceneNode::SpotLight3D(light) => Some(&light.base),
                    _ => None,
                }
            }

            /// Get the local transform if this is a Node3D-based node
            pub fn get_node3d_transform(&self) -> Option<crate::structs3d::Transform3D> {
                self.as_node3d().map(|node3d| node3d.transform)
            }

            /// Optimized typed access using compile-time match dispatch instead of Any downcast
            /// This uses a match statement on the enum variant, which the compiler can optimize to a jump table
            /// Returns Some(result) if the node matches type T, None otherwise
            ///
            /// This is faster than `Any::downcast_ref` because:
            /// 1. The match on enum variant can be optimized to a jump table
            /// 2. We only check the specific variant that matches, not all possible types
            /// 3. The compiler can inline and optimize the entire call chain
            /// 4. No trait dispatch overhead - match is generated directly
            #[inline(always)]
            pub fn with_typed_ref<T: NodeTypeDispatch, R>(&self, f: impl FnOnce(&T) -> R) -> Option<R> {
                // Use trait method but with always_inline - compiler will optimize away the call
                // The trait method itself is #[inline] so this should inline completely
                T::extract_ref(self).map(f)
            }

            /// Optimized typed mutable access using compile-time match dispatch instead of Any downcast
            /// This uses a match statement on the enum variant, which the compiler can optimize to a jump table
            /// Returns Some(result) if the node matches type T, None otherwise
            ///
            /// This is faster than `Any::downcast_mut` because:
            /// 1. The match on enum variant can be optimized to a jump table
            /// 2. We only check the specific variant that matches, not all possible types
            /// 3. The compiler can inline and optimize the entire call chain
            /// 4. No trait dispatch overhead - match is generated directly
            #[inline(always)]
            pub fn with_typed_mut<T: NodeTypeDispatch, R>(&mut self, f: impl FnOnce(&mut T) -> R) -> Option<R> {
                // Use trait method but with always_inline - compiler will optimize away the call
                // The trait method itself is #[inline] so this should inline completely
                T::extract_mut(self).map(f)
            }
        }

        $( impl_scene_node!($ty, $variant, $needs_internal, $needs_render, $is_renderable); )+
    };
}

// ─────────────────────────────────────────────
// Register all built-in node types here
// ─────────────────────────────────────────────

// Syntax: NodeName(FixedUpdate::True/False, Renderable::True/False) => path
// FixedUpdate::True means the node needs internal fixed updates
// Renderable::True means the node is actually rendered to screen
define_nodes!(
    Node(FixedUpdate::False, RenderUpdate::False, Renderable::False)     => crate::nodes::node::Node,
    Node2D(FixedUpdate::False, RenderUpdate::False, Renderable::False)   => crate::nodes::_2d::node_2d::Node2D,
    Sprite2D(FixedUpdate::False, RenderUpdate::False, Renderable::True) => crate::nodes::_2d::sprite_2d::Sprite2D,
    Area2D(FixedUpdate::True, RenderUpdate::False, Renderable::False)   => crate::nodes::_2d::area_2d::Area2D,
    CollisionShape2D(FixedUpdate::False, RenderUpdate::False, Renderable::False) => crate::nodes::_2d::collision_shape_2d::CollisionShape2D,
    ShapeInstance2D(FixedUpdate::False, RenderUpdate::False, Renderable::True) => crate::nodes::_2d::shape_instance_2d::ShapeInstance2D,
    Camera2D(FixedUpdate::False, RenderUpdate::False, Renderable::True)  => crate::nodes::_2d::camera_2d::Camera2D,


    UINode(FixedUpdate::False, RenderUpdate::True, Renderable::True)   => crate::nodes::ui_node::UINode,


    Node3D(FixedUpdate::False, RenderUpdate::False, Renderable::False)   => crate::nodes::_3d::node_3d::Node3D,
    MeshInstance3D(FixedUpdate::False, RenderUpdate::False, Renderable::True) => crate::nodes::_3d::mesh_instance_3d::MeshInstance3D,
    Camera3D(FixedUpdate::False, RenderUpdate::False, Renderable::True)  => crate::nodes::_3d::camera_3d::Camera3D,

    DirectionalLight3D(FixedUpdate::False, RenderUpdate::False, Renderable::True) => crate::nodes::_3d::light_dir_3d::DirectionalLight3D,
    OmniLight3D(FixedUpdate::False, RenderUpdate::False, Renderable::True) => crate::nodes::_3d::light_omni_3d::OmniLight3D,
    SpotLight3D(FixedUpdate::False, RenderUpdate::False, Renderable::True) => crate::nodes::_3d::light_spot_3d::SpotLight3D,
);
