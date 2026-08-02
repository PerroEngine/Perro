use super::stream_3d::finish_stream_lighting_3d;
use super::*;
use crate::runtime::world_state::AutoResolutionState;

/// Bucket 4 the long axis of an auto resolution, in target px. Rounds UP, so
/// the target is never smaller than the exact supersampled rect.
const AUTO_RESOLUTION_BUCKET: u32 = 64;
/// Refreshes a smaller size must hold b4 the target shrinks. Growth lands
/// same-refresh; only shrink waits.
const AUTO_RESOLUTION_SHRINK_HOLD: u32 = 60;
/// Shrink >= this many buckets = real layout chg, not animation wobble: skip
/// the hold + shrink now.
const AUTO_RESOLUTION_SHRINK_JUMP: u32 = 2;
/// Max relative aspect drift a held target may carry. The UI composite fits the
/// TARGET aspect into the rect (`camera_stream_aspect_ratio`), so a held pair
/// whose aspect no longer matches the rect letterboxes: past this, re-emit.
const AUTO_RESOLUTION_ASPECT_DRIFT: f32 = 0.005;

/// Exact px -> bucket, rounded UP + clamped to the target limit.
fn auto_resolution_bucket(exact: f32) -> u32 {
    // NaN casts to 0; `max(1)` keeps the result a real bucket.
    let exact = (exact.ceil().clamp(1.0, 8192.0) as u32).max(1);
    exact
        .div_ceil(AUTO_RESOLUTION_BUCKET)
        .saturating_mul(AUTO_RESOLUTION_BUCKET)
        .clamp(1, 8192)
}

/// Wanted target 4 an auto rect size: long axis snaps up to a bucket, short
/// axis follows the exact aspect. Bucketing both axes independently would skew
/// the target aspect (a 100x60 rect -> 256x128), which the UI composite turns
/// into letterbox bars, so only the long axis quantizes.
///
/// `scale` = THE project supersample factor (`perro_structs::supersample_scale`
/// via [`Runtime::auto_resolution_scale`]), the same number the UI raster
/// target multiplies by. Never a local constant here: equal factors are what
/// make this target land 1:1 with its footprint in the UI raster target instead
/// of stacking a 2nd supersample. Display DPI never reaches the runtime
/// (`ui_pixel_snap_scale_factor` is a hardcoded 1.0), so no
/// `min(scale, scale_factor)` term.
fn auto_resolution_target(size: [f32; 2], scale: f32) -> [u32; 2] {
    let exact = [(size[0] * scale).max(1.0), (size[1] * scale).max(1.0)];
    let long = usize::from(exact[1] > exact[0]);
    let short = 1 - long;
    let long_px = auto_resolution_bucket(exact[long]);
    let ratio = exact[short] / exact[long];
    // inf/inf + inf/x are NaN: fall back to square rather than a 1px axis.
    let ratio = if ratio.is_finite() {
        ratio.clamp(1.0e-4, 1.0)
    } else {
        1.0
    };
    let short_px = ((long_px as f32 * ratio).round() as u32).clamp(1, 8192);
    let mut out = [0u32; 2];
    out[long] = long_px;
    out[short] = short_px;
    out
}

/// How much a sub-view target oversamples its own on-screen rect, as ONE
/// uniform factor.
///
/// Uniform, not per-axis: nested rects resolve against the owner's aspect-FIT
/// basis (the biggest rect-aspect box inside the target), whose scale is the
/// min over axes. A per-axis factor would skew the nested target's aspect and
/// letterbox it whenever the owner's target aspect != its rect aspect.
///
/// Capped at `scale` (THE project supersample factor) so a nested target never
/// lands BELOW the footprint it occupies in the owner target. Past that the
/// owner already supersamples harder than the auto rule asks for, and the
/// owner's own downsample carries the quality; dropping under 1:1 there would
/// be visibly softer than the surface it composites into.
///
/// Degenerate rects (0 / NaN / sub-px) carry no usable ratio -> 1.0 (no
/// divide), never an inf that would collapse a nested target to 1px.
fn sub_view_supersample_factor(target: [u32; 2], logical: [f32; 2], scale: f32) -> f32 {
    let scale = scale.max(1.0);
    let axis = |target_px: u32, logical_px: f32| {
        if logical_px >= 1.0 {
            let factor = target_px as f32 / logical_px;
            factor.is_finite().then_some(factor)
        } else {
            None
        }
    };
    match (axis(target[0], logical[0]), axis(target[1], logical[1])) {
        (Some(x), Some(y)) => x.min(y).clamp(1.0, scale),
        (Some(v), None) | (None, Some(v)) => v.clamp(1.0, scale),
        (None, None) => 1.0,
    }
}

/// Relative aspect difference btw two target sizes.
fn auto_resolution_aspect_drift(held: [u32; 2], want: [u32; 2]) -> f32 {
    let aspect = |size: [u32; 2]| size[0] as f32 / size[1].max(1) as f32;
    let want_aspect = aspect(want).max(1.0e-6);
    ((aspect(held) - want_aspect) / want_aspect).abs()
}

impl AutoResolutionState {
    fn emit(&mut self, want: [u32; 2]) -> [u32; 2] {
        self.emitted = want;
        self.shrink_streak = 0;
        want
    }

    /// Hysteresis: grow now, shrink only aft `AUTO_RESOLUTION_SHRINK_HOLD`
    /// consecutive smaller refreshes, a `AUTO_RESOLUTION_SHRINK_JUMP`-bucket
    /// drop, or an aspect drift the composite would show.
    fn resolve(&mut self, want: [u32; 2]) -> [u32; 2] {
        let held = self.emitted;
        if held[0] == 0 || held[1] == 0 || want[0] > held[0] || want[1] > held[1] {
            return self.emit(want);
        }
        if want == held {
            self.shrink_streak = 0;
            return held;
        }
        let jump = AUTO_RESOLUTION_SHRINK_JUMP * AUTO_RESOLUTION_BUCKET;
        if held[0] - want[0] >= jump
            || held[1] - want[1] >= jump
            || auto_resolution_aspect_drift(held, want) > AUTO_RESOLUTION_ASPECT_DRIFT
        {
            return self.emit(want);
        }
        self.shrink_streak += 1;
        if self.shrink_streak >= AUTO_RESOLUTION_SHRINK_HOLD {
            return self.emit(want);
        }
        held
    }
}

/// Output of one fused member collection: every lane a stream state carries.
/// Dimensions the stream did not request stay shared-empty / default.
struct StreamCollectedLanes {
    sprites_2d: Arc<[Sprite2DCommand]>,
    lights_2d: Arc<[Light2DState]>,
    point_particles_2d: Arc<[(NodeID, PointParticles2DState)]>,
    waters_2d: Arc<[(NodeID, Water2DState)]>,
    draws_3d: Arc<[CameraStreamDraw3DState]>,
    lighting_3d: CameraStreamLighting3DState,
    point_particles_3d: Arc<[(NodeID, PointParticles3DState)]>,
    waters_3d: Arc<[(NodeID, Water3DState)]>,
}

