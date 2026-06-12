use std::borrow::Cow;

use perro_nodes::NodeType;

use crate::{SceneFieldName, SceneObjectField, SceneValue, default_scene_field_value_by_name};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneInspectorValueKind {
    Bool,
    I32,
    U32,
    F32,
    Vec2,
    Vec3,
    Vec4,
    String,
    NodeRef,
    BitMask,
    Object,
    Array,
    Asset(SceneAssetKind),
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
    pub kind: SceneInspectorValueKind,
    pub default: Option<SceneValue>,
    pub aliases: &'static [&'static str],
}

impl SceneInspectorField {
    pub fn new(section: &'static str, name: &'static str, kind: SceneInspectorValueKind) -> Self {
        Self {
            section,
            name,
            kind,
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
    push_base_fields(&mut fields, node_type);
    push_node_fields(&mut fields, node_type);
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
        .filter(|field| matches!(field.kind, SceneInspectorValueKind::Asset(_)))
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
            SceneInspectorValueKind::Vec2,
        );
        push_default(
            fields,
            node_type,
            "Transform",
            "rotation",
            SceneInspectorValueKind::F32,
        );
        push_default(
            fields,
            node_type,
            "Transform",
            "scale",
            SceneInspectorValueKind::Vec2,
        );
        push_default(
            fields,
            node_type,
            "Visibility",
            "visible",
            SceneInspectorValueKind::Bool,
        );
        push_default(
            fields,
            node_type,
            "Visibility",
            "modulate",
            SceneInspectorValueKind::Vec4,
        );
        push_default(
            fields,
            node_type,
            "Visibility",
            "render_layers",
            SceneInspectorValueKind::BitMask,
        );
    } else if node_type.is_a(NodeType::Node3D) {
        push_default(
            fields,
            node_type,
            "Transform",
            "position",
            SceneInspectorValueKind::Vec3,
        );
        push_default(
            fields,
            node_type,
            "Transform",
            "rotation",
            SceneInspectorValueKind::Vec4,
        );
        push_default(
            fields,
            node_type,
            "Transform",
            "scale",
            SceneInspectorValueKind::Vec3,
        );
        push_default(
            fields,
            node_type,
            "Visibility",
            "visible",
            SceneInspectorValueKind::Bool,
        );
        push_default(
            fields,
            node_type,
            "Visibility",
            "modulate",
            SceneInspectorValueKind::Vec4,
        );
        push_default(
            fields,
            node_type,
            "Visibility",
            "render_layers",
            SceneInspectorValueKind::BitMask,
        );
    } else if node_type.is_a(NodeType::UiBox) {
        fields.push(
            SceneInspectorField::new("Layout", "anchor", SceneInspectorValueKind::String)
                .with_default(SceneValue::Str(Cow::Borrowed("center"))),
        );
        fields.push(
            SceneInspectorField::new("Layout", "size_ratio", SceneInspectorValueKind::Vec2)
                .with_default(SceneValue::Vec2 { x: 0.20, y: 0.12 }),
        );
        push_default(
            fields,
            node_type,
            "Transform",
            "scale",
            SceneInspectorValueKind::Vec2,
        );
        push_default(
            fields,
            node_type,
            "Transform",
            "rotation",
            SceneInspectorValueKind::F32,
        );
        push_default(
            fields,
            node_type,
            "Visibility",
            "visible",
            SceneInspectorValueKind::Bool,
        );
        push_default(
            fields,
            node_type,
            "Visibility",
            "modulate",
            SceneInspectorValueKind::Vec4,
        );
        push_default(
            fields,
            node_type,
            "Layout",
            "z_index",
            SceneInspectorValueKind::I32,
        );
    }
}

