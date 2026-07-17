use super::*;

impl Gpu3D {
    pub(in super::super) fn resolve_mesh_range(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &ResourceStore,
        mesh_id: MeshID,
        source: &str,
        static_mesh_lookup: Option<StaticMeshLookup>,
    ) -> Option<MeshAssetRange> {
        if let Some(range) = self.builtin_mesh_ranges.get(source).copied() {
            let (bounds_center, bounds_radius) = self
                .builtin_mesh_bounds
                .get(source)
                .copied()
                .unwrap_or(([0.0, 0.0, 0.0], 1.0));
            return Some(MeshAssetRange {
                full: range,
                surface_ranges: Arc::from([range]),
                meshlets: self
                    .builtin_meshlets
                    .get(source)
                    .cloned()
                    .unwrap_or_else(|| Arc::from([])),
                lods: Arc::from([]),
                bounds_center,
                bounds_radius,
                blend_shape_delta_start: 0,
                blend_shape_target_count: 0,
                blend_shape_vertex_start: 0,
                blend_shape_vertex_count: 0,
            });
        }
        let revision = resources.mesh_revision(mesh_id);
        if let Some((cached_revision, range)) = self.custom_mesh_ranges.get(&mesh_id).cloned()
            && cached_revision == revision
        {
            return Some(range);
        }
        let decoded = if let Some(mesh) = resources.runtime_mesh_data_by_id(mesh_id) {
            load_mesh_from_source_no_dynamic_lods(source, static_mesh_lookup, Some(mesh))?
        } else {
            let runtime_mesh = resources.runtime_mesh_data(source);
            if let Some(mesh) = runtime_mesh {
                load_mesh_from_source_no_dynamic_lods(source, static_mesh_lookup, Some(mesh))?
            } else {
                load_mesh_from_source(
                    source,
                    static_mesh_lookup,
                    None,
                    self.meshlets_enabled && self.dev_meshlets,
                )?
            }
        };
        let range = self.append_mesh_data(device, queue, source, decoded)?;
        self.custom_mesh_ranges
            .insert(mesh_id, (revision, range.clone()));
        Some(range)
    }

    pub(in super::super) fn resolve_builtin_mesh_asset(
        &self,
        source: &str,
    ) -> Option<MeshAssetRange> {
        let full = self.builtin_mesh_ranges.get(source).copied()?;
        let meshlets = self
            .builtin_meshlets
            .get(source)
            .cloned()
            .unwrap_or_else(|| Arc::from([]));
        let (bounds_center, bounds_radius) = self
            .builtin_mesh_bounds
            .get(source)
            .copied()
            .unwrap_or(([0.0, 0.0, 0.0], 1.0));
        Some(MeshAssetRange {
            full,
            surface_ranges: Arc::from([full]),
            meshlets,
            lods: Arc::from([]),
            bounds_center,
            bounds_radius,
            blend_shape_delta_start: 0,
            blend_shape_target_count: 0,
            blend_shape_vertex_start: 0,
            blend_shape_vertex_count: 0,
        })
    }

