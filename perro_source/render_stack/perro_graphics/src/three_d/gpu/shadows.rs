use super::*;
use crate::gpu_shrink::SHRINK_LOW_TICKS;

/// Cascade window snap granularity: the window's world-space min corner is
/// quantized to `window_size / CASCADE_SNAP_STEPS`. Power of two (and the map
/// size is too), so the grid step stays a whole number of shadow texels and the
/// stable-CSM texel alignment survives; the coarseness is what lets a cascade
/// hold its cached depth across many frames of camera motion instead of
/// re-rendering every frame.
///
/// LOWER = coarser grid = more cache hits, and a slightly larger (so coarser-
/// texelled) window, since it grows by `2 / CASCADE_SNAP_STEPS` to keep covering
/// its slice. Measured at 2 world units/frame over 64 frames
/// (`coarser_cascade_snap_trades_texel_size_for_cache_hits`): 32 moves the four
/// cascade windows [63, 63, 47, 20] times, 16 moves them [61, 49, 25, 11] -- the
/// two far cascades, which are the expensive ones to re-raster, roughly halve --
/// for a flat 5.9% texel growth. 16 is the better trade; raise it back toward 32
/// if shadow edges look too blocky on a low map size.
const CASCADE_SNAP_STEPS: f32 = 16.0;

// Test-only override for `CASCADE_SNAP_STEPS`, so one process can measure the
// cascade re-render rate at more than one granularity (32 vs 16) on the same
// repro. Production always reads the constant.
#[cfg(test)]
thread_local! {
    static CASCADE_SNAP_STEPS_OVERRIDE: std::cell::Cell<Option<f32>> =
        const { std::cell::Cell::new(None) };
}

#[inline]
fn cascade_snap_steps() -> f32 {
    #[cfg(test)]
    if let Some(steps) = CASCADE_SNAP_STEPS_OVERRIDE.with(std::cell::Cell::get) {
        return steps;
    }
    CASCADE_SNAP_STEPS
}

#[cfg(test)]
pub(super) fn set_cascade_snap_steps_override(steps: Option<f32>) {
    CASCADE_SNAP_STEPS_OVERRIDE.with(|cell| cell.set(steps));
}

/// Cascade re-render budget: at most this many ray cascades re-render in one
/// frame. Cascade 0 always takes a slot (it holds the closest, most visible
/// shadows and must never lag); the cascades that lose the race keep their
/// previous depth for a frame or two. Raise it to trade GPU back for freshness,
/// lower it to shave more raster off a moving camera.
const CASCADE_RENDER_BUDGET: usize = 2;

/// Angular tolerance for "the ray light points the same way as when this
/// cascade's cached depth was rendered", expressed as `1 - cos(angle)`.
///
/// The budget only holds a cascade back on the assumption that a deferred layer
/// is *self-consistent*: old window, old contents, shadows in the right place.
/// That is true for a translating camera (the light is fixed, so the shadows
/// are), and false the moment the sun animates -- a deferred layer would then
/// paint shadows cast from an older sun angle while its neighbours paint the
/// current one, which reads as shimmering bands at every cascade seam. So a
/// direction change forces the re-render.
///
/// 1e-9 is ~0.0026 degrees. Re-normalizing an unchanged direction reproduces it
/// bit-for-bit (the comparison below is exact at zero difference), so the
/// tolerance only has to absorb noise, not real motion: any animated sun
/// accumulates past it -- against the direction the layer was RENDERED at, not
/// the previous frame's, so even a sub-tolerance drift per frame accumulates --
/// within a frame or two, while a static light never trips it and keeps the
/// budget's win.
const CASCADE_LIGHT_DIR_EPS: f64 = 1.0e-9;

/// True when the ray light no longer points the way `previous` did, past
/// `CASCADE_LIGHT_DIR_EPS`. Both inputs are normalized, so
/// `|a - b|^2 == 2 * (1 - dot(a, b))`; differencing instead of dotting keeps an
/// unchanged direction at exactly zero, where an f32-rounded unit vector's own
/// dot product already sits ~1e-7 short of 1.0 and would drown the tolerance.
/// A non-finite input compares as changed (forces the safe path).
#[inline]
pub(super) fn ray_light_dir_changed(previous: [f32; 3], current: [f32; 3]) -> bool {
    let dx = (current[0] - previous[0]) as f64;
    let dy = (current[1] - previous[1]) as f64;
    let dz = (current[2] - previous[2]) as f64;
    let diff_sq = dx * dx + dy * dy + dz * dz;
    diff_sq.is_nan() || diff_sq > 2.0 * CASCADE_LIGHT_DIR_EPS
}

/// Hard cap on how many consecutive frames one cascade may sit pending. A
/// cascade at the cap renders regardless of the budget, so no layer can starve;
/// it is only reachable when more cascades than the budget want a re-render for
/// this many frames running.
const CASCADE_MAX_DEFER_FRAMES: u32 = 3;

// Keep moving local-light shadows fresh at 30 Hz while lighting itself stays
// full-rate. At 60 Hz this updates half of four point lights each frame; at
// 144 Hz it updates one. Worst pose age stays below one 30 Hz interval.
const LOCAL_SHADOW_REFRESH_HZ: f32 = 30.0;

// Test-only override for `CASCADE_RENDER_BUDGET`, so the repro can measure the
// same camera walk with the budget on and off.
#[cfg(test)]
thread_local! {
    static CASCADE_RENDER_BUDGET_OVERRIDE: std::cell::Cell<Option<usize>> =
        const { std::cell::Cell::new(None) };
}

#[inline]
fn cascade_render_budget() -> usize {
    #[cfg(test)]
    if let Some(budget) = CASCADE_RENDER_BUDGET_OVERRIDE.with(std::cell::Cell::get) {
        return budget;
    }
    CASCADE_RENDER_BUDGET
}

#[cfg(test)]
pub(super) fn set_cascade_render_budget_override(budget: Option<usize>) {
    CASCADE_RENDER_BUDGET_OVERRIDE.with(|cell| cell.set(budget));
}

/// High-water tracker for grow-only *layer* arrays (shadow atlases, decal
/// texture array). Same policy as `crate::gpu_shrink::ShrinkTracker` -- shrink
/// after `SHRINK_LOW_TICKS` consecutive low ticks -- but the target is the
/// exact observed layer need instead of `used * 2`: these arrays re-render
/// (shadows) or re-upload (decals) their contents after a shrink, so nothing is
/// preserved and headroom would only waste memory.
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct LayerShrinkTracker {
    /// Peak need noted since the last GC tick.
    peak: u32,
    /// Most recent need; persists across ticks so an owner whose update is
    /// skipped (state unchanged, contents still drawn) never shrinks below the
    /// layers it last asked for.
    last: u32,
    low_ticks: u32,
}

impl LayerShrinkTracker {
    #[inline]
    pub(super) fn note_used(&mut self, used: u32) {
        self.last = used;
        if used > self.peak {
            self.peak = used;
        }
    }

    /// Periodic GC tick. `Some(target)` (always `< allocated`) once the
    /// observed need stayed below `allocated` for `SHRINK_LOW_TICKS`
    /// consecutive ticks.
    pub(super) fn tick(&mut self, allocated: u32) -> Option<u32> {
        let used = self.peak.max(self.last);
        self.peak = self.last;
        if used >= allocated {
            self.low_ticks = 0;
            return None;
        }
        self.low_ticks += 1;
        if self.low_ticks < SHRINK_LOW_TICKS {
            return None;
        }
        self.low_ticks = 0;
        Some(used)
    }
}

/// Per-atlas layer trackers for the three shadow arrays.
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct ShadowLayerShrink {
    pub(super) ray: LayerShrinkTracker,
    pub(super) spot: LayerShrinkTracker,
    pub(super) point: LayerShrinkTracker,
}

/// Consecutive GC ticks (one per `GC_INTERVAL_FRAMES`, so ~1s each at 60fps) a
/// camera-stream view may encode nothing before its shadow atlases are released
/// outright.
///
/// The shrink pass below cannot reach an idle stream: its trackers are fed from
/// `update_shadow_state`, and a stream the per-frame idle gate skips never
/// prepares, so `last` holds the layer count of the last frame it drew forever.
///
/// 10 matches the two existing idle TTLs (`DECODED_TEXTURE_EVICT_TTL_TICKS`,
/// `PIPELINE_EVICT_GC_TICKS`) -- ~10s of *uninterrupted* idle. Long on purpose:
/// a single rendered frame resets the count, and the resume frame pays a full
/// re-raster of every shadow layer (the grow path forces them all), so a
/// shorter window would trade a real frame spike for memory on a stream that is
/// merely between updates. A stream that is genuinely parked (paused security
/// camera, off-screen portal, minimap on a still scene) crosses it once and
/// gives back its whole atlas set.
pub(super) const STREAM_SHADOW_IDLE_RELEASE_TICKS: u32 = 10;

/// Escape hatch for the multiview shadow path: `PERRO_DISABLE_MULTIVIEW_SHADOWS`
/// forces every `Gpu3D` back onto one render pass per shadow layer, so the two
/// paths can be A/B measured in the same build.
pub(super) fn multiview_shadows_disabled() -> bool {
    static DISABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *DISABLED.get_or_init(|| std::env::var_os("PERRO_DISABLE_MULTIVIEW_SHADOWS").is_some())
}

/// Which of the three shadow atlases a layer set lives in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ShadowAtlas {
    Ray,
    Spot,
    Point,
}

/// One multiview candidate: a contiguous run of shadow layers inside a single
/// atlas that a single depth pass can cover.
///
/// The runs are the natural light groupings, not an arbitrary packing: the four
/// ray cascades, the spot lights (one layer each), and each point light's six
/// cube faces. A multiview pass CLEARS every layer in its mask, so a set is
/// all-or-nothing -- see `shadow_set_needs_render`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ShadowLayerSet {
    pub(super) set_index: usize,
    pub(super) atlas: ShadowAtlas,
    /// First layer in the flat `SHADOW_CAMERA_COUNT` space (scene uniforms,
    /// frustums, `shadow_layer_valid`, `shadow_layer_empty`).
    pub(super) flat_base: usize,
    /// First layer inside the set's own atlas texture.
    pub(super) atlas_base: usize,
    pub(super) count: usize,
    pub(super) label: &'static str,
}

/// Candidate sets: cascades, spot lights, then one per point light.
pub(super) const SHADOW_LAYER_SET_COUNT: usize = 2 + MAX_SHADOW_POINT_LIGHTS;

/// Build this frame's layer sets from the active layer counts. `None` slots are
/// sets with no active layers.
pub(super) fn shadow_layer_sets(
    ray_layers: usize,
    spot_layers: usize,
    point_lights: usize,
) -> [Option<ShadowLayerSet>; SHADOW_LAYER_SET_COUNT] {
    let mut sets = [None; SHADOW_LAYER_SET_COUNT];
    let ray_layers = ray_layers.min(MAX_SHADOW_RAY_CASCADES);
    if ray_layers > 0 {
        sets[0] = Some(ShadowLayerSet {
            set_index: 0,
            atlas: ShadowAtlas::Ray,
            flat_base: 0,
            atlas_base: 0,
            count: ray_layers,
            label: "perro_ray_shadow3d_pass",
        });
    }
    let spot_layers = spot_layers.min(MAX_SHADOW_SPOT_LIGHTS);
    if spot_layers > 0 {
        sets[1] = Some(ShadowLayerSet {
            set_index: 1,
            atlas: ShadowAtlas::Spot,
            flat_base: MAX_SHADOW_RAY_LIGHTS * MAX_SHADOW_RAY_CASCADES,
            atlas_base: 0,
            count: spot_layers,
            label: "perro_spot_shadow3d_pass",
        });
    }
    let point_base = MAX_SHADOW_RAY_LIGHTS * MAX_SHADOW_RAY_CASCADES + MAX_SHADOW_SPOT_LIGHTS;
    for light in 0..point_lights.min(MAX_SHADOW_POINT_LIGHTS) {
        sets[2 + light] = Some(ShadowLayerSet {
            set_index: 2 + light,
            atlas: ShadowAtlas::Point,
            flat_base: point_base + light * POINT_SHADOW_FACE_COUNT,
            atlas_base: light * POINT_SHADOW_FACE_COUNT,
            count: POINT_SHADOW_FACE_COUNT,
            label: "perro_point_shadow3d_pass",
        });
    }
    sets
}

/// Per-SET cache decision, the multiview counterpart of
/// [`shadow_layer_needs_render`] + [`empty_shadow_layer_skips`].
///
/// A multiview pass clears every layer in its mask, so the set cannot render a
/// subset: it renders when at least one of its layers has real work, and then
/// re-rasterizes the whole set. `valid[i]` is the existing per-layer depth cache
/// (`shadow_layer_valid`); `skips_empty[i]` is the existing empty-layer skip
/// (nothing survives that layer's cull AND the layer already holds exactly its
/// clear).
///
/// Both per-layer signals are computed exactly as the single-view path computes
/// them, so a set whose layers are all cached (static scene, unmoved light)
/// costs the same zero as before -- the caching that makes static scenes cheap
/// is preserved, it is only the GRANULARITY of a re-render that coarsens from
/// one layer to one set.
#[inline]
pub(super) fn shadow_set_needs_render(valid: &[bool], skips_empty: &[bool]) -> bool {
    valid
        .iter()
        .zip(skips_empty.iter())
        .any(|(&valid, &skips)| !valid && !skips)
}

/// Recreate one shadow atlas at `layers`. `layers == 0` restores the 1x1
/// single-layer placeholder the constructor allocates (empty layer-view vec, so
/// every shadow render loop clamps back to zero).
fn recreate_shadow_atlas(
    device: &wgpu::Device,
    label: &'static str,
    size: u32,
    layers: u32,
) -> (wgpu::Texture, wgpu::TextureView, Vec<wgpu::TextureView>) {
    if layers == 0 {
        let (texture, view, _) = create_shadow_map_array_texture(device, label, 1, 1);
        return (texture, view, Vec::new());
    }
    create_shadow_map_array_texture(device, label, size, layers)
}

const DEFAULT_SHADOW_STRENGTH: f32 = 0.82;
const DEFAULT_SHADOW_DEPTH_BIAS: f32 = 0.00003;
const DEFAULT_SHADOW_NORMAL_BIAS: f32 = 0.005;

pub(super) struct ShadowSetup {
    pub(super) scenes: Vec<Scene3DUniform>,
    pub(super) uniform: ShadowUniform,
    pub(super) enabled: bool,
    pub(super) ray_enabled: bool,
    pub(super) spot_count: usize,
    pub(super) point_count: usize,
    pub(super) focus_center: Vec3,
    pub(super) focus_radius: f32,
    /// Normalized direction the cascade windows were fitted for -- the only
    /// light state the depth side depends on (colour/intensity/strength/bias are
    /// shading-side and ride in the uniform, which is rewritten in full every
    /// frame regardless of what the budget defers). `[0; 3]` when no ray light
    /// casts shadows.
    pub(super) ray_light_dir: [f32; 3],
}

impl Gpu3D {
    // Grow-only lazy shadow atlases. Eager allocation cost 224MB per Gpu3D
    // (ray 4x2048^2 + spot 4x2048^2 + point 24x1024^2, Depth32Float) even for
    // views with zero shadow lights; each camera-stream subview pays it too.
    // Growing to the observed need keeps a shadowless view at ~0 and a typical
    // view at only the layers its lights use.
    fn ensure_shadow_atlas_capacity(
        &mut self,
        device: &wgpu::Device,
        ray_layers: u32,
        spot_layers: u32,
        point_layers: u32,
    ) -> bool {
        let mut recreated = false;
        if ray_layers > self.ray_shadow_layers_allocated {
            let (texture, view, layer_views) = create_shadow_map_array_texture(
                device,
                "perro_ray_shadow_map",
                self.shadow_map_size,
                ray_layers,
            );
            self._shadow_map_texture = texture;
            self.shadow_map_view = view;
            self.shadow_layer_views = layer_views;
            self.ray_shadow_layers_allocated = ray_layers;
            recreated = true;
        }
        if spot_layers > self.spot_shadow_layers_allocated {
            let (texture, view, layer_views) = create_shadow_map_array_texture(
                device,
                "perro_spot_shadow_map",
                self.shadow_spot_map_size,
                spot_layers,
            );
            self._spot_shadow_map_texture = texture;
            self.spot_shadow_map_view = view;
            self.spot_shadow_layer_views = layer_views;
            self.spot_shadow_layers_allocated = spot_layers;
            recreated = true;
        }
        if point_layers > self.point_shadow_layers_allocated {
            let (texture, view, layer_views) = create_shadow_map_array_texture(
                device,
                "perro_point_shadow_map",
                self.shadow_point_map_size,
                point_layers,
            );
            self._point_shadow_map_texture = texture;
            self.point_shadow_map_view = view;
            self.point_shadow_layer_views = layer_views;
            self.point_shadow_layers_allocated = point_layers;
            recreated = true;
        }
        if recreated {
            self.rebuild_shadow_bind_group(device);
            // The cached per-set array views point at the old textures.
            self.shadow_multiview_layer_views.clear();
        }
        recreated
    }

