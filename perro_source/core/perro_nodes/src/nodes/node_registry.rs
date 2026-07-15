use crate::{
    AmbientLight2D, AmbientLight3D, AnimatedSprite2D, AnimationPlayer, AnimationTree, Area2D,
    Area3D, AudioEffectZone2D, AudioEffectZone3D, AudioMask2D, AudioMask3D, AudioPortal2D,
    AudioPortal3D, BallJoint3D, BoneAttachment2D, BoneAttachment3D, BoneCollider2D, BoneCollider3D,
    Button2D, Camera2D, Camera3D, CameraStream2D, CameraStream3D, CharacterBody2D, CharacterBody3D,
    CollisionShape2D, CollisionShape3D, Decal3D, DistanceJoint2D, FixedJoint2D, FixedJoint3D,
    HingeJoint3D, IKTarget2D, IKTarget3D, ImageButton2D, Label2D, Label3D, MeshInstance3D,
    MultiMeshInstance3D, NineSlice2D, NineSliceButton2D, Node2D, Node3D, ParticleEmitter2D,
    ParticleEmitter3D, PhysicsBoneChain2D, PhysicsBoneChain3D, PhysicsForceEmitter2D,
    PhysicsForceEmitter3D, PinJoint2D, PointLight2D, PointLight3D, RayLight2D, RayLight3D,
    RigidBody2D, RigidBody3D, Skeleton2D, Skeleton3D, Sky3D, SpotLight2D, SpotLight3D, Sprite2D,
    Sprite3D, StaticBody2D, StaticBody3D, TextDecal3D, TileMap2D, UiCameraStream, UiVideoPlayer,
    UiViewport, VideoPlayer2D, VideoPlayer3D, WaterBody2D, WaterBody3D, Webcam,
};
use perro_ids::{NodeID, NodeTag, TagID};
use perro_structs::{Transform2D, Transform3D};
use perro_ui::{
    UiAnimatedImage, UiButton, UiCheckbox, UiColorPicker, UiDropdown, UiGrid, UiHLayout, UiImage,
    UiImageButton, UiLabel, UiLayout, UiNineSlice, UiNineSliceButton, UiNode, UiNodeBase, UiPanel,
    UiProgressBar, UiScrollContainer, UiShape, UiTextBlock, UiTextBox, UiTreeList, UiVLayout,
};
use std::borrow::Cow;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
/// Spatial family used by runtime transform + query paths.
pub enum Spatial {
    None,
    TwoD,
    ThreeD,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
/// Render eligibility flag generated for each node type.
pub enum Renderable {
    False,
    True,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
/// Per-frame internal update flag.
pub enum InternalUpdate {
    False,
    True,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
/// Fixed-step internal update flag.
pub enum InternalFixedUpdate {
    False,
    True,
}

#[macro_export]
/// Storage marker → concrete enum payload type. `Boxed` variants keep the
/// `SceneNodeData` stride small; access still auto-derefs, only construction
/// sites and by-value destructures see the `Box`.
macro_rules! __node_storage_ty {
    (Inline, $ty:ty) => { $ty };
    (Boxed, $ty:ty) => { ::std::boxed::Box<$ty> };
}

#[macro_export]
/// Storage marker → payload constructor expression (boxes when marked).
macro_rules! __node_wrap {
    (Inline, $value:expr) => {
        $value
    };
    (Boxed, $value:expr) => {
        ::std::boxed::Box::new($value)
    };
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
    (UiNode, None, $inner:ident, $f:ident) => {
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
    (UiNode, None, $inner:ident, $f:ident) => {
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
    (UiNode, $ty_ui:ty, $variant_ui:ident) => {};
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
/// Build node enum, node type metadata, and typed dispatch impls.
macro_rules! define_scene_nodes {
    (
        base: { $($base_variant:ident $(=> $base_ty:ty)?),* $(,)? }
        2d: { $($variant_2d:ident => ($parent_2d:ident, $ty_2d:ty, $storage_2d:ident, $renderable_2d:expr, $internal_update_2d:expr, $internal_fixed_update_2d:expr)),* $(,)? }
        3d: { $($variant_3d:ident => ($parent_3d:ident, $ty_3d:ty, $storage_3d:ident, $renderable_3d:expr, $internal_update_3d:expr, $internal_fixed_update_3d:expr)),* $(,)? }
        ui: { $($variant_ui:ident => ($parent_ui:ident, $ty_ui:ty, $storage_ui:ident, $renderable_ui:expr, $internal_update_ui:expr, $internal_fixed_update_ui:expr)),* $(,)? }
        resource: { $($variant_resource:ident => ($parent_resource:ident, $ty_resource:ty, $storage_resource:ident, $renderable_resource:expr, $internal_update_resource:expr, $internal_fixed_update_resource:expr)),* $(,)? }
    ) => {
        #[derive(Clone, Debug)]
        pub struct SceneNode {
            pub data: SceneNodeData,
            pub id: NodeID,
            pub name: Cow<'static, str>,
            pub parent: NodeID,
            pub children: Vec<NodeID>,
            pub tags: Vec<NodeTag>,
        }

        #[derive(Clone, Debug)]
        #[allow(clippy::large_enum_variant)]
        pub enum SceneNodeData {
            $(
                $base_variant$(($base_ty))?,
            )*
            $($variant_2d($crate::__node_storage_ty!($storage_2d, $ty_2d)),)*
            $($variant_3d($crate::__node_storage_ty!($storage_3d, $ty_3d)),)*
            $($variant_ui($crate::__node_storage_ty!($storage_ui, $ty_ui)),)*
            $($variant_resource($crate::__node_storage_ty!($storage_resource, $ty_resource)),)*
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
            pub fn new(data: SceneNodeData) -> Self {
                Self {
                    id: NodeID::nil(),
                    name: Cow::Borrowed("Node"),
                    parent: NodeID::nil(),
                    children: Vec::new(),
                    tags: Vec::new(),
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
                &self.children
            }

            pub fn set_children_ids<C>(&mut self, children: Option<C>)
            where
                C: Into<Vec<NodeID>>,
            {
                self.children = children.map(Into::into).unwrap_or_default();
            }

            pub fn get_tags(&self) -> &[NodeTag] {
                &self.tags
            }

            pub fn get_tag_ids(&self) -> Vec<TagID> {
                self.tags.iter().map(NodeTag::id).collect()
            }

            pub fn set_tag_ids<T>(&mut self, tags: Option<T>)
            where
                T: Into<Vec<TagID>>,
            {
                self.tags = tags
                    .map(Into::into)
                    .unwrap_or_default()
                    .into_iter()
                    .map(NodeTag::from)
                    .collect();
            }

            pub fn set_tags<T>(&mut self, tags: Option<T>)
            where
                T: Into<Vec<NodeTag>>,
            {
                self.tags = tags.map(Into::into).unwrap_or_default();
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
                self.children.push(child);
            }

            pub fn remove_child(&mut self, child: NodeID) {
                self.children.retain(|&c| c != child);
            }

            pub fn clear_children(&mut self) {
                self.children.clear();
            }

            pub fn children_slice(&self) -> &[NodeID] {
                self.get_children_ids()
            }

            pub fn add_tag<T>(&mut self, tag: T)
            where
                T: Into<NodeTag>,
            {
                self.tags.push(tag.into());
            }

            pub fn remove_tag(&mut self, tag: TagID) {
                self.tags.retain(|t| t.id != tag);
            }

            pub fn clear_tags(&mut self) {
                self.tags.clear();
            }

            pub fn has_tag(&self, tag: TagID) -> bool {
                self.tags.iter().any(|node_tag| node_tag.id == tag)
            }

            pub fn tags_slice(&self) -> &[NodeTag] {
                self.get_tags()
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

            pub const fn name(&self) -> &'static str {
                self.as_str()
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

            pub const fn is_a(&self, base: NodeType) -> bool {
                if *self as u8 == base as u8 {
                    return true;
                }

                let mut cursor = *self;
                loop {
                    match cursor.parent_type() {
                        Some(parent) => {
                            if parent as u8 == base as u8 {
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

            pub const fn get_renderable(&self) -> Renderable {
                match self {
                    $(NodeType::$base_variant => Renderable::False,)*
                    $(NodeType::$variant_2d => $renderable_2d,)*
                    $(NodeType::$variant_3d => $renderable_3d,)*
                    $(NodeType::$variant_ui => $renderable_ui,)*
                    $(NodeType::$variant_resource => $renderable_resource,)*
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
                SceneNodeData::$variant_2d($crate::__node_wrap!($storage_2d, value))
            }
        })*

        $(impl From<$ty_3d> for SceneNodeData {
            fn from(value: $ty_3d) -> Self {
                SceneNodeData::$variant_3d($crate::__node_wrap!($storage_3d, value))
            }
        })*

        $(impl From<$ty_ui> for SceneNodeData {
            fn from(value: $ty_ui) -> Self {
                SceneNodeData::$variant_ui($crate::__node_wrap!($storage_ui, value))
            }
        })*

        $(impl From<$ty_resource> for SceneNodeData {
            fn from(value: $ty_resource) -> Self {
                SceneNodeData::$variant_resource($crate::__node_wrap!($storage_resource, value))
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

        impl NodeBaseDispatch for UiNode {
            const BASE_NODE_TYPE: NodeType = NodeType::UiNode;

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
// Node registry
//
// Order rule:
// - core base first
// - runtime families next: camera, visual, lights, skeletal, physics
// - parents before children
// - 2D and 3D mirror family order
// ======================================================================

define_scene_nodes! {
    base: {
        Node,
    }
    2d: {
        // core
        Node2D => (None, Node2D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),

        // camera
        Camera2D => (Node2D, Camera2D, Inline, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),

        // visual
        CameraStream2D => (Node2D, CameraStream2D, Inline, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        Button2D => (Node2D, Button2D, Boxed, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        ImageButton2D => (Node2D, ImageButton2D, Boxed, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        NineSliceButton2D => (Node2D, NineSliceButton2D, Boxed, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        Sprite2D => (Node2D, Sprite2D, Inline, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        VideoPlayer2D => (Node2D, VideoPlayer2D, Inline, Renderable::True, InternalUpdate::True, InternalFixedUpdate::False),
        Label2D => (Node2D, Label2D, Inline, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        NineSlice2D => (Node2D, NineSlice2D, Inline, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        AnimatedSprite2D => (Node2D, AnimatedSprite2D, Inline, Renderable::True, InternalUpdate::True, InternalFixedUpdate::False),
        TileMap2D => (Node2D, TileMap2D, Inline, Renderable::True, InternalUpdate::False, InternalFixedUpdate::True),
        ParticleEmitter2D => (Node2D, ParticleEmitter2D, Inline, Renderable::True, InternalUpdate::True, InternalFixedUpdate::False),
        WaterBody2D => (Node2D, WaterBody2D, Boxed, Renderable::True, InternalUpdate::False, InternalFixedUpdate::True),

        // lights
        AmbientLight2D => (None, AmbientLight2D, Inline, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        RayLight2D => (Node2D, RayLight2D, Inline, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        PointLight2D => (Node2D, PointLight2D, Inline, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        SpotLight2D => (Node2D, SpotLight2D, Inline, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),

        // skeletal
        Skeleton2D => (Node2D, Skeleton2D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),
        BoneAttachment2D => (Node2D, BoneAttachment2D, Inline, Renderable::False, InternalUpdate::True, InternalFixedUpdate::False),
        IKTarget2D => (Node2D, IKTarget2D, Inline, Renderable::False, InternalUpdate::True, InternalFixedUpdate::False),
        PhysicsBoneChain2D => (Node2D, PhysicsBoneChain2D, Boxed, Renderable::False, InternalUpdate::False, InternalFixedUpdate::True),
        BoneCollider2D => (Node2D, BoneCollider2D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),

        // physics
        CollisionShape2D => (Node2D, CollisionShape2D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),
        StaticBody2D => (Node2D, StaticBody2D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::True),
        Area2D => (Node2D, Area2D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::True),
        RigidBody2D => (Node2D, RigidBody2D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::True),
        CharacterBody2D => (Node2D, CharacterBody2D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::True),
        PhysicsForceEmitter2D => (Node2D, PhysicsForceEmitter2D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::True),
        PinJoint2D => (Node2D, PinJoint2D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::True),
        DistanceJoint2D => (Node2D, DistanceJoint2D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::True),
        FixedJoint2D => (Node2D, FixedJoint2D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::True),

        // audio
        AudioMask2D => (Node2D, AudioMask2D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),
        AudioEffectZone2D => (Node2D, AudioEffectZone2D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),
        AudioPortal2D => (Node2D, AudioPortal2D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),
    }
    3d: {
        // core
        Node3D => (None, Node3D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),

        // camera
        Camera3D => (Node3D, Camera3D, Inline, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),

        // visual
        CameraStream3D => (Node3D, CameraStream3D, Inline, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        MeshInstance3D => (Node3D, MeshInstance3D, Inline, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        MultiMeshInstance3D => (Node3D, MultiMeshInstance3D, Inline, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        Sprite3D => (Node3D, Sprite3D, Inline, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        VideoPlayer3D => (Node3D, VideoPlayer3D, Inline, Renderable::True, InternalUpdate::True, InternalFixedUpdate::False),
        Label3D => (Node3D, Label3D, Inline, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        ParticleEmitter3D => (Node3D, ParticleEmitter3D, Inline, Renderable::True, InternalUpdate::True, InternalFixedUpdate::False),
        WaterBody3D => (Node3D, WaterBody3D, Boxed, Renderable::True, InternalUpdate::False, InternalFixedUpdate::True),
        Decal3D => (Node3D, Decal3D, Inline, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        TextDecal3D => (Node3D, TextDecal3D, Boxed, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        Sky3D => (None, Sky3D, Inline, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),

        // lights
        AmbientLight3D => (None, AmbientLight3D, Inline, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        RayLight3D => (Node3D, RayLight3D, Inline, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        PointLight3D => (Node3D, PointLight3D, Inline, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        SpotLight3D => (Node3D, SpotLight3D, Inline, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),

        // skeletal
        Skeleton3D => (Node3D, Skeleton3D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),
        BoneAttachment3D => (Node3D, BoneAttachment3D, Inline, Renderable::False, InternalUpdate::True, InternalFixedUpdate::False),
        IKTarget3D => (Node3D, IKTarget3D, Inline, Renderable::False, InternalUpdate::True, InternalFixedUpdate::False),
        PhysicsBoneChain3D => (Node3D, PhysicsBoneChain3D, Boxed, Renderable::False, InternalUpdate::False, InternalFixedUpdate::True),
        BoneCollider3D => (Node3D, BoneCollider3D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),

        // physics
        CollisionShape3D => (Node3D, CollisionShape3D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),
        StaticBody3D => (Node3D, StaticBody3D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::True),
        Area3D => (Node3D, Area3D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::True),
        RigidBody3D => (Node3D, RigidBody3D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::True),
        CharacterBody3D => (Node3D, CharacterBody3D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::True),
        PhysicsForceEmitter3D => (Node3D, PhysicsForceEmitter3D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::True),
        BallJoint3D => (Node3D, BallJoint3D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::True),
        HingeJoint3D => (Node3D, HingeJoint3D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::True),
        FixedJoint3D => (Node3D, FixedJoint3D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::True),

        // audio
        AudioMask3D => (Node3D, AudioMask3D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),
        AudioEffectZone3D => (Node3D, AudioEffectZone3D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),
        AudioPortal3D => (Node3D, AudioPortal3D, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),
    }
    ui: {
        // core
        UiNode => (None, UiNode, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),

        // visual
        UiCameraStream => (UiNode, UiCameraStream, Boxed, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        UiViewport => (UiNode, UiViewport, Boxed, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        UiPanel => (UiNode, UiPanel, Boxed, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        UiProgressBar => (UiNode, UiProgressBar, Boxed, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        UiButton => (UiNode, UiButton, Boxed, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        UiDropdown => (UiNode, UiDropdown, Boxed, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        UiColorPicker => (UiNode, UiColorPicker, Boxed, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        UiShape => (UiNode, UiShape, Inline, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        UiCheckbox => (UiNode, UiCheckbox, Boxed, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        UiImage => (UiNode, UiImage, Boxed, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        UiVideoPlayer => (UiNode, UiVideoPlayer, Boxed, Renderable::True, InternalUpdate::True, InternalFixedUpdate::False),
        UiImageButton => (UiNode, UiImageButton, Boxed, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        UiNineSliceButton => (UiNode, UiNineSliceButton, Boxed, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        UiNineSlice => (UiNode, UiNineSlice, Boxed, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        UiAnimatedImage => (UiNode, UiAnimatedImage, Boxed, Renderable::True, InternalUpdate::True, InternalFixedUpdate::False),
        UiLabel => (UiNode, UiLabel, Boxed, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        UiTextBox => (UiNode, UiTextBox, Boxed, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),
        UiTextBlock => (UiNode, UiTextBlock, Boxed, Renderable::True, InternalUpdate::False, InternalFixedUpdate::False),

        // layout
        UiScrollContainer => (UiNode, UiScrollContainer, Boxed, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),
        UiLayout => (UiNode, UiLayout, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),
        UiHLayout => (UiNode, UiHLayout, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),
        UiVLayout => (UiNode, UiVLayout, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),
        UiGrid => (UiNode, UiGrid, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),
        UiTreeList => (UiNode, UiTreeList, Boxed, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False)
    }
    resource: {
        // capture
        Webcam => (None, Webcam, Inline, Renderable::False, InternalUpdate::False, InternalFixedUpdate::False),

        // animation
        AnimationPlayer => (None, AnimationPlayer, Inline, Renderable::False, InternalUpdate::True, InternalFixedUpdate::False),
        AnimationTree => (None, AnimationTree, Inline, Renderable::False, InternalUpdate::True, InternalFixedUpdate::False)
    }
}

impl NodeType {
    /// True for node types whose data the physics world mirrors: bodies,
    /// colliders, joints, force emitters, water, and physics bone rigs.
    /// Mutating one of these must invalidate the physics sync gate.
    pub const fn is_physics(self) -> bool {
        matches!(
            self,
            NodeType::CollisionShape2D
                | NodeType::StaticBody2D
                | NodeType::Area2D
                | NodeType::RigidBody2D
                | NodeType::CharacterBody2D
                | NodeType::PhysicsForceEmitter2D
                | NodeType::PinJoint2D
                | NodeType::DistanceJoint2D
                | NodeType::FixedJoint2D
                | NodeType::WaterBody2D
                | NodeType::PhysicsBoneChain2D
                | NodeType::BoneCollider2D
                | NodeType::CollisionShape3D
                | NodeType::StaticBody3D
                | NodeType::Area3D
                | NodeType::RigidBody3D
                | NodeType::CharacterBody3D
                | NodeType::PhysicsForceEmitter3D
                | NodeType::BallJoint3D
                | NodeType::HingeJoint3D
                | NodeType::FixedJoint3D
                | NodeType::WaterBody3D
                | NodeType::PhysicsBoneChain3D
                | NodeType::BoneCollider3D
        )
    }
}
