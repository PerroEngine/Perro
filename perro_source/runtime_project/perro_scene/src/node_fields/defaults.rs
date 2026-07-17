use super::*;

pub fn resolve_node_field(node_type_name: &str, field: &str) -> Option<NodeField> {
    let node_type = NodeType::from_str(node_type_name).ok()?;
    resolve_node_field_for_type(node_type, field)
}

pub fn resolve_scene_node_field(node_type_name: &str, field: &SceneFieldName) -> Option<NodeField> {
    let node_type = NodeType::from_str(node_type_name).ok()?;
    resolve_scene_node_field_for_type(node_type, field)
        .or_else(|| resolve_node_field_for_type(node_type, field.as_ref()))
}

pub fn default_scene_field_value_by_name(node_type: NodeType, field: &str) -> Option<SceneValue> {
    let field = SceneFieldName::from_borrowed(field)?;
    default_scene_field_value(node_type, &field)
}

pub fn default_scene_field_value(
    node_type: NodeType,
    field: &SceneFieldName,
) -> Option<SceneValue> {
    let resolved = resolve_scene_node_field_for_type(node_type, field)
        .or_else(|| resolve_node_field_for_type(node_type, field.as_ref()))?;
    default_node_field_value(resolved)
}

pub fn default_node_field_value(field: NodeField) -> Option<SceneValue> {
    match field {
        NodeField::Node2D(field) => default_node_2d_field_value(field),
        NodeField::Node3D(field) => default_node_3d_field_value(field),
        NodeField::UiNode(field) => default_ui_node_field_value(field),
        NodeField::Camera2D(field) => default_camera_2d_field_value(field),
        NodeField::Camera3D(field) => default_camera_3d_field_value(field),
        NodeField::MeshInstance3D(field) => default_mesh_instance_3d_field_value(field),
        NodeField::StaticBody2D(field) => default_static_body_2d_field_value(field),
        NodeField::StaticBody3D(field) => default_static_body_3d_field_value(field),
        NodeField::RigidBody2D(field) => default_rigid_body_2d_field_value(field),
        NodeField::RigidBody3D(field) => default_rigid_body_3d_field_value(field),
        NodeField::CharacterBody2D(field) | NodeField::CharacterBody3D(field) => {
            default_character_body_field_value(field)
        }
        NodeField::Area2D(field) => default_area_2d_field_value(field),
        NodeField::Area3D(field) => default_area_3d_field_value(field),
        NodeField::PhysicsForceEmitter2D(field) => {
            default_physics_force_emitter_2d_field_value(field)
        }
        NodeField::PhysicsForceEmitter3D(field) => {
            default_physics_force_emitter_3d_field_value(field)
        }
        _ => None,
    }
}

pub(super) fn default_node_2d_field_value(field: Node2DField) -> Option<SceneValue> {
    let node = Node2D::new();
    Some(match field {
        Node2DField::Position => vec2_value(node.transform.position),
        Node2DField::Rotation => SceneValue::F32(node.transform.rotation),
        Node2DField::Scale => vec2_value(node.transform.scale),
        Node2DField::Visible => SceneValue::Bool(node.visible),
        Node2DField::Modulate => color_value(node.modulate.modulate),
        Node2DField::SelfModulate => color_value(node.modulate.self_modulate),
        Node2DField::ChildrenModulate => color_value(node.modulate.children_modulate),
        Node2DField::ZIndex => SceneValue::I32(node.z_index),
        Node2DField::RenderLayers => bit_mask_value(node.render_layers),
    })
}

pub(super) fn default_node_3d_field_value(field: Node3DField) -> Option<SceneValue> {
    let node = Node3D::new();
    Some(match field {
        Node3DField::Position => vec3_value(node.transform.position),
        Node3DField::Rotation => quat_value(node.transform.rotation),
        Node3DField::Scale => vec3_value(node.transform.scale),
        Node3DField::Visible => SceneValue::Bool(node.visible),
        Node3DField::Modulate => color_value(node.modulate.modulate),
        Node3DField::SelfModulate => color_value(node.modulate.self_modulate),
        Node3DField::ChildrenModulate => color_value(node.modulate.children_modulate),
        Node3DField::RenderLayers => bit_mask_value(node.render_layers),
    })
}