fn push_node_fields(fields: &mut Vec<SceneInspectorField>, node_type: NodeType) {
    match node_type {
        NodeType::Camera2D => {
            push(fields, "Camera", "zoom", SceneInspectorValueKind::F32);
            push(
                fields,
                "Camera",
                "render_mask",
                SceneInspectorValueKind::BitMask,
            );
            push(
                fields,
                "Camera",
                "post_processing",
                SceneInspectorValueKind::Object,
            );
            push(
                fields,
                "Camera",
                "audio_options",
                SceneInspectorValueKind::Object,
            );
            push(fields, "Camera", "active", SceneInspectorValueKind::Bool);
        }
        NodeType::Camera3D => {
            push(
                fields,
                "Camera",
                "projection",
                SceneInspectorValueKind::String,
            );
            push(
                fields,
                "Camera",
                "perspective_fov_y_degrees",
                SceneInspectorValueKind::F32,
            );
            push(
                fields,
                "Camera",
                "perspective_near",
                SceneInspectorValueKind::F32,
            );
            push(
                fields,
                "Camera",
                "perspective_far",
                SceneInspectorValueKind::F32,
            );
            push(
                fields,
                "Camera",
                "orthographic_size",
                SceneInspectorValueKind::F32,
            );
            push(
                fields,
                "Camera",
                "render_mask",
                SceneInspectorValueKind::BitMask,
            );
            push(
                fields,
                "Camera",
                "post_processing",
                SceneInspectorValueKind::Object,
            );
            push(
                fields,
                "Camera",
                "audio_options",
                SceneInspectorValueKind::Object,
            );
            push(fields, "Camera", "active", SceneInspectorValueKind::Bool);
        }
        NodeType::CameraStream2D | NodeType::CameraStream3D | NodeType::UiCameraStream => {
            push(
                fields,
                "Camera Stream",
                "camera",
                SceneInspectorValueKind::NodeRef,
            );
            push(
                fields,
                "Camera Stream",
                "resolution",
                SceneInspectorValueKind::Vec2,
            );
            push(
                fields,
                "Camera Stream",
                "aspect_ratio",
                SceneInspectorValueKind::F32,
            );
            push(
                fields,
                "Camera Stream",
                "aspect_mode",
                SceneInspectorValueKind::String,
            );
            push(
                fields,
                "Camera Stream",
                "post_processing",
                SceneInspectorValueKind::Object,
            );
            push(
                fields,
                "Camera Stream",
                "enabled",
                SceneInspectorValueKind::Bool,
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
                SceneInspectorValueKind::Vec4,
            );
        }
        NodeType::NineSlice2D => {
            texture_field(fields, "Nine Slice", "texture");
            push(
                fields,
                "Nine Slice",
                "texture_region",
                SceneInspectorValueKind::Vec4,
            );
            push(
                fields,
                "Nine Slice",
                "margins",
                SceneInspectorValueKind::Vec4,
            );
        }
        NodeType::AnimatedSprite2D => animated_image_fields(fields, "Animated Sprite"),
        NodeType::ParticleEmitter2D => particle_fields(fields, "Particles", false),
        NodeType::ParticleEmitter3D => particle_fields(fields, "Particles", true),
        NodeType::TileMap2D => {
            asset_field(fields, "Tile Map", "tileset", SceneAssetKind::TileSet);
            push(fields, "Tile Map", "width", SceneInspectorValueKind::U32);
            push(fields, "Tile Map", "height", SceneInspectorValueKind::U32);
            push(
                fields,
                "Tile Map",
                "empty_tile",
                SceneInspectorValueKind::I32,
            );
            push(fields, "Tile Map", "tiles", SceneInspectorValueKind::Array);
            push(
                fields,
                "Physics",
                "collision_enabled",
                SceneInspectorValueKind::Bool,
            );
            push(
                fields,
                "Physics",
                "collision_layers",
                SceneInspectorValueKind::BitMask,
            );
            push(
                fields,
                "Physics",
                "collision_mask",
                SceneInspectorValueKind::BitMask,
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
                SceneInspectorValueKind::NodeRef,
            );
            push(
                fields,
                "Skeleton",
                "bone_index",
                SceneInspectorValueKind::I32,
            );
        }
        NodeType::IKTarget2D | NodeType::IKTarget3D => {
            push(fields, "IK", "skeleton", SceneInspectorValueKind::NodeRef);
            push(fields, "IK", "bone_index", SceneInspectorValueKind::I32);
            push(fields, "IK", "chain_length", SceneInspectorValueKind::U32);
            push(fields, "IK", "iterations", SceneInspectorValueKind::U32);
            push(fields, "IK", "tolerance", SceneInspectorValueKind::F32);
            push(fields, "IK", "weight", SceneInspectorValueKind::F32);
            push(
                fields,
                "IK",
                "match_rotation",
                SceneInspectorValueKind::Bool,
            );
            push(fields, "IK", "solver", SceneInspectorValueKind::String);
        }
        NodeType::PhysicsBoneChain2D | NodeType::PhysicsBoneChain3D => {
            push(
                fields,
                "Physics Bone",
                "skeleton",
                SceneInspectorValueKind::NodeRef,
            );
            push(
                fields,
                "Physics Bone",
                "bone_index",
                SceneInspectorValueKind::I32,
            );
            push(
                fields,
                "Physics Bone",
                "chain_length",
                SceneInspectorValueKind::U32,
            );
            push(
                fields,
                "Physics Bone",
                "enabled",
                SceneInspectorValueKind::Bool,
            );
            push(
                fields,
                "Physics Bone",
                "gravity",
                if node_type.is_3d() {
                    SceneInspectorValueKind::Vec3
                } else {
                    SceneInspectorValueKind::Vec2
                },
            );
            push(
                fields,
                "Physics Bone",
                "damping",
                SceneInspectorValueKind::F32,
            );
            push(
                fields,
                "Physics Bone",
                "stiffness",
                SceneInspectorValueKind::F32,
            );
            push(
                fields,
                "Physics Bone",
                "radius",
                SceneInspectorValueKind::F32,
            );
            push(
                fields,
                "Physics Bone",
                "collisions",
                SceneInspectorValueKind::Bool,
            );
            push(
                fields,
                "Physics Bone",
                "iterations",
                SceneInspectorValueKind::U32,
            );
        }
        NodeType::BoneCollider2D | NodeType::BoneCollider3D => {
            push(
                fields,
                "Physics Bone",
                "enabled",
                SceneInspectorValueKind::Bool,
            );
        }
        NodeType::CollisionShape2D => {
            push(fields, "Physics", "shape", SceneInspectorValueKind::Object);
        }
        NodeType::CollisionShape3D => {
            push(fields, "Physics", "shape", SceneInspectorValueKind::Object);
            asset_field(fields, "Physics", "trimesh", SceneAssetKind::Mesh);
            push(fields, "Physics", "flip_x", SceneInspectorValueKind::Bool);
            push(fields, "Physics", "flip_y", SceneInspectorValueKind::Bool);
            push(fields, "Physics", "flip_z", SceneInspectorValueKind::Bool);
            push(fields, "Physics", "debug", SceneInspectorValueKind::Bool);
        }
        NodeType::StaticBody2D
        | NodeType::StaticBody3D
        | NodeType::Area2D
        | NodeType::Area3D
        | NodeType::RigidBody2D
        | NodeType::RigidBody3D => physics_body_fields(fields, node_type),
        NodeType::PhysicsForceEmitter2D | NodeType::PhysicsForceEmitter3D => {
            push(fields, "Force", "enabled", SceneInspectorValueKind::Bool);
            push(fields, "Force", "profile", SceneInspectorValueKind::Object);
            push(fields, "Force", "radius", SceneInspectorValueKind::F32);
            push(fields, "Force", "strength", SceneInspectorValueKind::F32);
            push(fields, "Force", "duration", SceneInspectorValueKind::F32);
            push(fields, "Force", "pulse", SceneInspectorValueKind::Bool);
            push(fields, "Force", "falloff", SceneInspectorValueKind::String);
            push(
                fields,
                "Force",
                "affect_bodies",
                SceneInspectorValueKind::Bool,
            );
            push(
                fields,
                "Force",
                "affect_water",
                SceneInspectorValueKind::Bool,
            );
            push(
                fields,
                "Force",
                "collision_layers",
                SceneInspectorValueKind::BitMask,
            );
            push(
                fields,
                "Force",
                "collision_mask",
                SceneInspectorValueKind::BitMask,
            );
            push(fields, "Force", "vectors", SceneInspectorValueKind::Array);
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
                SceneInspectorValueKind::Object,
            );
            push(fields, "Animation", "speed", SceneInspectorValueKind::F32);
            push(fields, "Animation", "paused", SceneInspectorValueKind::Bool);
            push(
                fields,
                "Animation",
                "playback",
                SceneInspectorValueKind::String,
            );
        }
        NodeType::AnimationTree => {
            asset_field(fields, "Animation", "tree", SceneAssetKind::AnimationTree);
            push(
                fields,
                "Animation",
                "animations",
                SceneInspectorValueKind::Array,
            );
            push(
                fields,
                "Animation",
                "bindings",
                SceneInspectorValueKind::Object,
            );
            push(fields, "Animation", "speed", SceneInspectorValueKind::F32);
            push(fields, "Animation", "paused", SceneInspectorValueKind::Bool);
        }
        NodeType::Sky3D => sky_fields(fields),
        NodeType::UiImage | NodeType::UiImageButton | NodeType::UiNineSlice => {
            texture_field(fields, "Image", "texture");
            push(
                fields,
                "Image",
                "texture_region",
                SceneInspectorValueKind::Vec4,
            );
            if matches!(node_type, NodeType::UiNineSlice) {
                push(fields, "Image", "margins", SceneInspectorValueKind::Vec4);
            }
        }
        NodeType::UiAnimatedImage => animated_image_fields(fields, "Image"),
        NodeType::UiPanel
        | NodeType::UiButton
        | NodeType::UiCheckbox
        | NodeType::UiTextBox
        | NodeType::UiTextBlock => {
            asset_field(fields, "Style", "style", SceneAssetKind::UiStyle);
            if matches!(node_type, NodeType::UiButton | NodeType::UiCheckbox) {
                fields.push(
                    SceneInspectorField::new("Text", "text", SceneInspectorValueKind::String)
                        .with_default(SceneValue::Str(Cow::Borrowed("New Node"))),
                );
            }
            if matches!(node_type, NodeType::UiCheckbox) {
                push(fields, "State", "checked", SceneInspectorValueKind::Bool);
            }
            if matches!(node_type, NodeType::UiTextBox | NodeType::UiTextBlock) {
                push(fields, "Text", "text", SceneInspectorValueKind::String);
                push(
                    fields,
                    "Text",
                    "placeholder",
                    SceneInspectorValueKind::String,
                );
            }
        }
        NodeType::UiLabel => {
            fields.push(
                SceneInspectorField::new("Text", "text", SceneInspectorValueKind::String)
                    .with_default(SceneValue::Str(Cow::Borrowed("New Node"))),
            );
            push(fields, "Text", "color", SceneInspectorValueKind::Vec4);
            push(
                fields,
                "Text",
                "text_size_ratio",
                SceneInspectorValueKind::F32,
            );
            push(fields, "Text", "h_align", SceneInspectorValueKind::String);
            push(fields, "Text", "v_align", SceneInspectorValueKind::String);
        }
        NodeType::UiScrollContainer => {
            push(fields, "Scroll", "scroll", SceneInspectorValueKind::Vec2);
        }
        NodeType::UiLayout
        | NodeType::UiHLayout
        | NodeType::UiVLayout
        | NodeType::UiGrid
        | NodeType::UiList => {
            push(fields, "Layout", "spacing", SceneInspectorValueKind::F32);
            push(fields, "Layout", "h_spacing", SceneInspectorValueKind::F32);
            push(fields, "Layout", "v_spacing", SceneInspectorValueKind::F32);
            if matches!(node_type, NodeType::UiGrid | NodeType::UiLayout) {
                push(fields, "Layout", "columns", SceneInspectorValueKind::U32);
            }
            if matches!(node_type, NodeType::UiList) {
                push(fields, "Layout", "indent", SceneInspectorValueKind::F32);
            }
        }
        NodeType::AudioEffectZone2D | NodeType::AudioEffectZone3D => {
            push(fields, "Audio", "enabled", SceneInspectorValueKind::Bool);
            push(
                fields,
                "Audio",
                "audio_mask",
                SceneInspectorValueKind::BitMask,
            );
            push(fields, "Audio", "bounce", SceneInspectorValueKind::Bool);
            push(fields, "Audio", "effects", SceneInspectorValueKind::Array);
        }
        NodeType::AudioPortal2D | NodeType::AudioPortal3D => {
            push(fields, "Audio", "enabled", SceneInspectorValueKind::Bool);
            push(fields, "Audio", "strength", SceneInspectorValueKind::F32);
            push(fields, "Audio", "targets", SceneInspectorValueKind::Array);
        }
        _ => {}
    }
}

