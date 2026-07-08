fn build_collision_shape_3d(data: &SceneDefNodeData) -> CollisionShape3D {
    let mut node = CollisionShape3D::default();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_collision_shape_3d_fields(&mut node, &data.fields);
    node
}

fn build_static_body_3d(data: &SceneDefNodeData) -> StaticBody3D {
    let mut node = StaticBody3D::default();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_static_body_3d_fields(&mut node, &data.fields);
    node
}

fn build_rigid_body_3d(data: &SceneDefNodeData) -> RigidBody3D {
    let mut node = RigidBody3D::default();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_rigid_body_3d_fields(&mut node, &data.fields);
    node
}

fn build_character_body_3d(data: &SceneDefNodeData) -> CharacterBody3D {
    let mut node = CharacterBody3D::default();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_character_body_3d_fields(&mut node, &data.fields);
    node
}

fn apply_character_body_3d_fields(node: &mut CharacterBody3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("CharacterBody3D", name) {
            Some(NodeField::CharacterBody3D(CharacterBodyField::Enabled)) => {
                if let Some(v) = as_bool(value) {
                    node.enabled = v;
                }
            }
            Some(NodeField::CharacterBody3D(CharacterBodyField::CollisionLayers)) => {
                if let Some(v) = as_bitmask(value) {
                    node.collision_layers = v;
                }
            }
            Some(NodeField::CharacterBody3D(CharacterBodyField::CollisionMask)) => {
                if let Some(v) = as_bitmask(value) {
                    node.collision_mask = v;
                }
            }
            Some(NodeField::CharacterBody3D(CharacterBodyField::Friction)) => {
                if let Some(v) = as_f32(value) {
                    node.friction = v;
                }
            }
            Some(NodeField::CharacterBody3D(CharacterBodyField::Restitution)) => {
                if let Some(v) = as_f32(value) {
                    node.restitution = v;
                }
            }
            Some(NodeField::CharacterBody3D(CharacterBodyField::Density)) => {
                if let Some(v) = as_f32(value) {
                    node.density = v;
                }
            }
            _ => {}
        }
    });
}

fn build_physics_force_emitter_3d(data: &SceneDefNodeData) -> PhysicsForceEmitter3D {
    let mut node = PhysicsForceEmitter3D::default();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_physics_force_emitter_3d_fields(&mut node, &data.fields);
    node
}

fn build_area_3d(data: &SceneDefNodeData) -> Area3D {
    let mut node = Area3D::default();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_area_3d_fields(&mut node, &data.fields);
    node
}

fn apply_physics_force_emitter_3d_fields(
    node: &mut PhysicsForceEmitter3D,
    fields: &[SceneObjectField],
) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("PhysicsForceEmitter3D", name) {
            Some(NodeField::PhysicsForceEmitter3D(PhysicsForceEmitterField::Enabled)) => {
                if let Some(v) = as_bool(value) {
                    node.enabled = v;
                }
            }
            Some(NodeField::PhysicsForceEmitter3D(PhysicsForceEmitterField::Profile)) => {
                if let Some(v) = as_force_profile(value) {
                    node.profile = v;
                }
            }
            Some(NodeField::PhysicsForceEmitter3D(PhysicsForceEmitterField::Radius)) => {
                if let Some(v) = as_f32(value) {
                    node.radius = v.max(0.0);
                }
            }
            Some(NodeField::PhysicsForceEmitter3D(PhysicsForceEmitterField::Strength)) => {
                if let Some(v) = as_f32(value) {
                    node.strength = v;
                }
            }
            Some(NodeField::PhysicsForceEmitter3D(PhysicsForceEmitterField::Duration)) => {
                if let Some(v) = as_f32(value) {
                    node.duration = v.max(0.0);
                }
            }
            Some(NodeField::PhysicsForceEmitter3D(PhysicsForceEmitterField::Pulse)) => {
                if let Some(v) = as_bool(value) {
                    node.pulse = v;
                }
            }
            Some(NodeField::PhysicsForceEmitter3D(PhysicsForceEmitterField::Falloff)) => {
                if let Some(v) = as_f32(value) {
                    node.falloff = v.max(0.0);
                }
            }
            Some(NodeField::PhysicsForceEmitter3D(PhysicsForceEmitterField::AffectBodies)) => {
                if let Some(v) = as_bool(value) {
                    node.affect_bodies = v;
                }
            }
            Some(NodeField::PhysicsForceEmitter3D(PhysicsForceEmitterField::AffectWater)) => {
                if let Some(v) = as_bool(value) {
                    node.affect_water = v;
                }
            }
            Some(NodeField::PhysicsForceEmitter3D(PhysicsForceEmitterField::CollisionLayers)) => {
                if let Some(v) = as_bitmask(value) {
                    node.collision_layers = v;
                }
            }
            Some(NodeField::PhysicsForceEmitter3D(PhysicsForceEmitterField::CollisionMask)) => {
                if let Some(v) = as_bitmask(value) {
                    node.collision_mask = v;
                }
            }
            Some(NodeField::PhysicsForceEmitter3D(PhysicsForceEmitterField::Vectors)) => {
                if let Some(v) = as_vec3_array(value) {
                    node.vectors = v;
                }
            }
            _ => {}
        }
    });
}