    /// Depth attachment for one multiview shadow set: a `D2Array` view over
    /// exactly that set's layers, so the pass's mask is the contiguous
    /// `(1 << count) - 1` and plain `MULTIVIEW` suffices (a sparse mask would
    /// need `SELECTIVE_MULTIVIEW`).
    ///
    /// Cached per set and rebuilt whenever an atlas is; `None` means the atlas
    /// does not hold the set's layers yet, and the caller falls back to the
    /// per-layer path.
    fn ensure_shadow_multiview_view(&mut self, set: ShadowLayerSet) -> Option<&wgpu::TextureView> {
        if self.shadow_multiview_layer_views.len() != SHADOW_LAYER_SET_COUNT {
            self.shadow_multiview_layer_views.clear();
            self.shadow_multiview_layer_views
                .resize_with(SHADOW_LAYER_SET_COUNT, || None);
        }
        let (texture, label, allocated) = match set.atlas {
            ShadowAtlas::Ray => (
                &self._shadow_map_texture,
                "perro_ray_shadow_map_multiview",
                self.ray_shadow_layers_allocated,
            ),
            ShadowAtlas::Spot => (
                &self._spot_shadow_map_texture,
                "perro_spot_shadow_map_multiview",
                self.spot_shadow_layers_allocated,
            ),
            ShadowAtlas::Point => (
                &self._point_shadow_map_texture,
                "perro_point_shadow_map_multiview",
                self.point_shadow_layers_allocated,
            ),
        };
        if (set.atlas_base + set.count) as u32 > allocated {
            return None;
        }
        let slot = self.shadow_multiview_layer_views.get_mut(set.set_index)?;
        if slot.as_ref().map(|(count, _)| *count) != Some(set.count as u32) {
            *slot = Some((
                set.count as u32,
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some(label),
                    format: Some(SHADOW_DEPTH_FORMAT),
                    dimension: Some(wgpu::TextureViewDimension::D2Array),
                    usage: Some(wgpu::TextureUsages::RENDER_ATTACHMENT),
                    aspect: wgpu::TextureAspect::DepthOnly,
                    base_mip_level: 0,
                    mip_level_count: Some(1),
                    base_array_layer: set.atlas_base as u32,
                    array_layer_count: Some(set.count as u32),
                }),
            ));
        }
        slot.as_ref().map(|(_, view)| view)
    }

    /// Refresh the per-set attachment views and the group-1 view-proj arrays.
    ///
    /// Runs at the tail of `update_shadow_state`, where the per-layer scene
    /// uniforms have just settled (including the deferred layers, which keep the
    /// matrix their cached depth was drawn with -- `last_shadow_scenes` is the
    /// authority for both the per-layer buffers and this array, so a deferred
    /// layer inside a re-rendered set redraws consistently).
    fn update_shadow_multiview_state(
        &mut self,
        queue: &wgpu::Queue,
        ray_layers: usize,
        spot_layers: usize,
        point_lights: usize,
    ) {
        if !self.shadow_multiview_supported {
            return;
        }
        if self.last_shadow_multiview.len() != SHADOW_LAYER_SET_COUNT {
            self.last_shadow_multiview.clear();
            self.last_shadow_multiview
                .resize(SHADOW_LAYER_SET_COUNT, None);
        }
        for set in shadow_layer_sets(ray_layers, spot_layers, point_lights)
            .into_iter()
            .flatten()
        {
            if self.ensure_shadow_multiview_view(set).is_none() {
                continue;
            }
            let mut uniform = ShadowMultiviewUniform::zeroed();
            for view in 0..set.count.min(MAX_MULTIVIEW_SHADOW_VIEWS) {
                if let Some(Some(scene)) = self.last_shadow_scenes.get(set.flat_base + view) {
                    uniform.view_proj[view] = scene.view_proj;
                }
            }
            if self
                .last_shadow_multiview
                .get(set.set_index)
                .copied()
                .flatten()
                == Some(uniform)
            {
                continue;
            }
            let Some(buffer) = self.shadow_multiview_buffers.get(set.set_index) else {
                continue;
            };
            queue.write_buffer(buffer, 0, bytemuck::bytes_of(&uniform));
            self.last_shadow_multiview[set.set_index] = Some(uniform);
        }
    }

    /// GC-tick half of the lazy shadow atlases: once an atlas needs fewer
    /// layers than it holds for `SHRINK_LOW_TICKS` consecutive ticks, recreate
    /// it at the observed need (0 => back to the 1x1 placeholder). Worst case
    /// this returns 64 + 64 + 96 MB per `Gpu3D` -- and every camera-stream
    /// subview owns its own set.
    ///
    /// No content is preserved: a fresh atlas holds garbage depth, so every
    /// layer is forced to re-render through `shadow_casters_dirty` (which also
    /// defeats `update_shadow_state`'s input memo) plus a direct
    /// `shadow_layer_valid` reset -- the same invalidation the grow path uses.
    pub(super) fn shrink_shadow_atlases_tick(&mut self, device: &wgpu::Device) -> bool {
        let mut shrunk = false;
        if let Some(target) = self
            .shadow_layer_shrink
            .ray
            .tick(self.ray_shadow_layers_allocated)
        {
            let (texture, view, layer_views) =
                recreate_shadow_atlas(device, "perro_ray_shadow_map", self.shadow_map_size, target);
            self._shadow_map_texture = texture;
            self.shadow_map_view = view;
            self.shadow_layer_views = layer_views;
            self.ray_shadow_layers_allocated = target;
            shrunk = true;
        }
        if let Some(target) = self
            .shadow_layer_shrink
            .spot
            .tick(self.spot_shadow_layers_allocated)
        {
            let (texture, view, layer_views) = recreate_shadow_atlas(
                device,
                "perro_spot_shadow_map",
                self.shadow_spot_map_size,
                target,
            );
            self._spot_shadow_map_texture = texture;
            self.spot_shadow_map_view = view;
            self.spot_shadow_layer_views = layer_views;
            self.spot_shadow_layers_allocated = target;
            shrunk = true;
        }
        if let Some(target) = self
            .shadow_layer_shrink
            .point
            .tick(self.point_shadow_layers_allocated)
        {
            let (texture, view, layer_views) = recreate_shadow_atlas(
                device,
                "perro_point_shadow_map",
                self.shadow_point_map_size,
                target,
            );
            self._point_shadow_map_texture = texture;
            self.point_shadow_map_view = view;
            self.point_shadow_layer_views = layer_views;
            self.point_shadow_layers_allocated = target;
            shrunk = true;
        }
        if shrunk {
            self.rebuild_shadow_bind_group(device);
            self.shadow_multiview_layer_views.clear();
            self.invalidate_shadow_layers();
        }
        shrunk
    }

    /// Note that this view encoded its passes this frame. Cleared by the GC
    /// tick, so a camera stream sitting behind the per-frame idle gate reads as
    /// idle from the very first tick that sees no render.
    pub(super) fn note_render_pass(&mut self) {
        self.rendered_since_gc_tick = true;
    }

    /// GC-tick idle reclaim for a CAMERA-STREAM view. Never call it for the
    /// main view: that one renders every frame, so the counter would never
    /// arm -- and a stall that did arm it would thrash a 224MB re-allocation
    /// onto the next frame.
    ///
    /// Releases the shadow atlases and the lazy mesh-blend depth/mask targets.
    /// Returns true on the tick that actually released; the counter never even
    /// arms while everything still sits on its 1x1 placeholder, which is where
    /// a stream with no shadow-casting lights and no mesh blends stays.
    pub fn note_stream_gc_tick(&mut self, device: &wgpu::Device) -> bool {
        if std::mem::take(&mut self.rendered_since_gc_tick) {
            self.idle_gc_ticks = 0;
            return false;
        }
        let holds_atlas = self.ray_shadow_layers_allocated != 0
            || self.spot_shadow_layers_allocated != 0
            || self.point_shadow_layers_allocated != 0;
        let holds_blend_targets = self.mesh_blend_depth_texture.width() > 1
            || self.mesh_blend_depth_texture.height() > 1
            || self.mesh_blend_scene_copy.is_some();
        if !holds_atlas && !holds_blend_targets {
            self.idle_gc_ticks = 0;
            return false;
        }
        self.idle_gc_ticks = self.idle_gc_ticks.saturating_add(1);
        if self.idle_gc_ticks < STREAM_SHADOW_IDLE_RELEASE_TICKS {
            return false;
        }
        self.idle_gc_ticks = 0;
        if holds_atlas {
            self.release_shadow_atlases(device);
        }
        // Same TTL, same self-healing shape: the lazy 1x1 -> full-res upgrade
        // for the mesh-blend depth/mask targets has no release path either.
        if holds_blend_targets {
            self.release_mesh_blend_targets(device);
        }
        true
    }

    /// Drop all three shadow atlases back to the 1x1 placeholder the
    /// constructor allocates, i.e. the fresh-lazy state
    /// `ensure_shadow_atlas_capacity` grows from.
    ///
    /// Same invalidation contract as `shrink_shadow_atlases_tick` (this is that
    /// path with an unconditional target of 0): the atlas views feed exactly one
    /// bind group (`shadow_bind_group`, rebuilt here) and the per-layer views
    /// feed the depth attachments directly, so nothing else holds a handle. The
    /// shadow camera / rigid-shadow / multimesh-shadow-cull bind groups are
    /// buffer-only and stay valid. Layer contents are gone, so every layer is
    /// forced to re-render via `invalidate_shadow_layers`, which also raises
    /// `shadow_casters_dirty` -- that defeats `update_shadow_state`'s input
    /// memo, so the resume frame is guaranteed to reach the grow path even
    /// though camera and lights never moved while the stream was parked.
    ///
    /// No texture is `destroy`ed: the handles are dropped and wgpu keeps the
    /// allocation alive behind any command buffer still in flight.
    pub(super) fn release_shadow_atlases(&mut self, device: &wgpu::Device) {
        let (texture, view, layer_views) =
            recreate_shadow_atlas(device, "perro_ray_shadow_map", self.shadow_map_size, 0);
        self._shadow_map_texture = texture;
        self.shadow_map_view = view;
        self.shadow_layer_views = layer_views;
        self.ray_shadow_layers_allocated = 0;
        let (texture, view, layer_views) = recreate_shadow_atlas(
            device,
            "perro_spot_shadow_map",
            self.shadow_spot_map_size,
            0,
        );
        self._spot_shadow_map_texture = texture;
        self.spot_shadow_map_view = view;
        self.spot_shadow_layer_views = layer_views;
        self.spot_shadow_layers_allocated = 0;
        let (texture, view, layer_views) = recreate_shadow_atlas(
            device,
            "perro_point_shadow_map",
            self.shadow_point_map_size,
            0,
        );
        self._point_shadow_map_texture = texture;
        self.point_shadow_map_view = view;
        self.point_shadow_layer_views = layer_views;
        self.point_shadow_layers_allocated = 0;
        // Trackers held the parked stream's last need; from a placeholder the
        // high-water restarts at whatever the resume frame asks for.
        self.shadow_layer_shrink = ShadowLayerShrink::default();
        // Cascade bookkeeping describes depth that no longer exists. `!was_valid`
        // already forces every cascade, but a stale recorded light direction
        // would outlive its layer.
        self.shadow_cascade_defer_age = [0; MAX_SHADOW_RAY_CASCADES];
        self.shadow_cascade_defer_count = 0;
        self.shadow_cascade_light_dir = [[0.0; 3]; MAX_SHADOW_RAY_CASCADES];
        self.rebuild_shadow_bind_group(device);
        self.shadow_multiview_layer_views.clear();
        self.invalidate_shadow_layers();
    }

    /// Unconditional "every layer's depth contents are gone" reset. Used by the
    /// paths that recreate the atlases or the bind groups the shadow passes
    /// draw through; unlike `shadow_casters_dirty` it is not subject to the
    /// caster-fingerprint confirmation in `update_shadow_state` (nothing about
    /// the *casters* changed -- the target did).
    pub(super) fn invalidate_shadow_layers(&mut self) {
        self.shadow_casters_dirty = true;
        for valid in &mut self.shadow_layer_valid {
            *valid = false;
        }
        // A recreated atlas holds garbage, not a clear, so no layer may claim
        // the empty-layer skip until it has actually been cleared again.
        for empty in &mut self.shadow_layer_empty {
            *empty = false;
        }
    }

    /// Fingerprint of everything the shadow depth passes read: the staged
    /// caster lanes (via the content hashes the upload gates already computed
    /// this prepare, so this costs no second pass over the instance bytes) plus
    /// the batch geometry the passes walk.
    ///
    /// `None` means at least one lane's content is unknown -- a span patch
    /// clears its hash -- which always counts as "moved".
    fn shadow_caster_key(&self) -> Option<u64> {
        use std::hash::{BuildHasher, Hasher};
        fn lane(
            hasher: &mut impl Hasher,
            staged_len: usize,
            uploaded: Option<(usize, u64)>,
        ) -> bool {
            hasher.write_usize(staged_len);
            if staged_len == 0 {
                return true;
            }
            match uploaded {
                Some((len, hash)) => {
                    hasher.write_usize(len);
                    hasher.write_u64(hash);
                    true
                }
                None => false,
            }
        }
        let mut hasher =
            ahash::RandomState::with_seeds(0x5ad0_0001, 0x5ad0_0002, 0x5ad0_0003, 0x5ad0_0004)
                .build_hasher();
        let known = lane(
            &mut hasher,
            self.staged_instance_transforms.len(),
            self.last_uploaded_instance_transforms_hash,
        ) && lane(
            &mut hasher,
            self.staged_rigid_instance_meta.len(),
            self.last_uploaded_rigid_instance_meta_hash,
        ) && lane(
            &mut hasher,
            // Mirrors the upload gate: with no skinned draw staged, nothing
            // reads the rows, so they are staged but never sent (and so carry
            // no content hash).
            if self.staged_has_skinned {
                self.staged_skinned_instance_meta.len()
            } else {
                0
            },
            self.last_uploaded_skinned_instance_meta_hash,
        ) && lane(
            &mut hasher,
            self.staged_skeletons.len(),
            self.last_uploaded_skeletons_hash,
        ) && lane(
            &mut hasher,
            self.staged_blend_shape_weights.len(),
            self.last_uploaded_blend_shape_weights_hash,
        ) && lane(
            &mut hasher,
            self.staged_blend_shape_instance_meta.len(),
            self.last_uploaded_blend_shape_meta_hash,
        ) && lane(
            &mut hasher,
            self.staged_multimesh_instances.len(),
            self.last_uploaded_multimesh_instances_hash,
        ) && lane(
            &mut hasher,
            self.staged_multimesh_draw_params.len(),
            self.last_uploaded_multimesh_draw_params_hash,
        );
        if !known {
            return None;
        }
        // Multimesh blend-shape tails are revision-gated rather than hashed.
        std::hash::Hash::hash(&self.uploaded_multimesh_blend_meta, &mut hasher);
        std::hash::Hash::hash(&self.uploaded_multimesh_blend_weights, &mut hasher);
        hasher.write_usize(self.shadow_batch_indices.len());
        for &index in &self.shadow_batch_indices {
            let batch = self.draw_batches.get(index)?;
            hasher.write_u32(batch.mesh.index_start);
            hasher.write_u32(batch.mesh.index_count);
            hasher.write_i32(batch.mesh.base_vertex);
            hasher.write_u32(batch.instance_start);
            hasher.write_u32(batch.instance_count);
            hasher.write_u8(batch.path as u8);
            hasher.write_u8(u8::from(batch.packed_lod));
            hasher.write_u8(u8::from(batch.double_sided));
        }
        hasher.write_usize(self.multimesh_batches.len());
        for batch in self.multimesh_batches.iter() {
            hasher.write_u32(batch.mesh.index_start);
            hasher.write_u32(batch.mesh.index_count);
            hasher.write_i32(batch.mesh.base_vertex);
            hasher.write_u32(batch.instance_start);
            hasher.write_u32(batch.instance_count);
            hasher.write_u32(batch.draw_param_index);
            hasher.write_u8(u8::from(batch.casts_shadows));
            hasher.write_u8(u8::from(batch.mesh_blend));
            hasher.write_u8(u8::from(batch.double_sided));
        }
        Some(hasher.finish())
    }

    fn rebuild_shadow_bind_group(&mut self, device: &wgpu::Device) {
        self.shadow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_shadow3d_bg"),
            layout: &self.shadow_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.shadow_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.shadow_map_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.shadow_map_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&self.spot_shadow_map_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&self.point_shadow_map_view),
                },
            ],
        });
    }

    pub(super) fn update_shadow_state(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        camera: &Camera3DState,
        lighting: &Lighting3DState,
        has_casters: bool,
    ) {
        static SHADOWS_DISABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let shadows_disabled =
            *SHADOWS_DISABLED.get_or_init(|| std::env::var_os("PERRO_DISABLE_SHADOWS").is_some());
        // Input memo: identical camera/lights/caster-state/target-sizes with no
        // caster movement produce identical shadow state -- skip the whole
        // setup (focus fitting is O(draw_batches) per call).
        let input_key = Some((
            has_casters,
            self.depth_size,
            self.shadow_map_size,
            self.shadow_pcf_high,
        ));
        if !shadows_disabled
            && !self.shadow_casters_dirty
            // Pending temporal work must still run after its input stops;
            // taking the memo here would freeze stale cached depth forever.
            && self.shadow_cascade_defer_count == 0
            && self.shadow_local_defer_count == 0
            && self.last_shadow_input_key == input_key
            && self.last_shadow_input_camera.as_ref() == Some(camera)
            // Shadow-specific compare, not the general `content_eq`: an
            // unpaused Sky3D advances `sky_time_seconds` every frame and would
            // otherwise defeat this memo forever, even though no shadow input
            // moved. See `Lighting3DState::shadow_input_eq`.
            && self
                .last_shadow_input_lighting
                .as_ref()
                .is_some_and(|prev| prev.shadow_input_eq(lighting))
        {
            return;
        }
        self.shadow_setup_run_count = self.shadow_setup_run_count.saturating_add(1);
        self.last_shadow_input_key = input_key;
        self.last_shadow_input_camera = Some(camera.clone());
        self.last_shadow_input_lighting = Some(lighting.clone());
        if shadows_disabled {
            let zero = ShadowUniform::zeroed();
            if self.last_shadow != Some(zero) {
                queue.write_buffer(&self.shadow_buffer, 0, bytemuck::bytes_of(&zero));
                self.last_shadow = Some(zero);
            }
            self.shadow_pass_enabled = false;
            self.ray_shadow_enabled = false;
            self.spot_shadow_count = 0;
            self.point_shadow_count = 0;
            // Nothing needs an atlas: let the GC tick reclaim whatever a
            // pre-disable frame allocated.
            self.shadow_layer_shrink.ray.note_used(0);
            self.shadow_layer_shrink.spot.note_used(0);
            self.shadow_layer_shrink.point.note_used(0);
            self.shadow_cascade_defer_age = [0; MAX_SHADOW_RAY_CASCADES];
            self.shadow_cascade_defer_count = 0;
            self.shadow_spot_defer_age = [0; MAX_SHADOW_SPOT_LIGHTS];
            self.shadow_point_defer_age = [0; MAX_SHADOW_POINT_LIGHTS];
            self.shadow_local_defer_count = 0;
            self.shadow_cascade_light_dir = [[0.0; 3]; MAX_SHADOW_RAY_CASCADES];
            return;
        }
        let mut setup = build_shadow_setup(ShadowSetupArgs {
            camera,
            lighting,
            draw_batches: &self.draw_batches,
            staged_instances: &self.staged_instance_transforms,
            fallback_focus_center: self.shadow_focus_center,
            fallback_focus_radius: self.shadow_focus_radius,
            viewport_width: self.depth_size.0,
            viewport_height: self.depth_size.1,
            shadow_map_size: self.shadow_map_size,
            has_casters,
            scale_budget_to_target: self.shadow_scale_to_target,
        });
        // ray_params.w = PCF kernel select (0 = 4-tap default, 1 = 9-tap via
        // graphics.shadow_quality = "high"); applies to ray, spot and point.
        setup.uniform.ray_params[3] = if self.shadow_pcf_high { 1.0 } else { 0.0 };
        self.shadow_focus_center = setup.focus_center;
        self.shadow_focus_radius = setup.focus_radius;
        let ray_layers = if setup.ray_enabled {
            MAX_SHADOW_RAY_CASCADES as u32
        } else {
            0
        };
        let spot_layers = setup.spot_count as u32;
        let point_layers = setup.point_count.saturating_mul(POINT_SHADOW_FACE_COUNT) as u32;
        // High-water for the GC-tick shrink pass. Noted here (not in the ensure
        // helper) so a *drop* in demand is recorded too; frames that take the
        // input memo above keep the tracker's last value.
        self.shadow_layer_shrink.ray.note_used(ray_layers);
        self.shadow_layer_shrink.spot.note_used(spot_layers);
        self.shadow_layer_shrink.point.note_used(point_layers);
        // Fresh atlas layers hold garbage depth: force every layer to re-render
        // this frame instead of trusting the per-layer cache.
        let atlas_recreated =
            self.ensure_shadow_atlas_capacity(device, ray_layers, spot_layers, point_layers);
        if self.last_shadow_scenes.len() != SHADOW_CAMERA_COUNT {
            self.last_shadow_scenes.resize(SHADOW_CAMERA_COUNT, None);
        }
        if self.shadow_camera_frustums.len() != SHADOW_CAMERA_COUNT {
            self.shadow_camera_frustums
                .resize(SHADOW_CAMERA_COUNT, [Vec4::ZERO; 6]);
        }
        if self.shadow_layer_valid.len() != SHADOW_CAMERA_COUNT {
            self.shadow_layer_valid.resize(SHADOW_CAMERA_COUNT, false);
        }
        if self.shadow_layer_empty.len() != SHADOW_CAMERA_COUNT {
            self.shadow_layer_empty.resize(SHADOW_CAMERA_COUNT, false);
        }
        // Casters moved (rebuild / transform patch / multimesh upload) or a
        // resize/sample-count change touched the shadow targets: every layer's
        // depth is stale this frame.
        //
        // A full rebuild raises `shadow_casters_dirty` for every restage, and a
        // moving camera forces one every frame (LOD selection is camera-driven)
        // even though it reproduces byte-identical caster geometry. Confirming
        // the flag against the caster fingerprint keeps the light-local
        // spot/point layers -- whose view matrices never move with the camera --
        // cached instead of re-rendering all of them every frame the camera
        // moves. The cascade layers still re-render: their scene uniform
        // genuinely changes (`scene_changed` below).
        let caster_key = self.shadow_caster_key();
        let casters_dirty = atlas_recreated
            || (self.shadow_casters_dirty
                && (caster_key.is_none() || self.last_shadow_caster_key != caster_key));
        self.last_shadow_caster_key = caster_key;
        self.shadow_casters_dirty = false;
        if atlas_recreated {
            // Grown atlases carry garbage in the new layers; nothing may claim
            // "already a clean clear" until it has been drawn again.
            for empty in &mut self.shadow_layer_empty {
                *empty = false;
            }
        }
        // Cascade round-robin budget. A translating camera drags several cascade
        // windows across their snap grid at once; rendering all of them in the
        // same frame is the spike. Cap the frame at CASCADE_RENDER_BUDGET
        // cascades and let the losers ride one more frame on their cached depth.
        let cascade_layers = if setup.ray_enabled {
            MAX_SHADOW_RAY_CASCADES
        } else {
            0
        };
        // Falling back to the previous window needs a previous uniform to
        // restore the cascade's matrix + texel from.
        let previous_shadow = self.last_shadow;
        let can_defer = previous_shadow.is_some();
        let ray_light_dir = setup.ray_light_dir;
        let mut cascade_wants = [false; MAX_SHADOW_RAY_CASCADES];
        let mut cascade_forced = [false; MAX_SHADOW_RAY_CASCADES];
        for index in 0..cascade_layers {
            let scene_changed = self.last_shadow_scenes.get(index).copied().flatten()
                != setup.scenes.get(index).copied();
            let was_valid = self.shadow_layer_valid.get(index).copied().unwrap_or(false);
            let (wants, forced) = cascade_render_flags(
                casters_dirty,
                can_defer,
                scene_changed,
                was_valid,
                ray_light_dir_changed(self.shadow_cascade_light_dir[index], ray_light_dir),
            );
            cascade_wants[index] = wants;
            cascade_forced[index] = forced;
        }
        let cascade_granted = schedule_cascade_renders(
            cascade_wants,
            cascade_forced,
            &mut self.shadow_cascade_defer_age,
            cascade_render_budget(),
        );
        self.shadow_cascade_defer_count = (0..cascade_layers)
            .filter(|&index| cascade_wants[index] && !cascade_granted[index])
            .count() as u32;

        let spot_base = MAX_SHADOW_RAY_LIGHTS * MAX_SHADOW_RAY_CASCADES;
        let point_base = spot_base + MAX_SHADOW_SPOT_LIGHTS;
        let mut spot_wants = [false; MAX_SHADOW_SPOT_LIGHTS];
        let mut spot_forced = [false; MAX_SHADOW_SPOT_LIGHTS];
        for slot in 0..setup.spot_count {
            let layer = spot_base + slot;
            let scene_changed = self.last_shadow_scenes.get(layer).copied().flatten()
                != setup.scenes.get(layer).copied();
            let was_valid = self.shadow_layer_valid.get(layer).copied().unwrap_or(false);
            let same_light = previous_shadow.is_some_and(|previous| {
                previous.spot_params[slot][0] > 0.5
                    && previous.spot_params[slot][1] == setup.uniform.spot_params[slot][1]
            });
            spot_wants[slot] = shadow_layer_needs_render(casters_dirty, scene_changed, was_valid);
            // Local lights own isolated maps, so a moving caster may lag with
            // the light as one self-consistent cached image. Unlike cascades,
            // no neighbouring layer shows a newer pose at a seam.
            spot_forced[slot] = !was_valid || !can_defer || !same_light;
        }
        let spot_granted = schedule_local_shadow_renders(
            spot_wants,
            spot_forced,
            &mut self.shadow_spot_defer_age,
            local_shadow_update_budget(setup.spot_count, lighting.frame_delta_seconds),
        );

        let mut point_wants = [false; MAX_SHADOW_POINT_LIGHTS];
        let mut point_forced = [false; MAX_SHADOW_POINT_LIGHTS];
        for slot in 0..setup.point_count {
            let first_layer = point_base + slot * POINT_SHADOW_FACE_COUNT;
            let layers = first_layer..first_layer + POINT_SHADOW_FACE_COUNT;
            let scene_changed = layers.clone().any(|layer| {
                self.last_shadow_scenes.get(layer).copied().flatten()
                    != setup.scenes.get(layer).copied()
            });
            let all_valid = layers
                .clone()
                .all(|layer| self.shadow_layer_valid.get(layer).copied().unwrap_or(false));
            let same_light = previous_shadow.is_some_and(|previous| {
                previous.point_params[slot][0] > 0.5
                    && previous.point_params[slot][1] == setup.uniform.point_params[slot][1]
            });
            point_wants[slot] = shadow_layer_needs_render(casters_dirty, scene_changed, all_valid);
            point_forced[slot] = !all_valid || !can_defer || !same_light;
        }
        let point_granted = schedule_local_shadow_renders(
            point_wants,
            point_forced,
            &mut self.shadow_point_defer_age,
            local_shadow_update_budget(setup.point_count, lighting.frame_delta_seconds),
        );
        self.shadow_local_defer_count = (0..setup.spot_count)
            .filter(|&slot| spot_wants[slot] && !spot_granted[slot])
            .count()
            .saturating_add(
                (0..setup.point_count)
                    .filter(|&slot| point_wants[slot] && !point_granted[slot])
                    .count(),
            ) as u32;

        let scenes = std::mem::take(&mut setup.scenes);
        for (index, scene) in scenes.iter().copied().enumerate() {
            if index < cascade_layers && cascade_wants[index] && !cascade_granted[index] {
                // Deferred: hold the whole cascade at the window its cached
                // depth was rendered for -- matrix, texel, frustum planes and
                // cull planes all stay put, so receivers keep sampling a map
                // that matches them. Only the outer rim of the window (where
                // the camera has since advanced past the fit slack) reads lit.
                if let Some(previous) = previous_shadow {
                    setup.uniform.ray_light_view_proj[index] = previous.ray_light_view_proj[index];
                    setup.uniform.ray_texel[index] = previous.ray_texel[index];
                }
                continue;
            }
            if index >= spot_base && index < spot_base + setup.spot_count {
                let slot = index - spot_base;
                if spot_wants[slot] && !spot_granted[slot] {
                    if let Some(previous) = previous_shadow {
                        setup.uniform.spot_light_view_proj[slot] =
                            previous.spot_light_view_proj[slot];
                        setup.uniform.spot_params[slot] = previous.spot_params[slot];
                    }
                    continue;
                }
            }
            if index >= point_base
                && index < point_base + setup.point_count * POINT_SHADOW_FACE_COUNT
            {
                let local_index = index - point_base;
                let slot = local_index / POINT_SHADOW_FACE_COUNT;
                let face = local_index % POINT_SHADOW_FACE_COUNT;
                if point_wants[slot] && !point_granted[slot] {
                    if let Some(previous) = previous_shadow {
                        setup.uniform.point_light_view_proj
                            [slot * POINT_SHADOW_FACE_COUNT + face] =
                            previous.point_light_view_proj[slot * POINT_SHADOW_FACE_COUNT + face];
                        setup.uniform.point_params[slot] = previous.point_params[slot];
                    }
                    continue;
                }
            }
            if index < cascade_layers {
                // Not deferred: this layer either re-renders now or its window
                // is already the one the current direction fits, so its cached
                // depth belongs to `ray_light_dir` from here on.
                self.shadow_cascade_light_dir[index] = ray_light_dir;
            }
            // Cache the frustum planes so shadow draws can sphere-cull per view.
            let view_proj = Mat4::from_cols_array_2d(&scene.view_proj);
            self.shadow_camera_frustums[index] = extract_frustum_planes(view_proj);
            // Same planes drive the per-layer multimesh instance cull.
            let planes = self.shadow_camera_frustums[index];
            self.write_shadow_cull_planes_if_needed(queue, index, &planes);
            // Per-layer scene diff: an unchanged view keeps its cached depth (skip
            // the buffer write). A changed view (light moved, or a slot newly
            // enabled with a fresh matrix) re-uploads.
            let scene_changed =
                self.last_shadow_scenes.get(index).copied().flatten() != Some(scene);
            if scene_changed {
                if let Some(buffer) = self.shadow_camera_buffers.get(index) {
                    queue.write_buffer(buffer, 0, bytemuck::bytes_of(&scene));
                }
                self.last_shadow_scenes[index] = Some(scene);
            }
            if let Some(valid) = self.shadow_layer_valid.get_mut(index)
                && shadow_layer_needs_render(casters_dirty, scene_changed, *valid)
            {
                *valid = false;
            }
        }
        if self.last_shadow != Some(setup.uniform) {
            queue.write_buffer(&self.shadow_buffer, 0, bytemuck::bytes_of(&setup.uniform));
            self.last_shadow = Some(setup.uniform);
        }
        self.shadow_pass_enabled = setup.enabled;
        self.ray_shadow_enabled = setup.ray_enabled;
        self.spot_shadow_count = setup.spot_count;
        self.point_shadow_count = setup.point_count;
        self.update_shadow_multiview_state(
            queue,
            cascade_layers,
            setup.spot_count,
            setup.point_count,
        );
    }
}