fn push_default(
    fields: &mut Vec<SceneInspectorField>,
    node_type: NodeType,
    section: &'static str,
    name: &'static str,
    kind: SceneInspectorValueKind,
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
    kind: SceneInspectorValueKind,
) {
    fields.push(SceneInspectorField::new(section, name, kind));
}

fn asset_field(
    fields: &mut Vec<SceneInspectorField>,
    section: &'static str,
    name: &'static str,
    kind: SceneAssetKind,
) {
    push(fields, section, name, SceneInspectorValueKind::Asset(kind));
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
        SceneInspectorValueKind::Vec4,
    );
    push(fields, section, "flip_x", SceneInspectorValueKind::Bool);
    push(fields, section, "flip_y", SceneInspectorValueKind::Bool);
}

fn button_2d_fields(fields: &mut Vec<SceneInspectorField>, section: &'static str) {
    push(fields, section, "size", SceneInspectorValueKind::Vec2);
    push(
        fields,
        section,
        "input_enabled",
        SceneInspectorValueKind::Bool,
    );
    push(fields, section, "disabled", SceneInspectorValueKind::Bool);
}

fn animated_image_fields(fields: &mut Vec<SceneInspectorField>, section: &'static str) {
    texture_field(fields, section, "texture");
    push(
        fields,
        section,
        "animations",
        SceneInspectorValueKind::Array,
    );
    push(
        fields,
        section,
        "texture_region",
        SceneInspectorValueKind::Vec4,
    );
    push(
        fields,
        section,
        "current_animation",
        SceneInspectorValueKind::String,
    );
    push(
        fields,
        section,
        "current_frame",
        SceneInspectorValueKind::U32,
    );
    push(fields, section, "fps_scale", SceneInspectorValueKind::F32);
    push(fields, section, "playing", SceneInspectorValueKind::Bool);
    push(fields, section, "looping", SceneInspectorValueKind::Bool);
}

