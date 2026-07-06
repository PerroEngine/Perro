use std::borrow::Cow;

use perro_nodes::NodeType;

use crate::{SceneFieldName, SceneObjectField, SceneValue, default_scene_field_value_by_name};

const CAMERA_REF_TYPES: &[NodeType] = &[NodeType::Camera2D, NodeType::Camera3D];
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
    let mut fields = Vec::new();
    push_node_fields(&mut fields, node_type);
    push_base_fields(&mut fields, node_type);
    let mut seen = Vec::new();
    fields.retain(|field| {
        if seen.contains(&field.name) {
            false
        } else {
            seen.push(field.name);
            true
        }
    });
    fields
}

pub fn scene_default_fields(node_type: NodeType) -> Vec<SceneObjectField> {
    scene_node_fields(node_type)
        .into_iter()
        .filter_map(|field| {
            field
                .default
                .map(|value| (SceneFieldName::from_name(field.name.to_string()), value))
        })
        .collect()
}

pub fn scene_node_asset_fields(node_type: NodeType) -> Vec<SceneNodeField> {
    scene_node_fields(node_type)
        .into_iter()
        .filter(|field| matches!(field.ty, NodeFieldType::Asset(_)))
        .collect()
}

pub fn scene_node_field(node_type: NodeType, name: &str) -> Option<SceneNodeField> {
    scene_node_fields(node_type)
        .into_iter()
        .find(|field| field.name == name || field.aliases.contains(&name))
}

fn push_base_fields(fields: &mut Vec<SceneNodeField>, node_type: NodeType) {
    if node_type.is_a(NodeType::Node2D) {
        push_default(
            fields,
            node_type,
            "Transform",
            "position",
            NodeFieldType::Vec2,
        );
        push_default(
            fields,
            node_type,
            "Transform",
            "rotation",
            NodeFieldType::F32,
        );
        push_default(fields, node_type, "Transform", "scale", NodeFieldType::Vec2);
        push_default(
            fields,
            node_type,
            "Visibility",
            "visible",
            NodeFieldType::Bool,
        );
        push_default(
            fields,
            node_type,
            "Visibility",
            "modulate",
            NodeFieldType::Color,
        );
        push_default(
            fields,
            node_type,
            "Visibility",
            "render_layers",
            NodeFieldType::BitMask,
        );
    } else if node_type.is_a(NodeType::Node3D) {
        push_default(
            fields,
            node_type,
            "Transform",
            "position",
            NodeFieldType::Vec3,
        );
        push_default(
            fields,
            node_type,
            "Transform",
            "rotation",
            NodeFieldType::Quat,
        );
        push_default(fields, node_type, "Transform", "scale", NodeFieldType::Vec3);
        push_default(
            fields,
            node_type,
            "Visibility",
            "visible",
            NodeFieldType::Bool,
        );
        push_default(
            fields,
            node_type,
            "Visibility",
            "modulate",
            NodeFieldType::Color,
        );
        push_default(
            fields,
            node_type,
            "Visibility",
            "render_layers",
            NodeFieldType::BitMask,
        );
    } else if node_type.is_a(NodeType::UiNode) {
        fields.push(
            SceneNodeField::new(
                "Layout",
                "anchor",
                NodeFieldType::enumeration(UI_ANCHOR_OPTIONS),
            )
            .with_default(SceneValue::Str(Cow::Borrowed("center"))),
        );
        fields.push(
            SceneNodeField::new("Layout", "size_ratio", NodeFieldType::Vec2)
                .with_default(SceneValue::Vec2 { x: 0.20, y: 0.12 }),
        );
        push_default(fields, node_type, "Transform", "scale", NodeFieldType::Vec2);
        push_default(
            fields,
            node_type,
            "Transform",
            "rotation",
            NodeFieldType::F32,
        );
        push_default(
            fields,
            node_type,
            "Visibility",
            "visible",
            NodeFieldType::Bool,
        );
        push_default(
            fields,
            node_type,
            "Visibility",
            "modulate",
            NodeFieldType::Color,
        );
        push_default(fields, node_type, "Layout", "z_index", NodeFieldType::I32);
    }
}

