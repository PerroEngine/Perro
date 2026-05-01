use crate::ambient_light_3d::AmbientLight3D;
use crate::animation_player::AnimationPlayer;
use crate::camera_2d::Camera2D;
use crate::camera_3d::Camera3D;
use crate::mesh_instance_3d::MeshInstance3D;
use crate::multi_mesh_instance_3d::MultiMeshInstance3D;
use crate::node_2d::Node2D;
use crate::node_3d::Node3D;
use crate::particle_emitter_3d::ParticleEmitter3D;
use crate::physics_2d::{Area2D, CollisionShape2D, RigidBody2D, StaticBody2D};
use crate::physics_3d::{Area3D, CollisionShape3D, RigidBody3D, StaticBody3D};
use crate::point_light_3d::PointLight3D;
use crate::ray_light_3d::RayLight3D;
use crate::skeleton_3d::Skeleton3D;
use crate::sky_3d::Sky3D;
use crate::spot_light_3d::SpotLight3D;
use crate::sprite_2d::Sprite2D;
use perro_ids::{NodeID, TagID};
use perro_structs::{Transform2D, Transform3D};
use perro_ui::{
    UiBox, UiButton, UiGrid, UiHLayout, UiLabel, UiLayout, UiNodeBase, UiPanel, UiTextBlock,
    UiTextBox, UiVLayout,
};
use std::borrow::Cow;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Spatial {
    None,
    TwoD,
    ThreeD,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Renderable {
    False,
    True,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum InternalUpdate {
    False,
    True,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum InternalFixedUpdate {
    False,
    True,
}

#[macro_export]
macro_rules! __node_parent_opt {
    (None) => {
        None
    };
    ($parent:ident) => {
        Some(NodeType::$parent)
    };
}

#[macro_export]
macro_rules! __node2d_base_expr {
    (Node2D, None, $inner:ident, $f:ident) => {
        Some($f($inner))
    };
    ($_variant:ident, None, $inner:ident, $_f:ident) => {{
        let _ = &$inner;
        None
    }};
    ($_variant:ident, $parent:ident, $inner:ident, $f:ident) => {
        Some($f($inner))
    };
}

#[macro_export]
macro_rules! __node3d_base_expr {
    (Node3D, None, $inner:ident, $f:ident) => {
        Some($f($inner))
    };
    ($_variant:ident, None, $inner:ident, $_f:ident) => {{
        let _ = &$inner;
        None
    }};
    ($_variant:ident, $parent:ident, $inner:ident, $f:ident) => {
        Some($f($inner))
    };
}

#[macro_export]
macro_rules! __ui_base_expr {
    (UiBox, None, $inner:ident, $f:ident) => {
        Some($f($inner))
    };
    ($_variant:ident, None, $inner:ident, $_f:ident) => {{
        let _ = &$inner;
        None
    }};
    ($_variant:ident, $parent:ident, $inner:ident, $f:ident) => {
        Some($f($inner.ui_base()))
    };
}

#[macro_export]
macro_rules! __ui_base_mut_expr {
    (UiBox, None, $inner:ident, $f:ident) => {
        Some($f($inner))
    };
    ($_variant:ident, None, $inner:ident, $_f:ident) => {{
        let _ = &$inner;
        None
    }};
    ($_variant:ident, $parent:ident, $inner:ident, $f:ident) => {
        Some($f($inner.ui_base_mut()))
    };
}

#[macro_export]
macro_rules! __impl_exact_node_base_dispatch_2d {
    (Node2D, $ty_2d:ty, $variant_2d:ident) => {};
    ($variant:ident, $ty_2d:ty, $variant_2d:ident) => {
        impl NodeBaseDispatch for $ty_2d {
            const BASE_NODE_TYPE: NodeType = NodeType::$variant_2d;

            fn with_base_ref<R>(data: &SceneNodeData, f: impl FnOnce(&Self) -> R) -> Option<R> {
                match data {
                    SceneNodeData::$variant_2d(inner) => Some(f(inner)),
                    _ => None,
                }
            }

            fn with_base_mut<R>(
                data: &mut SceneNodeData,
                f: impl FnOnce(&mut Self) -> R,
            ) -> Option<R> {
                match data {
                    SceneNodeData::$variant_2d(inner) => Some(f(inner)),
                    _ => None,
                }
            }
        }
    };
}

#[macro_export]
macro_rules! __impl_exact_node_base_dispatch_3d {
    (Node3D, $ty_3d:ty, $variant_3d:ident) => {};
    ($variant:ident, $ty_3d:ty, $variant_3d:ident) => {
        impl NodeBaseDispatch for $ty_3d {
            const BASE_NODE_TYPE: NodeType = NodeType::$variant_3d;

            fn with_base_ref<R>(data: &SceneNodeData, f: impl FnOnce(&Self) -> R) -> Option<R> {
                match data {
                    SceneNodeData::$variant_3d(inner) => Some(f(inner)),
                    _ => None,
                }
            }

            fn with_base_mut<R>(
                data: &mut SceneNodeData,
                f: impl FnOnce(&mut Self) -> R,
            ) -> Option<R> {
                match data {
                    SceneNodeData::$variant_3d(inner) => Some(f(inner)),
                    _ => None,
                }
            }
        }
    };
}

#[macro_export]
macro_rules! __impl_exact_node_base_dispatch_ui {
    (UiBox, $ty_ui:ty, $variant_ui:ident) => {};
    ($variant:ident, $ty_ui:ty, $variant_ui:ident) => {
        impl NodeBaseDispatch for $ty_ui {
            const BASE_NODE_TYPE: NodeType = NodeType::$variant_ui;

            fn with_base_ref<R>(data: &SceneNodeData, f: impl FnOnce(&Self) -> R) -> Option<R> {
                match data {
                    SceneNodeData::$variant_ui(inner) => Some(f(inner)),
                    _ => None,
                }
            }

            fn with_base_mut<R>(
                data: &mut SceneNodeData,
                f: impl FnOnce(&mut Self) -> R,
            ) -> Option<R> {
                match data {
                    SceneNodeData::$variant_ui(inner) => Some(f(inner)),
                    _ => None,
                }
            }
        }
    };
}

#[macro_export]
macro_rules! define_scene_nodes {
    (
        base: { $($base_variant:ident $(=> $base_ty:ty)?),* $(,)? }
        2d: { $($variant_2d:ident => ($parent_2d:ident, $ty_2d:ty, $renderable_2d:expr, $internal_update_2d:expr, $internal_fixed_update_2d:expr)),* $(,)? }
        3d: { $($variant_3d:ident => ($parent_3d:ident, $ty_3d:ty, $renderable_3d:expr, $internal_update_3d:expr, $internal_fixed_update_3d:expr)),* $(,)? }
        ui: { $($variant_ui:ident => ($parent_ui:ident, $ty_ui:ty, $renderable_ui:expr, $internal_update_ui:expr, $internal_fixed_update_ui:expr)),* $(,)? }
        resource: { $($variant_resource:ident => ($parent_resource:ident, $ty_resource:ty, $renderable_resource:expr, $internal_update_resource:expr, $internal_fixed_update_resource:expr)),* $(,)? }
    ) => {
        #[derive(Clone, Debug)]
        pub struct SceneNode {
            pub data: SceneNodeData,
            pub id: NodeID,
            pub name: Cow<'static, str>,
            pub parent: NodeID,
            pub children: Option<Cow<'static, [NodeID]>>,
            pub tags: Option<Cow<'static, [TagID]>>,
        }

        #[derive(Clone, Debug)]
        #[allow(clippy::large_enum_variant)]
        pub enum SceneNodeData {
            $(
                $base_variant$(($base_ty))?,
            )*
            $($variant_2d($ty_2d),)*
            $($variant_3d($ty_3d),)*
            $($variant_ui($ty_ui),)*
            $($variant_resource($ty_resource),)*
        }

        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[repr(u8)]
        pub enum NodeType {
            $($base_variant,)*
            $($variant_2d,)*
            $($variant_3d,)*
            $($variant_ui,)*
            $($variant_resource,)*
        }

        impl SceneNode {
            pub const fn new(data: SceneNodeData) -> Self {
                Self {
                    id: NodeID::nil(),
                    name: Cow::Borrowed("Node"),
                    parent: NodeID::nil(),
                    children: None,
                    tags: None,
                    data,
                }
            }

            pub const fn has_parent(&self) -> bool {
                !self.parent.is_nil()
            }

            pub fn get_name(&self) -> &str {
                self.name.as_ref()
            }

            pub fn set_name<S>(&mut self, name: S)
            where
                S: Into<Cow<'static, str>>,
            {
                self.name = name.into();
            }

            pub const fn get_parent(&self) -> NodeID {
                self.parent
            }

            pub fn get_children_ids(&self) -> &[NodeID] {
                self.children
                    .as_ref()
                    .map(|cow| cow.as_ref())
                    .unwrap_or(&[])
            }

            pub fn set_children_ids<C>(&mut self, children: Option<C>)
            where
                C: Into<Cow<'static, [NodeID]>>,
            {
                self.children = children.map(Into::into);
            }

            pub fn get_tag_ids(&self) -> &[TagID] {
                self.tags
                    .as_ref()
                    .map(|cow| cow.as_ref())
                    .unwrap_or(&[])
            }

            pub fn set_tag_ids<T>(&mut self, tags: Option<T>)
            where
                T: Into<Cow<'static, [TagID]>>,
            {
                self.tags = tags.map(Into::into);
            }

            pub const fn node_type(&self) -> NodeType {
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
                    $(
                        SceneNodeData::$variant_ui(_) =>
                            NodeType::$variant_ui,
                    )*
                    $(
                        SceneNodeData::$variant_resource(_) =>
                            NodeType::$variant_resource,
                    )*
                }
            }

            pub const fn spatial(&self) -> Spatial {
                match &self.data {
                    $(
                        SceneNodeData::$base_variant { .. } => Spatial::None,
                    )*
                    $(SceneNodeData::$variant_2d(_) => Spatial::TwoD,)*
                    $(SceneNodeData::$variant_3d(_) => Spatial::ThreeD,)*
                    $(SceneNodeData::$variant_ui(_) => Spatial::None,)*
                    $(SceneNodeData::$variant_resource(_) => Spatial::None,)*
                }
            }

            pub const fn is_2d(&self) -> bool {
                matches!(self.spatial(), Spatial::TwoD)
            }

            pub const fn is_3d(&self) -> bool {
                matches!(self.spatial(), Spatial::ThreeD)
            }

            pub const fn is_spatial(&self) -> bool {
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
                self.get_children_ids()
            }

            pub fn add_tag(&mut self, tag: TagID) {
                self.tags
                    .get_or_insert_with(|| Cow::Owned(Vec::new()))
                    .to_mut()
                    .push(tag);
            }

            pub fn remove_tag(&mut self, tag: TagID) {
                if let Some(tags) = &mut self.tags {
                    tags.to_mut().retain(|&t| t != tag);
                }
            }

            pub fn clear_tags(&mut self) {
                self.tags = None;
            }

            pub fn has_tag(&self, tag: TagID) -> bool {
                self.get_tag_ids().contains(&tag)
            }

            pub fn tags_slice(&self) -> &[TagID] {
                self.get_tag_ids()
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

            pub fn with_base_ref<T: NodeBaseDispatch, R>(
                &self,
                f: impl FnOnce(&T) -> R,
            ) -> Option<R> {
                T::with_base_ref(&self.data, f)
            }

            pub fn with_base_mut<T: NodeBaseDispatch, R>(
                &mut self,
                f: impl FnOnce(&mut T) -> R,
            ) -> Option<R> {
                T::with_base_mut(&mut self.data, f)
            }
        }

        pub trait NodeTypeDispatch: Sized {
            const NODE_TYPE: NodeType;
            const SPATIAL: Spatial;
            const RENDERABLE: Renderable;
            const INTERNAL_UPDATE: InternalUpdate;
            const INTERNAL_FIXED_UPDATE: InternalFixedUpdate;
            type TransformSnapshot: Copy + PartialEq;

            fn with_ref<R>(data: &SceneNodeData, f: impl FnOnce(&Self) -> R) -> Option<R>;
            fn with_mut<R>(data: &mut SceneNodeData, f: impl FnOnce(&mut Self) -> R)
                -> Option<R>;

            #[inline]
            fn snapshot_transform(_value: &Self) -> Option<Self::TransformSnapshot> {
                None
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
                    $(stringify!($variant_ui) => Ok(NodeType::$variant_ui),)*
                    $(stringify!($variant_resource) => Ok(NodeType::$variant_resource),)*
                    _ => Err(format!("Unknown node type: {}", s)),
                }
            }
        }

        impl NodeType {
            pub const ALL: &'static [NodeType] = &[
                $(NodeType::$base_variant,)*
                $(NodeType::$variant_2d,)*
                $(NodeType::$variant_3d,)*
                $(NodeType::$variant_ui,)*
                $(NodeType::$variant_resource,)*
            ];

            pub const fn as_str(&self) -> &'static str {
                match self {
                    $(NodeType::$base_variant => stringify!($base_variant),)*
                    $(NodeType::$variant_2d => stringify!($variant_2d),)*
                    $(NodeType::$variant_3d => stringify!($variant_3d),)*
                    $(NodeType::$variant_ui => stringify!($variant_ui),)*
                    $(NodeType::$variant_resource => stringify!($variant_resource),)*
                }
            }

            pub const fn parent_type(&self) -> Option<NodeType> {
                match self {
                    $(NodeType::$base_variant => None,)*
                    $(NodeType::$variant_2d => $crate::__node_parent_opt!($parent_2d),)*
                    $(NodeType::$variant_3d => $crate::__node_parent_opt!($parent_3d),)*
                    $(NodeType::$variant_ui => $crate::__node_parent_opt!($parent_ui),)*
                    $(NodeType::$variant_resource => $crate::__node_parent_opt!($parent_resource),)*
                }
            }

            pub fn is_a(&self, base: NodeType) -> bool {
                if *self == base {
                    return true;
                }

                let mut cursor = *self;
                loop {
                    match cursor.parent_type() {
                        Some(parent) => {
                            if parent == base {
                                return true;
                            }
                            cursor = parent;
                        }
                        None => return false,
                    }
                }
            }

            pub const fn get_spatial(&self) -> Spatial {
                match self {
                    $(NodeType::$base_variant => Spatial::None,)*
                    $(NodeType::$variant_2d => Spatial::TwoD,)*
                    $(NodeType::$variant_3d => Spatial::ThreeD,)*
                    $(NodeType::$variant_ui => Spatial::None,)*
                    $(NodeType::$variant_resource => Spatial::None,)*
                }
            }

            pub const fn get_internal_update(&self) -> InternalUpdate {
                match self {
                    $(NodeType::$base_variant => InternalUpdate::False,)*
                    $(NodeType::$variant_2d => $internal_update_2d,)*
                    $(NodeType::$variant_3d => $internal_update_3d,)*
                    $(NodeType::$variant_ui => $internal_update_ui,)*
                    $(NodeType::$variant_resource => $internal_update_resource,)*
                }
            }

            pub const fn get_internal_fixed_update(&self) -> InternalFixedUpdate {
                match self {
                    $(NodeType::$base_variant => InternalFixedUpdate::False,)*
                    $(NodeType::$variant_2d => $internal_fixed_update_2d,)*
                    $(NodeType::$variant_3d => $internal_fixed_update_3d,)*
                    $(NodeType::$variant_ui => $internal_fixed_update_ui,)*
                    $(NodeType::$variant_resource => $internal_fixed_update_resource,)*
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

        $(impl From<$ty_ui> for SceneNodeData {
            fn from(value: $ty_ui) -> Self {
                SceneNodeData::$variant_ui(value)
            }
        })*

        $(impl From<$ty_resource> for SceneNodeData {
            fn from(value: $ty_resource) -> Self {
                SceneNodeData::$variant_resource(value)
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
            const RENDERABLE: Renderable = $renderable_2d;
            const INTERNAL_UPDATE: InternalUpdate = $internal_update_2d;
            const INTERNAL_FIXED_UPDATE: InternalFixedUpdate = $internal_fixed_update_2d;
            type TransformSnapshot = Transform2D;

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

            #[inline]
            fn snapshot_transform(value: &Self) -> Option<Self::TransformSnapshot> {
                Some(value.transform)
            }
        })*

        $(impl NodeTypeDispatch for $ty_3d {
            const NODE_TYPE: NodeType = NodeType::$variant_3d;
            const SPATIAL: Spatial = Spatial::ThreeD;
            const RENDERABLE: Renderable = $renderable_3d;
            const INTERNAL_UPDATE: InternalUpdate = $internal_update_3d;
            const INTERNAL_FIXED_UPDATE: InternalFixedUpdate = $internal_fixed_update_3d;
            type TransformSnapshot = Transform3D;

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

            #[inline]
            fn snapshot_transform(value: &Self) -> Option<Self::TransformSnapshot> {
                Some(value.transform)
            }
        })*

        $(impl NodeTypeDispatch for $ty_ui {
            const NODE_TYPE: NodeType = NodeType::$variant_ui;
            const SPATIAL: Spatial = Spatial::None;
            const RENDERABLE: Renderable = $renderable_ui;
            const INTERNAL_UPDATE: InternalUpdate = $internal_update_ui;
            const INTERNAL_FIXED_UPDATE: InternalFixedUpdate = $internal_fixed_update_ui;
            type TransformSnapshot = ();

            fn with_ref<R>(data: &SceneNodeData, f: impl FnOnce(&Self) -> R) -> Option<R> {
                match data {
                    SceneNodeData::$variant_ui(inner) => Some(f(inner)),
                    _ => None,
                }
            }

            fn with_mut<R>(
                data: &mut SceneNodeData,
                f: impl FnOnce(&mut Self) -> R,
            ) -> Option<R> {
                match data {
                    SceneNodeData::$variant_ui(inner) => Some(f(inner)),
                    _ => None,
                }
            }
        })*

        $(impl NodeTypeDispatch for $ty_resource {
            const NODE_TYPE: NodeType = NodeType::$variant_resource;
            const SPATIAL: Spatial = Spatial::None;
            const RENDERABLE: Renderable = $renderable_resource;
            const INTERNAL_UPDATE: InternalUpdate = $internal_update_resource;
            const INTERNAL_FIXED_UPDATE: InternalFixedUpdate = $internal_fixed_update_resource;
            type TransformSnapshot = ();

            fn with_ref<R>(data: &SceneNodeData, f: impl FnOnce(&Self) -> R) -> Option<R> {
                match data {
                    SceneNodeData::$variant_resource(inner) => Some(f(inner)),
                    _ => None,
                }
            }

            fn with_mut<R>(
                data: &mut SceneNodeData,
                f: impl FnOnce(&mut Self) -> R,
            ) -> Option<R> {
                match data {
                    SceneNodeData::$variant_resource(inner) => Some(f(inner)),
                    _ => None,
                }
            }
        })*

        $($crate::__impl_exact_node_base_dispatch_2d!($variant_2d, $ty_2d, $variant_2d);)*
        $($crate::__impl_exact_node_base_dispatch_3d!($variant_3d, $ty_3d, $variant_3d);)*
        $($crate::__impl_exact_node_base_dispatch_ui!($variant_ui, $ty_ui, $variant_ui);)*

        impl NodeBaseDispatch for Node2D {
            const BASE_NODE_TYPE: NodeType = NodeType::Node2D;

            fn with_base_ref<R>(data: &SceneNodeData, f: impl FnOnce(&Self) -> R) -> Option<R> {
                match data {
                    $(SceneNodeData::$variant_2d(inner) => {
                        $crate::__node2d_base_expr!($variant_2d, $parent_2d, inner, f)
                    },)*
                    _ => None,
                }
            }

            fn with_base_mut<R>(
                data: &mut SceneNodeData,
                f: impl FnOnce(&mut Self) -> R,
            ) -> Option<R> {
                match data {
                    $(SceneNodeData::$variant_2d(inner) => {
                        $crate::__node2d_base_expr!($variant_2d, $parent_2d, inner, f)
                    },)*
                    _ => None,
                }
            }
        }

        impl NodeBaseDispatch for Node3D {
            const BASE_NODE_TYPE: NodeType = NodeType::Node3D;

            fn with_base_ref<R>(data: &SceneNodeData, f: impl FnOnce(&Self) -> R) -> Option<R> {
                match data {
                    $(SceneNodeData::$variant_3d(inner) => {
                        $crate::__node3d_base_expr!($variant_3d, $parent_3d, inner, f)
                    },)*
                    _ => None,
                }
            }

            fn with_base_mut<R>(
                data: &mut SceneNodeData,
                f: impl FnOnce(&mut Self) -> R,
            ) -> Option<R> {
                match data {
                    $(SceneNodeData::$variant_3d(inner) => {
                        $crate::__node3d_base_expr!($variant_3d, $parent_3d, inner, f)
                    },)*
                    _ => None,
                }
            }
        }

        impl NodeBaseDispatch for UiBox {
            const BASE_NODE_TYPE: NodeType = NodeType::UiBox;

            fn with_base_ref<R>(data: &SceneNodeData, f: impl FnOnce(&Self) -> R) -> Option<R> {
                match data {
                    $(SceneNodeData::$variant_ui(inner) => {
                        $crate::__ui_base_expr!($variant_ui, $parent_ui, inner, f)
                    },)*
                    _ => None,
                }
            }

            fn with_base_mut<R>(
                data: &mut SceneNodeData,
                f: impl FnOnce(&mut Self) -> R,
            ) -> Option<R> {
                match data {
                    $(SceneNodeData::$variant_ui(inner) => {
                        $crate::__ui_base_mut_expr!($variant_ui, $parent_ui, inner, f)
                    },)*
                    _ => None,
                }
            }
        }
    };
}

pub trait NodeBaseDispatch: Sized {
    const BASE_NODE_TYPE: NodeType;

    fn with_base_ref<R>(data: &SceneNodeData, f: impl FnOnce(&Self) -> R) -> Option<R>;
    fn with_base_mut<R>(data: &mut SceneNodeData, f: impl FnOnce(&mut Self) -> R) -> Option<R>;
}

// ======================================================================
//                          DEFINE NODES
// ======================================================================

define_scene_nodes! {
    base: {
        Node,
    }
    2d: {
        Node2D => (None, Node2D, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),
        Camera2D => (Node2D, Camera2D, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        Sprite2D => (Node2D, Sprite2D, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        CollisionShape2D => (Node2D, CollisionShape2D, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),
        StaticBody2D => (Node2D, StaticBody2D, Renderable::False, InternalUpdate::False, InternalFixedUpdate::True),
        Area2D => (Node2D, Area2D, Renderable::False, InternalUpdate::False, InternalFixedUpdate::True),
        RigidBody2D => (Node2D, RigidBody2D, Renderable::False, InternalUpdate::False, InternalFixedUpdate::True),
    }
    3d: {
        Node3D => (None, Node3D, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),
        Camera3D => (Node3D, Camera3D, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        MeshInstance3D => (Node3D, MeshInstance3D, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        MultiMeshInstance3D => (Node3D, MultiMeshInstance3D, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        CollisionShape3D => (Node3D, CollisionShape3D, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),
        StaticBody3D => (Node3D, StaticBody3D, Renderable::False, InternalUpdate::False, InternalFixedUpdate::True),
        Area3D => (Node3D, Area3D, Renderable::False, InternalUpdate::False, InternalFixedUpdate::True),
        RigidBody3D => (Node3D, RigidBody3D, Renderable::False, InternalUpdate::False, InternalFixedUpdate::True),
        Skeleton3D => (Node3D, Skeleton3D, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),
        ParticleEmitter3D => (Node3D, ParticleEmitter3D, Renderable::True, InternalUpdate::True, InternalFixedUpdate::False),
        //Lights
        AmbientLight3D => (None, AmbientLight3D, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        Sky3D => (None, Sky3D, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        RayLight3D => (Node3D, RayLight3D, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        PointLight3D => (Node3D, PointLight3D, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        SpotLight3D => (Node3D, SpotLight3D, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False)
    }
    ui: {
        UiBox => (None, UiBox, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),
        UiPanel => (UiBox, UiPanel, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        UiButton => (UiBox, UiButton, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        UiLabel => (UiBox, UiLabel, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        UiTextBox => (UiBox, UiTextBox, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        UiTextBlock => (UiBox, UiTextBlock, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        UiLayout => (UiBox, UiLayout, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),
        UiHLayout => (UiBox, UiHLayout, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),
        UiVLayout => (UiBox, UiVLayout, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),
        UiGrid => (UiBox, UiGrid, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False)
    }
    resource: {
        AnimationPlayer => (None, AnimationPlayer, Renderable::False, InternalUpdate::True, InternalFixedUpdate::False)
    }
}
