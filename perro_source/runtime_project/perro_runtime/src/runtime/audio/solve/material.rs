use super::*;

impl Runtime {
    pub(in super::super) fn emitter_attenuation_2d(
        &mut self,
        sound: &ActiveSpatialSound,
        source_pos: Vector2,
        listener_pos: Vector2,
    ) -> f32 {
        if matches!(sound.options.direction_2d, AudioDirection::Omni) {
            return 1.0;
        }
        let Some((mode, direction)) = self.emitter_direction_2d(sound) else {
            return 1.0;
        };
        let to_listener = source_pos.direction_to(listener_pos);
        let dot = direction.dot(to_listener).clamp(-1.0, 1.0);
        emitter_lobe(mode, dot)
    }

    pub(in super::super) fn emitter_attenuation_3d(
        &mut self,
        sound: &ActiveSpatialSound,
        source_pos: Vector3,
        listener_pos: Vector3,
    ) -> f32 {
        if matches!(sound.options.direction_3d, AudioDirection::Omni) {
            return 1.0;
        }
        let Some((mode, direction)) = self.emitter_direction_3d(sound) else {
            return 1.0;
        };
        let to_listener = source_pos.direction_to(listener_pos);
        let dot = direction.dot(to_listener).clamp(-1.0, 1.0);
        emitter_lobe(mode, dot)
    }

    pub(super) fn emitter_direction_2d(
        &mut self,
        sound: &ActiveSpatialSound,
    ) -> Option<(EmitterMode, Vector2)> {
        let (mode, fallback) = match sound.options.direction_2d {
            AudioDirection::Omni => return None,
            AudioDirection::Directional(v) => (EmitterMode::Directional, v),
            AudioDirection::InverseDirectional(v) => (EmitterMode::InverseDirectional, v),
            AudioDirection::Bidirectional(v) => (EmitterMode::Bidirectional, v),
        };
        let direction = match sound.pos {
            SpatialSoundPos::Attached(node) => {
                let transform = self.get_global_transform_2d(node)?;
                Vector2::new(transform.rotation.sin(), -transform.rotation.cos())
            }
            _ => fallback,
        };
        (direction.length_squared() > 0.0001).then_some((mode, direction.normalized()))
    }

    pub(super) fn emitter_direction_3d(
        &mut self,
        sound: &ActiveSpatialSound,
    ) -> Option<(EmitterMode, Vector3)> {
        let (mode, fallback) = match sound.options.direction_3d {
            AudioDirection::Omni => return None,
            AudioDirection::Directional(v) => (EmitterMode::Directional, v),
            AudioDirection::InverseDirectional(v) => (EmitterMode::InverseDirectional, v),
            AudioDirection::Bidirectional(v) => (EmitterMode::Bidirectional, v),
        };
        let direction = match sound.pos {
            SpatialSoundPos::Attached(node) => {
                let transform = self.get_global_transform_3d(node)?;
                rotate_vec3(
                    [
                        transform.rotation.x,
                        transform.rotation.y,
                        transform.rotation.z,
                        transform.rotation.w,
                    ],
                    Vector3::new(0.0, 0.0, -1.0),
                )
            }
            _ => fallback,
        };
        (direction.length_squared() > 0.0001).then_some((mode, direction.normalized()))
    }

    pub(in super::super) fn audio_material_for_node(&self, node: NodeID) -> Option<AudioMaterial> {
        let data = &self.nodes.get(node)?.data;
        // Bodies default to Some(AudioInteraction::new()) at construction, so
        // colliders occlude out of the box; `audio_interaction = none` is the
        // documented opt-out and must stay silent here.
        match data {
            SceneNodeData::StaticBody2D(v) => v.audio_interaction.map(|audio| audio.material),
            SceneNodeData::StaticBody3D(v) => v.audio_interaction.map(|audio| audio.material),
            SceneNodeData::RigidBody2D(v) => v.audio_interaction.map(|audio| audio.material),
            SceneNodeData::RigidBody3D(v) => v.audio_interaction.map(|audio| audio.material),
            SceneNodeData::CharacterBody2D(v) => v.audio_interaction.map(|audio| audio.material),
            SceneNodeData::CharacterBody3D(v) => v.audio_interaction.map(|audio| audio.material),
            SceneNodeData::Area2D(v) => v.audio_interaction.map(|audio| audio.material),
            SceneNodeData::Area3D(v) => v.audio_interaction.map(|audio| audio.material),
            SceneNodeData::AudioMask2D(v) if v.active => Some(v.material),
            SceneNodeData::AudioMask3D(v) if v.active => Some(v.material),
            _ => Some(AudioMaterial::default()),
        }
    }

    pub(in super::super) fn audio_diffusion_for_node(&self, node: NodeID) -> AudioDiffusion {
        let Some(data) = self.nodes.get(node).map(|n| &n.data) else {
            return AudioDiffusion::default();
        };
        match data {
            SceneNodeData::StaticBody2D(v) => v
                .audio_interaction
                .map(|audio| audio.diffusion)
                .unwrap_or_default(),
            SceneNodeData::StaticBody3D(v) => v
                .audio_interaction
                .map(|audio| audio.diffusion)
                .unwrap_or_default(),
            SceneNodeData::RigidBody2D(v) => v
                .audio_interaction
                .map(|audio| audio.diffusion)
                .unwrap_or_default(),
            SceneNodeData::RigidBody3D(v) => v
                .audio_interaction
                .map(|audio| audio.diffusion)
                .unwrap_or_default(),
            SceneNodeData::CharacterBody2D(v) => v
                .audio_interaction
                .map(|audio| audio.diffusion)
                .unwrap_or_default(),
            SceneNodeData::CharacterBody3D(v) => v
                .audio_interaction
                .map(|audio| audio.diffusion)
                .unwrap_or_default(),
            SceneNodeData::Area2D(v) => v
                .audio_interaction
                .map(|audio| audio.diffusion)
                .unwrap_or_default(),
            SceneNodeData::Area3D(v) => v
                .audio_interaction
                .map(|audio| audio.diffusion)
                .unwrap_or_default(),
            _ => AudioDiffusion::default(),
        }
    }