    pub(in super::super) fn append_mesh_data(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _source: &str,
        decoded: DecodedMesh,
    ) -> Option<MeshAssetRange> {
        if decoded.vertices.is_empty() || decoded.indices.is_empty() {
            return None;
        }
        let DecodedMesh {
            vertices: decoded_vertices,
            indices: decoded_indices,
            surface_ranges: decoded_surface_ranges,
            blend_shapes: decoded_blend_shapes,
            meshlets: decoded_meshlets,
            lods: decoded_lods,
            has_skinning: _,
        } = decoded;
        let base_vertex = self.mesh_vertices.len() as u32;
        let index_start = self.mesh_indices.len() as u32;
        let index_count = decoded_indices.len() as u32;

        let (bounds_center, bounds_radius) = mesh_bounds_from_vertices(&decoded_vertices)?;
        let surface_ranges = if decoded_surface_ranges.is_empty() {
            vec![MeshRange {
                index_start,
                index_count,
                base_vertex: 0,
            }]
        } else {
            decoded_surface_ranges
                .iter()
                .copied()
                .map(|range| MeshRange {
                    index_start: index_start + range.index_start,
                    index_count: range.index_count,
                    base_vertex: 0,
                })
                .collect()
        };
        let added_vertices: Vec<SkinnedMeshVertex> = decoded_vertices
            .iter()
            .map(pack_skinned_mesh_vertex)
            .collect();
        let added_rigid_vertices: Vec<RigidMeshVertex> = decoded_vertices
            .iter()
            .map(pack_rigid_mesh_vertex)
            .collect();
        let mut added_indices = Vec::with_capacity(decoded_indices.len());
        for idx in decoded_indices {
            added_indices.push(idx + base_vertex);
        }

        let new_vertex_len = self.mesh_vertices.len() + added_vertices.len();
        let new_index_len = self.mesh_indices.len() + added_indices.len();
        self.ensure_mesh_buffer_capacity(device, queue, new_vertex_len, new_index_len);

        let vertex_offset =
            self.mesh_vertices.len() as u64 * std::mem::size_of::<SkinnedMeshVertex>() as u64;
        let rigid_vertex_offset =
            self.rigid_mesh_vertices.len() as u64 * std::mem::size_of::<RigidMeshVertex>() as u64;
        let index_offset = self.mesh_indices.len() as u64 * std::mem::size_of::<u32>() as u64;

        self.mesh_vertices.extend_from_slice(&added_vertices);
        self.rigid_mesh_vertices
            .extend_from_slice(&added_rigid_vertices);
        self.mesh_indices.extend_from_slice(&added_indices);

        queue.write_buffer(
            &self.vertex_buffer,
            vertex_offset,
            bytemuck::cast_slice(&added_vertices),
        );
        queue.write_buffer(
            &self.rigid_vertex_buffer,
            rigid_vertex_offset,
            bytemuck::cast_slice(&added_rigid_vertices),
        );
        queue.write_buffer(
            &self.index_buffer,
            index_offset,
            bytemuck::cast_slice(&added_indices),
        );

        let blend_shape_delta_start = self.blend_shape_deltas.len() as u32;
        let blend_shape_target_count = decoded_blend_shapes.len() as u32;
        let blend_shape_vertex_start = base_vertex;
        let blend_shape_vertex_count = decoded_vertices.len() as u32;
        if !decoded_blend_shapes.is_empty() {
            let added_delta_count = decoded_blend_shapes.len() * decoded_vertices.len();
            let old_delta_len = self.blend_shape_deltas.len();
            self.ensure_blend_shape_delta_capacity(
                device,
                queue,
                old_delta_len + added_delta_count,
            );
            self.blend_shape_deltas.reserve(added_delta_count);
            for shape in &decoded_blend_shapes {
                for vertex_index in 0..decoded_vertices.len() {
                    let vertex = shape.vertices.get(vertex_index).copied();
                    self.blend_shape_deltas.push(BlendShapeDeltaGpu {
                        position_delta: vertex
                            .map(|v| {
                                [
                                    v.position_delta[0],
                                    v.position_delta[1],
                                    v.position_delta[2],
                                    0.0,
                                ]
                            })
                            .unwrap_or([0.0; 4]),
                        normal_delta: vertex
                            .map(|v| [v.normal_delta[0], v.normal_delta[1], v.normal_delta[2], 0.0])
                            .unwrap_or([0.0; 4]),
                    });
                }
            }
            queue.write_buffer(
                &self.blend_shape_delta_buffer,
                old_delta_len as u64 * std::mem::size_of::<BlendShapeDeltaGpu>() as u64,
                bytemuck::cast_slice(&self.blend_shape_deltas[old_delta_len..]),
            );
        }

        let full = MeshRange {
            index_start,
            index_count,
            base_vertex: 0,
        };

        let meshlets: Vec<MeshletRange> = decoded_meshlets
            .iter()
            .copied()
            .filter_map(|meshlet| {
                if meshlet.index_count == 0 {
                    return None;
                }
                Some(MeshletRange {
                    index_start: index_start + meshlet.index_start,
                    index_count: meshlet.index_count,
                    center: meshlet.center,
                    radius: meshlet.radius.max(0.0),
                })
            })
            .collect();
        let meshlets_arc: Arc<[MeshletRange]> = Arc::from(meshlets);
        let surface_ranges_arc: Arc<[MeshRange]> = Arc::from(surface_ranges);
        let packed_lods = self.append_packed_lod_data(AppendPackedLodDataArgs {
            device,
            queue,
            vertices: &decoded_vertices,
            mesh_indices: &added_indices,
            base_vertex,
            decoded_lods: &decoded_lods,
            decoded_surfaces: &decoded_surface_ranges,
        });
        let lods = build_mesh_lod_ranges(BuildMeshLodRangesArgs {
            index_start,
            index_count,
            decoded_surfaces: &decoded_surface_ranges,
            uploaded_surfaces: &surface_ranges_arc,
            decoded_meshlets: &decoded_meshlets,
            uploaded_meshlets: &meshlets_arc,
            decoded_lods: &decoded_lods,
            packed_lods: &packed_lods,
        });

        Some(MeshAssetRange {
            full,
            surface_ranges: surface_ranges_arc,
            meshlets: meshlets_arc,
            lods: Arc::from(lods),
            bounds_center,
            bounds_radius,
            blend_shape_delta_start,
            blend_shape_target_count,
            blend_shape_vertex_start,
            blend_shape_vertex_count,
        })
    }

