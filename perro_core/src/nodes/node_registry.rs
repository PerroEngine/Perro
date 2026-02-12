use crate::mesh_instance_3d::MeshInstance3D;
use crate::node_2d::node_2d::Node2D;
use crate::node_3d::node_3d::Node3D;
use crate::sprite_2d::Sprite2D;
use perro_ids::NodeID;
use std::borrow::Cow;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
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
            pub data: SceneNodeData,
            pub id: NodeID,
            pub name: Cow<'static, str>,
            pub parent: NodeID,
            pub children: Option<Cow<'static, [NodeID]>>,
            pub script: Option<Cow<'static, str>>,
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
        #[repr(u8)]
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
                    parent: NodeID::nil(),
                    children: None,
                    script: None,
                    data,
                }
            }

            pub fn has_parent(&self) -> bool {
                !self.parent.is_nil()
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

            pub fn with_typed_ref<T: NodeTypeDispatch, R>(
                &self,
                f: impl FnOnce(&T) -> R,
            ) -> Option<R> {
                T::with_ref(&self.data, f)
            }

            pub fn with_typed_mut<T: NodeTypeDispatch, R>(
                &mut self,
                f: impl FnOnce(&mut T) -> R,
            ) -> Option<R> {
                T::with_mut(&mut self.data, f)
            }
        }

        pub trait NodeTypeDispatch: Sized {
            const NODE_TYPE: NodeType;
            const SPATIAL: Spatial;

            fn with_ref<R>(data: &SceneNodeData, f: impl FnOnce(&Self) -> R) -> Option<R>;
            fn with_mut<R>(data: &mut SceneNodeData, f: impl FnOnce(&mut Self) -> R)
                -> Option<R>;
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

        $(impl From<$ty_2d> for SceneNodeData {
            fn from(value: $ty_2d) -> Self {
                SceneNodeData::$variant_2d(value)
            }
        })*

        $(impl From<$ty_3d> for SceneNodeData {
            fn from(value: $ty_3d) -> Self {
                SceneNodeData::$variant_3d(value)
            }
        })*

        impl From<SceneNodeData> for SceneNode {
            fn from(value: SceneNodeData) -> Self {
                SceneNode::new(value)
            }
        }

        $(impl NodeTypeDispatch for $ty_2d {
            const NODE_TYPE: NodeType = NodeType::$variant_2d;
            const SPATIAL: Spatial = Spatial::TwoD;

            fn with_ref<R>(data: &SceneNodeData, f: impl FnOnce(&Self) -> R) -> Option<R> {
                match data {
                    SceneNodeData::$variant_2d(inner) => Some(f(inner)),
                    _ => None,
                }
            }

            fn with_mut<R>(
                data: &mut SceneNodeData,
                f: impl FnOnce(&mut Self) -> R,
            ) -> Option<R> {
                match data {
                    SceneNodeData::$variant_2d(inner) => Some(f(inner)),
                    _ => None,
                }
            }
        })*

        $(impl NodeTypeDispatch for $ty_3d {
            const NODE_TYPE: NodeType = NodeType::$variant_3d;
            const SPATIAL: Spatial = Spatial::ThreeD;

            fn with_ref<R>(data: &SceneNodeData, f: impl FnOnce(&Self) -> R) -> Option<R> {
                match data {
                    SceneNodeData::$variant_3d(inner) => Some(f(inner)),
                    _ => None,
                }
            }

            fn with_mut<R>(
                data: &mut SceneNodeData,
                f: impl FnOnce(&mut Self) -> R,
            ) -> Option<R> {
                match data {
                    SceneNodeData::$variant_3d(inner) => Some(f(inner)),
                    _ => None,
                }
            }
        })*
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_typed_ref_returns_some_for_matching_type() {
        let scene = SceneNode::new(SceneNodeData::Node2D(Node2D::new()));
        let visible = scene.with_typed_ref::<Node2D, _>(|n| n.visible);
        assert_eq!(visible, Some(true));
    }

    #[test]
    fn with_typed_ref_returns_none_for_mismatched_type() {
        let scene = SceneNode::new(SceneNodeData::Node2D(Node2D::new()));
        let value = scene.with_typed_ref::<Node3D, _>(|_| 1usize);
        assert_eq!(value, None);
    }

    #[test]
    fn with_typed_mut_applies_mutation_for_matching_type() {
        let mut scene = SceneNode::new(SceneNodeData::Node2D(Node2D::new()));

        let result = scene.with_typed_mut::<Node2D, _>(|n| {
            n.z_index = 42;
        });

        assert!(result.is_some());
        let z_index = scene.with_typed_ref::<Node2D, _>(|n| n.z_index);
        assert_eq!(z_index, Some(42));
    }

    #[test]
    fn with_typed_mut_returns_none_for_mismatched_type() {
        let mut scene = SceneNode::new(SceneNodeData::Node2D(Node2D::new()));

        let result = scene.with_typed_mut::<Node3D, _>(|_| {});
        assert!(result.is_none());

        let z_index = scene.with_typed_ref::<Node2D, _>(|n| n.z_index);
        assert_eq!(z_index, Some(0));
    }

    #[test]
    fn node_type_dispatch_constants_match_expected_variants() {
        assert_eq!(<Node2D as NodeTypeDispatch>::NODE_TYPE, NodeType::Node2D);
        assert_eq!(<Node2D as NodeTypeDispatch>::SPATIAL, Spatial::TwoD);

        assert_eq!(
            <MeshInstance3D as NodeTypeDispatch>::NODE_TYPE,
            NodeType::MeshInstance3D
        );
        assert_eq!(
            <MeshInstance3D as NodeTypeDispatch>::SPATIAL,
            Spatial::ThreeD
        );
    }

    #[test]
    fn from_impls_convert_payload_to_scene_node_data_and_scene_node() {
        let data: SceneNodeData = Node2D::new().into();
        assert!(matches!(data, SceneNodeData::Node2D(_)));

        let scene: SceneNode = data.into();
        assert!(matches!(scene.data, SceneNodeData::Node2D(_)));
    }
}
