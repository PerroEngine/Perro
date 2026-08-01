//! Process-wide shared mesh arena.
//!
//! Every `Gpu3D` (the main view plus one per camera stream) used to own a full
//! set of mesh vertex/index arenas, so a custom mesh drawn in the main view and
//! in two streams was decoded and uploaded three times -- at 36B/vertex on the
//! rigid path and 48B/vertex on the skinned one. This store is the mesh-side
//! analogue of `SharedTextureStore`: one arena set, one `MeshAssetEntry` map,
//! one builtin prefix, owned by `Gpu` and handed to every `Gpu3D::prepare`.
//!
//! `Gpu3D` keeps only *clones* of the arena's `wgpu::Buffer` handles (wgpu
//! buffers are internally refcounted, so a clone is a refcount bump) plus the
//! two generation counters below. Draw code therefore still reads
//! `self.vertex_buffer` and friends unchanged; `Gpu3D::sync_mesh_arena` is the
//! single place that refreshes them.
//!
//! Two generations, because the two kinds of invalidation cost very different
//! amounts:
//!
//! * `buffer_generation` -- any arena buffer was reallocated (grow or shrink).
//!   Consumers re-clone the seven handles. Cheap.
//! * `bind_generation` -- one of the two arena buffers that live *inside* bind
//!   groups (`blend_shape_delta_buffer`, `packed_lod_param_buffer`) was
//!   reallocated. Consumers must also run `rebuild_camera_bind_groups`
//!   (~101 bind groups), so it is bumped only when that is actually required.
//! * `layout_generation` -- compaction invalidated every resolved range.
//!   Consumers force a full draw-batch rebuild.

use super::*;
use perro_graphics_assets::{BorrowedBlendShape, BorrowedMesh, borrow_runtime_mesh};

/// Per-vertex stride of each mesh arena. The two arenas are independent: a mesh
/// is appended to the skinned one only when a skinned draw asks for it, so
/// rigid-only content pays `RIGID_VERTEX_STRIDE` and nothing more.
const SKINNED_VERTEX_STRIDE: usize = std::mem::size_of::<SkinnedMeshVertex>();
const RIGID_VERTEX_STRIDE: usize = std::mem::size_of::<RigidMeshVertex>();

/// Arena floor for GC-driven compaction. Below this the stranded bytes are not
/// worth the full re-resolve (every live mesh is decoded + re-uploaded), so
/// small scenes never churn.
const MESH_ARENA_COMPACT_MIN_BYTES: usize = 16 * 1024 * 1024;

/// Live fraction under which the arena is worth compacting: less than half of
/// the appended bytes still reachable through `custom_mesh_ranges`.
const MESH_ARENA_COMPACT_LIVE_RATIO_DEN: usize = 2;

/// GC ticks a compaction request may be deferred for arena churn before it is
/// raised anyway. A scene transition is exactly the shape that trips the live
/// ratio for the wrong reason - the outgoing scene's meshes are dropped while
/// the incoming scene's are still streaming in, so the arena looks half dead
/// while it is really mid-refill - and compacting there pays a full CPU decode,
/// repack and re-upload of every mesh the new scene resolved so far, in one
/// frame, only to have the rest of the load append on top of it. Waiting for a
/// tick with no appends puts the compaction after the load instead of inside
/// it. The cap keeps content that streams meshes forever (open worlds) from
/// deferring the reclaim indefinitely; at the 60-frame GC interval it is a few
/// seconds of churn before compaction happens regardless.
pub(in super::super) const MESH_ARENA_COMPACT_MAX_DEFER_TICKS: u32 = 5;

/// Snapshot of the shared mesh arena's GPU memory. Reported once for the whole
/// process, not once per view -- that is the point of the store.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct MeshArenaMemoryReport {
    /// Vertices appended across both mesh arenas (builtin prefix counted in
    /// each, since both hold it).
    pub mesh_arena_vertices: usize,
    /// Vertices still reachable: builtin prefix + live `custom_mesh_ranges`.
    pub mesh_arena_live_vertices: usize,
    /// Bytes across both arenas, each at its own stride.
    pub mesh_arena_bytes: usize,
    pub mesh_arena_live_bytes: usize,
    /// Rigid arena (36B/vertex): every mesh a rigid-path draw resolved.
    pub rigid_arena_vertices: usize,
    pub rigid_arena_bytes: usize,
    /// Skinned arena (48B/vertex): only meshes a skinned-path draw resolved.
    pub skinned_arena_vertices: usize,
    pub skinned_arena_bytes: usize,
    pub mesh_index_len: usize,
    /// True while a GC tick has asked the next prepare to compact the arena.
    pub mesh_compact_requested: bool,
    // Per-family buffer capacities (element counts).
    pub vertex_capacity: usize,
    pub rigid_vertex_capacity: usize,
    pub index_capacity: usize,
    pub packed_lod_vertex_capacity: usize,
    pub packed_lod_index_capacity: usize,
    pub packed_lod_param_capacity: usize,
    pub blend_shape_delta_capacity: usize,
}

/// The seven arena buffer handles a `Gpu3D` mirrors, plus the generations they
/// were taken at.
pub(in super::super) struct MeshArenaHandles {
    pub vertex_buffer: wgpu::Buffer,
    pub rigid_vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub packed_lod_vertex_buffer: wgpu::Buffer,
    pub packed_lod_index_buffer: wgpu::Buffer,
    pub packed_lod_param_buffer: wgpu::Buffer,
    pub blend_shape_delta_buffer: wgpu::Buffer,
    pub buffer_generation: u64,
    pub bind_generation: u64,
    pub layout_generation: u64,
}

#[derive(Default)]
struct MeshArenaShrink {
    rigid_vertices: ShrinkTracker,
    skinned_vertices: ShrinkTracker,
    indices: ShrinkTracker,
    packed_lod_vertices: ShrinkTracker,
    packed_lod_indices: ShrinkTracker,
    packed_lod_params: ShrinkTracker,
    blend_shape_deltas: ShrinkTracker,
}

/// The arena bytes one already-resolved variant owns, as the in-place rewrite
/// path needs them. Read off `MeshAssetRange`, which records the mesh's own
/// vertex block in `blend_shape_vertex_start/count` for every appended mesh,
/// blend shapes or not.
#[derive(Clone, Copy)]
struct InPlaceSlot {
    index_start: u32,
    index_count: u32,
    vertex_start: u32,
    vertex_count: u32,
    blend_shape_delta_start: u32,
    blend_shape_target_count: u32,
}

