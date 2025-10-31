use std::any::Any;
use std::fmt::Debug;
use uuid::Uuid;

use serde::{Serialize, Deserialize};

/// Base trait implemented by all engine node types
pub trait BaseNode: Any + Debug + Send {
    fn get_id(&self) -> &Uuid;
    fn set_id(&mut self, id: Uuid);
    fn get_name(&self) -> &str;
    fn get_parent(&self) -> Option<Uuid>;
    fn get_children(&self) -> &Vec<Uuid>;
    fn get_type(&self) -> &str;
    fn get_script_path(&self) -> Option<&String>;

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

/// Used to unwrap enums back into concrete types
pub trait IntoInner<T> {
    fn into_inner(self) -> T;
}

/// Common macro implementing `BaseNode` for concrete node types
#[macro_export]
macro_rules! impl_scene_node {
    ($ty:ty, $variant:ident) => {
        impl crate::nodes::node_registry::BaseNode for $ty {
            fn get_id(&self) -> &uuid::Uuid { &self.id }
            fn set_id(&mut self, id: uuid::Uuid) { self.id = id; }
            fn get_name(&self) -> &str { &self.name }
            fn get_parent(&self) -> Option<uuid::Uuid> { self.parent }
            fn get_children(&self) -> &Vec<uuid::Uuid> { &self.children }
            fn get_type(&self) -> &str { &self.ty }
            fn get_script_path(&self) -> Option<&String> { self.script_path.as_ref() }

            fn set_parent(&mut self, p: Option<uuid::Uuid>) { self.parent = p; }
            fn add_child(&mut self, c: uuid::Uuid) { self.children.push(c); }
            fn remove_child(&mut self, c: &uuid::Uuid) { self.children.retain(|x| x != c); }
            fn set_script_path(&mut self, path: &str) { self.script_path = Some(path.to_string()); }

            fn is_dirty(&self) -> bool { self.dirty }
            fn set_dirty(&mut self, dirty: bool) { self.dirty = dirty; }
            fn get_children_mut(&mut self) -> &mut Vec<uuid::Uuid> { &mut self.children }

            fn as_any(&self) -> &dyn std::any::Any { self }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
        }

        impl $ty {
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

/// Generates the NodeType + SceneNode enums and auto‑implements BaseNode via impl_scene_node!
macro_rules! define_nodes {
    ( $( $variant:ident => $ty:path ),+ $(,)? ) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum NodeType { $( $variant, )+ }

        #[derive(Debug, Clone, Serialize, Deserialize)]
        #[serde(untagged)]
        pub enum SceneNode {
            $( $variant($ty), )+
        }

        // Manual implementation of BaseNode for SceneNode
        impl BaseNode for SceneNode {
            fn get_id(&self) -> &Uuid {
                match self {
                    $( SceneNode::$variant(n) => n.get_id(), )+
                }
            }

            fn set_id(&mut self, id: Uuid) {
                match self {
                    $( SceneNode::$variant(n) => n.set_id(id), )+
                }
            }

            fn get_name(&self) -> &str {
                match self {
                    $( SceneNode::$variant(n) => n.get_name(), )+
                }
            }

            fn get_parent(&self) -> Option<Uuid> {
                match self {
                    $( SceneNode::$variant(n) => n.get_parent(), )+
                }
            }

            fn get_children(&self) -> &Vec<Uuid> {
                match self {
                    $( SceneNode::$variant(n) => n.get_children(), )+
                }
            }

            fn get_type(&self) -> &str {
                match self {
                    $( SceneNode::$variant(n) => n.get_type(), )+
                }
            }

            fn get_script_path(&self) -> Option<&String> {
                match self {
                    $( SceneNode::$variant(n) => n.get_script_path(), )+
                }
            }

            fn set_parent(&mut self, parent: Option<Uuid>) {
                match self {
                    $( SceneNode::$variant(n) => n.set_parent(parent), )+
                }
            }

            fn add_child(&mut self, child: Uuid) {
                match self {
                    $( SceneNode::$variant(n) => n.add_child(child), )+
                }
            }

            fn remove_child(&mut self, c: &Uuid) {
                match self {
                    $( SceneNode::$variant(n) => n.remove_child(c), )+
                }
            }

            fn set_script_path(&mut self, path: &str) {
                match self {
                    $( SceneNode::$variant(n) => n.set_script_path(path), )+
                }
            }

            fn is_dirty(&self) -> bool {
                match self {
                    $( SceneNode::$variant(n) => n.is_dirty(), )+
                }
            }

            fn set_dirty(&mut self, dirty: bool) {
                match self {
                    $( SceneNode::$variant(n) => n.set_dirty(dirty), )+
                }
            }

            fn get_children_mut(&mut self) -> &mut Vec<Uuid> {
                match self {
                    $( SceneNode::$variant(n) => n.get_children_mut(), )+
                }
            }

            fn as_any(&self) -> &dyn Any {
                match self {
                    $( SceneNode::$variant(n) => n.as_any(), )+
                }
            }

            fn as_any_mut(&mut self) -> &mut dyn Any {
                match self {
                    $( SceneNode::$variant(n) => n.as_any_mut(), )+
                }
            }
        }

        $( impl_scene_node!($ty, $variant); )+
    };
}

// ─────────────────────────────────────────────
// Declare every node once right here.
// Adding new types is a single line.
// ─────────────────────────────────────────────
define_nodes!(
    Node     => crate::nodes::node::Node,
    Node2D   => crate::nodes::_2d::node2d::Node2D,
    Sprite2D => crate::nodes::_2d::sprite2d::Sprite2D,
    UINode   => crate::nodes::ui_node::UINode,
);