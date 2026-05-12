use super::*;

impl Runtime {
    pub(super) fn audio_zone_mix_2d(
        &mut self,
        listener_pos: Vector2,
        source_pos: Vector2,
    ) -> AudioZoneMix {
        let mut mix = AudioZoneMix::default();
        self.audio.scratch_ids.clear();
        for (id, node) in self.nodes.iter() {
            if matches!(node.data, SceneNodeData::AudioZone2D(_)) {
                self.audio.scratch_ids.push(id);
            }
        }
        for index in 0..self.audio.scratch_ids.len() {
            let zone_id = self.audio.scratch_ids[index];
            let Some(SceneNodeData::AudioZone2D(zone)) = self.nodes.get(zone_id).map(|n| &n.data)
            else {
                continue;
            };
            if !zone.enabled {
                continue;
            }
            let effect = zone.effect;
            let affect_listener = zone.affect_listener;
            let affect_emitters = zone.affect_emitters;
            let affect_path = zone.affect_path;
            let listener_inside =
                affect_listener && self.point_in_audio_zone_2d(zone_id, listener_pos);
            let source_inside = affect_emitters && self.point_in_audio_zone_2d(zone_id, source_pos);
            let path_inside =
                affect_path && self.segment_hits_audio_zone_2d(zone_id, listener_pos, source_pos);
            if listener_inside || source_inside || path_inside {
                mix.add(effect);
            }
        }
        mix
    }

    pub(super) fn point_in_audio_zone_2d(&mut self, zone: NodeID, point: Vector2) -> bool {
        self.audio.scratch_child_ids.clear();
        if let Some(node) = self.nodes.get(zone) {
            self.audio
                .scratch_child_ids
                .extend_from_slice(node.children_slice());
        }
        for index in 0..self.audio.scratch_child_ids.len() {
            let child = self.audio.scratch_child_ids[index];
            let Some((center, half_w, half_h)) = self.audio_zone_shape_2d(child) else {
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

    pub(super) fn segment_hits_audio_zone_2d(
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
            let Some((center, half_w, half_h)) = self.audio_zone_shape_2d(child) else {
                continue;
            };
            if segment_aabb(from, dir, center, half_w, half_h).is_some() {
                return true;
            }
        }
        false
    }

    pub(super) fn audio_zone_shape_2d(&mut self, node: NodeID) -> Option<(Vector2, f32, f32)> {
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

    pub(super) fn audio_zone_mix_3d(
        &mut self,
        listener_pos: Vector3,
        source_pos: Vector3,
    ) -> AudioZoneMix {
        let mut mix = AudioZoneMix::default();
        self.audio.scratch_ids.clear();
        for (id, node) in self.nodes.iter() {
            if matches!(node.data, SceneNodeData::AudioZone3D(_)) {
                self.audio.scratch_ids.push(id);
            }
        }
        for index in 0..self.audio.scratch_ids.len() {
            let zone_id = self.audio.scratch_ids[index];
            let Some(SceneNodeData::AudioZone3D(zone)) = self.nodes.get(zone_id).map(|n| &n.data)
            else {
                continue;
            };
            if !zone.enabled {
                continue;
            }
            let effect = zone.effect;
            let affect_listener = zone.affect_listener;
            let affect_emitters = zone.affect_emitters;
            let affect_path = zone.affect_path;
            let listener_inside =
                affect_listener && self.point_in_audio_zone_3d(zone_id, listener_pos);
            let source_inside = affect_emitters && self.point_in_audio_zone_3d(zone_id, source_pos);
            let path_inside =
                affect_path && self.segment_hits_audio_zone_3d(zone_id, listener_pos, source_pos);
            if listener_inside || source_inside || path_inside {
                mix.add(effect);
            }
        }
        mix
    }

    pub(super) fn point_in_audio_zone_3d(&mut self, zone: NodeID, point: Vector3) -> bool {
        self.audio.scratch_child_ids.clear();
        if let Some(node) = self.nodes.get(zone) {
            self.audio
                .scratch_child_ids
                .extend_from_slice(node.children_slice());
        }
        for index in 0..self.audio.scratch_child_ids.len() {
            let child = self.audio.scratch_child_ids[index];
            let Some((center, half)) = self.audio_zone_shape_3d(child) else {
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

    pub(super) fn segment_hits_audio_zone_3d(
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
            let Some((center, half)) = self.audio_zone_shape_3d(child) else {
                continue;
            };
            if segment_aabb_3d(from, dir, center, half).is_some() {
                return true;
            }
        }
        false
    }

    pub(super) fn audio_zone_shape_3d(&mut self, node: NodeID) -> Option<(Vector3, Vector3)> {
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

    pub(super) fn start_spatial_sound(
        &mut self,
        audio: RuntimeAudio<'_>,
        pos: SpatialSoundPos,
        options: SpatialAudioOptions,
        last_2d: Option<Vector2>,
        last_3d: Option<Vector3>,
    ) -> bool {
        let range = options.range.max(0.0001);
        let bus_id = options.bus_id;
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
            options: SpatialAudioOptions { range, ..options },
            pos,
            last_2d,
            last_3d,
            playback_id,
            elapsed_since_prop: f32::MAX,
            remaining,
            last_result: None,
        });
        true
    }

    pub(super) fn start_spatial_midi_note(&mut self, start: SpatialMidiNoteStart<'_>) -> bool {
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
        let range = spatial.range.max(0.0001);
        let pan = perro_pawdio::AudioPan::CENTER;
        let mut play_options = options;
        play_options.pan = pan;
        let playback_id = self.resource_api.bark.lock().ok().and_then(|guard| {
            guard.as_ref().and_then(|player| {
                player
                    .play_midi_note(perro_pawdio::midi::MidiNoteRequest {
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
            options: SpatialAudioOptions { range, ..spatial },
            pos,
            last_2d,
            last_3d,
            playback_id,
            elapsed_since_prop: f32::MAX,
            remaining,
            last_result: None,
        });
        true
    }

    pub(super) fn start_spatial_midi_file(
        &mut self,
        id: u64,
        song: perro_pawdio::MidiSong<'_>,
        pos: SpatialSoundPos,
        spatial: SpatialAudioOptions,
        last_2d: Option<Vector2>,
        last_3d: Option<Vector3>,
    ) -> bool {
        let range = spatial.range.max(0.0001);
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
            options: SpatialAudioOptions { range, ..spatial },
            pos,
            last_2d,
            last_3d,
            playback_id,
            elapsed_since_prop: f32::MAX,
            remaining: None,
            last_result: None,
        });
        true
    }

    pub(super) fn play_runtime_audio_2d(
        &mut self,
        audio: RuntimeAudio<'_>,
        position: Vector2,
        options: SpatialAudioOptions,
    ) -> bool {
        self.start_spatial_sound(
            audio,
            SpatialSoundPos::TwoD(position),
            options,
            Some(position),
            None,
        )
    }

    pub(super) fn play_runtime_audio_3d(
        &mut self,
        audio: RuntimeAudio<'_>,
        position: Vector3,
        options: SpatialAudioOptions,
    ) -> bool {
        self.start_spatial_sound(
            audio,
            SpatialSoundPos::ThreeD(position),
            options,
            None,
            Some(position),
        )
    }
}
