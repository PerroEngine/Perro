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
    let mut node = Sky3D::default();
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
                if let Some(v) = as_light_color(value) {
                    node.color = v;
                }
            }
            Some(NodeField::Light3D(Light3DField::Intensity)) => {
                if let Some(v) = as_f32(value) {
                    node.intensity = v;
                }
            }
            Some(NodeField::Light3D(Light3DField::CastShadows)) => {
                if let Some(v) = as_bool(value) {
                    node.cast_shadows = v;
                }
            }
            Some(NodeField::Light3D(Light3DField::Shadow)) => apply_shadow_tuning_object(
                &mut node.shadow_strength,
                &mut node.shadow_depth_bias,
                &mut node.shadow_normal_bias,
                value,
            ),
            Some(NodeField::Light3D(Light3DField::ShadowStrength)) => {
                set_shadow_strength(&mut node.shadow_strength, value);
            }
            Some(NodeField::Light3D(Light3DField::ShadowDepthBias)) => {
                set_shadow_depth_bias(&mut node.shadow_depth_bias, value);
            }
            Some(NodeField::Light3D(Light3DField::ShadowNormalBias)) => {
                set_shadow_normal_bias(&mut node.shadow_normal_bias, value);
            }
            Some(NodeField::Light3D(Light3DField::Active)) => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            Some(NodeField::Light3D(Light3DField::RenderLayers)) => {
                if let Some(v) = as_bitmask(value) {
                    node.render_layers = v;
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
                if let Some(v) = as_light_color(value) {
                    node.color = v;
                }
            }
            Some(NodeField::Light3D(Light3DField::Intensity)) => {
                if let Some(v) = as_f32(value) {
                    node.intensity = v;
                }
            }
            Some(NodeField::Light3D(Light3DField::CastShadows)) => {
                if let Some(v) = as_bool(value) {
                    node.cast_shadows = v;
                }
            }
            Some(NodeField::Light3D(
                Light3DField::Shadow
                | Light3DField::ShadowStrength
                | Light3DField::ShadowDepthBias
                | Light3DField::ShadowNormalBias,
            )) => {}
            Some(NodeField::Light3D(Light3DField::Active)) => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            Some(NodeField::Light3D(Light3DField::RenderLayers)) => {
                if let Some(v) = as_bitmask(value) {
                    node.render_layers = v;
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

fn apply_point_light_3d_fields(node: &mut PointLight3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("PointLight3D", name) {
            Some(NodeField::Light3D(Light3DField::Color)) => {
                if let Some(v) = as_light_color(value) {
                    node.color = v;
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
            Some(NodeField::Light3D(Light3DField::CastShadows)) => {
                if let Some(v) = as_bool(value) {
                    node.cast_shadows = v;
                }
            }
            Some(NodeField::Light3D(Light3DField::Shadow)) => apply_shadow_tuning_object(
                &mut node.shadow_strength,
                &mut node.shadow_depth_bias,
                &mut node.shadow_normal_bias,
                value,
            ),
            Some(NodeField::Light3D(Light3DField::ShadowStrength)) => {
                set_shadow_strength(&mut node.shadow_strength, value);
            }
            Some(NodeField::Light3D(Light3DField::ShadowDepthBias)) => {
                set_shadow_depth_bias(&mut node.shadow_depth_bias, value);
            }
            Some(NodeField::Light3D(Light3DField::ShadowNormalBias)) => {
                set_shadow_normal_bias(&mut node.shadow_normal_bias, value);
            }
            Some(NodeField::Light3D(Light3DField::Active)) => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            Some(NodeField::Light3D(Light3DField::RenderLayers)) => {
                if let Some(v) = as_bitmask(value) {
                    node.render_layers = v;
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
                    node.palette.day_colors = colors;
                }
            }
            Some(NodeField::Sky3D(Sky3DField::EveningColors)) => {
                if let Some(colors) = as_color_array(value) {
                    node.palette.evening_colors = colors;
                }
            }
            Some(NodeField::Sky3D(Sky3DField::NightColors)) => {
                if let Some(colors) = as_color_array(value) {
                    node.palette.night_colors = colors;
                }
            }
            Some(NodeField::Sky3D(Sky3DField::HorizonColors)) => {
                if let Some(colors) = as_color_array(value) {
                    node.palette.horizon_colors = colors;
                }
            }
            Some(NodeField::Sky3D(Sky3DField::Environment)) => {
                node.environment = as_sky_environment(value);
            }
            Some(NodeField::Sky3D(Sky3DField::Palette)) => {
                apply_sky_palette_fields(node, value);
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
            Some(NodeField::Sky3D(Sky3DField::Shaders)) => {
                if let Some(shaders) = as_sky_shaders(value) {
                    node.shaders = shaders;
                }
            }
            Some(NodeField::Sky3D(Sky3DField::Active)) => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            Some(NodeField::Sky3D(Sky3DField::RenderLayers)) => {
                if let Some(v) = as_bitmask(value) {
                    node.render_layers = v;
                }
            }
            _ => {}
        }
    });
}

fn as_sky_environment(value: &SceneValue) -> Option<perro_nodes::SkyEnvironment> {
    let SceneValue::Object(entries) = value else {
        return None;
    };
    let mut source = None;
    let mut intensity = 1.0;
    let mut rotation_degrees = 0.0;
    for (name, value) in entries.iter() {
        match name.as_ref() {
            "source" | "image" | "texture" => {
                let value = as_str(value)?.trim();
                if !value.is_empty() {
                    source = Some(Cow::Owned(value.to_string()));
                }
            }
            "intensity" | "energy" => intensity = as_f32(value)?.max(0.0),
            "rotation_degrees" | "rotation" => rotation_degrees = as_f32(value)?,
            _ => {}
        }
    }
    Some(perro_nodes::SkyEnvironment {
        source: source?,
        intensity,
        rotation_degrees,
    })
}

fn apply_sky_palette_fields(node: &mut Sky3D, value: &SceneValue) {
    let SceneValue::Object(entries) = value else {
        return;
    };
    for (name, value) in entries.iter() {
        match resolve_node_field("Sky3D", name) {
            Some(NodeField::Sky3D(Sky3DField::DayColors)) => {
                if let Some(colors) = as_color_array(value) {
                    node.palette.day_colors = colors;
                }
            }
            Some(NodeField::Sky3D(Sky3DField::EveningColors)) => {
                if let Some(colors) = as_color_array(value) {
                    node.palette.evening_colors = colors;
                }
            }
            Some(NodeField::Sky3D(Sky3DField::NightColors)) => {
                if let Some(colors) = as_color_array(value) {
                    node.palette.night_colors = colors;
                }
            }
            Some(NodeField::Sky3D(Sky3DField::HorizonColors)) => {
                if let Some(colors) = as_color_array(value) {
                    node.palette.horizon_colors = colors;
                }
            }
            _ => {}
        }
    }
}

fn as_sky_shaders(value: &SceneValue) -> Option<Vec<SkyShaderPass>> {
    match value {
        SceneValue::Array(items) => {
            let mut out = Vec::new();
            for item in items.as_ref() {
                out.push(sky_shader_pass_from(item)?);
            }
            Some(out)
        }
        _ => None,
    }
}

fn sky_shader_pass_from(value: &SceneValue) -> Option<SkyShaderPass> {
    let SceneValue::Object(entries) = value else {
        return None;
    };
    let mut path: Option<Cow<'static, str>> = None;
    let mut params = Vec::new();
    for (k, v) in entries.as_ref() {
        match k.as_ref() {
            "path" | "shader" | "shader_path" => {
                if let Some(s) = as_str(v) {
                    let s = s.trim();
                    if !s.is_empty() {
                        path = Some(Cow::Owned(s.to_string()));
                    }
                }
            }
            "params" => {
                params = as_post_params(v)?;
            }
            _ => {}
        }
    }
    let mut pass = SkyShaderPass::new(path?);
    pass.params = params;
    Some(pass)
}

fn apply_spot_light_3d_fields(node: &mut SpotLight3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("SpotLight3D", name) {
            Some(NodeField::Light3D(Light3DField::Color)) => {
                if let Some(v) = as_light_color(value) {
                    node.color = v;
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
            Some(NodeField::Light3D(Light3DField::CastShadows)) => {
                if let Some(v) = as_bool(value) {
                    node.cast_shadows = v;
                }
            }
            Some(NodeField::Light3D(Light3DField::Shadow)) => apply_shadow_tuning_object(
                &mut node.shadow_strength,
                &mut node.shadow_depth_bias,
                &mut node.shadow_normal_bias,
                value,
            ),
            Some(NodeField::Light3D(Light3DField::ShadowStrength)) => {
                set_shadow_strength(&mut node.shadow_strength, value);
            }
            Some(NodeField::Light3D(Light3DField::ShadowDepthBias)) => {
                set_shadow_depth_bias(&mut node.shadow_depth_bias, value);
            }
            Some(NodeField::Light3D(Light3DField::ShadowNormalBias)) => {
                set_shadow_normal_bias(&mut node.shadow_normal_bias, value);
            }
            Some(NodeField::Light3D(Light3DField::Active)) => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            Some(NodeField::Light3D(Light3DField::RenderLayers)) => {
                if let Some(v) = as_bitmask(value) {
                    node.render_layers = v;
                }
            }
            _ => {}
        }
    });
}

fn apply_shadow_tuning_object(
    strength: &mut f32,
    depth_bias: &mut f32,
    normal_bias: &mut f32,
    value: &SceneValue,
) {
    let SceneValue::Object(entries) = value else {
        return;
    };
    for (name, value) in entries.iter() {
        match name.as_str() {
            "strength" | "shadow_strength" | "opacity" | "shadow_opacity" => {
                set_shadow_strength(strength, value);
            }
            "depth_bias" | "bias" | "shadow_depth_bias" | "shadow_bias" => {
                set_shadow_depth_bias(depth_bias, value);
            }
            "normal_bias" | "normal" | "shadow_normal_bias" => {
                set_shadow_normal_bias(normal_bias, value);
            }
            _ => {}
        }
    }
}

fn set_shadow_strength(out: &mut f32, value: &SceneValue) {
    if let Some(v) = as_f32(value) {
        *out = v;
    }
}

fn set_shadow_depth_bias(out: &mut f32, value: &SceneValue) {
    if let Some(v) = as_f32(value) {
        *out = v;
    }
}

fn set_shadow_normal_bias(out: &mut f32, value: &SceneValue) {
    if let Some(v) = as_f32(value) {
        *out = v;
    }
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

#[cfg(test)]
mod lights_tests {
    use super::*;
    use perro_scene::SceneFieldName;

    #[test]
    fn sky_environment_object_parses_and_clamps_intensity() {
        let value = SceneValue::Object(Cow::Owned(vec![
            (
                SceneFieldName::Source,
                SceneValue::Str(Cow::Borrowed("res://studio.png")),
            ),
            (SceneFieldName::Intensity, SceneValue::F32(-2.0)),
            (
                SceneFieldName::Custom(Cow::Borrowed("rotation_degrees")),
                SceneValue::F32(45.0),
            ),
        ]));
        let environment = as_sky_environment(&value).expect("valid environment");
        assert_eq!(environment.source, "res://studio.png");
        assert_eq!(environment.intensity, 0.0);
        assert_eq!(environment.rotation_degrees, 45.0);
    }
}