fn push_node_fields(fields: &mut Vec<SceneNodeField>, node_type: NodeType) {
    match node_type {
        NodeType::Camera2D => {
            push(fields, "Camera", "zoom", NodeFieldType::F32);
            push(fields, "Camera", "render_mask", NodeFieldType::BitMask);
            push(
                fields,
                "Camera",
                "post_processing",
                NodeFieldType::object(Vec::new()),
            );
            push(
                fields,
                "Camera",
                "audio_options",
                NodeFieldType::object(Vec::new()),
            );
            push(fields, "Camera", "active", NodeFieldType::Bool);
        }
        NodeType::Camera3D => {
            push(
                fields,
                "Camera",
                "projection",
                NodeFieldType::enum_submenu(CAMERA_PROJECTION_SUBMENUS),
            );
            push(
                fields,
                "Camera",
                "perspective_fov_y_degrees",
                NodeFieldType::F32,
            );
            push(fields, "Camera", "perspective_near", NodeFieldType::F32);
            push(fields, "Camera", "perspective_far", NodeFieldType::F32);
            push(fields, "Camera", "orthographic_size", NodeFieldType::F32);
            push(fields, "Camera", "render_mask", NodeFieldType::BitMask);
            push(
                fields,
                "Camera",
                "post_processing",
                NodeFieldType::object(Vec::new()),
            );
            push(
                fields,
                "Camera",
                "audio_options",
                NodeFieldType::object(Vec::new()),
            );
            push(fields, "Camera", "active", NodeFieldType::Bool);
        }
        NodeType::CameraStream2D | NodeType::CameraStream3D | NodeType::UiCameraStream => {
            push(
                fields,
                "Camera Stream",
                "camera",
                NodeFieldType::NodeRef(NodeRefHint::many(CAMERA_REF_TYPES)),
            );
            push(fields, "Camera Stream", "resolution", NodeFieldType::Vec2);
            push(fields, "Camera Stream", "aspect_ratio", NodeFieldType::F32);
            push(
                fields,
                "Camera Stream",
                "aspect_mode",
                NodeFieldType::enumeration(CAMERA_STREAM_ASPECT_MODE_OPTIONS),
            );
            push(
                fields,
                "Camera Stream",
                "post_processing",
                NodeFieldType::object(Vec::new()),
            );
            push(fields, "Camera Stream", "enabled", NodeFieldType::Bool);
        }
        NodeType::Sprite2D => sprite_fields(fields, "Sprite"),
        NodeType::Button2D => button_2d_fields(fields, "Button"),
        NodeType::ImageButton2D => {
            button_2d_fields(fields, "Button");
            texture_field(fields, "Image", "texture");
            push(fields, "Image", "texture_region", NodeFieldType::Vec4);
        }
        NodeType::NineSlice2D => {
            texture_field(fields, "Nine Slice", "texture");
            push(fields, "Nine Slice", "texture_region", NodeFieldType::Vec4);
            push(fields, "Nine Slice", "margins", NodeFieldType::Vec4);
        }
        NodeType::AnimatedSprite2D => animated_image_fields(fields, "Animated Sprite"),
        NodeType::ParticleEmitter2D => particle_fields(fields, "Particles", false),
        NodeType::ParticleEmitter3D => particle_fields(fields, "Particles", true),
        NodeType::TileMap2D => {
            asset_field(fields, "Tile Map", "tileset", SceneAssetKind::TileSet);
            push(fields, "Tile Map", "width", NodeFieldType::U32);
            push(fields, "Tile Map", "height", NodeFieldType::U32);
            push(fields, "Tile Map", "empty_tile", NodeFieldType::I32);
            push(
                fields,
                "Tile Map",
                "tiles",
                NodeFieldType::array(NodeFieldType::I32),
            );
            push(fields, "Physics", "collision_enabled", NodeFieldType::Bool);
            push(
                fields,
                "Physics",
                "collision_layers",
                NodeFieldType::BitMask,
            );
            push(fields, "Physics", "collision_mask", NodeFieldType::BitMask);
        }
        NodeType::WaterBody2D | NodeType::WaterBody3D => water_fields(fields),
        NodeType::AmbientLight2D
        | NodeType::RayLight2D
        | NodeType::PointLight2D
        | NodeType::SpotLight2D
        | NodeType::AmbientLight3D
        | NodeType::RayLight3D
        | NodeType::PointLight3D
        | NodeType::SpotLight3D => light_fields(fields, node_type),
        NodeType::MeshInstance3D | NodeType::MultiMeshInstance3D => mesh_fields(fields, node_type),
        NodeType::Decal3D => decal_fields(fields),
        NodeType::Skeleton2D | NodeType::Skeleton3D => {
            asset_field(fields, "Skeleton", "skeleton", SceneAssetKind::Skeleton);
        }
        NodeType::BoneAttachment2D | NodeType::BoneAttachment3D => {
            push(
                fields,
                "Skeleton",
                "skeleton",
                NodeFieldType::NodeRef(NodeRefHint::many(if node_type.is_3d() {
                    SKELETON_3D_REF_TYPES
                } else {
                    SKELETON_2D_REF_TYPES
                })),
            );
            push(fields, "Skeleton", "bone_index", NodeFieldType::I32);
        }
        NodeType::IKTarget2D | NodeType::IKTarget3D => {
            push(
                fields,
                "IK",
                "skeleton",
                NodeFieldType::NodeRef(NodeRefHint::many(if node_type.is_3d() {
                    SKELETON_3D_REF_TYPES
                } else {
                    SKELETON_2D_REF_TYPES
                })),
            );
            push(fields, "IK", "bone_index", NodeFieldType::I32);
            push(fields, "IK", "chain_length", NodeFieldType::U32);
            push(fields, "IK", "iterations", NodeFieldType::U32);
            push(fields, "IK", "tolerance", NodeFieldType::F32);
            push(fields, "IK", "weight", NodeFieldType::F32);
            push(fields, "IK", "match_rotation", NodeFieldType::Bool);
            push(
                fields,
                "IK",
                "solver",
                NodeFieldType::enumeration(IK_SOLVER_OPTIONS),
            );
        }
        NodeType::PhysicsBoneChain2D | NodeType::PhysicsBoneChain3D => {
            push(
                fields,
                "Physics Bone",
                "skeleton",
                NodeFieldType::NodeRef(NodeRefHint::many(if node_type.is_3d() {
                    SKELETON_3D_REF_TYPES
                } else {
                    SKELETON_2D_REF_TYPES
                })),
            );
            push(fields, "Physics Bone", "bone_index", NodeFieldType::I32);
            push(fields, "Physics Bone", "chain_length", NodeFieldType::U32);
            push(fields, "Physics Bone", "enabled", NodeFieldType::Bool);
            push(
                fields,
                "Physics Bone",
                "gravity",
                if node_type.is_3d() {
                    NodeFieldType::Vec3
                } else {
                    NodeFieldType::Vec2
                },
            );
            push(fields, "Physics Bone", "damping", NodeFieldType::F32);
            push(fields, "Physics Bone", "stiffness", NodeFieldType::F32);
            push(fields, "Physics Bone", "radius", NodeFieldType::F32);
            push(fields, "Physics Bone", "collisions", NodeFieldType::Bool);
            push(fields, "Physics Bone", "iterations", NodeFieldType::U32);
        }
        NodeType::BoneCollider2D | NodeType::BoneCollider3D => {
            push(fields, "Physics Bone", "enabled", NodeFieldType::Bool);
        }
        NodeType::CollisionShape2D => {
            push(
                fields,
                "Physics",
                "shape",
                NodeFieldType::object(Vec::new()),
            );
        }
        NodeType::CollisionShape3D => {
            push(
                fields,
                "Physics",
                "shape",
                NodeFieldType::object(Vec::new()),
            );
            asset_field(fields, "Physics", "trimesh", SceneAssetKind::Mesh);
            push(fields, "Physics", "flip_x", NodeFieldType::Bool);
            push(fields, "Physics", "flip_y", NodeFieldType::Bool);
            push(fields, "Physics", "flip_z", NodeFieldType::Bool);
            push(fields, "Physics", "debug", NodeFieldType::Bool);
        }
        NodeType::StaticBody2D
        | NodeType::StaticBody3D
        | NodeType::Area2D
        | NodeType::Area3D
        | NodeType::RigidBody2D
        | NodeType::RigidBody3D
        | NodeType::CharacterBody2D
        | NodeType::CharacterBody3D => physics_body_fields(fields, node_type),
        NodeType::PhysicsForceEmitter2D | NodeType::PhysicsForceEmitter3D => {
            push(fields, "Force", "enabled", NodeFieldType::Bool);
            push(
                fields,
                "Force",
                "profile",
                NodeFieldType::object(Vec::new()),
            );
            push(fields, "Force", "radius", NodeFieldType::F32);
            push(fields, "Force", "strength", NodeFieldType::F32);
            push(fields, "Force", "duration", NodeFieldType::F32);
            push(fields, "Force", "pulse", NodeFieldType::Bool);
            push(fields, "Force", "falloff", NodeFieldType::F32);
            push(fields, "Force", "affect_bodies", NodeFieldType::Bool);
            push(fields, "Force", "affect_water", NodeFieldType::Bool);
            push(fields, "Force", "collision_layers", NodeFieldType::BitMask);
            push(fields, "Force", "collision_mask", NodeFieldType::BitMask);
            push(
                fields,
                "Force",
                "vectors",
                NodeFieldType::array(NodeFieldType::Vec3),
            );
        }
        NodeType::PinJoint2D
        | NodeType::DistanceJoint2D
        | NodeType::FixedJoint2D
        | NodeType::BallJoint3D
        | NodeType::HingeJoint3D
        | NodeType::FixedJoint3D => {
            joint_fields(fields, node_type);
        }
        NodeType::AnimationPlayer => {
            asset_field(fields, "Animation", "animation", SceneAssetKind::Animation);
            push(
                fields,
                "Animation",
                "bindings",
                NodeFieldType::object(Vec::new()),
            );
            push(fields, "Animation", "speed", NodeFieldType::F32);
            push(fields, "Animation", "paused", NodeFieldType::Bool);
            push(
                fields,
                "Animation",
                "playback",
                NodeFieldType::enumeration(ANIMATION_PLAYBACK_OPTIONS),
            );
        }
        NodeType::AnimationTree => {
            asset_field(fields, "Animation", "tree", SceneAssetKind::AnimationTree);
            push(
                fields,
                "Animation",
                "animations",
                NodeFieldType::array(NodeFieldType::String),
            );
            push(
                fields,
                "Animation",
                "bindings",
                NodeFieldType::object(Vec::new()),
            );
            push(fields, "Animation", "speed", NodeFieldType::F32);
            push(fields, "Animation", "paused", NodeFieldType::Bool);
        }
        NodeType::Sky3D => sky_fields(fields),
        NodeType::UiImage | NodeType::UiImageButton | NodeType::UiNineSlice => {
            texture_field(fields, "Image", "texture");
            push(fields, "Image", "texture_region", NodeFieldType::Vec4);
            if matches!(node_type, NodeType::UiNineSlice) {
                push(fields, "Image", "margins", NodeFieldType::Vec4);
            }
        }
        NodeType::UiAnimatedImage => animated_image_fields(fields, "Image"),
        NodeType::UiPanel
        | NodeType::UiButton
        | NodeType::UiDropdown
        | NodeType::UiCheckbox
        | NodeType::UiColorPicker
        | NodeType::UiTextBox
        | NodeType::UiTextBlock => {
            asset_field(fields, "Style", "style", SceneAssetKind::UiStyle);
            ui_style_fields(fields, "Style", "");
            if matches!(
                node_type,
                NodeType::UiButton | NodeType::UiDropdown | NodeType::UiCheckbox
            ) {
                ui_style_fields(fields, "Hover", "hover_");
                ui_style_fields(fields, "Pressed", "pressed_");
                fields.push(
                    SceneNodeField::new("Text", "text", NodeFieldType::String)
                        .with_default(SceneValue::Str(Cow::Borrowed("New Node"))),
                );
            }
            if matches!(node_type, NodeType::UiDropdown) {
                push(fields, "State", "selected_index", NodeFieldType::I32);
                push(fields, "State", "open", NodeFieldType::Bool);
            }
            if matches!(node_type, NodeType::UiCheckbox) {
                push(fields, "State", "checked", NodeFieldType::Bool);
            }
            if matches!(node_type, NodeType::UiColorPicker) {
                push(fields, "State", "color", NodeFieldType::Color);
                push(fields, "State", "popup_open", NodeFieldType::Bool);
            }
            if matches!(node_type, NodeType::UiTextBox | NodeType::UiTextBlock) {
                asset_field(fields, "Style", "focused_style", SceneAssetKind::UiStyle);
                ui_style_fields(fields, "Focus", "focused_");
                push(fields, "Text", "text", NodeFieldType::String);
                push(fields, "Text", "placeholder", NodeFieldType::String);
            }
        }
        NodeType::UiLabel => {
            fields.push(
                SceneNodeField::new("Text", "text", NodeFieldType::String)
                    .with_default(SceneValue::Str(Cow::Borrowed("New Node"))),
            );
            push(fields, "Text", "color", NodeFieldType::Color);
            push(fields, "Text", "text_size_ratio", NodeFieldType::F32);
            push(
                fields,
                "Text",
                "h_align",
                NodeFieldType::enumeration(UI_TEXT_ALIGN_OPTIONS),
            );
            push(
                fields,
                "Text",
                "v_align",
                NodeFieldType::enumeration(UI_TEXT_ALIGN_OPTIONS),
            );
        }
        NodeType::UiScrollContainer => {
            push(fields, "Scroll", "scroll", NodeFieldType::Vec2);
            push(
                fields,
                "Scroll",
                "scroll_dir",
                NodeFieldType::enumeration(UI_SCROLL_DIRECTION_OPTIONS),
            );
            push(
                fields,
                "Scroll",
                "scrollbar_side",
                NodeFieldType::enumeration(UI_SCROLL_BAR_SIDE_OPTIONS),
            );
            push(fields, "Scroll", "scrollbar_padding", NodeFieldType::F32);
        }
        NodeType::UiLayout
        | NodeType::UiHLayout
        | NodeType::UiVLayout
        | NodeType::UiGrid
        | NodeType::UiTreeList => {
            push(fields, "Layout", "spacing", NodeFieldType::F32);
            push(fields, "Layout", "h_spacing", NodeFieldType::F32);
            push(fields, "Layout", "v_spacing", NodeFieldType::F32);
            if matches!(node_type, NodeType::UiGrid | NodeType::UiLayout) {
                push(fields, "Layout", "columns", NodeFieldType::U32);
            }
            if matches!(node_type, NodeType::UiTreeList) {
                push(fields, "Layout", "indent", NodeFieldType::F32);
            }
            if matches!(node_type, NodeType::UiTreeList) {
                push(fields, "Layout", "row_height", NodeFieldType::F32);
                push(fields, "Layout", "icon_size", NodeFieldType::F32);
                push(fields, "Layout", "toggle_size", NodeFieldType::F32);
                push(
                    fields,
                    "Tree",
                    "items",
                    NodeFieldType::array(NodeFieldType::object(Vec::new())),
                );
                push(fields, "Tree", "selected_index", NodeFieldType::I32);
                push(fields, "Tree", "line_color", NodeFieldType::Color);
                push(fields, "Tree", "triangle_color", NodeFieldType::Color);
                push(fields, "Tree", "text_color", NodeFieldType::Color);
            }
        }
        NodeType::AudioEffectZone2D | NodeType::AudioEffectZone3D => {
            push(fields, "Audio", "enabled", NodeFieldType::Bool);
            push(fields, "Audio", "audio_mask", NodeFieldType::BitMask);
            push(fields, "Audio", "bounce", NodeFieldType::Bool);
            push(
                fields,
                "Audio",
                "effects",
                NodeFieldType::array(NodeFieldType::String),
            );
        }
        NodeType::AudioPortal2D | NodeType::AudioPortal3D => {
            push(fields, "Audio", "enabled", NodeFieldType::Bool);
            push(fields, "Audio", "strength", NodeFieldType::F32);
            push(
                fields,
                "Audio",
                "targets",
                NodeFieldType::array(NodeFieldType::NodeRef(NodeRefHint::any())),
            );
        }
        _ => {}
    }
}