fn particle_fields(fields: &mut Vec<SceneInspectorField>, section: &'static str, is_3d: bool) {
    push(fields, section, "active", SceneInspectorValueKind::Bool);
    push(fields, section, "looping", SceneInspectorValueKind::Bool);
    push(fields, section, "prewarm", SceneInspectorValueKind::Bool);
    push(fields, section, "spawn_rate", SceneInspectorValueKind::F32);
    push(fields, section, "seed", SceneInspectorValueKind::U32);
    push(fields, section, "params", SceneInspectorValueKind::Object);
    asset_field(fields, section, "profile", SceneAssetKind::ParticleProfile);
    push(fields, section, "sim_mode", SceneInspectorValueKind::String);
    if is_3d {
        push(
            fields,
            section,
            "render_mode",
            SceneInspectorValueKind::String,
        );
    }
}

fn light_fields(fields: &mut Vec<SceneInspectorField>, node_type: NodeType) {
    push(fields, "Light", "color", SceneInspectorValueKind::Vec3);
    push(fields, "Light", "intensity", SceneInspectorValueKind::F32);
    push(
        fields,
        "Light",
        "cast_shadows",
        SceneInspectorValueKind::Bool,
    );
    push(fields, "Light", "active", SceneInspectorValueKind::Bool);
    push(
        fields,
        "Light",
        "render_layers",
        SceneInspectorValueKind::BitMask,
    );
    if matches!(
        node_type,
        NodeType::PointLight2D
            | NodeType::PointLight3D
            | NodeType::SpotLight2D
            | NodeType::SpotLight3D
    ) {
        push(fields, "Light", "range", SceneInspectorValueKind::F32);
    }
    if matches!(node_type, NodeType::SpotLight2D | NodeType::SpotLight3D) {
        push(
            fields,
            "Light",
            "inner_angle_radians",
            SceneInspectorValueKind::F32,
        );
        push(
            fields,
            "Light",
            "outer_angle_radians",
            SceneInspectorValueKind::F32,
        );
    }
}

