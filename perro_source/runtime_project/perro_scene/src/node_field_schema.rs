use std::borrow::Cow;

use perro_nodes::NodeType;

use crate::{SceneFieldName, SceneObjectField, SceneValue, default_scene_field_value_by_name};

const CAMERA_REF_TYPES: &[NodeType] = &[NodeType::Camera2D, NodeType::Camera3D, NodeType::Webcam];
const SKELETON_2D_REF_TYPES: &[NodeType] = &[NodeType::Skeleton2D];
const SKELETON_3D_REF_TYPES: &[NodeType] = &[NodeType::Skeleton3D];
const BODY_2D_REF_TYPES: &[NodeType] = &[
    NodeType::StaticBody2D,
    NodeType::RigidBody2D,
    NodeType::CharacterBody2D,
    NodeType::Area2D,
];
const BODY_3D_REF_TYPES: &[NodeType] = &[
    NodeType::StaticBody3D,
    NodeType::RigidBody3D,
    NodeType::CharacterBody3D,
    NodeType::Area3D,
];
const CAMERA_STREAM_ASPECT_MODE_OPTIONS: &[&str] = &["fit", "stretch", "cover"];
const IK_SOLVER_OPTIONS: &[&str] = &["ccd", "fabrik"];
const ANIMATION_PLAYBACK_OPTIONS: &[&str] = &["once", "loop", "boomerang"];
const UI_ANCHOR_OPTIONS: &[&str] = &[
    "center",
    "left",
    "right",
    "top",
    "bottom",
    "top_left",
    "top_right",
    "bottom_left",
    "bottom_right",
];
const UI_TEXT_ALIGN_OPTIONS: &[&str] = &["start", "center", "end"];
const UI_FILL_KIND_OPTIONS: &[&str] = &["solid", "linear"];
const UI_COLOR_PICKER_MODE_OPTIONS: &[&str] = &["smooth_wheel", "block_wheel", "swatches"];
const UI_SCROLL_DIRECTION_OPTIONS: &[&str] = &["vertical", "horizontal"];
const UI_SCROLL_BAR_SIDE_OPTIONS: &[&str] = &["right", "left", "bottom", "top"];
const PARTICLE_SIM_MODE_2D_OPTIONS: &[&str] = &["default", "cpu"];
const PARTICLE_SIM_MODE_3D_OPTIONS: &[&str] = &["default", "cpu", "hybrid", "gpu"];
const PARTICLE_RENDER_MODE_3D_OPTIONS: &[&str] = &["point", "billboard"];
const WATER_IDLE_MODE_OPTIONS: &[&str] = &["calm", "sine", "chop", "storm", "river"];
const CAMERA_PROJECTION_SUBMENUS: &[NodeFieldEnumVariant] = &[
    NodeFieldEnumVariant::new(
        "perspective",
        &[
            "perspective_fov_y_degrees",
            "perspective_near",
            "perspective_far",
        ],
    ),
    NodeFieldEnumVariant::new(
        "orthographic",
        &["orthographic_size", "orthographic_near", "orthographic_far"],
    ),
    NodeFieldEnumVariant::new(
        "frustum",
        &[
            "frustum_left",
            "frustum_right",
            "frustum_bottom",
            "frustum_top",
            "frustum_near",
            "frustum_far",
        ],
    ),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeRefHint {
    pub allowed: &'static [NodeType],
}

impl NodeRefHint {
    pub const fn any() -> Self {
        Self { allowed: &[] }
    }

    pub const fn many(allowed: &'static [NodeType]) -> Self {
        Self { allowed }
    }

    pub fn allows(&self, node_type: NodeType) -> bool {
        self.allowed.is_empty() || self.allowed.iter().any(|allowed| node_type.is_a(*allowed))
    }

    pub fn label(&self) -> String {
        if self.allowed.is_empty() {
            return "Node".to_string();
        }
        format!(
            "Node({})",
            self.allowed
                .iter()
                .map(|node_type| node_type.name())
                .collect::<Vec<_>>()
                .join("|")
        )
    }
}

#[derive(Clone, Debug)]
pub enum NodeFieldType {
    Bool,
    I32,
    U32,
    F32,
    Vec2,
    Vec3,
    Vec4,
    IVec2,
    IVec3,
    IVec4,
    UVec2,
    UVec3,
    UVec4,
    Quat,
    Color,
    String,
    Enum(&'static [&'static str]),
    EnumSubmenu(&'static [NodeFieldEnumVariant]),
    NodeRef(NodeRefHint),
    BitMask,
    Object(Vec<NodeFieldDef>),
    Array(Box<NodeFieldType>),
    Matrix {
        rows: usize,
        cols: usize,
        item: Box<NodeFieldType>,
    },
    Asset(SceneAssetKind),
    Unknown,
}

impl NodeFieldType {
    pub fn object(fields: Vec<NodeFieldDef>) -> Self {
        Self::Object(fields)
    }

    pub const fn enumeration(options: &'static [&'static str]) -> Self {
        Self::Enum(options)
    }

    pub const fn enum_submenu(variants: &'static [NodeFieldEnumVariant]) -> Self {
        Self::EnumSubmenu(variants)
    }

    pub fn array(item: NodeFieldType) -> Self {
        Self::Array(Box::new(item))
    }

    pub fn matrix(rows: usize, cols: usize, item: NodeFieldType) -> Self {
        Self::Matrix {
            rows,
            cols,
            item: Box::new(item),
        }
    }

    pub fn default_value(&self) -> SceneValue {
        match self {
            Self::Bool => SceneValue::Bool(false),
            Self::I32 | Self::U32 | Self::BitMask => SceneValue::I32(0),
            Self::F32 => SceneValue::F32(0.0),
            Self::Vec2 => SceneValue::Vec2 { x: 0.0, y: 0.0 },
            Self::Vec3 => SceneValue::Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            Self::IVec2 => SceneValue::IVec2 { x: 0, y: 0 },
            Self::IVec3 => SceneValue::IVec3 { x: 0, y: 0, z: 0 },
            Self::IVec4 => SceneValue::IVec4 {
                x: 0,
                y: 0,
                z: 0,
                w: 0,
            },
            Self::UVec2 => SceneValue::UVec2 { x: 0, y: 0 },
            Self::UVec3 => SceneValue::UVec3 { x: 0, y: 0, z: 0 },
            Self::UVec4 => SceneValue::UVec4 {
                x: 0,
                y: 0,
                z: 0,
                w: 0,
            },
            Self::Vec4 | Self::Quat | Self::Color => SceneValue::Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            },
            Self::String => SceneValue::Str(Cow::Borrowed("")),
            Self::Enum(options) => SceneValue::Key(options.first().copied().unwrap_or("").into()),
            Self::EnumSubmenu(variants) => {
                SceneValue::Key(variants.first().map(|v| v.name).unwrap_or("").into())
            }
            Self::NodeRef(_) => SceneValue::Key(crate::SceneValueKey::from("null")),
            Self::Asset(_) => SceneValue::Str(Cow::Borrowed("")),
            Self::Array(_) => SceneValue::Array(Cow::Owned(Vec::new())),
            Self::Matrix { rows, cols, item } => {
                let value = item.default_value();
                let row = SceneValue::Array(Cow::Owned(vec![value; *cols]));
                SceneValue::Array(Cow::Owned(vec![row; *rows]))
            }
            Self::Object(fields) => SceneValue::Object(Cow::Owned(
                fields
                    .iter()
                    .map(|field| {
                        (
                            SceneFieldName::from_name(field.name.to_string()),
                            field
                                .default
                                .clone()
                                .unwrap_or_else(|| field.ty.default_value()),
                        )
                    })
                    .collect(),
            )),
            Self::Unknown => SceneValue::Object(Cow::Owned(Vec::new())),
        }
    }
}

