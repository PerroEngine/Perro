use std::borrow::Cow;

use perro_nodes::NodeType;

use crate::{SceneFieldName, SceneObjectField, SceneValue, default_scene_field_value_by_name};

#[derive(Clone, Debug)]
pub enum NodeFieldType {
    Bool,
    I32,
    U32,
    F32,
    Vec2,
    Vec3,
    Vec4,
    Quat,
    Color,
    String,
    NodeRef,
    BitMask,
    Object(Vec<NodeFieldDef>),
    Array(Box<NodeFieldType>),
    Asset(SceneAssetKind),
    Unknown,
}

impl NodeFieldType {
    pub fn object(fields: Vec<NodeFieldDef>) -> Self {
        Self::Object(fields)
    }

    pub fn array(item: NodeFieldType) -> Self {
        Self::Array(Box::new(item))
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
            Self::Vec4 | Self::Quat | Self::Color => SceneValue::Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            },
            Self::String => SceneValue::Str(Cow::Borrowed("")),
            Self::NodeRef => SceneValue::Key(crate::SceneValueKey::from("null")),
            Self::Asset(_) => SceneValue::Str(Cow::Borrowed("")),
            Self::Array(_) => SceneValue::Array(Cow::Owned(Vec::new())),
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneAssetFilter {
    pub label: &'static str,
    pub extensions: &'static [&'static str],
}

#[derive(Clone, Debug)]
pub struct SceneInspectorField {
    pub section: &'static str,
    pub name: &'static str,
    pub ty: NodeFieldType,
    pub default: Option<SceneValue>,
    pub aliases: &'static [&'static str],
}

impl SceneInspectorField {
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

pub fn scene_asset_filters(kind: SceneAssetKind) -> &'static [SceneAssetFilter] {
    match kind {
        SceneAssetKind::Scene => &[SceneAssetFilter {
            label: "Scenes",
            extensions: &["scn"],
        }],
        SceneAssetKind::Script => &[SceneAssetFilter {
            label: "Rust Scripts",
            extensions: &["rs"],
        }],
        SceneAssetKind::Texture => &[SceneAssetFilter {
            label: "Images",
            extensions: &["png", "jpg", "jpeg", "webp", "bmp", "tga", "svg"],
        }],
        SceneAssetKind::Mesh | SceneAssetKind::Model => &[SceneAssetFilter {
            label: "Meshes",
            extensions: &["glb", "gltf", "pmesh", "obj", "fbx"],
        }],
        SceneAssetKind::Material => &[SceneAssetFilter {
            label: "Perro Materials",
            extensions: &["pmat"],
        }],
        SceneAssetKind::Animation => &[SceneAssetFilter {
            label: "Perro Animations",
            extensions: &["panim"],
        }],
        SceneAssetKind::AnimationTree => &[SceneAssetFilter {
            label: "Perro Animation Trees",
            extensions: &["panimtree"],
        }],
        SceneAssetKind::Skeleton => &[SceneAssetFilter {
            label: "Perro Skeletons",
            extensions: &["pskel", "pskel2d", "pskel3d"],
        }],
        SceneAssetKind::ParticleProfile => &[SceneAssetFilter {
            label: "Perro Particles",
            extensions: &["ppart"],
        }],
        SceneAssetKind::TileSet => &[SceneAssetFilter {
            label: "Perro Tile Sets",
            extensions: &["ptileset"],
        }],
        SceneAssetKind::UiStyle => &[SceneAssetFilter {
            label: "Perro UI Styles",
            extensions: &["uistyle"],
        }],
    }
}

pub fn scene_inspector_fields(node_type: NodeType) -> Vec<SceneInspectorField> {
    let mut fields = Vec::new();
    push_node_fields(&mut fields, node_type);
    push_base_fields(&mut fields, node_type);
    fields
}

pub fn scene_default_fields(node_type: NodeType) -> Vec<SceneObjectField> {
    scene_inspector_fields(node_type)
        .into_iter()
        .filter_map(|field| {
            field
                .default
                .map(|value| (SceneFieldName::from_name(field.name.to_string()), value))
        })
        .collect()
}

