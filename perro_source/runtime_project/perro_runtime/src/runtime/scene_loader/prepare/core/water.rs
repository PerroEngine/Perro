use super::*;

pub(super) fn apply_water_body_fields(node: &mut WaterSurfaceParams, ty: &str, fields: &[SceneObjectField]) {
    // `quality` owns the readback-rate default, so an explicit
    // `sample_readback_rate` has to win no matter which order they appear in.
    let mut explicit_readback_rate = None;
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        let field = match resolve_node_field(ty, name) {
            Some(NodeField::WaterBody2D(field)) | Some(NodeField::WaterBody3D(field)) => field,
            _ => {
                warn_removed_water_field(ty, name);
                return;
            }
        };
        match field {
            WaterBodyField::Shape => match ty {
                "WaterBody3D" => {
                    if let Some(shape) = as_shape_3d(value).and_then(water_shape_from_shape_3d) {
                        node.shape = shape;
                        node.depth = shape.depth(node.depth);
                    }
                }
                _ => {
                    if let Some(shape) = as_shape_2d(value).and_then(water_shape_from_shape_2d) {
                        node.shape = shape;
                    }
                }
            },
            WaterBodyField::Quality => {
                if let Some(quality) = as_water_quality(value) {
                    node.quality = quality;
                    node.physics.sample_readback_rate = quality.sample_readback_rate();
                }
            }
            WaterBodyField::Depth => {
                if let Some(v) = as_f32(value) {
                    node.depth = v.max(0.0);
                    if let WaterShape::Box { size } = node.shape {
                        node.shape = WaterShape::box_volume(Vector3::new(
                            size.x,
                            node.depth.max(0.001),
                            size.z,
                        ));
                    }
                }
            }
            WaterBodyField::Flow => {
                if let Some(v) = as_vec2(value) {
                    node.flow = v;
                }
            }
            WaterBodyField::Wind => {
                if let Some(v) = as_vec2(value) {
                    node.wind = v;
                }
            }
            WaterBodyField::IdleMode => {
                if let Some(v) = as_water_idle_mode(value) {
                    node.idle_mode = v;
                }
            }
            WaterBodyField::WaveSpeed => {
                if let Some(v) = as_f32(value) {
                    node.wave.speed = v.max(0.0);
                }
            }
            WaterBodyField::WaveScale => {
                if let Some(v) = as_f32(value) {
                    node.wave.scale = v.max(0.0);
                }
            }
            WaterBodyField::WaveLength => {
                if let Some(v) = as_f32(value) {
                    node.wave.length = v.max(0.001);
                }
            }
            WaterBodyField::WakeStrength => {
                if let Some(v) = as_f32(value) {
                    node.physics.wake_strength = v.max(0.0);
                }
            }
            WaterBodyField::FoamStrength => {
                if let Some(v) = as_f32(value) {
                    node.physics.foam_strength = v.max(0.0);
                }
            }
            WaterBodyField::Damping => {
                if let Some(v) = as_f32(value) {
                    node.wave.damping = v.clamp(0.0, 1.0);
                }
            }
            WaterBodyField::Buoyancy => {
                if let Some(v) = as_f32(value) {
                    node.physics.buoyancy = v.max(0.0);
                }
            }
            WaterBodyField::Drag => {
                if let Some(v) = as_f32(value) {
                    node.physics.drag = v.max(0.0);
                }
            }
            WaterBodyField::SampleReadbackRate => {
                if let Some(v) = as_f32(value) {
                    explicit_readback_rate = Some(v.max(0.0));
                }
            }
            WaterBodyField::CollisionLayers => {
                if let Some(v) = as_bitmask(value) {
                    node.collision_layers = v;
                }
            }
            WaterBodyField::CollisionMask => {
                if let Some(v) = as_bitmask(value) {
                    node.collision_mask = v;
                }
            }
            WaterBodyField::LinkLayers => {
                if let Some(v) = as_bitmask(value) {
                    node.link.link_layers = v;
                }
            }
            WaterBodyField::LinkMask => {
                if let Some(v) = as_bitmask(value) {
                    node.link.link_mask = v;
                }
            }
            WaterBodyField::BlendWidth => {
                if let Some(v) = as_f32(value) {
                    node.link.blend_width = v.max(0.0);
                }
            }
            WaterBodyField::WaveTransfer => {
                if let Some(v) = as_f32(value) {
                    node.link.wave_transfer = v.max(0.0);
                }
            }
            WaterBodyField::FlowTransfer => {
                if let Some(v) = as_f32(value) {
                    node.link.flow_transfer = v.max(0.0);
                }
            }
            WaterBodyField::DeepColor => {
                if let Some(v) = as_color(value) {
                    node.optics.deep_color = v;
                }
            }
            WaterBodyField::ShallowColor => {
                if let Some(v) = as_color(value) {
                    node.optics.shallow_color = v;
                }
            }
            WaterBodyField::ShallowDepth => {
                if let Some(v) = as_f32(value) {
                    node.optics.shallow_depth = v.max(-1.0);
                }
            }
            WaterBodyField::SkyBias => {
                if let Some(v) = as_water_sky_bias(value) {
                    node.optics.sky_bias = v;
                }
            }
            WaterBodyField::Optics => {
                apply_water_optics_settings(&mut node.optics, value);
            }
            WaterBodyField::Material => {
                apply_water_visual_params(&mut node.visual, value);
            }
            WaterBodyField::Transparency => {
                if let Some(v) = as_f32(value) {
                    node.visual.transparency = v.clamp(0.0, 1.0);
                }
            }
            WaterBodyField::Reflectivity => {
                if let Some(v) = as_f32(value) {
                    node.visual.reflectivity = v.clamp(0.0, 1.0);
                }
            }
            WaterBodyField::Roughness => {
                if let Some(v) = as_f32(value) {
                    node.visual.roughness = v.clamp(0.0, 1.0);
                }
            }
            WaterBodyField::FresnelPower => {
                if let Some(v) = as_f32(value) {
                    node.visual.fresnel_power = v.max(0.001);
                }
            }
            WaterBodyField::NormalStrength => {
                if let Some(v) = as_f32(value) {
                    node.visual.normal_strength = v.max(0.0);
                }
            }
            WaterBodyField::RippleScale => {
                if let Some(v) = as_f32(value) {
                    node.visual.ripple_scale = v.max(0.001);
                }
            }
            WaterBodyField::FoamColor => {
                if let Some(v) = as_color(value) {
                    node.visual.foam_color = v;
                }
            }
            WaterBodyField::FoamAmount => {
                if let Some(v) = as_f32(value) {
                    node.visual.foam_amount = v.max(0.0);
                }
            }
            WaterBodyField::CrestFoamThreshold => {
                if let Some(v) = as_f32(value) {
                    node.visual.crest_foam_threshold = v.max(0.0);
                }
            }
            WaterBodyField::CausticStrength => {
                if let Some(v) = as_f32(value) {
                    node.visual.caustic_strength = v.max(0.0);
                }
            }
            WaterBodyField::RefractionStrength => {
                if let Some(v) = as_f32(value) {
                    node.visual.refraction_strength = v.max(0.0);
                }
            }
            WaterBodyField::ScatteringStrength => {
                if let Some(v) = as_f32(value) {
                    node.visual.scattering_strength = v.max(0.0);
                }
            }
            WaterBodyField::DistanceFogStrength => {
                if let Some(v) = as_f32(value) {
                    node.visual.distance_fog_strength = v.max(0.0);
                }
            }
            WaterBodyField::Coastline => {
                apply_coastline_settings(&mut node.coastline, value);
            }
            WaterBodyField::Debug => {
                if let Some(v) = as_bool(value) {
                    node.debug = v;
                }
            }
        }
    });
    if let Some(rate) = explicit_readback_rate {
        node.physics.sample_readback_rate = rate;
    }
}