impl Default for StreamCollectedLanes {
    fn default() -> Self {
        Self {
            sprites_2d: empty_arc_slice(),
            lights_2d: empty_arc_slice(),
            point_particles_2d: empty_arc_slice(),
            waters_2d: empty_arc_slice(),
            draws_3d: empty_arc_slice(),
            lighting_3d: CameraStreamLighting3DState::default(),
            point_particles_3d: empty_arc_slice(),
            waters_3d: empty_arc_slice(),
        }
    }
}

impl Runtime {
    /// THE supersample factor an auto (0) sub-view resolution multiplies by.
    ///
    /// # Single-supersample invariant
    ///
    /// For any render target `T`: `effective_supersample(T) == own_factor(T)`.
    /// `own_factor(T)` is derived from `T`'s OWN on-screen size (or the
    /// resolution its author pinned) and never from its nesting depth or from
    /// any ancestor's factor. Two targets in a parent/child chain each
    /// supersample exactly once; the chain does not multiply.
    ///
    /// Enforced in three places:
    /// - here: the factor is ONE shared number
    ///   (`perro_structs::supersample_scale`), so the UI raster target and an
    ///   auto sub-view target can never drift apart and stack;
    /// - `prepare_nested_sub_views`: a nested rect arrives in OWNER-TARGET px,
    ///   so the owner's oversample is divided back out before this factor is
    ///   applied;
    /// - `sub_view_supersample_factor`: the divisor is capped at this factor,
    ///   so a nested target never lands below its footprint in the owner.
    ///
    /// Camera streams (`CameraStream2D` / `CameraStream3D` / `UiCameraStream`)
    /// never derive a size from an on-screen rect: their resolution is the
    /// author's, so their own factor is 1.0 by construction and they cannot
    /// stack from either side of a nesting chain.
    pub(crate) fn auto_resolution_scale(&self) -> f32 {
        perro_structs::supersample_scale_f32(
            self.project()
                .map(|project| project.config.texture_filter)
                .unwrap_or_default(),
        )
    }

    /// Scratch maps come from the caller (cleared here) so per-node calls in
    /// hot rebuild loops reuse capacity instead of allocating fresh containers.
    pub(super) fn nested_ui_sub_view_rect(
        &self,
        owner: NodeID,
        node: NodeID,
        owner_resolution: [u32; 2],
        computed: &mut AHashMap<NodeID, ComputedUiRect>,
        scales: &mut AHashMap<NodeID, Vector2>,
        auto: &mut AHashSet<NodeID>,
    ) -> Option<ComputedUiRect> {
        computed.clear();
        scales.clear();
        auto.clear();
        let root = ComputedUiRect::new(
            Vector2::ZERO,
            Vector2::new(owner_resolution[0] as f32, owner_resolution[1] as f32),
        );
        computed.insert(owner, root);
        scales.insert(owner, Vector2::ONE);
        self.compute_ui_rect(node, root, computed, scales, auto)
    }

    /// `owner_supersample` = owner target px / owner on-screen logical px,
    /// uniform + capped (see `sub_view_supersample_factor`). Nested rects come
    /// out of `nested_ui_sub_view_rect` in the OWNER
    /// TARGET's pixel space, which already carries the owner's supersample.
    /// Feeding that straight to `auto_resolution_target` would apply the
    /// project supersample factor a 2nd time and compound per nesting level (4x
    /// px @ depth 1, 16x @ depth 2). Divide it back out so each level
    /// supersamples exactly once against its true on-screen size. This is the
    /// nesting half of the invariant on `Runtime::auto_resolution_scale`.
    fn prepare_nested_sub_views(
        &mut self,
        owner: NodeID,
        owner_resolution: [u32; 2],
        owner_supersample: f32,
    ) {
        let mut members = self.render_ui.acquire_node_vec();
        self.fill_world_members(owner, &mut members);
        // take-pattern scratch: recursion (views nested in views) takes empty
        // maps, the common depth-1 case reuses persisted capacity.
        let mut computed = std::mem::take(&mut self.render_ui.nested_rect_computed_scratch);
        let mut scales = std::mem::take(&mut self.render_ui.nested_rect_scales_scratch);
        let mut auto = std::mem::take(&mut self.render_ui.nested_rect_auto_scratch);
        for &node in members.iter() {
            let Some(scene_node) = self.nodes.get(node) else {
                continue;
            };
            let (view, auto_size) = match &scene_node.data {
                SceneNodeData::UiSubView(view) => {
                    let view = SubView::from(view.as_ref());
                    let auto_size = self
                        .nested_ui_sub_view_rect(
                            owner,
                            node,
                            owner_resolution,
                            &mut computed,
                            &mut scales,
                            &mut auto,
                        )
                        // owner-target px -> on-screen logical px.
                        .map(|rect| {
                            [
                                rect.size.x / owner_supersample,
                                rect.size.y / owner_supersample,
                            ]
                        })
                        // Fallback rects come from the MAIN ui layout pass, which
                        // is already logical screen space: no divide.
                        .or_else(|| {
                            self.render_ui
                                .retained_rects
                                .get(&node)
                                .map(|rect| rect.size)
                        });
                    (view, auto_size)
                }
                SceneNodeData::SubView2D(view) => (view.sub_view.clone(), None),
                SceneNodeData::SubView3D(view) => (view.sub_view.clone(), None),
                _ => continue,
            };
            if !self.is_effectively_visible(node) {
                self.queue_camera_stream_remove(node);
                continue;
            }
            if let Some(state) = self.sub_view_state(node, &view, auto_size) {
                self.queue_camera_stream_upsert(node, Arc::new(state));
            } else {
                self.queue_camera_stream_remove(node);
            }
        }
        computed.clear();
        scales.clear();
        auto.clear();
        self.render_ui.nested_rect_computed_scratch = computed;
        self.render_ui.nested_rect_scales_scratch = scales;
        self.render_ui.nested_rect_auto_scratch = auto;
        self.render_ui.release_node_vec(members);
    }