#[derive(Clone, Debug)]
pub struct NodeFieldDef {
    pub name: &'static str,
    pub ty: NodeFieldType,
    pub default: Option<SceneValue>,
}

impl NodeFieldDef {
    pub fn new(name: &'static str, ty: NodeFieldType) -> Self {
        Self {
            name,
            ty,
            default: None,
        }
    }

    pub fn with_default(mut self, default: SceneValue) -> Self {
        self.default = Some(default);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeFieldEnumVariant {
    pub name: &'static str,
    pub fields: &'static [&'static str],
}

impl NodeFieldEnumVariant {
    pub const fn new(name: &'static str, fields: &'static [&'static str]) -> Self {
        Self { name, fields }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneAssetKind {
    Scene,
    Script,
    Texture,
    Mesh,
    Model,
    Material,
    Animation,
    AnimationTree,
    Skeleton,
    ParticleProfile,
    TileSet,
    UiStyle,
}

#[derive(Clone, Debug)]
pub struct SceneNodeField {
    pub section: &'static str,
    pub name: &'static str,
    pub ty: NodeFieldType,
    pub default: Option<SceneValue>,
    pub aliases: &'static [&'static str],
}

impl SceneNodeField {
    pub fn new(section: &'static str, name: &'static str, ty: NodeFieldType) -> Self {
        Self {
            section,
            name,
            ty,
            default: None,
            aliases: &[],
        }
    }

    pub fn with_default(mut self, default: SceneValue) -> Self {
        self.default = Some(default);
        self
    }

    pub fn with_aliases(mut self, aliases: &'static [&'static str]) -> Self {
        self.aliases = aliases;
        self
    }
}

pub fn scene_node_fields(node_type: NodeType) -> Vec<SceneNodeField> {
    cached_scene_node_fields(node_type).to_vec()
}

/// Schema per type is immutable after build; cache it so repeated lookups
/// (doctor validates every field of every node) don't rebuild the Vec, and so
/// the `Box::leak`ed style-prefix names leak once per type instead of per call.
fn cached_scene_node_fields(node_type: NodeType) -> &'static [SceneNodeField] {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<NodeType, &'static [SceneNodeField]>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let mut map = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(slice) = map.get(&node_type) {
        return slice;
    }
    let built: &'static [SceneNodeField] = Box::leak(build_scene_node_fields(node_type).into());
    map.insert(node_type, built);
    built
}

mod build;
pub use build::*;
mod helpers;
use helpers::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_ref_hints_cover_camera_streams_and_skeletons() {
        let stream = scene_node_field(NodeType::UiCameraStream, "camera").unwrap();
        let NodeFieldType::NodeRef(stream_hint) = stream.ty else {
            panic!("camera must be node ref");
        };
        assert!(stream_hint.allows(NodeType::Camera2D));
        assert!(stream_hint.allows(NodeType::Camera3D));
        assert!(stream_hint.allows(NodeType::Webcam));
        assert!(!stream_hint.allows(NodeType::MeshInstance3D));

        let mesh = scene_node_field(NodeType::MeshInstance3D, "skeleton").unwrap();
        let NodeFieldType::NodeRef(mesh_hint) = mesh.ty else {
            panic!("skeleton must be node ref");
        };
        assert!(mesh_hint.allows(NodeType::Skeleton3D));
        assert!(!mesh_hint.allows(NodeType::Skeleton2D));
    }