pub(super) fn default_ui_node_field_value(field: UiNodeField) -> Option<SceneValue> {
    let node = UiNode::new();
    Some(match field {
        UiNodeField::Position => ui_vec2_ratio_value(node.transform.position),
        UiNodeField::Scale => vec2_value(node.transform.scale),
        UiNodeField::Rotation => SceneValue::F32(node.transform.rotation),
        UiNodeField::Visible => SceneValue::Bool(node.visible),
        UiNodeField::Modulate => color_value(node.modulate.modulate),
        UiNodeField::SelfModulate => color_value(node.modulate.self_modulate),
        UiNodeField::ChildrenModulate => color_value(node.modulate.children_modulate),
        UiNodeField::InputEnabled => SceneValue::Bool(node.input_enabled),
        UiNodeField::ClipChildren => SceneValue::Bool(node.clip_children),
        UiNodeField::ZIndex => SceneValue::I32(node.layout.z_index),
    })
}

pub(super) fn default_camera_2d_field_value(field: Camera2DField) -> Option<SceneValue> {
    let node = Camera2D::default();
    Some(match field {
        Camera2DField::Zoom => SceneValue::F32(node.zoom),
        Camera2DField::RenderMask => bit_mask_value(node.render_mask),
        Camera2DField::PostProcessing => SceneValue::Object(Default::default()),
        Camera2DField::AudioOptions => SceneValue::Object(Default::default()),
        Camera2DField::AudioMask => bit_mask_value(BitMask::NONE),
        Camera2DField::Active => SceneValue::Bool(node.active),
    })
}

pub(super) fn default_camera_3d_field_value(field: Camera3DField) -> Option<SceneValue> {
    let node = Camera3D::default();
    Some(match field {
        Camera3DField::Zoom => SceneValue::F32(1.0),
        Camera3DField::RenderMask => bit_mask_value(node.render_mask),
        Camera3DField::Projection => SceneValue::Key("perspective".to_string().into()),
        Camera3DField::PerspectiveFovYDegrees => SceneValue::F32(60.0),
        Camera3DField::PerspectiveNear => SceneValue::F32(0.1),
        Camera3DField::PerspectiveFar => SceneValue::F32(1_000_000.0),
        Camera3DField::OrthographicSize => SceneValue::F32(10.0),
        Camera3DField::OrthographicNear => SceneValue::F32(0.1),
        Camera3DField::OrthographicFar => SceneValue::F32(1_000_000.0),
        Camera3DField::FrustumLeft => SceneValue::F32(-1.0),
        Camera3DField::FrustumRight => SceneValue::F32(1.0),
        Camera3DField::FrustumBottom => SceneValue::F32(-1.0),
        Camera3DField::FrustumTop => SceneValue::F32(1.0),
        Camera3DField::FrustumNear => SceneValue::F32(0.1),
        Camera3DField::FrustumFar => SceneValue::F32(1_000_000.0),
        Camera3DField::PostProcessing => SceneValue::Object(Default::default()),
        Camera3DField::AudioOptions => SceneValue::Object(Default::default()),
        Camera3DField::AudioMask => bit_mask_value(BitMask::NONE),
        Camera3DField::Active => SceneValue::Bool(node.active),
    })
}