    /// One member walk picks both explicit sub-view cameras (was 2 walks):
    /// last active `Camera2D` wins, highest activation-order `Camera3D` wins.
    fn sub_view_cameras(
        &mut self,
        localize_2d: &StreamLocalize2D,
        localize_3d: &StreamLocalize3D,
    ) -> (Option<Camera2DState>, Option<Camera3DState>) {
        let mut found_2d = None;
        let mut found_3d_priority: Option<(u64, u32, u32)> = None;
        let mut found_3d = None;
        for idx in 0..self.camera_stream_node_scratch.len() {
            let node = self.camera_stream_node_scratch[idx];
            let Some(scene_node) = self.nodes.get(node) else {
                continue;
            };
            match &scene_node.data {
                SceneNodeData::Camera2D(camera) => {
                    if !camera.active || !self.is_effectively_visible(node) {
                        continue;
                    }
                    found_2d = Some((
                        node,
                        camera.transform,
                        camera.zoom,
                        camera.render_mask,
                        camera.post_processing.clone(),
                        camera.audio_options.clone(),
                    ));
                }
                SceneNodeData::Camera3D(camera) => {
                    if !camera.active || !self.is_effectively_visible(node) {
                        continue;
                    }
                    let order = self
                        .render_3d
                        .camera_activation_order
                        .get(&node)
                        .copied()
                        .unwrap_or(0);
                    let priority = (order, node.generation(), node.index());
                    let replace = found_3d_priority
                        .map(|current| priority > current)
                        .unwrap_or(true);
                    if replace {
                        found_3d_priority = Some(priority);
                        found_3d = Some((
                            node,
                            camera.transform,
                            camera.projection.clone(),
                            camera.render_mask,
                            camera.post_processing.clone(),
                            camera.audio_options.clone(),
                        ));
                    }
                }
                _ => {}
            }
        }
        let camera_2d = found_2d.map(
            |(node, local_transform, zoom, render_mask, post_processing, audio_options)| {
                let transform = self
                    .stream_localized_transform_2d(node, localize_2d)
                    .unwrap_or(local_transform);
                Camera2DState {
                    position: [transform.position.x, transform.position.y],
                    rotation_radians: transform.rotation,
                    zoom,
                    render_mask,
                    post_processing: self.camera_postfx_arc(node, &post_processing),
                    audio_options,
                }
            },
        );
        let camera_3d = found_3d.map(
            |(node, local_transform, projection, render_mask, post_processing, audio_options)| {
                let transform = self
                    .stream_localized_transform_3d(node, localize_3d)
                    .unwrap_or(local_transform);
                Camera3DState {
                    position: [
                        transform.position.x,
                        transform.position.y,
                        transform.position.z,
                    ],
                    rotation: [
                        transform.rotation.x,
                        transform.rotation.y,
                        transform.rotation.z,
                        transform.rotation.w,
                    ],
                    projection: camera_stream_projection_state(&projection),
                    render_mask,
                    post_processing: self.camera_postfx_arc(node, &post_processing),
                    audio_options,
                }
            },
        );
        (camera_2d, camera_3d)
    }

    pub(crate) fn camera_stream_texture_id(node: NodeID) -> TextureID {
        TextureID::from_parts(node.index(), node.generation())
    }

    /// Retained post-processing lane 4 a stream node (see `retained_arc_lane`).
    fn retained_post_fx(
        &mut self,
        stream_node: NodeID,
        built: &[perro_structs::PostProcessEffect],
    ) -> Arc<[perro_structs::PostProcessEffect]> {
        let lanes = self.stream_retention.lanes.entry(stream_node).or_default();
        retained_arc_lane(Some(&mut lanes.post_processing), built)
    }

    /// Fused member collection: ONE walk over the watched world fills every
    /// requested lane (was 4 walks per dimension, 8 for sub-views). Lane
    /// `Arc`s are retained per stream node: a refresh whose lane contents are
    /// unchanged hands back the previously emitted `Arc`, so graphics-side
    /// compares collapse to `Arc::ptr_eq`. `retain: false` = one-shot camera
    /// captures (no retention entry).
    fn collect_camera_stream_lanes(
        &mut self,
        stream_node: NodeID,
        two_d: Option<(BitMask, Option<[u32; 2]>)>,
        three_d: Option<BitMask>,
        retain: bool,
    ) -> StreamCollectedLanes {
        let localize_2d = two_d
            .is_some()
            .then(|| self.stream_localizer_2d(stream_node));
        let localize_3d = three_d
            .is_some()
            .then(|| self.stream_localizer_3d(stream_node));
        let mut out_2d = two_d.is_some().then(|| self.take_stream_2d_collect());
        let mut out_3d = three_d.is_some().then(|| self.take_stream_3d_collect());
        for idx in 0..self.camera_stream_node_scratch.len() {
            let node = self.camera_stream_node_scratch[idx];
            if node == stream_node
                || !self.is_effectively_visible(node)
                || self.stream_skips_isolated_child(node, stream_node)
            {
                continue;
            }
            if let (Some((mask, resolution)), Some(out), Some(localize)) =
                (two_d, out_2d.as_mut(), localize_2d.as_ref())
            {
                self.stream_member_2d(node, stream_node, mask, resolution, localize, out);
            }
            if let (Some(mask), Some(out), Some(localize)) =
                (three_d, out_3d.as_mut(), localize_3d.as_ref())
            {
                self.stream_member_3d(node, mask, localize, out);
            }
        }
        let mut lanes = StreamCollectedLanes::default();
        if let Some(out) = out_3d.as_mut() {
            lanes.lighting_3d = finish_stream_lighting_3d(out);
        }
        {
            let mut retained =
                retain.then(|| self.stream_retention.lanes.entry(stream_node).or_default());
            if let Some(out) = out_2d.as_ref() {
                lanes.sprites_2d = retained_arc_lane(
                    retained.as_deref_mut().map(|lanes| &mut lanes.sprites_2d),
                    &out.sprites,
                );
                lanes.lights_2d = retained_arc_lane(
                    retained.as_deref_mut().map(|lanes| &mut lanes.lights_2d),
                    &out.lights,
                );
                lanes.point_particles_2d = retained_arc_lane(
                    retained
                        .as_deref_mut()
                        .map(|lanes| &mut lanes.point_particles_2d),
                    &out.particles,
                );
                lanes.waters_2d = retained_arc_lane(
                    retained.as_deref_mut().map(|lanes| &mut lanes.waters_2d),
                    &out.waters,
                );
            }
            if let Some(out) = out_3d.as_ref() {
                lanes.draws_3d = retained_arc_lane(
                    retained.as_deref_mut().map(|lanes| &mut lanes.draws_3d),
                    &out.draws,
                );
                lanes.point_particles_3d = retained_arc_lane(
                    retained
                        .as_deref_mut()
                        .map(|lanes| &mut lanes.point_particles_3d),
                    &out.particles,
                );
                lanes.waters_3d =
                    retained_arc_lane(retained.map(|lanes| &mut lanes.waters_3d), &out.waters);
            }
        }
        if let Some(out) = out_2d.take() {
            self.restore_stream_2d_collect(out);
        }
        if let Some(out) = out_3d.take() {
            self.restore_stream_3d_collect(out);
        }
        lanes
    }

