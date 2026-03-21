fn build_collision_shape_2d(data: &SceneDefNodeData) -> CollisionShape2D {
    let mut node = CollisionShape2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_collision_shape_2d_fields(&mut node, &data.fields);
    node
}

fn build_static_body_2d(data: &SceneDefNodeData) -> StaticBody2D {
    let mut node = StaticBody2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_static_body_2d_fields(&mut node, &data.fields);
    node
}

fn build_rigid_body_2d(data: &SceneDefNodeData) -> RigidBody2D {
    let mut node = RigidBody2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_rigid_body_2d_fields(&mut node, &data.fields);
    node
}

fn build_area_2d(data: &SceneDefNodeData) -> Area2D {
    let mut node = Area2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_area_2d_fields(&mut node, &data.fields);
    node
}

fn apply_collision_shape_2d_fields(node: &mut CollisionShape2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
            "shape" => {
                if let Some(shape) = as_shape_2d(value) {
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

fn apply_static_body_2d_fields(node: &mut StaticBody2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        if name == "enabled" {
            if let Some(enabled) = as_bool(value) {
                node.enabled = enabled;
            }
        }
    });
}

fn apply_rigid_body_2d_fields(node: &mut RigidBody2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
            "enabled" => {
                if let Some(enabled) = as_bool(value) {
                    node.enabled = enabled;
                }
            }
            "linear_velocity" | "velocity" => {
                if let Some(velocity) = as_vec2(value) {
                    node.linear_velocity = velocity;
                }
            }
            "angular_velocity" => {
                if let Some(angular_velocity) = as_f32(value) {
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
            "lock_rotation" => {
                if let Some(lock_rotation) = as_bool(value) {
                    node.lock_rotation = lock_rotation;
                }
            }
            _ => {}
        });
}

fn apply_area_2d_fields(node: &mut Area2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        if name == "enabled" {
            if let Some(enabled) = as_bool(value) {
                node.enabled = enabled;
            }
        }
    });
}

fn as_shape_2d(value: &SceneValue) -> Option<Shape2D> {
    let SceneValue::Object(entries) = value else {
        return None;
    };
    let ty = entries.iter().find_map(|(k, v)| match k.as_ref() {
        "type" | "kind" => as_str(v).map(|s| s.to_ascii_lowercase()),
        _ => None,
    })?;
    let width = entries
        .iter()
        .find_map(|(k, v)| (k == "width").then(|| as_f32(v)).flatten())
        .unwrap_or(1.0);
    let height = entries
        .iter()
        .find_map(|(k, v)| (k == "height").then(|| as_f32(v)).flatten())
        .unwrap_or(width);
    let radius = entries
        .iter()
        .find_map(|(k, v)| (k == "radius").then(|| as_f32(v)).flatten())
        .unwrap_or(0.5);

    match ty.as_ref() {
        "quad" | "rect" | "rectangle" => Some(Shape2D::Quad { width, height }),
        "circle" => Some(Shape2D::Circle { radius }),
        "tri" | "triangle" => {
            let tri_kind = entries
                .iter()
                .find_map(|(k, v)| (k == "triangle").then(|| as_str(v)).flatten())
                .or_else(|| {
                    entries
                        .iter()
                        .find_map(|(k, v)| (k == "variant").then(|| as_str(v)).flatten())
                })
                .map(|raw| match raw.to_ascii_lowercase().as_ref() {
                    "right" => Triangle2DKind::Right,
                    "isosceles" => Triangle2DKind::Isosceles,
                    _ => Triangle2DKind::Equilateral,
                })
                .unwrap_or(Triangle2DKind::Equilateral);
            Some(Shape2D::Triangle {
                kind: tri_kind,
                width,
                height,
            })
        }
        _ => None,
    }
}

