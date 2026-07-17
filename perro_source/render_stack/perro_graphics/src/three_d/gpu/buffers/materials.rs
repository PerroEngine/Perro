use super::*;

impl Gpu3D {
    pub fn draw_call_count(&self) -> u32 {
        (self.draw_batches.len() + self.multimesh_batches.len()) as u32
    }

    #[inline]
    pub fn draw_batch_count(&self) -> u32 {
        self.perf_counters.draw_batches
    }

    #[inline]
    pub fn pipeline_switch_count(&self) -> u32 {
        self.perf_counters.pipeline_switches
    }

    #[inline]
    pub fn texture_bind_group_switch_count(&self) -> u32 {
        self.perf_counters.texture_bind_group_switches
    }

    #[inline]
    pub fn prepare_step_timing(&self) -> Prepare3DStepTiming {
        self.last_prepare_step_timing
    }

    pub(in super::super) fn fallback_material_texture_bind_group(
        &self,
    ) -> Option<&wgpu::BindGroup> {
        self.material_fallback_bind_group.as_ref()
    }

    pub(in super::super) fn material_texture_set_bind_group(
        &self,
        key: MaterialTextureKey,
    ) -> Option<&wgpu::BindGroup> {
        self.material_texture_bind_groups
            .get(&key)
            .or_else(|| self.fallback_material_texture_bind_group())
    }

