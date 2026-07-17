use super::super::*;

pub(in super::super) fn resolve_scene_joint2d_common(
    field: &SceneFieldName,
) -> Option<Joint2DField> {
    match field {
        SceneFieldName::BodyA => Some(Joint2DField::BodyA),
        SceneFieldName::BodyB => Some(Joint2DField::BodyB),
        SceneFieldName::AnchorA => Some(Joint2DField::AnchorA),
        SceneFieldName::AnchorB => Some(Joint2DField::AnchorB),
        SceneFieldName::Enabled => Some(Joint2DField::Enabled),
        SceneFieldName::CollideConnected => Some(Joint2DField::CollideConnected),
        _ => None,
    }
}

pub(in super::super) fn resolve_scene_joint3d_common(
    field: &SceneFieldName,
) -> Option<Joint3DField> {
    match field {
        SceneFieldName::BodyA => Some(Joint3DField::BodyA),
        SceneFieldName::BodyB => Some(Joint3DField::BodyB),
        SceneFieldName::AnchorA => Some(Joint3DField::AnchorA),
        SceneFieldName::AnchorB => Some(Joint3DField::AnchorB),
        SceneFieldName::Enabled => Some(Joint3DField::Enabled),
        SceneFieldName::CollideConnected => Some(Joint3DField::CollideConnected),
        _ => None,
    }
}

pub(in super::super) fn resolve_scene_light2d_common(
    field: &SceneFieldName,
) -> Option<Light2DField> {
    match field {
        SceneFieldName::Color => Some(Light2DField::Color),
        SceneFieldName::Intensity => Some(Light2DField::Intensity),
        SceneFieldName::CastShadows => Some(Light2DField::CastShadows),
        SceneFieldName::ShadowSoftness => Some(Light2DField::ShadowSoftness),
        SceneFieldName::ShadowSamples => Some(Light2DField::ShadowSamples),
        SceneFieldName::Active => Some(Light2DField::Active),
        SceneFieldName::RenderLayers => Some(Light2DField::RenderLayers),
        _ => None,
    }
}

pub(in super::super) fn resolve_scene_water_body(field: &SceneFieldName) -> Option<WaterBodyField> {
    resolve_water_body(field.as_ref())
}

