use super::*;

pub(super) fn build_scene_node_fields(node_type: NodeType) -> Vec<SceneNodeField> {
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
    cached_scene_node_fields(node_type)
        .iter()
        .filter_map(|field| {
            field
                .default
                .clone()
                .map(|value| (SceneFieldName::from_name(field.name.to_string()), value))
        })
        .collect()
}

pub fn scene_node_asset_fields(node_type: NodeType) -> Vec<SceneNodeField> {
    cached_scene_node_fields(node_type)
        .iter()
        .filter(|field| matches!(field.ty, NodeFieldType::Asset(_)))
        .cloned()
        .collect()
}

pub fn scene_node_field(node_type: NodeType, name: &str) -> Option<SceneNodeField> {
    cached_scene_node_fields(node_type)
        .iter()
        .find(|field| field.name == name || field.aliases.contains(&name))
        .cloned()
}

pub(super) fn push_base_fields(fields: &mut Vec<SceneNodeField>, node_type: NodeType) {
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
        let mut top_level = SceneNodeField::new("Transform", "top_level", NodeFieldType::Bool);
        top_level.default = Some(SceneValue::Bool(false));
        fields.push(top_level);
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
        let mut top_level = SceneNodeField::new("Transform", "top_level", NodeFieldType::Bool);
        top_level.default = Some(SceneValue::Bool(false));
        fields.push(top_level);
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

pub(super) fn push_node_fields(fields: &mut Vec<SceneNodeField>, node_type: NodeType) {
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
        NodeType::UiViewport => {
            push(fields, "Viewport", "resolution", NodeFieldType::Vec2);
            push(fields, "Viewport", "aspect_ratio", NodeFieldType::F32);
            push(
                fields,
                "Viewport",
                "aspect_mode",
                NodeFieldType::enumeration(CAMERA_STREAM_ASPECT_MODE_OPTIONS),
            );
            push(fields, "Viewport", "view_position", NodeFieldType::Vec3);
            push(fields, "Viewport", "view_rotation", NodeFieldType::Quat);
            push(
                fields,
                "Viewport 2D",
                "view_2d_position",
                NodeFieldType::Vec2,
            );
            push(
                fields,
                "Viewport 2D",
                "view_2d_rotation",
                NodeFieldType::F32,
            );
            push(fields, "Viewport 2D", "view_2d_zoom", NodeFieldType::F32);
            push(
                fields,
                "Viewport Camera",
                "projection",
                NodeFieldType::enum_submenu(CAMERA_PROJECTION_SUBMENUS),
            );
            push(
                fields,
                "Viewport Camera",
                "perspective_fov_y_degrees",
                NodeFieldType::F32,
            );
            push(
                fields,
                "Viewport Camera",
                "perspective_near",
                NodeFieldType::F32,
            );
            push(
                fields,
                "Viewport Camera",
                "perspective_far",
                NodeFieldType::F32,
            );
            push(
                fields,
                "Viewport Camera",
                "orthographic_size",
                NodeFieldType::F32,
            );
            push(
                fields,
                "Viewport Camera",
                "orthographic_near",
                NodeFieldType::F32,
            );
            push(
                fields,
                "Viewport Camera",
                "orthographic_far",
                NodeFieldType::F32,
            );
            push(
                fields,
                "Viewport",
                "post_processing",
                NodeFieldType::object(Vec::new()),
            );
            push(fields, "Viewport", "tint", NodeFieldType::Color);
            push(fields, "Viewport", "background", NodeFieldType::Color);
            push(fields, "Viewport", "corner_radius", NodeFieldType::F32);
            push(fields, "Viewport", "enabled", NodeFieldType::Bool);
            push(
                fields,
                "Viewport",
                "suspend_when_hidden",
                NodeFieldType::Bool,
            );
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
        NodeType::Webcam => {
            push(fields, "Webcam", "slot", NodeFieldType::String);
            push(fields, "Webcam", "resolution", NodeFieldType::Vec2);
            push(fields, "Webcam", "width", NodeFieldType::U32);
            push(fields, "Webcam", "height", NodeFieldType::U32);
            push(fields, "Webcam", "fps", NodeFieldType::U32);
            push(fields, "Webcam", "mirror", NodeFieldType::Bool);
            push(fields, "Webcam", "cpu_frames", NodeFieldType::Bool);
            push(fields, "Webcam", "enabled", NodeFieldType::Bool);
        }
        NodeType::Sprite2D => sprite_fields(fields, "Sprite"),
        NodeType::VideoPlayer2D => video_player_fields(fields, "Video", true, false),
        NodeType::Label2D => label_world_fields(fields, "Label"),
        NodeType::Button2D => button_2d_fields(fields, "Button"),
        NodeType::ImageButton2D => {
            button_2d_fields(fields, "Button");
            texture_field(fields, "Image", "texture");
            push(fields, "Image", "texture_region", NodeFieldType::Vec4);
        }
        NodeType::NineSliceButton2D => {
            button_2d_fields(fields, "Nine Slice Button");
            texture_field(fields, "Nine Slice", "texture");
            push(fields, "Nine Slice", "texture_region", NodeFieldType::Vec4);
            push(fields, "Nine Slice", "margins", NodeFieldType::Vec4);
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
        NodeType::Sprite3D => sprite_world_fields(fields, "Sprite"),
        NodeType::VideoPlayer3D => video_player_fields(fields, "Video", true, false),
        NodeType::Label3D => label_world_fields(fields, "Label"),
        NodeType::Decal3D => decal_fields(fields),
        NodeType::Skeleton2D | NodeType::Skeleton3D => {
            asset_field(fields, "Skeleton", "skeleton", SceneAssetKind::Skeleton);
            // Per-bone pose overrides: bones = { Name = { position/rotation/
            // rotation_deg/scale } }. Free-form object keyed by bone name.
            push(
                fields,
                "Skeleton",
                "bones",
                NodeFieldType::object(Vec::new()),
            );
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
        NodeType::UiImage
        | NodeType::UiImageButton
        | NodeType::UiNineSliceButton
        | NodeType::UiNineSlice => {
            texture_field(fields, "Image", "texture");
            push(fields, "Image", "texture_region", NodeFieldType::Vec4);
            if matches!(
                node_type,
                NodeType::UiNineSliceButton | NodeType::UiNineSlice
            ) {
                push(fields, "Image", "margins", NodeFieldType::Vec4);
            }
        }
        NodeType::UiAnimatedImage => animated_image_fields(fields, "Image"),
        NodeType::UiVideoPlayer => video_player_fields(fields, "Video", false, true),
        NodeType::UiPanel
        | NodeType::UiProgressBar
        | NodeType::UiButton
        | NodeType::UiDropdown
        | NodeType::UiCheckbox
        | NodeType::UiColorPicker
        | NodeType::UiTextBox
        | NodeType::UiTextBlock => {
            asset_field(fields, "Style", "style", SceneAssetKind::UiStyle);
            ui_style_fields(fields, "Style", "");
            if matches!(node_type, NodeType::UiProgressBar) {
                fields.push(
                    SceneNodeField::new("Progress", "value", NodeFieldType::F32)
                        .with_default(SceneValue::F32(0.0)),
                );
                push(
                    fields,
                    "Background",
                    "background_color",
                    NodeFieldType::Color,
                );
                push(
                    fields,
                    "Background",
                    "background_rounding",
                    NodeFieldType::F32,
                );
                asset_field(
                    fields,
                    "Background",
                    "background_style",
                    SceneAssetKind::UiStyle,
                );
                ui_style_fields(fields, "Background", "background_");
                push(fields, "Fill", "fill_color", NodeFieldType::Color);
                push(fields, "Fill", "fill_rounding", NodeFieldType::F32);
                asset_field(fields, "Fill", "fill_style", SceneAssetKind::UiStyle);
                ui_style_fields(fields, "Fill", "fill_");
            }
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
                push(fields, "Popup", "popup_size", NodeFieldType::Vec2);
                push(fields, "Popup", "popup_offset", NodeFieldType::Vec2);
                push(fields, "Popup", "popup_direction", NodeFieldType::String);
                push(fields, "Popup", "open_animation", NodeFieldType::String);
                push(
                    fields,
                    "Popup",
                    "open_animation_duration",
                    NodeFieldType::F32,
                );
            }
            if matches!(node_type, NodeType::UiCheckbox) {
                push(fields, "State", "checked", NodeFieldType::Bool);
            }
            if matches!(node_type, NodeType::UiColorPicker) {
                fields.push(
                    SceneNodeField::new("State", "color", NodeFieldType::Color).with_default(
                        SceneValue::Vec4 {
                            x: 1.0,
                            y: 1.0,
                            z: 1.0,
                            w: 1.0,
                        },
                    ),
                );
                push(fields, "State", "popup_open", NodeFieldType::Bool);
                push(
                    fields,
                    "Picker",
                    "picker_mode",
                    NodeFieldType::enumeration(UI_COLOR_PICKER_MODE_OPTIONS),
                );
                for name in ["show_selector", "show_hex", "show_rgba", "show_hsl"] {
                    fields.push(
                        SceneNodeField::new("Picker", name, NodeFieldType::Bool)
                            .with_default(SceneValue::Bool(true)),
                    );
                }
                fields.push(
                    SceneNodeField::new("Picker", "popup_size", NodeFieldType::Vec2)
                        .with_default(SceneValue::Vec2 { x: 360.0, y: 344.0 }),
                );
                fields.push(
                    SceneNodeField::new("Picker", "wheel_radius", NodeFieldType::F32)
                        .with_default(SceneValue::F32(88.0)),
                );
            }
            if matches!(node_type, NodeType::UiTextBox | NodeType::UiTextBlock) {
                asset_field(fields, "Style", "focused_style", SceneAssetKind::UiStyle);
                ui_style_fields(fields, "Focus", "focused_");
                push(fields, "Text", "text", NodeFieldType::String);
                push(fields, "Text", "placeholder", NodeFieldType::String);
                push(fields, "Text", "font", NodeFieldType::String);
            }
        }
        NodeType::UiLabel => {
            fields.push(
                SceneNodeField::new("Text", "text", NodeFieldType::String)
                    .with_default(SceneValue::Str(Cow::Borrowed("New Node"))),
            );
            push(fields, "Text", "color", NodeFieldType::Color);
            push(fields, "Text", "text_size_ratio", NodeFieldType::F32);
            push(fields, "Text", "font", NodeFieldType::String);
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
        NodeType::AudioMask2D | NodeType::AudioMask3D => {
            push(fields, "Audio", "active", NodeFieldType::Bool);
        }
        NodeType::AudioEffectZone2D | NodeType::AudioEffectZone3D => {
            push(fields, "Audio", "active", NodeFieldType::Bool);
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
            push(fields, "Audio", "active", NodeFieldType::Bool);
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
