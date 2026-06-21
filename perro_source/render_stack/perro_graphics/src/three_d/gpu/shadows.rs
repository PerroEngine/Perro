use super::*;

pub(super) struct ShadowSetup {
    pub(super) scenes: Vec<Scene3DUniform>,
    pub(super) uniform: ShadowUniform,
    pub(super) enabled: bool,
    pub(super) ray_enabled: bool,
    pub(super) spot_count: usize,
    pub(super) point_count: usize,
    pub(super) focus_center: Vec3,
    pub(super) focus_radius: f32,
}

impl Gpu3D {
    pub(super) fn update_shadow_state(
        &mut self,
        queue: &wgpu::Queue,
        camera: &Camera3DState,
        lighting: &Lighting3DState,
    ) {
        if std::env::var_os("PERRO_DISABLE_SHADOWS").is_some() {
            let zero = ShadowUniform::zeroed();
            if self.last_shadow != Some(zero) {
                queue.write_buffer(&self.shadow_buffer, 0, bytemuck::bytes_of(&zero));
                self.last_shadow = Some(zero);
            }
            self.shadow_pass_enabled = false;
            self.ray_shadow_enabled = false;
            self.spot_shadow_count = 0;
            self.point_shadow_count = 0;
            return;
        }
        let setup = build_shadow_setup(ShadowSetupArgs {
            camera,
            lighting,
            draw_batches: &self.draw_batches,
            staged_instances: &self.staged_instance_transforms,
            fallback_focus_center: self.shadow_focus_center,
            fallback_focus_radius: self.shadow_focus_radius,
            viewport_width: self.depth_size.0,
            viewport_height: self.depth_size.1,
        });
        self.shadow_focus_center = setup.focus_center;
        self.shadow_focus_radius = setup.focus_radius;
        if self.last_shadow_scenes.len() != SHADOW_CAMERA_COUNT {
            self.last_shadow_scenes.resize(SHADOW_CAMERA_COUNT, None);
        }
        for (index, scene) in setup.scenes.iter().copied().enumerate() {
            if self.last_shadow_scenes.get(index).copied().flatten() != Some(scene)
                && let Some(buffer) = self.shadow_camera_buffers.get(index)
            {
                queue.write_buffer(buffer, 0, bytemuck::bytes_of(&scene));
                self.last_shadow_scenes[index] = Some(scene);
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
    }
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
    } = args;
    let mut scenes = vec![Scene3DUniform::zeroed(); SHADOW_CAMERA_COUNT];
    let mut uniform = ShadowUniform::zeroed();
    let mut focus_center = fallback_focus_center;
    let mut focus_radius = fallback_focus_radius;

    let has_casters = draw_batches
        .iter()
        .any(|batch| !batch.draw_on_top && batch.casts_shadows && batch.alpha_mode == 0);
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
        uniform.ray_params = [
            1.0,
            MAX_SHADOW_RAY_CASCADES as f32,
            ray_setup.splits[3],
            0.0,
        ];
        focus_center = ray_setup.focus_center;
        focus_radius = ray_setup.focus_radius;
        any_enabled = true;
        ray_enabled = true;
    }

    let mut spot_count = 0usize;
    for (light_index, spot) in lighting.spot_lights.iter().enumerate() {
        if spot_count >= MAX_SHADOW_SPOT_LIGHTS {
            break;
        }
        let Some(spot) = spot else {
            continue;
        };
        if !spot.cast_shadows || spot.intensity <= 1.0e-4 {
            continue;
        }
        let Some(light_view_proj) = spot_light_view_proj(*spot) else {
            continue;
        };
        let camera_index = MAX_SHADOW_RAY_LIGHTS * MAX_SHADOW_RAY_CASCADES + spot_count;
        scenes[camera_index].view_proj = light_view_proj.to_cols_array_2d();
        uniform.spot_light_view_proj[spot_count] = scenes[camera_index].view_proj;
        uniform.spot_params[spot_count] = [1.0, light_index as f32, spot_count as f32, 0.0];
        spot_count += 1;
        any_enabled = true;
    }

    let mut point_count = 0usize;
    for (light_index, point) in lighting.point_lights.iter().enumerate() {
        if point_count >= MAX_SHADOW_POINT_LIGHTS {
            break;
        }
        let Some(point) = point else {
            continue;
        };
        if !point.cast_shadows || point.intensity <= 1.0e-4 || point.range <= 0.01 {
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
            light_index as f32,
            base_layer as f32,
            point.range.max(0.01),
        ];
        point_count += 1;
        any_enabled = true;
    }

    uniform.params0 = if any_enabled {
        [1.0, 0.82, 0.00018, 0.045]
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
    }
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
}