/// Loud one-line deprecation for a removed water fidelity knob. Scene load
/// cannot fail on a bad field, so the runtime warns and `perro doctor` errors.
fn warn_removed_water_field(ty: &str, name: &str) {
    if !matches!(ty, "WaterBody2D" | "WaterBody3D") {
        return;
    }
    let Some(detail) = perro_scene::water_body_removed_field(name) else {
        return;
    };
    eprintln!("[perro][runtime] {ty}.{name}: {detail}");
}

pub(super) fn as_water_quality(value: &SceneValue) -> Option<WaterQuality> {
    WaterQuality::from_name(as_str(value)?)
}

pub(super) fn water_shape_from_shape_2d(shape: Shape2D) -> Option<WaterShape> {
    match shape {
        Shape2D::Quad { width, height } => Some(WaterShape::rect(Vector2::new(
            width.max(0.001),
            height.max(0.001),
        ))),
        Shape2D::Circle { radius } => Some(WaterShape::Circle {
            radius: radius.max(0.001),
        }),
        Shape2D::Triangle { .. } => None,
    }
}

pub(super) fn water_shape_from_shape_3d(shape: Shape3D) -> Option<WaterShape> {
    match shape {
        Shape3D::Cube { size } => Some(WaterShape::box_volume(Vector3::new(
            size.x.max(0.001),
            size.y.max(0.001),
            size.z.max(0.001),
        ))),
        Shape3D::Cylinder {
            radius,
            half_height,
        } => Some(WaterShape::Cylinder {
            radius: radius.max(0.001),
            half_height: half_height.max(0.001),
        }),
        Shape3D::Sphere { radius } => Some(WaterShape::Cylinder {
            radius: radius.max(0.001),
            half_height: radius.max(0.001),
        }),
        _ => None,
    }
}