fn mesh_fields(fields: &mut Vec<SceneInspectorField>, node_type: NodeType) {
    asset_field(fields, "Mesh", "mesh", SceneAssetKind::Mesh);
    asset_field(fields, "Mesh", "model", SceneAssetKind::Model);
    asset_field(fields, "Material", "material", SceneAssetKind::Material);
    push(
        fields,
        "Material",
        "surfaces",
        SceneInspectorValueKind::Array,
    );
    push(fields, "Mesh", "skeleton", SceneInspectorValueKind::NodeRef);
    push(
        fields,
        "Mesh",
        "blend_shape_weights",
        SceneInspectorValueKind::Array,
    );
    push(fields, "Mesh", "flip_x", SceneInspectorValueKind::Bool);
    push(fields, "Mesh", "flip_y", SceneInspectorValueKind::Bool);
    push(fields, "Mesh", "flip_z", SceneInspectorValueKind::Bool);
    push(fields, "Mesh", "meshlets", SceneInspectorValueKind::Bool);
    push(fields, "Mesh", "min_lod", SceneInspectorValueKind::U32);
    push(fields, "Mesh", "max_lod", SceneInspectorValueKind::U32);
    push(
        fields,
        "Mesh",
        "cast_shadows",
        SceneInspectorValueKind::Bool,
    );
    push(
        fields,
        "Mesh",
        "receive_shadows",
        SceneInspectorValueKind::Bool,
    );
    push(fields, "Blend", "blend", SceneInspectorValueKind::Object);
    if node_type == NodeType::MultiMeshInstance3D {
        push(
            fields,
            "Instances",
            "instances",
            SceneInspectorValueKind::Array,
        );
        push(
            fields,
            "Instances",
            "instance_grid",
            SceneInspectorValueKind::Object,
        );
        push(
            fields,
            "Instances",
            "instance_scale",
            SceneInspectorValueKind::F32,
        );
    }
}

