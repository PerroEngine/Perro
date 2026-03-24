fn build_ray_light_3d(data: &SceneDefNodeData) -> RayLight3D {
    let mut node = RayLight3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_ray_light_3d_fields(&mut node, &data.fields);
    node
}

fn build_ambient_light_3d(data: &SceneDefNodeData) -> AmbientLight3D {
    let mut node = AmbientLight3D::new();
    apply_ambient_light_3d_fields(&mut node, &data.fields);
    node
}

fn build_point_light_3d(data: &SceneDefNodeData) -> PointLight3D {
    let mut node = PointLight3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_point_light_3d_fields(&mut node, &data.fields);
    node
}

fn build_sky_3d(data: &SceneDefNodeData) -> Sky3D {
    let mut node = Sky3D::new();
    apply_sky_3d_fields(&mut node, &data.fields);
    node
}

fn build_spot_light_3d(data: &SceneDefNodeData) -> SpotLight3D {
    let mut node = SpotLight3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_spot_light_3d_fields(&mut node, &data.fields);
    node
}

fn apply_ray_light_3d_fields(node: &mut RayLight3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("RayLight3D", name) {
            Some(NodeField::Light3D(Light3DField::Color)) => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            Some(NodeField::Light3D(Light3DField::Intensity)) => {
                if let Some(v) = as_f32(value) {
                    node.intensity = v;
                }
            }
            Some(NodeField::Light3D(Light3DField::Active)) => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            Some(NodeField::RayLight3D(RayLight3DField::Visible)) => {
                if let Some(v) = as_bool(value) {
                    node.visible = v;
                }
            }
            _ => {}
        }
    });
}

fn apply_ambient_light_3d_fields(node: &mut AmbientLight3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("AmbientLight3D", name) {
            Some(NodeField::Light3D(Light3DField::Color)) => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            Some(NodeField::Light3D(Light3DField::Intensity)) => {
                if let Some(v) = as_f32(value) {
                    node.intensity = v;
                }
            }
            Some(NodeField::Light3D(Light3DField::Active)) => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            _ => {}
        }
    });
}

fn apply_point_light_3d_fields(node: &mut PointLight3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("PointLight3D", name) {
            Some(NodeField::Light3D(Light3DField::Color)) => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            Some(NodeField::Light3D(Light3DField::Intensity)) => {
                if let Some(v) = as_f32(value) {
                    node.intensity = v;
                }
            }
            Some(NodeField::PointLight3D(PointLight3DField::Range)) => {
                if let Some(v) = as_f32(value) {
                    node.range = v;
                }
            }
            Some(NodeField::Light3D(Light3DField::Active)) => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            _ => {}
        }
    });
}

