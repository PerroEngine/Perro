use super::*;

impl Runtime {
    pub(super) fn occlusion_openness_2d(
        &mut self,
        field: &mut PropagationField,
        listener_pos: Vector2,
        source_pos: Vector2,
        audio_layer: BitMask,
        attached_node: Option<NodeID>,
    ) -> (f32, Vector2) {
        let to_source = source_pos - listener_pos;
        let distance = to_source.length();
        if distance <= 0.0001 {
            return (0.0, Vector2::ZERO);
        }
        let dir = to_source * distance.recip();
        let perp = Vector2::new(-dir.y, dir.x);
        let offsets = [
            (
                perp * AUDIO_DIFFUSION_SPREAD_NEAR,
                AUDIO_DIFFUSION_NEAR_WEIGHT,
            ),
            (
                perp * -AUDIO_DIFFUSION_SPREAD_NEAR,
                AUDIO_DIFFUSION_NEAR_WEIGHT,
            ),
            (perp * AUDIO_DIFFUSION_SPREAD, AUDIO_DIFFUSION_FAR_WEIGHT),
            (perp * -AUDIO_DIFFUSION_SPREAD, AUDIO_DIFFUSION_FAR_WEIGHT),
        ];
        let n = offsets.len();
        let filter = audio_raycast_filter(audio_layer, attached_node);
        // First-time sample: probe every slot so openness starts accurate.
        let refresh = if field.initialized { PROBE_SLICE } else { n };
        for step in 0..refresh {
            if self.audio.counters.raycasts >= self.audio.config.rays_per_tick_2d {
                break;
            }
            let idx = (field.cursor + step) % n;
            let (offset, _weight) = offsets[idx];
            let probe = source_pos + offset;
            let probe_delta = probe - listener_pos;
            let probe_dist = probe_delta.length();
            if probe_dist <= 0.0001 {
                continue;
            }
            self.audio.counters.raycasts = self.audio.counters.raycasts.saturating_add(1);
            let blocked_physics = self
                .prepared_audio_raycast_2d(
                    listener_pos,
                    probe_delta * probe_dist.recip(),
                    probe_dist,
                    &filter,
                )
                .is_some_and(|hit| hit.distance > LISTENER_EMBED_EPSILON);
            let blocked = blocked_physics
                || (self.audio.has_audio_mask_2d
                    && self
                        .first_audio_mask_2d(listener_pos, probe, audio_layer)
                        .is_some());
            field.probe_open[idx] = if blocked { 0.0 } else { 1.0 };
            field.probe_seen[idx] = true;
            if !blocked {
                self.queue_audio_debug_ray_2d(
                    listener_pos,
                    probe,
                    audio_debug_color(AUDIO_DEBUG_DIRECT, 0.5),
                    0.5,
                );
            }
        }
        field.cursor = (field.cursor + refresh) % n;
        field.initialized = true;

        let mut attempted = 0.0f32;
        let mut open = 0.0f32;
        let mut open_shift = Vector2::ZERO;
        for (idx, (offset, weight)) in offsets.iter().enumerate() {
            if !field.probe_seen[idx] {
                continue;
            }
            attempted += weight;
            if field.probe_open[idx] > 0.5 {
                open += weight;
                open_shift += *offset * *weight;
            }
        }
        let instant = if attempted > 0.0 {
            open / attempted
        } else {
            0.0
        };
        field.smoothed_openness = smooth_openness(field.smoothed_openness, instant);
        let shift = if open > 0.0 {
            open_shift / open
        } else {
            Vector2::ZERO
        };
        (field.smoothed_openness, shift)
    }