    #[test]
    fn camera_projection_field_exposes_enum_options() {
        let field = scene_node_field(NodeType::Camera3D, "projection").unwrap();
        let NodeFieldType::EnumSubmenu(options) = field.ty else {
            panic!("projection must be enum submenu");
        };
        assert_eq!(
            options.iter().map(|v| v.name).collect::<Vec<_>>(),
            ["perspective", "orthographic", "frustum"],
        );
        assert_eq!(
            options[0].fields,
            [
                "perspective_fov_y_degrees",
                "perspective_near",
                "perspective_far"
            ],
        );
    }

    #[test]
    fn sub_view_schemas_own_mixed_views_without_camera_ref() {
        for node_type in [
            NodeType::UiSubView,
            NodeType::SubView2D,
            NodeType::SubView3D,
        ] {
            let fields = scene_node_fields(node_type);
            assert!(fields.iter().any(|field| field.name == "view_position"));
            assert!(fields.iter().any(|field| field.name == "view_rotation"));
            assert!(fields.iter().any(|field| field.name == "view_2d_position"));
            assert!(fields.iter().any(|field| field.name == "projection"));
            assert!(fields.iter().any(|field| field.name == "resolution"));
            assert!(!fields.iter().any(|field| field.name == "camera"));
        }
    }

