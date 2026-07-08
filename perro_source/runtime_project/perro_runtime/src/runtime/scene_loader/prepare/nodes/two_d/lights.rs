fn build_ambient_light_2d(data: &SceneDefNodeData) -> AmbientLight2D {
    let mut node = AmbientLight2D::new();
    apply_ambient_light_2d_fields(&mut node, &data.fields);
    node
}

fn build_ray_light_2d(data: &SceneDefNodeData) -> RayLight2D {
    let mut node = RayLight2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_ray_light_2d_fields(&mut node, &data.fields);
    node
}

fn build_point_light_2d(data: &SceneDefNodeData) -> PointLight2D {
    let mut node = PointLight2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_point_light_2d_fields(&mut node, &data.fields);
    node
}

fn build_spot_light_2d(data: &SceneDefNodeData) -> SpotLight2D {
    let mut node = SpotLight2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_spot_light_2d_fields(&mut node, &data.fields);
    node
}

fn apply_ambient_light_2d_fields(node: &mut AmbientLight2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        apply_light_2d_common("AmbientLight2D", name, value, |field| match field {
            Light2DField::Color => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                } else if let Some((x, y, z, _)) = value.as_vec4() {
                    node.color = [x, y, z];
                }
            }
            Light2DField::Intensity => {
                if let Some(v) = value.as_f32() {
                    node.intensity = v;
                }
            }
            Light2DField::CastShadows => {
                if let Some(v) = value.as_bool() {
                    node.cast_shadows = v;
                }
            }
            Light2DField::Active => {
                if let Some(v) = value.as_bool() {
                    node.active = v;
                }
            }
            Light2DField::RenderLayers => {
                if let Some(v) = as_bitmask(value) {
                    node.render_layers = v;
                }
            }
        });
    });
}

fn apply_ray_light_2d_fields(node: &mut RayLight2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        apply_light_2d_common("RayLight2D", name, value, |field| match field {
            Light2DField::Color => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                } else if let Some((x, y, z, _)) = value.as_vec4() {
                    node.color = [x, y, z];
                }
            }
            Light2DField::Intensity => {
                if let Some(v) = value.as_f32() {
                    node.intensity = v;
                }
            }
            Light2DField::CastShadows => {
                if let Some(v) = value.as_bool() {
                    node.cast_shadows = v;
                }
            }
            Light2DField::Active => {
                if let Some(v) = value.as_bool() {
                    node.active = v;
                }
            }
            Light2DField::RenderLayers => {
                if let Some(v) = as_bitmask(value) {
                    node.render_layers = v;
                }
            }
        });
        if resolve_node_field("RayLight2D", name)
            == Some(NodeField::RayLight2D(RayLight2DField::Visible))
            && let Some(v) = value.as_bool()
        {
            node.visible = v;
        }
    });
}

fn apply_point_light_2d_fields(node: &mut PointLight2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("PointLight2D", name) {
            Some(NodeField::Light2D(Light2DField::Color)) => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                } else if let Some((x, y, z, _)) = value.as_vec4() {
                    node.color = [x, y, z];
                }
            }
            Some(NodeField::Light2D(Light2DField::Intensity)) => {
                if let Some(v) = value.as_f32() {
                    node.intensity = v;
                }
            }
            Some(NodeField::PointLight2D(PointLight2DField::Range)) => {
                if let Some(v) = value.as_f32() {
                    node.range = v;
                }
            }
            Some(NodeField::Light2D(Light2DField::CastShadows)) => {
                if let Some(v) = value.as_bool() {
                    node.cast_shadows = v;
                }
            }
            Some(NodeField::Light2D(Light2DField::Active)) => {
                if let Some(v) = value.as_bool() {
                    node.active = v;
                }
            }
            Some(NodeField::Light2D(Light2DField::RenderLayers)) => {
                if let Some(v) = as_bitmask(value) {
                    node.render_layers = v;
                }
            }
            _ => {}
        }
    });
}

fn apply_spot_light_2d_fields(node: &mut SpotLight2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("SpotLight2D", name) {
            Some(NodeField::Light2D(Light2DField::Color)) => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                } else if let Some((x, y, z, _)) = value.as_vec4() {
                    node.color = [x, y, z];
                }
            }
            Some(NodeField::Light2D(Light2DField::Intensity)) => {
                if let Some(v) = value.as_f32() {
                    node.intensity = v;
                }
            }
            Some(NodeField::SpotLight2D(SpotLight2DField::Range)) => {
                if let Some(v) = value.as_f32() {
                    node.range = v;
                }
            }
            Some(NodeField::SpotLight2D(SpotLight2DField::InnerAngleRadians)) => {
                if let Some(v) = value.as_f32() {
                    node.inner_angle_radians = v;
                }
            }
            Some(NodeField::SpotLight2D(SpotLight2DField::OuterAngleRadians)) => {
                if let Some(v) = value.as_f32() {
                    node.outer_angle_radians = v;
                }
            }
            Some(NodeField::Light2D(Light2DField::CastShadows)) => {
                if let Some(v) = value.as_bool() {
                    node.cast_shadows = v;
                }
            }
            Some(NodeField::Light2D(Light2DField::Active)) => {
                if let Some(v) = value.as_bool() {
                    node.active = v;
                }
            }
            Some(NodeField::Light2D(Light2DField::RenderLayers)) => {
                if let Some(v) = as_bitmask(value) {
                    node.render_layers = v;
                }
            }
            _ => {}
        }
    });
}

fn apply_light_2d_common(
    node_type: &str,
    name: &str,
    value: &SceneValue,
    mut apply: impl FnMut(Light2DField),
) {
    if let Some(NodeField::Light2D(field)) = resolve_node_field(node_type, name) {
        let _ = value;
        apply(field);
    }
}