    fn append_packed_lod_data(
        &mut self,
        args: AppendPackedLodDataArgs<'_>,
    ) -> Vec<Option<PackedMeshLodRange>> {
        let AppendPackedLodDataArgs {
            device,
            queue,
            vertices,
            mesh_indices,
            base_vertex,
            decoded_lods,
            decoded_surfaces,
        } = args;
        if decoded_lods.len() <= 1 {
            return vec![None; decoded_lods.len()];
        }
        let param_upload_start = self.packed_lod_params.len();
        self.ensure_packed_lod_param_capacity(
            device,
            queue,
            param_upload_start + decoded_lods.len().saturating_sub(1),
        );
        let mut out = Vec::with_capacity(decoded_lods.len());
        for (lod_index, lod) in decoded_lods.iter().enumerate() {
            if lod_index == 0 || lod.index_count == 0 {
                out.push(None);
                continue;
            }
            let src_start = lod.index_start as usize;
            let src_end = src_start
                .saturating_add(lod.index_count as usize)
                .min(mesh_indices.len());
            if src_start >= src_end {
                out.push(None);
                continue;
            }
            let src_indices = &mesh_indices[src_start..src_end];
            let Some(param) = packed_lod_param(vertices, src_indices, base_vertex) else {
                out.push(None);
                continue;
            };
            let param_index = self.packed_lod_params.len() as u32;
            self.packed_lod_params.push(param);

            let packed_index_start = self.packed_lod_indices.len() as u32;
            let packed_vertex_start = self.packed_lod_vertices.len() as u32;
            let mut remap: AHashMap<u32, u32> = AHashMap::with_capacity(src_indices.len());
            let mut new_vertices = Vec::with_capacity(src_indices.len());
            let mut new_indices = Vec::with_capacity(src_indices.len());
            for &uploaded_index in src_indices {
                let local_index = uploaded_index.saturating_sub(base_vertex);
                let next_index = packed_vertex_start + new_vertices.len() as u32;
                let packed_index = *remap.entry(local_index).or_insert_with(|| {
                    if let Some(vertex) = vertices.get(local_index as usize) {
                        new_vertices.push(pack_packed_lod_vertex(vertex, &param));
                        next_index
                    } else {
                        0
                    }
                });
                new_indices.push(packed_index);
            }
            if new_vertices.is_empty() || new_indices.is_empty() {
                out.push(None);
                continue;
            }
            self.ensure_packed_lod_buffer_capacity(
                device,
                queue,
                self.packed_lod_vertices.len() + new_vertices.len(),
                self.packed_lod_indices.len() + new_indices.len(),
            );
            let vertex_offset = self.packed_lod_vertices.len() as u64
                * std::mem::size_of::<PackedRigidLodVertex>() as u64;
            let index_offset =
                self.packed_lod_indices.len() as u64 * std::mem::size_of::<u32>() as u64;
            self.packed_lod_vertices.extend_from_slice(&new_vertices);
            self.packed_lod_indices.extend_from_slice(&new_indices);
            queue.write_buffer(
                &self.packed_lod_vertex_buffer,
                vertex_offset,
                bytemuck::cast_slice(&new_vertices),
            );
            queue.write_buffer(
                &self.packed_lod_index_buffer,
                index_offset,
                bytemuck::cast_slice(&new_indices),
            );

            let mut packed_surfaces = Vec::new();
            let surface_start = lod.surface_start as usize;
            let surface_end = surface_start
                .saturating_add(lod.surface_count as usize)
                .min(decoded_surfaces.len());
            for surface in &decoded_surfaces[surface_start..surface_end] {
                let rel_start = surface.index_start.saturating_sub(lod.index_start);
                if rel_start >= lod.index_count {
                    continue;
                }
                packed_surfaces.push(MeshRange {
                    index_start: packed_index_start + rel_start,
                    index_count: surface.index_count.min(lod.index_count - rel_start),
                    base_vertex: 0,
                });
            }
            if packed_surfaces.is_empty() {
                packed_surfaces.push(MeshRange {
                    index_start: packed_index_start,
                    index_count: new_indices.len() as u32,
                    base_vertex: 0,
                });
            }
            out.push(Some(PackedMeshLodRange {
                full: MeshRange {
                    index_start: packed_index_start,
                    index_count: new_indices.len() as u32,
                    base_vertex: 0,
                },
                surface_ranges: Arc::from(packed_surfaces),
                param_index,
            }));
        }
        if self.packed_lod_params.len() > param_upload_start {
            let offset =
                param_upload_start as u64 * std::mem::size_of::<PackedLodParamGpu>() as u64;
            queue.write_buffer(
                &self.packed_lod_param_buffer,
                offset,
                bytemuck::cast_slice(&self.packed_lod_params[param_upload_start..]),
            );
        }
        out
    }

