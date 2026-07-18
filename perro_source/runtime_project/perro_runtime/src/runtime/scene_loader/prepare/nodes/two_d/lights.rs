fn as_light_color(value: &SceneValue) -> Option<Color> {
    match value {
        SceneValue::Vec4 { x, y, z, w } => Some(Color::new(*x, *y, *z, *w)),
        SceneValue::Vec3 { x, y, z } => Some(Color::rgb(*x, *y, *z)),
        SceneValue::Str(v) => Color::from_hex(v.as_ref()),
        SceneValue::Key(v) => Color::from_hex(v.as_ref()),
        SceneValue::Object(entries) => {
            let mut r = None;
            let mut g = None;
            let mut b = None;
            let mut a = Some(1.0);
            for (k, v) in entries.iter() {
                match k.as_ref() {
                    "r" | "x" => r = as_f32(v),
                    "g" | "y" => g = as_f32(v),
                    "b" | "z" => b = as_f32(v),
                    "a" | "w" => a = as_f32(v),
                    _ => {}
                }
            }
            Some(Color::new(r?, g?, b?, a?))
        }
        _ => None,
    }
}

define_scene_node_builder! {
    fn build_ambient_light_2d -> AmbientLight2D = AmbientLight2D::new();
    base none;
    apply [apply_ambient_light_2d_fields];
}

define_scene_node_builder! {
    fn build_ray_light_2d -> RayLight2D = RayLight2D::new();
    base node_2d;
    apply [apply_ray_light_2d_fields];
}

define_scene_node_builder! {
    fn build_point_light_2d -> PointLight2D = PointLight2D::new();
    base node_2d;
    apply [apply_point_light_2d_fields];
}

define_scene_node_builder! {
    fn build_spot_light_2d -> SpotLight2D = SpotLight2D::new();
    base node_2d;
    apply [apply_spot_light_2d_fields];
}

fn apply_ambient_light_2d_fields(node: &mut AmbientLight2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        apply_light_2d_common("AmbientLight2D", name, value, |field| match field {
            Light2DField::Color => {
                if let Some(v) = as_light_color(value) {
                    node.color = v;
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
            Light2DField::ShadowSoftness | Light2DField::ShadowSamples => {}
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
                if let Some(v) = as_light_color(value) {
                    node.color = v;
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
            Light2DField::ShadowSoftness => {
                if let Some(v) = as_f32(value).filter(|v| v.is_finite()) {
                    node.shadow_softness = v.clamp(0.0, 1.0);
                }
            }
            Light2DField::ShadowSamples => {
                if let Some(v) = as_u32(value) {
                    node.shadow_samples = v.clamp(1, 16);
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
                if let Some(v) = as_light_color(value) {
                    node.color = v;
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
            Some(NodeField::Light2D(Light2DField::ShadowSoftness)) => {
                if let Some(v) = as_f32(value).filter(|v| v.is_finite()) {
                    node.shadow_softness = v.clamp(0.0, 1.0);
                }
            }
            Some(NodeField::Light2D(Light2DField::ShadowSamples)) => {
                if let Some(v) = as_u32(value) {
                    node.shadow_samples = v.clamp(1, 16);
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
                if let Some(v) = as_light_color(value) {
                    node.color = v;
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
            Some(NodeField::Light2D(Light2DField::ShadowSoftness)) => {
                if let Some(v) = as_f32(value).filter(|v| v.is_finite()) {
                    node.shadow_softness = v.clamp(0.0, 1.0);
                }
            }
            Some(NodeField::Light2D(Light2DField::ShadowSamples)) => {
                if let Some(v) = as_u32(value) {
                    node.shadow_samples = v.clamp(1, 16);
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
