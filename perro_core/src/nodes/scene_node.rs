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
/// We include Serialize/Deserialize so the enum can derive them.
#[enum_dispatch]
pub trait BaseNode: Any + Debug + Send + Serialize +
    for<'de> Deserialize<'de>
{
    // read-only accessors
    fn get_id(&self) -> &Uuid;
    fn set_id(&mut self, id: Uuid);
    fn get_name(&self) -> &str;
    fn get_parent(&self) -> Option<Uuid>;
    fn get_children(&self) -> &Vec<Uuid>;
    fn get_type(&self) -> &str;
    fn get_script_path(&self) -> Option<&String>;

    // mutation
    fn set_parent(&mut self, parent: Option<Uuid>);
    fn add_child(&mut self, child: Uuid);
    fn remove_child(&mut self, c: &Uuid);
    fn set_script_path(&mut self, path: &str);

    // needed if you only serialize parent pointers
    fn get_children_mut(&mut self) -> &mut Vec<Uuid>;

    // for down-casting
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    // convenience “clear” APIs
    /// Removes *all* children.
    fn clear_children(&mut self) {
        self.get_children_mut().clear();
    }

    /// Un-parents this node (sets its parent to `None`).
    fn clear_parent(&mut self) {
        self.set_parent(None);
    }
}

#[macro_export]
macro_rules! impl_scene_node {
    ($ty:ty) => {
        impl crate::nodes::scene_node::BaseNode for $ty {
            // readonly getters
            fn get_id(&self) -> &uuid::Uuid {
                &self.id
            }
            fn set_id(&mut self, id: uuid::Uuid) {
                self.id = id;
            }
            fn get_name(&self) -> &str {
                &self.name
            }
            fn get_parent(&self) -> Option<uuid::Uuid> {
                self.parent
            }
            fn get_children(&self) -> &Vec<uuid::Uuid> {
                &self.children
            }
            fn get_type(&self) -> &str {
                &self.ty
            }
            fn get_script_path(&self) -> Option<&String> {
                self.script_path.as_ref()
            }

            // mutation
            fn set_parent(&mut self, p: Option<uuid::Uuid>) {
                self.parent = p;
            }
            fn add_child(&mut self, c: uuid::Uuid) {
                self.children.push(c);
            }
            fn remove_child(&mut self, c: &uuid::Uuid) {
                self.children.retain(|x| x != c);
            }
            fn set_script_path(&mut self, path: &str) {
                self.script_path = Some(path.to_string());
            }

            // needed to rebuild children from parents on load
            fn get_children_mut(&mut self) -> &mut Vec<uuid::Uuid> {
                &mut self.children
            }

            // down-casting helpers
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }
    };
}

/// The enum that holds any of your node types.
/// enum_dispatch will auto-generate:
///   impl BaseNode for SceneNode { /* forward each method */ }
#[enum_dispatch(BaseNode)]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum SceneNode {
    Node(Node),
    Node2D(Node2D),
    Sprite2D(Sprite2D),
    UI(Ui)
}