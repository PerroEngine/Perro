use super::*;

impl GpuPointParticles3D {
    pub(super) fn resolve_spawn_state(
        &mut self,
        node: NodeID,
        particle_key: u32,
        current_origin: [f32; 3],
        current_rotation: [f32; 4],
    ) -> ([f32; 3], [f32; 4]) {
        let per_particle = self.spawn_origin_cache.entry(node).or_default();
        let generation = self.spawn_origin_generation;
        let entry = per_particle
            .entry(particle_key)
            .or_insert(SpawnOriginEntry {
                origin: current_origin,
                rotation: current_rotation,
                last_seen_generation: generation,
            });
        entry.last_seen_generation = generation;
        (entry.origin, entry.rotation)
    }

    pub(super) fn push_emitter_particles(&mut self, node: NodeID, emitter: &PointParticles3DState) {
        if !emitter.active || emitter.emission_rate <= 0.0 {
            return;
        }
        let model = Mat4::from_cols_array_2d(&emitter.model);
        let current_origin = model.transform_point3(Vec3::ZERO);
        let (_, rot_raw, _) = model.to_scale_rotation_translation();
        let spawn_rot = if rot_raw.is_finite() && rot_raw.length_squared() > 1.0e-6 {
            rot_raw.normalize()
        } else {
            Quat::IDENTITY
        };
        let time = emitter.simulation_time.max(0.0);
        let sim_delta = emitter.simulation_delta.max(0.0);
        let life_min = emitter.lifetime_min.max(0.001);
        let life_max = emitter.lifetime_max.max(life_min);
        let max_alive_budget = emitter.alive_budget.max(1);
        if max_alive_budget == 0 {
            return;
        }
        let emit_count = emitter_emission_count(emitter, max_alive_budget);
        if emit_count == 0 {
            return;
        }
        let speed_min = emitter.speed_min.max(0.0);
        let speed_max = emitter.speed_max.max(speed_min);
        let size_min = emitter.size_min.max(0.01);
        let size_max = emitter.size_max.max(size_min);
        enum CustomEval<'a> {
            ProgramIds(usize, usize, usize),
            Ops(&'a [Op], &'a [Op], &'a [Op]),
        }

        let compiled_custom = if let (Some(x_ops), Some(y_ops), Some(z_ops)) = (
            emitter.profile.expr_x_ops.as_ref(),
            emitter.profile.expr_y_ops.as_ref(),
            emitter.profile.expr_z_ops.as_ref(),
        ) {
            Some(CustomEval::Ops(
                x_ops.as_ref(),
                y_ops.as_ref(),
                z_ops.as_ref(),
            ))
        } else {
            match &emitter.profile.path {
                ParticlePath3D::Custom {
                    expr_x,
                    expr_y,
                    expr_z,
                } => {
                    match (
                        self.get_or_compile_expr(expr_x),
                        self.get_or_compile_expr(expr_y),
                        self.get_or_compile_expr(expr_z),
                    ) {
                        (Some(x), Some(y), Some(z)) => Some(CustomEval::ProgramIds(x, y, z)),
                        _ => None,
                    }
                }
                ParticlePath3D::CustomCompiled {
                    expr_x_ops,
                    expr_y_ops,
                    expr_z_ops,
                } => Some(CustomEval::Ops(
                    expr_x_ops.as_ref(),
                    expr_y_ops.as_ref(),
                    expr_z_ops.as_ref(),
                )),
                _ => None,
            }
        };
        let billboard_mode = emitter.render_mode == ParticleRenderMode3D::Billboard;
        if billboard_mode {
            self.staged_billboards.reserve(emit_count as usize);
        } else {
            self.staged.reserve(emit_count as usize);
        }
        let prewarm_time = if emitter.looping && emitter.prewarm {
            time + life_max
        } else {
            time
        };
        let emission_rate = emitter.emission_rate.max(1.0e-6);
        let mut total_spawned = (prewarm_time * emission_rate).floor() as u32;
        if emitter.looping && emitter.prewarm {
            total_spawned = total_spawned.max(emit_count.saturating_sub(1));
        }

        for i in 0..emit_count {
            let spawn_index = if emitter.looping {
                let back = emit_count.saturating_sub(1).saturating_sub(i);
                total_spawned.saturating_sub(back)
            } else {
                i
            };
            let particle_key = spawn_index;
            let h0 = hash01(emitter.seed ^ particle_key);
            let h1 = hash01(emitter.seed.wrapping_add(0x9E37_79B9) ^ particle_key.wrapping_mul(3));
            let h2 = hash01(emitter.seed.wrapping_add(0x7F4A_7C15) ^ particle_key.wrapping_mul(7));
            let h3 = hash01(emitter.seed.wrapping_add(0x94D0_49BB) ^ particle_key.wrapping_mul(11));
            let life = life_min + (life_max - life_min) * h0;
            let spawn_t = (spawn_index as f32) / emission_rate;
            let local_t = prewarm_time - spawn_t;
            if !(0.0..=life).contains(&local_t) {
                continue;
            }
            let prev_local_t = (local_t - sim_delta).max(0.0);
            let age = (local_t / life).clamp(0.0, 1.0);
            let prev_age = (prev_local_t / life).clamp(0.0, 1.0);
            let speed = speed_min + (speed_max - speed_min) * h1;
            let spread = emitter.spread_radians * (h2 * 2.0 - 1.0);
            let (yaw_sin, yaw_cos) = (h0 * std::f32::consts::TAU).sin_cos();
            let (spread_sin, spread_cos) = spread.sin_cos();
            let dir_y = spread_cos - yaw_cos * spread_sin;
            let dir_z = spread_sin + yaw_cos * spread_cos;
            let dir = Vec3::new(yaw_sin, dir_y, dir_z).normalize_or_zero();
            let vel = dir * speed;
            let lifetime = life;
            let ring_u = ((particle_key as f32) * 0.618_033_95 + h3 * 0.123_456_7).fract();
            let index01 = if emit_count > 1 {
                (i as f32) / ((emit_count - 1) as f32)
            } else {
                0.0
            };
            let seed_value = particle_key as f32;
            let dir_arr = [dir.x, dir.y, dir.z];
            let vel_arr = [vel.x, vel.y, vel.z];
            let (spawn_origin, spawn_rotation) = self.resolve_spawn_state(
                node,
                particle_key,
                [current_origin.x, current_origin.y, current_origin.z],
                [spawn_rot.x, spawn_rot.y, spawn_rot.z, spawn_rot.w],
            );
            let origin = Vec3::from_array(spawn_origin);
            let spawn_rotation = Quat::from_xyzw(
                spawn_rotation[0],
                spawn_rotation[1],
                spawn_rotation[2],
                spawn_rotation[3],
            );
            let spawn_rotation =
                if spawn_rotation.is_finite() && spawn_rotation.length_squared() > 1.0e-6 {
                    spawn_rotation.normalize()
                } else {
                    Quat::IDENTITY
                };
            let emitter_pos = spawn_origin;
            let mut pos = origin;
            let mut prev_pos = origin;
            match &emitter.profile.path {
                ParticlePath3D::None => {}
                ParticlePath3D::Ballistic => {
                    pos += dir * speed * local_t;
                    prev_pos += dir * speed * prev_local_t;
                }
                ParticlePath3D::Spiral {
                    angular_velocity,
                    radius,
                } => {
                    let theta = local_t * *angular_velocity + h0 * std::f32::consts::TAU;
                    pos += Vec3::new(theta.cos() * *radius, 0.0, theta.sin() * *radius);
                    let prev_theta = prev_local_t * *angular_velocity + h0 * std::f32::consts::TAU;
                    prev_pos +=
                        Vec3::new(prev_theta.cos() * *radius, 0.0, prev_theta.sin() * *radius);
                }
                ParticlePath3D::OrbitY {
                    angular_velocity,
                    radius,
                } => {
                    let theta = local_t * *angular_velocity + h1 * std::f32::consts::TAU;
                    pos = origin
                        + Vec3::new(
                            theta.cos() * *radius,
                            pos.y - origin.y,
                            theta.sin() * *radius,
                        );
                    let prev_theta = prev_local_t * *angular_velocity + h1 * std::f32::consts::TAU;
                    prev_pos = origin
                        + Vec3::new(
                            prev_theta.cos() * *radius,
                            prev_pos.y - origin.y,
                            prev_theta.sin() * *radius,
                        );
                }
                ParticlePath3D::NoiseDrift {
                    amplitude,
                    frequency,
                } => {
                    let n = (local_t * *frequency + h2 * 37.0).sin();
                    let m = (local_t * *frequency * 1.37 + h1 * 17.0).cos();
                    pos += Vec3::new(n, m, n * m) * *amplitude;
                    let prev_n = (prev_local_t * *frequency + h2 * 37.0).sin();
                    let prev_m = (prev_local_t * *frequency * 1.37 + h1 * 17.0).cos();
                    prev_pos += Vec3::new(prev_n, prev_m, prev_n * prev_m) * *amplitude;
                }
                ParticlePath3D::FlatDisk { radius } => {
                    let seq = ((i as f32) + 0.5) / (emit_count.max(1) as f32);
                    let theta = (i as f32) * 2.399_963_1 + h3 * 0.35;
                    let radial = seq.sqrt();
                    let r = *radius * radial * age;
                    pos += Vec3::new(theta.cos() * r, 0.0, theta.sin() * r);
                    let prev_r = *radius * radial * prev_age;
                    prev_pos += Vec3::new(theta.cos() * prev_r, 0.0, theta.sin() * prev_r);
                }
                ParticlePath3D::Custom { .. } | ParticlePath3D::CustomCompiled { .. } => {}
            }
            let force = Vec3::from_array(emitter.gravity);
            pos += 0.5 * force * local_t * local_t;
            prev_pos += 0.5 * force * prev_local_t * prev_local_t;
            let prev_pos_arr = [prev_pos.x, prev_pos.y, prev_pos.z];
            if let Some(custom_eval) = &compiled_custom {
                let eval_input = ParticleEvalInput {
                    t: age,
                    life: local_t,
                    lifetime,
                    spawn_time: spawn_t,
                    emitter_time: time,
                    speed,
                    particle_id: particle_key as f32,
                    dir: dir_arr,
                    vel: vel_arr,
                    rand: [h0, h1, h2],
                    seed: seed_value,
                    ring_u,
                    index01,
                    emitter_pos,
                    prev_pos: prev_pos_arr,
                    params: &emitter.params,
                };
                let (dx, dy, dz) = match *custom_eval {
                    CustomEval::ProgramIds(x_id, y_id, z_id) => (
                        self.eval_compiled_expr(x_id, &eval_input).unwrap_or(0.0),
                        self.eval_compiled_expr(y_id, &eval_input).unwrap_or(0.0),
                        self.eval_compiled_expr(z_id, &eval_input).unwrap_or(0.0),
                    ),
                    CustomEval::Ops(x_ops, y_ops, z_ops) => (
                        eval_ops_particle(x_ops, &eval_input, &mut self.eval_stack).unwrap_or(0.0),
                        eval_ops_particle(y_ops, &eval_input, &mut self.eval_stack).unwrap_or(0.0),
                        eval_ops_particle(z_ops, &eval_input, &mut self.eval_stack).unwrap_or(0.0),
                    ),
                };
                pos += Vec3::new(dx, dy, dz);
            }
            if emitter.profile.spin_angular_velocity.abs() > 1.0e-6 {
                let rel = pos - origin;
                let theta = emitter.profile.spin_angular_velocity * local_t;
                let (s, c) = theta.sin_cos();
                let spun = Vec3::new(rel.x * c - rel.z * s, rel.y, rel.x * s + rel.z * c);
                pos = origin + spun;
            }
            pos = origin + spawn_rotation * (pos - origin);
            let size = emitter.size * (size_min + (size_max - size_min) * h2);
            let color = lerp4(emitter.color_start.into(), emitter.color_end.into(), age);
            let particle = PointParticleGpu {
                world_pos: pos.to_array(),
                color: pack_unorm8x4(color),
                size_alpha: pack_f16x2([size, color[3]]),
                emissive: pack_f16x4([
                    emitter.emissive[0],
                    emitter.emissive[1],
                    emitter.emissive[2],
                    0.0,
                ]),
            };
            if billboard_mode {
                self.staged_billboards.push(particle);
            } else {
                self.staged.push(particle);
            }
        }
    }

