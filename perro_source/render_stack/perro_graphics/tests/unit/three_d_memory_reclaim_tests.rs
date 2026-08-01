//! GPU memory reclamation: shared-mesh-arena compaction (once for every view),
//! plus the per-view shadow-atlas and decal texture-array shrinks driven by the
//! periodic GC tick (`Gpu3D::reclaim_memory_tick`).
//!
//! Runs against a headless wgpu device; skipped with a note when no adapter is
//! available.
use super::*;
use crate::gpu_shrink::SHRINK_LOW_TICKS;
use crate::three_d::gpu::buffers::mesh_arena::MESH_ARENA_COMPACT_MAX_DEFER_TICKS;
use perro_ids::{MeshID, TextureID};
use perro_structs::UnitVector4;

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
// Three of these clear MESH_ARENA_COMPACT_MIN_BYTES (16MiB at the rigid arena's
// 36B stride) with room to spare, so dropping two takes the live share well
// under half.
const TEST_MESH_VERTICES: usize = 160_000;

async fn test_device() -> Option<(wgpu::Device, wgpu::Queue)> {
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
            label: Some("perro_memory_reclaim_test_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
        })
        .await
        .ok()
}

fn new_arena(device: &wgpu::Device) -> SharedMeshArena {
    SharedMeshArena::new(device, false, false)
}

fn new_gpu_3d(device: &wgpu::Device, queue: &wgpu::Queue, arena: &SharedMeshArena) -> Gpu3D {
    let cache = PipelineRegistryCache::new();
    let pipelines = cache.get_or_create(device, COLOR_FORMAT, 1);
    Gpu3D::new(
        device,
        queue,
        COLOR_FORMAT,
        Gpu3DConfig {
            sample_count: 1,
            width: 64,
            height: 64,
            meshlets_enabled: false,
            dev_meshlets: false,
            meshlet_debug_view: false,
            occlusion_culling: crate::OcclusionCullingMode::Off,
            ssao: crate::SsaoQuality::Off,
            indirect_first_instance_enabled: false,
            multi_draw_indirect_enabled: false,
            multi_draw_indirect_count_enabled: false,
            texture_filter: TextureFilterMode::default(),
            shader_variant_mode: crate::ShaderVariantMode::Generic,
            shadow_pcf_high: false,
        },
        pipelines,
        arena,
    )
}

fn decoded_mesh(vertices: usize) -> DecodedMesh {
    let verts: Vec<DecodedMeshVertex> = (0..vertices)
        .map(|i| DecodedMeshVertex {
            pos: [i as f32 * 0.001, 0.0, 0.0],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
            paint_uv: [0.0, 0.0],
            joints: [0; 4],
            weights: UnitVector4::new([1.0, 0.0, 0.0, 0.0]),
        })
        .collect();
    DecodedMesh {
        indices: (0..vertices as u32).collect(),
        vertices: verts,
        surface_ranges: Vec::new(),
        blend_shapes: Vec::new(),
        meshlets: Vec::new(),
        lods: Vec::new(),
        has_skinning: false,
    }
}

fn rigid_entry(range: MeshAssetRange) -> MeshAssetEntry {
    MeshAssetEntry {
        revision: 0,
        rigid: Some(range),
        skinned: None,
    }
}

fn shadow_point_light(x: f32) -> PointLight3DState {
    PointLight3DState {
        position: [x, 2.0, 0.0],
        color: [1.0, 1.0, 1.0],
        intensity: 4.0,
        range: 12.0,
        cast_shadows: true,
        shadow_strength: 0.8,
        shadow_depth_bias: 0.0,
        shadow_normal_bias: 0.0,
    }
}