fn build_ball_joint_3d(data: &SceneDefNodeData) -> BallJoint3D {
    let mut node = BallJoint3D::default();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_ball_joint_3d_fields(&mut node, &data.fields);
    node
}

fn build_hinge_joint_3d(data: &SceneDefNodeData) -> HingeJoint3D {
    let mut node = HingeJoint3D::default();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_hinge_joint_3d_fields(&mut node, &data.fields);
    node
}

fn build_fixed_joint_3d(data: &SceneDefNodeData) -> FixedJoint3D {
    let mut node = FixedJoint3D::default();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_fixed_joint_3d_fields(&mut node, &data.fields);
    node
}

fn apply_collision_shape_3d_fields(node: &mut CollisionShape3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("CollisionShape3D", name) {
            Some(NodeField::CollisionShape3D(CollisionShape3DField::Shape)) => {
                if let Some(shape) = as_shape_3d(value) {
                    node.shape = shape;
                }
            }
            Some(NodeField::CollisionShape3D(CollisionShape3DField::Trimesh)) => {
                if let Some(source) = as_asset_source(value) {
                    node.shape = Shape3D::TriMesh { source };
                }
            }
            Some(NodeField::CollisionShape3D(CollisionShape3DField::FlipX)) => {
                if let Some(flip) = as_bool(value) {
                    node.flip_x = flip;
                }
            }
            Some(NodeField::CollisionShape3D(CollisionShape3DField::FlipY)) => {
                if let Some(flip) = as_bool(value) {
                    node.flip_y = flip;
                }
            }
            Some(NodeField::CollisionShape3D(CollisionShape3DField::FlipZ)) => {
                if let Some(flip) = as_bool(value) {
                    node.flip_z = flip;
                }
            }
            Some(NodeField::CollisionShape3D(CollisionShape3DField::Debug)) => {
                if let Some(debug) = as_bool(value) {
                    node.debug = debug;
                }
            }
            _ => {}
        }
    });
}

fn apply_static_body_3d_fields(node: &mut StaticBody3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("StaticBody3D", name) {
            Some(NodeField::StaticBody3D(StaticBody3DField::Enabled)) => {
                if let Some(enabled) = as_bool(value) {
                    node.enabled = enabled;
                }
            }
            Some(NodeField::StaticBody3D(StaticBody3DField::CollisionLayers)) => {
                if let Some(v) = as_bitmask(value) {
                    node.collision_layers = v;
                }
            }
            Some(NodeField::StaticBody3D(StaticBody3DField::CollisionMask)) => {
                if let Some(v) = as_bitmask(value) {
                    node.collision_mask = v;
                }
            }
            Some(NodeField::StaticBody3D(StaticBody3DField::Friction)) => {
                if let Some(v) = as_f32(value) {
                    node.friction = v;
                }
            }
            Some(NodeField::StaticBody3D(StaticBody3DField::Restitution)) => {
                if let Some(v) = as_f32(value) {
                    node.restitution = v;
                }
            }
            Some(NodeField::StaticBody3D(StaticBody3DField::Density)) => {
                if let Some(v) = as_f32(value) {
                    node.density = v;
                }
            }
            _ => {}
        }
    });
}

