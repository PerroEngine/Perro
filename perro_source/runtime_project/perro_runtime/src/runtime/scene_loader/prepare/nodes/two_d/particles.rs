fn build_particle_emitter_2d(data: &SceneDefNodeData) -> ParticleEmitter2D {
    let mut node = ParticleEmitter2D::default();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_particle_emitter_2d_fields(&mut node, &data.fields);
    node
}

fn apply_particle_emitter_2d_fields(node: &mut ParticleEmitter2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("ParticleEmitter2D", name) {
            Some(NodeField::ParticleEmitter2D(ParticleEmitter2DField::Active)) => {
                if let Some(v) = value.as_bool() {
                    node.active = v;
                }
            }
            Some(NodeField::ParticleEmitter2D(ParticleEmitter2DField::Looping)) => {
                if let Some(v) = value.as_bool() {
                    node.looping = v;
                }
            }
            Some(NodeField::ParticleEmitter2D(ParticleEmitter2DField::Prewarm)) => {
                if let Some(v) = value.as_bool() {
                    node.prewarm = v;
                }
            }
            Some(NodeField::ParticleEmitter2D(ParticleEmitter2DField::SpawnRate)) => {
                if let Some(v) = value.as_f32() {
                    node.spawn_rate = v.max(0.0);
                }
            }
            Some(NodeField::ParticleEmitter2D(ParticleEmitter2DField::Seed)) => {
                if let Some(v) = as_i32(value) {
                    node.seed = v.max(0) as u32;
                }
            }
            Some(NodeField::ParticleEmitter2D(ParticleEmitter2DField::Params)) => {
                if let Some(v) = as_particle_params(value) {
                    node.params = v;
                }
            }
            Some(NodeField::ParticleEmitter2D(ParticleEmitter2DField::Profile)) => {
                if let Some(path) = as_str(value) {
                    node.profile = path.to_string();
                } else if let SceneValue::Object(entries) = value {
                    node.profile = inline_pparticle(entries);
                }
            }
            Some(NodeField::ParticleEmitter2D(ParticleEmitter2DField::SimMode)) => {
                if let Some(v) = as_particle_sim_mode_2d(value) {
                    node.sim_mode = v;
                }
            }
            _ => {}
        }
    });
}

fn as_particle_sim_mode_2d(value: &SceneValue) -> Option<ParticleEmitterSimMode2D> {
    match as_str(value)?.trim().to_ascii_lowercase().as_str() {
        "default" => Some(ParticleEmitterSimMode2D::Default),
        "cpu" => Some(ParticleEmitterSimMode2D::Cpu),
        _ => None,
    }
}