#[test]
fn dead_mesh_ranges_trigger_arena_compaction_from_the_gc_tick() {
    let Some((device, queue)) = pollster::block_on(test_device()) else {
        eprintln!("no wgpu adapter; skipping mesh arena compaction test");
        return;
    };
    let mut arena = new_arena(&device);
    let builtin_vertices = arena.memory_report().rigid_arena_vertices;

    // Three custom meshes resolved into the rigid arena (what prepare's
    // resolve_mesh_range does on a scene load with no skeletons in sight).
    for index in 0..3u32 {
        let range = arena
            .append_mesh_data(
                &device,
                &queue,
                "test://reclaim_mesh",
                decoded_mesh(TEST_MESH_VERTICES),
                false,
            )
            .expect("append_mesh_data");
        arena
            .custom_mesh_ranges
            .insert(MeshID::from_parts(index + 1, 1), rigid_entry(range));
    }
    let grown = arena.memory_report();
    assert_eq!(
        grown.rigid_arena_vertices,
        builtin_vertices + 3 * TEST_MESH_VERTICES
    );
    // Rigid-only content never touches the skinned arena.
    assert_eq!(grown.skinned_arena_vertices, builtin_vertices);
    assert_eq!(grown.mesh_arena_live_vertices, grown.mesh_arena_vertices);
    assert!(!grown.mesh_compact_requested);
    // A full arena never asks for compaction.
    arena.reclaim_tick();
    assert!(!arena.memory_report().mesh_compact_requested);

    // Scene switch: the resources are gone, so prepare's
    // `custom_mesh_ranges.retain(has_mesh)` drops their ranges. The bytes stay
    // stranded in the append-only arena.
    arena
        .custom_mesh_ranges
        .retain(|mesh_id, _| *mesh_id == MeshID::from_parts(1, 1));
    let stranded = arena.memory_report();
    assert_eq!(
        stranded.mesh_arena_live_vertices,
        2 * builtin_vertices + TEST_MESH_VERTICES
    );
    assert!(stranded.mesh_arena_bytes > stranded.mesh_arena_live_bytes * 2);

    arena.reclaim_tick();
    let requested = arena.memory_report();
    assert!(
        requested.mesh_compact_requested,
        "GC tick must request compaction once under half the arena is live"
    );
    // Still deferred: the arena is untouched until a prepare can consume it.
    assert_eq!(requested.mesh_arena_vertices, grown.mesh_arena_vertices);

    // What prepare does at the top of the frame; `true` becomes
    // `force_full_rebuild`, which re-resolves every live mesh.
    assert!(arena.compact_if_needed(&device, true));
    let compacted = arena.memory_report();
    assert_eq!(compacted.rigid_arena_vertices, builtin_vertices);
    assert_eq!(compacted.skinned_arena_vertices, builtin_vertices);
    assert_eq!(compacted.mesh_arena_live_vertices, 2 * builtin_vertices);
    assert!(!compacted.mesh_compact_requested);
    assert!(arena.custom_mesh_ranges.is_empty());
    eprintln!("mesh arena vertices: {} -> {} (live {} -> {}), bytes {} -> {}",
        stranded.mesh_arena_vertices,
        compacted.mesh_arena_vertices,
        stranded.mesh_arena_live_vertices,
        compacted.mesh_arena_live_vertices,
        stranded.mesh_arena_bytes,
        compacted.mesh_arena_bytes,
    );

    // A second request with nothing appended past the builtin prefix is a
    // no-op: no pointless full rebuild.
    arena.request_compact();
    assert!(!arena.compact_if_needed(&device, true));
}

/// Sizes the *other* half of a scene-transition frame against the pipeline-warm
/// half: what one mesh costs to repack and upload into the arena, and what a
/// compaction's forced re-append of a whole scene costs. Diagnostic only - it
/// reports rather than asserting a wall time - but it is what says whether
/// budgeting mesh uploads is worth doing next to budgeting pipeline compiles.
#[test]
fn mesh_arena_append_cost_report() {
    let Some((device, queue)) = pollster::block_on(test_device()) else {
        eprintln!("no wgpu adapter; skipping mesh arena append cost report");
        return;
    };
    let mut arena = new_arena(&device);
    const MESHES: usize = 24;
    let before = arena.memory_report().rigid_arena_bytes;

    let start = std::time::Instant::now();
    for _ in 0..MESHES {
        arena
            .append_mesh_data(
                &device,
                &queue,
                "test://append_cost",
                decoded_mesh(TEST_MESH_VERTICES),
                false,
            )
            .expect("append_mesh_data");
    }
    let elapsed = start.elapsed();
    let appended = arena.memory_report().rigid_arena_bytes - before;
    eprintln!(
        "[mesh-arena] {MESHES} appends totalling {:.1}MiB: {:.2}ms total, {:.2}ms per mesh",
        appended as f64 / (1024.0 * 1024.0),
        elapsed.as_secs_f64() * 1e3,
        elapsed.as_secs_f64() * 1e3 / MESHES as f64,
    );
}