fn water_fields(fields: &mut Vec<SceneInspectorField>) {
    push(fields, "Water", "shape", SceneInspectorValueKind::Object);
    push(fields, "Water", "resolution", SceneInspectorValueKind::Vec2);
    push(
        fields,
        "Water",
        "render_resolution",
        SceneInspectorValueKind::Vec2,
    );
    push(
        fields,
        "Water",
        "vertices_per_meter",
        SceneInspectorValueKind::F32,
    );
    push(fields, "Water", "depth", SceneInspectorValueKind::F32);
    push(fields, "Water", "flow", SceneInspectorValueKind::Vec2);
    push(fields, "Water", "wind", SceneInspectorValueKind::Vec2);
    push(
        fields,
        "Water",
        "idle_mode",
        SceneInspectorValueKind::String,
    );
    push(fields, "Water", "wave_speed", SceneInspectorValueKind::F32);
    push(fields, "Water", "wave_scale", SceneInspectorValueKind::F32);
    push(fields, "Water", "wave_length", SceneInspectorValueKind::F32);
    push(
        fields,
        "Physics",
        "collision_layers",
        SceneInspectorValueKind::BitMask,
    );
    push(
        fields,
        "Physics",
        "collision_mask",
        SceneInspectorValueKind::BitMask,
    );
    push(
        fields,
        "Optics",
        "deep_color",
        SceneInspectorValueKind::Vec4,
    );
    push(
        fields,
        "Optics",
        "shallow_color",
        SceneInspectorValueKind::Vec4,
    );
    push(fields, "Optics", "optics", SceneInspectorValueKind::Object);
    push(
        fields,
        "Material",
        "material",
        SceneInspectorValueKind::Object,
    );
    push(fields, "Debug", "debug", SceneInspectorValueKind::Bool);
}