// Per-layer cache decision: a layer must re-render when its casters moved
// (dirty) or its light view-proj (scene uniform) changed since last render.
// An already-valid layer with an unchanged scene keeps its depth contents.
#[inline]
pub(super) fn shadow_layer_needs_render(
    casters_dirty: bool,
    scene_changed: bool,
    was_valid: bool,
) -> bool {
    casters_dirty || scene_changed || !was_valid
}

/// One cascade's two budget inputs: does it want a re-render, and may the
/// round-robin budget hold it back?
///
/// A cascade is *forced* (budget bypassed) whenever deferring it would be
/// wrong rather than merely stale:
/// - `casters_dirty`: a caster moved, so the cached depth holds a shadow that
///   is no longer there.
/// - `!was_valid`: the layer holds garbage depth (fresh/resized atlas).
/// - `!can_defer`: no previously-uploaded uniform to restore the cached
///   window's matrix + texel from, so the layer could not be held consistent.
/// - `light_dir_changed`: the sun rotated. Deferring assumes the cached depth
///   still shows shadows in the right place, which is true when only the camera
///   moved and false as soon as the light direction turns.
///
/// Everything else -- a window that slid because the camera translated -- is
/// freshness, and is what the budget exists to spread over frames.
#[inline]
pub(super) fn cascade_render_flags(
    casters_dirty: bool,
    can_defer: bool,
    scene_changed: bool,
    was_valid: bool,
    light_dir_changed: bool,
) -> (bool, bool) {
    let wants = shadow_layer_needs_render(casters_dirty, scene_changed, was_valid);
    let forced = casters_dirty || !was_valid || !can_defer || light_dir_changed;
    (wants, forced)
}