pub fn scene_inspector_asset_fields(node_type: NodeType) -> Vec<SceneInspectorField> {
    scene_inspector_fields(node_type)
        .into_iter()
        .filter(|field| matches!(field.ty, NodeFieldType::Asset(_)))
        .collect()
}

pub fn scene_inspector_field(node_type: NodeType, name: &str) -> Option<SceneInspectorField> {
    scene_inspector_fields(node_type)
        .into_iter()
        .find(|field| field.name == name || field.aliases.contains(&name))
}

fn push_base_fields(fields: &mut Vec<SceneInspectorField>, node_type: NodeType) {
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
        push_default(
            fields,
            node_type,
            "Transform",
            "scale",
            NodeFieldType::Vec2,
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
        push_default(
            fields,
            node_type,
            "Transform",
            "scale",
            NodeFieldType::Vec3,
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
        push_default(
            fields,
            node_type,
            "Visibility",
            "render_layers",
            NodeFieldType::BitMask,
        );
    } else if node_type.is_a(NodeType::UiNode) {
        fields.push(
            SceneInspectorField::new("Layout", "anchor", NodeFieldType::String)
                .with_default(SceneValue::Str(Cow::Borrowed("center"))),
        );
        fields.push(
            SceneInspectorField::new("Layout", "size_ratio", NodeFieldType::Vec2)
                .with_default(SceneValue::Vec2 { x: 0.20, y: 0.12 }),
        );
        push_default(
            fields,
            node_type,
            "Transform",
            "scale",
            NodeFieldType::Vec2,
        );
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
        push_default(
            fields,
            node_type,
            "Layout",
            "z_index",
            NodeFieldType::I32,
        );
    }
}

fn push_node_fields(fields: &mut Vec<SceneInspectorField>, node_type: NodeType) {
    match node_type {
        NodeType::Camera2D => {
            push(fields, "Camera", "zoom", NodeFieldType::F32);
            push(
                fields,
                "Camera",
                "render_mask",
                NodeFieldType::BitMask,
            );
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
                NodeFieldType::String,
            );
            push(
                fields,
                "Camera",
                "perspective_fov_y_degrees",
                NodeFieldType::F32,
            );
            push(
                fields,
                "Camera",
                "perspective_near",
                NodeFieldType::F32,
            );
            push(
                fields,
                "Camera",
                "perspective_far",
                NodeFieldType::F32,
            );
            push(
                fields,
                "Camera",
                "orthographic_size",
                NodeFieldType::F32,
            );
            push(
                fields,
                "Camera",
                "render_mask",
                NodeFieldType::BitMask,
            );
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
                NodeFieldType::NodeRef,
            );
            push(
                fields,
                "Camera Stream",
                "resolution",
                NodeFieldType::Vec2,
            );
            push(
                fields,
                "Camera Stream",
                "aspect_ratio",
                NodeFieldType::F32,
            );
            push(
                fields,
                "Camera Stream",
                "aspect_mode",
                NodeFieldType::String,
            );
            push(
                fields,
                "Camera Stream",
                "post_processing",
                NodeFieldType::object(Vec::new()),
            );
            push(
                fields,
                "Camera Stream",
                "enabled",
                NodeFieldType::Bool,
            );
        }
        NodeType::Sprite2D => sprite_fields(fields, "Sprite"),
        NodeType::Button2D => button_2d_fields(fields, "Button"),
        NodeType::ImageButton2D => {
            button_2d_fields(fields, "Button");
            texture_field(fields, "Image", "texture");
            push(
                fields,
                "Image",
                "texture_region",
                NodeFieldType::Vec4,
            );
        }
        NodeType::NineSlice2D => {
            texture_field(fields, "Nine Slice", "texture");
            push(
                fields,
                "Nine Slice",
                "texture_region",
                NodeFieldType::Vec4,
            );
            push(
                fields,
                "Nine Slice",
                "margins",
                NodeFieldType::Vec4,
            );
        }
        NodeType::AnimatedSprite2D => animated_image_fields(fields, "Animated Sprite"),
        NodeType::ParticleEmitter2D => particle_fields(fields, "Particles", false),
        NodeType::ParticleEmitter3D => particle_fields(fields, "Particles", true),
        NodeType::TileMap2D => {
            asset_field(fields, "Tile Map", "tileset", SceneAssetKind::TileSet);
            push(fields, "Tile Map", "width", NodeFieldType::U32);
            push(fields, "Tile Map", "height", NodeFieldType::U32);
            push(
                fields,
                "Tile Map",
                "empty_tile",
                NodeFieldType::I32,
            );
            push(fields, "Tile Map", "tiles", NodeFieldType::array(NodeFieldType::I32));
            push(
                fields,
                "Physics",
                "collision_enabled",
                NodeFieldType::Bool,
            );
            push(
                fields,
                "Physics",
                "collision_layers",
                NodeFieldType::BitMask,
            );
            push(
                fields,
                "Physics",
                "collision_mask",
                NodeFieldType::BitMask,
            );
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
        NodeType::Skeleton2D | NodeType::Skeleton3D => {
            asset_field(fields, "Skeleton", "skeleton", SceneAssetKind::Skeleton);
        }
        NodeType::BoneAttachment2D | NodeType::BoneAttachment3D => {
            push(
                fields,
                "Skeleton",
                "skeleton",
                NodeFieldType::NodeRef,
            );
            push(
                fields,
                "Skeleton",
                "bone_index",
                NodeFieldType::I32,
            );
        }
        NodeType::IKTarget2D | NodeType::IKTarget3D => {
            push(fields, "IK", "skeleton", NodeFieldType::NodeRef);
            push(fields, "IK", "bone_index", NodeFieldType::I32);
            push(fields, "IK", "chain_length", NodeFieldType::U32);
            push(fields, "IK", "iterations", NodeFieldType::U32);
            push(fields, "IK", "tolerance", NodeFieldType::F32);
            push(fields, "IK", "weight", NodeFieldType::F32);
            push(
                fields,
                "IK",
                "match_rotation",
                NodeFieldType::Bool,
            );
            push(fields, "IK", "solver", NodeFieldType::String);
        }
        NodeType::PhysicsBoneChain2D | NodeType::PhysicsBoneChain3D => {
            push(
                fields,
                "Physics Bone",
                "skeleton",
                NodeFieldType::NodeRef,
            );
            push(
                fields,
                "Physics Bone",
                "bone_index",
                NodeFieldType::I32,
            );
            push(
                fields,
                "Physics Bone",
                "chain_length",
                NodeFieldType::U32,
            );
            push(
                fields,
                "Physics Bone",
                "enabled",
                NodeFieldType::Bool,
            );
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
            push(
                fields,
                "Physics Bone",
                "damping",
                NodeFieldType::F32,
            );
            push(
                fields,
                "Physics Bone",
                "stiffness",
                NodeFieldType::F32,
            );
            push(
                fields,
                "Physics Bone",
                "radius",
                NodeFieldType::F32,
            );
            push(
                fields,
                "Physics Bone",
                "collisions",
                NodeFieldType::Bool,
            );
            push(
                fields,
                "Physics Bone",
                "iterations",
                NodeFieldType::U32,
            );
        }
        NodeType::BoneCollider2D | NodeType::BoneCollider3D => {
            push(
                fields,
                "Physics Bone",
                "enabled",
                NodeFieldType::Bool,
            );
        }
        NodeType::CollisionShape2D => {
            push(fields, "Physics", "shape", NodeFieldType::object(Vec::new()));
        }
        NodeType::CollisionShape3D => {
            push(fields, "Physics", "shape", NodeFieldType::object(Vec::new()));
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
        | NodeType::RigidBody3D => physics_body_fields(fields, node_type),
        NodeType::PhysicsForceEmitter2D | NodeType::PhysicsForceEmitter3D => {
            push(fields, "Force", "enabled", NodeFieldType::Bool);
            push(fields, "Force", "profile", NodeFieldType::object(Vec::new()));
            push(fields, "Force", "radius", NodeFieldType::F32);
            push(fields, "Force", "strength", NodeFieldType::F32);
            push(fields, "Force", "duration", NodeFieldType::F32);
            push(fields, "Force", "pulse", NodeFieldType::Bool);
            push(fields, "Force", "falloff", NodeFieldType::String);
            push(
                fields,
                "Force",
                "affect_bodies",
                NodeFieldType::Bool,
            );
            push(
                fields,
                "Force",
                "affect_water",
                NodeFieldType::Bool,
            );
            push(
                fields,
                "Force",
                "collision_layers",
                NodeFieldType::BitMask,
            );
            push(
                fields,
                "Force",
                "collision_mask",
                NodeFieldType::BitMask,
            );
            push(fields, "Force", "vectors", NodeFieldType::array(NodeFieldType::Vec3));
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
                NodeFieldType::String,
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
            push(
                fields,
                "Image",
                "texture_region",
                NodeFieldType::Vec4,
            );
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
            if matches!(
                node_type,
                NodeType::UiButton | NodeType::UiDropdown | NodeType::UiCheckbox
            ) {
                fields.push(
                    SceneInspectorField::new("Text", "text", NodeFieldType::String)
                        .with_default(SceneValue::Str(Cow::Borrowed("New Node"))),
                );
            }
            if matches!(node_type, NodeType::UiDropdown) {
                push(
                    fields,
                    "State",
                    "selected_index",
                    NodeFieldType::I32,
                );
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
                push(fields, "Text", "text", NodeFieldType::String);
                push(
                    fields,
                    "Text",
                    "placeholder",
                    NodeFieldType::String,
                );
            }
        }
        NodeType::UiLabel => {
            fields.push(
                SceneInspectorField::new("Text", "text", NodeFieldType::String)
                    .with_default(SceneValue::Str(Cow::Borrowed("New Node"))),
            );
            push(fields, "Text", "color", NodeFieldType::Color);
            push(
                fields,
                "Text",
                "text_size_ratio",
                NodeFieldType::F32,
            );
            push(fields, "Text", "h_align", NodeFieldType::String);
            push(fields, "Text", "v_align", NodeFieldType::String);
        }
        NodeType::UiScrollContainer => {
            push(fields, "Scroll", "scroll", NodeFieldType::Vec2);
        }
        NodeType::UiLayout
        | NodeType::UiHLayout
        | NodeType::UiVLayout
        | NodeType::UiGrid
        | NodeType::UiList => {
            push(fields, "Layout", "spacing", NodeFieldType::F32);
            push(fields, "Layout", "h_spacing", NodeFieldType::F32);
            push(fields, "Layout", "v_spacing", NodeFieldType::F32);
            if matches!(node_type, NodeType::UiGrid | NodeType::UiLayout) {
                push(fields, "Layout", "columns", NodeFieldType::U32);
            }
            if matches!(node_type, NodeType::UiList) {
                push(fields, "Layout", "indent", NodeFieldType::F32);
            }
        }
        NodeType::AudioEffectZone2D | NodeType::AudioEffectZone3D => {
            push(fields, "Audio", "enabled", NodeFieldType::Bool);
            push(
                fields,
                "Audio",
                "audio_mask",
                NodeFieldType::BitMask,
            );
            push(fields, "Audio", "bounce", NodeFieldType::Bool);
            push(fields, "Audio", "effects", NodeFieldType::array(NodeFieldType::String));
        }
        NodeType::AudioPortal2D | NodeType::AudioPortal3D => {
            push(fields, "Audio", "enabled", NodeFieldType::Bool);
            push(fields, "Audio", "strength", NodeFieldType::F32);
            push(fields, "Audio", "targets", NodeFieldType::array(NodeFieldType::NodeRef));
        }
        _ => {}
    }
}

fn push_default(
    fields: &mut Vec<SceneInspectorField>,
    node_type: NodeType,
    section: &'static str,
    name: &'static str,
    kind: NodeFieldType,
) {
    let mut field = SceneInspectorField::new(section, name, kind);
    if let Some(default) = default_scene_field_value_by_name(node_type, name) {
        field.default = Some(default);
    }
    fields.push(field);
}

fn push(
    fields: &mut Vec<SceneInspectorField>,
    section: &'static str,
    name: &'static str,
    kind: NodeFieldType,
) {
    fields.push(SceneInspectorField::new(section, name, kind));
}

fn asset_field(
    fields: &mut Vec<SceneInspectorField>,
    section: &'static str,
    name: &'static str,
    kind: SceneAssetKind,
) {
    push(fields, section, name, NodeFieldType::Asset(kind));
}

fn texture_field(fields: &mut Vec<SceneInspectorField>, section: &'static str, name: &'static str) {
    asset_field(fields, section, name, SceneAssetKind::Texture);
}

fn sprite_fields(fields: &mut Vec<SceneInspectorField>, section: &'static str) {
    texture_field(fields, section, "texture");
    push(
        fields,
        section,
        "texture_region",
        NodeFieldType::Vec4,
    );
    push(fields, section, "flip_x", NodeFieldType::Bool);
    push(fields, section, "flip_y", NodeFieldType::Bool);
}

fn button_2d_fields(fields: &mut Vec<SceneInspectorField>, section: &'static str) {
    push(fields, section, "size", NodeFieldType::Vec2);
    push(
        fields,
        section,
        "input_enabled",
        NodeFieldType::Bool,
    );
    push(fields, section, "disabled", NodeFieldType::Bool);
}

fn animated_image_fields(fields: &mut Vec<SceneInspectorField>, section: &'static str) {
    texture_field(fields, section, "texture");
    push(
        fields,
        section,
        "animations",
        NodeFieldType::array(NodeFieldType::String),
    );
    push(
        fields,
        section,
        "texture_region",
        NodeFieldType::Vec4,
    );
    push(
        fields,
        section,
        "current_animation",
        NodeFieldType::String,
    );
    push(
        fields,
        section,
        "current_frame",
        NodeFieldType::U32,
    );
    push(fields, section, "fps_scale", NodeFieldType::F32);
    push(fields, section, "playing", NodeFieldType::Bool);
    push(fields, section, "looping", NodeFieldType::Bool);
}

fn particle_fields(fields: &mut Vec<SceneInspectorField>, section: &'static str, is_3d: bool) {
    push(fields, section, "active", NodeFieldType::Bool);
    push(fields, section, "looping", NodeFieldType::Bool);
    push(fields, section, "prewarm", NodeFieldType::Bool);
    push(fields, section, "spawn_rate", NodeFieldType::F32);
    push(fields, section, "seed", NodeFieldType::U32);
    push(fields, section, "params", NodeFieldType::object(Vec::new()));
    asset_field(fields, section, "profile", SceneAssetKind::ParticleProfile);
    push(fields, section, "sim_mode", NodeFieldType::String);
    if is_3d {
        push(
            fields,
            section,
            "render_mode",
            NodeFieldType::String,
        );
    }
}

fn light_fields(fields: &mut Vec<SceneInspectorField>, node_type: NodeType) {
    push(fields, "Light", "color", NodeFieldType::Color);
    push(fields, "Light", "intensity", NodeFieldType::F32);
    push(
        fields,
        "Light",
        "cast_shadows",
        NodeFieldType::Bool,
    );
    push(fields, "Light", "active", NodeFieldType::Bool);
    push(
        fields,
        "Light",
        "render_layers",
        NodeFieldType::BitMask,
    );
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
        push(
            fields,
            "Light",
            "inner_angle_radians",
            NodeFieldType::F32,
        );
        push(
            fields,
            "Light",
            "outer_angle_radians",
            NodeFieldType::F32,
        );
    }
}

