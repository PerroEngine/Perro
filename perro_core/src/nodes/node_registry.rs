use serde_json::Value;
use std::fmt::Debug;
use std::{any::Any, collections::HashMap};
use uuid::Uuid;

use serde::Serialize;

/// Trait for inner node types that need internal fixed updates (e.g., Area2D for physics)
/// Nodes implement this trait to opt into internal fixed updates.
///
/// To use this trait:
/// 1. Implement `NodeWithInternalFixedUpdate` for your node type
/// 2. The macro-generated BaseNode impl will automatically call your trait method
///
/// Example for Node:
/// ```rust
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

/// Base trait implemented by all engine node types.
/// Provides unified access and manipulation for all node variants stored in `SceneNode`.
pub trait BaseNode: Any + Debug + Send {
    fn get_id(&self) -> Uuid;
    fn get_local_id(&self) -> Uuid;
    fn set_id(&mut self, id: Uuid);
    fn set_local_id(&mut self, local_id: Uuid);

    fn get_name(&self) -> &str;
    fn set_name(&mut self, name: String);
    fn get_is_root_of(&self) -> Option<&str>;
    fn get_parent(&self) -> Uuid;

    /// Returns a reference to the children list.
    /// If the node has `None` for its children field, this returns an empty slice.
    fn get_children(&self) -> &Vec<Uuid>;

    fn get_type(&self) -> &str;
    fn get_script_path(&self) -> Option<&str>;

    fn set_parent(&mut self, parent: Option<Uuid>);
    fn add_child(&mut self, child: Uuid);
    fn remove_child(&mut self, c: &Uuid);
    fn set_script_path(&mut self, path: &str);

    fn get_script_exp_vars(&self) -> Option<HashMap<String, Value>>;
    fn set_script_exp_vars(&mut self, vars: Option<HashMap<String, Value>>);

    fn is_dirty(&self) -> bool;
    fn set_dirty(&mut self, dirty: bool);

    fn mark_dirty(&mut self) {
        self.set_dirty(true);
    }

    fn get_children_mut(&mut self) -> &mut Vec<Uuid>;

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

