use super::*;

impl Runtime {
    pub(super) fn audio_effect_zone_mix_2d(
        &mut self,
        listener_pos: Vector2,
        source_pos: Vector2,
        audio_layer: BitMask,
    ) -> AudioEffectZoneMix {
        let mut mix = AudioEffectZoneMix::default();
        let mut scratch_ids = std::mem::take(&mut self.audio.scratch_ids);
        scratch_ids.clear();
        crate::runtime::scan_node_type_slots(
            &self.nodes,
            perro_nodes::NodeType::AudioEffectZone2D,
            |_| true,
            &mut scratch_ids,
        );
        self.audio.scratch_ids = scratch_ids;
        for index in 0..self.audio.scratch_ids.len() {
            let zone_id = self.audio.scratch_ids[index];
            let Some(SceneNodeData::AudioEffectZone2D(zone)) =
                self.nodes.get(zone_id).map(|n| &n.data)
            else {
                continue;
            };
            if !zone.active || zone.bounce || zone.audio_mask.intersects(audio_layer) {
                continue;
            }
            let effects = zone.effects.clone();
            let touches_zone = self.point_in_audio_effect_zone_2d(zone_id, listener_pos)
                || self.point_in_audio_effect_zone_2d(zone_id, source_pos)
                || self.segment_hits_audio_effect_zone_2d(zone_id, listener_pos, source_pos);
            if touches_zone {
                for effect in effects {
                    mix.apply(effect);
                }
            }
        }
        mix
    }

    pub(super) fn point_in_audio_effect_zone_2d(&mut self, zone: NodeID, point: Vector2) -> bool {
        self.audio.scratch_child_ids.clear();
        if let Some(node) = self.nodes.get(zone) {
            self.audio
                .scratch_child_ids
                .extend_from_slice(node.children_slice());
        }
        for index in 0..self.audio.scratch_child_ids.len() {
            let child = self.audio.scratch_child_ids[index];
            let Some((center, half_w, half_h)) = self.audio_effect_zone_shape_2d(child) else {
                continue;
            };
            if point.x >= center.x - half_w
                && point.x <= center.x + half_w
                && point.y >= center.y - half_h
                && point.y <= center.y + half_h
            {
                return true;
            }
        }
        false
    }

    pub(super) fn segment_hits_audio_effect_zone_2d(
        &mut self,
        zone: NodeID,
        from: Vector2,
        to: Vector2,
    ) -> bool {
        let dir = to - from;
        if dir.length() <= 0.0001 {
            return false;
        }
        self.audio.scratch_child_ids.clear();
        if let Some(node) = self.nodes.get(zone) {
            self.audio
                .scratch_child_ids
                .extend_from_slice(node.children_slice());
        }
        for index in 0..self.audio.scratch_child_ids.len() {
            let child = self.audio.scratch_child_ids[index];
            let Some((center, half_w, half_h)) = self.audio_effect_zone_shape_2d(child) else {
                continue;
            };
            if segment_aabb(from, dir, center, half_w, half_h).is_some() {
                return true;
            }
        }
        false
    }

    pub(super) fn audio_effect_zone_shape_2d(
        &mut self,
        node: NodeID,
    ) -> Option<(Vector2, f32, f32)> {
        let shape_kind = self
            .nodes
            .get(node)
            .and_then(|shape_node| match &shape_node.data {
                SceneNodeData::CollisionShape2D(shape) => Some(shape.shape),
                _ => None,
            })?;
        let global = self.get_global_transform_2d(node)?;
        let sx = global.scale.x.abs().max(0.0001);
        let sy = global.scale.y.abs().max(0.0001);
        let (half_w, half_h) = match shape_kind {
            perro_nodes::Shape2D::Quad { width, height } => {
                (width.abs() * sx * 0.5, height.abs() * sy * 0.5)
            }
            perro_nodes::Shape2D::Circle { radius } => (radius.abs() * sx, radius.abs() * sy),
            perro_nodes::Shape2D::Triangle { width, height, .. } => {
                (width.abs() * sx * 0.5, height.abs() * sy * 0.5)
            }
        };
        Some((global.position, half_w, half_h))
    }