/// A scene transition drops the outgoing scene's mesh ranges while the incoming
/// scene's meshes are still streaming in, so the arena reads as mostly-dead
/// while it is really mid-refill. Compacting there is the worst possible time:
/// it throws away every range the new scene resolved so far and forces a full
/// CPU decode + repack + re-upload of all of them in one frame, only for the
/// rest of the load to append on top. The GC tick therefore waits for a tick
/// with no appends, and gives up waiting after
/// `MESH_ARENA_COMPACT_MAX_DEFER_TICKS`.
#[test]
fn arena_compaction_waits_for_a_tick_with_no_mesh_appends() {
    let Some((device, queue)) = pollster::block_on(test_device()) else {
        eprintln!("no wgpu adapter; skipping mesh arena compaction churn test");
        return;
    };
    let mut arena = new_arena(&device);
    // `vertices` is what makes a mesh count as outgoing bulk or as an
    // incoming trickle: the incoming ones stay small so the live share stays
    // under half throughout the load, which is exactly the reading that used to
    // fire a compaction mid-transition.
    let append = |arena: &mut SharedMeshArena, id: u32, vertices: usize| {
        let range = arena
            .append_mesh_data(
                &device,
                &queue,
                "test://churn_mesh",
                decoded_mesh(vertices),
                false,
            )
            .expect("append_mesh_data");
        arena
            .custom_mesh_ranges
            .insert(MeshID::from_parts(id, 1), rigid_entry(range));
    };

    // Outgoing scene.
    for id in 1..=3u32 {
        append(&mut arena, id, TEST_MESH_VERTICES);
    }
    arena.reclaim_tick();
    assert!(!arena.memory_report().mesh_compact_requested);

    // Transition: old ranges dropped, new scene starts streaming in. Every GC
    // tick during the load sees "less than half live" and must still hold off.
    arena.custom_mesh_ranges.clear();
    for tick in 0..MESH_ARENA_COMPACT_MAX_DEFER_TICKS {
        append(&mut arena, 100 + tick, 1_000);
        let report = arena.memory_report();
        assert!(
            report.mesh_arena_live_bytes * 2 < report.mesh_arena_bytes,
            "the mid-load arena must actually look mostly-dead (tick {tick})"
        );
        arena.reclaim_tick();
        assert!(
            !arena.memory_report().mesh_compact_requested,
            "compaction must not be requested mid-load (tick {tick})"
        );
    }

    // Load finishes: a quiet tick, and the reclaim goes through.
    arena.reclaim_tick();
    assert!(
        arena.memory_report().mesh_compact_requested,
        "a settled arena under half live must still compact"
    );
    assert!(arena.compact_if_needed(&device, true));

    // Content that streams meshes forever must not defer the reclaim forever:
    // after the cap, a churning arena compacts anyway.
    for id in 200..203u32 {
        append(&mut arena, id, TEST_MESH_VERTICES);
    }
    arena.custom_mesh_ranges.clear();
    let mut requested_after = None;
    for tick in 0..=MESH_ARENA_COMPACT_MAX_DEFER_TICKS {
        append(&mut arena, 300 + tick, 1_000);
        arena.reclaim_tick();
        if arena.memory_report().mesh_compact_requested {
            requested_after = Some(tick);
            break;
        }
    }
    assert_eq!(
        requested_after,
        Some(MESH_ARENA_COMPACT_MAX_DEFER_TICKS),
        "a permanently churning arena must compact after the defer cap"
    );
}