    pub(super) fn occlusion_openness_3d(
        &mut self,
        field: &mut PropagationField,
        listener_pos: Vector3,
        source_pos: Vector3,
        audio_layer: BitMask,
        attached_node: Option<NodeID>,
    ) -> (f32, Vector3) {
        let zero = Vector3::new(0.0, 0.0, 0.0);
        let to_source = source_pos - listener_pos;
        let distance = to_source.length();
        if distance <= 0.0001 {
            return (0.0, zero);
        }
        let dir = to_source * distance.recip();
        let mut tangent = dir.cross(Vector3::new(0.0, 1.0, 0.0));
        if tangent.length_squared() <= 0.0001 {
            tangent = dir.cross(Vector3::new(1.0, 0.0, 0.0));
        }
        if tangent.length_squared() <= 0.0001 {
            return (0.0, zero);
        }
        let tangent = tangent.normalized();
        let bitangent = dir.cross(tangent).normalized();
        let offsets = [
            (
                tangent * AUDIO_DIFFUSION_SPREAD_NEAR,
                AUDIO_DIFFUSION_NEAR_WEIGHT,
            ),
            (
                tangent * -AUDIO_DIFFUSION_SPREAD_NEAR,
                AUDIO_DIFFUSION_NEAR_WEIGHT,
            ),
            (tangent * AUDIO_DIFFUSION_SPREAD, AUDIO_DIFFUSION_FAR_WEIGHT),
            (
                tangent * -AUDIO_DIFFUSION_SPREAD,
                AUDIO_DIFFUSION_FAR_WEIGHT,
            ),
            (
                bitangent * AUDIO_DIFFUSION_SPREAD,
                AUDIO_DIFFUSION_FAR_WEIGHT,
            ),
            (
                bitangent * -AUDIO_DIFFUSION_SPREAD,
                AUDIO_DIFFUSION_FAR_WEIGHT,
            ),
        ];
        let n = offsets.len();
        let refresh = if field.initialized { PROBE_SLICE } else { n };
        for step in 0..refresh {
            if self.audio.counters.raycasts >= self.audio.config.rays_per_tick_3d {
                break;
            }
            let idx = (field.cursor + step) % n;
            let (offset, _weight) = offsets[idx];
            let probe = source_pos + offset;
            let probe_delta = probe - listener_pos;
            let probe_dist = probe_delta.length();
            if probe_dist <= 0.0001 {
                continue;
            }
            self.audio.counters.raycasts = self.audio.counters.raycasts.saturating_add(1);
            let blocked_physics = self
                .prepared_audio_raycast_3d(
                    listener_pos,
                    probe_delta * probe_dist.recip(),
                    probe_dist,
                    false,
                )
                .is_some_and(|hit| {
                    Some(hit.node) != attached_node && hit.distance > LISTENER_EMBED_EPSILON
                });
            let blocked = blocked_physics
                || (self.audio.has_audio_mask_3d
                    && self
                        .first_audio_mask_3d(listener_pos, probe, audio_layer)
                        .is_some());
            field.probe_open[idx] = if blocked { 0.0 } else { 1.0 };
            field.probe_seen[idx] = true;
            if !blocked {
                self.queue_audio_debug_ray_3d(
                    listener_pos,
                    probe,
                    audio_debug_color(AUDIO_DEBUG_DIRECT, 0.5),
                    0.5,
                );
            }
        }
        field.cursor = (field.cursor + refresh) % n;
        field.initialized = true;

        let mut attempted = 0.0f32;
        let mut open = 0.0f32;
        let mut open_shift = zero;
        for (idx, (offset, weight)) in offsets.iter().enumerate() {
            if !field.probe_seen[idx] {
                continue;
            }
            attempted += weight;
            if field.probe_open[idx] > 0.5 {
                open += weight;
                open_shift += *offset * *weight;
            }
        }
        let instant = if attempted > 0.0 {
            open / attempted
        } else {
            0.0
        };
        field.smoothed_openness = smooth_openness(field.smoothed_openness, instant);
        let shift = if open > 0.0 { open_shift / open } else { zero };
        (field.smoothed_openness, shift)
    }