fn apply_sky_3d_fields(node: &mut Sky3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("Sky3D", name) {
            Some(NodeField::Sky3D(Sky3DField::DayColors)) => {
                if let Some(colors) = as_color_array(value) {
                    node.day_colors = Cow::Owned(colors);
                }
            }
            Some(NodeField::Sky3D(Sky3DField::NightColors)) => {
                if let Some(colors) = as_color_array(value) {
                    node.night_colors = Cow::Owned(colors);
                }
            }
            Some(NodeField::Sky3D(Sky3DField::SkyAngle)) => {
                if let Some(v) = as_f32(value) {
                    node.sky_angle = v;
                }
            }
            Some(NodeField::Sky3D(Sky3DField::Time)) => {
                if let SceneValue::Object(entries) = value {
                    for (k, v) in entries.iter() {
                        match k.as_ref() {
                            "time_of_day" | "of_day" => {
                                if let Some(t) = as_f32(v) {
                                    node.time.time_of_day = t;
                                }
                            }
                            "paused" | "time_paused" => {
                                if let Some(p) = as_bool(v) {
                                    node.time.paused = p;
                                }
                            }
                            "scale" | "time_scale" | "speed" => {
                                if let Some(s) = as_f32(v) {
                                    node.time.scale = s;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Some(NodeField::Sky3D(Sky3DField::TimeOfDay)) => {
                if let Some(v) = as_f32(value) {
                    node.time.time_of_day = v;
                }
            }
            Some(NodeField::Sky3D(Sky3DField::TimePaused)) => {
                if let Some(v) = as_bool(value) {
                    node.time.paused = v;
                }
            }
            Some(NodeField::Sky3D(Sky3DField::TimeScale)) => {
                if let Some(v) = as_f32(value) {
                    node.time.scale = v;
                }
            }
            Some(NodeField::Sky3D(Sky3DField::CloudSize)) => {
                if let Some(v) = as_f32(value) {
                    node.clouds.size = v;
                }
            }
            Some(NodeField::Sky3D(Sky3DField::CloudDensity)) => {
                if let Some(v) = as_f32(value) {
                    node.clouds.density = v;
                }
            }
            Some(NodeField::Sky3D(Sky3DField::CloudVariance)) => {
                if let Some(v) = as_f32(value) {
                    node.clouds.variance = v;
                }
            }
            Some(NodeField::Sky3D(Sky3DField::StarSize)) => {
                if let Some(v) = as_f32(value) {
                    node.stars.size = v;
                }
            }
            Some(NodeField::Sky3D(Sky3DField::StarScatter)) => {
                if let Some(v) = as_f32(value) {
                    node.stars.scatter = v;
                }
            }
            Some(NodeField::Sky3D(Sky3DField::StarGleam)) => {
                if let Some(v) = as_f32(value) {
                    node.stars.gleam = v;
                }
            }
            Some(NodeField::Sky3D(Sky3DField::MoonSize)) => {
                if let Some(v) = as_f32(value) {
                    node.moon.size = v;
                }
            }
            Some(NodeField::Sky3D(Sky3DField::SunSize)) => {
                if let Some(v) = as_f32(value) {
                    node.sun.size = v;
                }
            }
            Some(NodeField::Sky3D(Sky3DField::SkyShader)) => {
                if let Some(v) = as_str(value) {
                    node.sky_shader = Some(Cow::Owned(v.to_string()));
                }
            }
            Some(NodeField::Sky3D(Sky3DField::Active)) => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            _ => {}
        }
    });
}

fn apply_spot_light_3d_fields(node: &mut SpotLight3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("SpotLight3D", name) {
            Some(NodeField::Light3D(Light3DField::Color)) => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            Some(NodeField::Light3D(Light3DField::Intensity)) => {
                if let Some(v) = as_f32(value) {
                    node.intensity = v;
                }
            }
            Some(NodeField::SpotLight3D(SpotLight3DField::Range)) => {
                if let Some(v) = as_f32(value) {
                    node.range = v;
                }
            }
            Some(NodeField::SpotLight3D(SpotLight3DField::InnerAngleRadians)) => {
                if let Some(v) = as_f32(value) {
                    node.inner_angle_radians = v;
                }
            }
            Some(NodeField::SpotLight3D(SpotLight3DField::OuterAngleRadians)) => {
                if let Some(v) = as_f32(value) {
                    node.outer_angle_radians = v;
                }
            }
            Some(NodeField::Light3D(Light3DField::Active)) => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            _ => {}
        }
    });
}

fn as_color_array(value: &SceneValue) -> Option<Vec<[f32; 3]>> {
    let SceneValue::Array(items) = value else {
        return None;
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items.iter() {
        match item {
            SceneValue::Vec3 { x, y, z } => out.push([*x, *y, *z]),
            SceneValue::Vec4 { x, y, z, .. } => out.push([*x, *y, *z]),
            SceneValue::Object(entries) => {
                let mut r = None;
                let mut g = None;
                let mut b = None;
                for (k, v) in entries.iter() {
                    match k.as_ref() {
                        "r" | "x" => r = as_f32(v),
                        "g" | "y" => g = as_f32(v),
                        "b" | "z" => b = as_f32(v),
                        _ => {}
                    }
                }
                if let (Some(r), Some(g), Some(b)) = (r, g, b) {
                    out.push([r, g, b]);
                }
            }
            _ => {}
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}