    pub(super) fn push_hybrid_emitter_particles(
        &mut self,
        node: NodeID,
        emitter: &PointParticles3DState,
    ) -> bool {
        if !emitter.active || emitter.emission_rate <= 0.0 {
            return true;
        }
        if emitter.profile.expr_x_ops.is_some()
            || emitter.profile.expr_y_ops.is_some()
            || emitter.profile.expr_z_ops.is_some()
        {
            return false;
        }
        let Some((path_kind, path_a, path_b)) = gpu_path_params(&emitter.profile.path) else {
            return false;
        };

        let life_min = emitter.lifetime_min.max(0.001);
        let life_max = emitter.lifetime_max.max(life_min);
        let max_alive_budget = emitter.alive_budget.max(1);
        let mut emit_count = emitter_emission_count(emitter, max_alive_budget);
        if emit_count == 0 {
            return true;
        }
        if self.hybrid_particle_count > u32::MAX - emit_count {
            emit_count = u32::MAX - self.hybrid_particle_count;
        }
        if emit_count == 0 {
            return true;
        }
        let model = Mat4::from_cols_array_2d(&emitter.model);
        let current_origin = model.transform_point3(Vec3::ZERO);
        let (_, rot_raw, _) = model.to_scale_rotation_translation();
        let spawn_rot = if rot_raw.is_finite() && rot_raw.length_squared() > 1.0e-6 {
            rot_raw.normalize()
        } else {
            Quat::IDENTITY
        };
        let spawn_rot_arr = [spawn_rot.x, spawn_rot.y, spawn_rot.z, spawn_rot.w];
        let time = emitter.simulation_time.max(0.0);
        let prewarm_time = if emitter.looping && emitter.prewarm {
            time + life_max
        } else {
            time
        };
        let emission_rate = emitter.emission_rate.max(1.0e-6);
        let mut total_spawned = (prewarm_time * emission_rate).floor() as u32;
        if emitter.looping && emitter.prewarm {
            total_spawned = total_spawned.max(emit_count.saturating_sub(1));
        }
        let particle_start = self.hybrid_particle_count;
        let emitter_index = self.hybrid_emitters.len() as u32;
        append_emitter_map_entries(
            &mut self.hybrid_particle_emitter_map,
            emitter_index,
            emit_count,
            &mut self.hybrid_map_fingerprint,
        );
        let spawn_origin_capacity = max_alive_budget.max(1);
        let mut spawn_origin_updates = Vec::<(u32, [f32; 3], [f32; 4])>::new();
        let spawn_origin_base = {
            let entry = self.hybrid_spawn_rings.entry(node).or_insert_with(|| {
                let base = self.hybrid_particle_spawn_origins.len() as u32;
                self.hybrid_particle_spawn_origins
                    .resize((base + spawn_origin_capacity) as usize, [0.0; 4]);
                self.hybrid_particle_spawn_rotations.resize(
                    (base + spawn_origin_capacity) as usize,
                    [0.0, 0.0, 0.0, 1.0],
                );
                SpawnRingState {
                    base,
                    capacity: spawn_origin_capacity,
                    slot_spawn_keys: vec![u32::MAX; spawn_origin_capacity as usize],
                }
            });
            if entry.capacity != spawn_origin_capacity {
                let base = self.hybrid_particle_spawn_origins.len() as u32;
                self.hybrid_particle_spawn_origins
                    .resize((base + spawn_origin_capacity) as usize, [0.0; 4]);
                self.hybrid_particle_spawn_rotations.resize(
                    (base + spawn_origin_capacity) as usize,
                    [0.0, 0.0, 0.0, 1.0],
                );
                entry.base = base;
                entry.capacity = spawn_origin_capacity;
                entry.slot_spawn_keys = vec![u32::MAX; spawn_origin_capacity as usize];
            }
            for i in 0..emit_count {
                let spawn_index = if emitter.looping {
                    let back = emit_count.saturating_sub(1).saturating_sub(i);
                    total_spawned.saturating_sub(back)
                } else {
                    i
                };
                let slot = spawn_index % entry.capacity;
                let slot_idx = slot as usize;
                if entry.slot_spawn_keys[slot_idx] != spawn_index {
                    entry.slot_spawn_keys[slot_idx] = spawn_index;
                    spawn_origin_updates.push((
                        entry.base + slot,
                        [current_origin.x, current_origin.y, current_origin.z],
                        spawn_rot_arr,
                    ));
                }
            }
            entry.base
        };
        for (slot, origin, rotation) in spawn_origin_updates {
            self.hybrid_particle_spawn_origins[slot as usize] =
                [origin[0], origin[1], origin[2], 0.0];
            self.hybrid_particle_spawn_rotations[slot as usize] = rotation;
            self.hybrid_spawn_origin_dirty_slots.push(slot);
            self.hybrid_spawn_rotation_dirty_slots.push(slot);
        }
        self.hybrid_emitters.push(GpuEmitterParticle {
            model_0: emitter.model[0],
            model_1: emitter.model[1],
            model_2: emitter.model[2],
            model_3: emitter.model[3],
            gravity_path: [
                emitter.gravity[0],
                emitter.gravity[1],
                emitter.gravity[2],
                path_kind as f32,
            ],
            color_start: emitter.color_start.into(),
            color_end: emitter.color_end.into(),
            emissive_point: [
                emitter.emissive[0],
                emitter.emissive[1],
                emitter.emissive[2],
                emitter.size,
            ],
            life_speed: [
                life_min,
                life_max,
                emitter.speed_min.max(0.0),
                emitter.speed_max.max(emitter.speed_min.max(0.0)),
            ],
            size_spread_rate: [
                emitter.size_min.max(0.01),
                emitter.size_max.max(emitter.size_min.max(0.01)),
                emitter.spread_radians.clamp(0.0, std::f32::consts::PI),
                emitter.emission_rate.max(0.0),
            ],
            time_path: [
                emitter.simulation_time.max(0.0),
                path_a,
                path_b,
                emitter.simulation_delta.max(0.0),
            ],
            counts_seed: [
                particle_start,
                emit_count,
                max_alive_budget.max(1),
                emitter.seed,
            ],
            flags: [
                u32::from(emitter.looping),
                u32::from(emitter.prewarm),
                emitter.profile.spin_angular_velocity.to_bits(),
                spawn_origin_base,
            ],
            custom_ops_xy: [0; 4],
            custom_ops_zp: [0; 4],
        });
        if emitter.render_mode == ParticleRenderMode3D::Billboard {
            self.hybrid_has_billboard = true;
            push_instance_range(
                &mut self.hybrid_billboard_ranges,
                particle_start,
                emit_count,
                path_kind,
            );
        } else {
            self.hybrid_has_point = true;
            push_instance_range(
                &mut self.hybrid_point_ranges,
                particle_start,
                emit_count,
                path_kind,
            );
        }
        self.hybrid_particle_count += emit_count;
        true
    }