/// Round-robin cascade budget.
///
/// `wants[i]` = cascade i asked for a re-render (its window moved, or its depth
/// is gone). `forced[i]` = it cannot be deferred at all (see
/// `cascade_render_flags`): casters actually moved, the layer holds garbage
/// depth, the light direction turned, or there is no previously-uploaded
/// uniform to fall back on. Returns which cascades render this frame; the rest
/// keep their cached depth AND the matrix it was drawn with, so each deferred
/// cascade stays internally consistent (old window, old contents).
///
/// Spot/point layers use their own whole-light scheduler below. Their work and
/// consistency rules differ: one point-light grant covers all six cube faces.
pub(super) fn schedule_cascade_renders(
    wants: [bool; MAX_SHADOW_RAY_CASCADES],
    forced: [bool; MAX_SHADOW_RAY_CASCADES],
    ages: &mut [u32; MAX_SHADOW_RAY_CASCADES],
    budget: usize,
) -> [bool; MAX_SHADOW_RAY_CASCADES] {
    let mut granted = [false; MAX_SHADOW_RAY_CASCADES];
    let mut used = 0usize;
    // Un-deferrable work, cascade 0 (closest shadows never lag) and anything
    // that hit the age cap all ignore the budget: correctness before budget.
    for index in 0..MAX_SHADOW_RAY_CASCADES {
        if wants[index] && (forced[index] || index == 0 || ages[index] >= CASCADE_MAX_DEFER_FRAMES)
        {
            granted[index] = true;
            used += 1;
        }
    }
    // Spend what is left oldest-pending first, nearest cascade breaking ties. A
    // straight nearest-first scan would re-serve the same cascade every frame
    // and march the far ones into the age cap together, which then bursts back
    // to a full four-cascade frame -- the cost this budget exists to spread.
    while used < budget {
        let Some(next) = (0..MAX_SHADOW_RAY_CASCADES)
            .filter(|&index| wants[index] && !granted[index])
            .max_by_key(|&index| (ages[index], std::cmp::Reverse(index)))
        else {
            break;
        };
        granted[next] = true;
        used += 1;
    }
    for index in 0..MAX_SHADOW_RAY_CASCADES {
        if wants[index] && !granted[index] {
            ages[index] += 1;
        } else {
            ages[index] = 0;
        }
    }
    granted
}

/// Local-shadow updates per frame for a target refresh of 30 Hz.
///
/// Low frame rates update every light. High frame rates spread moving lights
/// across frames while bounding each slot's age to one refresh interval.
#[inline]
pub(super) fn local_shadow_update_budget(count: usize, frame_delta_seconds: f32) -> usize {
    if count == 0 {
        return 0;
    }
    static BUDGET_DISABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *BUDGET_DISABLED
        .get_or_init(|| std::env::var_os("PERRO_DISABLE_LOCAL_SHADOW_BUDGET").is_some())
    {
        return count;
    }
    if !frame_delta_seconds.is_finite() || frame_delta_seconds <= 0.0 {
        return count;
    }
    ((count as f32 * frame_delta_seconds * LOCAL_SHADOW_REFRESH_HZ).ceil() as usize).clamp(1, count)
}

/// Oldest-first temporal scheduler for spot lights or whole point lights.
/// Forced slots bypass the budget; point-light callers treat all six faces as
/// one slot so no cube map mixes poses.
pub(super) fn schedule_local_shadow_renders<const N: usize>(
    wants: [bool; N],
    forced: [bool; N],
    ages: &mut [u32; N],
    budget: usize,
) -> [bool; N] {
    let mut granted = [false; N];
    let mut used = 0usize;
    for index in 0..N {
        if wants[index] && forced[index] {
            granted[index] = true;
            used += 1;
        }
    }
    while used < budget {
        let Some(next) = (0..N)
            .filter(|&index| wants[index] && !granted[index])
            .max_by_key(|&index| (ages[index], std::cmp::Reverse(index)))
        else {
            break;
        };
        granted[next] = true;
        used += 1;
    }
    for index in 0..N {
        if wants[index] && !granted[index] {
            ages[index] = ages[index].saturating_add(1);
        } else {
            ages[index] = 0;
        }
    }
    granted
}

pub(super) struct ShadowSetupArgs<'a> {
    camera: &'a Camera3DState,
    lighting: &'a Lighting3DState,
    draw_batches: &'a [DrawBatch],
    staged_instances: &'a [TransformInstanceGpu],
    fallback_focus_center: Vec3,
    fallback_focus_radius: f32,
    viewport_width: u32,
    viewport_height: u32,
    shadow_map_size: u32,
    has_casters: bool,
    /// Camera-stream / sub-view instance: the local-light shadow budget scales
    /// with the target (see [`local_shadow_light_budget`]). False leaves the
    /// main view on the full compiled budget.
    scale_budget_to_target: bool,
}

/// How many local (spot / point) lights this view may shadow.
///
/// A point light costs 6 depth passes and a spot light one, at ~7-10us of GPU
/// each -- measured flat in the atlas resolution, so the cost is the PASS, not
/// its pixels. A 256x144 sub-view spending 24 passes on point shadows to shade
/// 37k pixels is the case this exists to stop; every camera stream owns its own
/// set, so the bill multiplies per stream.
///
/// Scales on target HEIGHT, the same basis as `shadow_map_sizes_for_target`, and
/// never drops below one light: a view with shadow-casting lights keeps shadows.
/// A target at or above the reference height keeps the full budget.
pub(super) fn local_shadow_light_budget(max: usize, height: u32) -> usize {
    if max == 0 {
        return 0;
    }
    let height = u64::from(height.max(1)).min(u64::from(SHADOW_TARGET_REFERENCE_HEIGHT));
    let scaled = (max as u64 * height).div_ceil(u64::from(SHADOW_TARGET_REFERENCE_HEIGHT));
    (scaled as usize).clamp(1, max)
}

/// Ranking key for one local light competing for a shadow slot.
///
/// Bigger = more worth a shadow. `range / distance_to_camera` is the light
/// volume's angular size, so a big nearby light outranks a small far one; the
/// intensity factor breaks ties between equally-placed lights. A light the
/// camera sits inside scores at the ceiling.
fn local_shadow_light_score(position: [f32; 3], range: f32, intensity: f32, camera: Vec3) -> f32 {
    let distance = (Vec3::from(position) - camera).length();
    let angular = if distance <= range {
        f32::MAX / 4.0
    } else {
        range / distance.max(1.0e-4)
    };
    if angular.is_finite() {
        angular * intensity.max(1.0e-4)
    } else {
        f32::MAX / 4.0
    }
}

/// Rank the candidates and keep the `budget` best, then restore light order.
///
/// Returning to light order matters: the shadow slot a light lands in feeds
/// `point_light_slots` / `spot_light_slots`, and keeping the order stable across
/// frames keeps the per-layer scene uniforms (and so the depth cache) put while
/// the set does not change.
fn take_best_shadow_lights(mut candidates: Vec<(usize, f32)>, budget: usize) -> Vec<usize> {
    candidates.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    candidates.truncate(budget);
    let mut kept: Vec<usize> = candidates.into_iter().map(|(index, _)| index).collect();
    kept.sort_unstable();
    kept
}

pub(super) fn build_shadow_setup(args: ShadowSetupArgs<'_>) -> ShadowSetup {
    let ShadowSetupArgs {
        camera,
        lighting,
        draw_batches,
        staged_instances,
        fallback_focus_center,
        fallback_focus_radius,
        viewport_width,
        viewport_height,
        shadow_map_size,
        has_casters,
        scale_budget_to_target,
    } = args;
    let mut scenes = vec![Scene3DUniform::zeroed(); SHADOW_CAMERA_COUNT];
    let mut uniform = ShadowUniform::zeroed();
    // -1.0 marks "no shadow slot" for the direct light-index -> slot lookup.
    uniform.spot_light_slots = [[-1.0; 4]; MAX_SPOT_LIGHTS.div_ceil(4)];
    uniform.point_light_slots = [[-1.0; 4]; MAX_POINT_LIGHTS.div_ceil(4)];
    let mut focus_center = fallback_focus_center;
    let mut focus_radius = fallback_focus_radius;
    let mut ray_light_dir = [0.0f32; 3];

    if !has_casters {
        return ShadowSetup {
            scenes,
            uniform,
            enabled: false,
            ray_enabled: false,
            spot_count: 0,
            point_count: 0,
            focus_center,
            focus_radius,
            ray_light_dir,
        };
    }

    let mut any_enabled = false;
    let mut ray_enabled = false;
    if let Some(ray_setup) = build_ray_shadow_scenes(RayShadowSceneArgs {
        camera,
        lighting,
        draw_batches,
        staged_instances,
        fallback_focus_center,
        fallback_focus_radius,
        viewport_width,
        viewport_height,
        shadow_map_size,
    }) {
        for (index, scene) in ray_setup
            .scenes
            .into_iter()
            .enumerate()
            .take(MAX_SHADOW_RAY_CASCADES)
        {
            scenes[index] = scene;
            uniform.ray_light_view_proj[index] = ray_setup.matrices[index].to_cols_array_2d();
        }
        uniform.ray_splits = ray_setup.splits;
        uniform.ray_texel = ray_setup.texels;
        uniform.ray_params = [
            1.0,
            MAX_SHADOW_RAY_CASCADES as f32,
            ray_setup.splits[3],
            0.0,
        ];
        focus_center = ray_setup.focus_center;
        focus_radius = ray_setup.focus_radius;
        ray_light_dir = ray_setup.light_dir.to_array();
        any_enabled = true;
        ray_enabled = true;
    }

    // Camera frustum for the local-light cull below. A light whose volume never
    // reaches the view cannot shade a visible pixel, so its depth layers are
    // pure cost -- and the old selection was first-N-in-list-order, which had no
    // way to know that.
    let camera_frustum = extract_frustum_planes(camera::compute_view_proj_mat(
        camera,
        viewport_width,
        viewport_height,
    ));
    let camera_position = Vec3::from(camera.position);
    let spot_budget = if scale_budget_to_target {
        local_shadow_light_budget(MAX_SHADOW_SPOT_LIGHTS, viewport_height)
    } else {
        MAX_SHADOW_SPOT_LIGHTS
    };
    let point_budget = if scale_budget_to_target {
        local_shadow_light_budget(MAX_SHADOW_POINT_LIGHTS, viewport_height)
    } else {
        MAX_SHADOW_POINT_LIGHTS
    };

    // Dense GPU index -> score, for the lights that clear the eligibility and
    // frustum tests. `take_best_shadow_lights` then keeps the best `budget`.
    let mut spot_candidates: Vec<(usize, f32)> = Vec::new();
    for (dense_index, spot) in lighting.spot_lights.iter().flatten().enumerate() {
        if dense_index >= MAX_SPOT_LIGHTS {
            break;
        }
        if !spot.cast_shadows || spot.intensity <= 1.0e-4 {
            continue;
        }
        // Cone bounded by the light's origin sphere: conservative, and the cheap
        // test that catches lights parked outside the view entirely.
        if !render_pass::shadows::sphere_in_frustum(
            Vec3::from(spot.position),
            spot.range.max(0.0),
            &camera_frustum,
        ) {
            continue;
        }
        spot_candidates.push((
            dense_index,
            local_shadow_light_score(spot.position, spot.range, spot.intensity, camera_position),
        ));
    }
    let spot_keep = take_best_shadow_lights(spot_candidates, spot_budget);

    // The shader indexes scene.spot_lights, which build_scene_uniform fills by
    // compacting Some slots densely, so shadow params must carry the dense
    // index, not the sparse slot index.
    let mut spot_count = 0usize;
    let mut spot_dense_index = 0usize;
    for spot in lighting.spot_lights.iter() {
        let Some(spot) = spot else {
            continue;
        };
        let gpu_index = spot_dense_index;
        spot_dense_index += 1;
        if spot_count >= MAX_SHADOW_SPOT_LIGHTS || gpu_index >= MAX_SPOT_LIGHTS {
            break;
        }
        if !spot_keep.contains(&gpu_index) {
            continue;
        }
        let Some(light_view_proj) = spot_light_view_proj(*spot) else {
            continue;
        };
        let camera_index = MAX_SHADOW_RAY_LIGHTS * MAX_SHADOW_RAY_CASCADES + spot_count;
        scenes[camera_index].view_proj = light_view_proj.to_cols_array_2d();
        uniform.spot_light_view_proj[spot_count] = scenes[camera_index].view_proj;
        uniform.spot_params[spot_count] = [1.0, gpu_index as f32, spot_count as f32, 0.0];
        if gpu_index < MAX_SPOT_LIGHTS {
            uniform.spot_light_slots[gpu_index / 4][gpu_index % 4] = spot_count as f32;
        }
        spot_count += 1;
        any_enabled = true;
    }

    // Point lights are the expensive case: 6 depth layers each.
    let mut point_candidates: Vec<(usize, f32)> = Vec::new();
    for (dense_index, point) in lighting.point_lights.iter().flatten().enumerate() {
        if dense_index >= MAX_POINT_LIGHTS {
            break;
        }
        if !point.cast_shadows || point.intensity <= 1.0e-4 || point.range <= 0.01 {
            continue;
        }
        if !render_pass::shadows::sphere_in_frustum(
            Vec3::from(point.position),
            point.range,
            &camera_frustum,
        ) {
            continue;
        }
        point_candidates.push((
            dense_index,
            local_shadow_light_score(
                point.position,
                point.range,
                point.intensity,
                camera_position,
            ),
        ));
    }
    let point_keep = take_best_shadow_lights(point_candidates, point_budget);

    let mut point_count = 0usize;
    let mut point_dense_index = 0usize;
    for point in lighting.point_lights.iter() {
        let Some(point) = point else {
            continue;
        };
        let gpu_index = point_dense_index;
        point_dense_index += 1;
        if point_count >= MAX_SHADOW_POINT_LIGHTS || gpu_index >= MAX_POINT_LIGHTS {
            break;
        }
        if !point_keep.contains(&gpu_index) {
            continue;
        }
        let matrices = point_light_view_proj(*point);
        let base_layer = point_count * POINT_SHADOW_FACE_COUNT;
        for (face, matrix) in matrices.iter().enumerate().take(POINT_SHADOW_FACE_COUNT) {
            let camera_index = MAX_SHADOW_RAY_LIGHTS * MAX_SHADOW_RAY_CASCADES
                + MAX_SHADOW_SPOT_LIGHTS
                + base_layer
                + face;
            scenes[camera_index].view_proj = matrix.to_cols_array_2d();
            uniform.point_light_view_proj[base_layer + face] = scenes[camera_index].view_proj;
        }
        uniform.point_params[point_count] = [
            1.0,
            gpu_index as f32,
            base_layer as f32,
            point.range.max(0.01),
        ];
        if gpu_index < MAX_POINT_LIGHTS {
            uniform.point_light_slots[gpu_index / 4][gpu_index % 4] = point_count as f32;
        }
        point_count += 1;
        any_enabled = true;
    }

    let [shadow_strength, shadow_depth_bias, shadow_normal_bias] =
        shadow_tuning_from_lighting(lighting);
    uniform.params0 = if any_enabled {
        [1.0, shadow_strength, shadow_depth_bias, shadow_normal_bias]
    } else {
        [0.0; 4]
    };

    ShadowSetup {
        scenes,
        uniform,
        enabled: any_enabled,
        ray_enabled,
        spot_count,
        point_count,
        focus_center,
        focus_radius,
        ray_light_dir,
    }
}