    pub(super) fn audio_effect_zone_mix_3d(
        &mut self,
        listener_pos: Vector3,
        source_pos: Vector3,
        audio_layer: BitMask,
    ) -> AudioEffectZoneMix {
        let mut mix = AudioEffectZoneMix::default();
        let mut scratch_ids = std::mem::take(&mut self.audio.scratch_ids);
        scratch_ids.clear();
        crate::runtime::scan_node_type_slots(
            &self.nodes,
            perro_nodes::NodeType::AudioEffectZone3D,
            |_| true,
            &mut scratch_ids,
        );
        self.audio.scratch_ids = scratch_ids;
        for index in 0..self.audio.scratch_ids.len() {
            let zone_id = self.audio.scratch_ids[index];
            let Some(SceneNodeData::AudioEffectZone3D(zone)) =
                self.nodes.get(zone_id).map(|n| &n.data)
            else {
                continue;
            };
            if !zone.active || zone.bounce || zone.audio_mask.intersects(audio_layer) {
                continue;
            }
            let effects = zone.effects.clone();
            let touches_zone = self.point_in_audio_effect_zone_3d(zone_id, listener_pos)
                || self.point_in_audio_effect_zone_3d(zone_id, source_pos)
                || self.segment_hits_audio_effect_zone_3d(zone_id, listener_pos, source_pos);
            if touches_zone {
                for effect in effects {
                    mix.apply(effect);
                }
            }
        }
        mix
    }

    pub(super) fn point_in_audio_effect_zone_3d(&mut self, zone: NodeID, point: Vector3) -> bool {
        self.audio.scratch_child_ids.clear();
        if let Some(node) = self.nodes.get(zone) {
            self.audio
                .scratch_child_ids
                .extend_from_slice(node.children_slice());
        }
        for index in 0..self.audio.scratch_child_ids.len() {
            let child = self.audio.scratch_child_ids[index];
            let Some((center, half)) = self.audio_effect_zone_shape_3d(child) else {
                continue;
            };
            if point.x >= center.x - half.x
                && point.x <= center.x + half.x
                && point.y >= center.y - half.y
                && point.y <= center.y + half.y
                && point.z >= center.z - half.z
                && point.z <= center.z + half.z
            {
                return true;
            }
        }
        false
    }

    pub(super) fn segment_hits_audio_effect_zone_3d(
        &mut self,
        zone: NodeID,
        from: Vector3,
        to: Vector3,
    ) -> bool {
        let dir = to - from;
        if dir.length() <= 0.0001 {
            return false;
        }
        self.audio.scratch_child_ids.clear();
        if let Some(node) = self.nodes.get(zone) {
            self.audio
                .scratch_child_ids
                .extend_from_slice(node.children_slice());
        }
        for index in 0..self.audio.scratch_child_ids.len() {
            let child = self.audio.scratch_child_ids[index];
            let Some((center, half)) = self.audio_effect_zone_shape_3d(child) else {
                continue;
            };
            if segment_aabb_3d(from, dir, center, half).is_some() {
                return true;
            }
        }
        false
    }

    pub(super) fn audio_effect_zone_shape_3d(
        &mut self,
        node: NodeID,
    ) -> Option<(Vector3, Vector3)> {
        let shape_kind = self
            .nodes
            .get(node)
            .and_then(|shape_node| match &shape_node.data {
                SceneNodeData::CollisionShape3D(shape) => Some(shape.shape.clone()),
                _ => None,
            })?;
        let global = self.get_global_transform_3d(node)?;
        let scale = Vector3::new(
            global.scale.x.abs().max(0.0001),
            global.scale.y.abs().max(0.0001),
            global.scale.z.abs().max(0.0001),
        );
        let half = match shape_kind {
            perro_nodes::Shape3D::Cube { size }
            | perro_nodes::Shape3D::TriPrism { size }
            | perro_nodes::Shape3D::TriangularPyramid { size }
            | perro_nodes::Shape3D::SquarePyramid { size } => Vector3::new(
                size.x.abs() * scale.x * 0.5,
                size.y.abs() * scale.y * 0.5,
                size.z.abs() * scale.z * 0.5,
            ),
            perro_nodes::Shape3D::Sphere { radius } => Vector3::new(
                radius.abs() * scale.x,
                radius.abs() * scale.y,
                radius.abs() * scale.z,
            ),
            perro_nodes::Shape3D::Capsule { radius, .. }
            | perro_nodes::Shape3D::Cylinder { radius, .. }
            | perro_nodes::Shape3D::Cone { radius, .. } => Vector3::new(
                radius.abs() * scale.x,
                radius.abs() * scale.y,
                radius.abs() * scale.z,
            ),
            perro_nodes::Shape3D::TriMesh { .. } => scale,
        };
        Some((global.position, half))
    }