    // Verify a cached aperture cheaply: two raycasts (listener->aperture,
    // aperture->source) must both be unobstructed. Returns updated total path
    // distance on success. Used before falling back to the full fan re-search.
    pub(super) fn verify_aperture_2d(
        &mut self,
        listener_pos: Vector2,
        source_pos: Vector2,
        aperture: Vector2,
        audio_layer: BitMask,
        attached_node: Option<NodeID>,
    ) -> Option<f32> {
        let filter = audio_raycast_filter(audio_layer, attached_node);
        let leg_a =
            self.reconcile_segment_clear_2d(listener_pos, aperture, audio_layer, &filter)?;
        let leg_b = self.reconcile_segment_clear_2d(aperture, source_pos, audio_layer, &filter)?;
        Some(leg_a + leg_b)
    }

    pub(super) fn verify_aperture_3d(
        &mut self,
        listener_pos: Vector3,
        source_pos: Vector3,
        aperture: Vector3,
        audio_layer: BitMask,
        attached_node: Option<NodeID>,
    ) -> Option<f32> {
        let leg_a =
            self.reconcile_segment_clear_3d(listener_pos, aperture, audio_layer, attached_node)?;
        let leg_b =
            self.reconcile_segment_clear_3d(aperture, source_pos, audio_layer, attached_node)?;
        Some(leg_a + leg_b)
    }

    // Returns segment length if the straight path a->b is unobstructed (no
    // physics body and no audio mask between). Counts one raycast.
    pub(super) fn reconcile_segment_clear_2d(
        &mut self,
        a: Vector2,
        b: Vector2,
        audio_layer: BitMask,
        filter: &PhysicsQueryFilter,
    ) -> Option<f32> {
        let delta = b - a;
        let dist = delta.length();
        if dist <= 0.0001 {
            return Some(0.0);
        }
        self.audio.counters.raycasts = self.audio.counters.raycasts.saturating_add(1);
        let dir = delta * dist.recip();
        // Small skin at each end skips the surface a bounce point sits ON while
        // still catching any wall strictly BETWEEN the two points (so two
        // points straddling a thin wall do NOT falsely reconcile).
        let skin = AUDIO_PORTAL_EPSILON.min(dist * 0.25);
        let start = a + dir * skin;
        let seg = (dist - skin * 2.0).max(0.0);
        let blocked_physics = self
            .prepared_audio_raycast_2d(start, dir, seg, filter)
            .is_some_and(|hit| hit.distance > LISTENER_EMBED_EPSILON);
        if blocked_physics {
            return None;
        }
        if self.audio.has_audio_mask_2d
            && self
                .first_audio_mask_2d(start, b - dir * skin, audio_layer)
                .is_some()
        {
            return None;
        }
        Some(dist)
    }

    pub(super) fn reconcile_segment_clear_3d(
        &mut self,
        a: Vector3,
        b: Vector3,
        audio_layer: BitMask,
        attached_node: Option<NodeID>,
    ) -> Option<f32> {
        let delta = b - a;
        let dist = delta.length();
        if dist <= 0.0001 {
            return Some(0.0);
        }
        self.audio.counters.raycasts = self.audio.counters.raycasts.saturating_add(1);
        let dir = delta * dist.recip();
        let skin = AUDIO_PORTAL_EPSILON.min(dist * 0.25);
        let start = a + dir * skin;
        let seg = (dist - skin * 2.0).max(0.0);
        let blocked_physics = self
            .prepared_audio_raycast_3d(start, dir, seg, false)
            .is_some_and(|hit| {
                Some(hit.node) != attached_node && hit.distance > LISTENER_EMBED_EPSILON
            });
        if blocked_physics {
            return None;
        }
        if self.audio.has_audio_mask_3d
            && self
                .first_audio_mask_3d(start, b - dir * skin, audio_layer)
                .is_some()
        {
            return None;
        }
        Some(dist)
    }