fn shadow_tuning_from_lighting(lighting: &Lighting3DState) -> [f32; 3] {
    if let Some(light) = lighting
        .ray_lights
        .iter()
        .flatten()
        .find(|light| light.cast_shadows && light.intensity > 1.0e-4)
    {
        return sanitize_shadow_tuning(
            light.shadow_strength,
            light.shadow_depth_bias,
            light.shadow_normal_bias,
        );
    }
    if let Some(light) = lighting
        .spot_lights
        .iter()
        .flatten()
        .find(|light| light.cast_shadows && light.intensity > 1.0e-4)
    {
        return sanitize_shadow_tuning(
            light.shadow_strength,
            light.shadow_depth_bias,
            light.shadow_normal_bias,
        );
    }
    if let Some(light) = lighting
        .point_lights
        .iter()
        .flatten()
        .find(|light| light.cast_shadows && light.intensity > 1.0e-4 && light.range > 0.01)
    {
        return sanitize_shadow_tuning(
            light.shadow_strength,
            light.shadow_depth_bias,
            light.shadow_normal_bias,
        );
    }
    [
        DEFAULT_SHADOW_STRENGTH,
        DEFAULT_SHADOW_DEPTH_BIAS,
        DEFAULT_SHADOW_NORMAL_BIAS,
    ]
}

fn sanitize_shadow_tuning(strength: f32, depth_bias: f32, normal_bias: f32) -> [f32; 3] {
    [
        strength.clamp(0.0, 1.0),
        depth_bias.clamp(0.0, 0.01),
        normal_bias.clamp(0.0, 1.0),
    ]
}

struct RayShadowSceneArgs<'a> {
    camera: &'a Camera3DState,
    lighting: &'a Lighting3DState,
    draw_batches: &'a [DrawBatch],
    staged_instances: &'a [TransformInstanceGpu],
    fallback_focus_center: Vec3,
    fallback_focus_radius: f32,
    viewport_width: u32,
    viewport_height: u32,
    shadow_map_size: u32,
}

struct RayShadowScenes {
    scenes: Vec<Scene3DUniform>,
    matrices: [Mat4; MAX_SHADOW_RAY_CASCADES],
    splits: [f32; 4],
    texels: [f32; MAX_SHADOW_RAY_CASCADES],
    focus_center: Vec3,
    focus_radius: f32,
    /// The normalized light direction every window above was fitted with. Kept
    /// so the cascade budget can tell a moved camera (same direction, safe to
    /// defer) from a moved sun (rotated shadows, must re-render).
    light_dir: Vec3,
}

fn build_ray_shadow_scenes(args: RayShadowSceneArgs<'_>) -> Option<RayShadowScenes> {
    let RayShadowSceneArgs {
        camera,
        lighting,
        draw_batches,
        staged_instances,
        fallback_focus_center,
        fallback_focus_radius,
        viewport_width,
        viewport_height,
        shadow_map_size,
    } = args;
    let explicit_shadow_ray = lighting
        .ray_lights
        .iter()
        .flatten()
        .copied()
        .find(|light| light.cast_shadows && light.intensity > 1.0e-4);
    let dir = if DEBUG_FORCE_WORLD_SUN_DIR {
        Vec3::new(
            DEBUG_WORLD_SUN_DIR[0],
            DEBUG_WORLD_SUN_DIR[1],
            DEBUG_WORLD_SUN_DIR[2],
        )
        .normalize_or_zero()
    } else {
        let ray = explicit_shadow_ray?;
        Vec3::from(ray.direction).normalize_or_zero()
    };
    if dir.length_squared() <= 1.0e-6 || !dir.is_finite() {
        return None;
    }

    let (batch_focus_center, batch_focus_radius, has_batch_bounds) =
        compute_shadow_focus_bounds(camera, draw_batches, staged_instances);
    let cascade_splits = ray_cascade_splits(camera);
    let full_corners = camera_frustum_slice_corners_world(
        camera,
        viewport_width,
        viewport_height,
        0.0,
        cascade_splits[MAX_SHADOW_RAY_CASCADES - 1],
    )?;
    let mut focus_center = full_corners
        .iter()
        .copied()
        .fold(Vec3::ZERO, |acc, p| acc + p)
        / (full_corners.len() as f32);
    let mut focus_radius = full_corners
        .iter()
        .copied()
        .map(|p| (p - focus_center).length())
        .fold(0.0f32, f32::max)
        .clamp(10.0, 600.0);
    if has_batch_bounds {
        focus_center = batch_focus_center;
        focus_radius = batch_focus_radius.clamp(10.0, 600.0);
    }
    // Scene caster box. NOT unioned into the per-cascade XY fit (one huge
    // caster would blow every cascade up to the whole scene and defeat the
    // cascade split); it only clamps each cascade's XY window and extends its
    // near range toward the light (see cascade_light_bounds).
    let scene_corners = has_batch_bounds
        .then(|| stable_shadow_focus_points(batch_focus_center, batch_focus_radius));
    if fallback_focus_center.is_finite() && fallback_focus_radius.is_finite() {
        focus_center = fallback_focus_center.lerp(focus_center, 0.20);
        focus_radius = (fallback_focus_radius.max(10.0)
            + (focus_radius.max(10.0) - fallback_focus_radius.max(10.0)) * 0.20)
            .clamp(10.0, 600.0);
    }

    let up = if dir.dot(Vec3::Y).abs() > 0.95 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let (right_axis, up_axis) = light_stable_axes(dir, up);
    // Widest light-plane extent of the (world-static) caster box. Used as the
    // stable upper bound on a cascade's window: it does not move with the
    // camera, so it cannot reintroduce per-frame window resizing.
    let scene_span = scene_corners.as_deref().and_then(|corners| {
        let span = |axis: Vec3| {
            let mut min = f32::INFINITY;
            let mut max = f32::NEG_INFINITY;
            for p in corners {
                let v = axis.dot(*p);
                min = min.min(v);
                max = max.max(v);
            }
            max - min
        };
        let span = span(right_axis).max(span(up_axis));
        span.is_finite().then_some(span)
    });
    let snap_steps = cascade_snap_steps();
    let mut scenes = Vec::with_capacity(MAX_SHADOW_RAY_CASCADES);
    let mut matrices = [Mat4::IDENTITY; MAX_SHADOW_RAY_CASCADES];
    let mut texels = [0.0f32; MAX_SHADOW_RAY_CASCADES];
    for cascade in 0..MAX_SHADOW_RAY_CASCADES {
        let corners = camera_frustum_slice_corners_world(
            camera,
            viewport_width,
            viewport_height,
            if cascade == 0 {
                0.0
            } else {
                cascade_splits[cascade - 1]
            },
            cascade_splits[cascade],
        )?;
        let center =
            corners.iter().copied().fold(Vec3::ZERO, |acc, p| acc + p) / (corners.len() as f32);
        let radius = corners
            .iter()
            .copied()
            .map(|p| (p - center).length())
            .fold(0.0f32, f32::max)
            .clamp(2.0, 600.0);
        let distance = (radius * 3.0).max(80.0);
        let fit_eye = center - dir * distance;
        let fit_view = Mat4::look_at_rh(fit_eye, center, up);
        let (ls_min, ls_max) = cascade_light_bounds(&corners, scene_corners.as_deref(), fit_view)?;
        // Stable window size. The slice's bounding-sphere diameter is invariant
        // under camera rotation AND translation (it depends only on fov, aspect
        // and this cascade's split distances), and the caster-box span is
        // world-static, so the min of the two never changes with camera motion.
        // A per-frame fit of the light-space AABB -- which is what the XY
        // clamp in `cascade_light_bounds` produces -- would resize the window
        // every frame and make the texel grid below meaningless.
        //
        // Coverage still holds: the window is centred on the clamped fit, whose
        // extent is at most `min(slice extent, caster-box extent) <= size`.
        let mut size = 2.0 * radius;
        if let Some(scene_span) = scene_span {
            size = size.min(scene_span);
        }
        // Margin for the PCF kernel reach plus the coarse snap below: the snap
        // can pull the min corner down by up to one grid step, so the window
        // must carry two steps of slack over the fit extent
        // (`size * (1 - 2/CASCADE_SNAP_STEPS) >= fit extent`, satisfied because
        // 1.05 * (1 + 2/STEPS) * (1 - 2/STEPS) > 1).
        let size = (size * 1.05 * (1.0 + 2.0 / snap_steps)).max(2.0);
        let texel = (size / shadow_map_size as f32).max(1.0e-6);
        texels[cascade] = texel.max(1.0e-4);
        // Snap grid, in world units. A whole number of texels (the map size is
        // a power of two and so is CASCADE_SNAP_STEPS), so quantizing to it
        // still lands every texel centre on the same world grid -- the stable-
        // CSM property -- while advancing the window only once per grid step
        // instead of once per texel. A camera translating at 2 units/frame
        // moves a far cascade's window ~8 texels/frame: snapping per texel
        // re-renders that cascade every frame, snapping per grid step
        // re-renders it once every `grid / speed` frames and lets the cached
        // depth stand in between (`shadow_layer_needs_render`).
        let grid = (size / snap_steps).max(1.0e-6);
        let z_pad = (radius * 0.65).max(12.0);
        // Convert the fit (measured in the `fit_eye`-anchored light frame) to
        // absolute light-plane coordinates, so the quantization below is
        // anchored in the world and not in the moving camera.
        // Light space is `axis.dot(p - eye)` for x/y and `-dir.dot(p - eye)`
        // for z, so `axis.dot(fit_eye) + ls` is the absolute coordinate.
        let anchor_x = right_axis.dot(fit_eye);
        let anchor_y = up_axis.dot(fit_eye);
        let anchor_z = dir.dot(fit_eye);
        let fit_center_x = anchor_x + (ls_min.x + ls_max.x) * 0.5;
        let fit_center_y = anchor_y + (ls_min.y + ls_max.y) * 0.5;
        // Larger light-space z = closer to the light, so `-ls_max.z` is the
        // near side. With scene bounds `ls_max.z` already reaches the caster
        // closest to the light, so off-frustum casters between the light and
        // this slice still write depth.
        let near_world = anchor_z - ls_max.z - z_pad;
        let far_world = anchor_z - ls_min.z + z_pad;
        let x0 = ((fit_center_x - size * 0.5) / grid).floor() * grid;
        let y0 = ((fit_center_y - size * 0.5) / grid).floor() * grid;
        let z0 = (near_world / grid).floor() * grid;
        let z1 = (far_world / grid).ceil() * grid;
        let depth = (z1 - z0).max(1.0);
        // Rebuild the light view from the QUANTIZED window corner instead of
        // the camera-following fit eye. Every input to the matrix is then a
        // quantized world coordinate, so an unchanged window reproduces a
        // bit-identical `Scene3DUniform` and the per-layer cache
        // (`scene_changed` in `update_shadow_state`) actually holds. Deriving
        // it from `fit_eye` instead left the matrix drifting in the low bits
        // every frame, which invalidated every cascade on every camera move.
        let eye = right_axis * x0 + up_axis * y0 + dir * z0;
        let view = Mat4::look_at_rh(eye, eye + dir, up);
        let light_view_proj = Mat4::orthographic_rh(0.0, size, 0.0, size, 0.0, depth) * view;
        if !light_view_proj.is_finite() {
            return None;
        }
        let mut scene = Scene3DUniform::zeroed();
        scene.view_proj = light_view_proj.to_cols_array_2d();
        scenes.push(scene);
        matrices[cascade] = light_view_proj;
    }
    Some(RayShadowScenes {
        scenes,
        matrices,
        splits: cascade_splits,
        texels,
        focus_center,
        focus_radius,
        light_dir: dir,
    })
}

fn spot_light_view_proj(spot: SpotLight3DState) -> Option<Mat4> {
    let pos = Vec3::from(spot.position);
    let dir = Vec3::from(spot.direction).normalize_or_zero();
    if dir.length_squared() <= 1.0e-6 || !dir.is_finite() {
        return None;
    }
    let up = if dir.dot(Vec3::Y).abs() > 0.95 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let view = Mat4::look_at_rh(pos, pos + dir, up);
    let outer = spot
        .outer_angle_radians
        .clamp(0.01, std::f32::consts::FRAC_PI_2);
    let proj = Mat4::perspective_rh(
        (outer * 2.0).clamp(0.02, std::f32::consts::PI - 0.01),
        1.0,
        0.05,
        spot.range.max(0.1),
    );
    let vp = proj * view;
    vp.is_finite().then_some(vp)
}

fn ray_cascade_splits(camera: &Camera3DState) -> [f32; MAX_SHADOW_RAY_CASCADES] {
    let (near, far) = match camera.projection {
        CameraProjectionState::Perspective { near, far, .. }
        | CameraProjectionState::Orthographic { near, far, .. }
        | CameraProjectionState::Frustum { near, far, .. } => {
            let near = sanitize_near(near);
            let far = sanitize_far(far, near).min(near + 220.0);
            (near, far)
        }
    };
    let lambda = 0.65f32;
    let mut splits = [far; MAX_SHADOW_RAY_CASCADES];
    for i in 1..=MAX_SHADOW_RAY_CASCADES {
        let p = i as f32 / MAX_SHADOW_RAY_CASCADES as f32;
        let log = near * (far / near).powf(p);
        let uniform = near + (far - near) * p;
        splits[i - 1] = (log * lambda + uniform * (1.0 - lambda)).min(far);
    }
    splits
}

fn camera_rotation(camera: &Camera3DState) -> Quat {
    let rot = Quat::from_xyzw(
        camera.rotation[0],
        camera.rotation[1],
        camera.rotation[2],
        camera.rotation[3],
    );
    if rot.is_finite() && rot.length_squared() > 1.0e-6 {
        rot.normalize()
    } else {
        Quat::IDENTITY
    }
}

fn camera_frustum_slice_corners_world(
    camera: &Camera3DState,
    width: u32,
    height: u32,
    start_distance: f32,
    end_distance: f32,
) -> Option<Vec<Vec3>> {
    let aspect = (width.max(1) as f32 / height.max(1) as f32).max(1.0e-6);
    let pos = Vec3::from(camera.position);
    let rot = camera_rotation(camera);
    let right = rot * Vec3::X;
    let up = rot * Vec3::Y;
    let forward = rot * Vec3::NEG_Z;
    let mut out = Vec::with_capacity(8);
    match camera.projection {
        CameraProjectionState::Perspective {
            fov_y_degrees,
            near,
            far,
        } => {
            let near = sanitize_near(near);
            let far = sanitize_far(far, near).min(near + 220.0);
            let a = start_distance.max(near).min(far);
            let b = end_distance.max(a + 1.0e-3).min(far);
            let tan_y = (perspective_fov_y_radians(fov_y_degrees) * 0.5).tan();
            for d in [a, b] {
                let center = pos + forward * d;
                let half_h = d * tan_y;
                let half_w = half_h * aspect;
                for y in [-1.0f32, 1.0] {
                    for x in [-1.0f32, 1.0] {
                        out.push(center + right * (x * half_w) + up * (y * half_h));
                    }
                }
            }
        }
        CameraProjectionState::Orthographic { size, near, far } => {
            let near = sanitize_near(near);
            let far = sanitize_far(far, near).min(near + 220.0);
            let a = start_distance.max(near).min(far);
            let b = end_distance.max(a + 1.0e-3).min(far);
            let half_h = (size.abs() * 0.5).max(1.0e-3);
            let half_w = half_h * aspect;
            for d in [a, b] {
                let center = pos + forward * d;
                for y in [-1.0f32, 1.0] {
                    for x in [-1.0f32, 1.0] {
                        out.push(center + right * (x * half_w) + up * (y * half_h));
                    }
                }
            }
        }
        CameraProjectionState::Frustum {
            left,
            right: fr_right,
            bottom,
            top,
            near,
            far,
        } => {
            let near = sanitize_near(near);
            let far = sanitize_far(far, near).min(near + 220.0);
            let (left, fr_right) = sanitize_range(left, fr_right, -1.0, 1.0);
            let (bottom, top) = sanitize_range(bottom, top, -1.0, 1.0);
            let a = start_distance.max(near).min(far);
            let b = end_distance.max(a + 1.0e-3).min(far);
            for d in [a, b] {
                let scale = d / near;
                let center = pos + forward * d;
                for y in [bottom * scale, top * scale] {
                    for x in [left * scale, fr_right * scale] {
                        out.push(center + right * x + up * y);
                    }
                }
            }
        }
    }
    out.iter().all(|p| p.is_finite()).then_some(out)
}

fn point_light_view_proj(point: PointLight3DState) -> [Mat4; POINT_SHADOW_FACE_COUNT] {
    let pos = Vec3::from(point.position);
    let proj = Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, 0.05, point.range.max(0.1));
    let faces = [
        (Vec3::X, Vec3::NEG_Y),
        (Vec3::NEG_X, Vec3::NEG_Y),
        (Vec3::Y, Vec3::Z),
        (Vec3::NEG_Y, Vec3::NEG_Z),
        (Vec3::Z, Vec3::NEG_Y),
        (Vec3::NEG_Z, Vec3::NEG_Y),
    ];
    faces.map(|(dir, up)| proj * Mat4::look_at_rh(pos, pos + dir, up))
}