pub(in super::super) fn resolve_water_body(field: &str) -> Option<WaterBodyField> {
    match field {
        "shape" => Some(WaterBodyField::Shape),
        "resolution" | "sim_resolution" => Some(WaterBodyField::Resolution),
        "render_resolution" | "mesh_resolution" => Some(WaterBodyField::RenderResolution),
        "vertices_per_meter"
        | "verts_per_meter"
        | "vpm"
        | "resolution_per_meter"
        | "sim_vertices_per_meter" => Some(WaterBodyField::VerticesPerMeter),
        "sim_cells_per_meter" | "simulation_cells_per_meter" => {
            Some(WaterBodyField::SimCellsPerMeter)
        }
        "render_vertices_per_meter" | "render_verts_per_meter" | "mesh_vertices_per_meter" => {
            Some(WaterBodyField::RenderVerticesPerMeter)
        }
        "depth" => Some(WaterBodyField::Depth),
        "flow" => Some(WaterBodyField::Flow),
        "wind" => Some(WaterBodyField::Wind),
        "idle_mode" | "idle" => Some(WaterBodyField::IdleMode),
        "wave_speed" => Some(WaterBodyField::WaveSpeed),
        "wave_scale" => Some(WaterBodyField::WaveScale),
        "wave_length" | "wavelength" | "wave_size" => Some(WaterBodyField::WaveLength),
        "wake_strength" => Some(WaterBodyField::WakeStrength),
        "foam_strength" => Some(WaterBodyField::FoamStrength),
        "damping" => Some(WaterBodyField::Damping),
        "buoyancy" => Some(WaterBodyField::Buoyancy),
        "drag" => Some(WaterBodyField::Drag),
        "sample_readback_rate" | "readback_rate" => Some(WaterBodyField::SampleReadbackRate),
        "lod_near_distance" | "lod_near" => Some(WaterBodyField::LodNearDistance),
        "lod_mid_distance" | "lod_mid" => Some(WaterBodyField::LodMidDistance),
        "lod_far_distance" | "lod_far" => Some(WaterBodyField::LodFarDistance),
        "lod_min_resolution" | "min_resolution" => Some(WaterBodyField::LodMinResolution),
        "collision_layers" => Some(WaterBodyField::CollisionLayers),
        "collision_mask" => Some(WaterBodyField::CollisionMask),
        "link_layers" | "water_link_layers" => Some(WaterBodyField::LinkLayers),
        "link_mask" | "water_link_mask" => Some(WaterBodyField::LinkMask),
        "blend_width" | "link_blend_width" => Some(WaterBodyField::BlendWidth),
        "wave_transfer" | "link_wave_transfer" => Some(WaterBodyField::WaveTransfer),
        "flow_transfer" | "link_flow_transfer" => Some(WaterBodyField::FlowTransfer),
        "deep_color" | "deep_water_color" => Some(WaterBodyField::DeepColor),
        "shallow_color" | "shallow_water_color" => Some(WaterBodyField::ShallowColor),
        "shallow_depth" | "shallow_cutoff" | "shallowness" | "shallowness_depth" => {
            Some(WaterBodyField::ShallowDepth)
        }
        "sky_bias" | "sky_reflect" | "sky_reflection" => Some(WaterBodyField::SkyBias),
        "optics" | "water_colors" | "colors" => Some(WaterBodyField::Optics),
        "material" | "visual" | "water_material" => Some(WaterBodyField::Material),
        "transparency" => Some(WaterBodyField::Transparency),
        "reflectivity" | "reflection_strength" => Some(WaterBodyField::Reflectivity),
        "roughness" => Some(WaterBodyField::Roughness),
        "fresnel_power" => Some(WaterBodyField::FresnelPower),
        "normal_strength" => Some(WaterBodyField::NormalStrength),
        "ripple_scale" => Some(WaterBodyField::RippleScale),
        "foam_color" => Some(WaterBodyField::FoamColor),
        "foam_amount" => Some(WaterBodyField::FoamAmount),
        "crest_foam_threshold" => Some(WaterBodyField::CrestFoamThreshold),
        "caustic_strength" => Some(WaterBodyField::CausticStrength),
        "refraction_strength" => Some(WaterBodyField::RefractionStrength),
        "scattering_strength" => Some(WaterBodyField::ScatteringStrength),
        "distance_fog_strength" => Some(WaterBodyField::DistanceFogStrength),
        "coastline" => Some(WaterBodyField::Coastline),
        "debug" => Some(WaterBodyField::Debug),
        _ => None,
    }
}

pub(in super::super) fn resolve_scene_light3d_common(
    field: &SceneFieldName,
) -> Option<Light3DField> {
    match field {
        SceneFieldName::Color => Some(Light3DField::Color),
        SceneFieldName::Intensity => Some(Light3DField::Intensity),
        SceneFieldName::CastShadows => Some(Light3DField::CastShadows),
        SceneFieldName::Shadow => Some(Light3DField::Shadow),
        SceneFieldName::ShadowStrength => Some(Light3DField::ShadowStrength),
        SceneFieldName::ShadowDepthBias => Some(Light3DField::ShadowDepthBias),
        SceneFieldName::ShadowNormalBias => Some(Light3DField::ShadowNormalBias),
        SceneFieldName::Active => Some(Light3DField::Active),
        SceneFieldName::RenderLayers => Some(Light3DField::RenderLayers),
        _ => None,
    }
}

pub(in super::super) fn resolve_scene_static_body_2d(
    field: &SceneFieldName,
) -> Option<StaticBody2DField> {
    match field {
        SceneFieldName::Enabled => Some(StaticBody2DField::Enabled),
        SceneFieldName::CollisionLayers => Some(StaticBody2DField::CollisionLayers),
        SceneFieldName::CollisionMask => Some(StaticBody2DField::CollisionMask),
        SceneFieldName::Friction => Some(StaticBody2DField::Friction),
        SceneFieldName::Restitution => Some(StaticBody2DField::Restitution),
        SceneFieldName::Density => Some(StaticBody2DField::Density),
        _ => None,
    }
}