    pub(crate) fn camera_stream_state(
        &mut self,
        stream_node: NodeID,
        stream: &CameraStream,
    ) -> Option<CameraStreamState> {
        if !stream.enabled || stream.camera.is_nil() || stream.camera == stream_node {
            return None;
        }
        let source = self.camera_stream_source_state(stream.camera)?;
        // webcam sources render nothing from the scene: only source/output/
        // resolution/post-processing matter. short-circuit before the O(all
        // nodes) scratch fill + collectors (which return empty for webcam).
        if let CameraStreamSourceState::Webcam { texture, .. } = &source {
            let post_processing = stream.post_processing.to_effects_vec();
            let has_image_effect = post_processing
                .iter()
                .any(|effect| !matches!(effect, perro_structs::PostProcessEffect::Exposure { .. }));
            let output_texture = if has_image_effect {
                Self::camera_stream_texture_id(stream_node)
            } else {
                *texture
            };
            let post_processing = self.retained_post_fx(stream_node, &post_processing);
            return Some(CameraStreamState {
                source,
                tone_map_output: matches!(
                    self.nodes.get(stream_node).map(|node| &node.data),
                    Some(SceneNodeData::UiCameraStream(_))
                ),
                overlay_camera_2d: None,
                transparent_background: true,
                clear_color: None,
                resolution: [
                    stream.resolution.x.clamp(1, 8192),
                    stream.resolution.y.clamp(1, 8192),
                ],
                aspect_ratio: stream.aspect_ratio.max(0.0),
                post_processing,
                output_texture,
                sprites_2d: empty_arc_slice(),
                lights_2d: empty_arc_slice(),
                point_particles_2d: empty_arc_slice(),
                waters_2d: empty_arc_slice(),
                draws_3d: empty_arc_slice(),
                lighting_3d: CameraStreamLighting3DState::default(),
                point_particles_3d: empty_arc_slice(),
                waters_3d: empty_arc_slice(),
            });
        }
        // Share the owning world's direct member list once (refcount clone of
        // the membership cache; no per-stream copy). Nested sub-view
        // descendants never enter any collector for this stream.
        let owner = self.node_world(stream_node)?;
        self.camera_stream_node_scratch = self.world_members_arc(owner);
        let mut post_processing = match &source {
            CameraStreamSourceState::TwoD(camera) => camera.post_processing.to_vec(),
            CameraStreamSourceState::ThreeD(camera) => camera.post_processing.to_vec(),
            CameraStreamSourceState::Webcam { .. } => Vec::new(),
        };
        post_processing.extend(stream.post_processing.to_effects_vec());
        let post_processing = self.retained_post_fx(stream_node, &post_processing);
        let (two_d, three_d) = match &source {
            CameraStreamSourceState::TwoD(camera) => (Some((camera.render_mask, None)), None),
            CameraStreamSourceState::ThreeD(camera) => (None, Some(camera.render_mask)),
            CameraStreamSourceState::Webcam { .. } => (None, None),
        };
        let lanes = self.collect_camera_stream_lanes(stream_node, two_d, three_d, true);
        let output_texture = match &source {
            CameraStreamSourceState::Webcam { texture, .. } => *texture,
            _ => Self::camera_stream_texture_id(stream_node),
        };
        Some(CameraStreamState {
            source,
            tone_map_output: matches!(
                self.nodes.get(stream_node).map(|node| &node.data),
                Some(SceneNodeData::UiCameraStream(_))
            ),
            overlay_camera_2d: None,
            transparent_background: true,
            clear_color: None,
            resolution: [
                stream.resolution.x.clamp(1, 8192),
                stream.resolution.y.clamp(1, 8192),
            ],
            aspect_ratio: stream.aspect_ratio.max(0.0),
            post_processing,
            output_texture,
            sprites_2d: lanes.sprites_2d,
            lights_2d: lanes.lights_2d,
            point_particles_2d: lanes.point_particles_2d,
            waters_2d: lanes.waters_2d,
            draws_3d: lanes.draws_3d,
            lighting_3d: lanes.lighting_3d,
            point_particles_3d: lanes.point_particles_3d,
            waters_3d: lanes.waters_3d,
        })
    }

    pub(crate) fn camera_capture_state(
        &mut self,
        camera_node: NodeID,
        output_texture: TextureID,
        resolution: [u32; 2],
    ) -> Option<CameraStreamState> {
        let source = self.camera_stream_source_state(camera_node)?;
        if matches!(source, CameraStreamSourceState::Webcam { .. }) {
            return None;
        }
        let owner = self.node_world(camera_node)?;
        self.camera_stream_node_scratch = self.world_members_arc(owner);
        let post_processing = match &source {
            CameraStreamSourceState::TwoD(camera) => camera.post_processing.clone(),
            CameraStreamSourceState::ThreeD(camera) => camera.post_processing.clone(),
            CameraStreamSourceState::Webcam { .. } => empty_arc_slice(),
        };
        let (two_d, three_d) = match &source {
            CameraStreamSourceState::TwoD(camera) => {
                (Some((camera.render_mask, Some(resolution))), None)
            }
            CameraStreamSourceState::ThreeD(camera) => (None, Some(camera.render_mask)),
            CameraStreamSourceState::Webcam { .. } => unreachable!(),
        };
        // one-shot capture: no retention entry (always paired upsert/remove).
        let lanes = self.collect_camera_stream_lanes(camera_node, two_d, three_d, false);
        Some(CameraStreamState {
            source,
            tone_map_output: true,
            overlay_camera_2d: None,
            transparent_background: false,
            clear_color: None,
            resolution: [resolution[0].clamp(1, 8192), resolution[1].clamp(1, 8192)],
            aspect_ratio: 0.0,
            post_processing,
            output_texture,
            sprites_2d: lanes.sprites_2d,
            lights_2d: lanes.lights_2d,
            point_particles_2d: lanes.point_particles_2d,
            waters_2d: lanes.waters_2d,
            draws_3d: lanes.draws_3d,
            lighting_3d: lanes.lighting_3d,
            point_particles_3d: lanes.point_particles_3d,
            waters_3d: lanes.waters_3d,
        })
    }

    /// Explicit axes pass thru clamped; auto (0) axes go thru the bucket +
    /// shrink-hold, so a per-frame 1-px size animation keeps one target instead
    /// of recreating the whole chain every frame.
    fn sub_view_resolution(
        &mut self,
        view_node: NodeID,
        view: &SubView,
        auto_size: Option<[f32; 2]>,
    ) -> [u32; 2] {
        if view.resolution.x != 0 && view.resolution.y != 0 {
            return [
                view.resolution.x.clamp(1, 8192),
                view.resolution.y.clamp(1, 8192),
            ];
        }
        // `None` = no on-screen size is knowable for this host (SubView2D /
        // SubView3D live in world units, not px). The 1x1 fallback bottoms out
        // at one bucket; the node defaults carry an explicit 512x512 so the
        // branch above normally wins.
        let size = auto_size.unwrap_or([1.0, 1.0]);
        let mut want = auto_resolution_target(size, self.auto_resolution_scale());
        // one explicit axis: it pins that side, the other keeps its bucket.
        for axis in 0..2 {
            let explicit = [view.resolution.x, view.resolution.y][axis];
            if explicit != 0 {
                want[axis] = explicit.clamp(1, 8192);
            }
        }
        self.stream_retention
            .auto_resolutions
            .entry(view_node)
            .or_default()
            .resolve(want)
    }