    pub(in super::super) fn ensure_blend_shape_delta_capacity(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        needed: usize,
    ) {
        if needed <= self.blend_shape_delta_capacity {
            return;
        }
        let mut cap = self.blend_shape_delta_capacity.max(1);
        while cap < needed {
            cap *= 2;
        }
        let old_buffer = self.blend_shape_delta_buffer.clone();
        let old_size =
            self.blend_shape_deltas.len() as u64 * std::mem::size_of::<BlendShapeDeltaGpu>() as u64;
        self.blend_shape_delta_capacity = cap;
        self.blend_shape_delta_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_blend_shape_deltas"),
            size: (cap * std::mem::size_of::<BlendShapeDeltaGpu>()) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        if old_size > 0 {
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("perro_blend_shape_delta_growth_copy"),
            });
            encoder.copy_buffer_to_buffer(
                &old_buffer,
                0,
                &self.blend_shape_delta_buffer,
                0,
                old_size,
            );
            queue.submit(Some(encoder.finish()));
        }
        self.rebuild_camera_bind_groups(device);
    }

    pub(in super::super) fn ensure_mesh_buffer_capacity(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        needed_vertices: usize,
        needed_indices: usize,
    ) {
        let mut grew = false;
        let max_buffer_size = device.limits().max_buffer_size as usize;
        let max_vertex_capacity = max_buffer_size
            / std::mem::size_of::<SkinnedMeshVertex>().max(std::mem::size_of::<RigidMeshVertex>());
        let max_index_capacity = max_buffer_size / std::mem::size_of::<u32>();

        if needed_vertices > self.vertex_capacity {
            let cap = bounded_growth_capacity(
                self.vertex_capacity,
                needed_vertices,
                max_vertex_capacity,
            )
            .unwrap_or_else(|| {
                panic!(
                    "mesh vertex data needs {needed_vertices} vertices; device limit is {max_vertex_capacity}"
                )
            });
            self.vertex_capacity = cap;
            self.rigid_vertex_capacity = cap;
            grew = true;
        }

        if needed_indices > self.index_capacity {
            let cap = bounded_growth_capacity(
                self.index_capacity,
                needed_indices,
                max_index_capacity,
            )
            .unwrap_or_else(|| {
                panic!(
                    "mesh index data needs {needed_indices} indices; device limit is {max_index_capacity}"
                )
            });
            self.index_capacity = cap;
            grew = true;
        }

        if grew {
            let old_vertex_buffer = self.vertex_buffer.clone();
            let old_rigid_vertex_buffer = self.rigid_vertex_buffer.clone();
            let old_index_buffer = self.index_buffer.clone();
            let old_vertex_size =
                self.mesh_vertices.len() as u64 * std::mem::size_of::<SkinnedMeshVertex>() as u64;
            let old_rigid_vertex_size = self.rigid_mesh_vertices.len() as u64
                * std::mem::size_of::<RigidMeshVertex>() as u64;
            let old_index_size = self.mesh_indices.len() as u64 * std::mem::size_of::<u32>() as u64;
            self.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_mesh_vertices"),
                size: (self.vertex_capacity * std::mem::size_of::<SkinnedMeshVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            self.rigid_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_mesh_vertices_rigid"),
                size: (self.rigid_vertex_capacity * std::mem::size_of::<RigidMeshVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            self.index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_mesh_indices"),
                size: (self.index_capacity * std::mem::size_of::<u32>()) as u64,
                usage: wgpu::BufferUsages::INDEX
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            if old_vertex_size > 0 || old_rigid_vertex_size > 0 || old_index_size > 0 {
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("perro_mesh_buffer_growth_copy"),
                });
                if old_vertex_size > 0 {
                    encoder.copy_buffer_to_buffer(
                        &old_vertex_buffer,
                        0,
                        &self.vertex_buffer,
                        0,
                        old_vertex_size,
                    );
                }
                if old_rigid_vertex_size > 0 {
                    encoder.copy_buffer_to_buffer(
                        &old_rigid_vertex_buffer,
                        0,
                        &self.rigid_vertex_buffer,
                        0,
                        old_rigid_vertex_size,
                    );
                }
                if old_index_size > 0 {
                    encoder.copy_buffer_to_buffer(
                        &old_index_buffer,
                        0,
                        &self.index_buffer,
                        0,
                        old_index_size,
                    );
                }
                queue.submit([encoder.finish()]);
            }
        }
    }

    /// Drop append-only custom mesh revisions before the shared vertex arena
    /// reaches the device's single-buffer limit. Built-in meshes always occupy
    /// the prefix; every live custom mesh is resolved again by the forced full
    /// prepare that follows this reset.
    pub(in super::super) fn compact_custom_mesh_storage_if_needed(
        &mut self,
        device: &wgpu::Device,
    ) -> bool {
        let max_vertices = device.limits().max_buffer_size as usize
            / std::mem::size_of::<SkinnedMeshVertex>().max(std::mem::size_of::<RigidMeshVertex>());
        if self.mesh_vertices.len() < max_vertices.saturating_mul(3) / 4 {
            return false;
        }

        let builtin_index_len = self
            .builtin_mesh_ranges
            .values()
            .map(|range| range.index_start as usize + range.index_count as usize)
            .max()
            .unwrap_or(0);
        let builtin_vertex_len = self.mesh_indices[..builtin_index_len]
            .iter()
            .copied()
            .max()
            .map_or(0, |index| index as usize + 1);

        self.mesh_vertices.truncate(builtin_vertex_len);
        self.rigid_mesh_vertices.truncate(builtin_vertex_len);
        self.mesh_indices.truncate(builtin_index_len);
        self.packed_lod_vertices.clear();
        self.packed_lod_indices.clear();
        self.blend_shape_deltas.clear();
        self.custom_mesh_ranges.clear();
        true
    }

    pub(in super::super) fn ensure_packed_lod_buffer_capacity(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        needed_vertices: usize,
        needed_indices: usize,
    ) {
        let mut vertex_grew = false;
        let mut index_grew = false;
        if needed_vertices > self.packed_lod_vertex_capacity {
            while self.packed_lod_vertex_capacity < needed_vertices {
                self.packed_lod_vertex_capacity = self.packed_lod_vertex_capacity.max(1) * 2;
            }
            vertex_grew = true;
        }
        if needed_indices > self.packed_lod_index_capacity {
            while self.packed_lod_index_capacity < needed_indices {
                self.packed_lod_index_capacity = self.packed_lod_index_capacity.max(1) * 2;
            }
            index_grew = true;
        }
        if !vertex_grew && !index_grew {
            return;
        }
        let old_vertex_buffer = self.packed_lod_vertex_buffer.clone();
        let old_index_buffer = self.packed_lod_index_buffer.clone();
        let old_vertex_size = self.packed_lod_vertices.len() as u64
            * std::mem::size_of::<PackedRigidLodVertex>() as u64;
        let old_index_size =
            self.packed_lod_indices.len() as u64 * std::mem::size_of::<u32>() as u64;
        if vertex_grew {
            self.packed_lod_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_packed_lod_vertices_rigid"),
                size: (self.packed_lod_vertex_capacity
                    * std::mem::size_of::<PackedRigidLodVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
        }
        if index_grew {
            self.packed_lod_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_packed_lod_indices"),
                size: (self.packed_lod_index_capacity * std::mem::size_of::<u32>()) as u64,
                usage: wgpu::BufferUsages::INDEX
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("perro_packed_lod_growth_copy"),
        });
        if vertex_grew && old_vertex_size > 0 {
            encoder.copy_buffer_to_buffer(
                &old_vertex_buffer,
                0,
                &self.packed_lod_vertex_buffer,
                0,
                old_vertex_size,
            );
        }
        if index_grew && old_index_size > 0 {
            encoder.copy_buffer_to_buffer(
                &old_index_buffer,
                0,
                &self.packed_lod_index_buffer,
                0,
                old_index_size,
            );
        }
        queue.submit([encoder.finish()]);
    }
}