    pub(super) fn push_compute_emitter_particles(
        &mut self,
        node: NodeID,
        emitter: &PointParticles3DState,
    ) -> bool {
        if !emitter.active || emitter.emission_rate <= 0.0 {
            return true;
        }
        let (path_kind, path_a, path_b) = match &emitter.profile.path {
            ParticlePath3D::None => (0u32, 0.0, 0.0),
            ParticlePath3D::Ballistic => (1u32, 0.0, 0.0),
            ParticlePath3D::Spiral {
                angular_velocity,
                radius,
            } => (2u32, *angular_velocity, *radius),
            ParticlePath3D::OrbitY {
                angular_velocity,
                radius,
            } => (3u32, *angular_velocity, *radius),
            ParticlePath3D::NoiseDrift {
                amplitude,
                frequency,
            } => (4u32, *amplitude, *frequency),
            ParticlePath3D::FlatDisk { radius } => (5u32, 0.0, *radius),
            ParticlePath3D::CustomCompiled {
                expr_x_ops,
                expr_y_ops,
                expr_z_ops,
            } => {
                let _ = self.append_compute_custom_data(
                    expr_x_ops.as_ref(),
                    expr_y_ops.as_ref(),
                    expr_z_ops.as_ref(),
                    &emitter.params,
                );
                (0u32, 0.0, 0.0)
            }
            ParticlePath3D::Custom {
                expr_x,
                expr_y,
                expr_z,
            } => {
                let expr_x_prog = match compile_expression(expr_x) {
                    Ok(program) => program,
                    Err(_) => return false,
                };
                let expr_y_prog = match compile_expression(expr_y) {
                    Ok(program) => program,
                    Err(_) => return false,
                };
                let expr_z_prog = match compile_expression(expr_z) {
                    Ok(program) => program,
                    Err(_) => return false,
                };
                let _ = self.append_compute_custom_data(
                    expr_x_prog.ops(),
                    expr_y_prog.ops(),
                    expr_z_prog.ops(),
                    &emitter.params,
                );
                (0u32, 0.0, 0.0)
            }
        };
        let mut custom_ops_xy = [0u32; 4];
        let mut custom_ops_zp = [0u32; 4];
        if let (Some(x_ops), Some(y_ops), Some(z_ops)) = (
            emitter.profile.expr_x_ops.as_ref(),
            emitter.profile.expr_y_ops.as_ref(),
            emitter.profile.expr_z_ops.as_ref(),
        ) {
            let (ops_xy, ops_zp) = self.append_compute_custom_data(
                x_ops.as_ref(),
                y_ops.as_ref(),
                z_ops.as_ref(),
                &emitter.params,
            );
            custom_ops_xy = ops_xy;
            custom_ops_zp = ops_zp;
        } else {
            match &emitter.profile.path {
                ParticlePath3D::CustomCompiled {
                    expr_x_ops,
                    expr_y_ops,
                    expr_z_ops,
                } => {
                    let (ops_xy, ops_zp) = self.append_compute_custom_data(
                        expr_x_ops.as_ref(),
                        expr_y_ops.as_ref(),
                        expr_z_ops.as_ref(),
                        &emitter.params,
                    );
                    custom_ops_xy = ops_xy;
                    custom_ops_zp = ops_zp;
                }
                ParticlePath3D::Custom {
                    expr_x,
                    expr_y,
                    expr_z,
                } => {
                    let expr_x_prog = match compile_expression(expr_x) {
                        Ok(program) => program,
                        Err(_) => return false,
                    };
                    let expr_y_prog = match compile_expression(expr_y) {
                        Ok(program) => program,
                        Err(_) => return false,
                    };
                    let expr_z_prog = match compile_expression(expr_z) {
                        Ok(program) => program,
                        Err(_) => return false,
                    };
                    let (ops_xy, ops_zp) = self.append_compute_custom_data(
                        expr_x_prog.ops(),
                        expr_y_prog.ops(),
                        expr_z_prog.ops(),
                        &emitter.params,
                    );
                    custom_ops_xy = ops_xy;
                    custom_ops_zp = ops_zp;
                }
                _ => {}
            }
        }

        let life_min = emitter.lifetime_min.max(0.001);
        let life_max = emitter.lifetime_max.max(life_min);
        let max_alive_budget = emitter.alive_budget.max(1);
        let mut emit_count = emitter_emission_count(emitter, max_alive_budget);
        if emit_count == 0 {
            return true;
        }
        if self.compute_particle_count > u32::MAX - emit_count {
            emit_count = u32::MAX - self.compute_particle_count;
        }
        if emit_count == 0 {
            return true;
        }
        let model = Mat4::from_cols_array_2d(&emitter.model);
        let current_origin = model.transform_point3(Vec3::ZERO);
        let (_, rot_raw, _) = model.to_scale_rotation_translation();
        let spawn_rot = if rot_raw.is_finite() && rot_raw.length_squared() > 1.0e-6 {
            rot_raw.normalize()
        } else {
            Quat::IDENTITY
        };
        let spawn_rot_arr = [spawn_rot.x, spawn_rot.y, spawn_rot.z, spawn_rot.w];
        let time = emitter.simulation_time.max(0.0);
        let prewarm_time = if emitter.looping && emitter.prewarm {
            time + life_max
        } else {
            time
        };
        let emission_rate = emitter.emission_rate.max(1.0e-6);
        let mut total_spawned = (prewarm_time * emission_rate).floor() as u32;
        if emitter.looping && emitter.prewarm {
            total_spawned = total_spawned.max(emit_count.saturating_sub(1));
        }
        let particle_start = self.compute_particle_count;
        let emitter_index = self.compute_emitters.len() as u32;
        append_emitter_map_entries(
            &mut self.compute_particle_emitter_map,
            emitter_index,
            emit_count,
            &mut self.compute_map_fingerprint,
        );
        let spawn_origin_capacity = max_alive_budget.max(1);
        let mut spawn_origin_updates = Vec::<(u32, [f32; 3], [f32; 4])>::new();
        let spawn_origin_base = {
            let entry = self.compute_spawn_rings.entry(node).or_insert_with(|| {
                let base = self.compute_particle_spawn_origins.len() as u32;
                self.compute_particle_spawn_origins
                    .resize((base + spawn_origin_capacity) as usize, [0.0; 4]);
                self.compute_particle_spawn_rotations.resize(
                    (base + spawn_origin_capacity) as usize,
                    [0.0, 0.0, 0.0, 1.0],
                );
                SpawnRingState {
                    base,
                    capacity: spawn_origin_capacity,
                    slot_spawn_keys: vec![u32::MAX; spawn_origin_capacity as usize],
                }
            });
            if entry.capacity != spawn_origin_capacity {
                let base = self.compute_particle_spawn_origins.len() as u32;
                self.compute_particle_spawn_origins
                    .resize((base + spawn_origin_capacity) as usize, [0.0; 4]);
                self.compute_particle_spawn_rotations.resize(
                    (base + spawn_origin_capacity) as usize,
                    [0.0, 0.0, 0.0, 1.0],
                );
                entry.base = base;
                entry.capacity = spawn_origin_capacity;
                entry.slot_spawn_keys = vec![u32::MAX; spawn_origin_capacity as usize];
            }
            for i in 0..emit_count {
                let spawn_index = if emitter.looping {
                    let back = emit_count.saturating_sub(1).saturating_sub(i);
                    total_spawned.saturating_sub(back)
                } else {
                    i
                };
                let slot = spawn_index % entry.capacity;
                let slot_idx = slot as usize;
                if entry.slot_spawn_keys[slot_idx] != spawn_index {
                    entry.slot_spawn_keys[slot_idx] = spawn_index;
                    spawn_origin_updates.push((
                        entry.base + slot,
                        [current_origin.x, current_origin.y, current_origin.z],
                        spawn_rot_arr,
                    ));
                }
            }
            entry.base
        };
        for (slot, origin, rotation) in spawn_origin_updates {
            self.compute_particle_spawn_origins[slot as usize] =
                [origin[0], origin[1], origin[2], 0.0];
            self.compute_particle_spawn_rotations[slot as usize] = rotation;
            self.compute_spawn_origin_dirty_slots.push(slot);
            self.compute_spawn_rotation_dirty_slots.push(slot);
        }
        self.compute_emitters.push(GpuEmitterParticle {
            model_0: emitter.model[0],
            model_1: emitter.model[1],
            model_2: emitter.model[2],
            model_3: emitter.model[3],
            gravity_path: [
                emitter.gravity[0],
                emitter.gravity[1],
                emitter.gravity[2],
                path_kind as f32,
            ],
            color_start: emitter.color_start.into(),
            color_end: emitter.color_end.into(),
            emissive_point: [
                emitter.emissive[0],
                emitter.emissive[1],
                emitter.emissive[2],
                emitter.size,
            ],
            life_speed: [
                life_min,
                life_max,
                emitter.speed_min.max(0.0),
                emitter.speed_max.max(emitter.speed_min.max(0.0)),
            ],
            size_spread_rate: [
                emitter.size_min.max(0.01),
                emitter.size_max.max(emitter.size_min.max(0.01)),
                emitter.spread_radians.clamp(0.0, std::f32::consts::PI),
                emitter.emission_rate.max(0.0),
            ],
            time_path: [
                emitter.simulation_time.max(0.0),
                path_a,
                path_b,
                emitter.simulation_delta.max(0.0),
            ],
            counts_seed: [
                particle_start,
                emit_count,
                max_alive_budget.max(1),
                emitter.seed,
            ],
            flags: [
                u32::from(emitter.looping),
                u32::from(emitter.prewarm),
                emitter.profile.spin_angular_velocity.to_bits(),
                spawn_origin_base,
            ],
            custom_ops_xy,
            custom_ops_zp,
        });
        if emitter.render_mode == ParticleRenderMode3D::Billboard {
            self.compute_has_billboard = true;
            push_instance_range(
                &mut self.compute_billboard_ranges,
                particle_start,
                emit_count,
                path_kind,
            );
        } else {
            self.compute_has_point = true;
            push_instance_range(
                &mut self.compute_point_ranges,
                particle_start,
                emit_count,
                path_kind,
            );
        }
        self.compute_particle_count += emit_count;
        true
    }