pub(in super::super) fn resolve_scene_static_body_3d(
    field: &SceneFieldName,
) -> Option<StaticBody3DField> {
    match field {
        SceneFieldName::Enabled => Some(StaticBody3DField::Enabled),
        SceneFieldName::CollisionLayers => Some(StaticBody3DField::CollisionLayers),
        SceneFieldName::CollisionMask => Some(StaticBody3DField::CollisionMask),
        SceneFieldName::Friction => Some(StaticBody3DField::Friction),
        SceneFieldName::Restitution => Some(StaticBody3DField::Restitution),
        SceneFieldName::Density => Some(StaticBody3DField::Density),
        _ => None,
    }
}

pub(in super::super) fn resolve_scene_rigid_body_2d(
    field: &SceneFieldName,
) -> Option<RigidBody2DField> {
    match field {
        SceneFieldName::Enabled => Some(RigidBody2DField::Enabled),
        SceneFieldName::CollisionLayers => Some(RigidBody2DField::CollisionLayers),
        SceneFieldName::CollisionMask => Some(RigidBody2DField::CollisionMask),
        SceneFieldName::ContinuousCollisionDetection => {
            Some(RigidBody2DField::ContinuousCollisionDetection)
        }
        SceneFieldName::Mass => Some(RigidBody2DField::Mass),
        SceneFieldName::LinearVelocity => Some(RigidBody2DField::LinearVelocity),
        SceneFieldName::AngularVelocity => Some(RigidBody2DField::AngularVelocity),
        SceneFieldName::GravityScale => Some(RigidBody2DField::GravityScale),
        SceneFieldName::LinearDamping => Some(RigidBody2DField::LinearDamping),
        SceneFieldName::AngularDamping => Some(RigidBody2DField::AngularDamping),
        SceneFieldName::CanSleep => Some(RigidBody2DField::CanSleep),
        SceneFieldName::LockRotation => Some(RigidBody2DField::LockRotation),
        SceneFieldName::Friction => Some(RigidBody2DField::Friction),
        SceneFieldName::Restitution => Some(RigidBody2DField::Restitution),
        SceneFieldName::Density => Some(RigidBody2DField::Density),
        _ => None,
    }
}

pub(in super::super) fn resolve_scene_rigid_body_3d(
    field: &SceneFieldName,
) -> Option<RigidBody3DField> {
    match field {
        SceneFieldName::Enabled => Some(RigidBody3DField::Enabled),
        SceneFieldName::CollisionLayers => Some(RigidBody3DField::CollisionLayers),
        SceneFieldName::CollisionMask => Some(RigidBody3DField::CollisionMask),
        SceneFieldName::ContinuousCollisionDetection => {
            Some(RigidBody3DField::ContinuousCollisionDetection)
        }
        SceneFieldName::Mass => Some(RigidBody3DField::Mass),
        SceneFieldName::LinearVelocity => Some(RigidBody3DField::LinearVelocity),
        SceneFieldName::AngularVelocity => Some(RigidBody3DField::AngularVelocity),
        SceneFieldName::GravityScale => Some(RigidBody3DField::GravityScale),
        SceneFieldName::LinearDamping => Some(RigidBody3DField::LinearDamping),
        SceneFieldName::AngularDamping => Some(RigidBody3DField::AngularDamping),
        SceneFieldName::CanSleep => Some(RigidBody3DField::CanSleep),
        SceneFieldName::Friction => Some(RigidBody3DField::Friction),
        SceneFieldName::Restitution => Some(RigidBody3DField::Restitution),
        SceneFieldName::Density => Some(RigidBody3DField::Density),
        _ => None,
    }
}

pub(in super::super) fn resolve_scene_area_2d(field: &SceneFieldName) -> Option<Area2DField> {
    match field {
        SceneFieldName::Enabled => Some(Area2DField::Enabled),
        SceneFieldName::CollisionLayers => Some(Area2DField::CollisionLayers),
        SceneFieldName::CollisionMask => Some(Area2DField::CollisionMask),
        _ => None,
    }
}