fn apply_rigid_body_3d_fields(node: &mut RigidBody3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("RigidBody3D", name) {
            Some(NodeField::RigidBody3D(RigidBody3DField::Enabled)) => {
                if let Some(enabled) = as_bool(value) {
                    node.enabled = enabled;
                }
            }
            Some(NodeField::RigidBody3D(RigidBody3DField::CollisionLayers)) => {
                if let Some(v) = as_bitmask(value) {
                    node.collision_layers = v;
                }
            }
            Some(NodeField::RigidBody3D(RigidBody3DField::CollisionMask)) => {
                if let Some(v) = as_bitmask(value) {
                    node.collision_mask = v;
                }
            }
            Some(NodeField::RigidBody3D(
                RigidBody3DField::ContinuousCollisionDetection,
            )) => {
                if let Some(ccd) = as_bool(value) {
                    node.continuous_collision_detection = ccd;
                }
            }
            Some(NodeField::RigidBody3D(RigidBody3DField::Mass)) => {
                if let Some(mass) = as_f32(value) {
                    node.mass = mass.max(0.0);
                }
            }
            Some(NodeField::RigidBody3D(RigidBody3DField::LinearVelocity)) => {
                if let Some(velocity) = as_vec3(value) {
                    node.linear_velocity = velocity;
                }
            }
            Some(NodeField::RigidBody3D(RigidBody3DField::AngularVelocity)) => {
                if let Some(angular_velocity) = as_vec3(value) {
                    node.angular_velocity = angular_velocity;
                }
            }
            Some(NodeField::RigidBody3D(RigidBody3DField::GravityScale)) => {
                if let Some(gravity_scale) = as_f32(value) {
                    node.gravity_scale = gravity_scale;
                }
            }
            Some(NodeField::RigidBody3D(RigidBody3DField::LinearDamping)) => {
                if let Some(linear_damping) = as_f32(value) {
                    node.linear_damping = linear_damping;
                }
            }
            Some(NodeField::RigidBody3D(RigidBody3DField::AngularDamping)) => {
                if let Some(angular_damping) = as_f32(value) {
                    node.angular_damping = angular_damping;
                }
            }
            Some(NodeField::RigidBody3D(RigidBody3DField::CanSleep)) => {
                if let Some(can_sleep) = as_bool(value) {
                    node.can_sleep = can_sleep;
                }
            }
            Some(NodeField::RigidBody3D(RigidBody3DField::Friction)) => {
                if let Some(friction) = as_f32(value) {
                    node.friction = friction;
                }
            }
            Some(NodeField::RigidBody3D(RigidBody3DField::Restitution)) => {
                if let Some(restitution) = as_f32(value) {
                    node.restitution = restitution;
                }
            }
            Some(NodeField::RigidBody3D(RigidBody3DField::Density)) => {
                if let Some(density) = as_f32(value) {
                    node.density = density;
                }
            }
            _ => {}
        }
    });
}

fn apply_area_3d_fields(node: &mut Area3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("Area3D", name) {
            Some(NodeField::Area3D(Area3DField::Enabled)) => {
                if let Some(enabled) = as_bool(value) {
                    node.enabled = enabled;
                }
            }
            Some(NodeField::Area3D(Area3DField::CollisionLayers)) => {
                if let Some(v) = as_bitmask(value) {
                    node.collision_layers = v;
                }
            }
            Some(NodeField::Area3D(Area3DField::CollisionMask)) => {
                if let Some(v) = as_bitmask(value) {
                    node.collision_mask = v;
                }
            }
            _ => {}
        }
    });
}

struct Joint3DCommonMut<'a> {
    body_a: &'a mut NodeID,
    body_b: &'a mut NodeID,
    anchor_a: &'a mut Vector3,
    anchor_b: &'a mut Vector3,
    enabled: &'a mut bool,
    collide_connected: &'a mut bool,
}

fn apply_joint_3d_common(
    node: Joint3DCommonMut<'_>,
    ty: &str,
    name: &str,
    value: &SceneValue,
) {
    let common = match resolve_node_field(ty, name) {
        Some(NodeField::BallJoint3D(field)) => Some(field),
        Some(NodeField::FixedJoint3D(field)) => Some(field),
        Some(NodeField::HingeJoint3D(HingeJoint3DField::Common(field))) => Some(field),
        _ => None,
    };
    match common {
        Some(Joint3DField::BodyA) => {
            if let Some(v) = as_node_id(value) {
                *node.body_a = v;
            }
        }
        Some(Joint3DField::BodyB) => {
            if let Some(v) = as_node_id(value) {
                *node.body_b = v;
            }
        }
        Some(Joint3DField::AnchorA) => {
            if let Some(v) = as_vec3(value) {
                *node.anchor_a = v;
            }
        }
        Some(Joint3DField::AnchorB) => {
            if let Some(v) = as_vec3(value) {
                *node.anchor_b = v;
            }
        }
        Some(Joint3DField::Enabled) => {
            if let Some(v) = as_bool(value) {
                *node.enabled = v;
            }
        }
        Some(Joint3DField::CollideConnected) => {
            if let Some(v) = as_bool(value) {
                *node.collide_connected = v;
            }
        }
        None => {}
    }
}