pub(super) fn default_mesh_instance_3d_field_value(
    field: MeshInstance3DField,
) -> Option<SceneValue> {
    let node = MeshInstance3D::new();
    Some(match field {
        MeshInstance3DField::Mesh | MeshInstance3DField::Material | MeshInstance3DField::Model => {
            SceneValue::Str(Default::default())
        }
        MeshInstance3DField::Surfaces | MeshInstance3DField::BlendShapeWeights => {
            SceneValue::Array(Default::default())
        }
        MeshInstance3DField::Skeleton => SceneValue::Key("null".to_string().into()),
        MeshInstance3DField::FlipX => SceneValue::Bool(node.flip_x),
        MeshInstance3DField::FlipY => SceneValue::Bool(node.flip_y),
        MeshInstance3DField::FlipZ => SceneValue::Bool(node.flip_z),
        MeshInstance3DField::InstanceGrid => SceneValue::Object(Default::default()),
        MeshInstance3DField::Meshlets => SceneValue::Bool(false),
        MeshInstance3DField::MinLod => SceneValue::I32(node.lod.min_lod as i32),
        MeshInstance3DField::MaxLod => SceneValue::I32(node.lod.max_lod as i32),
        MeshInstance3DField::CastShadows => SceneValue::Bool(node.cast_shadows),
        MeshInstance3DField::ReceiveShadows => SceneValue::Bool(node.receive_shadows),
        MeshInstance3DField::Blend => mesh_blend_value(MeshBlendOptions::new()),
        MeshInstance3DField::BlendEnabled => SceneValue::Bool(node.blend.enabled),
        MeshInstance3DField::BlendScreen => SceneValue::Bool(node.blend.screen_blending),
        MeshInstance3DField::BlendNormals => SceneValue::Bool(node.blend.normal_blending),
        MeshInstance3DField::BlendLayers => bit_mask_value(node.blend.blend_layers),
        MeshInstance3DField::BlendMask => bit_mask_value(node.blend.blend_mask),
        MeshInstance3DField::BlendDistance => SceneValue::F32(node.blend.distance),
        MeshInstance3DField::BlendMinDistance => SceneValue::F32(node.blend.min_distance),
    })
}

pub(super) fn default_character_body_field_value(field: CharacterBodyField) -> Option<SceneValue> {
    // 2d + 3d char defaults match; use 3d node as source
    let node = CharacterBody3D::default();
    Some(match field {
        CharacterBodyField::Enabled => SceneValue::Bool(node.enabled),
        CharacterBodyField::CollisionLayers => bit_mask_value(node.collision_layers),
        CharacterBodyField::CollisionMask => bit_mask_value(node.collision_mask),
        CharacterBodyField::Friction => SceneValue::F32(node.friction),
        CharacterBodyField::Restitution => SceneValue::F32(node.restitution),
        CharacterBodyField::Density => SceneValue::F32(node.density),
    })
}

pub(super) fn default_static_body_2d_field_value(field: StaticBody2DField) -> Option<SceneValue> {
    let node = StaticBody2D::default();
    Some(match field {
        StaticBody2DField::Enabled => SceneValue::Bool(node.enabled),
        StaticBody2DField::CollisionLayers => bit_mask_value(node.collision_layers),
        StaticBody2DField::CollisionMask => bit_mask_value(node.collision_mask),
        StaticBody2DField::Friction => SceneValue::F32(node.friction),
        StaticBody2DField::Restitution => SceneValue::F32(node.restitution),
        StaticBody2DField::Density => SceneValue::F32(node.density),
    })
}

pub(super) fn default_static_body_3d_field_value(field: StaticBody3DField) -> Option<SceneValue> {
    let node = StaticBody3D::default();
    Some(match field {
        StaticBody3DField::Enabled => SceneValue::Bool(node.enabled),
        StaticBody3DField::CollisionLayers => bit_mask_value(node.collision_layers),
        StaticBody3DField::CollisionMask => bit_mask_value(node.collision_mask),
        StaticBody3DField::Friction => SceneValue::F32(node.friction),
        StaticBody3DField::Restitution => SceneValue::F32(node.restitution),
        StaticBody3DField::Density => SceneValue::F32(node.density),
    })
}