    pub(in super::super) fn ensure_material_fallback_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        if self.material_fallback_bind_group.is_some() {
            return;
        }
        let white = create_cached_material_texture(
            device,
            queue,
            CachedMaterialTextureInput {
                rgba: vec![255u8, 255, 255, 255],
                width: 1,
                height: 1,
                source: "__fallback__".to_string(),
                filter: self.texture_filter,
                color_space: MaterialTextureColorSpace::Srgb,
            },
        );
        let neutral_normal = create_cached_material_texture(
            device,
            queue,
            CachedMaterialTextureInput {
                rgba: vec![128u8, 128, 255, 255],
                width: 1,
                height: 1,
                source: "__normal_fallback__".to_string(),
                filter: self.texture_filter,
                color_space: MaterialTextureColorSpace::Linear,
            },
        );
        let custom_views = (0..CUSTOM_MATERIAL_IMAGE_COUNT)
            .map(|index| {
                if index == 1 {
                    &neutral_normal.view
                } else {
                    &white.view
                }
            })
            .collect::<Vec<_>>();
        let bind_group = create_material_texture_bind_group(
            device,
            &self.material_texture_bgl,
            &white.sampler,
            &white.view,
            &custom_views,
        );
        self.material_fallback_texture = Some(white);
        self.material_normal_fallback_texture = Some(neutral_normal);
        self.material_fallback_bind_group = Some(bind_group);
    }

    pub(in super::super) fn custom_material_image_key(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &ResourceStore,
        material: &Material3D,
    ) -> MaterialTextureKey {
        let params = material.standard_params();
        let mut key = if matches!(material, Material3D::Standard(_)) {
            MaterialTextureKey::from_standard(&params)
        } else {
            MaterialTextureKey::from_base(params.base_color_texture)
        };
        let Material3D::Custom(custom) = material else {
            return key;
        };
        self.ensure_material_fallback_texture(device, queue);
        for (index, image) in custom
            .images
            .iter()
            .take(CUSTOM_MATERIAL_IMAGE_COUNT)
            .enumerate()
        {
            let slot = self.custom_material_texture_slot(image.source.as_ref());
            self.ensure_material_texture_source(
                device,
                queue,
                resources,
                slot,
                image.source.as_ref(),
            );
            key.slots[index + 1] = slot;
        }
        key
    }

    pub(in super::super) fn custom_material_texture_slot(&mut self, source: &str) -> u32 {
        let source_hash = perro_ids::parse_hashed_source_uri(source)
            .unwrap_or_else(|| perro_ids::string_to_u64(source));
        if let Some(slot) = self
            .custom_material_texture_slots
            .get(&source_hash)
            .copied()
        {
            return slot;
        }
        let slot = self.next_custom_material_texture_slot;
        self.next_custom_material_texture_slot =
            self.next_custom_material_texture_slot.saturating_add(1);
        self.custom_material_texture_slots.insert(source_hash, slot);
        slot
    }

    pub(in super::super) fn ensure_material_texture_bind_group(
        &mut self,
        device: &wgpu::Device,
        key: MaterialTextureKey,
    ) {
        if self.material_texture_bind_groups.contains_key(&key) {
            return;
        }
        let Some(fallback) = self.material_fallback_texture.as_ref() else {
            return;
        };
        let Some(normal_fallback) = self.material_normal_fallback_texture.as_ref() else {
            return;
        };
        let base_cached = self.material_textures.get(&key.slots[0]);
        let base_view = base_cached
            .map(|cached| &cached.view)
            .unwrap_or(&fallback.view);
        let sampler = base_cached
            .map(|cached| &cached.sampler)
            .unwrap_or(&fallback.sampler);
        let mut custom_views = Vec::with_capacity(CUSTOM_MATERIAL_IMAGE_COUNT);
        for (index, slot) in key.slots.iter().skip(1).enumerate() {
            let view = self
                .material_textures
                .get(slot)
                .map(|cached| &cached.view)
                .unwrap_or_else(|| {
                    if key.standard && index == 1 {
                        &normal_fallback.view
                    } else {
                        &fallback.view
                    }
                });
            custom_views.push(view);
        }
        let bind_group = create_material_texture_bind_group(
            device,
            &self.material_texture_bgl,
            sampler,
            base_view,
            &custom_views,
        );
        self.material_texture_bind_groups.insert(key, bind_group);
    }

    pub(in super::super) fn ensure_material_texture_slot(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &ResourceStore,
        slot: u32,
        mesh_source: &str,
        _static_texture_lookup: Option<StaticTextureLookup>,
    ) {
        if slot == MATERIAL_TEXTURE_NONE {
            return;
        }
        self.ensure_material_fallback_texture(device, queue);
        let source_slot = material_texture_source_slot(slot);

        // glTF material texture indices are model-local, not global texture IDs.
        // Prefer glTF-local texture source when mesh source is glTF/glb.
        let gltf_source = gltf_texture_source_from_mesh_source(mesh_source, source_slot);
        let global_source = resources.texture_source_by_index(source_slot).or_else(|| {
            source_slot
                .checked_add(1)
                .and_then(|next| resources.texture_source_by_index(next))
        });
        let source = if gltf_source.is_some() {
            gltf_source.or_else(|| global_source.map(ToString::to_string))
        } else {
            global_source.map(ToString::to_string).or(gltf_source)
        };
        let Some(source) = source else {
            self.material_textures.remove(&slot);
            self.evict_material_texture_bind_groups_for_slot(slot);
            return;
        };
        if self
            .material_textures
            .get(&slot)
            .is_some_and(|cached| cached.source == source)
        {
            return;
        }

        let (rgba, width, height) =
            if let Some(decoded) = resources.decoded_texture_data_by_source(source.as_str()) {
                (decoded.rgba.clone(), decoded.width, decoded.height)
            } else if resources.has_texture_source(source.as_str()) {
                self.material_textures.remove(&slot);
                self.evict_material_texture_bind_groups_for_slot(slot);
                return;
            } else if let Some(decoded) = load_texture_rgba(source.as_str()) {
                decoded
            } else {
                self.material_textures.remove(&slot);
                self.evict_material_texture_bind_groups_for_slot(slot);
                return;
            };
        let cached = create_cached_material_texture(
            device,
            queue,
            CachedMaterialTextureInput {
                rgba,
                width,
                height,
                source,
                filter: self.material_texture_filter(source_slot),
                color_space: if material_texture_is_linear(slot) {
                    MaterialTextureColorSpace::Linear
                } else {
                    MaterialTextureColorSpace::Srgb
                },
            },
        );
        self.material_textures.insert(slot, cached);
        self.evict_material_texture_bind_groups_for_slot(slot);
    }

    pub(in super::super) fn ensure_standard_material_texture_slots(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &ResourceStore,
        material: &StandardMaterial3D,
        mesh_source: &str,
        static_texture_lookup: Option<StaticTextureLookup>,
    ) {
        let key = MaterialTextureKey::from_standard(material);
        for slot in key.slots.iter().take(5).copied() {
            self.ensure_material_texture_slot(
                device,
                queue,
                resources,
                slot,
                mesh_source,
                static_texture_lookup,
            );
        }
    }

    pub(in super::super) fn ensure_material_texture_source(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &ResourceStore,
        slot: u32,
        source: &str,
    ) {
        if self
            .material_textures
            .get(&slot)
            .is_some_and(|cached| cached.source == source)
        {
            return;
        }
        let (rgba, width, height) =
            if let Some(decoded) = resources.decoded_texture_data_by_source(source) {
                (decoded.rgba.clone(), decoded.width, decoded.height)
            } else if resources.has_texture_source(source) {
                self.material_textures.remove(&slot);
                self.evict_material_texture_bind_groups_for_slot(slot);
                return;
            } else if let Some(decoded) = load_texture_rgba(source) {
                decoded
            } else {
                self.material_textures.remove(&slot);
                self.evict_material_texture_bind_groups_for_slot(slot);
                return;
            };
        let cached = create_cached_material_texture(
            device,
            queue,
            CachedMaterialTextureInput {
                rgba,
                width,
                height,
                source: source.to_string(),
                filter: self.material_texture_filter(slot),
                color_space: MaterialTextureColorSpace::Srgb,
            },
        );
        self.material_textures.insert(slot, cached);
        self.evict_material_texture_bind_groups_for_slot(slot);
    }

    // stream slots skip mip chains: base level updates in place each frame.
    pub(in super::super) fn material_texture_filter(&self, slot: u32) -> TextureFilterMode {
        if self.stream_texture_slots.contains(&slot) {
            TextureFilterMode::Linear
        } else {
            self.texture_filter
        }
    }

    pub fn set_stream_texture(&mut self, slot: u32, is_stream: bool) {
        if is_stream {
            self.stream_texture_slots.insert(slot);
        } else if self.stream_texture_slots.remove(&slot) {
            self.invalidate_material_texture(slot);
        }
    }

    /// In-place base-level upload for a resident stream material texture. Returns
    /// false when no matching-dimension cache exists so the caller can rebuild.
    pub fn write_stream_material_texture(
        &mut self,
        queue: &wgpu::Queue,
        slot: u32,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> bool {
        self.material_textures
            .get(&slot)
            .is_some_and(|cached| cached.write_stream_base_level(queue, width, height, rgba))
    }

    pub fn upsert_external_material_texture(
        &mut self,
        device: &wgpu::Device,
        slot: u32,
        view: &wgpu::TextureView,
        source: String,
    ) {
        if slot == MATERIAL_TEXTURE_NONE {
            return;
        }
        let cached = create_external_material_texture(device, view, source);
        self.material_textures.insert(slot, cached);
        self.evict_material_texture_bind_groups_for_slot(slot);
    }

    pub fn invalidate_material_texture(&mut self, slot: u32) {
        self.material_textures.remove(&slot);
        self.material_textures
            .remove(&linear_material_texture_slot(slot));
        self.evict_material_texture_bind_groups_for_slot(slot);
    }

    pub fn invalidate_material_texture_source(&mut self, source: Option<&str>) {
        let Some(source) = source else {
            return;
        };
        let source_hash = perro_ids::parse_hashed_source_uri(source)
            .unwrap_or_else(|| perro_ids::string_to_u64(source));
        if let Some(slot) = self
            .custom_material_texture_slots
            .get(&source_hash)
            .copied()
        {
            self.invalidate_material_texture(slot);
        }
    }

    /// Stream texel update for a custom material image slot bound by source.
    /// In-place base write when the resident texture matches (single-level +
    /// dims); else invalidate so the next prepare rebuilds from decoded data.
    pub fn write_stream_material_texture_source(
        &mut self,
        queue: &wgpu::Queue,
        source: Option<&str>,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) {
        let Some(source) = source else {
            return;
        };
        let source_hash = perro_ids::parse_hashed_source_uri(source)
            .unwrap_or_else(|| perro_ids::string_to_u64(source));
        let Some(slot) = self
            .custom_material_texture_slots
            .get(&source_hash)
            .copied()
        else {
            return;
        };
        if !self.write_stream_material_texture(queue, slot, width, height, rgba) {
            self.invalidate_material_texture(slot);
        }
    }

    pub(in super::super) fn evict_material_texture_bind_groups_for_slot(&mut self, slot: u32) {
        self.material_texture_bind_groups
            .retain(|key, _| material_texture_key_survives_slot_evict(key, slot));
    }
}