pub(super) fn light_space_bounds(points_world: &[Vec3], light_view: Mat4) -> Option<(Vec3, Vec3)> {
    let mut it = points_world.iter().copied();
    let first = it.next()?;
    let first_ls = (light_view * first.extend(1.0)).truncate();
    if !first_ls.is_finite() {
        return None;
    }
    let mut min = first_ls;
    let mut max = first_ls;
    for p in it {
        let ls = (light_view * p.extend(1.0)).truncate();
        if !ls.is_finite() {
            continue;
        }
        min = min.min(ls);
        max = max.max(ls);
    }
    if !min.is_finite() || !max.is_finite() {
        None
    } else {
        Some((min, max))
    }
}

// Light-space fit for one cascade: fit the camera slice corners, then use the
// scene caster box (when present) two ways:
// - Clamp (intersect) the XY window so the cascade never spends texels on
//   area with no casters. A collapsed intersection falls back to the plain
//   slice fit — that map holds no casters, so every sample falls through lit.
// - Extend max z (toward the light) so casters outside the camera frustum but
//   between the light and the slice still land in the depth map.
fn cascade_light_bounds(
    slice_corners: &[Vec3],
    scene_corners: Option<&[Vec3]>,
    light_view: Mat4,
) -> Option<(Vec3, Vec3)> {
    let (mut ls_min, mut ls_max) = light_space_bounds(slice_corners, light_view)?;
    let Some(scene_corners) = scene_corners else {
        return Some((ls_min, ls_max));
    };
    let Some((scene_min, scene_max)) = light_space_bounds(scene_corners, light_view) else {
        return Some((ls_min, ls_max));
    };
    let clamped_min_x = ls_min.x.max(scene_min.x);
    let clamped_max_x = ls_max.x.min(scene_max.x);
    let clamped_min_y = ls_min.y.max(scene_min.y);
    let clamped_max_y = ls_max.y.min(scene_max.y);
    if clamped_max_x - clamped_min_x > 1.0e-3 && clamped_max_y - clamped_min_y > 1.0e-3 {
        ls_min.x = clamped_min_x;
        ls_max.x = clamped_max_x;
        ls_min.y = clamped_min_y;
        ls_max.y = clamped_max_y;
    }
    // Larger view-space z = closer to the light (look_at_rh looks down -Z).
    ls_max.z = ls_max.z.max(scene_max.z);
    Some((ls_min, ls_max))
}

pub(super) fn compute_shadow_focus_bounds(
    camera: &Camera3DState,
    draw_batches: &[DrawBatch],
    staged_instances: &[TransformInstanceGpu],
) -> (Vec3, f32, bool) {
    let mut any = false;
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for batch in draw_batches {
        if batch.draw_on_top || !batch.casts_shadows || batch.alpha_mode == 2 {
            continue;
        }
        let start = batch.instance_start as usize;
        let end = (batch.instance_start + batch.instance_count) as usize;
        for inst in staged_instances.get(start..end).unwrap_or(&[]).iter() {
            let model = Mat4::from_cols_array_2d(&model_cols_from_affine_rows(inst));
            if !model.is_finite() {
                continue;
            }
            let local_center = Vec3::new(
                batch.local_center[0],
                batch.local_center[1],
                batch.local_center[2],
            );
            let center_world = (model * local_center.extend(1.0)).truncate();
            let sx = Vec3::new(model.x_axis.x, model.x_axis.y, model.x_axis.z).length();
            let sy = Vec3::new(model.y_axis.x, model.y_axis.y, model.y_axis.z).length();
            let sz = Vec3::new(model.z_axis.x, model.z_axis.y, model.z_axis.z).length();
            let radius_world =
                (batch.local_radius.max(0.0) * sx.max(sy).max(sz).max(1.0e-6)).max(0.25);
            min = min.min(center_world - Vec3::splat(radius_world));
            max = max.max(center_world + Vec3::splat(radius_world));
            any = true;
        }
    }
    if !any {
        return (Vec3::from(camera.position), 64.0, false);
    }
    let center = (min + max) * 0.5;
    let radius = ((max - min) * 0.5).length().clamp(10.0, 600.0);
    (center, radius, true)
}

pub(super) fn light_stable_axes(light_dir: Vec3, fallback_up: Vec3) -> (Vec3, Vec3) {
    let f = light_dir.normalize_or_zero();
    let mut right = f.cross(fallback_up).normalize_or_zero();
    if right.length_squared() <= 1.0e-6 {
        let alt_up = if f.dot(Vec3::Y).abs() > 0.95 {
            Vec3::X
        } else {
            Vec3::Y
        };
        right = f.cross(alt_up).normalize_or_zero();
    }
    let up = right.cross(f).normalize_or_zero();
    (right, up)
}