pub(super) fn apply_water_optics_settings(node: &mut perro_nodes::WaterOpticsSettings, value: &SceneValue) {
    let SceneValue::Object(fields) = value else {
        return;
    };
    for (name, value) in fields.iter() {
        match name.as_ref() {
            "deep_color" | "deep" => {
                if let Some(v) = as_color(value) {
                    node.deep_color = v;
                }
            }
            "shallow_color" | "shallow" => {
                if let Some(v) = as_color(value) {
                    node.shallow_color = v;
                }
            }
            "shallow_depth" | "shallow_cutoff" | "shallowness" | "shallowness_depth" => {
                if let Some(v) = as_f32(value) {
                    node.shallow_depth = v.max(-1.0);
                }
            }
            "sky_bias" | "sky_reflect" | "sky_reflection" => {
                if let Some(v) = as_water_sky_bias(value) {
                    node.sky_bias = v;
                }
            }
            _ => {}
        }
    }
}

pub(super) fn apply_water_visual_params(node: &mut perro_nodes::WaterVisualParams, value: &SceneValue) {
    let SceneValue::Object(fields) = value else {
        return;
    };
    for (name, value) in fields.iter() {
        match name.as_ref() {
            "transparency" => {
                if let Some(v) = as_f32(value) {
                    node.transparency = v.clamp(0.0, 1.0);
                }
            }
            "reflectivity" | "reflection_strength" => {
                if let Some(v) = as_f32(value) {
                    node.reflectivity = v.clamp(0.0, 1.0);
                }
            }
            "roughness" => {
                if let Some(v) = as_f32(value) {
                    node.roughness = v.clamp(0.0, 1.0);
                }
            }
            "fresnel_power" => {
                if let Some(v) = as_f32(value) {
                    node.fresnel_power = v.max(0.001);
                }
            }
            "normal_strength" => {
                if let Some(v) = as_f32(value) {
                    node.normal_strength = v.max(0.0);
                }
            }
            "ripple_scale" => {
                if let Some(v) = as_f32(value) {
                    node.ripple_scale = v.max(0.001);
                }
            }
            "foam_color" => {
                if let Some(v) = as_color(value) {
                    node.foam_color = v;
                }
            }
            "foam_amount" => {
                if let Some(v) = as_f32(value) {
                    node.foam_amount = v.max(0.0);
                }
            }
            "crest_foam_threshold" => {
                if let Some(v) = as_f32(value) {
                    node.crest_foam_threshold = v.max(0.0);
                }
            }
            "caustic_strength" => {
                if let Some(v) = as_f32(value) {
                    node.caustic_strength = v.max(0.0);
                }
            }
            "refraction_strength" => {
                if let Some(v) = as_f32(value) {
                    node.refraction_strength = v.max(0.0);
                }
            }
            "scattering_strength" => {
                if let Some(v) = as_f32(value) {
                    node.scattering_strength = v.max(0.0);
                }
            }
            "distance_fog_strength" => {
                if let Some(v) = as_f32(value) {
                    node.distance_fog_strength = v.max(0.0);
                }
            }
            _ => {}
        }
    }
}

