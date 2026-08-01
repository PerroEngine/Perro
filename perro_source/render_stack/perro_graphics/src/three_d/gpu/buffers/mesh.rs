use super::*;

/// Widest mesh-arena vertex stride (skinned and rigid arenas hold one entry per
/// vertex each, so the skinned stride bounds both).
const MESH_VERTEX_STRIDE: usize = if std::mem::size_of::<SkinnedMeshVertex>()
    > std::mem::size_of::<RigidMeshVertex>()
{
    std::mem::size_of::<SkinnedMeshVertex>()
} else {
    std::mem::size_of::<RigidMeshVertex>()
};

/// Arena floor for GC-driven compaction. Below this the stranded bytes are not
/// worth the full re-resolve (every live mesh is decoded + re-uploaded), so
/// small scenes never churn.
const MESH_ARENA_COMPACT_MIN_BYTES: usize = 16 * 1024 * 1024;

/// Live fraction under which the arena is worth compacting: less than half of
/// the appended bytes still reachable through `custom_mesh_ranges`.
const MESH_ARENA_COMPACT_LIVE_RATIO_DEN: usize = 2;

/// Snapshot of the 3D renderer's reclaimable GPU memory. Same shape as the
/// `PostPerfCounters` pattern: cheap to build, never on a hot path, read by
/// tests and memory diagnostics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Gpu3DMemoryReport {
    /// Vertices appended to the shared mesh arena (builtin prefix included).
    pub mesh_arena_vertices: usize,
    /// Vertices still reachable: builtin prefix + live `custom_mesh_ranges`.
    pub mesh_arena_live_vertices: usize,
    pub mesh_arena_bytes: usize,
    pub mesh_arena_live_bytes: usize,
    pub mesh_index_len: usize,
    /// True while a GC tick has asked the next prepare to compact the arena.
    pub mesh_compact_requested: bool,
    // Per-family buffer capacities (element counts).
    pub vertex_capacity: usize,
    pub index_capacity: usize,
    pub packed_lod_vertex_capacity: usize,
    pub packed_lod_index_capacity: usize,
    pub blend_shape_delta_capacity: usize,
    pub decal_buffer_capacity: usize,
    // Shadow atlas layers: allocated vs what the last shadow update needed.
    pub ray_shadow_layers_allocated: u32,
    pub ray_shadow_layers_used: u32,
    pub spot_shadow_layers_allocated: u32,
    pub spot_shadow_layers_used: u32,
    pub point_shadow_layers_allocated: u32,
    pub point_shadow_layers_used: u32,
    // Decal texture array layers: allocated vs textures holding a layer.
    pub decal_layers_allocated: u32,
    pub decal_layers_live: u32,
}

impl Gpu3D {
    /// Vertices still reachable in the mesh arena: the builtin prefix (never
    /// freed, always the arena's head) plus every live custom mesh range.
    /// `blend_shape_vertex_count` is the mesh's own vertex count, recorded for
    /// every appended mesh (blend shapes or not) in `append_mesh_data`.
    fn mesh_arena_live_vertices(&self) -> usize {
        self.builtin_vertex_len
            + self
                .custom_mesh_ranges
                .values()
                .map(|(_, range)| range.blend_shape_vertex_count as usize)
                .sum::<usize>()
    }

    pub(in super::super) fn memory_report(&self) -> Gpu3DMemoryReport {
        let live_vertices = self.mesh_arena_live_vertices().min(self.mesh_vertex_len);
        Gpu3DMemoryReport {
            mesh_arena_vertices: self.mesh_vertex_len,
            mesh_arena_live_vertices: live_vertices,
            mesh_arena_bytes: self.mesh_vertex_len * MESH_VERTEX_STRIDE,
            mesh_arena_live_bytes: live_vertices * MESH_VERTEX_STRIDE,
            mesh_index_len: self.mesh_index_len,
            mesh_compact_requested: self.mesh_compact_requested,
            vertex_capacity: self.vertex_capacity,
            index_capacity: self.index_capacity,
            packed_lod_vertex_capacity: self.packed_lod_vertex_capacity,
            packed_lod_index_capacity: self.packed_lod_index_capacity,
            blend_shape_delta_capacity: self.blend_shape_delta_capacity,
            decal_buffer_capacity: self.decal_buffer_capacity,
            ray_shadow_layers_allocated: self.ray_shadow_layers_allocated,
            ray_shadow_layers_used: if self.ray_shadow_enabled {
                MAX_SHADOW_RAY_CASCADES as u32
            } else {
                0
            },
            spot_shadow_layers_allocated: self.spot_shadow_layers_allocated,
            spot_shadow_layers_used: self.spot_shadow_count as u32,
            point_shadow_layers_allocated: self.point_shadow_layers_allocated,
            point_shadow_layers_used: self
                .point_shadow_count
                .saturating_mul(POINT_SHADOW_FACE_COUNT) as u32,
            decal_layers_allocated: self.decal_texture_layers,
            decal_layers_live: self.decal_layer_by_texture.len() as u32,
        }
    }