pub(in super::super) fn resolve_scene_character_body(
    field: &SceneFieldName,
) -> Option<CharacterBodyField> {
    match field {
        SceneFieldName::Enabled => Some(CharacterBodyField::Enabled),
        SceneFieldName::CollisionLayers => Some(CharacterBodyField::CollisionLayers),
        SceneFieldName::CollisionMask => Some(CharacterBodyField::CollisionMask),
        SceneFieldName::Friction => Some(CharacterBodyField::Friction),
        SceneFieldName::Restitution => Some(CharacterBodyField::Restitution),
        SceneFieldName::Density => Some(CharacterBodyField::Density),
        _ => resolve_character_body(field.as_ref()),
    }
}

pub(in super::super) fn resolve_character_body(field: &str) -> Option<CharacterBodyField> {
    match field {
        "enabled" => Some(CharacterBodyField::Enabled),
        "collision_layers" => Some(CharacterBodyField::CollisionLayers),
        "collision_mask" => Some(CharacterBodyField::CollisionMask),
        "friction" => Some(CharacterBodyField::Friction),
        "restitution" => Some(CharacterBodyField::Restitution),
        "density" => Some(CharacterBodyField::Density),
        _ => None,
    }
}

pub(in super::super) fn resolve_scene_area_3d(field: &SceneFieldName) -> Option<Area3DField> {
    match field {
        SceneFieldName::Enabled => Some(Area3DField::Enabled),
        SceneFieldName::CollisionLayers => Some(Area3DField::CollisionLayers),
        SceneFieldName::CollisionMask => Some(Area3DField::CollisionMask),
        _ => None,
    }
}

pub(in super::super) fn resolve_scene_physics_force_emitter(
    field: &SceneFieldName,
) -> Option<PhysicsForceEmitterField> {
    resolve_physics_force_emitter(field.as_ref())
}

pub(in super::super) fn resolve_physics_force_emitter(
    field: &str,
) -> Option<PhysicsForceEmitterField> {
    match field {
        "enabled" => Some(PhysicsForceEmitterField::Enabled),
        "profile" => Some(PhysicsForceEmitterField::Profile),
        "radius" | "range" => Some(PhysicsForceEmitterField::Radius),
        "strength" | "intensity" => Some(PhysicsForceEmitterField::Strength),
        "duration" => Some(PhysicsForceEmitterField::Duration),
        "pulse" => Some(PhysicsForceEmitterField::Pulse),
        "falloff" => Some(PhysicsForceEmitterField::Falloff),
        "affect_bodies" | "bodies" => Some(PhysicsForceEmitterField::AffectBodies),
        "affect_water" | "water" => Some(PhysicsForceEmitterField::AffectWater),
        "collision_layers" => Some(PhysicsForceEmitterField::CollisionLayers),
        "collision_mask" => Some(PhysicsForceEmitterField::CollisionMask),
        "vectors" | "forces" => Some(PhysicsForceEmitterField::Vectors),
        _ => None,
    }
}

pub(in super::super) fn resolve_scene_bone_attachment_2d(
    field: &SceneFieldName,
) -> Option<BoneAttachment2DField> {
    match field {
        SceneFieldName::Skeleton => Some(BoneAttachment2DField::Skeleton),
        SceneFieldName::BoneIndex => Some(BoneAttachment2DField::BoneIndex),
        _ => None,
    }
}

pub(in super::super) fn resolve_scene_bone_attachment_3d(
    field: &SceneFieldName,
) -> Option<BoneAttachment3DField> {
    match field {
        SceneFieldName::Skeleton => Some(BoneAttachment3DField::Skeleton),
        SceneFieldName::BoneIndex => Some(BoneAttachment3DField::BoneIndex),
        _ => None,
    }
}

pub(in super::super) fn resolve_scene_ik_target_2d(
    field: &SceneFieldName,
) -> Option<IKTarget2DField> {
    match field {
        SceneFieldName::Skeleton => Some(IKTarget2DField::Skeleton),
        SceneFieldName::BoneIndex => Some(IKTarget2DField::BoneIndex),
        SceneFieldName::ChainLength => Some(IKTarget2DField::ChainLength),
        SceneFieldName::Iterations => Some(IKTarget2DField::Iterations),
        SceneFieldName::Tolerance => Some(IKTarget2DField::Tolerance),
        SceneFieldName::Weight => Some(IKTarget2DField::Weight),
        SceneFieldName::MatchRotation => Some(IKTarget2DField::MatchRotation),
        SceneFieldName::Solver => Some(IKTarget2DField::Solver),
        _ => None,
    }
}