    pub(crate) fn sub_view_state(
        &mut self,
        view_node: NodeID,
        view: &SubView,
        auto_size: Option<[f32; 2]>,
    ) -> Option<CameraStreamState> {
        if !view.enabled {
            return None;
        }

        let resolution = self.sub_view_resolution(view_node, view, auto_size);

        // How much this target oversamples its own on-screen rect. Auto res is
        // ~the project supersample factor (bucketing drifts it); an explicit
        // `resolution` is whatever the author picked. No known logical size
        // (SubView2D / SubView3D render targets) => the target IS the logical
        // size, 1.0.
        let owner_supersample = match auto_size {
            Some(size) => {
                sub_view_supersample_factor(resolution, size, self.auto_resolution_scale())
            }
            None => 1.0,
        };
        self.prepare_nested_sub_views(view_node, resolution, owner_supersample);
        self.camera_stream_node_scratch = self.world_members_arc(view_node);
        // root inverses once per refresh; shared by the camera pick + the
        // fused member walk (was 1 inverse per member per collector).
        let localize_2d = self.stream_localizer_2d(view_node);
        let localize_3d = self.stream_localizer_3d(view_node);

        let implicit_camera_3d = Camera3DState {
            position: [
                view.view_position.x,
                view.view_position.y,
                view.view_position.z,
            ],
            rotation: [
                view.view_rotation.x,
                view.view_rotation.y,
                view.view_rotation.z,
                view.view_rotation.w,
            ],
            projection: camera_stream_projection_state(&view.projection),
            render_mask: BitMask::NONE,
            post_processing: empty_arc_slice(),
            audio_options: perro_structs::AudioListenerOptions::new(),
        };
        let implicit_camera_2d = Camera2DState {
            position: [view.view_2d_position.x, view.view_2d_position.y],
            rotation_radians: view.view_2d_rotation,
            zoom: view.view_2d_zoom.max(0.001),
            render_mask: BitMask::NONE,
            post_processing: empty_arc_slice(),
            audio_options: perro_structs::AudioListenerOptions::new(),
        };
        let (camera_2d, camera_3d) = self.sub_view_cameras(&localize_2d, &localize_3d);
        let camera_3d = camera_3d.unwrap_or(implicit_camera_3d);
        let camera_2d = camera_2d.unwrap_or(implicit_camera_2d);
        let render_mask_3d = camera_3d.render_mask;
        let render_mask_2d = camera_2d.render_mask;
        let mut post_processing = camera_3d.post_processing.to_vec();
        post_processing.extend(camera_2d.post_processing.iter().cloned());
        post_processing.extend(view.post_processing.to_effects_vec());
        let post_processing = self.retained_post_fx(view_node, &post_processing);
        let source = CameraStreamSourceState::ThreeD(camera_3d);
        let overlay_camera_2d = Some(camera_2d);
        let lanes = self.collect_camera_stream_lanes(
            view_node,
            Some((render_mask_2d, Some(resolution))),
            Some(render_mask_3d),
            true,
        );

        Some(CameraStreamState {
            source,
            // Keep owned sub-view output scene-linear. The UI composite writes
            // it through the surface view, which performs the display transfer.
            // Applying ACES here compresses saturated art before composition.
            tone_map_output: false,
            overlay_camera_2d,
            transparent_background: view.background.a() < 1.0,
            clear_color: Some(view.background),
            resolution,
            aspect_ratio: view.aspect_ratio.max(0.0),
            post_processing,
            output_texture: Self::camera_stream_texture_id(view_node),
            sprites_2d: lanes.sprites_2d,
            lights_2d: lanes.lights_2d,
            point_particles_2d: lanes.point_particles_2d,
            waters_2d: lanes.waters_2d,
            draws_3d: lanes.draws_3d,
            lighting_3d: lanes.lighting_3d,
            point_particles_3d: lanes.point_particles_3d,
            waters_3d: lanes.waters_3d,
        })
    }

    pub(super) fn camera_stream_source_state(
        &mut self,
        camera_node: NodeID,
    ) -> Option<CameraStreamSourceState> {
        if !self.is_effectively_visible(camera_node) {
            let _ = self.resource_api.release_webcam_node_slot(camera_node);
            return None;
        }
        if self.nodes.get(camera_node).is_some_and(
            |node| matches!(&node.data, SceneNodeData::Webcam(webcam) if !webcam.enabled),
        ) {
            let _ = self.resource_api.release_webcam_node_slot(camera_node);
            return None;
        }
        let webcam_data = self
            .nodes
            .get(camera_node)
            .and_then(|node| match &node.data {
                SceneNodeData::Webcam(webcam) if webcam.enabled => Some(webcam.config.clone()),
                _ => None,
            });
        if let Some(config) = webcam_data {
            let webcam = self
                .resource_api
                .ensure_webcam_node_slot(camera_node, config);
            let texture = perro_resource_api::sub_apis::WebcamAPI::webcam_texture(
                self.resource_api.as_ref(),
                webcam,
            );
            let resolution = perro_resource_api::sub_apis::WebcamAPI::webcam_resolution(
                self.resource_api.as_ref(),
                webcam,
            )
            .or_else(|| {
                self.nodes
                    .get(camera_node)
                    .and_then(|node| match &node.data {
                        SceneNodeData::Webcam(webcam) => {
                            Some([webcam.config.width.max(1), webcam.config.height.max(1)])
                        }
                        _ => None,
                    })
            })
            .unwrap_or([1, 1]);
            return Some(CameraStreamSourceState::Webcam {
                texture,
                resolution,
            });
        }
        let camera_data = self
            .nodes
            .get(camera_node)
            .and_then(|node| match &node.data {
                SceneNodeData::Camera2D(camera) => Some((
                    camera.transform,
                    camera.zoom,
                    camera.render_mask,
                    camera.post_processing.clone(),
                    camera.audio_options.clone(),
                )),
                _ => None,
            });
        if let Some((local_transform, zoom, render_mask, post_processing, audio_options)) =
            camera_data
        {
            let global = self
                .get_render_global_transform_2d(camera_node)
                .unwrap_or(local_transform);
            return Some(CameraStreamSourceState::TwoD(Camera2DState {
                position: [global.position.x, global.position.y],
                rotation_radians: global.rotation,
                zoom,
                render_mask,
                post_processing: self.camera_postfx_arc(camera_node, &post_processing),
                audio_options,
            }));
        }

        let camera_data = self
            .nodes
            .get(camera_node)
            .and_then(|node| match &node.data {
                SceneNodeData::Camera3D(camera) => Some((
                    camera.transform,
                    camera.projection.clone(),
                    camera.render_mask,
                    camera.post_processing.clone(),
                    camera.audio_options.clone(),
                )),
                _ => None,
            });
        let (local_transform, projection, render_mask, post_processing, audio_options) =
            camera_data?;
        let global = self
            .get_render_global_transform_3d(camera_node)
            .unwrap_or(local_transform);
        Some(CameraStreamSourceState::ThreeD(Camera3DState {
            position: [global.position.x, global.position.y, global.position.z],
            rotation: [
                global.rotation.x,
                global.rotation.y,
                global.rotation.z,
                global.rotation.w,
            ],
            projection: camera_stream_projection_state(&projection),
            render_mask,
            post_processing: self.camera_postfx_arc(camera_node, &post_processing),
            audio_options,
        }))
    }
}

#[cfg(test)]
mod stream_retention_tests {
    use super::*;
    use perro_nodes::{AmbientLight2D, Node3D, SubView3D as SubView3DNode};
    use perro_runtime_api::sub_apis::NodeAPI;
    use perro_structs::TextureFilterMode;

    /// The factor a default (filtering) project runs at. `Runtime::new()` has
    /// no project, so `auto_resolution_scale` falls back to the same default.
    const SCALE: f32 = perro_structs::supersample_scale_f32(TextureFilterMode::LinearMipmap);
    /// A point-sampled (pixel-art) project opts out of supersampling entirely.
    const NEAREST_SCALE: f32 = perro_structs::supersample_scale_f32(TextureFilterMode::Nearest);

