//! GPU memory reclamation driven by the periodic GC tick
//! (`Gpu3D::reclaim_memory_tick`): mesh-arena compaction, shadow-atlas shrink
//! and decal texture-array shrink.
//!
//! Runs against a headless wgpu device; skipped with a note when no adapter is
//! available.
use super::*;
use crate::gpu_shrink::SHRINK_LOW_TICKS;
use perro_ids::{MeshID, TextureID};
use perro_structs::UnitVector4;

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
// Three of these clear MESH_ARENA_COMPACT_MIN_BYTES (16MiB at a 48B stride)
// with room to spare, so dropping two takes the live share well under half.
const TEST_MESH_VERTICES: usize = 120_000;

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

fn new_gpu_3d(device: &wgpu::Device, queue: &wgpu::Queue) -> Gpu3D {
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
    let mut gpu = new_gpu_3d(&device, &queue);
    let builtin_vertices = gpu.memory_report().mesh_arena_vertices;

    // Three custom meshes resolved into the shared arena (what prepare's
    // resolve_mesh_range does on a scene load).
    for index in 0..3u32 {
        let range = gpu
            .append_mesh_data(
                &device,
                &queue,
                "test://reclaim_mesh",
                decoded_mesh(TEST_MESH_VERTICES),
            )
            .expect("append_mesh_data");
        gpu.custom_mesh_ranges
            .insert(MeshID::from_parts(index + 1, 1), (0, range));
    }
    let grown = gpu.memory_report();
    assert_eq!(
        grown.mesh_arena_vertices,
        builtin_vertices + 3 * TEST_MESH_VERTICES
    );
    assert_eq!(grown.mesh_arena_live_vertices, grown.mesh_arena_vertices);
    assert!(!grown.mesh_compact_requested);
    // A full arena never asks for compaction.
    gpu.reclaim_memory_tick(&device);
    assert!(!gpu.memory_report().mesh_compact_requested);

    // Scene switch: the resources are gone, so prepare's
    // `custom_mesh_ranges.retain(has_mesh)` drops their ranges. The bytes stay
    // stranded in the append-only arena.
    gpu.custom_mesh_ranges
        .retain(|mesh_id, _| *mesh_id == MeshID::from_parts(1, 1));
    let stranded = gpu.memory_report();
    assert_eq!(
        stranded.mesh_arena_live_vertices,
        builtin_vertices + TEST_MESH_VERTICES
    );
    assert!(stranded.mesh_arena_bytes > stranded.mesh_arena_live_bytes * 2);

    gpu.reclaim_memory_tick(&device);
    let requested = gpu.memory_report();
    assert!(
        requested.mesh_compact_requested,
        "GC tick must request compaction once under half the arena is live"
    );
    // Still deferred: the arena is untouched until a prepare can consume it.
    assert_eq!(requested.mesh_arena_vertices, grown.mesh_arena_vertices);

    // What prepare does at the top of the frame; `true` becomes
    // `force_full_rebuild`, which re-resolves every live mesh.
    assert!(gpu.compact_custom_mesh_storage_if_needed(&device));
    let compacted = gpu.memory_report();
    assert_eq!(compacted.mesh_arena_vertices, builtin_vertices);
    assert_eq!(compacted.mesh_arena_live_vertices, builtin_vertices);
    assert!(!compacted.mesh_compact_requested);
    assert!(gpu.custom_mesh_ranges.is_empty());
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
    gpu.mesh_compact_requested = true;
    assert!(!gpu.compact_custom_mesh_storage_if_needed(&device));
}

#[test]
fn point_shadow_layers_return_to_baseline_after_lights_leave() {
    let Some((device, queue)) = pollster::block_on(test_device()) else {
        eprintln!("no wgpu adapter; skipping shadow atlas shrink test");
        return;
    };
    let mut gpu = new_gpu_3d(&device, &queue);
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
    let mut gpu = new_gpu_3d(&device, &queue);

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
