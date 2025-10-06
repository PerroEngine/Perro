use std::any::Any;
use std::fmt::Debug;

use enum_dispatch::enum_dispatch;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Pull in your concrete node types:
use crate::nodes::node::Node;
use crate::nodes::_2d::node2d::Node2D;
use crate::nodes::_2d::sprite2d::Sprite2D;
use crate::ui_node::Ui;

/// The “base-node” trait for all your node types.
#[enum_dispatch]
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

pub trait IntoInner<T> {
    fn into_inner(self) -> T;
}

/// Macro to implement `BaseNode` for any concrete type with the expected fields.
/// Also implements a `From` conversion into `SceneNode`.
#[macro_export]
macro_rules! impl_scene_node {
    ($ty:ty, $variant:ident) => {
        impl crate::nodes::scene_node::BaseNode for $ty {
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
            pub fn to_scene_node(self) -> crate::nodes::scene_node::SceneNode {
                crate::nodes::scene_node::SceneNode::$variant(self)
            }
        }

        impl crate::nodes::scene_node::IntoInner<$ty> for crate::nodes::scene_node::SceneNode {
            fn into_inner(self) -> $ty {
                match self {
                    crate::nodes::scene_node::SceneNode::$variant(inner) => inner,
                    other => panic!(
                        "Cannot extract {} from {}",
                        stringify!($ty),
                        other.get_type()
                    ),
                }
            }
        }
    };
}


/// The enum that holds any of your node types.
/// `enum_dispatch` auto-generates:
///   impl BaseNode for SceneNode { /* forwards methods */ }
#[enum_dispatch(BaseNode)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SceneNode {
    Node(Node),
    Node2D(Node2D),
    Sprite2D(Sprite2D),
    UI(Ui),
}

impl_scene_node!(Node, Node);
impl_scene_node!(Node2D, Node2D);
impl_scene_node!(Sprite2D, Sprite2D);
impl_scene_node!(Ui, UI);