fn apply_ball_joint_3d_fields(node: &mut BallJoint3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        apply_joint_3d_common(
            Joint3DCommonMut {
                body_a: &mut node.body_a,
                body_b: &mut node.body_b,
                anchor_a: &mut node.anchor_a,
                anchor_b: &mut node.anchor_b,
                enabled: &mut node.enabled,
                collide_connected: &mut node.collide_connected,
            },
            "BallJoint3D",
            name,
            value,
        );
    });
}

fn apply_fixed_joint_3d_fields(node: &mut FixedJoint3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        apply_joint_3d_common(
            Joint3DCommonMut {
                body_a: &mut node.body_a,
                body_b: &mut node.body_b,
                anchor_a: &mut node.anchor_a,
                anchor_b: &mut node.anchor_b,
                enabled: &mut node.enabled,
                collide_connected: &mut node.collide_connected,
            },
            "FixedJoint3D",
            name,
            value,
        );
    });
}

fn apply_hinge_joint_3d_fields(node: &mut HingeJoint3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("HingeJoint3D", name) {
            Some(NodeField::HingeJoint3D(HingeJoint3DField::Axis)) => {
                if let Some(v) = as_vec3(value) {
                    node.axis = v;
                }
            }
            _ => apply_joint_3d_common(
                Joint3DCommonMut {
                    body_a: &mut node.body_a,
                    body_b: &mut node.body_b,
                    anchor_a: &mut node.anchor_a,
                    anchor_b: &mut node.anchor_b,
                    enabled: &mut node.enabled,
                    collide_connected: &mut node.collide_connected,
                },
                "HingeJoint3D",
                name,
                value,
            ),
        }
    });
}

fn as_shape_3d(value: &SceneValue) -> Option<Shape3D> {
    if let Some(source) = as_asset_source(value) {
        return Some(Shape3D::TriMesh { source });
    }

    let SceneValue::Object(entries) = value else {
        return None;
    };
    let ty = entries.iter().find_map(|(k, v)| match k.as_ref() {
        "type" | "kind" => as_str(v).map(|s| s.to_ascii_lowercase()),
        _ => None,
    })?;

    let size = entries
        .iter()
        .find_map(|(k, v)| (k == "size").then(|| as_vec3(v)).flatten())
        .unwrap_or(Vector3::ONE);
    let radius = entries
        .iter()
        .find_map(|(k, v)| (k == "radius").then(|| as_f32(v)).flatten())
        .unwrap_or(0.5);
    let half_height = entries
        .iter()
        .find_map(|(k, v)| (k == "half_height").then(|| as_f32(v)).flatten())
        .or_else(|| {
            entries
                .iter()
                .find_map(|(k, v)| (k == "height").then(|| as_f32(v).map(|h| h * 0.5)).flatten())
        })
        .unwrap_or(0.5);
    let source = entries.iter().find_map(|(k, v)| match k.as_ref() {
        "source" | "mesh" | "trimesh" => as_asset_source(v),
        _ => None,
    });

    match ty.as_ref() {
        "cube" => Some(Shape3D::Cube { size }),
        "sphere" => Some(Shape3D::Sphere { radius }),
        "capsule" => Some(Shape3D::Capsule {
            radius,
            half_height,
        }),
        "cylinder" => Some(Shape3D::Cylinder {
            radius,
            half_height,
        }),
        "cone" => Some(Shape3D::Cone {
            radius,
            half_height,
        }),
        "tri_prism" | "triprism" => Some(Shape3D::TriPrism { size }),
        "triangular_pyramid" | "tri_pyr" => Some(Shape3D::TriangularPyramid { size }),
        "square_pyramid" | "sq_pyr" => Some(Shape3D::SquarePyramid { size }),
        "trimesh" | "tri_mesh" => source.map(|source| Shape3D::TriMesh { source }),
        _ => None,
    }
}
