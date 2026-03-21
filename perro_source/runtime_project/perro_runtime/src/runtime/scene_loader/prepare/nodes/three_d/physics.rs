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
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
            "shape" => {
                if let Some(shape) = as_shape_3d(value) {
                    node.shape = shape;
                }
            }
            "sensor" => {
                if let Some(sensor) = as_bool(value) {
                    node.sensor = sensor;
                }
            }
            "friction" => {
                if let Some(friction) = as_f32(value) {
                    node.friction = friction;
                }
            }
            "restitution" => {
                if let Some(restitution) = as_f32(value) {
                    node.restitution = restitution;
                }
            }
            "density" => {
                if let Some(density) = as_f32(value) {
                    node.density = density;
                }
            }
            _ => {}
        });
}

fn apply_static_body_3d_fields(node: &mut StaticBody3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        if name == "enabled" {
            if let Some(enabled) = as_bool(value) {
                node.enabled = enabled;
            }
        }
    });
}

fn apply_rigid_body_3d_fields(node: &mut RigidBody3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
            "enabled" => {
                if let Some(enabled) = as_bool(value) {
                    node.enabled = enabled;
                }
            }
            "linear_velocity" | "velocity" => {
                if let Some(velocity) = as_vec3(value) {
                    node.linear_velocity = velocity;
                }
            }
            "angular_velocity" => {
                if let Some(angular_velocity) = as_vec3(value) {
                    node.angular_velocity = angular_velocity;
                }
            }
            "gravity_scale" => {
                if let Some(gravity_scale) = as_f32(value) {
                    node.gravity_scale = gravity_scale;
                }
            }
            "linear_damping" => {
                if let Some(linear_damping) = as_f32(value) {
                    node.linear_damping = linear_damping;
                }
            }
            "angular_damping" => {
                if let Some(angular_damping) = as_f32(value) {
                    node.angular_damping = angular_damping;
                }
            }
            "can_sleep" => {
                if let Some(can_sleep) = as_bool(value) {
                    node.can_sleep = can_sleep;
                }
            }
            _ => {}
        });
}

fn apply_area_3d_fields(node: &mut Area3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        if name == "enabled" {
            if let Some(enabled) = as_bool(value) {
                node.enabled = enabled;
            }
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