    // Bidirectional reconciler (Phase 1). Casts a fan from the listener and a
    // fan from the source, records each ray's polyline points, then finds the
    // listener-point / source-point pair that reconciles (within epsilon or via
    // a short verification ray). The matched midpoint is a virtual source
    // (aperture). Returns (aperture, total path distance, loss).
    pub(in super::super) fn reconcile_aperture_2d(
        &mut self,
        listener_pos: Vector2,
        source_pos: Vector2,
        audio_layer: BitMask,
        attached_node: Option<NodeID>,
        range: f32,
    ) -> Option<(Vector2, f32, f32)> {
        let mut listener_pts = std::mem::take(&mut self.audio.scratch_reconcile_listener_2d);
        let mut source_pts = std::mem::take(&mut self.audio.scratch_reconcile_source_2d);
        listener_pts.clear();
        source_pts.clear();
        self.collect_reconcile_points_2d(
            listener_pos,
            source_pos,
            audio_layer,
            range,
            &mut listener_pts,
        );
        self.collect_reconcile_points_2d(
            source_pos,
            listener_pos,
            audio_layer,
            range,
            &mut source_pts,
        );

        // One filter for every pair verification (O(listener_pts × source_pts)).
        let filter = audio_raycast_filter(audio_layer, attached_node);
        let mut best: Option<(Vector2, f32, f32)> = None;
        for lp in listener_pts.iter().copied() {
            for sp in source_pts.iter().copied() {
                let gap = lp.point.distance_to(sp.point);
                if gap > RECONCILE_VERIFY_MAX {
                    continue;
                }
                // Always verify the connecting segment is unobstructed, even for
                // sub-epsilon gaps: two points can sit within epsilon on
                // opposite sides of a thin wall and must NOT reconcile.
                let (aperture, verify_dist) =
                    match self.reconcile_segment_clear_2d(lp.point, sp.point, audio_layer, &filter)
                    {
                        Some(d) => ((lp.point + sp.point) * 0.5, d),
                        None => continue,
                    };
                let total = lp.traveled + verify_dist + sp.traveled;
                if total > range {
                    continue;
                }
                // Effective loss: higher is better. Give sub-epsilon coincident
                // matches a small edge over verified-across-a-gap matches.
                let tight = if gap <= RECONCILE_EPSILON { 1.05 } else { 1.0 };
                let loss = (lp.loss * sp.loss * tight).min(1.0);
                let score = loss / (1.0 + total);
                if best
                    .as_ref()
                    .is_none_or(|&(_, bt, bl)| score > bl / (1.0 + bt))
                {
                    best = Some((aperture, total, loss));
                }
            }
        }
        listener_pts.clear();
        source_pts.clear();
        self.audio.scratch_reconcile_listener_2d = listener_pts;
        self.audio.scratch_reconcile_source_2d = source_pts;
        best
    }

    pub(in super::super) fn reconcile_aperture_3d(
        &mut self,
        listener_pos: Vector3,
        source_pos: Vector3,
        audio_layer: BitMask,
        attached_node: Option<NodeID>,
        range: f32,
    ) -> Option<(Vector3, f32, f32)> {
        let mut listener_pts = std::mem::take(&mut self.audio.scratch_reconcile_listener_3d);
        let mut source_pts = std::mem::take(&mut self.audio.scratch_reconcile_source_3d);
        listener_pts.clear();
        source_pts.clear();
        self.collect_reconcile_points_3d(
            listener_pos,
            source_pos,
            audio_layer,
            range,
            &mut listener_pts,
        );
        self.collect_reconcile_points_3d(
            source_pos,
            listener_pos,
            audio_layer,
            range,
            &mut source_pts,
        );

        let mut best: Option<(Vector3, f32, f32)> = None;
        for lp in listener_pts.iter().copied() {
            for sp in source_pts.iter().copied() {
                let gap = lp.point.distance_to(sp.point);
                if gap > RECONCILE_VERIFY_MAX {
                    continue;
                }
                let (aperture, verify_dist) = match self.reconcile_segment_clear_3d(
                    lp.point,
                    sp.point,
                    audio_layer,
                    attached_node,
                ) {
                    Some(d) => ((lp.point + sp.point) * 0.5, d),
                    None => continue,
                };
                let total = lp.traveled + verify_dist + sp.traveled;
                if total > range {
                    continue;
                }
                let tight = if gap <= RECONCILE_EPSILON { 1.05 } else { 1.0 };
                let loss = (lp.loss * sp.loss * tight).min(1.0);
                let score = loss / (1.0 + total);
                if best
                    .as_ref()
                    .is_none_or(|&(_, bt, bl)| score > bl / (1.0 + bt))
                {
                    best = Some((aperture, total, loss));
                }
            }
        }
        listener_pts.clear();
        source_pts.clear();
        self.audio.scratch_reconcile_listener_3d = listener_pts;
        self.audio.scratch_reconcile_source_3d = source_pts;
        best
    }