fn push_default(
    fields: &mut Vec<SceneNodeField>,
    node_type: NodeType,
    section: &'static str,
    name: &'static str,
    kind: NodeFieldType,
) {
    let mut field = SceneNodeField::new(section, name, kind);
    if let Some(default) = default_scene_field_value_by_name(node_type, name) {
        field.default = Some(default);
    }
    fields.push(field);
}

fn push(
    fields: &mut Vec<SceneNodeField>,
    section: &'static str,
    name: &'static str,
    kind: NodeFieldType,
) {
    fields.push(SceneNodeField::new(section, name, kind));
}

fn asset_field(
    fields: &mut Vec<SceneNodeField>,
    section: &'static str,
    name: &'static str,
    kind: SceneAssetKind,
) {
    push(fields, section, name, NodeFieldType::Asset(kind));
}

fn texture_field(fields: &mut Vec<SceneNodeField>, section: &'static str, name: &'static str) {
    asset_field(fields, section, name, SceneAssetKind::Texture);
}

fn decal_fields(fields: &mut Vec<SceneNodeField>) {
    push(fields, "Decal", "size", NodeFieldType::Vec3);
    texture_field(fields, "Decal", "albedo_texture");
    texture_field(fields, "Decal", "normal_texture");
    texture_field(fields, "Decal", "emission_texture");
    push(fields, "Decal", "albedo_mix", NodeFieldType::F32);
    push(fields, "Decal", "emission_energy", NodeFieldType::F32);
    push(fields, "Decal", "normal_strength", NodeFieldType::F32);
    push(fields, "Decal", "normal_fade", NodeFieldType::F32);
    push(fields, "Decal", "distance_fade_begin", NodeFieldType::F32);
    push(fields, "Decal", "distance_fade_length", NodeFieldType::F32);
    push(fields, "Decal", "sort_priority", NodeFieldType::I32);
    push(fields, "Decal", "active", NodeFieldType::Bool);
}