    fn sub_view_of(runtime: &Runtime, view: NodeID) -> SubView {
        match &runtime
            .nodes
            .get(view)
            .expect("test or bench setup must succeed")
            .data
        {
            SceneNodeData::SubView3D(node) => node.sub_view.clone(),
            _ => panic!("expected SubView3D"),
        }
    }

    fn retention_scene(runtime: &mut Runtime) -> (NodeID, NodeID, NodeID) {
        let view = NodeAPI::create::<SubView3DNode>(runtime);
        let light = NodeAPI::create::<AmbientLight2D>(runtime);
        let mover = NodeAPI::create::<Node3D>(runtime);
        assert!(runtime.reparent(view, light));
        assert!(runtime.reparent(view, mover));
        if let Some(mut node) = runtime.nodes.get_mut(view)
            && let SceneNodeData::SubView3D(sub_view) = &mut node.data
        {
            sub_view.sub_view.enabled = true;
        }
        if let Some(mut node) = runtime.nodes.get_mut(light)
            && let SceneNodeData::AmbientLight2D(ambient) = &mut node.data
        {
            ambient.active = true;
            ambient.intensity = 1.0;
        }
        (view, light, mover)
    }

    #[test]
    fn unchanged_refresh_reuses_lane_arcs() {
        let mut runtime = Runtime::new();
        let (view, light, mover) = retention_scene(&mut runtime);
        let sub_view = sub_view_of(&runtime, view);

        let first = runtime
            .sub_view_state(view, &sub_view, None)
            .expect("test or bench setup must succeed");
        assert!(!first.lights_2d.is_empty());
        let second = runtime
            .sub_view_state(view, &sub_view, None)
            .expect("test or bench setup must succeed");
        assert!(Arc::ptr_eq(&first.lights_2d, &second.lights_2d));
        assert!(Arc::ptr_eq(&first.post_processing, &second.post_processing));

        // unrelated node change: untouched lane keeps its Arc.
        if let Some(mut node) = runtime.nodes.get_mut(mover)
            && let SceneNodeData::Node3D(node_3d) = &mut node.data
        {
            node_3d.transform.position.x += 1.0;
        }
        let third = runtime
            .sub_view_state(view, &sub_view, None)
            .expect("test or bench setup must succeed");
        assert!(Arc::ptr_eq(&second.lights_2d, &third.lights_2d));

        // lane content change: fresh Arc.
        if let Some(mut node) = runtime.nodes.get_mut(light)
            && let SceneNodeData::AmbientLight2D(ambient) = &mut node.data
        {
            ambient.intensity = 0.5;
        }
        let fourth = runtime
            .sub_view_state(view, &sub_view, None)
            .expect("test or bench setup must succeed");
        assert!(!Arc::ptr_eq(&third.lights_2d, &fourth.lights_2d));
    }

    #[test]
    fn equal_state_upsert_resends_prior_whole_state_arc() {
        let mut runtime = Runtime::new();
        let (view, _light, _mover) = retention_scene(&mut runtime);
        let sub_view = sub_view_of(&runtime, view);

        let state = runtime
            .sub_view_state(view, &sub_view, None)
            .expect("test or bench setup must succeed");
        let first = Arc::new(state.clone());
        runtime.queue_camera_stream_upsert(view, first.clone());
        let retained = runtime
            .stream_retention
            .states
            .get(&view)
            .expect("retained whole state")
            .clone();
        assert!(Arc::ptr_eq(&retained, &first));

        // equal rebuild in a fresh Arc: retention keeps the FIRST Arc so the
        // gpu-side upsert compare hits Arc::ptr_eq.
        runtime.queue_camera_stream_upsert(view, Arc::new(state));
        let still_retained = runtime
            .stream_retention
            .states
            .get(&view)
            .expect("retained whole state")
            .clone();
        assert!(Arc::ptr_eq(&still_retained, &first));
    }

    #[test]
    fn auto_resolution_target_buckets_long_axis_and_keeps_aspect() {
        // long axis rounds up into a bucket, never below the exact size.
        for size in [[1.0f32, 1.0], [100.0, 60.0], [320.0, 180.0], [301.0, 173.0]] {
            let target = auto_resolution_target(size, SCALE);
            let long = usize::from(size[1] > size[0]);
            assert_eq!(target[long] % AUTO_RESOLUTION_BUCKET, 0);
            assert!(target[long] as f32 >= size[long] * SCALE);
            let want_aspect = size[0] / size[1];
            let got_aspect = target[0] as f32 / target[1] as f32;
            assert!((got_aspect - want_aspect).abs() / want_aspect < 0.01);
        }
        // exact-bucket 16:9 rects keep their exact supersampled target.
        assert_eq!(auto_resolution_target([320.0, 180.0], SCALE), [640, 360]);
        assert_eq!(auto_resolution_target([1280.0, 720.0], SCALE), [2560, 1440]);
        // clamped, not overflowed; degenerate input never panics.
        assert_eq!(auto_resolution_target([f32::MAX, f32::MAX], SCALE), [
            8192, 8192
        ]);
        assert_eq!(auto_resolution_target([f32::NAN, 0.0], SCALE), [64, 64]);
    }

    #[test]
    fn auto_resolution_absorbs_sub_bucket_wobble() {
        let mut state = AutoResolutionState::default();
        let first = state.resolve(auto_resolution_target([300.0, 300.0], SCALE));
        // a per-frame size animation inside one bucket: same target.
        for step in 0..40 {
            let size = 300.0 + step as f32 * 0.5;
            assert_eq!(
                state.resolve(auto_resolution_target([size, size], SCALE)),
                first
            );
        }
        // crossing the bucket grows same-refresh.
        let grown = state.resolve(auto_resolution_target([400.0, 400.0], SCALE));
        assert!(grown[0] > first[0]);
    }

    #[test]
    fn auto_resolution_shrink_waits_but_big_drop_lands_now() {
        let mut state = AutoResolutionState::default();
        let big = state.resolve(auto_resolution_target([1000.0, 1000.0], SCALE));
        // one bucket smaller, same aspect: hold applies.
        let want = auto_resolution_target([992.0, 992.0], SCALE);
        assert!(want[0] < big[0]);
        for _ in 0..AUTO_RESOLUTION_SHRINK_HOLD - 1 {
            assert_eq!(state.resolve(want), big);
        }
        assert_eq!(state.resolve(want), want);

        // >= 2 buckets smaller = real layout change: no hold.
        let mut state = AutoResolutionState::default();
        state.resolve(auto_resolution_target([1000.0, 1000.0], SCALE));
        let small = auto_resolution_target([880.0, 880.0], SCALE);
        assert_eq!(state.resolve(small), small);
    }

    #[test]
    fn auto_resolution_shrink_hold_yields_to_aspect_drift() {
        let mut state = AutoResolutionState::default();
        let square = state.resolve(auto_resolution_target([1000.0, 1000.0], SCALE));
        // shrink on one axis only: the held target would letterbox, so the
        // hold does not apply.
        let narrowed = auto_resolution_target([1000.0, 970.0], SCALE);
        assert_ne!(state.resolve(narrowed), square);
    }