fn mesh_fields(fields: &mut Vec<SceneInspectorField>, node_type: NodeType) {
    asset_field(fields, "Mesh", "mesh", SceneAssetKind::Mesh);
    asset_field(fields, "Material", "material", SceneAssetKind::Material);
    push(
        fields,
        "Material",
        "surfaces",
        NodeFieldType::array(NodeFieldType::Asset(SceneAssetKind::Material)),
    );
    push(fields, "Mesh", "skeleton", NodeFieldType::NodeRef);
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
    push(
        fields,
        "Mesh",
        "cast_shadows",
        NodeFieldType::Bool,
    );
    push(
        fields,
        "Mesh",
        "receive_shadows",
        NodeFieldType::Bool,
    );
    push(fields, "Blend", "blend", NodeFieldType::object(Vec::new()));
    if node_type == NodeType::MultiMeshInstance3D {
        push(
            fields,
            "Instances",
            "instances",
            NodeFieldType::array(NodeFieldType::Vec3),
        );
        push(
            fields,
            "Instances",
            "instance_grid",
            NodeFieldType::object(Vec::new()),
        );
        push(
            fields,
            "Instances",
            "instance_scale",
            NodeFieldType::F32,
        );
    }
}

fn water_fields(fields: &mut Vec<SceneInspectorField>) {
    push(fields, "Water", "shape", NodeFieldType::object(Vec::new()));
    push(fields, "Water", "resolution", NodeFieldType::Vec2);
    push(
        fields,
        "Water",
        "render_resolution",
        NodeFieldType::Vec2,
    );
    push(
        fields,
        "Water",
        "vertices_per_meter",
        NodeFieldType::F32,
    );
    push(fields, "Water", "depth", NodeFieldType::F32);
    push(fields, "Water", "flow", NodeFieldType::Vec2);
    push(fields, "Water", "wind", NodeFieldType::Vec2);
    push(
        fields,
        "Water",
        "idle_mode",
        NodeFieldType::String,
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
    push(
        fields,
        "Physics",
        "collision_mask",
        NodeFieldType::BitMask,
    );
    push(
        fields,
        "Optics",
        "deep_color",
        NodeFieldType::Color,
    );
    push(
        fields,
        "Optics",
        "shallow_color",
        NodeFieldType::Color,
    );
    push(fields, "Optics", "optics", NodeFieldType::object(Vec::new()));
    push(
        fields,
        "Material",
        "material",
        NodeFieldType::object(Vec::new()),
    );
    push(fields, "Debug", "debug", NodeFieldType::Bool);
}

fn physics_body_fields(fields: &mut Vec<SceneInspectorField>, node_type: NodeType) {
    push(fields, "Physics", "enabled", NodeFieldType::Bool);
    push(
        fields,
        "Physics",
        "collision_layers",
        NodeFieldType::BitMask,
    );
    push(
        fields,
        "Physics",
        "collision_mask",
        NodeFieldType::BitMask,
    );
    if matches!(
        node_type,
        NodeType::StaticBody2D
            | NodeType::StaticBody3D
            | NodeType::RigidBody2D
            | NodeType::RigidBody3D
    ) {
        push(fields, "Physics", "friction", NodeFieldType::F32);
        push(
            fields,
            "Physics",
            "restitution",
            NodeFieldType::F32,
        );
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
        push(
            fields,
            "Rigid Body",
            "gravity_scale",
            NodeFieldType::F32,
        );
        push(
            fields,
            "Rigid Body",
            "linear_damping",
            NodeFieldType::F32,
        );
        push(
            fields,
            "Rigid Body",
            "angular_damping",
            NodeFieldType::F32,
        );
        push(
            fields,
            "Rigid Body",
            "can_sleep",
            NodeFieldType::Bool,
        );
        if node_type == NodeType::RigidBody2D {
            push(
                fields,
                "Rigid Body",
                "lock_rotation",
                NodeFieldType::Bool,
            );
        }
    }
}

fn joint_fields(fields: &mut Vec<SceneInspectorField>, node_type: NodeType) {
    push(fields, "Joint", "body_a", NodeFieldType::NodeRef);
    push(fields, "Joint", "body_b", NodeFieldType::NodeRef);
    let vec_kind = if node_type.is_3d() {
        NodeFieldType::Vec3
    } else {
        NodeFieldType::Vec2
    };
    push(fields, "Joint", "anchor_a", vec_kind.clone());
    push(fields, "Joint", "anchor_b", vec_kind);
    push(fields, "Joint", "enabled", NodeFieldType::Bool);
    push(
        fields,
        "Joint",
        "collide_connected",
        NodeFieldType::Bool,
    );
    if node_type == NodeType::DistanceJoint2D {
        push(
            fields,
            "Joint",
            "min_distance",
            NodeFieldType::F32,
        );
        push(
            fields,
            "Joint",
            "max_distance",
            NodeFieldType::F32,
        );
    }
    if node_type == NodeType::HingeJoint3D {
        push(fields, "Joint", "axis", NodeFieldType::Vec3);
    }
}

fn sky_fields(fields: &mut Vec<SceneInspectorField>) {
    push(fields, "Sky", "day_colors", NodeFieldType::array(NodeFieldType::Color));
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
    push(
        fields,
        "Sky",
        "render_layers",
        NodeFieldType::BitMask,
    );
}
