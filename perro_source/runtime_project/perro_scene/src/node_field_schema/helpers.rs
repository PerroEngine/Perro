use super::*;

pub(super) fn push_default(
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

pub(super) fn push(
    fields: &mut Vec<SceneNodeField>,
    section: &'static str,
    name: &'static str,
    kind: NodeFieldType,
) {
    fields.push(SceneNodeField::new(section, name, kind));
}

pub(super) fn push_f32_default(
    fields: &mut Vec<SceneNodeField>,
    section: &'static str,
    name: &'static str,
    value: f32,
) {
    let mut field = SceneNodeField::new(section, name, NodeFieldType::F32);
    field.default = Some(SceneValue::F32(value));
    fields.push(field);
}

pub(super) fn push_u32_default(
    fields: &mut Vec<SceneNodeField>,
    section: &'static str,
    name: &'static str,
    value: u32,
) {
    let mut field = SceneNodeField::new(section, name, NodeFieldType::U32);
    field.default = Some(SceneValue::I32(value.min(i32::MAX as u32) as i32));
    fields.push(field);
}

pub(super) fn asset_field(
    fields: &mut Vec<SceneNodeField>,
    section: &'static str,
    name: &'static str,
    kind: SceneAssetKind,
) {
    push(fields, section, name, NodeFieldType::Asset(kind));
}

pub(super) fn texture_field(
    fields: &mut Vec<SceneNodeField>,
    section: &'static str,
    name: &'static str,
) {
    asset_field(fields, section, name, SceneAssetKind::Texture);
}

pub(super) fn decal_fields(fields: &mut Vec<SceneNodeField>) {
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

pub(super) fn sprite_fields(fields: &mut Vec<SceneNodeField>, section: &'static str) {
    texture_field(fields, section, "texture");
    push(fields, section, "texture_region", NodeFieldType::Vec4);
    push(fields, section, "flip_x", NodeFieldType::Bool);
    push(fields, section, "flip_y", NodeFieldType::Bool);
}

pub(super) fn sprite_world_fields(fields: &mut Vec<SceneNodeField>, section: &'static str) {
    sprite_fields(fields, section);
    push(fields, section, "size", NodeFieldType::Vec2);
    push(fields, section, "modulate", NodeFieldType::Color);
}

pub(super) fn label_world_fields(fields: &mut Vec<SceneNodeField>, section: &'static str) {
    push(fields, section, "text", NodeFieldType::String);
    push(fields, section, "size", NodeFieldType::Vec2);
    push(fields, section, "lock_orientation", NodeFieldType::Bool);
    push(fields, section, "backface_cull", NodeFieldType::Bool);
    push(
        fields,
        section,
        "visible_through_objects",
        NodeFieldType::Bool,
    );
    push(fields, section, "backdrop_color", NodeFieldType::Color);
    push(fields, section, "corner_radii", NodeFieldType::Vec4);
    push(fields, section, "padding", NodeFieldType::Vec4);
    push(fields, section, "color", NodeFieldType::Color);
    push(fields, section, "font_size", NodeFieldType::F32);
    push(fields, section, "font", NodeFieldType::String);
    push(
        fields,
        section,
        "h_align",
        NodeFieldType::enumeration(UI_TEXT_ALIGN_OPTIONS),
    );
    push(
        fields,
        section,
        "v_align",
        NodeFieldType::enumeration(UI_TEXT_ALIGN_OPTIONS),
    );
}

pub(super) fn button_2d_fields(fields: &mut Vec<SceneNodeField>, section: &'static str) {
    push(fields, section, "size", NodeFieldType::Vec2);
    push(fields, section, "input_enabled", NodeFieldType::Bool);
}

pub(super) fn animated_image_fields(fields: &mut Vec<SceneNodeField>, section: &'static str) {
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

pub(super) fn video_player_fields(
    fields: &mut Vec<SceneNodeField>,
    section: &'static str,
    world_size: bool,
    ui_image: bool,
) {
    push(fields, section, "source", NodeFieldType::String);
    push(fields, section, "playing", NodeFieldType::Bool);
    push(fields, section, "looping", NodeFieldType::Bool);
    push(fields, section, "fps_scale", NodeFieldType::F32);
    push(fields, section, "volume", NodeFieldType::F32);
    if world_size {
        push(fields, section, "size", NodeFieldType::Vec2);
        push(fields, section, "tint", NodeFieldType::Color);
        push(fields, section, "flip_x", NodeFieldType::Bool);
        push(fields, section, "flip_y", NodeFieldType::Bool);
    }
    if ui_image {
        push(fields, section, "tint", NodeFieldType::Color);
        push(
            fields,
            section,
            "scale_mode",
            NodeFieldType::enumeration(CAMERA_STREAM_ASPECT_MODE_OPTIONS),
        );
        push(fields, section, "aspect_ratio", NodeFieldType::F32);
        push(fields, section, "corner_radius", NodeFieldType::F32);
    }
}

pub(super) fn ui_style_fields(
    fields: &mut Vec<SceneNodeField>,
    section: &'static str,
    prefix: &'static str,
) {
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

pub(super) fn particle_fields(
    fields: &mut Vec<SceneNodeField>,
    section: &'static str,
    is_3d: bool,
) {
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

pub(super) fn light_fields(fields: &mut Vec<SceneNodeField>, node_type: NodeType) {
    push(fields, "Light", "color", NodeFieldType::Color);
    push(fields, "Light", "intensity", NodeFieldType::F32);
    push(fields, "Light", "cast_shadows", NodeFieldType::Bool);
    if matches!(
        node_type,
        NodeType::RayLight2D | NodeType::PointLight2D | NodeType::SpotLight2D
    ) {
        push_f32_default(fields, "Shadow", "shadow_softness", 0.0);
        push_u32_default(fields, "Shadow", "shadow_samples", 8);
    }
    if matches!(
        node_type,
        NodeType::RayLight3D | NodeType::PointLight3D | NodeType::SpotLight3D
    ) {
        push_f32_default(fields, "Shadow", "shadow_strength", 0.82);
        push_f32_default(fields, "Shadow", "shadow_depth_bias", 0.00003);
        push_f32_default(fields, "Shadow", "shadow_normal_bias", 0.005);
        push(
            fields,
            "Shadow",
            "shadow",
            NodeFieldType::object(vec![
                NodeFieldDef::new("strength", NodeFieldType::F32)
                    .with_default(SceneValue::F32(0.82)),
                NodeFieldDef::new("depth_bias", NodeFieldType::F32)
                    .with_default(SceneValue::F32(0.00003)),
                NodeFieldDef::new("normal_bias", NodeFieldType::F32)
                    .with_default(SceneValue::F32(0.005)),
            ]),
        );
    }
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

pub(super) fn mesh_fields(fields: &mut Vec<SceneNodeField>, node_type: NodeType) {
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

pub(super) fn water_fields(fields: &mut Vec<SceneNodeField>) {
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

pub(super) fn physics_body_fields(fields: &mut Vec<SceneNodeField>, node_type: NodeType) {
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

pub(super) fn joint_fields(fields: &mut Vec<SceneNodeField>, node_type: NodeType) {
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

pub(super) fn sky_fields(fields: &mut Vec<SceneNodeField>) {
    push(fields, "Sky", "palette", NodeFieldType::object(Vec::new()));
    push(
        fields,
        "Sky",
        "environment",
        NodeFieldType::object(Vec::new()),
    );
    push(fields, "Sky", "time_of_day", NodeFieldType::F32);
    push(fields, "Sky", "time_paused", NodeFieldType::Bool);
    push(fields, "Sky", "time_scale", NodeFieldType::F32);
    push(fields, "Sky", "shaders", NodeFieldType::object(Vec::new()));
    push(fields, "Sky", "active", NodeFieldType::Bool);
    push(fields, "Sky", "render_layers", NodeFieldType::BitMask);
}