    #[test]
    fn auto_sub_view_resolution_stays_put_across_wobbling_refreshes() {
        let mut runtime = Runtime::new();
        let (view, _light, _mover) = retention_scene(&mut runtime);
        let sub_view = sub_view_of(&runtime, view);

        let first = runtime
            .sub_view_state(view, &sub_view, Some([300.0, 200.0]))
            .expect("test or bench setup must succeed")
            .resolution;
        for step in 1..30 {
            let scale = 1.0 + step as f32 * 0.001;
            let size = [300.0 * scale, 200.0 * scale];
            let state = runtime
                .sub_view_state(view, &sub_view, Some(size))
                .expect("test or bench setup must succeed");
            assert_eq!(state.resolution, first);
        }
    }

    /// `sub_view_supersample_factor` divides the owner's oversample back out of
    /// a nested rect. Without it the project supersample factor compounds per
    /// nesting level (measured: 3 levels each 0.5x its parent all landed on
    /// 1920x1080 instead of 1920x1080 / 960x540 / 512x288).
    #[test]
    fn sub_view_supersample_factor_divides_owner_oversample_back_out() {
        // auto res: target is the project factor x the on-screen rect.
        assert_eq!(
            sub_view_supersample_factor([1920, 1080], [960.0, 540.0], SCALE),
            2.0
        );
        // 1:1 target adds no oversample to divide out.
        assert_eq!(
            sub_view_supersample_factor([960, 540], [960.0, 540.0], SCALE),
            1.0
        );
        // Past 2x the cap holds, so a nested target never drops below the
        // footprint it occupies in the owner target.
        assert_eq!(
            sub_view_supersample_factor([3840, 2160], [960.0, 540.0], SCALE),
            2.0
        );
        // Owner target aspect != rect aspect: uniform min-axis factor, so the
        // nested target keeps its aspect instead of letterboxing.
        assert_eq!(
            sub_view_supersample_factor([960, 1080], [320.0, 180.0], SCALE),
            2.0
        );
        assert_eq!(
            sub_view_supersample_factor([480, 1080], [320.0, 180.0], SCALE),
            1.5
        );
        // Degenerate logical sizes must not produce inf/NaN (would collapse a
        // nested target to 1px) -> neutral 1.0.
        assert_eq!(
            sub_view_supersample_factor([1920, 1080], [0.0, 0.0], SCALE),
            1.0
        );
        assert_eq!(
            sub_view_supersample_factor([1920, 1080], [f32::NAN, f32::NAN], SCALE),
            1.0
        );
        assert_eq!(
            sub_view_supersample_factor([1920, 1080], [-5.0, -5.0], SCALE),
            1.0
        );
        // One usable axis still yields a factor.
        assert_eq!(
            sub_view_supersample_factor([1920, 1080], [960.0, 0.0], SCALE),
            2.0
        );
        // Never scales a nested rect UP.
        assert_eq!(
            sub_view_supersample_factor([64, 64], [1024.0, 1024.0], SCALE),
            1.0
        );
    }

    /// End-to-end shape of the fix: a level-N rect arrives in owner-target px,
    /// and dividing by the owner factor before `auto_resolution_target` keeps
    /// each level supersampling exactly once. Mirrors sv_nested.scn (each level
    /// half its parent on both axes) at a 1920x1080 window.
    #[test]
    fn nested_auto_resolution_halves_per_level_instead_of_compounding() {
        // depth 0: rect is real screen px.
        let l1 = auto_resolution_target([960.0, 540.0], SCALE);
        assert_eq!(l1, [1920, 1080]);

        // depth 1: rect measured in the L1 TARGET = 960x540 target px.
        let f1 = sub_view_supersample_factor(l1, [960.0, 540.0], SCALE);
        assert_eq!(f1, 2.0);
        let l2 = auto_resolution_target([960.0 / f1, 540.0 / f1], SCALE);
        assert_eq!(l2, [960, 540]);

        // depth 2: rect measured in the L2 target = 480x270 target px.
        let f2 = sub_view_supersample_factor(l2, [480.0, 270.0], SCALE);
        let l3 = auto_resolution_target([480.0 / f2, 270.0 / f2], SCALE);
        // 240x135 * 2 = 480x270, long axis buckets up to 512.
        assert_eq!(l3, [512, 288]);

        // the whole point: strictly shrinking, not 3x the same target.
        assert!(l3[0] < l2[0] && l2[0] < l1[0]);
        let total: u64 = [l1, l2, l3]
            .iter()
            .map(|r| u64::from(r[0]) * u64::from(r[1]))
            .sum();
        // compounding cost 3 x 1920x1080 = 6_220_800 px.
        assert_eq!(total, 2_739_456);
    }

    /// THE INVARIANT, stated as a test: for any target,
    /// `effective_supersample(T) == own_factor(T)`, at every nesting depth.
    ///
    /// Walks 6 levels of the real pipeline shape (measure rect in owner-target
    /// px -> divide the owner factor out -> supersample once) and asserts each
    /// level's target is exactly its OWN on-screen size x the project factor
    /// (bucket rounding aside, which only ever rounds UP and never by a whole
    /// factor).
    #[test]
    fn effective_supersample_equals_own_factor_at_every_depth() {
        for scale in [SCALE, NEAREST_SCALE] {
            // level 0 on-screen size, then each level is half its parent.
            let mut on_screen = [960.0f32, 540.0];
            let mut owner: Option<([u32; 2], [f32; 2])> = None;
            for depth in 0..6 {
                // The rect the runtime actually measures: owner-target px at
                // depth > 0, real screen px at depth 0.
                let (target, factor) = match owner {
                    None => (auto_resolution_target(on_screen, scale), 1.0),
                    Some((owner_target, owner_on_screen)) => {
                        let owner_factor =
                            sub_view_supersample_factor(owner_target, owner_on_screen, scale);
                        // rect in owner-target px = on-screen px * owner factor.
                        let in_owner_px =
                            [on_screen[0] * owner_factor, on_screen[1] * owner_factor];
                        let divided =
                            [in_owner_px[0] / owner_factor, in_owner_px[1] / owner_factor];
                        (auto_resolution_target(divided, scale), owner_factor)
                    }
                };
                // THIS target measured against its OWN on-screen size. The only
                // slack permitted over `on_screen * scale` is the long-axis
                // bucket, which rounds UP by < AUTO_RESOLUTION_BUCKET px. A
                // leaked ancestor factor would instead multiply the whole
                // target, which cannot hide inside one bucket.
                let long = usize::from(on_screen[1] > on_screen[0]);
                let exact = on_screen[long] * scale;
                let got = target[long] as f32;
                assert!(
                    got >= exact,
                    "depth {depth} scale {scale}: target {got} below its own {exact}"
                );
                assert!(
                    got < exact + AUTO_RESOLUTION_BUCKET as f32,
                    "depth {depth} scale {scale}: ancestor factor {factor} leaked in \
                     (target {got} vs own {exact})"
                );
                owner = Some((target, on_screen));
                on_screen = [on_screen[0] * 0.5, on_screen[1] * 0.5];
            }
        }
    }