pub(super) fn default_rigid_body_2d_field_value(field: RigidBody2DField) -> Option<SceneValue> {
    let node = RigidBody2D::default();
    Some(match field {
        RigidBody2DField::Enabled => SceneValue::Bool(node.enabled),
        RigidBody2DField::CollisionLayers => bit_mask_value(node.collision_layers),
        RigidBody2DField::CollisionMask => bit_mask_value(node.collision_mask),
        RigidBody2DField::ContinuousCollisionDetection => {
            SceneValue::Bool(node.continuous_collision_detection)
        }
        RigidBody2DField::Mass => SceneValue::F32(node.mass),
        RigidBody2DField::LinearVelocity => vec2_value(node.linear_velocity),
        RigidBody2DField::AngularVelocity => SceneValue::F32(node.angular_velocity),
        RigidBody2DField::GravityScale => SceneValue::F32(node.gravity_scale),
        RigidBody2DField::LinearDamping => SceneValue::F32(node.linear_damping),
        RigidBody2DField::AngularDamping => SceneValue::F32(node.angular_damping),
        RigidBody2DField::CanSleep => SceneValue::Bool(node.can_sleep),
        RigidBody2DField::LockRotation => SceneValue::Bool(node.lock_rotation),
        RigidBody2DField::Friction => SceneValue::F32(node.friction),
        RigidBody2DField::Restitution => SceneValue::F32(node.restitution),
        RigidBody2DField::Density => SceneValue::F32(node.density),
    })
}

pub(super) fn default_rigid_body_3d_field_value(field: RigidBody3DField) -> Option<SceneValue> {
    let node = RigidBody3D::default();
    Some(match field {
        RigidBody3DField::Enabled => SceneValue::Bool(node.enabled),
        RigidBody3DField::CollisionLayers => bit_mask_value(node.collision_layers),
        RigidBody3DField::CollisionMask => bit_mask_value(node.collision_mask),
        RigidBody3DField::ContinuousCollisionDetection => {
            SceneValue::Bool(node.continuous_collision_detection)
        }
        RigidBody3DField::Mass => SceneValue::F32(node.mass),
        RigidBody3DField::LinearVelocity => vec3_value(node.linear_velocity),
        RigidBody3DField::AngularVelocity => vec3_value(node.angular_velocity),
        RigidBody3DField::GravityScale => SceneValue::F32(node.gravity_scale),
        RigidBody3DField::LinearDamping => SceneValue::F32(node.linear_damping),
        RigidBody3DField::AngularDamping => SceneValue::F32(node.angular_damping),
        RigidBody3DField::CanSleep => SceneValue::Bool(node.can_sleep),
        RigidBody3DField::Friction => SceneValue::F32(node.friction),
        RigidBody3DField::Restitution => SceneValue::F32(node.restitution),
        RigidBody3DField::Density => SceneValue::F32(node.density),
    })
}

pub(super) fn default_area_2d_field_value(field: Area2DField) -> Option<SceneValue> {
    let node = Area2D::default();
    Some(match field {
        Area2DField::Enabled => SceneValue::Bool(node.enabled),
        Area2DField::CollisionLayers => bit_mask_value(node.collision_layers),
        Area2DField::CollisionMask => bit_mask_value(node.collision_mask),
    })
}

pub(super) fn default_area_3d_field_value(field: Area3DField) -> Option<SceneValue> {
    let node = Area3D::default();
    Some(match field {
        Area3DField::Enabled => SceneValue::Bool(node.enabled),
        Area3DField::CollisionLayers => bit_mask_value(node.collision_layers),
        Area3DField::CollisionMask => bit_mask_value(node.collision_mask),
    })
}

pub(super) fn default_physics_force_emitter_2d_field_value(
    field: PhysicsForceEmitterField,
) -> Option<SceneValue> {
    let node = PhysicsForceEmitter2D::default();
    default_physics_force_emitter_field_value(
        field,
        node.enabled,
        node.radius,
        node.strength,
        node.duration,
        node.pulse,
        node.falloff,
        node.affect_bodies,
        node.affect_water,
        node.collision_layers,
        node.collision_mask,
    )
}

