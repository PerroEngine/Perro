use crate::ids::NodeID;
use crate::mesh_instance_3d::MeshInstance3D;
use crate::node_2d::node_2d::Node2D;
use crate::node_3d::node_3d::Node3D;
use crate::sprite_2d::Sprite2D;
use std::borrow::Cow;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Spatial {
    None,
    TwoD,
    ThreeD,
}

#[macro_export]
macro_rules! define_scene_nodes {
    (
        base: { $($base_variant:ident $(=> $base_ty:ty)?),* $(,)? }
        2d: { $($variant_2d:ident => $ty_2d:ty),* $(,)? }
        3d: { $($variant_3d:ident => $ty_3d:ty),* $(,)? }
    ) => {
        #[derive(Clone, Debug)]
        pub struct SceneNode {
            pub id: NodeID,
            pub name: Cow<'static, str>,
            pub parent: Option<NodeID>,
            pub children: Option<Cow<'static, [NodeID]>>,
            pub script: Option<Cow<'static, str>>,
            pub data: SceneNodeData,
        }

        #[derive(Clone, Debug)]
        pub enum SceneNodeData {
            $(
                $base_variant$(($base_ty))?,
            )*
            $($variant_2d($ty_2d),)*
            $($variant_3d($ty_3d),)*
        }

        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum NodeType {
            $($base_variant,)*
            $($variant_2d,)*
            $($variant_3d,)*
        }

        impl SceneNode {
            pub fn new(data: SceneNodeData) -> Self {
                Self {
                    id: NodeID::nil(),
                    name: Cow::Borrowed("Node"),
                    parent: None,
                    children: None,
                    script: None,
                    data,
                }
            }

            pub fn node_type(&self) -> NodeType {
                match &self.data {
                    $(
                        SceneNodeData::$base_variant { .. } =>
                            NodeType::$base_variant,
                    )*
                    $(
                        SceneNodeData::$variant_2d(_) =>
                            NodeType::$variant_2d,
                    )*
                    $(
                        SceneNodeData::$variant_3d(_) =>
                            NodeType::$variant_3d,
                    )*
                }
            }

            pub fn spatial(&self) -> Spatial {
                match &self.data {
                    $(
                        SceneNodeData::$base_variant { .. } => Spatial::None,
                    )*
                    $(SceneNodeData::$variant_2d(_) => Spatial::TwoD,)*
                    $(SceneNodeData::$variant_3d(_) => Spatial::ThreeD,)*
                }
            }

            pub fn is_2d(&self) -> bool {
                self.spatial() == Spatial::TwoD
            }

            pub fn is_3d(&self) -> bool {
                self.spatial() == Spatial::ThreeD
            }

            pub fn is_spatial(&self) -> bool {
                matches!(self.spatial(), Spatial::TwoD | Spatial::ThreeD)
            }

            pub fn add_child(&mut self, child: NodeID) {
                self.children
                    .get_or_insert_with(|| Cow::Owned(Vec::new()))
                    .to_mut()
                    .push(child);
            }

            pub fn remove_child(&mut self, child: NodeID) {
                if let Some(children) = &mut self.children {
                    children.to_mut().retain(|&c| c != child);
                }
            }

            pub fn clear_children(&mut self) {
                self.children = None;
            }

            pub fn children_slice(&self) -> &[NodeID] {
                self.children
                    .as_ref()
                    .map(|cow| cow.as_ref())
                    .unwrap_or(&[])
            }
        }

        impl Default for NodeType {
            fn default() -> Self {
                NodeType::Node
            }
        }

        impl std::fmt::Display for NodeType {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.as_str())
            }
        }

        impl std::str::FromStr for NodeType {
            type Err = String;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $(stringify!($base_variant) => Ok(NodeType::$base_variant),)*
                    $(stringify!($variant_2d) => Ok(NodeType::$variant_2d),)*
                    $(stringify!($variant_3d) => Ok(NodeType::$variant_3d),)*
                    _ => Err(format!("Unknown node type: {}", s)),
                }
            }
        }

        impl NodeType {
            pub fn as_str(&self) -> &'static str {
                match self {
                    $(NodeType::$base_variant => stringify!($base_variant),)*
                    $(NodeType::$variant_2d => stringify!($variant_2d),)*
                    $(NodeType::$variant_3d => stringify!($variant_3d),)*
                }
            }

            pub const fn get_spatial(&self) -> Spatial {
                match self {
                    $(NodeType::$base_variant => Spatial::None,)*
                    $(NodeType::$variant_2d => Spatial::TwoD,)*
                    $(NodeType::$variant_3d => Spatial::ThreeD,)*
                }
            }

            pub const fn is_2d(&self) -> bool {
                matches!(self.get_spatial(), Spatial::TwoD)
            }

            pub const fn is_3d(&self) -> bool {
                matches!(self.get_spatial(), Spatial::ThreeD)
            }

            pub const fn is_spatial(&self) -> bool {
                matches!(
                    self.get_spatial(),
                    Spatial::TwoD | Spatial::ThreeD
                )
            }
        }
    };
}

// ======================================================================
//                          DEFINE NODES
// ======================================================================

define_scene_nodes! {
    base: {
        Node,
    }
    2d: {
        Node2D => Node2D,
        Sprite2D => Sprite2D,
    }
    3d: {
        Node3D => Node3D,
        MeshInstance3D => MeshInstance3D,
    }
}
