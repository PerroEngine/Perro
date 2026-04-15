fn build_particle_emitter_3d(data: &SceneDefNodeData) -> ParticleEmitter3D {
    let mut node = ParticleEmitter3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_particle_emitter_3d_fields(&mut node, &data.fields);
    node
}

fn apply_particle_emitter_3d_fields(node: &mut ParticleEmitter3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("ParticleEmitter3D", name) {
            Some(NodeField::ParticleEmitter3D(ParticleEmitter3DField::Active)) => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            Some(NodeField::ParticleEmitter3D(ParticleEmitter3DField::Looping)) => {
                if let Some(v) = as_bool(value) {
                    node.looping = v;
                }
            }
            Some(NodeField::ParticleEmitter3D(ParticleEmitter3DField::Prewarm)) => {
                if let Some(v) = as_bool(value) {
                    node.prewarm = v;
                }
            }
            Some(NodeField::ParticleEmitter3D(ParticleEmitter3DField::SpawnRate)) => {
                if let Some(v) = as_f32(value) {
                    node.spawn_rate = v.max(0.0);
                }
            }
            Some(NodeField::ParticleEmitter3D(ParticleEmitter3DField::Seed)) => {
                if let Some(v) = as_i32(value) {
                    node.seed = v.max(0) as u32;
                }
            }
            Some(NodeField::ParticleEmitter3D(ParticleEmitter3DField::Params)) => {
                if let Some(v) = as_particle_params(value) {
                    node.params = v;
                }
            }
            Some(NodeField::ParticleEmitter3D(ParticleEmitter3DField::Profile)) => {
                if let Some(v) = as_asset_source(value) {
                    node.profile = v;
                } else if let SceneValue::Object(entries) = value {
                    node.profile = inline_pparticle(entries);
                }
            }
            Some(NodeField::ParticleEmitter3D(ParticleEmitter3DField::SimMode)) => {
                if let Some(v) = as_particle_sim_mode(value) {
                    node.sim_mode = v;
                }
            }
            Some(NodeField::ParticleEmitter3D(ParticleEmitter3DField::RenderMode)) => {
                if let Some(v) = as_particle_render_mode(value) {
                    node.render_mode = v;
                }
            }
            _ => {}
        }
    });
}

fn inline_pparticle(entries: &[SceneObjectField]) -> String {
    let mut out = String::from("inline://");
    for (key, value) in entries {
        if let Some(encoded) = encode_value_for_pparticle(value) {
            out.push_str(key);
            out.push_str(" = ");
            out.push_str(&encoded);
            out.push('\n');
        }
    }
    out
}

fn encode_value_for_pparticle(value: &SceneValue) -> Option<String> {
    if let Some(v) = value.as_const_param() {
        return Some(match v {
            perro_structs::ConstParamValue::Bool(v) => {
                if v { "true" } else { "false" }.to_string()
            }
            perro_structs::ConstParamValue::I32(v) => v.to_string(),
            perro_structs::ConstParamValue::F32(v) => v.to_string(),
            perro_structs::ConstParamValue::Vec2(v) => format!("({}, {})", v[0], v[1]),
            perro_structs::ConstParamValue::Vec3(v) => format!("({}, {}, {})", v[0], v[1], v[2]),
            perro_structs::ConstParamValue::Vec4(v) => {
                format!("({}, {}, {}, {})", v[0], v[1], v[2], v[3])
            }
        });
    }
    match value {
        SceneValue::Str(v) => Some(v.to_string()),
        SceneValue::Hashed(v) => Some(v.to_string()),
        SceneValue::Key(v) => Some(v.to_string()),
        SceneValue::Object(_) | SceneValue::Array(_) => None,
        _ => None,
    }
}

fn as_particle_sim_mode(value: &SceneValue) -> Option<ParticleEmitterSimMode3D> {
    let raw = as_str(value)?.trim().to_ascii_lowercase();
    match raw.as_ref() {
        "default" => Some(ParticleEmitterSimMode3D::Default),
        "cpu" => Some(ParticleEmitterSimMode3D::Cpu),
        "hybrid" => Some(ParticleEmitterSimMode3D::GpuVertex),
        "gpu" => Some(ParticleEmitterSimMode3D::GpuCompute),
        _ => None,
    }
}

fn as_particle_render_mode(value: &SceneValue) -> Option<ParticleType> {
    let raw = as_str(value)?.trim().to_ascii_lowercase();
    match raw.as_ref() {
        "point" => Some(ParticleType::Point),
        "billboard" => Some(ParticleType::Billboard),
        _ => None,
    }
}

fn as_particle_params(value: &SceneValue) -> Option<Vec<f32>> {
    if let Some(v) = value.as_const_param() {
        return match v {
            perro_structs::ConstParamValue::F32(n) => Some(vec![n]),
            perro_structs::ConstParamValue::I32(n) => Some(vec![n as f32]),
            perro_structs::ConstParamValue::Vec2(v) => Some(v.to_vec()),
            perro_structs::ConstParamValue::Vec3(v) => Some(v.to_vec()),
            perro_structs::ConstParamValue::Vec4(v) => Some(v.to_vec()),
            perro_structs::ConstParamValue::Bool(_) => None,
        };
    }
    match value {
        SceneValue::Object(entries) => {
            let mut indexed = Vec::<(usize, f32)>::new();
            for (k, v) in entries.as_ref() {
                let idx = parse_param_key_index(k)?;
                let val = match v.as_const_param() {
                    Some(perro_structs::ConstParamValue::F32(n)) => n,
                    Some(perro_structs::ConstParamValue::I32(n)) => n as f32,
                    _ => return None,
                };
                indexed.push((idx, val));
            }
            if indexed.is_empty() {
                return Some(Vec::new());
            }
            indexed.sort_unstable_by_key(|(i, _)| *i);
            let max = indexed.last().map(|(i, _)| *i).unwrap_or(0);
            let mut out = vec![0.0; max + 1];
            for (i, v) in indexed {
                out[i] = v;
            }
            Some(out)
        }
        _ => None,
    }
}