fn sprite_fields(fields: &mut Vec<SceneNodeField>, section: &'static str) {
    texture_field(fields, section, "texture");
    push(fields, section, "texture_region", NodeFieldType::Vec4);
    push(fields, section, "flip_x", NodeFieldType::Bool);
    push(fields, section, "flip_y", NodeFieldType::Bool);
}

fn button_2d_fields(fields: &mut Vec<SceneNodeField>, section: &'static str) {
    push(fields, section, "size", NodeFieldType::Vec2);
    push(fields, section, "input_enabled", NodeFieldType::Bool);
    push(fields, section, "disabled", NodeFieldType::Bool);
}

fn animated_image_fields(fields: &mut Vec<SceneNodeField>, section: &'static str) {
    texture_field(fields, section, "texture");
    push(
        fields,
        section,
        "animations",
        NodeFieldType::array(NodeFieldType::String),
    );
    push(fields, section, "texture_region", NodeFieldType::Vec4);
    push(fields, section, "current_animation", NodeFieldType::String);
    push(fields, section, "current_frame", NodeFieldType::U32);
    push(fields, section, "fps_scale", NodeFieldType::F32);
    push(fields, section, "playing", NodeFieldType::Bool);
    push(fields, section, "looping", NodeFieldType::Bool);
}