    pub(super) fn audio_thickness_3d(&self, node: NodeID) -> f32 {
        self.nodes
            .get(node)
            .and_then(|n| {
                n.children_slice()
                    .iter()
                    .find_map(|child| self.nodes.get(*child))
            })
            .and_then(|n| match &n.data {
                SceneNodeData::CollisionShape3D(CollisionShape3D { shape, .. }) => match shape {
                    perro_nodes::Shape3D::Cube { size } => Some(size.x.min(size.y).min(size.z)),
                    perro_nodes::Shape3D::Sphere { radius } => Some(radius * 2.0),
                    perro_nodes::Shape3D::Capsule { radius, .. }
                    | perro_nodes::Shape3D::Cylinder { radius, .. }
                    | perro_nodes::Shape3D::Cone { radius, .. } => Some(radius * 2.0),
                    perro_nodes::Shape3D::TriPrism { size }
                    | perro_nodes::Shape3D::TriangularPyramid { size }
                    | perro_nodes::Shape3D::SquarePyramid { size } => {
                        Some(size.x.min(size.y).min(size.z))
                    }
                    perro_nodes::Shape3D::TriMesh { .. } => Some(1.0),
                },
                _ => None,
            })
            .unwrap_or(1.0)
    }

    pub(super) fn first_audio_bounce_zone_2d(
        &mut self,
        origin: Vector2,
        direction: Vector2,
        max_distance: f32,
        audio_layer: BitMask,
    ) -> Option<AudioBounceHit2D> {
        if direction.length_squared() <= 0.0001 || max_distance <= 0.0001 {
            return None;
        }
        let sweep = direction.normalized() * max_distance;
        let mut best: Option<AudioBounceHit2D> = None;
        let mut scratch_ids = std::mem::take(&mut self.audio.scratch_ids);
        scratch_ids.clear();
        crate::runtime::scan_node_type_slots(
            &self.nodes,
            perro_nodes::NodeType::AudioEffectZone2D,
            |_| true,
            &mut scratch_ids,
        );
        self.audio.scratch_ids = scratch_ids;
        for index in 0..self.audio.scratch_ids.len() {
            let zone_id = self.audio.scratch_ids[index];
            let Some(SceneNodeData::AudioEffectZone2D(zone)) =
                self.nodes.get(zone_id).map(|n| &n.data)
            else {
                continue;
            };
            if !zone.active || !zone.bounce || zone.audio_mask.intersects(audio_layer) {
                continue;
            }
            let effects = zone.effects.clone();
            self.audio.scratch_child_ids.clear();
            if let Some(node) = self.nodes.get(zone_id) {
                self.audio
                    .scratch_child_ids
                    .extend_from_slice(node.children_slice());
            }
            for child_index in 0..self.audio.scratch_child_ids.len() {
                let child = self.audio.scratch_child_ids[child_index];
                let Some((center, half_w, half_h)) = self.audio_effect_zone_shape_2d(child) else {
                    continue;
                };
                let Some((t, normal)) = segment_aabb(origin, sweep, center, half_w, half_h) else {
                    continue;
                };
                let distance = t * max_distance;
                if distance <= AUDIO_PORTAL_EPSILON {
                    continue;
                }
                let mut mix = AudioEffectZoneMix::default();
                for effect in effects.iter().copied() {
                    mix.apply(effect);
                }
                let reflection = mix.echo.max(mix.reverb_send * 0.5).clamp(0.0, 1.0);
                let hit = AudioBounceHit2D {
                    point: origin + sweep * t,
                    normal,
                    distance,
                    reflection,
                    reverb_send: mix.reverb_send,
                    echo: mix.echo,
                    low_pass: mix.dampening,
                    volume_loss: (1.0 - mix.dampening.clamp(0.0, 1.0) * 0.35).clamp(0.0, 1.0),
                };
                if best
                    .as_ref()
                    .is_none_or(|best| hit.distance < best.distance)
                {
                    best = Some(hit);
                }
            }
        }
        best
    }