    /// Periodic GC tick for the grow-only GPU memory `shrink_tick` cannot
    /// reclaim: the append-only mesh arena, the shadow atlases and the decal
    /// texture array. Runs between frames (backend GC interval), so it only
    /// ever needs the device.
    pub fn reclaim_memory_tick(&mut self, device: &wgpu::Device) {
        let report = self.memory_report();
        // Mesh arena: dead meshes (scene switch, mesh-revision re-append) stay
        // resident until the device-limit backstop (~340MB desktop) fires.
        // Compaction itself is NOT safe here: it invalidates every resolved
        // mesh range, and the draw batches that reference them are only rebuilt
        // by the forced full prepare that follows it. So the tick only raises a
        // request; `compact_custom_mesh_storage_if_needed` consumes it at the
        // top of the next prepare, which turns it into `force_full_rebuild` in
        // the same frame.
        if report.mesh_arena_bytes >= MESH_ARENA_COMPACT_MIN_BYTES
            && report.mesh_arena_live_bytes * MESH_ARENA_COMPACT_LIVE_RATIO_DEN
                < report.mesh_arena_bytes
        {
            self.mesh_compact_requested = true;
        }
        // Both of these own their contents' lifetime (re-render / re-upload on
        // demand), so they can shrink in place right here.
        self.shrink_shadow_atlases_tick(device);
        self.shrink_decal_texture_tick(device);
    }
}