fn in_place_slot(range: &MeshAssetRange) -> InPlaceSlot {
    InPlaceSlot {
        index_start: range.full.index_start,
        index_count: range.full.index_count,
        vertex_start: range.blend_shape_vertex_start,
        vertex_count: range.blend_shape_vertex_count,
        blend_shape_delta_start: range.blend_shape_delta_start,
        blend_shape_target_count: range.blend_shape_target_count,
    }
}

/// Decoded surface ranges shifted onto the mesh's index block, with the whole
/// mesh as the fallback surface when the decode carried none.
fn uploaded_surface_ranges(
    decoded: &[MeshRange],
    index_start: u32,
    index_count: u32,
) -> Vec<MeshRange> {
    if decoded.is_empty() {
        return vec![MeshRange {
            index_start,
            index_count,
            base_vertex: 0,
        }];
    }
    decoded
        .iter()
        .map(|range| MeshRange {
            index_start: index_start + range.index_start,
            index_count: range.index_count,
            base_vertex: 0,
        })
        .collect()
}

fn uploaded_meshlets(decoded: &[DecodedMeshlet], index_start: u32) -> Vec<MeshletRange> {
    decoded
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
        .collect()
}

/// Pack one mesh's blend-shape deltas into the caller's scratch buffer, shape
/// major (the layout the delta arena stores and the skinning shaders index).
fn fill_blend_shape_deltas(
    scratch: &mut Vec<BlendShapeDeltaGpu>,
    shapes: &[BorrowedBlendShape<'_>],
    vertex_count: usize,
) {
    scratch.clear();
    scratch.reserve(shapes.len() * vertex_count);
    for shape in shapes {
        for vertex_index in 0..vertex_count {
            let vertex = shape.vertices.get(vertex_index).copied();
            scratch.push(BlendShapeDeltaGpu {
                position_delta: vertex
                    .map(|v| v.position_delta)
                    .unwrap_or([0.0; 3]),
                packed_normal_delta: vertex
                    .map(|v| pack_blend_normal_delta(v.normal_delta))
                    .unwrap_or(0),
            });
        }
    }
}

/// One mesh arena set shared by every `Gpu3D`.
pub(crate) struct SharedMeshArena {
    vertex_buffer: wgpu::Buffer,
    rigid_vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    packed_lod_vertex_buffer: wgpu::Buffer,
    packed_lod_index_buffer: wgpu::Buffer,
    packed_lod_param_buffer: wgpu::Buffer,
    blend_shape_delta_buffer: wgpu::Buffer,

    vertex_capacity: usize,
    rigid_vertex_capacity: usize,
    index_capacity: usize,
    packed_lod_vertex_capacity: usize,
    packed_lod_index_capacity: usize,
    packed_lod_param_capacity: usize,
    blend_shape_delta_capacity: usize,

    // Element counts of the GPU mesh arenas (no CPU mirrors are retained; the
    // buffers are append-only and grown via copy_buffer_to_buffer).
    // `mesh_vertex_len` is the SKINNED arena (48B/vertex, `vertex_buffer`) and
    // `rigid_vertex_len` the rigid one (36B/vertex, `rigid_vertex_buffer`).
    // They share only the builtin prefix; past it each has its own cursor and a
    // mesh lands in whichever arena its draw path binds (see `MeshAssetEntry`).
    mesh_vertex_len: usize,
    rigid_vertex_len: usize,
    mesh_index_len: usize,
    packed_lod_vertex_len: usize,
    packed_lod_index_len: usize,
    blend_shape_delta_len: usize,
    /// Vertex count of the builtin-mesh prefix; compaction truncates back to it.
    builtin_vertex_len: usize,

    packed_lod_params: Vec<PackedLodParamGpu>,
    /// Reused per-mesh upload staging; cleared after each upload.
    blend_shape_delta_scratch: Vec<BlendShapeDeltaGpu>,

    builtin_mesh_ranges: AHashMap<&'static str, MeshRange>,
    builtin_mesh_bounds: AHashMap<&'static str, ([f32; 3], f32)>,
    builtin_meshlets: AHashMap<&'static str, Arc<[MeshletRange]>>,
    /// Resolved custom-mesh ranges, shared by every view. `prepare` `mem::take`s
    /// this so `resolve_mesh_range` can hand out `&MeshAssetRange` while the
    /// arena stays mutably borrowed.
    pub(in super::super) custom_mesh_ranges: AHashMap<MeshID, MeshAssetEntry>,

    mesh_compact_requested: bool,
    /// Meshes appended since the arena was created. `reclaim_tick` compares it
    /// against `appends_at_last_reclaim_tick` to tell a settled arena from one
    /// still being filled by an in-flight scene load.
    appends: u64,
    appends_at_last_reclaim_tick: u64,
    /// Consecutive `reclaim_tick`s that wanted to compact but deferred for
    /// churn. Capped by `MESH_ARENA_COMPACT_MAX_DEFER_TICKS`.
    compact_deferred_ticks: u32,
    /// Decode-time meshlet decision. Identical for every `Gpu3D` (both come
    /// from the same `Gpu` config), which is what makes one shared arena legal.
    meshlets_enabled: bool,
    dev_meshlets: bool,

    buffer_generation: u64,
    bind_generation: u64,
    layout_generation: u64,

    shrink: MeshArenaShrink,
}

impl SharedMeshArena {
    pub(crate) fn new(device: &wgpu::Device, meshlets_enabled: bool, dev_meshlets: bool) -> Self {
        let (vertices, indices, builtin_mesh_ranges, builtin_meshlets) =
            build_builtin_mesh_buffer();
        let builtin_mesh_bounds =
            compute_builtin_mesh_bounds(&vertices, &indices, &builtin_mesh_ranges);
        let skinned_vertices: Vec<SkinnedMeshVertex> =
            vertices.iter().map(pack_skinned_mesh_vertex).collect();
        let rigid_vertices: Vec<RigidMeshVertex> =
            vertices.iter().map(pack_rigid_mesh_vertex).collect();
        let builtin_vertex_len = vertices.len();
        let builtin_index_len = indices.len();
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("perro_builtin_mesh_vertices"),
            contents: bytemuck::cast_slice(&skinned_vertices),
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("perro_builtin_mesh_indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });
        let rigid_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("perro_builtin_mesh_vertices_rigid"),
            contents: bytemuck::cast_slice(&rigid_vertices),
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });
        let packed_lod_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_packed_lod_vertices_rigid"),
            size: std::mem::size_of::<PackedRigidLodVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let packed_lod_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_packed_lod_indices"),
            size: std::mem::size_of::<u32>() as u64,
            usage: wgpu::BufferUsages::INDEX
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let blend_shape_delta_capacity = 64usize;
        let blend_shape_delta_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_blend_shape_deltas"),
            size: (blend_shape_delta_capacity * std::mem::size_of::<BlendShapeDeltaGpu>()) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let packed_lod_param_capacity = 64usize;
        let packed_lod_param_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_packed_lod_params"),
            size: (packed_lod_param_capacity * std::mem::size_of::<PackedLodParamGpu>()) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        Self {
            vertex_buffer,
            rigid_vertex_buffer,
            index_buffer,
            packed_lod_vertex_buffer,
            packed_lod_index_buffer,
            packed_lod_param_buffer,
            blend_shape_delta_buffer,
            vertex_capacity: builtin_vertex_len.max(1),
            rigid_vertex_capacity: builtin_vertex_len.max(1),
            index_capacity: builtin_index_len.max(1),
            packed_lod_vertex_capacity: 1,
            packed_lod_index_capacity: 1,
            packed_lod_param_capacity,
            blend_shape_delta_capacity,
            mesh_vertex_len: builtin_vertex_len,
            rigid_vertex_len: builtin_vertex_len,
            mesh_index_len: builtin_index_len,
            packed_lod_vertex_len: 0,
            packed_lod_index_len: 0,
            blend_shape_delta_len: 0,
            builtin_vertex_len,
            packed_lod_params: Vec::new(),
            blend_shape_delta_scratch: Vec::new(),
            builtin_mesh_ranges,
            builtin_mesh_bounds,
            builtin_meshlets,
            custom_mesh_ranges: AHashMap::new(),
            mesh_compact_requested: false,
            appends: 0,
            appends_at_last_reclaim_tick: 0,
            compact_deferred_ticks: 0,
            meshlets_enabled,
            dev_meshlets,
            buffer_generation: 1,
            bind_generation: 1,
            layout_generation: 1,
            shrink: MeshArenaShrink::default(),
        }
    }

    /// Current buffer handles + generations, for a `Gpu3D` to mirror.
    pub(in super::super) fn handles(&self) -> MeshArenaHandles {
        MeshArenaHandles {
            vertex_buffer: self.vertex_buffer.clone(),
            rigid_vertex_buffer: self.rigid_vertex_buffer.clone(),
            index_buffer: self.index_buffer.clone(),
            packed_lod_vertex_buffer: self.packed_lod_vertex_buffer.clone(),
            packed_lod_index_buffer: self.packed_lod_index_buffer.clone(),
            packed_lod_param_buffer: self.packed_lod_param_buffer.clone(),
            blend_shape_delta_buffer: self.blend_shape_delta_buffer.clone(),
            buffer_generation: self.buffer_generation,
            bind_generation: self.bind_generation,
            layout_generation: self.layout_generation,
        }
    }

    pub(in super::super) fn buffer_generation(&self) -> u64 {
        self.buffer_generation
    }

    pub(in super::super) fn bind_generation(&self) -> u64 {
        self.bind_generation
    }

    pub(in super::super) fn layout_generation(&self) -> u64 {
        self.layout_generation
    }

    /// Drop resolved ranges for meshes the resource store no longer holds.
    pub(in super::super) fn retain_live_meshes(&mut self, resources: &ResourceStore) {
        self.custom_mesh_ranges
            .retain(|mesh_id, _| resources.has_mesh(*mesh_id));
    }

    /// Vertices still reachable in each mesh arena: the builtin prefix (never
    /// freed, always each arena's head) plus every live custom mesh range that
    /// landed in that arena. `blend_shape_vertex_count` is the mesh's own
    /// vertex count, recorded for every appended mesh (blend shapes or not) in
    /// `append_mesh_data`.
    ///
    /// Returns `(rigid, skinned)`.
    fn live_vertices(&self) -> (usize, usize) {
        let mut rigid = self.builtin_vertex_len;
        let mut skinned = self.builtin_vertex_len;
        for entry in self.custom_mesh_ranges.values() {
            if let Some(range) = entry.rigid.as_ref() {
                rigid += range.blend_shape_vertex_count as usize;
            }
            if let Some(range) = entry.skinned.as_ref() {
                skinned += range.blend_shape_vertex_count as usize;
            }
        }
        (
            rigid.min(self.rigid_vertex_len),
            skinned.min(self.mesh_vertex_len),
        )
    }

    pub(crate) fn memory_report(&self) -> MeshArenaMemoryReport {
        let (live_rigid, live_skinned) = self.live_vertices();
        MeshArenaMemoryReport {
            mesh_arena_vertices: self.rigid_vertex_len + self.mesh_vertex_len,
            mesh_arena_live_vertices: live_rigid + live_skinned,
            mesh_arena_bytes: self.rigid_vertex_len * RIGID_VERTEX_STRIDE
                + self.mesh_vertex_len * SKINNED_VERTEX_STRIDE,
            mesh_arena_live_bytes: live_rigid * RIGID_VERTEX_STRIDE
                + live_skinned * SKINNED_VERTEX_STRIDE,
            rigid_arena_vertices: self.rigid_vertex_len,
            rigid_arena_bytes: self.rigid_vertex_len * RIGID_VERTEX_STRIDE,
            skinned_arena_vertices: self.mesh_vertex_len,
            skinned_arena_bytes: self.mesh_vertex_len * SKINNED_VERTEX_STRIDE,
            mesh_index_len: self.mesh_index_len,
            mesh_compact_requested: self.mesh_compact_requested,
            vertex_capacity: self.vertex_capacity,
            rigid_vertex_capacity: self.rigid_vertex_capacity,
            index_capacity: self.index_capacity,
            packed_lod_vertex_capacity: self.packed_lod_vertex_capacity,
            packed_lod_index_capacity: self.packed_lod_index_capacity,
            packed_lod_param_capacity: self.packed_lod_param_capacity,
            blend_shape_delta_capacity: self.blend_shape_delta_capacity,
        }
    }

    /// Periodic GC tick for the grow-only arena bytes `shrink_tick` cannot
    /// reclaim. Runs between frames (backend GC interval) once for the whole
    /// process, not per view.
    ///
    /// Compaction itself is NOT safe here: it invalidates every resolved mesh
    /// range, and the draw batches that reference them are only rebuilt by the
    /// forced full prepare that follows it. So the tick only raises a request;
    /// `compact_if_needed` consumes it at the top of the next prepare, which
    /// turns it into `force_full_rebuild` in every view in the same frame (via
    /// `layout_generation`).
    pub(crate) fn reclaim_tick(&mut self) {
        let appended_since_last_tick = self.appends != self.appends_at_last_reclaim_tick;
        self.appends_at_last_reclaim_tick = self.appends;
        let report = self.memory_report();
        let worth_compacting = report.mesh_arena_bytes >= MESH_ARENA_COMPACT_MIN_BYTES
            && report.mesh_arena_live_bytes * MESH_ARENA_COMPACT_LIVE_RATIO_DEN
                < report.mesh_arena_bytes;
        if !worth_compacting {
            self.compact_deferred_ticks = 0;
            return;
        }
        // Meshes landed since the previous tick, so the "half the arena is
        // dead" reading is most likely a scene transition mid-refill, not a
        // settled arena worth rebuilding. Wait for a quiet tick (see
        // MESH_ARENA_COMPACT_MAX_DEFER_TICKS).
        if appended_since_last_tick
            && self.compact_deferred_ticks < MESH_ARENA_COMPACT_MAX_DEFER_TICKS
        {
            self.compact_deferred_ticks += 1;
            return;
        }
        self.compact_deferred_ticks = 0;
        self.mesh_compact_requested = true;
    }

    /// Resolve a mesh id/source into its buffer range through the caller-held
    /// cache (the prepare loop `mem::take`s `custom_mesh_ranges` so the hit
    /// path can hand out `&MeshAssetRange` instead of cloning 3 Arcs per draw
    /// per frame).
    ///
    /// `skinned` must match the `RenderPath3D` the caller will batch this draw
    /// on: the returned ranges index the skinned arena when it is true and the
    /// rigid arena when it is false, and a mesh is uploaded to an arena only
    /// the first time a draw on that path asks for it -- from *any* view, which
    /// is the whole point of sharing.
    #[allow(clippy::too_many_arguments)]
    pub(in super::super) fn resolve_mesh_range<'cache>(
        &mut self,
        cache: &'cache mut AHashMap<MeshID, MeshAssetEntry>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &ResourceStore,
        mesh_id: MeshID,
        source: &str,
        static_mesh_lookup: Option<StaticMeshLookup>,
        skinned: bool,
    ) -> Option<&'cache MeshAssetRange> {
        let revision = resources.mesh_revision(mesh_id);
        if cache
            .get(&mesh_id)
            .is_some_and(|entry| entry.revision == revision && entry.variant(skinned).is_some())
        {
            return cache.get(&mesh_id).and_then(|entry| entry.variant(skinned));
        }
        // Miss path (new mesh, revision bump, or first draw on this path) is
        // cold; the double map lookup below keeps the hot hit path above
        // borrow-check friendly.
        //
        // Set when an in-place update refreshed the *other* arena's copy of the
        // same mesh, which must then survive the revision reset below.
        let mut refreshed_other = None;
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
        } else if let Some(runtime) = resources
            .runtime_mesh_data_by_id(mesh_id)
            .or_else(|| resources.runtime_mesh_data(source))
        {
            // Runtime meshes are borrowed straight out of the resource store
            // (zero copy, see `borrow_runtime_mesh`); only file-backed sources
            // below decode into an owned `DecodedMesh`.
            let mesh = borrow_runtime_mesh(runtime.as_ref())?;
            // Revision bump whose new decode is the same shape as the old one:
            // overwrite the bytes the mesh already owns instead of appending a
            // second copy and stranding the first. A mesh rewritten every frame
            // otherwise walks the arena to the compaction floor in seconds and
            // pays a full re-resolve of every live mesh in one frame.
            let stale = cache
                .get(&mesh_id)
                .filter(|entry| entry.revision != revision);
            let slot = stale
                .and_then(|entry| entry.variant(skinned))
                .map(in_place_slot);
            let other_slot = stale
                .and_then(|entry| entry.variant(!skinned))
                .map(in_place_slot);
            let updated =
                slot.and_then(|slot| self.update_mesh_data_in_place(queue, &mesh, slot, skinned));
            match updated {
                Some(range) => {
                    // The other arena holds the same (now stale) geometry, so
                    // it is refreshed in place too. Both variants share the
                    // decode's counts, so if this one fit, that one does.
                    refreshed_other = other_slot.and_then(|slot| {
                        self.update_mesh_data_in_place(queue, &mesh, slot, !skinned)
                    });
                    range
                }
                None => self.append_borrowed_mesh_data(device, queue, &mesh, skinned)?,
            }
        } else {
            // File-backed: the decode is owned already, and a static source has
            // no revision to bump, so there is nothing to rewrite in place.
            let decoded = load_mesh_from_source(
                source,
                static_mesh_lookup,
                None,
                self.meshlets_enabled && self.dev_meshlets,
            )?;
            self.append_mesh_data(device, queue, source, decoded, skinned)?
        };
        let entry = cache.entry(mesh_id).or_default();
        if entry.revision != revision {
            // Revision bump: unless it was refreshed in place above, the old
            // upload is stranded in the arena until the next compaction, so
            // both variants must be re-resolved.
            entry.revision = revision;
            entry.rigid = None;
            entry.skinned = None;
        }
        if let Some(other) = refreshed_other {
            *entry.variant_mut(!skinned) = Some(other);
        }
        *entry.variant_mut(skinned) = Some(range);
        cache.get(&mesh_id).and_then(|entry| entry.variant(skinned))
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

    /// Append one mesh to a single vertex arena -- the skinned one when
    /// `skinned`, the rigid one otherwise -- plus its own index block.
    ///
    /// The uploaded indices are absolute (`base_vertex` is folded in and
    /// `MeshRange::base_vertex` stays 0), so the returned ranges are only valid
    /// against the arena they were appended to. A mesh drawn on both paths is
    /// appended twice, once per arena, which is why `MeshAssetEntry` keeps a
    /// slot for each.
    /// Owned-decode entry point, for file-backed meshes. Runtime meshes take
    /// `append_borrowed_mesh_data` instead, with no per-vertex copy.
    pub(in super::super) fn append_mesh_data(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _source: &str,
        decoded: DecodedMesh,
        skinned: bool,
    ) -> Option<MeshAssetRange> {
        self.append_borrowed_mesh_data(device, queue, &decoded.as_borrowed(), skinned)
    }

    /// `append_mesh_data` against a mesh whose vertices/indices are borrowed
    /// rather than owned -- the shape a runtime mesh arrives in, so a procedural
    /// mesh is packed straight out of the resource store.
    pub(in super::super) fn append_borrowed_mesh_data(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mesh: &BorrowedMesh<'_>,
        skinned: bool,
    ) -> Option<MeshAssetRange> {
        if mesh.vertices.is_empty() || mesh.indices.is_empty() {
            return None;
        }
        // Churn marker for `reclaim_tick`: an arena still taking appends is
        // mid-load, and its live-byte ratio must not be trusted.
        self.appends = self.appends.wrapping_add(1);
        let decoded_vertices = mesh.vertices;
        let decoded_surface_ranges: &[MeshRange] = &mesh.surface_ranges;
        let decoded_blend_shapes = mesh.blend_shapes.as_slice();
        let decoded_meshlets = mesh.meshlets;
        let decoded_lods = mesh.lods;
        let base_vertex = if skinned {
            self.mesh_vertex_len as u32
        } else {
            self.rigid_vertex_len as u32
        };
        let index_start = self.mesh_index_len as u32;
        let index_count = mesh.indices.len() as u32;

        let (bounds_center, bounds_radius) = mesh_bounds_from_vertices(decoded_vertices)?;
        let surface_ranges =
            uploaded_surface_ranges(decoded_surface_ranges, index_start, index_count);
        let vertex_count = decoded_vertices.len();
        let added_indices: Vec<u32> = mesh.indices.iter().map(|idx| idx + base_vertex).collect();

        let new_index_len = self.mesh_index_len + added_indices.len();
        let new_skinned_len = self.mesh_vertex_len + if skinned { vertex_count } else { 0 };
        let new_rigid_len = self.rigid_vertex_len + if skinned { 0 } else { vertex_count };
        self.ensure_mesh_buffer_capacity(
            device,
            queue,
            new_skinned_len,
            new_rigid_len,
            new_index_len,
        );

        let index_offset = self.mesh_index_len as u64 * std::mem::size_of::<u32>() as u64;
        if skinned {
            let added_vertices: Vec<SkinnedMeshVertex> = decoded_vertices
                .iter()
                .map(pack_skinned_mesh_vertex)
                .collect();
            let vertex_offset = self.mesh_vertex_len as u64 * SKINNED_VERTEX_STRIDE as u64;
            queue.write_buffer(
                &self.vertex_buffer,
                vertex_offset,
                bytemuck::cast_slice(&added_vertices),
            );
        } else {
            let added_vertices: Vec<RigidMeshVertex> = decoded_vertices
                .iter()
                .map(pack_rigid_mesh_vertex)
                .collect();
            let vertex_offset = self.rigid_vertex_len as u64 * RIGID_VERTEX_STRIDE as u64;
            queue.write_buffer(
                &self.rigid_vertex_buffer,
                vertex_offset,
                bytemuck::cast_slice(&added_vertices),
            );
        }
        self.mesh_vertex_len = new_skinned_len;
        self.rigid_vertex_len = new_rigid_len;
        self.mesh_index_len = new_index_len;

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
            let mut scratch = std::mem::take(&mut self.blend_shape_delta_scratch);
            fill_blend_shape_deltas(&mut scratch, decoded_blend_shapes, decoded_vertices.len());
            queue.write_buffer(
                &self.blend_shape_delta_buffer,
                old_delta_len as u64 * std::mem::size_of::<BlendShapeDeltaGpu>() as u64,
                bytemuck::cast_slice(&scratch),
            );
            scratch.clear();
            self.blend_shape_delta_scratch = scratch;
            self.blend_shape_delta_len = old_delta_len + added_delta_count;
        }

        let full = MeshRange {
            index_start,
            index_count,
            base_vertex: 0,
        };

        let meshlets_arc: Arc<[MeshletRange]> =
            Arc::from(uploaded_meshlets(decoded_meshlets, index_start));
        let surface_ranges_arc: Arc<[MeshRange]> = Arc::from(surface_ranges);
        // Packed LODs are a rigid-path-only optimisation (prepare gates
        // `packed_lod` on `RenderPath3D::Rigid`), so the skinned variant never
        // reads them -- building them would only burn arena bytes.
        let packed_lods = if skinned {
            vec![None; decoded_lods.len()]
        } else {
            self.append_packed_lod_data(AppendPackedLodDataArgs {
                device,
                queue,
                vertices: decoded_vertices,
                mesh_indices: &added_indices,
                base_vertex,
                decoded_lods,
                decoded_surfaces: decoded_surface_ranges,
            })
        };
        let lods = build_mesh_lod_ranges(BuildMeshLodRangesArgs {
            index_start,
            index_count,
            decoded_surfaces: decoded_surface_ranges,
            uploaded_surfaces: &surface_ranges_arc,
            decoded_meshlets,
            uploaded_meshlets: &meshlets_arc,
            decoded_lods,
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

    /// Overwrite a mesh already resident in the arena with a same-shaped decode,
    /// keeping its range identity: same vertex block, same index block, same
    /// base vertex, no `layout_generation` bump, no stranded bytes. Retained
    /// draw batches stay valid, which is what makes this legal without a forced
    /// rebuild.
    ///
    /// Returns `None` when the new decode does not fit the bytes the old one
    /// owns -- vertex count, index count or blend-shape target count changed, or
    /// it carries packed LODs, which are appended and never overwritten -- and
    /// the caller falls back to `append_borrowed_mesh_data`.
    fn update_mesh_data_in_place(
        &mut self,
        queue: &wgpu::Queue,
        mesh: &BorrowedMesh<'_>,
        slot: InPlaceSlot,
        skinned: bool,
    ) -> Option<MeshAssetRange> {
        if mesh.vertices.len() != slot.vertex_count as usize
            || mesh.indices.len() != slot.index_count as usize
            || mesh.blend_shapes.len() != slot.blend_shape_target_count as usize
            || mesh.lods.len() > 1
        {
            return None;
        }
        let (bounds_center, bounds_radius) = mesh_bounds_from_vertices(mesh.vertices)?;
        let index_start = slot.index_start;
        let index_count = slot.index_count;
        // Uploaded indices are absolute (`MeshRange::base_vertex` stays 0), so
        // the rewrite folds in the base vertex the mesh already sits at -- the
        // arena tail is irrelevant here.
        let base_vertex = slot.vertex_start;
        let indices: Vec<u32> = mesh.indices.iter().map(|idx| idx + base_vertex).collect();

        if skinned {
            let vertices: Vec<SkinnedMeshVertex> = mesh
                .vertices
                .iter()
                .map(pack_skinned_mesh_vertex)
                .collect();
            queue.write_buffer(
                &self.vertex_buffer,
                base_vertex as u64 * SKINNED_VERTEX_STRIDE as u64,
                bytemuck::cast_slice(&vertices),
            );
        } else {
            let vertices: Vec<RigidMeshVertex> =
                mesh.vertices.iter().map(pack_rigid_mesh_vertex).collect();
            queue.write_buffer(
                &self.rigid_vertex_buffer,
                base_vertex as u64 * RIGID_VERTEX_STRIDE as u64,
                bytemuck::cast_slice(&vertices),
            );
        }
        queue.write_buffer(
            &self.index_buffer,
            index_start as u64 * std::mem::size_of::<u32>() as u64,
            bytemuck::cast_slice(&indices),
        );

        if slot.blend_shape_target_count > 0 {
            let mut scratch = std::mem::take(&mut self.blend_shape_delta_scratch);
            fill_blend_shape_deltas(&mut scratch, &mesh.blend_shapes, mesh.vertices.len());
            queue.write_buffer(
                &self.blend_shape_delta_buffer,
                slot.blend_shape_delta_start as u64
                    * std::mem::size_of::<BlendShapeDeltaGpu>() as u64,
                bytemuck::cast_slice(&scratch),
            );
            scratch.clear();
            self.blend_shape_delta_scratch = scratch;
        }

        let surface_ranges_arc: Arc<[MeshRange]> = Arc::from(uploaded_surface_ranges(
            &mesh.surface_ranges,
            index_start,
            index_count,
        ));
        let meshlets_arc: Arc<[MeshletRange]> =
            Arc::from(uploaded_meshlets(mesh.meshlets, index_start));
        // At most one LOD survived the fit check above, so there is no packed
        // LOD to point at.
        let packed_lods = vec![None; mesh.lods.len()];
        let lods = build_mesh_lod_ranges(BuildMeshLodRangesArgs {
            index_start,
            index_count,
            decoded_surfaces: &mesh.surface_ranges,
            uploaded_surfaces: &surface_ranges_arc,
            decoded_meshlets: mesh.meshlets,
            uploaded_meshlets: &meshlets_arc,
            decoded_lods: mesh.lods,
            packed_lods: &packed_lods,
        });
        Some(MeshAssetRange {
            full: MeshRange {
                index_start,
                index_count,
                base_vertex: 0,
            },
            surface_ranges: surface_ranges_arc,
            meshlets: meshlets_arc,
            lods: Arc::from(lods),
            bounds_center,
            bounds_radius,
            blend_shape_delta_start: slot.blend_shape_delta_start,
            blend_shape_target_count: slot.blend_shape_target_count,
            blend_shape_vertex_start: slot.vertex_start,
            blend_shape_vertex_count: slot.vertex_count,
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

    /// Grow the packed-LOD param storage buffer. It lives inside the camera
    /// bind groups, so growth bumps `bind_generation` and every view rebuilds
    /// its bind groups at its next `sync_mesh_arena`.
    fn ensure_packed_lod_param_capacity(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        needed: usize,
    ) {
        if needed <= self.packed_lod_param_capacity {
            return;
        }
        let mut new_capacity = self.packed_lod_param_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.packed_lod_param_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_packed_lod_params"),
            size: (new_capacity * std::mem::size_of::<PackedLodParamGpu>()) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        if !self.packed_lod_params.is_empty() {
            queue.write_buffer(
                &self.packed_lod_param_buffer,
                0,
                bytemuck::cast_slice(&self.packed_lod_params),
            );
        }
        self.packed_lod_param_capacity = new_capacity;
        self.bump_bind_generation();
    }

    /// Grow the blend-shape delta storage buffer. Also bind-group facing.
    fn ensure_blend_shape_delta_capacity(
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
        self.bump_bind_generation();
    }

    /// Grow the three mesh buffers to hold the requested lengths. The two
    /// vertex arenas grow independently (each at its own stride), so a scene
    /// with no skinned draws never grows `vertex_buffer` past the builtin
    /// prefix.
    fn ensure_mesh_buffer_capacity(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        needed_skinned_vertices: usize,
        needed_rigid_vertices: usize,
        needed_indices: usize,
    ) {
        let mut skinned_grew = false;
        let mut rigid_grew = false;
        let mut index_grew = false;
        let mut grew = false;
        let max_buffer_size = device.limits().max_buffer_size as usize;
        let max_skinned_capacity = max_buffer_size / SKINNED_VERTEX_STRIDE;
        let max_rigid_capacity = max_buffer_size / RIGID_VERTEX_STRIDE;
        let max_index_capacity = max_buffer_size / std::mem::size_of::<u32>();

        if needed_skinned_vertices > self.vertex_capacity {
            let cap = bounded_growth_capacity(
                self.vertex_capacity,
                needed_skinned_vertices,
                max_skinned_capacity,
            )
            .unwrap_or_else(|| {
                panic!(
                    "skinned mesh vertex data needs {needed_skinned_vertices} vertices; device limit is {max_skinned_capacity}"
                )
            });
            self.vertex_capacity = cap;
            skinned_grew = true;
            grew = true;
        }

        if needed_rigid_vertices > self.rigid_vertex_capacity {
            let cap = bounded_growth_capacity(
                self.rigid_vertex_capacity,
                needed_rigid_vertices,
                max_rigid_capacity,
            )
            .unwrap_or_else(|| {
                panic!(
                    "rigid mesh vertex data needs {needed_rigid_vertices} vertices; device limit is {max_rigid_capacity}"
                )
            });
            self.rigid_vertex_capacity = cap;
            rigid_grew = true;
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
            index_grew = true;
            grew = true;
        }

        if grew {
            // Only the buffers that actually grew are reallocated: an index-only
            // growth must not drag the (now independently sized) vertex arenas
            // through a pointless realloc + copy.
            let old_vertex_buffer = self.vertex_buffer.clone();
            let old_rigid_vertex_buffer = self.rigid_vertex_buffer.clone();
            let old_index_buffer = self.index_buffer.clone();
            let old_vertex_size = self.mesh_vertex_len as u64 * SKINNED_VERTEX_STRIDE as u64;
            let old_rigid_vertex_size = self.rigid_vertex_len as u64 * RIGID_VERTEX_STRIDE as u64;
            let old_index_size = self.mesh_index_len as u64 * std::mem::size_of::<u32>() as u64;
            if skinned_grew {
                self.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("perro_mesh_vertices"),
                    size: (self.vertex_capacity * SKINNED_VERTEX_STRIDE) as u64,
                    usage: wgpu::BufferUsages::VERTEX
                        | wgpu::BufferUsages::COPY_DST
                        | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                });
            }
            if rigid_grew {
                self.rigid_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("perro_mesh_vertices_rigid"),
                    size: (self.rigid_vertex_capacity * RIGID_VERTEX_STRIDE) as u64,
                    usage: wgpu::BufferUsages::VERTEX
                        | wgpu::BufferUsages::COPY_DST
                        | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                });
            }
            if index_grew {
                self.index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("perro_mesh_indices"),
                    size: (self.index_capacity * std::mem::size_of::<u32>()) as u64,
                    usage: wgpu::BufferUsages::INDEX
                        | wgpu::BufferUsages::COPY_DST
                        | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                });
            }
            let copy_skinned = skinned_grew && old_vertex_size > 0;
            let copy_rigid = rigid_grew && old_rigid_vertex_size > 0;
            let copy_index = index_grew && old_index_size > 0;
            if copy_skinned || copy_rigid || copy_index {
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("perro_mesh_buffer_growth_copy"),
                });
                if copy_skinned {
                    encoder.copy_buffer_to_buffer(
                        &old_vertex_buffer,
                        0,
                        &self.vertex_buffer,
                        0,
                        old_vertex_size,
                    );
                }
                if copy_rigid {
                    encoder.copy_buffer_to_buffer(
                        &old_rigid_vertex_buffer,
                        0,
                        &self.rigid_vertex_buffer,
                        0,
                        old_rigid_vertex_size,
                    );
                }
                if copy_index {
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
            self.buffer_generation = self.buffer_generation.wrapping_add(1);
        }
    }

    fn ensure_packed_lod_buffer_capacity(
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
        self.buffer_generation = self.buffer_generation.wrapping_add(1);
    }

    /// A bind-group-facing buffer moved: both generations advance, since the
    /// handle changed too.
    fn bump_bind_generation(&mut self) {
        self.buffer_generation = self.buffer_generation.wrapping_add(1);
        self.bind_generation = self.bind_generation.wrapping_add(1);
    }

    /// Drop append-only custom mesh revisions before the shared vertex arena
    /// reaches the device's single-buffer limit. Built-in meshes always occupy
    /// the prefix; every live custom mesh is resolved again by the forced full
    /// prepare that follows this reset.
    ///
    /// Two triggers: the device-limit backstop below, and a request raised by
    /// `reclaim_tick` when most of the arena went dead. The GC tick cannot
    /// compact directly -- this must run at the top of a prepare so the
    /// `layout_generation` bump can force a full rebuild in the same frame,
    /// before anything draws through the invalidated ranges. Because the bump
    /// is compared per view, the *other* views' next prepare forces a rebuild
    /// too, which is exactly what sharing one arena requires.
    ///
    /// `allow_requested` is the ordering guard sharing needs. Compaction frees
    /// arena space for reuse, so a view holding retained batches that is NOT
    /// going to prepare this frame would draw through ranges the re-resolve
    /// then overwrites. Only the main 3D view -- which prepares before the
    /// camera-stream loop, and rebuilds itself in the same call via
    /// `layout_generation` -- may consume the GC request; the streams leave the
    /// flag standing for it. The device-limit backstop below stays open to every
    /// view: at ~3/4 of `max_buffer_size` the alternative is a hard panic in
    /// `bounded_growth_capacity`, and one stale view beats a crash.
    pub(in super::super) fn compact_if_needed(
        &mut self,
        device: &wgpu::Device,
        allow_requested: bool,
    ) -> bool {
        let requested = allow_requested && std::mem::take(&mut self.mesh_compact_requested);
        // Backstop per arena: each has its own stride, so each has its own
        // device-limit ceiling.
        let max_buffer_size = device.limits().max_buffer_size as usize;
        let max_skinned = max_buffer_size / SKINNED_VERTEX_STRIDE;
        let max_rigid = max_buffer_size / RIGID_VERTEX_STRIDE;
        let near_limit = self.mesh_vertex_len >= max_skinned.saturating_mul(3) / 4
            || self.rigid_vertex_len >= max_rigid.saturating_mul(3) / 4;
        if !requested && !near_limit {
            return false;
        }
        // Nothing appended past the builtin prefix in either arena: compacting
        // would only cost a pointless full rebuild.
        if self.mesh_vertex_len <= self.builtin_vertex_len
            && self.rigid_vertex_len <= self.builtin_vertex_len
            && self.custom_mesh_ranges.is_empty()
        {
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
        self.layout_generation = self.layout_generation.wrapping_add(1);
        true
    }

    /// Periodic GC tick: shrink the arena buffers once usage stays far below
    /// capacity (see `crate::gpu_shrink`). Every shrink preserves content with
    /// a GPU->GPU prefix copy, so retained draws stay valid; views pick the new
    /// handles up (and rebuild bind groups when required) at their next
    /// `sync_mesh_arena`.
    pub(crate) fn shrink_tick(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        use crate::gpu_shrink::shrink_buffer_preserving;
        // Mesh arena lengths are authoritative retained content. The rigid and
        // skinned arenas grow independently, so they shrink independently too:
        // a scene that stopped drawing skinned meshes gives the 48B arena back
        // without dragging the rigid one down with it.
        self.shrink.rigid_vertices.note_used(self.rigid_vertex_len);
        self.shrink.skinned_vertices.note_used(self.mesh_vertex_len);
        self.shrink.indices.note_used(self.mesh_index_len);
        self.shrink
            .packed_lod_vertices
            .note_used(self.packed_lod_vertex_len);
        self.shrink
            .packed_lod_indices
            .note_used(self.packed_lod_index_len);
        self.shrink
            .packed_lod_params
            .note_used(self.packed_lod_params.len());
        self.shrink
            .blend_shape_deltas
            .note_used(self.blend_shape_delta_len);

        let vertex_usage = wgpu::BufferUsages::VERTEX
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC;
        let index_usage = wgpu::BufferUsages::INDEX
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC;
        let storage_usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC;

        let mut moved = false;
        if let Some(new_cap) = self
            .shrink
            .skinned_vertices
            .tick(self.vertex_capacity, 1024)
        {
            self.vertex_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.vertex_buffer,
                "perro_mesh_vertices",
                (new_cap * SKINNED_VERTEX_STRIDE) as u64,
                vertex_usage,
            );
            self.vertex_capacity = new_cap;
            moved = true;
        }
        if let Some(new_cap) = self
            .shrink
            .rigid_vertices
            .tick(self.rigid_vertex_capacity, 1024)
        {
            self.rigid_vertex_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.rigid_vertex_buffer,
                "perro_mesh_vertices_rigid",
                (new_cap * RIGID_VERTEX_STRIDE) as u64,
                vertex_usage,
            );
            self.rigid_vertex_capacity = new_cap;
            moved = true;
        }
        if let Some(new_cap) = self.shrink.indices.tick(self.index_capacity, 1024) {
            self.index_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.index_buffer,
                "perro_mesh_indices",
                (new_cap * std::mem::size_of::<u32>()) as u64,
                index_usage,
            );
            self.index_capacity = new_cap;
            moved = true;
        }
        if let Some(new_cap) = self
            .shrink
            .packed_lod_vertices
            .tick(self.packed_lod_vertex_capacity, 256)
        {
            self.packed_lod_vertex_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.packed_lod_vertex_buffer,
                "perro_packed_lod_vertices_rigid",
                (new_cap * std::mem::size_of::<PackedRigidLodVertex>()) as u64,
                vertex_usage,
            );
            self.packed_lod_vertex_capacity = new_cap;
            moved = true;
        }
        if let Some(new_cap) = self
            .shrink
            .packed_lod_indices
            .tick(self.packed_lod_index_capacity, 256)
        {
            self.packed_lod_index_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.packed_lod_index_buffer,
                "perro_packed_lod_indices",
                (new_cap * std::mem::size_of::<u32>()) as u64,
                index_usage,
            );
            self.packed_lod_index_capacity = new_cap;
            moved = true;
        }
        if moved {
            self.buffer_generation = self.buffer_generation.wrapping_add(1);
        }

        // Bind-group-facing pair: one bump covers both.
        let mut rebind = false;
        if let Some(new_cap) = self
            .shrink
            .blend_shape_deltas
            .tick(self.blend_shape_delta_capacity, 64)
        {
            self.blend_shape_delta_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.blend_shape_delta_buffer,
                "perro_blend_shape_deltas",
                (new_cap * std::mem::size_of::<BlendShapeDeltaGpu>()) as u64,
                storage_usage,
            );
            self.blend_shape_delta_capacity = new_cap;
            rebind = true;
        }
        if let Some(new_cap) = self
            .shrink
            .packed_lod_params
            .tick(self.packed_lod_param_capacity, 64)
        {
            self.packed_lod_param_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.packed_lod_param_buffer,
                "perro_packed_lod_params",
                (new_cap * std::mem::size_of::<PackedLodParamGpu>()) as u64,
                storage_usage,
            );
            self.packed_lod_param_capacity = new_cap;
            rebind = true;
        }
        if rebind {
            self.bump_bind_generation();
        }
    }

    #[cfg(test)]
    pub(in super::super) fn request_compact(&mut self) {
        self.mesh_compact_requested = true;
    }

    #[cfg(test)]
    pub(in super::super) fn arena_buffer_ids(&self) -> [&wgpu::Buffer; 5] {
        [
            &self.vertex_buffer,
            &self.rigid_vertex_buffer,
            &self.index_buffer,
            &self.packed_lod_vertex_buffer,
            &self.packed_lod_index_buffer,
        ]
    }
}