pub(super) fn default_physics_force_emitter_3d_field_value(
    field: PhysicsForceEmitterField,
) -> Option<SceneValue> {
    let node = PhysicsForceEmitter3D::default();
    default_physics_force_emitter_field_value(
        field,
        node.enabled,
        node.radius,
        node.strength,
        node.duration,
        node.pulse,
        node.falloff,
        node.affect_bodies,
        node.affect_water,
        node.collision_layers,
        node.collision_mask,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn default_physics_force_emitter_field_value(
    field: PhysicsForceEmitterField,
    enabled: bool,
    radius: f32,
    strength: f32,
    duration: f32,
    pulse: bool,
    falloff: f32,
    affect_bodies: bool,
    affect_water: bool,
    collision_layers: BitMask,
    collision_mask: BitMask,
) -> Option<SceneValue> {
    Some(match field {
        PhysicsForceEmitterField::Enabled => SceneValue::Bool(enabled),
        PhysicsForceEmitterField::Profile => SceneValue::Object(Default::default()),
        PhysicsForceEmitterField::Radius => SceneValue::F32(radius),
        PhysicsForceEmitterField::Strength => SceneValue::F32(strength),
        PhysicsForceEmitterField::Duration => SceneValue::F32(duration),
        PhysicsForceEmitterField::Pulse => SceneValue::Bool(pulse),
        PhysicsForceEmitterField::Falloff => SceneValue::F32(falloff),
        PhysicsForceEmitterField::AffectBodies => SceneValue::Bool(affect_bodies),
        PhysicsForceEmitterField::AffectWater => SceneValue::Bool(affect_water),
        PhysicsForceEmitterField::CollisionLayers => bit_mask_value(collision_layers),
        PhysicsForceEmitterField::CollisionMask => bit_mask_value(collision_mask),
        PhysicsForceEmitterField::Vectors => SceneValue::Array(Default::default()),
    })
}

pub(super) fn vec2_value(value: Vector2) -> SceneValue {
    SceneValue::Vec2 {
        x: value.x,
        y: value.y,
    }
}

pub(super) fn vec3_value(value: Vector3) -> SceneValue {
    SceneValue::Vec3 {
        x: value.x,
        y: value.y,
        z: value.z,
    }
}

pub(super) fn quat_value(value: Quaternion) -> SceneValue {
    SceneValue::Vec4 {
        x: value.x,
        y: value.y,
        z: value.z,
        w: value.w,
    }
}

pub(super) fn color_value(value: Color) -> SceneValue {
    let [r, g, b, a] = value.to_rgba();
    SceneValue::Vec4 {
        x: r,
        y: g,
        z: b,
        w: a,
    }
}

pub(super) fn bit_mask_value(value: BitMask) -> SceneValue {
    SceneValue::I32(value.bits() as i32)
}

pub(super) fn mesh_blend_value(value: MeshBlendOptions) -> SceneValue {
    SceneValue::Object(std::borrow::Cow::Owned(vec![
        (SceneFieldName::Enabled, SceneValue::Bool(value.enabled)),
        (
            SceneFieldName::BlendScreen,
            SceneValue::Bool(value.screen_blending),
        ),
        (
            SceneFieldName::BlendNormals,
            SceneValue::Bool(value.normal_blending),
        ),
        (
            SceneFieldName::BlendLayers,
            bit_mask_value(value.blend_layers),
        ),
        (SceneFieldName::BlendMask, bit_mask_value(value.blend_mask)),
        (
            SceneFieldName::BlendDistance,
            SceneValue::F32(value.distance),
        ),
        (
            SceneFieldName::BlendMinDistance,
            SceneValue::F32(value.min_distance),
        ),
    ]))
}

pub(super) fn ui_vec2_ratio_value(value: UiVector2) -> SceneValue {
    vec2_value(Vector2::new(
        ui_unit_ratio_value(value.x),
        ui_unit_ratio_value(value.y),
    ))
}

pub(super) fn ui_unit_ratio_value(value: UiUnit) -> f32 {
    match value {
        UiUnit::Pixels(value) => value,
        UiUnit::Percent(value) => value * 0.01,
    }
}