impl Gpu3D {
    /// Resolve a mesh id/source into its buffer range through the caller-held
    /// cache (the prepare loop `mem::take`s `custom_mesh_ranges` so the hit
    /// path can hand out `&MeshAssetRange` instead of cloning 3 Arcs per draw
    /// per frame).
    #[allow(clippy::too_many_arguments)]
    pub(in super::super) fn resolve_mesh_range<'cache>(
        &mut self,
        cache: &'cache mut AHashMap<MeshID, (u64, MeshAssetRange)>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &ResourceStore,
        mesh_id: MeshID,
        source: &str,
        static_mesh_lookup: Option<StaticMeshLookup>,
    ) -> Option<&'cache MeshAssetRange> {
        let revision = resources.mesh_revision(mesh_id);
        if cache
            .get(&mesh_id)
            .is_some_and(|(cached_revision, _)| *cached_revision == revision)
        {
            return cache.get(&mesh_id).map(|(_, range)| range);
        }
        // Miss path (new mesh or revision bump) is cold; the double map lookup
        // below keeps the hot hit path above borrow-check friendly.
        let range = if let Some(range) = self.builtin_mesh_ranges.get(source).copied() {
            let (bounds_center, bounds_radius) = self
                .builtin_mesh_bounds
                .get(source)
                .copied()
                .unwrap_or(([0.0, 0.0, 0.0], 1.0));
            MeshAssetRange {
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
            }
        } else {
            let decoded = if let Some(mesh) = resources.runtime_mesh_data_by_id(mesh_id) {
                load_mesh_from_source_no_dynamic_lods(
                    source,
                    static_mesh_lookup,
                    Some(mesh.as_ref()),
                )?
            } else {
                let runtime_mesh = resources.runtime_mesh_data(source);
                if let Some(mesh) = runtime_mesh {
                    load_mesh_from_source_no_dynamic_lods(
                        source,
                        static_mesh_lookup,
                        Some(mesh.as_ref()),
                    )?
                } else {
                    load_mesh_from_source(
                        source,
                        static_mesh_lookup,
                        None,
                        self.meshlets_enabled && self.dev_meshlets,
                    )?
                }
            };
            self.append_mesh_data(device, queue, source, decoded)?
        };
        cache.insert(mesh_id, (revision, range));
        cache.get(&mesh_id).map(|(_, range)| range)
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
        let base_vertex = self.mesh_vertex_len as u32;
        let index_start = self.mesh_index_len as u32;
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

        let new_vertex_len = self.mesh_vertex_len + added_vertices.len();
        let new_index_len = self.mesh_index_len + added_indices.len();
        self.ensure_mesh_buffer_capacity(device, queue, new_vertex_len, new_index_len);

        let vertex_offset =
            self.mesh_vertex_len as u64 * std::mem::size_of::<SkinnedMeshVertex>() as u64;
        let rigid_vertex_offset =
            self.rigid_vertex_len as u64 * std::mem::size_of::<RigidMeshVertex>() as u64;
        let index_offset = self.mesh_index_len as u64 * std::mem::size_of::<u32>() as u64;

        self.mesh_vertex_len = new_vertex_len;
        self.rigid_vertex_len += added_rigid_vertices.len();
        self.mesh_index_len = new_index_len;

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

        let blend_shape_delta_start = self.blend_shape_delta_len as u32;
        let blend_shape_target_count = decoded_blend_shapes.len() as u32;
        let blend_shape_vertex_start = base_vertex;
        let blend_shape_vertex_count = decoded_vertices.len() as u32;
        if !decoded_blend_shapes.is_empty() {
            let added_delta_count = decoded_blend_shapes.len() * decoded_vertices.len();
            let old_delta_len = self.blend_shape_delta_len;
            self.ensure_blend_shape_delta_capacity(
                device,
                queue,
                old_delta_len + added_delta_count,
            );
            self.blend_shape_delta_scratch.clear();
            self.blend_shape_delta_scratch.reserve(added_delta_count);
            for shape in &decoded_blend_shapes {
                for vertex_index in 0..decoded_vertices.len() {
                    let vertex = shape.vertices.get(vertex_index).copied();
                    self.blend_shape_delta_scratch.push(BlendShapeDeltaGpu {
                        position_delta: vertex
                            .map(|v| {
                                [
                                    v.position_delta[0],
                                    v.position_delta[1],
                                    v.position_delta[2],
                                ]
                            })
                            .unwrap_or([0.0; 3]),
                        packed_normal_delta: vertex
                            .map(|v| {
                                pack_blend_normal_delta([
                                    v.normal_delta[0],
                                    v.normal_delta[1],
                                    v.normal_delta[2],
                                ])
                            })
                            .unwrap_or(0),
                    });
                }
            }
            queue.write_buffer(
                &self.blend_shape_delta_buffer,
                old_delta_len as u64 * std::mem::size_of::<BlendShapeDeltaGpu>() as u64,
                bytemuck::cast_slice(&self.blend_shape_delta_scratch),
            );
            self.blend_shape_delta_scratch.clear();
            self.blend_shape_delta_len = old_delta_len + added_delta_count;
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

            let packed_index_start = self.packed_lod_index_len as u32;
            let packed_vertex_start = self.packed_lod_vertex_len as u32;
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
                self.packed_lod_vertex_len + new_vertices.len(),
                self.packed_lod_index_len + new_indices.len(),
            );
            let vertex_offset = self.packed_lod_vertex_len as u64
                * std::mem::size_of::<PackedRigidLodVertex>() as u64;
            let index_offset = self.packed_lod_index_len as u64 * std::mem::size_of::<u32>() as u64;
            self.packed_lod_vertex_len += new_vertices.len();
            self.packed_lod_index_len += new_indices.len();
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
            self.blend_shape_delta_len as u64 * std::mem::size_of::<BlendShapeDeltaGpu>() as u64;
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
                self.mesh_vertex_len as u64 * std::mem::size_of::<SkinnedMeshVertex>() as u64;
            let old_rigid_vertex_size =
                self.rigid_vertex_len as u64 * std::mem::size_of::<RigidMeshVertex>() as u64;
            let old_index_size = self.mesh_index_len as u64 * std::mem::size_of::<u32>() as u64;
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
            // Release the old handles as soon as the copy is submitted so the
            // driver can reclaim them; holding them longer keeps a 3x peak
            // (old + new arenas) alive.
            drop(old_vertex_buffer);
            drop(old_rigid_vertex_buffer);
            drop(old_index_buffer);
        }
    }

    /// Drop append-only custom mesh revisions before the shared vertex arena
    /// reaches the device's single-buffer limit. Built-in meshes always occupy
    /// the prefix; every live custom mesh is resolved again by the forced full
    /// prepare that follows this reset.
    ///
    /// Two triggers: the device-limit backstop below, and a request raised by
    /// `reclaim_memory_tick` when most of the arena went dead. The GC tick
    /// cannot compact directly -- this must run at the top of a prepare so the
    /// `true` return can force a full rebuild in the same frame, before
    /// anything draws through the invalidated ranges.
    pub(in super::super) fn compact_custom_mesh_storage_if_needed(
        &mut self,
        device: &wgpu::Device,
    ) -> bool {
        let requested = std::mem::take(&mut self.mesh_compact_requested);
        let max_vertices = device.limits().max_buffer_size as usize / MESH_VERTEX_STRIDE;
        if !requested && self.mesh_vertex_len < max_vertices.saturating_mul(3) / 4 {
            return false;
        }
        // Nothing appended past the builtin prefix: compacting would only cost
        // a pointless full rebuild.
        if self.mesh_vertex_len <= self.builtin_vertex_len && self.custom_mesh_ranges.is_empty() {
            return false;
        }

        let builtin_index_len = self
            .builtin_mesh_ranges
            .values()
            .map(|range| range.index_start as usize + range.index_count as usize)
            .max()
            .unwrap_or(0);

        self.mesh_vertex_len = self.builtin_vertex_len;
        self.rigid_vertex_len = self.builtin_vertex_len;
        self.mesh_index_len = builtin_index_len;
        self.packed_lod_vertex_len = 0;
        self.packed_lod_index_len = 0;
        // Packed-LOD params are only ever referenced by custom mesh ranges (the
        // builtin prefix has no packed LODs), so the reset drops them with the
        // ranges instead of letting the arena keep growing across compactions.
        self.packed_lod_params.clear();
        self.blend_shape_delta_len = 0;
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
        let old_vertex_size =
            self.packed_lod_vertex_len as u64 * std::mem::size_of::<PackedRigidLodVertex>() as u64;
        let old_index_size = self.packed_lod_index_len as u64 * std::mem::size_of::<u32>() as u64;
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

#[cfg(test)]
#[path = "../../../../tests/unit/three_d_memory_reclaim_tests.rs"]
mod memory_reclaim_tests;