fn ui_style_fields(fields: &mut Vec<SceneNodeField>, section: &'static str, prefix: &'static str) {
    push(
        fields,
        section,
        Box::leak(format!("{prefix}fill_kind").into_boxed_str()),
        NodeFieldType::enumeration(UI_FILL_KIND_OPTIONS),
    );
    push(
        fields,
        section,
        Box::leak(format!("{prefix}gradient").into_boxed_str()),
        NodeFieldType::object(vec![
            NodeFieldDef::new("start_color", NodeFieldType::Color),
            NodeFieldDef::new("end_color", NodeFieldType::Color),
            NodeFieldDef::new("vector", NodeFieldType::Vec2),
        ]),
    );
    push(
        fields,
        section,
        Box::leak(format!("{prefix}corner_radii").into_boxed_str()),
        NodeFieldType::Vec4,
    );
    for name in [
        "fill",
        "stroke",
        "stroke_width",
        "radius",
        "radius_tl",
        "radius_tr",
        "radius_br",
        "radius_bl",
        "shadow",
        "outer_shadow",
        "inner_shadow",
        "highlight",
        "outer_highlight",
        "inner_highlight",
    ] {
        let ty = match name {
            "fill" | "stroke" => NodeFieldType::Color,
            "stroke_width" | "radius" | "radius_tl" | "radius_tr" | "radius_br" | "radius_bl" => {
                NodeFieldType::F32
            }
            _ => NodeFieldType::object(vec![
                NodeFieldDef::new("color", NodeFieldType::Color),
                NodeFieldDef::new("distance", NodeFieldType::F32),
                NodeFieldDef::new("falloff", NodeFieldType::F32),
                NodeFieldDef::new("vector", NodeFieldType::Vec2),
                NodeFieldDef::new("size", NodeFieldType::F32),
            ]),
        };
        push(
            fields,
            section,
            Box::leak(format!("{prefix}{name}").into_boxed_str()),
            ty,
        );
    }
}