/// The split-arena win: rigid-only meshes stay out of the 48B/vertex skinned
/// arena entirely, and a mesh drawn on both paths lands in both arenas with its
/// own index block (the ranges of one variant are never valid against the
/// other's arena, because the uploaded indices are absolute).
#[test]
fn rigid_only_meshes_stay_out_of_the_skinned_arena() {
    let Some((device, queue)) = pollster::block_on(test_device()) else {
        eprintln!("no wgpu adapter; skipping mesh arena split test");
        return;
    };
    let mut arena = new_arena(&device);
    let base = arena.memory_report();
    let builtin_vertices = base.rigid_arena_vertices;
    assert_eq!(base.skinned_arena_vertices, builtin_vertices);

    const RIGID_MESHES: usize = 10;
    const RIGID_VERTICES: usize = 100_000;
    const SKINNED_MESHES: usize = 2;
    const SKINNED_VERTICES: usize = 50_000;

    for index in 0..RIGID_MESHES {
        let range = arena
            .append_mesh_data(
                &device,
                &queue,
                "test://rigid_mesh",
                decoded_mesh(RIGID_VERTICES),
                false,
            )
            .expect("append rigid mesh");
        arena
            .custom_mesh_ranges
            .insert(MeshID::from_parts(index as u32 + 1, 1), rigid_entry(range));
    }
    for index in 0..SKINNED_MESHES {
        let range = arena
            .append_mesh_data(
                &device,
                &queue,
                "test://skinned_mesh",
                decoded_mesh(SKINNED_VERTICES),
                true,
            )
            .expect("append skinned mesh");
        arena.custom_mesh_ranges.insert(
            MeshID::from_parts((RIGID_MESHES + index) as u32 + 1, 1),
            MeshAssetEntry {
                revision: 0,
                rigid: None,
                skinned: Some(range),
            },
        );
    }

    let report = arena.memory_report();
    let rigid_custom = RIGID_MESHES * RIGID_VERTICES;
    let skinned_custom = SKINNED_MESHES * SKINNED_VERTICES;
    assert_eq!(
        report.rigid_arena_vertices,
        builtin_vertices + rigid_custom,
        "every rigid-path mesh lands in the rigid arena"
    );
    assert_eq!(
        report.skinned_arena_vertices,
        builtin_vertices + skinned_custom,
        "the skinned arena holds only the meshes a skinned draw asked for"
    );
    // Everything appended is still reachable.
    assert_eq!(report.mesh_arena_live_vertices, report.mesh_arena_vertices);

    // What the old single-cursor layout cost: both copies of every vertex.
    let before = (builtin_vertices + rigid_custom + skinned_custom) * (48 + 36);
    let after = report.mesh_arena_bytes;
    assert!(
        after * 2 < before,
        "split arenas must more than halve the vertex bytes: {after} vs {before}"
    );
    eprintln!(
        "mesh arena bytes: {before} (both copies of every vertex) -> {after} \
         (rigid {} @36B + skinned {} @48B)",
        report.rigid_arena_vertices, report.skinned_arena_vertices,
    );

    // A mesh drawn on both paths occupies both arenas, each with its own index
    // block -- the case the split has to keep correct, not just small.
    let both_id = MeshID::from_parts(9_000, 1);
    let index_len_before = arena.memory_report().mesh_index_len;
    let rigid = arena
        .append_mesh_data(&device, &queue, "test://both", decoded_mesh(1_024), false)
        .expect("append rigid variant");
    let skinned = arena
        .append_mesh_data(&device, &queue, "test://both", decoded_mesh(1_024), true)
        .expect("append skinned variant");
    assert_ne!(
        rigid.full.index_start, skinned.full.index_start,
        "each variant owns a distinct index block"
    );
    assert_eq!(rigid.full.base_vertex, 0);
    assert_eq!(skinned.full.base_vertex, 0);
    assert_eq!(
        arena.memory_report().mesh_index_len,
        index_len_before + 2 * 1_024,
        "the skinned variant duplicates the index block, not the vertex data"
    );
    arena.custom_mesh_ranges.insert(
        both_id,
        MeshAssetEntry {
            revision: 0,
            rigid: Some(rigid),
            skinned: Some(skinned),
        },
    );
    let both = arena.memory_report();
    assert_eq!(both.rigid_arena_vertices, builtin_vertices + rigid_custom + 1_024);
    assert_eq!(
        both.skinned_arena_vertices,
        builtin_vertices + skinned_custom + 1_024
    );
    assert_eq!(both.mesh_arena_live_vertices, both.mesh_arena_vertices);
}