struct RayShadowScenes {
    scenes: Vec<Scene3DUniform>,
    matrices: [Mat4; MAX_SHADOW_RAY_CASCADES],
    splits: [f32; 4],
    focus_center: Vec3,
    focus_radius: f32,
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
    } else if let Some(ray) = explicit_shadow_ray {
        Vec3::from(ray.direction).normalize_or_zero()
    } else {
        return None;
    };
    if dir.length_squared() <= 1.0e-6 || !dir.is_finite() {
        return None;
    }

    let (batch_focus_center, batch_focus_radius, has_batch_bounds) =
        compute_shadow_focus_bounds(camera, draw_batches, staged_instances);
    let cascade_splits = ray_cascade_splits(camera);
    let mut full_corners = camera_frustum_slice_corners_world(
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
        full_corners = stable_shadow_focus_points(focus_center, focus_radius);
    }
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
    let mut scenes = Vec::with_capacity(MAX_SHADOW_RAY_CASCADES);
    let mut matrices = [Mat4::IDENTITY; MAX_SHADOW_RAY_CASCADES];
    for cascade in 0..MAX_SHADOW_RAY_CASCADES {
        let mut corners = camera_frustum_slice_corners_world(
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
        if has_batch_bounds {
            corners.extend_from_slice(&full_corners);
        }
        let center =
            corners.iter().copied().fold(Vec3::ZERO, |acc, p| acc + p) / (corners.len() as f32);
        let radius = corners
            .iter()
            .copied()
            .map(|p| (p - center).length())
            .fold(0.0f32, f32::max)
            .clamp(2.0, 600.0);
        let distance = (radius * 3.0).max(80.0);
        let mut eye = center - dir * distance;
        let mut target = center;
        let mut view = Mat4::look_at_rh(eye, target, up);
        let (mut ls_min, mut ls_max) = light_space_bounds(&corners, view)?;
        let span_x = (ls_max.x - ls_min.x).max(2.0);
        let span_y = (ls_max.y - ls_min.y).max(2.0);
        let wupt_x = (span_x / SHADOW_MAP_SIZE as f32).max(1.0e-6);
        let wupt_y = (span_y / SHADOW_MAP_SIZE as f32).max(1.0e-6);
        let center_ls_x = (ls_min.x + ls_max.x) * 0.5;
        let center_ls_y = (ls_min.y + ls_max.y) * 0.5;
        let center_delta = right_axis * ((center_ls_x / wupt_x).round() * wupt_x - center_ls_x)
            + up_axis * ((center_ls_y / wupt_y).round() * wupt_y - center_ls_y);
        eye += center_delta;
        target += center_delta;
        view = Mat4::look_at_rh(eye, target, up);
        (ls_min, ls_max) = light_space_bounds(&corners, view)?;
        let xy_pad = ((ls_max.x - ls_min.x).max(ls_max.y - ls_min.y) * 0.08).max(1.0);
        ls_min.x -= xy_pad;
        ls_max.x += xy_pad;
        ls_min.y -= xy_pad;
        ls_max.y += xy_pad;
        let z_pad = (radius * 0.65).max(12.0);
        let near = (-ls_max.z - z_pad).max(0.1);
        let far = (-ls_min.z + z_pad).max(near + 1.0);
        let light_view_proj =
            Mat4::orthographic_rh(ls_min.x, ls_max.x, ls_min.y, ls_max.y, near, far) * view;
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
        focus_center,
        focus_radius,
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

pub(super) fn compute_shadow_focus_bounds(
    camera: &Camera3DState,
    draw_batches: &[DrawBatch],
    staged_instances: &[TransformInstanceGpu],
) -> (Vec3, f32, bool) {
    let mut any = false;
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for batch in draw_batches {
        if batch.draw_on_top || !batch.casts_shadows || batch.alpha_mode != 0 {
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
        DrawBatch {
            state_key,
            render_state: render_state_key(state_key, 0, 0, 0, false, 0, false),
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
            local_center: [0.0, 0.0, 0.0],
            local_radius: 2.0,
            occlusion_query: None,
            disable_hiz_occlusion: false,
            casts_shadows: true,
            receives_shadows: true,
            mesh_blend: false,
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
        });
        lighting
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
        });
        lighting.spot_lights[1] = Some(SpotLight3DState {
            cast_shadows: false,
            ..lighting.spot_lights[0].unwrap()
        });
        lighting.point_lights[0] = Some(PointLight3DState {
            position: [2.0, 3.0, 4.0],
            color: [1.0, 1.0, 1.0],
            intensity: 1.0,
            range: 10.0,
            cast_shadows: true,
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
}