    pub(super) fn first_audio_bounce_zone_3d(
        &mut self,
        origin: Vector3,
        direction: Vector3,
        max_distance: f32,
        audio_layer: BitMask,
    ) -> Option<AudioBounceHit3D> {
        if direction.length_squared() <= 0.0001 || max_distance <= 0.0001 {
            return None;
        }
        let sweep = direction.normalized() * max_distance;
        let mut best: Option<AudioBounceHit3D> = None;
        let mut scratch_ids = std::mem::take(&mut self.audio.scratch_ids);
        scratch_ids.clear();
        crate::runtime::scan_node_type_slots(
            &self.nodes,
            perro_nodes::NodeType::AudioEffectZone3D,
            |_| true,
            &mut scratch_ids,
        );
        self.audio.scratch_ids = scratch_ids;
        for index in 0..self.audio.scratch_ids.len() {
            let zone_id = self.audio.scratch_ids[index];
            let Some(SceneNodeData::AudioEffectZone3D(zone)) =
                self.nodes.get(zone_id).map(|n| &n.data)
            else {
                continue;
            };
            if !zone.active || !zone.bounce || zone.audio_mask.intersects(audio_layer) {
                continue;
            }
            let effects = zone.effects.clone();
            self.audio.scratch_child_ids.clear();
            if let Some(node) = self.nodes.get(zone_id) {
                self.audio
                    .scratch_child_ids
                    .extend_from_slice(node.children_slice());
            }
            for child_index in 0..self.audio.scratch_child_ids.len() {
                let child = self.audio.scratch_child_ids[child_index];
                let Some((center, half)) = self.audio_effect_zone_shape_3d(child) else {
                    continue;
                };
                let Some((t, normal)) = segment_aabb_3d_with_normal(origin, sweep, center, half)
                else {
                    continue;
                };
                let distance = t * max_distance;
                if distance <= AUDIO_PORTAL_EPSILON {
                    continue;
                }
                let mut mix = AudioEffectZoneMix::default();
                for effect in effects.iter().copied() {
                    mix.apply(effect);
                }
                let reflection = mix.echo.max(mix.reverb_send * 0.5).clamp(0.0, 1.0);
                let hit = AudioBounceHit3D {
                    point: origin + sweep * t,
                    normal,
                    distance,
                    reflection,
                    reverb_send: mix.reverb_send,
                    echo: mix.echo,
                    low_pass: mix.dampening,
                    volume_loss: (1.0 - mix.dampening.clamp(0.0, 1.0) * 0.35).clamp(0.0, 1.0),
                };
                if best
                    .as_ref()
                    .is_none_or(|best| hit.distance < best.distance)
                {
                    best = Some(hit);
                }
            }
        }
        best
    }

    pub(super) fn start_spatial_sound(
        &mut self,
        audio: RuntimeAudio<'_>,
        pos: SpatialSoundPos,
        bus_id: Option<perro_ids::AudioBusID>,
        options: SpatialAudioOptions,
        last_2d: Option<Vector2>,
        last_3d: Option<Vector3>,
    ) -> bool {
        let options = normalize_spatial_options(options);
        let pan = perro_pawdio::AudioPan::CENTER;
        let playback_id = self.resource_api.bark.lock().ok().and_then(|guard| {
            guard.as_ref().and_then(|player| {
                player.play_spatial_source(perro_pawdio::AudioPlaybackRequest {
                    id: 0,
                    source: audio.source,
                    bus_id,
                    looped: audio.looped,
                    volume: audio.volume,
                    speed: audio.effects.speed,
                    pan,
                    low_pass: audio.effects.low_pass,
                    reverb_send: audio.effects.reverb_send,
                    echo: audio.effects.echo,
                    reflection: audio.effects.reflection,
                    occlusion: audio.effects.occlusion,
                    eq: perro_pawdio::AudioEq {
                        low_gain: audio.effects.eq.low_gain,
                        mid_gain: audio.effects.eq.mid_gain,
                        high_gain: audio.effects.eq.high_gain,
                    },
                    compression: perro_pawdio::AudioCompression {
                        threshold: audio.effects.compression.threshold,
                        ratio: audio.effects.compression.ratio,
                        attack: audio.effects.compression.attack,
                        release: audio.effects.compression.release,
                    },
                    from_start: audio.from_start,
                    from_end: audio.from_end,
                })
            })
        });
        let remaining = if audio.looped {
            None
        } else {
            self.resource_api.audio_length_seconds(audio.source)
        };
        self.audio.sounds.push(ActiveSpatialSound {
            source: audio.source.to_string(),
            kind: ActiveSpatialSoundKind::Audio,
            looped: audio.looped,
            volume: audio.volume,
            effects: audio.effects,
            options,
            pos,
            last_2d,
            last_3d,
            playback_id,
            elapsed_since_prop: f32::MAX,
            remaining,
            last_result: None,
            aperture_2d: None,
            aperture_3d: None,
            aperture_age: 0,
            field: PropagationField::default(),
        });
        true
    }