fn stable_shadow_focus_points(center: Vec3, radius: f32) -> Vec<Vec3> {
    let r = radius.max(0.1);
    let mut points = Vec::with_capacity(8);
    for z in [-1.0f32, 1.0] {
        for y in [-1.0f32, 1.0] {
            for x in [-1.0f32, 1.0] {
                points.push(center + Vec3::new(x, y, z) * r);
            }
        }
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;
    use perro_graphics_assets::MeshRange;
    use perro_render_bridge::RayLight3DState;

    fn camera(rotation: Quat) -> Camera3DState {
        Camera3DState {
            position: [0.0, 2.0, 8.0],
            rotation: rotation.to_array(),
            projection: CameraProjectionState::Perspective {
                fov_y_degrees: 60.0,
                near: 0.1,
                far: 100.0,
            },
            render_mask: BitMask::NONE,
            post_processing: Arc::from([]),
            audio_options: perro_structs::AudioListenerOptions::new(),
        }
    }

    fn caster_batch() -> DrawBatch {
        let material_kind = MaterialPipelineKind::Standard;
        let state_key =
            draw_batch_state_key(RenderPath3D::Rigid, false, false, 0, false, &material_kind);
        let material_texture_key = MaterialTextureKey::from_base(0);
        DrawBatch {
            state_key,
            render_state: render_state_key(
                state_key,
                material_texture_key.state_hash(),
                0,
                0,
                false,
                0,
                false,
            ),
            mesh: MeshRange {
                index_start: 0,
                index_count: 3,
                base_vertex: 0,
            },
            instance_start: 0,
            instance_count: 1,
            path: RenderPath3D::Rigid,
            packed_lod: false,
            double_sided: false,
            material_kind,
            alpha_mode: 0,
            draw_on_top: false,
            base_color_texture_slot: 0,
            material_texture_key,
            local_center: [0.0, 0.0, 0.0],
            local_radius: 2.0,
            occlusion_query: None,
            disable_hiz_occlusion: false,
            casts_shadows: true,
            receives_shadows: true,
            mesh_blend: false,
            mesh_blend_screen: false,
            mesh_blend_params: 0,
            mesh_blend_params_ext: 0,
            mesh_blend_depth: false,
            blend_layers: BitMask::ALL.bits(),
            blend_mask: BitMask::NONE.bits(),
            order_index: 0,
        }
    }

    fn identity_instance() -> TransformInstanceGpu {
        TransformInstanceGpu {
            model_row_0: [1.0, 0.0, 0.0, 0.0],
            model_row_1: [0.0, 1.0, 0.0, 0.0],
            model_row_2: [0.0, 0.0, 1.0, 0.0],
        }
    }

    fn lighting_with_ray(dir: [f32; 3]) -> Lighting3DState {
        let mut lighting = Lighting3DState::default();
        lighting.ray_lights[0] = Some(RayLight3DState {
            direction: dir,
            color: [1.0, 1.0, 1.0],
            intensity: 1.0,
            cast_shadows: true,
            shadow_strength: 0.82,
            shadow_depth_bias: 0.00018,
            shadow_normal_bias: 0.045,
        });
        lighting
    }

    /// Walk the camera in a straight line and count how many times each
    /// cascade's window (its light view-proj) actually moves. A window that does
    /// not move is a cached layer that skips its depth pass entirely, so this is
    /// the cache-hit measurement `CASCADE_SNAP_STEPS` is tuned against.
    fn cascade_window_moves(steps: f32, frames: usize, speed: f32) -> ([u32; 4], [f32; 4]) {
        set_cascade_snap_steps_override(Some(steps));
        let mut ground = caster_batch();
        ground.local_radius = 400.0;
        let batches = [ground];
        let instances = [identity_instance()];
        let mut moves = [0u32; MAX_SHADOW_RAY_CASCADES];
        let mut texels = [0.0f32; MAX_SHADOW_RAY_CASCADES];
        let mut previous: Option<[[[f32; 4]; 4]; MAX_SHADOW_RAY_CASCADES]> = None;
        for frame in 0..frames {
            let mut moving = camera(Quat::IDENTITY);
            moving.position[0] = frame as f32 * speed;
            let setup = build_shadow_setup(ShadowSetupArgs {
                camera: &moving,
                lighting: &lighting_with_ray([-0.4, -1.0, -0.3]),
                draw_batches: &batches,
                staged_instances: &instances,
                fallback_focus_center: Vec3::ZERO,
                fallback_focus_radius: 64.0,
                viewport_width: 1280,
                viewport_height: 720,
                shadow_map_size: SHADOW_MAP_SIZE,
                has_casters: true,
                scale_budget_to_target: false,
            });
            if let Some(previous) = previous {
                for cascade in 0..MAX_SHADOW_RAY_CASCADES {
                    if setup.uniform.ray_light_view_proj[cascade] != previous[cascade] {
                        moves[cascade] += 1;
                    }
                }
            }
            previous = Some(setup.uniform.ray_light_view_proj);
            texels = setup.uniform.ray_texel;
        }
        set_cascade_snap_steps_override(None);
        (moves, texels)
    }

    /// The `CASCADE_SNAP_STEPS` trade, measured: a coarser snap grid holds every
    /// cascade's cached depth over more frames of camera motion, at the price of
    /// a proportionally larger (so coarser-texelled) window.
    #[test]
    fn coarser_cascade_snap_trades_texel_size_for_cache_hits() {
        let frames = 64;
        let speed = 2.0;
        let (moves_32, texels_32) = cascade_window_moves(32.0, frames, speed);
        let (moves_16, texels_16) = cascade_window_moves(16.0, frames, speed);
        let total_32: u32 = moves_32.iter().sum();
        let total_16: u32 = moves_16.iter().sum();
        let coarsening: Vec<f32> = (0..MAX_SHADOW_RAY_CASCADES)
            .map(|c| texels_16[c] / texels_32[c].max(1.0e-9))
            .collect();
        println!(
            "cascade snap steps over {frames} frames @ {speed} u/frame: \
             32 -> per-cascade window moves {moves_32:?} (total {total_32}), \
             16 -> {moves_16:?} (total {total_16}); texel 32={texels_32:?} \
             16={texels_16:?} coarsening={coarsening:?}"
        );
        assert!(
            total_16 < total_32,
            "16 steps must not move windows more often than 32 ({total_16} vs {total_32})"
        );
        // Halving the step count doubles the grid, so a window survives about
        // twice as many frames of the same motion. Only the far cascades can
        // show it: cascades 0/1 have windows small enough that any real camera
        // speed crosses a grid step every frame at either setting -- and they
        // are also the cheapest layers to re-render.
        for cascade in 2..MAX_SHADOW_RAY_CASCADES {
            assert!(
                moves_16[cascade] * 3 < moves_32[cascade] * 2,
                "cascade {cascade}: 16 steps must cut window moves well below 32 \
                 ({} vs {})",
                moves_16[cascade],
                moves_32[cascade]
            );
        }
        // ...and the window (hence the texel) only grows by the extra snap
        // slack, a few percent -- not the ~2x the cache win is worth.
        for (cascade, ratio) in coarsening.iter().enumerate() {
            assert!(
                *ratio < 1.10,
                "cascade {cascade} texel grew {ratio}x, far past the snap slack"
            );
        }
    }

    /// Outcome of running the real cascade-budget predicates
    /// (`cascade_render_flags` + `schedule_cascade_renders`) over real
    /// `build_shadow_setup` output, frame by frame -- the same sequence
    /// `update_shadow_state` runs, minus the GPU.
    #[derive(Debug, Default)]
    struct CascadeBudgetSim {
        /// Frames each cascade asked for a re-render.
        wanted: [u32; MAX_SHADOW_RAY_CASCADES],
        /// Frames each cascade got one.
        rendered: [u32; MAX_SHADOW_RAY_CASCADES],
        /// Total (cascade, frame) pairs the budget held back.
        deferrals: u32,
        /// Cascades rendered per frame.
        per_frame: Vec<usize>,
        /// Worst angle, in degrees, between the light direction a deferred
        /// cascade's cached depth was rendered at and the current one. This is
        /// the artifact being bounded: a deferred layer paints shadows cast
        /// from a sun this far from where the sun now is.
        worst_stale_sun_deg: f64,
    }

    fn simulate_cascade_budget(
        frames: usize,
        budget: usize,
        mut frame_state: impl FnMut(usize) -> (Camera3DState, Lighting3DState),
    ) -> CascadeBudgetSim {
        let mut ground = caster_batch();
        ground.local_radius = 400.0;
        let batches = [ground];
        let instances = [identity_instance()];
        let mut sim = CascadeBudgetSim::default();
        let mut ages = [0u32; MAX_SHADOW_RAY_CASCADES];
        let mut valid = [false; MAX_SHADOW_RAY_CASCADES];
        let mut cached_dir = [[0.0f32; 3]; MAX_SHADOW_RAY_CASCADES];
        let mut cached_scene: [Option<Scene3DUniform>; MAX_SHADOW_RAY_CASCADES] =
            [None; MAX_SHADOW_RAY_CASCADES];
        for frame in 0..frames {
            let (view, lighting) = frame_state(frame);
            let setup = build_shadow_setup(ShadowSetupArgs {
                camera: &view,
                lighting: &lighting,
                draw_batches: &batches,
                staged_instances: &instances,
                fallback_focus_center: Vec3::ZERO,
                fallback_focus_radius: 64.0,
                viewport_width: 1280,
                viewport_height: 720,
                shadow_map_size: SHADOW_MAP_SIZE,
                has_casters: true,
                scale_budget_to_target: false,
            });
            assert!(setup.ray_enabled, "frame {frame} lost its ray shadow");
            let dir = setup.ray_light_dir;
            // `can_defer` is "a previous uniform exists to restore the cached
            // window from", i.e. every frame after the first.
            let can_defer = frame > 0;
            let mut wants = [false; MAX_SHADOW_RAY_CASCADES];
            let mut forced = [false; MAX_SHADOW_RAY_CASCADES];
            for cascade in 0..MAX_SHADOW_RAY_CASCADES {
                let scene_changed = cached_scene[cascade] != setup.scenes.get(cascade).copied();
                let (want, force) = cascade_render_flags(
                    false,
                    can_defer,
                    scene_changed,
                    valid[cascade],
                    ray_light_dir_changed(cached_dir[cascade], dir),
                );
                wants[cascade] = want;
                forced[cascade] = force;
            }
            let granted = schedule_cascade_renders(wants, forced, &mut ages, budget);
            let mut this_frame = 0usize;
            for cascade in 0..MAX_SHADOW_RAY_CASCADES {
                if wants[cascade] {
                    sim.wanted[cascade] += 1;
                }
                if wants[cascade] && !granted[cascade] {
                    sim.deferrals += 1;
                    let stale = (Vec3::from(cached_dir[cascade]) - Vec3::from(dir)).length() as f64;
                    sim.worst_stale_sun_deg = sim
                        .worst_stale_sun_deg
                        .max((2.0 * (stale * 0.5).clamp(0.0, 1.0).asin()).to_degrees());
                    continue;
                }
                if granted[cascade] {
                    sim.rendered[cascade] += 1;
                    this_frame += 1;
                }
                // Rendered, or already holding the window the current direction
                // fits: either way its depth now belongs to `dir`.
                cached_scene[cascade] = setup.scenes.get(cascade).copied();
                cached_dir[cascade] = dir;
                valid[cascade] = true;
            }
            sim.per_frame.push(this_frame);
        }
        sim
    }

    /// An animated sun -- the sky demo's time-of-day cycle -- rotates the light
    /// direction every frame, which rotates the shadows themselves. The budget's
    /// premise (a deferred layer is self-consistent: old window, old contents,
    /// shadows still in the right place) only holds while the light is fixed. A
    /// cascade deferred across a sun move paints shadows cast from the OLD sun
    /// angle next to neighbours painting the new one, which bands and shimmers
    /// at every cascade seam -- worse than the spike the budget avoids. So a
    /// direction change forces every invalidated cascade, budget or not.
    #[test]
    fn animated_sun_forces_every_cascade_past_the_budget() {
        // 0.25 deg/frame: a 24-second full sky cycle at 60 fps.
        let step = 0.25f32.to_radians();
        let sim = simulate_cascade_budget(48, 2, |frame| {
            let angle = 0.7 + frame as f32 * step;
            (
                camera(Quat::IDENTITY),
                lighting_with_ray([-angle.cos(), -angle.sin(), -0.25]),
            )
        });
        println!(
            "animated sun @0.25 deg/frame: wanted={:?} rendered={:?} deferrals={} \
             per-frame renders max={:?}",
            sim.wanted,
            sim.rendered,
            sim.deferrals,
            sim.per_frame.iter().max()
        );
        assert_eq!(
            sim.deferrals, 0,
            "a moving sun must not leave any cascade on stale-direction depth"
        );
        for cascade in 0..MAX_SHADOW_RAY_CASCADES {
            assert!(
                sim.wanted[cascade] >= 47,
                "cascade {cascade} should be invalidated every frame the sun turns: {:?}",
                sim.wanted
            );
            assert_eq!(
                sim.rendered[cascade], sim.wanted[cascade],
                "cascade {cascade} must render every frame it is invalidated by the sun"
            );
        }
    }

    /// Even a sun far too slow to trip the tolerance in one frame must not
    /// stick: the comparison is against the direction each layer was RENDERED
    /// at, not the previous frame's, so the drift accumulates and forces within
    /// a frame or two. What the tolerance buys is a bound on how far a deferred
    /// cascade's sun may lag -- and that bound must be invisible.
    #[test]
    fn a_very_slow_sun_still_forces_and_bounds_the_stale_angle() {
        // 0.002 deg/frame: a ~50-minute sky cycle at 60 fps, ~1/70th of the
        // tolerance angle per frame.
        let step = 0.002f32.to_radians();
        let sim = simulate_cascade_budget(120, 2, |frame| {
            let angle = 0.7 + frame as f32 * step;
            (
                camera(Quat::IDENTITY),
                lighting_with_ray([-angle.cos(), -angle.sin(), -0.25]),
            )
        });
        println!(
            "very slow sun @0.002 deg/frame: rendered={:?} of wanted={:?} \
             deferrals={} worst stale sun={:.6} deg",
            sim.rendered, sim.wanted, sim.deferrals, sim.worst_stale_sun_deg
        );
        for cascade in 0..MAX_SHADOW_RAY_CASCADES {
            assert!(
                sim.rendered[cascade] > 0,
                "cascade {cascade} never re-rendered under a slow sun"
            );
        }
        assert!(
            sim.worst_stale_sun_deg < 0.01,
            "a deferred cascade lagged the sun by {:.6} degrees -- past the \
             invisible-error budget the tolerance is meant to hold",
            sim.worst_stale_sun_deg
        );
    }

    /// The case the budget exists for, and the one the fix must not cost: the
    /// camera translates under a fixed sun. Shadows do not move, so a deferred
    /// cascade stays correct, and the frame stays capped at the budget.
    #[test]
    fn translating_camera_under_a_static_sun_keeps_the_budget() {
        let budget = 2;
        let sim = simulate_cascade_budget(48, budget, |frame| {
            let mut moving = camera(Quat::IDENTITY);
            moving.position[0] = frame as f32 * 2.0;
            (moving, lighting_with_ray([-0.4, -1.0, -0.3]))
        });
        println!(
            "translating camera @2 u/frame, static sun: wanted={:?} rendered={:?} \
             deferrals={} per-frame renders={:?}",
            sim.wanted, sim.rendered, sim.deferrals, sim.per_frame
        );
        assert_eq!(
            sim.worst_stale_sun_deg, 0.0,
            "a static sun must never register as moved"
        );
        assert!(
            sim.deferrals > 0,
            "the budget must still spread a translating camera's cascade renders"
        );
        // Frame 0 has no cached depth and no previous uniform, so every cascade
        // is forced; every frame after it must respect the cap.
        for (frame, rendered) in sim.per_frame.iter().enumerate().skip(1) {
            assert!(
                *rendered <= budget,
                "frame {frame} rendered {rendered} cascades, past the budget of {budget}"
            );
        }
    }

    #[test]
    fn light_dir_change_tolerance_ignores_noise_and_catches_animation() {
        let dir = Vec3::new(-0.5, -1.0, -0.2).normalize().to_array();
        assert!(
            !ray_light_dir_changed(dir, dir),
            "an unchanged direction must never force a re-render"
        );
        // One f32 ulp of wobble on each component: noise, not animation.
        let wobble = [
            f32::from_bits(dir[0].to_bits() + 1),
            f32::from_bits(dir[1].to_bits() + 1),
            f32::from_bits(dir[2].to_bits() + 1),
        ];
        assert!(!ray_light_dir_changed(dir, wobble));
        // A hundredth of a degree is already real animation.
        let turned = (Quat::from_rotation_y(0.01f32.to_radians()) * Vec3::from(dir)).to_array();
        assert!(
            ray_light_dir_changed(dir, turned),
            "0.01 degrees of sun rotation must force a re-render"
        );
        // A zeroed slot (no cached direction yet) counts as changed.
        assert!(ray_light_dir_changed([0.0; 3], dir));
    }

    #[test]
    fn cascade_budget_caps_renders_and_always_serves_cascade_zero() {
        let mut ages = [0u32; MAX_SHADOW_RAY_CASCADES];
        // All four windows moved; nothing is forced.
        let granted = schedule_cascade_renders([true; 4], [false; 4], &mut ages, 2);
        assert!(granted[0], "cascade 0 must never be deferred");
        assert_eq!(
            granted.iter().filter(|g| **g).count(),
            2,
            "budget of 2 must not render more than 2 cascades: {granted:?}"
        );
        // Deferred cascades age, served ones reset.
        assert_eq!(ages[0], 0);
        assert_eq!(ages.iter().filter(|age| **age == 1).count(), 2);
    }

    #[test]
    fn cascade_budget_ignores_the_cap_for_undeferrable_layers() {
        let mut ages = [0u32; MAX_SHADOW_RAY_CASCADES];
        // Casters moved / layers hold garbage depth: every layer must render
        // now, budget or not.
        let granted = schedule_cascade_renders([true; 4], [true; 4], &mut ages, 2);
        assert_eq!(granted, [true; 4]);
        assert_eq!(ages, [0; 4]);
    }

    #[test]
    fn cascade_budget_never_starves_a_cascade() {
        let mut ages = [0u32; MAX_SHADOW_RAY_CASCADES];
        // Worst case: every cascade wants a re-render every frame forever.
        let mut waited = [0u32; MAX_SHADOW_RAY_CASCADES];
        let mut worst = [0u32; MAX_SHADOW_RAY_CASCADES];
        let mut rendered = [0u32; MAX_SHADOW_RAY_CASCADES];
        for _ in 0..64 {
            let granted = schedule_cascade_renders([true; 4], [false; 4], &mut ages, 2);
            for cascade in 0..MAX_SHADOW_RAY_CASCADES {
                if granted[cascade] {
                    rendered[cascade] += 1;
                    waited[cascade] = 0;
                } else {
                    waited[cascade] += 1;
                    worst[cascade] = worst[cascade].max(waited[cascade]);
                }
            }
        }
        println!("cascade budget saturation: renders={rendered:?} worst wait={worst:?}");
        for cascade in 0..MAX_SHADOW_RAY_CASCADES {
            assert!(
                rendered[cascade] > 0,
                "cascade {cascade} never rendered in 64 saturated frames"
            );
            assert!(
                worst[cascade] <= CASCADE_MAX_DEFER_FRAMES,
                "cascade {cascade} waited {} frames, past the {CASCADE_MAX_DEFER_FRAMES}-frame cap",
                worst[cascade]
            );
        }
        assert_eq!(worst[0], 0, "cascade 0 must render every frame it wants to");
    }

    #[test]
    fn shadow_layer_cache_decision_matches_invalidation_rules() {
        // Static scene + static light + already valid: skip re-render.
        assert!(!shadow_layer_needs_render(false, false, true));
        // Casters moved: must re-render even if the light view is unchanged.
        assert!(shadow_layer_needs_render(true, false, true));
        // Light view changed: must re-render.
        assert!(shadow_layer_needs_render(false, true, true));
        // Newly enabled layer (was invalid): must render once.
        assert!(shadow_layer_needs_render(false, false, false));
    }

    #[test]
    fn ray_shadow_matrix_changes_with_light_dir_and_keeps_splits_stable() {
        let batches = [caster_batch()];
        let instances = [identity_instance()];
        let setup_a = build_shadow_setup(ShadowSetupArgs {
            camera: &camera(Quat::IDENTITY),
            lighting: &lighting_with_ray([-0.5, -1.0, -0.2]),
            draw_batches: &batches,
            staged_instances: &instances,
            fallback_focus_center: Vec3::ZERO,
            fallback_focus_radius: 64.0,
            viewport_width: 1280,
            viewport_height: 720,
            shadow_map_size: SHADOW_MAP_SIZE,
            has_casters: true,
            scale_budget_to_target: false,
        });
        let setup_yaw = build_shadow_setup(ShadowSetupArgs {
            camera: &camera(Quat::from_rotation_y(1.0)),
            lighting: &lighting_with_ray([-0.5, -1.0, -0.2]),
            draw_batches: &batches,
            staged_instances: &instances,
            fallback_focus_center: Vec3::ZERO,
            fallback_focus_radius: 64.0,
            viewport_width: 1280,
            viewport_height: 720,
            shadow_map_size: SHADOW_MAP_SIZE,
            has_casters: true,
            scale_budget_to_target: false,
        });
        let setup_b = build_shadow_setup(ShadowSetupArgs {
            camera: &camera(Quat::IDENTITY),
            lighting: &lighting_with_ray([0.3, -1.0, -0.7]),
            draw_batches: &batches,
            staged_instances: &instances,
            fallback_focus_center: Vec3::ZERO,
            fallback_focus_radius: 64.0,
            viewport_width: 1280,
            viewport_height: 720,
            shadow_map_size: SHADOW_MAP_SIZE,
            has_casters: true,
            scale_budget_to_target: false,
        });
        assert_eq!(setup_a.uniform.ray_splits, setup_yaw.uniform.ray_splits);
        assert_ne!(
            setup_a.uniform.ray_light_view_proj[0],
            setup_b.uniform.ray_light_view_proj[0]
        );
    }

    #[test]
    fn ray_shadow_cascades_are_enabled_and_monotonic() {
        let batches = [caster_batch()];
        let instances = [identity_instance()];
        let setup = build_shadow_setup(ShadowSetupArgs {
            camera: &camera(Quat::IDENTITY),
            lighting: &lighting_with_ray([-0.5, -1.0, -0.2]),
            draw_batches: &batches,
            staged_instances: &instances,
            fallback_focus_center: Vec3::ZERO,
            fallback_focus_radius: 64.0,
            viewport_width: 1280,
            viewport_height: 720,
            shadow_map_size: SHADOW_MAP_SIZE,
            has_casters: true,
            scale_budget_to_target: false,
        });
        assert!(setup.ray_enabled);
        assert_eq!(setup.uniform.ray_params[1], MAX_SHADOW_RAY_CASCADES as f32);
        assert!(setup.uniform.ray_splits[0] > 0.0);
        assert!(setup.uniform.ray_splits[0] < setup.uniform.ray_splits[1]);
        assert!(setup.uniform.ray_splits[1] < setup.uniform.ray_splits[2]);
        assert!(setup.uniform.ray_splits[2] < setup.uniform.ray_splits[3]);
        for matrix in setup.uniform.ray_light_view_proj {
            assert_ne!(matrix, [[0.0; 4]; 4]);
        }
    }

    #[test]
    fn huge_caster_bounds_keep_cascade_texels_tight_and_monotonic() {
        // One scene-spanning caster (e.g. a 500-unit water plane) must not
        // blow every cascade up to the whole scene: each cascade stays fit to
        // its own camera slice, so cascade 0 keeps contact-shadow resolution.
        let mut huge = caster_batch();
        huge.local_radius = 500.0;
        let batches = [huge];
        let instances = [identity_instance()];
        let setup = build_shadow_setup(ShadowSetupArgs {
            camera: &camera(Quat::IDENTITY),
            lighting: &lighting_with_ray([-0.5, -1.0, -0.2]),
            draw_batches: &batches,
            staged_instances: &instances,
            fallback_focus_center: Vec3::ZERO,
            fallback_focus_radius: 64.0,
            viewport_width: 1280,
            viewport_height: 720,
            shadow_map_size: SHADOW_MAP_SIZE,
            has_casters: true,
            scale_budget_to_target: false,
        });
        assert!(setup.ray_enabled);
        assert!(
            setup.uniform.ray_texel[0] < 0.1,
            "cascade 0 texel {} must stay fine-grained",
            setup.uniform.ray_texel[0]
        );
        for cascade in 1..MAX_SHADOW_RAY_CASCADES {
            assert!(
                setup.uniform.ray_texel[cascade] >= setup.uniform.ray_texel[cascade - 1],
                "texels must grow with cascade distance: {:?}",
                setup.uniform.ray_texel
            );
        }
    }

    #[test]
    fn shadow_tuning_uses_first_active_caster() {
        let batches = [caster_batch()];
        let instances = [identity_instance()];
        let mut lighting = lighting_with_ray([-0.5, -1.0, -0.2]);
        if let Some(light) = lighting.ray_lights[0].as_mut() {
            light.shadow_strength = 0.5;
            light.shadow_depth_bias = 0.001;
            light.shadow_normal_bias = 0.1;
        }
        let setup = build_shadow_setup(ShadowSetupArgs {
            camera: &camera(Quat::IDENTITY),
            lighting: &lighting,
            draw_batches: &batches,
            staged_instances: &instances,
            fallback_focus_center: Vec3::ZERO,
            fallback_focus_radius: 64.0,
            viewport_width: 1280,
            viewport_height: 720,
            shadow_map_size: SHADOW_MAP_SIZE,
            has_casters: true,
            scale_budget_to_target: false,
        });
        assert_eq!(setup.uniform.params0, [1.0, 0.5, 0.001, 0.1]);
    }

    #[test]
    fn spot_and_point_shadow_slots_follow_cast_shadow_flags() {
        let batches = [caster_batch()];
        let instances = [identity_instance()];
        let mut lighting = Lighting3DState::default();
        lighting.spot_lights[0] = Some(SpotLight3DState {
            position: [0.0, 4.0, 0.0],
            direction: [0.0, -1.0, 0.0],
            color: [1.0, 1.0, 1.0],
            intensity: 1.0,
            range: 12.0,
            inner_angle_radians: 0.25,
            outer_angle_radians: 0.5,
            cast_shadows: true,
            shadow_strength: 0.82,
            shadow_depth_bias: 0.00018,
            shadow_normal_bias: 0.045,
        });
        lighting.spot_lights[1] = Some(SpotLight3DState {
            cast_shadows: false,
            ..lighting.spot_lights[0].expect("required value must be present")
        });
        lighting.point_lights[0] = Some(PointLight3DState {
            position: [2.0, 3.0, 4.0],
            color: [1.0, 1.0, 1.0],
            intensity: 1.0,
            range: 10.0,
            cast_shadows: true,
            shadow_strength: 0.82,
            shadow_depth_bias: 0.00018,
            shadow_normal_bias: 0.045,
        });
        let setup = build_shadow_setup(ShadowSetupArgs {
            camera: &camera(Quat::IDENTITY),
            lighting: &lighting,
            draw_batches: &batches,
            staged_instances: &instances,
            fallback_focus_center: Vec3::ZERO,
            fallback_focus_radius: 64.0,
            viewport_width: 1280,
            viewport_height: 720,
            shadow_map_size: SHADOW_MAP_SIZE,
            has_casters: true,
            scale_budget_to_target: false,
        });
        assert_eq!(setup.spot_count, 1);
        assert_eq!(setup.point_count, 1);
        assert_eq!(setup.uniform.spot_params[0][1], 0.0);
        assert_eq!(setup.uniform.point_params[0][2], 0.0);
        for face in 0..POINT_SHADOW_FACE_COUNT {
            assert_ne!(
                setup.uniform.point_light_view_proj[face], [[0.0; 4]; 4],
                "point face {face} must have matrix"
            );
        }
    }
    /// Sun directions the cascade fit has to hold up under. The grazing pair is
    /// a ~5-degree sunrise/sunset in a time-of-day cycle: the camera slice's
    /// light-plane footprint is at its most elongated there, so it is where a
    /// window that fit an AABB (instead of the slice's bounding sphere) or a
    /// caster-box clamp that cut the footprint would start missing coverage.
    fn sun_dirs() -> [(&'static str, [f32; 3]); 4] {
        let grazing = 5.0f32.to_radians();
        [
            ("default", [-0.5, -1.0, -0.2]),
            ("near-vertical", [-0.05, -1.0, 0.02]),
            ("grazing +x", [-grazing.cos(), -grazing.sin(), 0.0]),
            (
                "grazing diagonal",
                [-0.70 * grazing.cos(), -grazing.sin(), -0.71 * grazing.cos()],
            ),
        ]
    }

    /// The stable (bounding-sphere) window must still cover every cascade's
    /// camera slice, at any camera orientation AND any sun elevation --
    /// otherwise receivers near a slice edge sample outside the map and read as
    /// fully lit.
    #[test]
    fn every_cascade_window_covers_its_camera_slice() {
        let mut ground = caster_batch();
        ground.local_radius = 400.0;
        let batches = [ground];
        let instances = [identity_instance()];
        for (sun_name, sun) in sun_dirs() {
            for (yaw, pitch) in [(0.0, 0.0), (0.9, 0.3), (2.4, -0.6), (4.1, 0.15)] {
                let view = camera(Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch));
                let setup = build_shadow_setup(ShadowSetupArgs {
                    camera: &view,
                    lighting: &lighting_with_ray(sun),
                    draw_batches: &batches,
                    staged_instances: &instances,
                    fallback_focus_center: Vec3::ZERO,
                    fallback_focus_radius: 64.0,
                    viewport_width: 1280,
                    viewport_height: 720,
                    shadow_map_size: SHADOW_MAP_SIZE,
                    has_casters: true,
                    scale_budget_to_target: false,
                });
                let splits = setup.uniform.ray_splits;
                for cascade in 0..MAX_SHADOW_RAY_CASCADES {
                    let corners = camera_frustum_slice_corners_world(
                        &view,
                        1280,
                        720,
                        if cascade == 0 {
                            0.0
                        } else {
                            splits[cascade - 1]
                        },
                        splits[cascade],
                    )
                    .expect("slice corners must be finite");
                    let light =
                        Mat4::from_cols_array_2d(&setup.uniform.ray_light_view_proj[cascade]);
                    for corner in corners {
                        let clip = light * corner.extend(1.0);
                        assert!(
                            clip.x.abs() <= 1.0 && clip.y.abs() <= 1.0,
                            "cascade {cascade} window misses slice corner {corner} \
                             (clip {clip}) at yaw {yaw} pitch {pitch} under the {sun_name} sun"
                        );
                    }
                }
            }
        }
    }

    /// World-space depth range one cascade's ortho window spans, recovered from
    /// its view-proj: NDC z is a linear function of world position, so the
    /// length of its world-space gradient is `1 / depth_range`.
    fn cascade_depth_range(view_proj: &[[f32; 4]; 4]) -> f32 {
        let m = Mat4::from_cols_array_2d(view_proj);
        let gradient = Vec3::new(m.x_axis.z, m.y_axis.z, m.z_axis.z);
        1.0 / gradient.length().max(1.0e-9)
    }

    /// The cascade window is fitted to the slice's bounding SPHERE (and clamped
    /// only by the world-static caster box), both of which are independent of
    /// where the sun is. So a sunrise must not resize the window or coarsen the
    /// texel relative to noon -- if it did, a time-of-day cycle would pump the
    /// shadow resolution as the sun sank.
    ///
    /// Also reports the grazing-angle depth-bias headroom, which is a separate
    /// (pre-existing, shading-side) matter: at a 5-degree sun the receiver
    /// surface is nearly parallel to the light, so the depth offset needed to
    /// clear self-shadowing is `texel / tan(elevation)` ~ 11x the texel, while
    /// the constant NDC bias buys `params0.z * depth_range` world units.
    #[test]
    fn grazing_sun_keeps_the_cascade_window_size() {
        let mut ground = caster_batch();
        ground.local_radius = 400.0;
        let batches = [ground];
        let instances = [identity_instance()];
        let view = camera(Quat::IDENTITY);
        let fit = |sun: [f32; 3]| {
            build_shadow_setup(ShadowSetupArgs {
                camera: &view,
                lighting: &lighting_with_ray(sun),
                draw_batches: &batches,
                staged_instances: &instances,
                fallback_focus_center: Vec3::ZERO,
                fallback_focus_radius: 64.0,
                viewport_width: 1280,
                viewport_height: 720,
                shadow_map_size: SHADOW_MAP_SIZE,
                has_casters: true,
                scale_budget_to_target: false,
            })
        };
        let high = fit([-0.05, -1.0, 0.02]);
        for (sun_name, sun) in sun_dirs() {
            let setup = fit(sun);
            let elevation = {
                let dir = Vec3::from(setup.ray_light_dir);
                (-dir.y).asin()
            };
            let depths: Vec<f32> = (0..MAX_SHADOW_RAY_CASCADES)
                .map(|c| cascade_depth_range(&setup.uniform.ray_light_view_proj[c]))
                .collect();
            let bias_world: Vec<f32> = depths
                .iter()
                .map(|d| d * setup.uniform.params0[2])
                .collect();
            let bias_needed: Vec<f32> = (0..MAX_SHADOW_RAY_CASCADES)
                .map(|c| setup.uniform.ray_texel[c] / elevation.tan().max(1.0e-4))
                .collect();
            println!(
                "{sun_name} sun ({:.2} deg above horizon): texel={:?} depth_range={depths:?} \
                 bias world units available={bias_world:?} needed at this incidence={bias_needed:?}",
                elevation.to_degrees(),
                setup.uniform.ray_texel
            );
            for cascade in 0..MAX_SHADOW_RAY_CASCADES {
                let ratio =
                    setup.uniform.ray_texel[cascade] / high.uniform.ray_texel[cascade].max(1.0e-9);
                assert!(
                    (ratio - 1.0).abs() < 1.0e-4,
                    "{sun_name} sun resized cascade {cascade}'s window {ratio}x vs a high sun: \
                     the fit must not depend on sun elevation"
                );
            }
        }
    }

    /// Stable cascaded shadow maps: as the camera moves, a cascade's window
    /// must advance in WHOLE shadow texels, so a static receiver keeps sampling
    /// the same texel centres. When it slides continuously instead, every
    /// shadow edge in the scene crawls/shimmers while the camera moves -- the
    /// classic "shadows look weird when I move" artifact.
    #[test]
    fn cascade_window_advances_in_whole_texels_under_camera_motion() {
        let mut ground = caster_batch();
        ground.local_radius = 60.0;
        let batches = [ground];
        let instances = [identity_instance()];
        let mut prev: Option<(f32, f32, f32)> = None;
        for step in 0..12 {
            let mut moving = camera(Quat::IDENTITY);
            // 1 mm per frame: far below one shadow texel at any cascade.
            moving.position[0] = step as f32 * 0.001;
            let setup = build_shadow_setup(ShadowSetupArgs {
                camera: &moving,
                lighting: &lighting_with_ray([-0.5, -1.0, -0.2]),
                draw_batches: &batches,
                staged_instances: &instances,
                fallback_focus_center: Vec3::ZERO,
                fallback_focus_radius: 64.0,
                viewport_width: 1280,
                viewport_height: 720,
                shadow_map_size: SHADOW_MAP_SIZE,
                has_casters: true,
                scale_budget_to_target: false,
            });
            // Where a fixed world point lands in cascade 0's shadow map.
            let view_proj = Mat4::from_cols_array_2d(&setup.uniform.ray_light_view_proj[0]);
            let clip = view_proj * Vec3::ZERO.extend(1.0);
            let map = SHADOW_MAP_SIZE as f32;
            let u = (clip.x * 0.5 + 0.5) * map;
            let v = (clip.y * 0.5 + 0.5) * map;
            let texel = setup.uniform.ray_texel[0];
            if let Some((prev_u, prev_v, prev_texel)) = prev {
                for (axis, delta) in [("u", u - prev_u), ("v", v - prev_v)] {
                    let off_grid = (delta - delta.round()).abs();
                    assert!(
                        off_grid < 0.01,
                        "cascade window slid {delta} texels on {axis} (step {step}):                          a sub-texel camera move must shift it by a whole texel or not at all"
                    );
                }
                assert!(
                    (texel - prev_texel).abs() <= prev_texel * 1.0e-4,
                    "cascade texel size must not resize with camera motion                      ({prev_texel} -> {texel})"
                );
            }
            prev = Some((u, v, texel));
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/three_d_shadow_memo_tests.rs"]
mod shadow_memo_tests;

#[cfg(test)]
mod local_light_budget_tests {
    use super::{
        MAX_SHADOW_POINT_LIGHTS, MAX_SHADOW_SPOT_LIGHTS, local_shadow_light_budget,
        local_shadow_light_score, local_shadow_update_budget, schedule_local_shadow_renders,
        take_best_shadow_lights,
    };
    use glam::Vec3;

    /// The main view keeps the compiled budget at any window at or above the
    /// reference height.
    #[test]
    fn reference_and_larger_targets_keep_the_full_budget() {
        for height in [1080, 1440, 2160] {
            assert_eq!(
                local_shadow_light_budget(MAX_SHADOW_POINT_LIGHTS, height),
                MAX_SHADOW_POINT_LIGHTS,
                "height {height}"
            );
        }
    }

    /// Sub-view targets: a point light is 6 depth passes, so a 256x144 stream
    /// drops from 4 shadowed point lights (24 passes) to 1 (6).
    #[test]
    fn stream_targets_scale_the_budget_down() {
        assert_eq!(local_shadow_light_budget(4, 540), 2);
        assert_eq!(local_shadow_light_budget(4, 288), 2);
        assert_eq!(local_shadow_light_budget(4, 144), 1);
        assert_eq!(local_shadow_light_budget(MAX_SHADOW_SPOT_LIGHTS, 144), 1);
    }

    /// Never zero: a view with shadow-casting lights keeps shadows, however
    /// small. Zero max stays zero.
    #[test]
    fn budget_never_drops_below_one_light() {
        for height in [1, 8, 64, 200] {
            assert_eq!(
                local_shadow_light_budget(4, height).max(1),
                local_shadow_light_budget(4, height)
            );
            assert!(local_shadow_light_budget(4, height) >= 1);
        }
        assert_eq!(local_shadow_light_budget(0, 1080), 0);
    }

    /// A big near light outranks a small far one, and a light the camera sits
    /// inside ranks at the ceiling.
    #[test]
    fn score_ranks_by_angular_size() {
        let camera = Vec3::ZERO;
        let near_big = local_shadow_light_score([0.0, 0.0, 20.0], 15.0, 5.0, camera);
        let far_small = local_shadow_light_score([0.0, 0.0, 200.0], 4.0, 5.0, camera);
        assert!(near_big > far_small);
        let inside = local_shadow_light_score([0.0, 0.0, 1.0], 30.0, 1.0, camera);
        assert!(inside > near_big);
    }

    /// The kept set comes back in light order so a stable set reproduces stable
    /// shadow slots -- which is what keeps the per-layer depth cache valid.
    #[test]
    fn kept_lights_return_in_light_order() {
        let kept = take_best_shadow_lights(vec![(5, 1.0), (0, 9.0), (3, 5.0), (7, 0.1)], 3);
        assert_eq!(kept, vec![0, 3, 5]);
        assert_eq!(
            take_best_shadow_lights(vec![(2, 1.0)], 0),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn update_budget_holds_thirty_hz_at_common_frame_rates() {
        assert_eq!(local_shadow_update_budget(0, 1.0 / 144.0), 0);
        assert_eq!(local_shadow_update_budget(4, 1.0 / 144.0), 1);
        assert_eq!(local_shadow_update_budget(4, 1.0 / 60.0), 2);
        assert_eq!(local_shadow_update_budget(4, 1.0 / 30.0), 4);
        assert_eq!(local_shadow_update_budget(4, 1.0 / 15.0), 4);
        assert_eq!(local_shadow_update_budget(4, 0.0), 4);
    }

    #[test]
    fn update_scheduler_rotates_without_starvation() {
        let mut ages = [0; 4];
        let mut grants = [0u32; 4];
        for _ in 0..8 {
            let frame = schedule_local_shadow_renders([true; 4], [false; 4], &mut ages, 1);
            assert_eq!(frame.iter().filter(|granted| **granted).count(), 1);
            for (count, granted) in grants.iter_mut().zip(frame) {
                *count += u32::from(granted);
            }
        }
        assert_eq!(grants, [2; 4]);
        assert!(ages.into_iter().max().unwrap_or(0) <= 3);
    }

    #[test]
    fn forced_update_bypasses_zero_budget() {
        let mut ages = [0; 4];
        assert_eq!(
            schedule_local_shadow_renders(
                [true, true, false, false],
                [false, true, false, false],
                &mut ages,
                0,
            ),
            [false, true, false, false]
        );
    }
}

#[cfg(test)]
mod shadow_layer_set_tests {
    use super::{
        MAX_SHADOW_POINT_LIGHTS, MAX_SHADOW_RAY_CASCADES, MAX_SHADOW_RAY_LIGHTS,
        MAX_SHADOW_SPOT_LIGHTS, POINT_SHADOW_FACE_COUNT, SHADOW_CAMERA_COUNT, ShadowAtlas,
        shadow_layer_sets, shadow_set_needs_render,
    };

    /// Sets are the natural light groupings, and every layer of the flat
    /// `SHADOW_CAMERA_COUNT` space belongs to exactly one of them.
    #[test]
    fn layer_sets_cover_the_flat_layer_space_without_overlap() {
        let sets = shadow_layer_sets(
            MAX_SHADOW_RAY_CASCADES,
            MAX_SHADOW_SPOT_LIGHTS,
            MAX_SHADOW_POINT_LIGHTS,
        );
        let mut seen = vec![false; SHADOW_CAMERA_COUNT];
        for set in sets.into_iter().flatten() {
            for layer in 0..set.count {
                let flat = set.flat_base + layer;
                assert!(!seen[flat], "layer {flat} claimed by two sets");
                seen[flat] = true;
            }
        }
        assert!(seen.into_iter().all(|hit| hit));
    }

    /// A point light's whole cube is one set (a face may never show a different
    /// pose than its neighbours), and inactive lights produce no set at all.
    #[test]
    fn layer_sets_follow_the_active_light_counts() {
        let sets = shadow_layer_sets(MAX_SHADOW_RAY_CASCADES, 1, 1);
        let live: Vec<_> = sets.into_iter().flatten().collect();
        assert_eq!(live.len(), 3);
        assert_eq!(live[0].count, MAX_SHADOW_RAY_CASCADES);
        assert_eq!(live[0].atlas, ShadowAtlas::Ray);
        assert_eq!(live[1].count, 1);
        assert_eq!(
            live[1].flat_base,
            MAX_SHADOW_RAY_LIGHTS * MAX_SHADOW_RAY_CASCADES
        );
        assert_eq!(live[2].count, POINT_SHADOW_FACE_COUNT);
        assert_eq!(live[2].atlas_base, 0);
        assert!(
            shadow_layer_sets(0, 0, 0)
                .into_iter()
                .all(|set| set.is_none())
        );
    }

    /// The second point light's faces start after the first light's six, in
    /// both the flat space and its own atlas.
    #[test]
    fn point_light_sets_are_contiguous_runs_per_light() {
        let sets = shadow_layer_sets(0, 0, 2);
        let live: Vec<_> = sets.into_iter().flatten().collect();
        assert_eq!(live[0].atlas_base, 0);
        assert_eq!(live[1].atlas_base, POINT_SHADOW_FACE_COUNT);
        assert_eq!(
            live[1].flat_base - live[0].flat_base,
            POINT_SHADOW_FACE_COUNT
        );
    }

    /// The cache that makes static scenes cheap survives: a set whose layers are
    /// all still valid encodes nothing at all.
    #[test]
    fn fully_cached_set_renders_nothing() {
        assert!(!shadow_set_needs_render(&[true; 6], &[false; 6]));
    }

    /// ... and so does the empty-layer skip: a set whose only stale layers can
    /// all be skipped as already-clear stays out of the encoder.
    #[test]
    fn set_of_skippable_empty_layers_renders_nothing() {
        assert!(!shadow_set_needs_render(
            &[true, false, false, true],
            &[false, true, true, false],
        ));
    }

    /// One layer with real work pulls the whole set in -- a multiview pass
    /// clears every layer in its mask, so it cannot render a subset.
    #[test]
    fn one_stale_layer_renders_the_whole_set() {
        assert!(shadow_set_needs_render(
            &[true, true, false, true],
            &[false, false, false, false],
        ));
        assert!(shadow_set_needs_render(&[false; 6], &[false; 6]));
    }
}