fn particle_fields(fields: &mut Vec<SceneNodeField>, section: &'static str, is_3d: bool) {
    push(fields, section, "active", NodeFieldType::Bool);
    push(fields, section, "looping", NodeFieldType::Bool);
    push(fields, section, "prewarm", NodeFieldType::Bool);
    push(fields, section, "spawn_rate", NodeFieldType::F32);
    push(fields, section, "seed", NodeFieldType::U32);
    push(fields, section, "params", NodeFieldType::object(Vec::new()));
    asset_field(fields, section, "profile", SceneAssetKind::ParticleProfile);
    push(
        fields,
        section,
        "sim_mode",
        NodeFieldType::enumeration(if is_3d {
            PARTICLE_SIM_MODE_3D_OPTIONS
        } else {
            PARTICLE_SIM_MODE_2D_OPTIONS
        }),
    );
    if is_3d {
        push(
            fields,
            section,
            "render_mode",
            NodeFieldType::enumeration(PARTICLE_RENDER_MODE_3D_OPTIONS),
        );
    }
}

fn light_fields(fields: &mut Vec<SceneNodeField>, node_type: NodeType) {
    push(fields, "Light", "color", NodeFieldType::Color);
    push(fields, "Light", "intensity", NodeFieldType::F32);
    push(fields, "Light", "cast_shadows", NodeFieldType::Bool);
    push(fields, "Light", "active", NodeFieldType::Bool);
    push(fields, "Light", "render_layers", NodeFieldType::BitMask);
    if matches!(
        node_type,
        NodeType::PointLight2D
            | NodeType::PointLight3D
            | NodeType::SpotLight2D
            | NodeType::SpotLight3D
    ) {
        push(fields, "Light", "range", NodeFieldType::F32);
    }
    if matches!(node_type, NodeType::SpotLight2D | NodeType::SpotLight3D) {
        push(fields, "Light", "inner_angle_radians", NodeFieldType::F32);
        push(fields, "Light", "outer_angle_radians", NodeFieldType::F32);
    }
}