fn physics_body_fields(fields: &mut Vec<SceneInspectorField>, node_type: NodeType) {
    push(fields, "Physics", "enabled", SceneInspectorValueKind::Bool);
    push(
        fields,
        "Physics",
        "collision_layers",
        SceneInspectorValueKind::BitMask,
    );
    push(
        fields,
        "Physics",
        "collision_mask",
        SceneInspectorValueKind::BitMask,
    );
    if matches!(
        node_type,
        NodeType::StaticBody2D
            | NodeType::StaticBody3D
            | NodeType::RigidBody2D
            | NodeType::RigidBody3D
    ) {
        push(fields, "Physics", "friction", SceneInspectorValueKind::F32);
        push(
            fields,
            "Physics",
            "restitution",
            SceneInspectorValueKind::F32,
        );
        push(fields, "Physics", "density", SceneInspectorValueKind::F32);
    }
    if matches!(node_type, NodeType::RigidBody2D | NodeType::RigidBody3D) {
        push(
            fields,
            "Rigid Body",
            "continuous_collision_detection",
            SceneInspectorValueKind::Bool,
        );
        push(fields, "Rigid Body", "mass", SceneInspectorValueKind::F32);
        push(
            fields,
            "Rigid Body",
            "linear_velocity",
            if node_type.is_3d() {
                SceneInspectorValueKind::Vec3
            } else {
                SceneInspectorValueKind::Vec2
            },
        );
        push(
            fields,
            "Rigid Body",
            "angular_velocity",
            if node_type.is_3d() {
                SceneInspectorValueKind::Vec3
            } else {
                SceneInspectorValueKind::F32
            },
        );
        push(
            fields,
            "Rigid Body",
            "gravity_scale",
            SceneInspectorValueKind::F32,
        );
        push(
            fields,
            "Rigid Body",
            "linear_damping",
            SceneInspectorValueKind::F32,
        );
        push(
            fields,
            "Rigid Body",
            "angular_damping",
            SceneInspectorValueKind::F32,
        );
        push(
            fields,
            "Rigid Body",
            "can_sleep",
            SceneInspectorValueKind::Bool,
        );
        if node_type == NodeType::RigidBody2D {
            push(
                fields,
                "Rigid Body",
                "lock_rotation",
                SceneInspectorValueKind::Bool,
            );
        }
    }
}

fn joint_fields(fields: &mut Vec<SceneInspectorField>, node_type: NodeType) {
    push(fields, "Joint", "body_a", SceneInspectorValueKind::NodeRef);
    push(fields, "Joint", "body_b", SceneInspectorValueKind::NodeRef);
    let vec_kind = if node_type.is_3d() {
        SceneInspectorValueKind::Vec3
    } else {
        SceneInspectorValueKind::Vec2
    };
    push(fields, "Joint", "anchor_a", vec_kind);
    push(fields, "Joint", "anchor_b", vec_kind);
    push(fields, "Joint", "enabled", SceneInspectorValueKind::Bool);
    push(
        fields,
        "Joint",
        "collide_connected",
        SceneInspectorValueKind::Bool,
    );
    if node_type == NodeType::DistanceJoint2D {
        push(
            fields,
            "Joint",
            "min_distance",
            SceneInspectorValueKind::F32,
        );
        push(
            fields,
            "Joint",
            "max_distance",
            SceneInspectorValueKind::F32,
        );
    }
    if node_type == NodeType::HingeJoint3D {
        push(fields, "Joint", "axis", SceneInspectorValueKind::Vec3);
    }
}

fn sky_fields(fields: &mut Vec<SceneInspectorField>) {
    push(fields, "Sky", "day_colors", SceneInspectorValueKind::Array);
    push(
        fields,
        "Sky",
        "evening_colors",
        SceneInspectorValueKind::Array,
    );
    push(
        fields,
        "Sky",
        "night_colors",
        SceneInspectorValueKind::Array,
    );
    push(
        fields,
        "Sky",
        "horizon_colors",
        SceneInspectorValueKind::Array,
    );
    push(fields, "Sky", "time_of_day", SceneInspectorValueKind::F32);
    push(fields, "Sky", "time_paused", SceneInspectorValueKind::Bool);
    push(fields, "Sky", "time_scale", SceneInspectorValueKind::F32);
    push(fields, "Sky", "shaders", SceneInspectorValueKind::Object);
    push(fields, "Sky", "active", SceneInspectorValueKind::Bool);
    push(
        fields,
        "Sky",
        "render_layers",
        SceneInspectorValueKind::BitMask,
    );
}
