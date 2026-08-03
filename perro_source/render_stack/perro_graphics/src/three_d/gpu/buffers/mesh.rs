//! Per-view mirror of the shared mesh arena, plus the per-view GPU memory
//! report.
//!
//! The arenas themselves live in `mesh_arena::SharedMeshArena`, owned by `Gpu`
//! and shared by every `Gpu3D`. What stays here is the thin view side: cloned
//! `wgpu::Buffer` handles (so all the draw and bind-group code keeps reading
//! `self.vertex_buffer` unchanged) and the generation compare that refreshes
//! them.

use super::*;

/// Snapshot of one view's reclaimable GPU memory. Same shape as the
/// `PostPerfCounters` pattern: cheap to build, never on a hot path, read by
/// tests and memory diagnostics.
///
/// Mesh-arena numbers are NOT here: the arena is process-wide now, so it
/// reports once through `SharedMeshArena::memory_report` instead of being
/// double-counted per view.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(dead_code, reason = "diagnostics + test surface, no production reader")]
pub struct Gpu3DMemoryReport {
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
    #[allow(dead_code, reason = "diagnostics + test surface, no production reader")]
    pub(in super::super) fn memory_report(&self) -> Gpu3DMemoryReport {
        Gpu3DMemoryReport {
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
                .saturating_mul(POINT_SHADOW_FACE_COUNT)
                as u32,
            decal_layers_allocated: self.decal_texture_layers,
            decal_layers_live: self.decal_layer_by_texture.len() as u32,
        }
    }

    /// Periodic GC tick for this view's grow-only GPU memory `shrink_tick`
    /// cannot reclaim: the shadow atlases and the decal texture array. Both own
    /// their contents' lifetime (re-render / re-upload on demand), so they can
    /// shrink in place right here. The mesh arena is shared and ticks once, in
    /// `SharedMeshArena::reclaim_tick`.
    pub fn reclaim_memory_tick(&mut self, device: &wgpu::Device) {
        self.shrink_shadow_atlases_tick(device);
        self.shrink_decal_texture_tick(device);
    }

    /// Adopt the shared arena's current buffer handles and act on the two
    /// invalidations sharing introduces.
    ///
    /// Returns true when the arena was compacted (by any view, or by this one's
    /// `compact_if_needed` moments ago) since this view last synced, in which
    /// case every retained draw batch of this view still points at ranges that
    /// no longer exist and prepare must do a full rebuild.
    ///
    /// Call it twice per prepare: once at the top (pick up other views' growth
    /// and compaction) and once after the mesh-resolve loop (pick up growth
    /// this view just caused, before `render_pass` binds anything).
    ///
    /// A view that skips prepare and still renders keeps last frame's handles,
    /// which stays correct: growth and shrink both preserve the old buffer's
    /// contents (the arena copies the prefix into the new allocation and never
    /// rewrites the old one), and every range this view retained was resolved
    /// below the old length. It just pins one superseded allocation until its
    /// next prepare. Compaction is the one exception, and
    /// `SharedMeshArena::compact_if_needed` is ordered so it cannot happen
    /// behind a view that is going to render without preparing.
    pub(in super::super) fn sync_mesh_arena(
        &mut self,
        device: &wgpu::Device,
        arena: &SharedMeshArena,
    ) -> bool {
        let compacted = arena.layout_generation() != self.mesh_arena_layout_generation;
        if compacted {
            self.mesh_arena_layout_generation = arena.layout_generation();
        }
        if arena.buffer_generation() != self.mesh_arena_buffer_generation {
            let rebind = arena.bind_generation() != self.mesh_arena_bind_generation;
            self.adopt_mesh_arena_handles(arena);
            if rebind {
                // blend_shape_delta_buffer / packed_lod_param_buffer are bind
                // group entries; the vertex/index arenas are not, so a plain
                // arena growth skips this ~101 bind group rebuild.
                self.rebuild_camera_bind_groups(device);
            }
        }
        compacted
    }

    pub(in super::super) fn adopt_mesh_arena_handles(&mut self, arena: &SharedMeshArena) {
        let handles = arena.handles();
        self.vertex_buffer = handles.vertex_buffer;
        self.rigid_vertex_buffer = handles.rigid_vertex_buffer;
        self.index_buffer = handles.index_buffer;
        self.packed_lod_vertex_buffer = handles.packed_lod_vertex_buffer;
        self.packed_lod_index_buffer = handles.packed_lod_index_buffer;
        self.packed_lod_param_buffer = handles.packed_lod_param_buffer;
        self.blend_shape_delta_buffer = handles.blend_shape_delta_buffer;
        self.mesh_arena_buffer_generation = handles.buffer_generation;
        self.mesh_arena_bind_generation = handles.bind_generation;
        self.mesh_arena_layout_generation = handles.layout_generation;
    }

    /// The five vertex/index arena handles this view currently draws through,
    /// for tests that assert two views really do bind the same allocations.
    #[cfg(test)]
    pub(in super::super) fn mesh_arena_buffer_ids(&self) -> [&wgpu::Buffer; 5] {
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
#[path = "../../../../tests/unit/three_d_memory_reclaim_tests.rs"]
mod memory_reclaim_tests;