fn mesh_fields(fields: &mut Vec<SceneNodeField>, node_type: NodeType) {
    asset_field(fields, "Mesh", "mesh", SceneAssetKind::Mesh);
    push(
        fields,
        "Material",
        "surfaces",
        NodeFieldType::array(NodeFieldType::Asset(SceneAssetKind::Material)),
    );
    push(
        fields,
        "Mesh",
        "skeleton",
        NodeFieldType::NodeRef(NodeRefHint::many(SKELETON_3D_REF_TYPES)),
    );
    push(
        fields,
        "Mesh",
        "blend_shape_weights",
        NodeFieldType::array(NodeFieldType::F32),
    );
    push(fields, "Mesh", "flip_x", NodeFieldType::Bool);
    push(fields, "Mesh", "flip_y", NodeFieldType::Bool);
    push(fields, "Mesh", "flip_z", NodeFieldType::Bool);
    push(fields, "Mesh", "meshlets", NodeFieldType::Bool);
    push(fields, "Mesh", "min_lod", NodeFieldType::U32);
    push(fields, "Mesh", "max_lod", NodeFieldType::U32);
    push(fields, "Mesh", "cast_shadows", NodeFieldType::Bool);
    push(fields, "Mesh", "receive_shadows", NodeFieldType::Bool);
    push(fields, "Blend", "blend", NodeFieldType::object(Vec::new()));
    if node_type == NodeType::MultiMeshInstance3D {
        push(
            fields,
            "Instances",
            "instances",
            NodeFieldType::array(NodeFieldType::object(vec![
                NodeFieldDef::new("position", NodeFieldType::Vec3),
                NodeFieldDef::new("rotation", NodeFieldType::Quat),
                NodeFieldDef::new("rotation_deg", NodeFieldType::Vec3),
                NodeFieldDef::new("scale", NodeFieldType::Vec3),
                NodeFieldDef::new(
                    "blend_shape_weights",
                    NodeFieldType::array(NodeFieldType::F32),
                ),
            ])),
        );
        push(
            fields,
            "Instances",
            "instance_grid",
            NodeFieldType::object(Vec::new()),
        );
        push(fields, "Instances", "instance_scale", NodeFieldType::F32);
    }
}

fn water_fields(fields: &mut Vec<SceneNodeField>) {
    push(fields, "Water", "shape", NodeFieldType::object(Vec::new()));
    push(fields, "Water", "resolution", NodeFieldType::Vec2);
    push(fields, "Water", "render_resolution", NodeFieldType::Vec2);
    push(fields, "Water", "vertices_per_meter", NodeFieldType::F32);
    push(fields, "Water", "depth", NodeFieldType::F32);
    push(fields, "Water", "flow", NodeFieldType::Vec2);
    push(fields, "Water", "wind", NodeFieldType::Vec2);
    push(
        fields,
        "Water",
        "idle_mode",
        NodeFieldType::enumeration(WATER_IDLE_MODE_OPTIONS),
    );
    push(fields, "Water", "wave_speed", NodeFieldType::F32);
    push(fields, "Water", "wave_scale", NodeFieldType::F32);
    push(fields, "Water", "wave_length", NodeFieldType::F32);
    push(
        fields,
        "Physics",
        "collision_layers",
        NodeFieldType::BitMask,
    );
    push(fields, "Physics", "collision_mask", NodeFieldType::BitMask);
    push(fields, "Optics", "deep_color", NodeFieldType::Color);
    push(fields, "Optics", "shallow_color", NodeFieldType::Color);
    push(
        fields,
        "Optics",
        "optics",
        NodeFieldType::object(Vec::new()),
    );
    push(
        fields,
        "Material",
        "material",
        NodeFieldType::object(Vec::new()),
    );
    push(fields, "Debug", "debug", NodeFieldType::Bool);
}