    // Cast a fan from `origin` (aimed generally toward `target`) and record
    // each ray's bounce points plus interior free-segment samples with their
    // accumulated traveled-distance and loss. Budget-guarded by rays_per_tick.
    pub(super) fn collect_reconcile_points_2d(
        &mut self,
        origin: Vector2,
        target: Vector2,
        audio_layer: BitMask,
        range: f32,
        out: &mut Vec<ReconcilePoint2D>,
    ) {
        for i in 0..RECONCILE_FAN_2D {
            if self.audio.counters.raycasts >= self.audio.config.rays_per_tick_2d {
                break;
            }
            let angle = i as f32 * TAU / RECONCILE_FAN_2D as f32;
            let dir = Vector2::new(angle.cos(), angle.sin());
            self.march_reconcile_ray_2d(origin, target, dir, audio_layer, range, out);
        }
    }

    pub(super) fn collect_reconcile_points_3d(
        &mut self,
        origin: Vector3,
        target: Vector3,
        audio_layer: BitMask,
        range: f32,
        out: &mut Vec<ReconcilePoint3D>,
    ) {
        for i in 0..RECONCILE_FAN_3D {
            if self.audio.counters.raycasts >= self.audio.config.rays_per_tick_3d {
                break;
            }
            let n = RECONCILE_FAN_3D as f32;
            let y = 1.0 - (i as f32 + 0.5) * 2.0 / n;
            let radius = (1.0 - y * y).max(0.0).sqrt();
            let theta = i as f32 * 2.399_963_1;
            let dir = Vector3::new(theta.cos() * radius, y, theta.sin() * radius);
            self.march_reconcile_ray_3d(origin, target, dir, audio_layer, range, out);
        }
    }