    pub(super) fn append_compute_custom_data(
        &mut self,
        expr_x_ops: &[Op],
        expr_y_ops: &[Op],
        expr_z_ops: &[Op],
        params: &[f32],
    ) -> ([u32; 4], [u32; 4]) {
        let (x_off, x_len) = append_gpu_ops(&mut self.compute_expr_ops, expr_x_ops);
        let (y_off, y_len) = append_gpu_ops(&mut self.compute_expr_ops, expr_y_ops);
        let (z_off, z_len) = append_gpu_ops(&mut self.compute_expr_ops, expr_z_ops);
        let params_off = self.compute_custom_params.len() as u32;
        self.compute_custom_params.extend_from_slice(params);
        let params_len = params.len() as u32;
        (
            [x_off, x_len, y_off, y_len],
            [z_off, z_len, params_off, params_len],
        )
    }

    pub(super) fn get_or_compile_expr(&mut self, expr: &str) -> Option<usize> {
        let expr_hash = perro_ids::string_to_u64(expr);
        if let Some(id) = self.compiled_expr_lookup.get(&expr_hash).copied() {
            return Some(id);
        }
        let compiled = compile_expression(expr).ok()?;
        let id = self.compiled_exprs.len();
        self.compiled_exprs.push(compiled);
        self.compiled_expr_lookup.insert(expr_hash, id);
        Some(id)
    }

    pub(super) fn eval_compiled_expr(
        &mut self,
        id: usize,
        input: &ParticleEvalInput<'_>,
    ) -> Option<f32> {
        let compiled = self.compiled_exprs.get(id)?;
        compiled.eval_particle(input, &mut self.eval_stack)
    }
}
