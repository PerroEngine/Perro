fn build_collision_shape_3d(data: &SceneDefNodeData) -> CollisionShape3D {
    let mut node = CollisionShape3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_collision_shape_3d_fields(&mut node, &data.fields);
    node
}

fn build_static_body_3d(data: &SceneDefNodeData) -> StaticBody3D {
    let mut node = StaticBody3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_static_body_3d_fields(&mut node, &data.fields);
    node
}

fn build_rigid_body_3d(data: &SceneDefNodeData) -> RigidBody3D {
    let mut node = RigidBody3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_rigid_body_3d_fields(&mut node, &data.fields);
    node
}

fn build_area_3d(data: &SceneDefNodeData) -> Area3D {
    let mut node = Area3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_area_3d_fields(&mut node, &data.fields);
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
            Some(NodeField::CollisionShape3D(CollisionShape3DField::Sensor)) => {
                if let Some(sensor) = as_bool(value) {
                    node.sensor = sensor;
                }
            }
            Some(NodeField::CollisionShape3D(CollisionShape3DField::Friction)) => {
                if let Some(friction) = as_f32(value) {
                    node.friction = friction;
                }
            }
            Some(NodeField::CollisionShape3D(CollisionShape3DField::Restitution)) => {
                if let Some(restitution) = as_f32(value) {
                    node.restitution = restitution;
                }
            }
            Some(NodeField::CollisionShape3D(CollisionShape3DField::Density)) => {
                if let Some(density) = as_f32(value) {
                    node.density = density;
                }
            }
            _ => {}
        }
    });
}

fn apply_static_body_3d_fields(node: &mut StaticBody3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        if resolve_node_field("StaticBody3D", name)
            == Some(NodeField::StaticBody3D(StaticBody3DField::Enabled))
            && let Some(enabled) = as_bool(value) {
                node.enabled = enabled;
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
            _ => {}
        }
    });
}

fn apply_area_3d_fields(node: &mut Area3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        if resolve_node_field("Area3D", name) == Some(NodeField::Area3D(Area3DField::Enabled))
            && let Some(enabled) = as_bool(value) {
                node.enabled = enabled;
            }
    });
}

fn as_shape_3d(value: &SceneValue) -> Option<Shape3D> {
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
        _ => None,
    }
}
