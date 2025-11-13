use std::any::Any;
use std::fmt::Debug;
use uuid::Uuid;

use serde::{Serialize, Deserialize};

/// Base trait implemented by all engine node types.
/// Provides unified access and manipulation for all node variants stored in `SceneNode`.
pub trait BaseNode: Any + Debug + Send {
    fn get_id(&self) -> Uuid;
    fn get_local_id(&self) -> Uuid;
    fn set_id(&mut self, id: Uuid);
    fn set_local_id(&mut self, local_id: Uuid);

    fn get_name(&self) -> &str;
    fn get_parent(&self) -> Option<Uuid>;

    /// Returns a reference to the children list.
    /// If the node has `None` for its children field, this returns an empty slice.
    fn get_children(&self) -> &Vec<Uuid>;

    fn get_type(&self) -> &str;
    fn get_script_path(&self) -> Option<&str>;

    fn set_parent(&mut self, parent: Option<Uuid>);
    fn add_child(&mut self, child: Uuid);
    fn remove_child(&mut self, c: &Uuid);
    fn set_script_path(&mut self, path: &str);

    fn is_dirty(&self) -> bool;
    fn set_dirty(&mut self, dirty: bool);

    fn mark_dirty(&mut self) { self.set_dirty(true); }

    fn get_children_mut(&mut self) -> &mut Vec<Uuid>;

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn clear_children(&mut self) { self.get_children_mut().clear(); }
    fn clear_parent(&mut self) { self.set_parent(None); }
}

/// Used to unwrap `SceneNode` variants back into their concrete types.
pub trait IntoInner<T> {
    fn into_inner(self) -> T;
}

/// Common macro implementing `BaseNode` for each concrete node type.
/// This version supports `Option<Vec<Uuid>>` for `children`.
#[macro_export]
macro_rules! impl_scene_node {
    ($ty:ty, $variant:ident) => {
        impl crate::nodes::node_registry::BaseNode for $ty {
            fn get_id(&self) -> uuid::Uuid { self.id }
            fn get_local_id(&self) -> uuid::Uuid { self.local_id }
            fn set_id(&mut self, id: uuid::Uuid) { self.id = id; }
            fn set_local_id(&mut self, local_id: uuid::Uuid) { self.local_id = local_id; }

            fn get_name(&self) -> &str { &self.name }
            fn get_parent(&self) -> Option<uuid::Uuid> { self.parent }

            fn get_children(&self) -> &Vec<uuid::Uuid> {
                // Return empty vec reference if None
                static EMPTY_CHILDREN: Vec<uuid::Uuid> = Vec::new();
                match &self.children {
                    Some(children) => children,
                    None => &EMPTY_CHILDREN,
                }
            }

            fn get_type(&self) -> &str { &self.ty }

            fn get_script_path(&self) -> Option<&str> {
                self.script_path.as_deref() // This works for both Cow and Option<String>
            }

            fn set_parent(&mut self, p: Option<uuid::Uuid>) { self.parent = p; }

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

            fn is_dirty(&self) -> bool { self.dirty }
            fn set_dirty(&mut self, dirty: bool) { self.dirty = dirty; }

            fn get_children_mut(&mut self) -> &mut Vec<uuid::Uuid> {
                self.children.get_or_insert_with(Vec::new)
            }

            fn as_any(&self) -> &dyn std::any::Any { self }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
        }

        impl $ty {
            /// Converts this specific node into a generic `SceneNode` enum variant.
            pub fn to_scene_node(self) -> crate::nodes::node_registry::SceneNode {
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
#[macro_export]
macro_rules! define_nodes {
    ( $( $variant:ident => $ty:path ),+ $(,)? ) => {
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

            fn get_parent(&self) -> Option<uuid::Uuid> {
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
        }

        $( impl_scene_node!($ty, $variant); )+
    };
}

// ─────────────────────────────────────────────
// Register all built-in node types here
// ─────────────────────────────────────────────
define_nodes!(
    Node     => crate::nodes::node::Node,
    Node2D   => crate::nodes::_2d::node2d::Node2D,
    Sprite2D => crate::nodes::_2d::sprite2d::Sprite2D,
    UINode   => crate::nodes::ui_node::UINode,
);