    /// A `Nearest` project drops the supersample everywhere at once, so the
    /// runtime target still lands 1:1 in the (now un-supersampled) UI raster
    /// target. Both ends read the same function, so they cannot disagree.
    #[test]
    fn nearest_filter_drops_the_factor_on_both_ends_together() {
        assert_eq!(NEAREST_SCALE, 1.0);
        // target == rect: the depth-0 footprint in a 1x UI raster target.
        assert_eq!(
            auto_resolution_target([960.0, 540.0], NEAREST_SCALE),
            [960, 540]
        );
        // No oversample exists to divide back out, and the cap is 1.0, so a
        // nested level cannot be scaled up either.
        assert_eq!(
            sub_view_supersample_factor([960, 540], [960.0, 540.0], NEAREST_SCALE),
            1.0
        );
        assert_eq!(
            sub_view_supersample_factor([1920, 1080], [960.0, 540.0], NEAREST_SCALE),
            1.0
        );
        // Guard the enforced floor: a bogus sub-1 factor never shrinks a target
        // below its own rect.
        assert_eq!(
            sub_view_supersample_factor([1920, 1080], [960.0, 540.0], 0.0),
            1.0
        );
        assert_eq!(auto_resolution_target([320.0, 180.0], NEAREST_SCALE), [
            320, 180
        ]);
    }

    /// TASK 3 path: camera streams (`CameraStream2D` / `CameraStream3D` /
    /// `UiCameraStream`) never derive a size from an on-screen rect, so their
    /// own factor is 1.0 by construction. Nesting cannot change it: the
    /// resolution shipped is exactly the author's, clamped.
    #[test]
    fn camera_stream_resolution_is_author_pinned_and_never_supersampled() {
        use perro_nodes::{Camera3D, CameraStream3D as CameraStream3DNode};

        let mut runtime = Runtime::new();
        let camera = NodeAPI::create::<Camera3D>(&mut runtime);
        if let Some(mut node) = runtime.nodes.get_mut(camera)
            && let SceneNodeData::Camera3D(camera) = &mut node.data
        {
            camera.active = true;
        }
        let stream_node = NodeAPI::create::<CameraStream3DNode>(&mut runtime);
        let stream = match &mut runtime
            .nodes
            .get_mut(stream_node)
            .expect("test or bench setup must succeed")
            .data
        {
            SceneNodeData::CameraStream3D(node) => {
                node.stream.enabled = true;
                node.stream.camera = camera;
                node.stream.resolution = perro_structs::UVector2::new(829, 467);
                node.stream.clone()
            }
            _ => panic!("expected CameraStream3D"),
        };

        let state = runtime
            .camera_stream_state(stream_node, &stream)
            .expect("test or bench setup must succeed");
        // Author's px, verbatim: no bucket, no factor, no ancestor term.
        assert_eq!(state.resolution, [829, 467]);
        // Second refresh (a nested/dirty rebuild) cannot drift it either.
        let again = runtime
            .camera_stream_state(stream_node, &stream)
            .expect("test or bench setup must succeed");
        assert_eq!(again.resolution, [829, 467]);
    }

    /// TASK 3 path: `SubView2D` / `SubView3D` hosts live in world units, so no
    /// on-screen px size is knowable and every call site passes `auto_size:
    /// None`. Their target is their explicit `resolution` (512x512 by default),
    /// own factor 1.0.
    ///
    /// LATENT LIMITATION locked here: an author who zeroes `resolution` to ask
    /// for "auto" gets the 1x1 fallback, i.e. one bucket (64x64), NOT a target
    /// derived from `size`. Auto resolution is meaningful only for `UiSubView`.
    #[test]
    fn world_space_sub_views_use_explicit_resolution_and_bottom_out_at_one_bucket() {
        let mut runtime = Runtime::new();
        let (view, _light, _mover) = retention_scene(&mut runtime);
        let mut sub_view = sub_view_of(&runtime, view);
        // node default: explicit, so auto resolution never engages.
        assert_eq!(sub_view.resolution, perro_structs::UVector2::new(512, 512));
        let state = runtime
            .sub_view_state(view, &sub_view, None)
            .expect("test or bench setup must succeed");
        assert_eq!(state.resolution, [512, 512]);

        // zeroed = "auto", but there is no rect to scale: one bucket, both
        // scales. Not `size`-derived; documented above.
        sub_view.resolution = perro_structs::UVector2::new(0, 0);
        let state = runtime
            .sub_view_state(view, &sub_view, None)
            .expect("test or bench setup must succeed");
        assert_eq!(state.resolution, [AUTO_RESOLUTION_BUCKET; 2]);
    }

    /// TASK 3 path: mixed nesting. A sub-view whose OWNER is a camera stream
    /// gets its rect in real screen/world px (a camera stream carries no
    /// oversample to divide out), so its target is its own factor once. And a
    /// camera stream nested under a sub-view keeps its pinned resolution. The
    /// chain is factor-free in both directions.
    #[test]
    fn mixed_nesting_never_multiplies_factors() {
        // sub-view under a camera stream: the owner factor is 1.0, so the
        // nested target is exactly the auto rule applied once.
        let owner_target = [829u32, 467];
        let owner_on_screen = [829.0f32, 467.0];
        let owner_factor = sub_view_supersample_factor(owner_target, owner_on_screen, SCALE);
        assert_eq!(owner_factor, 1.0);
        let nested_rect = [400.0f32, 225.0];
        let nested = auto_resolution_target(
            [nested_rect[0] / owner_factor, nested_rect[1] / owner_factor],
            SCALE,
        );
        // 400x225 * 2 = 800x450, long axis buckets up to 832.
        assert_eq!(nested, [832, 468]);
        assert!((nested[0] as f32 / nested_rect[0]) < SCALE * 2.0);

        // camera stream under a sub-view: the owner target oversamples 2x, and
        // the stream's pinned resolution ignores it entirely (a camera stream
        // has no rect -> target derivation to inherit the factor through).
        let ui_owner_target = auto_resolution_target([960.0, 540.0], SCALE);
        assert_eq!(
            sub_view_supersample_factor(ui_owner_target, [960.0, 540.0], SCALE),
            SCALE
        );
        let mut runtime = Runtime::new();
        let (view, _light, _mover) = retention_scene(&mut runtime);
        let camera = NodeAPI::create::<perro_nodes::Camera3D>(&mut runtime);
        if let Some(mut node) = runtime.nodes.get_mut(camera)
            && let SceneNodeData::Camera3D(camera) = &mut node.data
        {
            camera.active = true;
        }
        let stream_node = NodeAPI::create::<perro_nodes::CameraStream3D>(&mut runtime);
        // both live INSIDE the sub-view's isolated world.
        assert!(runtime.reparent(view, camera));
        assert!(runtime.reparent(view, stream_node));
        let stream = match &mut runtime
            .nodes
            .get_mut(stream_node)
            .expect("test or bench setup must succeed")
            .data
        {
            SceneNodeData::CameraStream3D(node) => {
                node.stream.enabled = true;
                node.stream.camera = camera;
                node.stream.resolution = perro_structs::UVector2::new(300, 200);
                node.stream.clone()
            }
            _ => panic!("expected CameraStream3D"),
        };
        let state = runtime
            .camera_stream_state(stream_node, &stream)
            .expect("test or bench setup must succeed");
        assert_eq!(state.resolution, [300, 200]);
    }
}