/// Mem audit #4: the main view and two camera streams draw the same custom mesh
/// through ONE arena set. Before this, each `Gpu3D` owned private arenas, so the
/// same 100k-vertex mesh was decoded and uploaded once per view.
#[test]
fn one_mesh_arena_serves_the_main_view_and_two_camera_streams() {
    let Some((device, queue)) = pollster::block_on(test_device()) else {
        eprintln!("no wgpu adapter; skipping shared mesh arena test");
        return;
    };
    const VERTICES: usize = 100_000;
    const RIGID_STRIDE: usize = 36;

    let mut arena = new_arena(&device);
    let base = arena.memory_report();
    // Main view + two camera-stream views, all mirroring the one arena.
    let mut views = [
        new_gpu_3d(&device, &queue, &arena),
        new_gpu_3d(&device, &queue, &arena),
        new_gpu_3d(&device, &queue, &arena),
    ];
    // The builtin prefix is uploaded once at arena init, not once per view.
    assert_eq!(arena.memory_report(), base);

    // The main view's prepare resolves the mesh first: one decode, one upload.
    let mesh_id = MeshID::from_parts(1, 1);
    let range = arena
        .append_mesh_data(
            &device,
            &queue,
            "test://shared_mesh",
            decoded_mesh(VERTICES),
            false,
        )
        .expect("append shared mesh");
    arena.custom_mesh_ranges.insert(mesh_id, rigid_entry(range));

    // Both streams then resolve the same mesh id. `resolve_mesh_range` reads the
    // shared `custom_mesh_ranges`, so its hit path returns without appending.
    for view in 0..2 {
        assert!(
            arena
                .custom_mesh_ranges
                .get(&mesh_id)
                .and_then(|entry| entry.variant(false))
                .is_some(),
            "stream view {view} must hit the shared resolved range"
        );
    }

    let shared = arena.memory_report();
    let shared_bytes = shared.rigid_arena_bytes - base.rigid_arena_bytes;
    let private_bytes = 3 * VERTICES * RIGID_STRIDE;
    assert_eq!(
        shared.rigid_arena_vertices,
        base.rigid_arena_vertices + VERTICES,
        "the mesh is appended once, not once per view"
    );
    assert_eq!(shared_bytes, VERTICES * RIGID_STRIDE);
    eprintln!(
        "3 views x {VERTICES}-vertex mesh: {private_bytes} B private arenas -> \
         {shared_bytes} B shared ({}x)",
        private_bytes / shared_bytes.max(1)
    );

    // Growth in one view is visible to the others with no re-upload: after the
    // sync every view binds the arena's current allocations.
    for view in views.iter_mut() {
        assert!(
            !view.sync_mesh_arena(&device, &arena),
            "a plain growth is not a layout change"
        );
        for (mine, theirs) in view
            .mesh_arena_buffer_ids()
            .into_iter()
            .zip(arena.arena_buffer_ids())
        {
            assert!(mine == theirs, "view must bind the arena's current buffer");
        }
    }

    // Compaction invalidates every resolved range, so it must force a full
    // rebuild in ALL views, not just the one that ran it.
    arena.request_compact();
    assert!(arena.compact_if_needed(&device, true));
    for (index, view) in views.iter_mut().enumerate() {
        assert!(
            view.sync_mesh_arena(&device, &arena),
            "compaction must force a full rebuild in view {index}"
        );
    }
    // ...and exactly once: a second sync at the same generation is a no-op.
    for view in views.iter_mut() {
        assert!(!view.sync_mesh_arena(&device, &arena));
    }
    let compacted = arena.memory_report();
    assert_eq!(compacted.rigid_arena_vertices, base.rigid_arena_vertices);
    assert!(arena.custom_mesh_ranges.is_empty());
}