    pub(in super::super) fn audio_thickness_2d(&self, node: NodeID) -> f32 {
        self.nodes
            .get(node)
            .and_then(|n| {
                n.children_slice()
                    .iter()
                    .find_map(|child| self.nodes.get(*child))
            })
            .and_then(|n| match &n.data {
                SceneNodeData::CollisionShape2D(CollisionShape2D { shape, .. }) => match shape {
                    perro_nodes::Shape2D::Quad { width, height } => Some(width.min(*height)),
                    perro_nodes::Shape2D::Circle { radius } => Some(radius * 2.0),
                    perro_nodes::Shape2D::Triangle { width, height, .. } => {
                        Some(width.min(*height))
                    }
                },
                _ => None,
            })
            .unwrap_or(1.0)
    }

    pub(in super::super) fn first_audio_mask_2d(
        &mut self,
        from: Vector2,
        to: Vector2,
        audio_layer: BitMask,
    ) -> Option<AudioHit2D> {
        let dir = to - from;
        let len = dir.length();
        if len <= 0.0001 {
            return None;
        }
        let mut best: Option<AudioHit2D> = None;
        for index in 0..self.audio.audio_mask_ids_2d.len() {
            let mask_id = self.audio.audio_mask_ids_2d[index];
            let Some(SceneNodeData::AudioMask2D(mask)) = self.nodes.get(mask_id).map(|n| &n.data)
            else {
                continue;
            };
            if !mask.active || mask.material.audio_mask.intersects(audio_layer) {
                continue;
            }
            let material = mask.material;
            self.audio.scratch_child_ids.clear();
            if let Some(node) = self.nodes.get(mask_id) {
                self.audio
                    .scratch_child_ids
                    .extend_from_slice(node.children_slice());
            }
            for child_index in 0..self.audio.scratch_child_ids.len() {
                let child = self.audio.scratch_child_ids[child_index];
                let Some(shape_kind) =
                    self.nodes
                        .get(child)
                        .and_then(|shape_node| match &shape_node.data {
                            SceneNodeData::CollisionShape2D(shape) => Some(shape.shape),
                            _ => None,
                        })
                else {
                    continue;
                };
                let Some(global) = self.get_global_transform_2d(child) else {
                    continue;
                };
                let (half_w, half_h) = match shape_kind {
                    perro_nodes::Shape2D::Quad { width, height } => {
                        (width.abs() * 0.5, height.abs() * 0.5)
                    }
                    perro_nodes::Shape2D::Circle { radius } => (radius.abs(), radius.abs()),
                    perro_nodes::Shape2D::Triangle { width, height, .. } => {
                        (width.abs() * 0.5, height.abs() * 0.5)
                    }
                };
                if let Some((t, normal)) = segment_aabb(from, dir, global.position, half_w, half_h)
                {
                    let distance = t * len;
                    if best.as_ref().is_none_or(|hit| distance < hit.distance) {
                        best = Some(AudioHit2D {
                            node: mask_id,
                            point: from + dir * t,
                            normal,
                            distance,
                            material,
                            thickness: (half_w.min(half_h) * 2.0).max(0.05),
                        });
                    }
                }
            }
        }
        best
    }

    pub(in super::super) fn first_audio_mask_3d(
        &mut self,
        from: Vector3,
        to: Vector3,
        audio_layer: BitMask,
    ) -> Option<AudioHit3D> {
        let dir = to - from;
        let len = dir.length();
        if len <= 0.0001 {
            return None;
        }
        let mut best: Option<AudioHit3D> = None;
        for index in 0..self.audio.audio_mask_ids_3d.len() {
            let mask_id = self.audio.audio_mask_ids_3d[index];
            let Some(SceneNodeData::AudioMask3D(mask)) = self.nodes.get(mask_id).map(|n| &n.data)
            else {
                continue;
            };
            if !mask.active || mask.material.audio_mask.intersects(audio_layer) {
                continue;
            }
            let material = mask.material;
            self.audio.scratch_child_ids.clear();
            if let Some(node) = self.nodes.get(mask_id) {
                self.audio
                    .scratch_child_ids
                    .extend_from_slice(node.children_slice());
            }
            for child_index in 0..self.audio.scratch_child_ids.len() {
                let child = self.audio.scratch_child_ids[child_index];
                let Some((center, half)) = self.audio_effect_zone_shape_3d(child) else {
                    continue;
                };
                let Some((t, normal)) = segment_aabb_3d_with_normal(from, dir, center, half) else {
                    continue;
                };
                let distance = t * len;
                if best.as_ref().is_none_or(|hit| distance < hit.distance) {
                    best = Some(AudioHit3D {
                        node: mask_id,
                        point: from + dir * t,
                        normal,
                        distance,
                        material,
                        thickness: (half.x.min(half.y).min(half.z) * 2.0).max(0.05),
                    });
                }
            }
        }
        best
    }
}