    pub(super) fn start_spatial_midi_note(&mut self, start: SpatialMidiNoteStart) -> bool {
        let SpatialMidiNoteStart {
            id,
            note,
            options,
            held,
            pos,
            spatial,
            last_2d,
            last_3d,
        } = start;
        let spatial = normalize_spatial_options(spatial);
        let pan = perro_pawdio::AudioPan::CENTER;
        let mut play_options = options;
        play_options.pan = pan;
        let playback_id = self.resource_api.bark.lock().ok().and_then(|guard| {
            guard.as_ref().and_then(|player| {
                player
                    .play_midi_note_spatial(perro_pawdio::midi::MidiNoteRequest {
                        id,
                        note,
                        options: play_options,
                        held,
                    })
                    .then_some(id)
            })
        });
        let remaining = if held {
            None
        } else {
            Some(options.sustain.as_secs_f32().max(0.01))
        };
        self.audio.sounds.push(ActiveSpatialSound {
            source: format!("midi:note:{id}"),
            kind: ActiveSpatialSoundKind::MidiNote,
            looped: held,
            volume: options.volume,
            effects: AudioEffects::default(),
            options: spatial,
            pos,
            last_2d,
            last_3d,
            playback_id,
            elapsed_since_prop: f32::MAX,
            remaining,
            last_result: None,
            aperture_2d: None,
            aperture_3d: None,
            aperture_age: 0,
            field: PropagationField::default(),
        });
        true
    }

    pub(super) fn start_spatial_midi_file(
        &mut self,
        id: u64,
        song: perro_pawdio::MidiSong,
        pos: SpatialSoundPos,
        spatial: SpatialAudioOptions,
        last_2d: Option<Vector2>,
        last_3d: Option<Vector3>,
    ) -> bool {
        let spatial = normalize_spatial_options(spatial);
        let playback_id = self.resource_api.bark.lock().ok().and_then(|guard| {
            guard.as_ref().and_then(|player| {
                player
                    .play_midi_file(perro_pawdio::midi::MidiFileRequest {
                        id,
                        song,
                        pan: perro_pawdio::AudioPan::CENTER,
                    })
                    .then_some(id)
            })
        });
        self.audio.sounds.push(ActiveSpatialSound {
            source: song.source.to_string(),
            kind: ActiveSpatialSoundKind::MidiFile,
            looped: song.looped,
            volume: song.volume,
            effects: AudioEffects::default(),
            options: spatial,
            pos,
            last_2d,
            last_3d,
            playback_id,
            elapsed_since_prop: f32::MAX,
            remaining: None,
            last_result: None,
            aperture_2d: None,
            aperture_3d: None,
            aperture_age: 0,
            field: PropagationField::default(),
        });
        true
    }

    #[allow(dead_code)]
    pub(super) fn play_runtime_audio_2d(
        &mut self,
        audio: RuntimeAudio<'_>,
        position: Vector2,
        options: SpatialAudioOptions,
    ) -> bool {
        self.start_spatial_sound(
            audio,
            SpatialSoundPos::TwoD(position),
            None,
            options,
            Some(position),
            None,
        )
    }

    #[allow(dead_code)]
    pub(super) fn play_runtime_audio_3d(
        &mut self,
        audio: RuntimeAudio<'_>,
        position: Vector3,
        options: SpatialAudioOptions,
    ) -> bool {
        self.start_spatial_sound(
            audio,
            SpatialSoundPos::ThreeD(position),
            None,
            options,
            None,
            Some(position),
        )
    }
}