pub(in super::super) fn resolve_scene_ik_target_3d(
    field: &SceneFieldName,
) -> Option<IKTarget3DField> {
    match field {
        SceneFieldName::Skeleton => Some(IKTarget3DField::Skeleton),
        SceneFieldName::BoneIndex => Some(IKTarget3DField::BoneIndex),
        SceneFieldName::ChainLength => Some(IKTarget3DField::ChainLength),
        SceneFieldName::Iterations => Some(IKTarget3DField::Iterations),
        SceneFieldName::Tolerance => Some(IKTarget3DField::Tolerance),
        SceneFieldName::Weight => Some(IKTarget3DField::Weight),
        SceneFieldName::MatchRotation => Some(IKTarget3DField::MatchRotation),
        SceneFieldName::Solver => Some(IKTarget3DField::Solver),
        _ => None,
    }
}

pub(in super::super) fn resolve_scene_physics_bone_chain_2d(
    field: &SceneFieldName,
) -> Option<PhysicsBoneChain2DField> {
    match field {
        SceneFieldName::Skeleton => Some(PhysicsBoneChain2DField::Skeleton),
        SceneFieldName::BoneIndex => Some(PhysicsBoneChain2DField::BoneIndex),
        SceneFieldName::ChainLength => Some(PhysicsBoneChain2DField::ChainLength),
        SceneFieldName::Enabled => Some(PhysicsBoneChain2DField::Enabled),
        SceneFieldName::Gravity => Some(PhysicsBoneChain2DField::Gravity),
        SceneFieldName::Damping => Some(PhysicsBoneChain2DField::Damping),
        SceneFieldName::Stiffness => Some(PhysicsBoneChain2DField::Stiffness),
        SceneFieldName::Radius => Some(PhysicsBoneChain2DField::Radius),
        SceneFieldName::Collisions => Some(PhysicsBoneChain2DField::Collisions),
        SceneFieldName::Iterations => Some(PhysicsBoneChain2DField::Iterations),
        _ => None,
    }
}

pub(in super::super) fn resolve_scene_physics_bone_chain_3d(
    field: &SceneFieldName,
) -> Option<PhysicsBoneChain3DField> {
    match field {
        SceneFieldName::Skeleton => Some(PhysicsBoneChain3DField::Skeleton),
        SceneFieldName::BoneIndex => Some(PhysicsBoneChain3DField::BoneIndex),
        SceneFieldName::ChainLength => Some(PhysicsBoneChain3DField::ChainLength),
        SceneFieldName::Enabled => Some(PhysicsBoneChain3DField::Enabled),
        SceneFieldName::Gravity => Some(PhysicsBoneChain3DField::Gravity),
        SceneFieldName::Damping => Some(PhysicsBoneChain3DField::Damping),
        SceneFieldName::Stiffness => Some(PhysicsBoneChain3DField::Stiffness),
        SceneFieldName::Radius => Some(PhysicsBoneChain3DField::Radius),
        SceneFieldName::Collisions => Some(PhysicsBoneChain3DField::Collisions),
        SceneFieldName::Iterations => Some(PhysicsBoneChain3DField::Iterations),
        _ => None,
    }
}

pub(in super::super) fn resolve_scene_sky3d_field(field: &SceneFieldName) -> Option<Sky3DField> {
    match field {
        SceneFieldName::DayColors => Some(Sky3DField::DayColors),
        SceneFieldName::EveningColors => Some(Sky3DField::EveningColors),
        SceneFieldName::NightColors => Some(Sky3DField::NightColors),
        SceneFieldName::HorizonColors => Some(Sky3DField::HorizonColors),
        SceneFieldName::Environment => Some(Sky3DField::Environment),
        SceneFieldName::Time => Some(Sky3DField::Time),
        SceneFieldName::TimeOfDay => Some(Sky3DField::TimeOfDay),
        SceneFieldName::TimePaused => Some(Sky3DField::TimePaused),
        SceneFieldName::TimeScale => Some(Sky3DField::TimeScale),
        SceneFieldName::Shaders => Some(Sky3DField::Shaders),
        SceneFieldName::Active => Some(Sky3DField::Active),
        SceneFieldName::RenderLayers => Some(Sky3DField::RenderLayers),
        _ => None,
    }
}