    #[test]
    fn ui_button_schema_exposes_decor_fields() {
        let fields = scene_node_fields(NodeType::UiButton);
        assert!(fields.iter().any(|field| field.name == "fill_kind"));
        assert!(fields.iter().any(|field| field.name == "gradient"));
        assert!(fields.iter().any(|field| field.name == "corner_radii"));
        assert!(fields.iter().any(|field| field.name == "outer_shadow"));
        assert!(fields.iter().any(|field| field.name == "inner_highlight"));
        assert!(fields.iter().any(|field| field.name == "hover_fill_kind"));
        assert!(
            fields
                .iter()
                .any(|field| field.name == "pressed_outer_shadow")
        );
    }

    #[test]
    fn world_label_and_sprite_schemas_expose_runtime_fields() {
        let label_2d = scene_node_fields(NodeType::Label2D);
        let label_3d = scene_node_fields(NodeType::Label3D);
        for fields in [&label_2d, &label_3d] {
            assert!(fields.iter().any(|field| field.name == "text"));
            assert!(fields.iter().any(|field| field.name == "size"));
            assert!(fields.iter().any(|field| field.name == "font_size"));
            assert!(fields.iter().any(|field| field.name == "h_align"));
            assert!(fields.iter().any(|field| field.name == "v_align"));
        }

        let sprite_3d = scene_node_fields(NodeType::Sprite3D);
        assert!(sprite_3d.iter().any(|field| field.name == "texture"));
        assert!(sprite_3d.iter().any(|field| field.name == "texture_region"));
        assert!(sprite_3d.iter().any(|field| field.name == "size"));
        assert!(sprite_3d.iter().any(|field| field.name == "modulate"));
    }

    #[test]
    fn video_player_schemas_expose_runtime_fields() {
        for ty in [NodeType::VideoPlayer2D, NodeType::VideoPlayer3D] {
            let fields = scene_node_fields(ty);
            assert!(fields.iter().any(|field| field.name == "source"));
            assert!(fields.iter().any(|field| field.name == "playing"));
            assert!(fields.iter().any(|field| field.name == "looping"));
            assert!(fields.iter().any(|field| field.name == "fps_scale"));
            assert!(fields.iter().any(|field| field.name == "size"));
            assert!(fields.iter().any(|field| field.name == "tint"));
        }

        let ui = scene_node_fields(NodeType::UiVideoPlayer);
        assert!(ui.iter().any(|field| field.name == "source"));
        assert!(ui.iter().any(|field| field.name == "playing"));
        assert!(ui.iter().any(|field| field.name == "scale_mode"));
        assert!(ui.iter().any(|field| field.name == "corner_radius"));
    }

    #[test]
    fn audio_node_schemas_use_active_state() {
        for ty in [
            NodeType::AudioMask2D,
            NodeType::AudioMask3D,
            NodeType::AudioEffectZone2D,
            NodeType::AudioEffectZone3D,
            NodeType::AudioPortal2D,
            NodeType::AudioPortal3D,
        ] {
            let fields = scene_node_fields(ty);
            assert!(fields.iter().any(|field| field.name == "active"));
            assert!(!fields.iter().any(|field| field.name == "enabled"));
        }
    }
}