pub(super) fn apply_coastline_settings(node: &mut perro_nodes::CoastlineSettings, value: &SceneValue) {
    let SceneValue::Object(fields) = value else {
        return;
    };
    for (name, value) in fields.iter() {
        match name.as_ref() {
            "foam_color" => {
                if let Some(v) = as_color(value) {
                    node.foam_color = v;
                }
            }
            "foam_strength" => {
                if let Some(v) = as_f32(value) {
                    node.foam_strength = v.max(0.0);
                }
            }
            "foam_width" => {
                if let Some(v) = as_f32(value) {
                    node.foam_width = v.max(0.0);
                }
            }
            "cutoff_softness" => {
                if let Some(v) = as_f32(value) {
                    node.cutoff_softness = v.max(0.0);
                }
            }
            "wave_reflection" => {
                if let Some(v) = as_f32(value) {
                    node.wave_reflection = v.clamp(0.0, 1.0);
                }
            }
            "wave_damping" => {
                if let Some(v) = as_f32(value) {
                    node.wave_damping = v.clamp(0.0, 1.0);
                }
            }
            "edge_noise" => {
                if let Some(v) = as_f32(value) {
                    node.edge_noise = v.max(0.0);
                }
            }
            _ => {}
        }
    }
}

pub(super) fn as_water_sky_bias(value: &SceneValue) -> Option<WaterSkyBias> {
    if let Some(v) = as_f32(value) {
        return if v <= 0.0 {
            Some(WaterSkyBias::None)
        } else {
            Some(WaterSkyBias::Active {
                ratio: v.clamp(0.0, 1.0),
            })
        };
    }
    if let Some(v) = as_str(value) {
        return match v.trim().to_ascii_lowercase().as_str() {
            "none" | "off" | "false" => Some(WaterSkyBias::None),
            "active" | "sky" | "on" | "true" => Some(WaterSkyBias::Active { ratio: 0.35 }),
            _ => None,
        };
    }
    let SceneValue::Object(fields) = value else {
        return None;
    };
    let mut ratio = 0.35;
    let mut active = true;
    for (name, value) in fields.iter() {
        match name.as_ref() {
            "ratio" | "strength" | "amount" => {
                if let Some(v) = as_f32(value) {
                    ratio = v.clamp(0.0, 1.0);
                }
            }
            "active" => {
                if let Some(v) = as_bool(value) {
                    active = v;
                }
            }
            "mode" | "type" => {
                if let Some(v) = as_str(value)
                    && matches!(v.trim().to_ascii_lowercase().as_str(), "none" | "off")
                {
                    active = false;
                }
            }
            _ => {}
        }
    }
    if active && ratio > 0.0 {
        Some(WaterSkyBias::Active { ratio })
    } else {
        Some(WaterSkyBias::None)
    }
}

pub(super) fn as_color(value: &SceneValue) -> Option<Color> {
    match value {
        SceneValue::Vec4 { x, y, z, w } => Some(Color::new(*x, *y, *z, *w)),
        SceneValue::Vec3 { x, y, z } => Some(Color::rgb(*x, *y, *z)),
        SceneValue::Str(v) => Color::from_hex(v.as_ref()),
        SceneValue::Key(v) => Color::from_hex(v.as_ref()),
        _ => None,
    }
}

pub(super) fn as_water_idle_mode(value: &SceneValue) -> Option<WaterIdleMode> {
    match as_str(value)?.trim().to_ascii_lowercase().as_str() {
        "calm" => Some(WaterIdleMode::Calm),
        "sine" => Some(WaterIdleMode::Sine),
        "chop" | "choppy" => Some(WaterIdleMode::Chop),
        "storm" => Some(WaterIdleMode::Storm),
        "river" => Some(WaterIdleMode::River),
        _ => None,
    }
}

pub(super) fn as_force_profile(value: &SceneValue) -> Option<PhysicsForceProfile> {
    match as_str(value)?.trim().to_ascii_lowercase().as_str() {
        "lift" => Some(PhysicsForceProfile::Lift),
        "explosion" => Some(PhysicsForceProfile::Explosion),
        "current" => Some(PhysicsForceProfile::Current),
        "vortex" => Some(PhysicsForceProfile::Vortex),
        "custom" => Some(PhysicsForceProfile::Custom),
        _ => None,
    }
}

pub(super) fn as_vec2_array(value: &SceneValue) -> Option<Vec<Vector2>> {
    let SceneValue::Array(items) = value else {
        return None;
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items.iter() {
        out.push(as_vec2(item)?);
    }
    Some(out)
}

pub(super) fn as_vec3_array(value: &SceneValue) -> Option<Vec<Vector3>> {
    let SceneValue::Array(items) = value else {
        return None;
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items.iter() {
        out.push(as_vec3(item)?);
    }
    Some(out)
}