pub(in super::super) fn resolve_joint2d_common(field: &str) -> Option<Joint2DField> {
    match field {
        "body_a" | "a" => Some(Joint2DField::BodyA),
        "body_b" | "b" => Some(Joint2DField::BodyB),
        "anchor_a" => Some(Joint2DField::AnchorA),
        "anchor_b" => Some(Joint2DField::AnchorB),
        "enabled" => Some(Joint2DField::Enabled),
        "collide_connected" | "collision" => Some(Joint2DField::CollideConnected),
        _ => None,
    }
}

pub(in super::super) fn resolve_joint3d_common(field: &str) -> Option<Joint3DField> {
    match field {
        "body_a" | "a" => Some(Joint3DField::BodyA),
        "body_b" | "b" => Some(Joint3DField::BodyB),
        "anchor_a" => Some(Joint3DField::AnchorA),
        "anchor_b" => Some(Joint3DField::AnchorB),
        "enabled" => Some(Joint3DField::Enabled),
        "collide_connected" | "collision" => Some(Joint3DField::CollideConnected),
        _ => None,
    }
}

pub(in super::super) fn resolve_light3d_common(field: &str) -> Option<Light3DField> {
    match field {
        "color" => Some(Light3DField::Color),
        "intensity" => Some(Light3DField::Intensity),
        "cast_shadows" | "casts_shadows" => Some(Light3DField::CastShadows),
        "shadow" => Some(Light3DField::Shadow),
        "shadow_strength" | "shadow_opacity" => Some(Light3DField::ShadowStrength),
        "shadow_depth_bias" | "shadow_bias" => Some(Light3DField::ShadowDepthBias),
        "shadow_normal_bias" => Some(Light3DField::ShadowNormalBias),
        "active" => Some(Light3DField::Active),
        "render_layers" => Some(Light3DField::RenderLayers),
        _ => None,
    }
}

pub(in super::super) fn resolve_light2d_common(field: &str) -> Option<Light2DField> {
    match field {
        "color" => Some(Light2DField::Color),
        "intensity" => Some(Light2DField::Intensity),
        "cast_shadows" | "casts_shadows" => Some(Light2DField::CastShadows),
        "shadow_softness" => Some(Light2DField::ShadowSoftness),
        "shadow_samples" => Some(Light2DField::ShadowSamples),
        "active" => Some(Light2DField::Active),
        "render_layers" => Some(Light2DField::RenderLayers),
        _ => None,
    }
}

pub(in super::super) fn resolve_sky3d_field(field: &str) -> Option<Sky3DField> {
    match field {
        "palette" => Some(Sky3DField::Palette),
        "sky_colors" | "colors" | "day_colors" => Some(Sky3DField::DayColors),
        "evening_colors" | "sunset_colors" | "dusk_colors" => Some(Sky3DField::EveningColors),
        "night_colors" => Some(Sky3DField::NightColors),
        "horizon_colors" | "horizon" => Some(Sky3DField::HorizonColors),
        "environment" | "ibl" => Some(Sky3DField::Environment),
        "time" => Some(Sky3DField::Time),
        "time_of_day" | "time.time_of_day" => Some(Sky3DField::TimeOfDay),
        "time_paused" | "pause_time" | "time.paused" => Some(Sky3DField::TimePaused),
        "time_scale" | "time_speed" | "time.scale" => Some(Sky3DField::TimeScale),
        "shaders" => Some(Sky3DField::Shaders),
        "active" => Some(Sky3DField::Active),
        "render_layers" => Some(Sky3DField::RenderLayers),
        _ => None,
    }
}