#[cfg(test)]
mod in_place_tests {
    use super::*;
    use perro_render_bridge::{Mesh3D, RuntimeMeshVertex};
    use perro_structs::UnitVector4;

    async fn request_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .ok()?;
        adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("perro_mesh_arena_in_place_test_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::default(),
            })
            .await
            .ok()
    }

    /// Triangle fan along +x, `offset` shifting the whole mesh so a rewrite is
    /// observable in the returned bounds.
    fn runtime_mesh(vertices: usize, offset: f32) -> Mesh3D {
        Mesh3D {
            vertices: (0..vertices)
                .map(|i| RuntimeMeshVertex {
                    position: [offset + i as f32, 0.0, 0.0],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0, 0.0],
                    paint_uv: [0.0, 0.0],
                    joints: [0; 4],
                    weights: UnitVector4::new([1.0, 0.0, 0.0, 0.0]),
                })
                .collect(),
            indices: (0..vertices as u32).collect(),
            surface_ranges: Vec::new(),
            blend_shapes: Vec::new(),
        }
    }

    #[test]
    fn same_count_rewrite_keeps_the_range_and_appends_nothing() {
        let Some((device, queue)) = pollster::block_on(request_device()) else {
            eprintln!("no wgpu adapter; skipping mesh arena in-place test");
            return;
        };
        for skinned in [false, true] {
            let mut arena = SharedMeshArena::new(&device, false, false);
            let first = runtime_mesh(96, 0.0);
            let range = arena
                .append_borrowed_mesh_data(
                    &device,
                    &queue,
                    &borrow_runtime_mesh(&first).expect("borrow"),
                    skinned,
                )
                .expect("append");
            let before = arena.memory_report();

            let second = runtime_mesh(96, 100.0);
            let updated = arena
                .update_mesh_data_in_place(
                    &queue,
                    &borrow_runtime_mesh(&second).expect("borrow"),
                    in_place_slot(&range),
                    skinned,
                )
                .expect("in-place update");

            assert_eq!(updated.full.index_start, range.full.index_start);
            assert_eq!(updated.full.index_count, range.full.index_count);
            assert_eq!(
                updated.blend_shape_vertex_start,
                range.blend_shape_vertex_start
            );
            // Content changed, arena did not grow and nothing was stranded.
            assert_ne!(updated.bounds_center[0], range.bounds_center[0]);
            let after = arena.memory_report();
            assert_eq!(after.rigid_arena_vertices, before.rigid_arena_vertices);
            assert_eq!(after.skinned_arena_vertices, before.skinned_arena_vertices);
            assert_eq!(after.mesh_index_len, before.mesh_index_len);
            assert_eq!(arena.layout_generation(), 1);
        }
    }

    #[test]
    fn different_count_rewrite_falls_back_to_append() {
        let Some((device, queue)) = pollster::block_on(request_device()) else {
            eprintln!("no wgpu adapter; skipping mesh arena in-place fallback test");
            return;
        };
        let mut arena = SharedMeshArena::new(&device, false, false);
        let first = runtime_mesh(96, 0.0);
        let range = arena
            .append_borrowed_mesh_data(
                &device,
                &queue,
                &borrow_runtime_mesh(&first).expect("borrow"),
                false,
            )
            .expect("append");
        let before = arena.memory_report();

        let grown = runtime_mesh(192, 0.0);
        let borrowed = borrow_runtime_mesh(&grown).expect("borrow");
        assert!(
            arena
                .update_mesh_data_in_place(&queue, &borrowed, in_place_slot(&range), false)
                .is_none()
        );
        let appended = arena
            .append_borrowed_mesh_data(&device, &queue, &borrowed, false)
            .expect("append");
        assert_ne!(appended.full.index_start, range.full.index_start);
        assert_eq!(
            arena.memory_report().rigid_arena_vertices,
            before.rigid_arena_vertices + 192
        );
    }
}