    /// Mark transform as dirty for Node2D nodes (no-op for other node types)
    /// This is called after deserialization to ensure transforms are recalculated
    fn mark_transform_dirty_if_node2d(&mut self) {
        // Default implementation does nothing - only Node2D nodes override this
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

/// Common macro implementing `BaseNode` for each concrete node type.
/// This version supports `Option<Vec<Uuid>>` for `children`.
#[macro_export]
macro_rules! impl_scene_node {
    ($ty:ty, $variant:ident, $needs_internal:literal) => {
        impl crate::nodes::node_registry::BaseNode for $ty {
            fn get_id(&self) -> uuid::Uuid {
                self.id
            }
            fn get_local_id(&self) -> uuid::Uuid {
                self.local_id
            }
            fn set_id(&mut self, id: uuid::Uuid) {
                self.id = id;
            }
            fn set_local_id(&mut self, local_id: uuid::Uuid) {
                self.local_id = local_id;
            }

            fn get_name(&self) -> &str {
                &self.name
            }
            fn set_name(&mut self, name: String) {
                self.name = std::borrow::Cow::Owned(name);
            }
            fn get_is_root_of(&self) -> Option<&str> {
                self.is_root_of.as_deref()
            }
            fn get_parent(&self) -> uuid::Uuid {
                self.parent_id
            }

            fn get_children(&self) -> &Vec<uuid::Uuid> {
                // Return empty vec reference if None
                static EMPTY_CHILDREN: Vec<uuid::Uuid> = Vec::new();
                match &self.children {
                    Some(children) => children,
                    None => &EMPTY_CHILDREN,
                }
            }

            fn get_type(&self) -> &str {
                &self.ty
            }

            fn get_script_path(&self) -> Option<&str> {
                self.script_path.as_deref() // This works for both Cow and Option<String>
            }

            fn set_parent(&mut self, p: Option<uuid::Uuid>) {
                self.parent_id = p.unwrap_or(uuid::Uuid::nil());
            }

            fn add_child(&mut self, c: uuid::Uuid) {
                self.children.get_or_insert_with(Vec::new).push(c);
            }

            fn remove_child(&mut self, c: &uuid::Uuid) {
                if let Some(children) = &mut self.children {
                    children.retain(|x| x != c);
                }
            }

            fn set_script_path(&mut self, path: &str) {
                self.script_path = Some(std::borrow::Cow::Owned(path.to_string()));
            }

            fn is_dirty(&self) -> bool {
                self.dirty
            }
            fn set_dirty(&mut self, dirty: bool) {
                self.dirty = dirty;
            }

            fn get_children_mut(&mut self) -> &mut Vec<uuid::Uuid> {
                self.children.get_or_insert_with(Vec::new)
            }

            fn get_script_exp_vars(&self) -> Option<HashMap<String, Value>> {
                self.script_exp_vars.clone()
            }

            fn set_script_exp_vars(&mut self, vars: Option<HashMap<String, Value>>) {
                self.script_exp_vars = vars;
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
                if $needs_internal {
                    self.internal_fixed_update(api);
                }
            }

            fn needs_internal_fixed_update(&self) -> bool {
                $needs_internal
            }

            fn get_created_timestamp(&self) -> u64 {
                // Access created_timestamp directly (works for Node and Node2D types via Deref)
                // Same pattern as get_id() and get_name() - they access directly, not through base
                self.created_timestamp
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
    };
}

/// Declares all node types and generates `NodeType` + `SceneNode` enums.
/// Also implements the `BaseNode` trait for `SceneNode` by delegating to its inner value.
///
/// Syntax: `NodeName(needs_internal_fixed_update) => path::to::NodeType`
/// where `needs_internal_fixed_update` is `true` or `false`
/// If true, the node must have an `internal_fixed_update` method in its impl block
#[macro_export]
macro_rules! define_nodes {
    ( $( $variant:ident($needs_internal:literal) => $ty:path ),+ $(,)? ) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum NodeType { $( $variant, )+ }

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
            fn get_id(&self) -> uuid::Uuid {
                match self { $( SceneNode::$variant(n) => n.get_id(), )+ }
            }

            fn get_local_id(&self) -> uuid::Uuid {
                match self { $( SceneNode::$variant(n) => n.get_local_id(), )+ }
            }

            fn set_id(&mut self, id: uuid::Uuid) {
                match self { $( SceneNode::$variant(n) => n.set_id(id), )+ }
            }

            fn set_local_id(&mut self, local_id: uuid::Uuid) {
                match self { $( SceneNode::$variant(n) => n.set_local_id(local_id), )+ }
            }

            fn get_name(&self) -> &str {
                match self { $( SceneNode::$variant(n) => n.get_name(), )+ }
            }

            fn set_name(&mut self, name: String) {
                match self { $( SceneNode::$variant(n) => n.set_name(name), )+ }
            }

            fn get_is_root_of(&self) -> Option<&str> {
                match self { $( SceneNode::$variant(n) => n.get_is_root_of(), )+ }
            }

            fn get_parent(&self) -> uuid::Uuid {
                match self { $( SceneNode::$variant(n) => n.get_parent(), )+ }
            }

            fn get_children(&self) -> &Vec<uuid::Uuid> {
                match self { $( SceneNode::$variant(n) => n.get_children(), )+ }
            }

            fn get_type(&self) -> &str {
                match self { $( SceneNode::$variant(n) => n.get_type(), )+ }
            }

            fn get_script_path(&self) -> Option<&str> {
                match self { $( SceneNode::$variant(n) => n.get_script_path(), )+ }
            }

            fn set_parent(&mut self, parent: Option<uuid::Uuid>) {
                match self { $( SceneNode::$variant(n) => n.set_parent(parent), )+ }
            }

            fn add_child(&mut self, child: uuid::Uuid) {
                match self { $( SceneNode::$variant(n) => n.add_child(child), )+ }
            }

            fn remove_child(&mut self, c: &uuid::Uuid) {
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

            fn is_dirty(&self) -> bool {
                match self { $( SceneNode::$variant(n) => n.is_dirty(), )+ }
            }

            fn set_dirty(&mut self, dirty: bool) {
                match self { $( SceneNode::$variant(n) => n.set_dirty(dirty), )+ }
            }

            fn get_children_mut(&mut self) -> &mut Vec<uuid::Uuid> {
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

            fn mark_transform_dirty_if_node2d(&mut self) {
                if let Some(node2d) = self.as_node2d_mut() {
                    node2d.transform_dirty = true;
                }
            }

            fn get_created_timestamp(&self) -> u64 {
                match self {
                    $( SceneNode::$variant(n) => n.get_created_timestamp(), )+
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
                    SceneNode::Shape2D(shape) => Some(&mut shape.base),
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
                    SceneNode::Shape2D(shape) => Some(&shape.base),
                    SceneNode::Camera2D(cam) => Some(&cam.base),
                    _ => None,
                }
            }

            /// Get the local transform if this is a Node2D-based node
            /// Uses Deref to access transform through Node2D
            pub fn get_node2d_transform(&self) -> Option<crate::structs2d::Transform2D> {
                self.as_node2d().map(|node2d| node2d.transform)
            }
        }

        $( impl_scene_node!($ty, $variant, $needs_internal); )+
    };
}

// ─────────────────────────────────────────────
// Register all built-in node types here
// ─────────────────────────────────────────────

//true means it has internal fixed update logic
define_nodes!(
    Node(false)     => crate::nodes::node::Node,
    Node2D(false)   => crate::nodes::_2d::node_2d::Node2D,
    Sprite2D(false) => crate::nodes::_2d::sprite_2d::Sprite2D,
    Area2D(true)   => crate::nodes::_2d::area_2d::Area2D,
    CollisionShape2D(false) => crate::nodes::_2d::collision_shape_2d::CollisionShape2D,
    Shape2D(false) => crate::nodes::_2d::shape_2d::Shape2D,
    Camera2D(false)  => crate::nodes::_2d::camera_2d::Camera2D,


    UINode(true)   => crate::nodes::ui_node::UINode,


    Node3D(false)   => crate::nodes::_3d::node_3d::Node3D,
    MeshInstance3D(false) => crate::nodes::_3d::mesh_instance_3d::MeshInstance3D,
    Camera3D(false)  => crate::nodes::_3d::camera_3d::Camera3D,

    DirectionalLight3D(false) => crate::nodes::_3d::light_dir_3d::DirectionalLight3D,
    OmniLight3D(false) => crate::nodes::_3d::light_omni_3d::OmniLight3D,
    SpotLight3D(false) => crate::nodes::_3d::light_spot_3d::SpotLight3D,
);