fn physics_body_fields(fields: &mut Vec<SceneNodeField>, node_type: NodeType) {
    push(fields, "Physics", "enabled", NodeFieldType::Bool);
    push(
        fields,
        "Physics",
        "collision_layers",
        NodeFieldType::BitMask,
    );
    push(fields, "Physics", "collision_mask", NodeFieldType::BitMask);
    if matches!(
        node_type,
        NodeType::StaticBody2D
            | NodeType::StaticBody3D
            | NodeType::RigidBody2D
            | NodeType::RigidBody3D
            | NodeType::CharacterBody2D
            | NodeType::CharacterBody3D
    ) {
        push(fields, "Physics", "friction", NodeFieldType::F32);
        push(fields, "Physics", "restitution", NodeFieldType::F32);
        push(fields, "Physics", "density", NodeFieldType::F32);
    }
    if matches!(node_type, NodeType::RigidBody2D | NodeType::RigidBody3D) {
        push(
            fields,
            "Rigid Body",
            "continuous_collision_detection",
            NodeFieldType::Bool,
        );
        push(fields, "Rigid Body", "mass", NodeFieldType::F32);
        push(
            fields,
            "Rigid Body",
            "linear_velocity",
            if node_type.is_3d() {
                NodeFieldType::Vec3
            } else {
                NodeFieldType::Vec2
            },
        );
        push(
            fields,
            "Rigid Body",
            "angular_velocity",
            if node_type.is_3d() {
                NodeFieldType::Vec3
            } else {
                NodeFieldType::F32
            },
        );
        push(fields, "Rigid Body", "gravity_scale", NodeFieldType::F32);
        push(fields, "Rigid Body", "linear_damping", NodeFieldType::F32);
        push(fields, "Rigid Body", "angular_damping", NodeFieldType::F32);
        push(fields, "Rigid Body", "can_sleep", NodeFieldType::Bool);
        if node_type == NodeType::RigidBody2D {
            push(fields, "Rigid Body", "lock_rotation", NodeFieldType::Bool);
        }
    }
}

fn joint_fields(fields: &mut Vec<SceneNodeField>, node_type: NodeType) {
    let body_types = if node_type.is_3d() {
        BODY_3D_REF_TYPES
    } else {
        BODY_2D_REF_TYPES
    };
    push(
        fields,
        "Joint",
        "body_a",
        NodeFieldType::NodeRef(NodeRefHint::many(body_types)),
    );
    push(
        fields,
        "Joint",
        "body_b",
        NodeFieldType::NodeRef(NodeRefHint::many(body_types)),
    );
    let vec_kind = if node_type.is_3d() {
        NodeFieldType::Vec3
    } else {
        NodeFieldType::Vec2
    };
    push(fields, "Joint", "anchor_a", vec_kind.clone());
    push(fields, "Joint", "anchor_b", vec_kind);
    push(fields, "Joint", "enabled", NodeFieldType::Bool);
    push(fields, "Joint", "collide_connected", NodeFieldType::Bool);
    if node_type == NodeType::DistanceJoint2D {
        push(fields, "Joint", "min_distance", NodeFieldType::F32);
        push(fields, "Joint", "max_distance", NodeFieldType::F32);
    }
    if node_type == NodeType::HingeJoint3D {
        push(fields, "Joint", "axis", NodeFieldType::Vec3);
    }
}

fn sky_fields(fields: &mut Vec<SceneNodeField>) {
    push(
        fields,
        "Sky",
        "day_colors",
        NodeFieldType::array(NodeFieldType::Color),
    );
    push(
        fields,
        "Sky",
        "evening_colors",
        NodeFieldType::array(NodeFieldType::Color),
    );
    push(
        fields,
        "Sky",
        "night_colors",
        NodeFieldType::array(NodeFieldType::Color),
    );
    push(
        fields,
        "Sky",
        "horizon_colors",
        NodeFieldType::array(NodeFieldType::Color),
    );
    push(fields, "Sky", "time_of_day", NodeFieldType::F32);
    push(fields, "Sky", "time_paused", NodeFieldType::Bool);
    push(fields, "Sky", "time_scale", NodeFieldType::F32);
    push(fields, "Sky", "shaders", NodeFieldType::object(Vec::new()));
    push(fields, "Sky", "active", NodeFieldType::Bool);
    push(fields, "Sky", "render_layers", NodeFieldType::BitMask);
}

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
}