pub(in super::super) fn resolve_base_node_field(
    node_type: NodeType,
    field: &str,
) -> Option<NodeField> {
    if node_type.is_a(NodeType::Node2D) {
        return match field {
            "position" => Some(NodeField::Node2D(Node2DField::Position)),
            "rotation" | "rotation_deg" => Some(NodeField::Node2D(Node2DField::Rotation)),
            "scale" => Some(NodeField::Node2D(Node2DField::Scale)),
            "visible" => Some(NodeField::Node2D(Node2DField::Visible)),
            "modulate" | "tint" => Some(NodeField::Node2D(Node2DField::Modulate)),
            "self_modulate" | "self_tint" | "self_color" => {
                Some(NodeField::Node2D(Node2DField::SelfModulate))
            }
            "children_modulate" | "child_modulate" | "children_tint" | "child_tint" => {
                Some(NodeField::Node2D(Node2DField::ChildrenModulate))
            }
            "z_index" => Some(NodeField::Node2D(Node2DField::ZIndex)),
            "render_layers" => Some(NodeField::Node2D(Node2DField::RenderLayers)),
            _ => None,
        };
    }

    if node_type.is_a(NodeType::Node3D) {
        return match field {
            "position" => Some(NodeField::Node3D(Node3DField::Position)),
            "rotation" | "rotation_deg" => Some(NodeField::Node3D(Node3DField::Rotation)),
            "scale" => Some(NodeField::Node3D(Node3DField::Scale)),
            "visible" => Some(NodeField::Node3D(Node3DField::Visible)),
            "modulate" | "tint" => Some(NodeField::Node3D(Node3DField::Modulate)),
            "self_modulate" | "self_tint" | "self_color" => {
                Some(NodeField::Node3D(Node3DField::SelfModulate))
            }
            "children_modulate" | "child_modulate" | "children_tint" | "child_tint" => {
                Some(NodeField::Node3D(Node3DField::ChildrenModulate))
            }
            "render_layers" => Some(NodeField::Node3D(Node3DField::RenderLayers)),
            _ => None,
        };
    }

    if node_type.is_a(NodeType::UiNode) {
        return match field {
            "position" | "position_percent" | "position_pct" | "position_ratio" => {
                Some(NodeField::UiNode(UiNodeField::Position))
            }
            "scale" => Some(NodeField::UiNode(UiNodeField::Scale)),
            "rotation" | "rotation_deg" => Some(NodeField::UiNode(UiNodeField::Rotation)),
            "visible" => Some(NodeField::UiNode(UiNodeField::Visible)),
            "modulate" | "tint" => Some(NodeField::UiNode(UiNodeField::Modulate)),
            "self_modulate" | "self_tint" | "self_color" => {
                Some(NodeField::UiNode(UiNodeField::SelfModulate))
            }
            "children_modulate" | "child_modulate" | "children_tint" | "child_tint" => {
                Some(NodeField::UiNode(UiNodeField::ChildrenModulate))
            }
            "input_enabled" => Some(NodeField::UiNode(UiNodeField::InputEnabled)),
            "clip_children" => Some(NodeField::UiNode(UiNodeField::ClipChildren)),
            "z_index" => Some(NodeField::UiNode(UiNodeField::ZIndex)),
            _ => None,
        };
    }

    None
}