#[test]
fn point_shadow_layers_return_to_baseline_after_lights_leave() {
    let Some((device, queue)) = pollster::block_on(test_device()) else {
        eprintln!("no wgpu adapter; skipping shadow atlas shrink test");
        return;
    };
    let arena = new_arena(&device);
    let mut gpu = new_gpu_3d(&device, &queue, &arena);
    let camera = Camera3DState::default();
    assert_eq!(gpu.memory_report().point_shadow_layers_allocated, 0);

    let mut lighting = Lighting3DState::default();
    for (index, slot) in lighting.point_lights.iter_mut().enumerate().take(4) {
        *slot = Some(shadow_point_light(index as f32 * 3.0));
    }
    gpu.update_shadow_state(&device, &queue, &camera, &lighting, true);
    let grown = gpu.memory_report();
    assert_eq!(grown.point_shadow_layers_allocated, 24);
    assert_eq!(grown.point_shadow_layers_used, 24);

    // Lights removed; the atlas is still allocated at its high-water mark.
    gpu.update_shadow_state(&device, &queue, &camera, &Lighting3DState::default(), true);
    let idle = gpu.memory_report();
    assert_eq!(idle.point_shadow_layers_allocated, 24);
    assert_eq!(idle.point_shadow_layers_used, 0);

    // One tick still carries the pre-drop peak; the streak starts after it.
    for _ in 0..=SHRINK_LOW_TICKS {
        gpu.reclaim_memory_tick(&device);
    }
    let shrunk = gpu.memory_report();
    assert_eq!(shrunk.point_shadow_layers_allocated, 0);
    assert!(
        gpu.shadow_casters_dirty,
        "a shrunk atlas holds garbage depth; every layer must re-render"
    );
    assert!(gpu.point_shadow_layer_views.is_empty());
    eprintln!(
        "point shadow layers allocated: {} -> {} (used {})",
        idle.point_shadow_layers_allocated,
        shrunk.point_shadow_layers_allocated,
        shrunk.point_shadow_layers_used
    );

    // Lights come back: the grow path re-allocates from the placeholder.
    gpu.update_shadow_state(&device, &queue, &camera, &lighting, true);
    assert_eq!(gpu.memory_report().point_shadow_layers_allocated, 24);
}

#[test]
fn decal_texture_array_shrinks_to_the_live_layer_count() {
    let Some((device, queue)) = pollster::block_on(test_device()) else {
        eprintln!("no wgpu adapter; skipping decal array shrink test");
        return;
    };
    let arena = new_arena(&device);
    let mut gpu = new_gpu_3d(&device, &queue, &arena);

    // Stand in for a decal-heavy scene that grew the array to 16 layers.
    let (texture, view) = create_decal_texture_array(&device, 16);
    gpu.decal_texture = texture;
    gpu.decal_texture_view = view;
    gpu.decal_texture_layers = 16;
    gpu.decal_layer_by_texture
        .insert(TextureID::from_parts(1, 1), 0);
    gpu.decal_layer_by_texture
        .insert(TextureID::from_parts(2, 1), 1);
    gpu.decal_sources_pending = false;
    let grown = gpu.memory_report();
    assert_eq!(grown.decal_layers_allocated, 16);
    assert_eq!(grown.decal_layers_live, 2);

    for _ in 0..SHRINK_LOW_TICKS - 1 {
        gpu.reclaim_memory_tick(&device);
        assert_eq!(gpu.memory_report().decal_layers_allocated, 16);
    }
    gpu.reclaim_memory_tick(&device);
    let shrunk = gpu.memory_report();
    assert_eq!(shrunk.decal_layers_allocated, 2);
    // Contents are dropped, so live decals must re-resolve (and re-upload)
    // their layers on the next prepare.
    assert_eq!(shrunk.decal_layers_live, 0);
    assert!(gpu.decal_sources_pending);
    eprintln!(
        "decal layers allocated: {} -> {}",
        grown.decal_layers_allocated, shrunk.decal_layers_allocated
    );
}

#[test]
fn layer_tracker_needs_consecutive_low_ticks() {
    let mut tracker = shadows::LayerShrinkTracker::default();
    tracker.note_used(24);
    // The carried peak keeps the first tick after a drop conservative.
    tracker.note_used(0);
    assert_eq!(tracker.tick(24), None);
    for _ in 0..SHRINK_LOW_TICKS - 1 {
        assert_eq!(tracker.tick(24), None);
    }
    assert_eq!(tracker.tick(24), Some(0));

    // A spike inside the window resets the streak.
    let mut tracker = shadows::LayerShrinkTracker::default();
    tracker.note_used(6);
    assert_eq!(tracker.tick(24), None);
    tracker.note_used(24);
    assert_eq!(tracker.tick(24), None);
    // The carried peak costs one extra tick before the streak restarts.
    tracker.note_used(6);
    for _ in 0..=SHRINK_LOW_TICKS - 1 {
        assert_eq!(tracker.tick(24), None);
    }
    assert_eq!(tracker.tick(24), Some(6));

    // Never shrinks to (or past) what is already allocated.
    let mut tracker = shadows::LayerShrinkTracker::default();
    for _ in 0..=SHRINK_LOW_TICKS {
        tracker.note_used(4);
        assert_eq!(tracker.tick(4), None);
    }
}