    pub(super) fn march_reconcile_ray_2d(
        &mut self,
        origin: Vector2,
        target: Vector2,
        initial_direction: Vector2,
        audio_layer: BitMask,
        range: f32,
        out: &mut Vec<ReconcilePoint2D>,
    ) {
        let mut pos = origin;
        let mut direction = initial_direction;
        if direction.length_squared() <= 0.0001 {
            return;
        }
        direction = direction.normalized();
        let mut traveled = 0.0f32;
        let mut loss = 1.0f32;
        for _ in 0..self.audio.config.max_bounces_2d {
            let remaining = (range - traveled)
                .min(self.audio.config.max_ray_distance_2d)
                .max(0.0);
            if remaining <= AUDIO_PORTAL_EPSILON {
                break;
            }
            if self.audio.counters.raycasts >= self.audio.config.rays_per_tick_2d {
                break;
            }
            let hit = self.nearest_audio_bounce_hit_2d(pos, direction, remaining, audio_layer);
            let seg_len = hit.as_ref().map(|h| h.distance).unwrap_or(remaining);
            // Sample points along the free segment at fixed spacing so density
            // stays high near apertures on long rays.
            let samples =
                ((seg_len / RECONCILE_SAMPLE_SPACING) as usize).min(RECONCILE_MAX_SAMPLES);
            for s in 1..=samples {
                let d = s as f32 * RECONCILE_SAMPLE_SPACING;
                if d >= seg_len {
                    break;
                }
                out.push(ReconcilePoint2D {
                    point: pos + direction * d,
                    traveled: traveled + d,
                    loss,
                });
            }
            let Some(hit) = hit else {
                // Free ray end: record its far endpoint.
                out.push(ReconcilePoint2D {
                    point: pos + direction * seg_len,
                    traveled: traveled + seg_len,
                    loss,
                });
                break;
            };
            traveled += hit.distance;
            loss *= hit.volume_loss.clamp(0.0, 1.0);
            out.push(ReconcilePoint2D {
                point: hit.point,
                traveled,
                loss,
            });
            let reflect_energy = hit.reflection.clamp(0.0, 1.0);
            if reflect_energy < self.audio.config.energy_cutoff
                || loss < self.audio.config.energy_cutoff
            {
                break;
            }
            let Some(reflected) = reflect_2d(direction, hit.normal) else {
                break;
            };
            loss *= reflect_energy;
            self.queue_audio_debug_ray_2d(
                pos,
                hit.point,
                audio_debug_color(AUDIO_DEBUG_BOUNCE, loss),
                loss,
            );
            pos = hit.point + reflected * AUDIO_PORTAL_EPSILON;
            direction = reflected;
        }
        let _ = target;
    }

    pub(super) fn march_reconcile_ray_3d(
        &mut self,
        origin: Vector3,
        target: Vector3,
        initial_direction: Vector3,
        audio_layer: BitMask,
        range: f32,
        out: &mut Vec<ReconcilePoint3D>,
    ) {
        let mut pos = origin;
        let mut direction = initial_direction;
        if direction.length_squared() <= 0.0001 {
            return;
        }
        direction = direction.normalized();
        let mut traveled = 0.0f32;
        let mut loss = 1.0f32;
        for _ in 0..self.audio.config.max_bounces_3d {
            let remaining = (range - traveled)
                .min(self.audio.config.max_ray_distance_3d)
                .max(0.0);
            if remaining <= AUDIO_PORTAL_EPSILON {
                break;
            }
            if self.audio.counters.raycasts >= self.audio.config.rays_per_tick_3d {
                break;
            }
            let hit = self.nearest_audio_bounce_hit_3d(pos, direction, remaining, audio_layer);
            let seg_len = hit.as_ref().map(|h| h.distance).unwrap_or(remaining);
            let samples =
                ((seg_len / RECONCILE_SAMPLE_SPACING) as usize).min(RECONCILE_MAX_SAMPLES);
            for s in 1..=samples {
                let d = s as f32 * RECONCILE_SAMPLE_SPACING;
                if d >= seg_len {
                    break;
                }
                out.push(ReconcilePoint3D {
                    point: pos + direction * d,
                    traveled: traveled + d,
                    loss,
                });
            }
            let Some(hit) = hit else {
                out.push(ReconcilePoint3D {
                    point: pos + direction * seg_len,
                    traveled: traveled + seg_len,
                    loss,
                });
                break;
            };
            traveled += hit.distance;
            loss *= hit.volume_loss.clamp(0.0, 1.0);
            out.push(ReconcilePoint3D {
                point: hit.point,
                traveled,
                loss,
            });
            let reflect_energy = hit.reflection.clamp(0.0, 1.0);
            if reflect_energy < self.audio.config.energy_cutoff
                || loss < self.audio.config.energy_cutoff
            {
                break;
            }
            let Some(reflected) = reflect_3d(direction, hit.normal) else {
                break;
            };
            loss *= reflect_energy;
            self.queue_audio_debug_ray_3d(
                pos,
                hit.point,
                audio_debug_color(AUDIO_DEBUG_BOUNCE, loss),
                loss,
            );
            pos = hit.point + reflected * AUDIO_PORTAL_EPSILON;
            direction = reflected;
        }
        let _ = target;
    }
}