pub(in super::super) fn resolve_base_scene_node_field(
    node_type: NodeType,
    field: &SceneFieldName,
) -> Option<NodeField> {
    if node_type.is_a(NodeType::Node2D) {
        return match field {
            SceneFieldName::Position => Some(NodeField::Node2D(Node2DField::Position)),
            SceneFieldName::Rotation => Some(NodeField::Node2D(Node2DField::Rotation)),
            SceneFieldName::Scale => Some(NodeField::Node2D(Node2DField::Scale)),
            SceneFieldName::Visible => Some(NodeField::Node2D(Node2DField::Visible)),
            SceneFieldName::Modulate => Some(NodeField::Node2D(Node2DField::Modulate)),
            SceneFieldName::SelfModulate => Some(NodeField::Node2D(Node2DField::SelfModulate)),
            SceneFieldName::ChildrenModulate => {
                Some(NodeField::Node2D(Node2DField::ChildrenModulate))
            }
            SceneFieldName::ZIndex => Some(NodeField::Node2D(Node2DField::ZIndex)),
            SceneFieldName::RenderLayers => Some(NodeField::Node2D(Node2DField::RenderLayers)),
            _ => None,
        };
    }

    if node_type.is_a(NodeType::Node3D) {
        return match field {
            SceneFieldName::Position => Some(NodeField::Node3D(Node3DField::Position)),
            SceneFieldName::Rotation => Some(NodeField::Node3D(Node3DField::Rotation)),
            SceneFieldName::Scale => Some(NodeField::Node3D(Node3DField::Scale)),
            SceneFieldName::Visible => Some(NodeField::Node3D(Node3DField::Visible)),
            SceneFieldName::Modulate => Some(NodeField::Node3D(Node3DField::Modulate)),
            SceneFieldName::SelfModulate => Some(NodeField::Node3D(Node3DField::SelfModulate)),
            SceneFieldName::ChildrenModulate => {
                Some(NodeField::Node3D(Node3DField::ChildrenModulate))
            }
            SceneFieldName::RenderLayers => Some(NodeField::Node3D(Node3DField::RenderLayers)),
            _ => None,
        };
    }

    if node_type.is_a(NodeType::UiNode) {
        return match field {
            SceneFieldName::Position => Some(NodeField::UiNode(UiNodeField::Position)),
            SceneFieldName::Scale => Some(NodeField::UiNode(UiNodeField::Scale)),
            SceneFieldName::Rotation => Some(NodeField::UiNode(UiNodeField::Rotation)),
            SceneFieldName::Visible => Some(NodeField::UiNode(UiNodeField::Visible)),
            SceneFieldName::Modulate => Some(NodeField::UiNode(UiNodeField::Modulate)),
            SceneFieldName::SelfModulate => Some(NodeField::UiNode(UiNodeField::SelfModulate)),
            SceneFieldName::ChildrenModulate => {
                Some(NodeField::UiNode(UiNodeField::ChildrenModulate))
            }
            SceneFieldName::ZIndex => Some(NodeField::UiNode(UiNodeField::ZIndex)),
            _ => None,
        };
    }

    None
}

pub(in super::super) fn resolve_scene_camera_stream(
    field: &SceneFieldName,
) -> Option<CameraStreamField> {
    match field {
        SceneFieldName::Camera | SceneFieldName::Source => Some(CameraStreamField::Camera),
        SceneFieldName::Resolution => Some(CameraStreamField::Resolution),
        SceneFieldName::Width => Some(CameraStreamField::Width),
        SceneFieldName::Height => Some(CameraStreamField::Height),
        SceneFieldName::AspectRatio => Some(CameraStreamField::AspectRatio),
        SceneFieldName::AspectMode => Some(CameraStreamField::AspectMode),
        SceneFieldName::PostProcessing => Some(CameraStreamField::PostProcessing),
        SceneFieldName::Enabled | SceneFieldName::Active => Some(CameraStreamField::Enabled),
        SceneFieldName::Size => Some(CameraStreamField::Size),
        SceneFieldName::ZIndex => Some(CameraStreamField::ZIndex),
        SceneFieldName::Custom(name) if name.as_ref() == "webcam" => {
            Some(CameraStreamField::Camera)
        }
        _ => None,
    }
}

pub(in super::super) fn resolve_scene_webcam(field: &SceneFieldName) -> Option<WebcamField> {
    match field {
        SceneFieldName::Source | SceneFieldName::Src => Some(WebcamField::Device),
        SceneFieldName::Resolution => Some(WebcamField::Resolution),
        SceneFieldName::Width => Some(WebcamField::Width),
        SceneFieldName::Height => Some(WebcamField::Height),
        SceneFieldName::FpsScale => Some(WebcamField::Fps),
        SceneFieldName::FlipX => Some(WebcamField::Mirror),
        SceneFieldName::Enabled | SceneFieldName::Active => Some(WebcamField::Enabled),
        SceneFieldName::Custom(name) => match name.as_ref() {
            "slot" | "device" | "device_id" | "name" => Some(WebcamField::Device),
            "fps" | "frame_rate" => Some(WebcamField::Fps),
            "mirror" => Some(WebcamField::Mirror),
            "cpu_frames" | "cpu_frame" | "readback" => Some(WebcamField::CpuFrames),
            _ => None,
        },
        _ => None,
    }
}
