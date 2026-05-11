use super::*;

impl Gpu3D {
    pub(super) fn update_shadow_state(
        &mut self,
        queue: &wgpu::Queue,
        camera: &Camera3DState,
        lighting: &Lighting3DState,
    ) {
        let (shadow_scene, shadow_uniform, enabled, focus_center, focus_radius) =
            build_shadow_setup(ShadowSetupArgs {
                camera,
                lighting,
                draw_batches: &self.draw_batches,
                staged_instances: &self.staged_instance_transforms,
                fallback_focus_center: self.shadow_focus_center,
                fallback_focus_radius: self.shadow_focus_radius,
                viewport_width: self.depth_size.0,
                viewport_height: self.depth_size.1,
            });
        self.shadow_focus_center = focus_center;
        self.shadow_focus_radius = focus_radius;
        if self.last_shadow_scene != Some(shadow_scene) {
            queue.write_buffer(
                &self.shadow_camera_buffer,
                0,
                bytemuck::bytes_of(&shadow_scene),
            );
            self.last_shadow_scene = Some(shadow_scene);
        }
        if self.last_shadow != Some(shadow_uniform) {
            queue.write_buffer(&self.shadow_buffer, 0, bytemuck::bytes_of(&shadow_uniform));
            self.last_shadow = Some(shadow_uniform);
        }
        self.shadow_pass_enabled = enabled;
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

pub(super) fn build_shadow_setup(
    args: ShadowSetupArgs<'_>,
) -> (Scene3DUniform, ShadowUniform, bool, Vec3, f32) {
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
    let mut shadow_scene = Scene3DUniform::zeroed();
    let mut shadow_uniform = ShadowUniform::zeroed();
    if TEMP_DISABLE_SHADOWS {
        return (
            shadow_scene,
            shadow_uniform,
            false,
            fallback_focus_center,
            fallback_focus_radius,
        );
    }

    let explicit_shadow_ray = lighting
        .ray_lights
        .iter()
        .flatten()
        .copied()
        .find(|light| light.cast_shadows && light.intensity > 1.0e-4);

    let sky_shadow_dir = lighting.sky.as_ref().and_then(|sky| {
        let (sun_body_dir, moon_body_dir) =
            sun_moon_dirs_from_time(sky.time.time_of_day, sky.sky_angle);
        let sun_dir = -sun_body_dir;
        let moon_dir = -moon_body_dir;
        let day_amt = day_weight_from_time(sky.time.time_of_day).powf(1.20);
        let dusk_amt = evening_weight_from_time(sky.time.time_of_day) * (1.0 - day_amt * 0.55);
        let night_amt = (1.0 - day_amt).clamp(0.0, 1.0);
        let sun_intensity = (((day_amt * 1.35) + (dusk_amt * 0.22))
            * sky.sun_size.max(0.1)
            * horizon_visibility(sun_body_dir.y))
        .max(0.0);
        let moon_intensity =
            ((night_amt * 0.18) * sky.moon_size.max(0.05) * horizon_visibility(moon_body_dir.y))
                .max(0.0);
        if sun_intensity > 1.0e-4 {
            Some(sun_dir)
        } else if moon_intensity > 1.0e-4 {
            Some(moon_dir)
        } else {
            None
        }
    });

    // Prefer authored directional lights when present.
    let dir = if DEBUG_FORCE_WORLD_SUN_DIR {
        Vec3::new(
            DEBUG_WORLD_SUN_DIR[0],
            DEBUG_WORLD_SUN_DIR[1],
            DEBUG_WORLD_SUN_DIR[2],
        )
        .normalize_or_zero()
    } else if let Some(ray) = explicit_shadow_ray {
        Vec3::from(ray.direction).normalize_or_zero()
    } else if let Some(dir) = sky_shadow_dir {
        dir.normalize_or_zero()
    } else {
        return (
            shadow_scene,
            shadow_uniform,
            false,
            fallback_focus_center,
            fallback_focus_radius,
        );
    };
    if dir.length_squared() <= 1.0e-6 || !dir.is_finite() {
        return (
            shadow_scene,
            shadow_uniform,
            false,
            fallback_focus_center,
            fallback_focus_radius,
        );
    }

    let has_casters = draw_batches
        .iter()
        .any(|batch| !batch.draw_on_top && batch.casts_shadows);
    if !has_casters {
        return (
            shadow_scene,
            shadow_uniform,
            false,
            fallback_focus_center,
            fallback_focus_radius,
        );
    }

    let (batch_focus_center, batch_focus_radius, has_batch_bounds) =
        compute_shadow_focus_bounds(camera, draw_batches, staged_instances);

    let Some(mut frustum_corners) =
        camera_frustum_corners_world(camera, viewport_width, viewport_height)
    else {
        return (
            shadow_scene,
            shadow_uniform,
            false,
            fallback_focus_center,
            fallback_focus_radius,
        );
    };

    // Clamp shadow coverage depth for stability/quality.
    let camera_pos = Vec3::from(camera.position);
    let max_shadow_distance = 220.0f32;
    for corner in &mut frustum_corners {
        let to = *corner - camera_pos;
        let d = to.length();
        if d.is_finite() && d > max_shadow_distance && d > 1.0e-4 {
            *corner = camera_pos + to * (max_shadow_distance / d);
        }
    }

    let mut focus_center = frustum_corners
        .iter()
        .copied()
        .fold(Vec3::ZERO, |acc, p| acc + p)
        / (frustum_corners.len() as f32);
    let mut focus_radius = frustum_corners
        .iter()
        .copied()
        .map(|p| (p - focus_center).length())
        .fold(0.0f32, f32::max)
        .clamp(10.0, 600.0);
    if has_batch_bounds {
        focus_center = focus_center.lerp(batch_focus_center, 0.35);
        focus_radius = focus_radius
            .max(batch_focus_radius * 0.85)
            .clamp(10.0, 600.0);
    }

    if fallback_focus_center.is_finite() && fallback_focus_radius.is_finite() {
        let t = 0.20;
        focus_center = fallback_focus_center.lerp(focus_center, t);
        let current = fallback_focus_radius.max(10.0);
        let target = focus_radius.max(10.0);
        focus_radius = (current + (target - current) * t).clamp(10.0, 600.0);
    }

    let up = if dir.dot(Vec3::Y).abs() > 0.95 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let distance = (focus_radius * 3.0).max(80.0);
    let mut eye = focus_center - dir * distance;
    let (right_axis, up_axis) = light_stable_axes(dir, up);

    let mut view = Mat4::look_at_rh(eye, focus_center, up);
    let Some((mut ls_min, mut ls_max)) = light_space_bounds(&frustum_corners, view) else {
        return (
            shadow_scene,
            shadow_uniform,
            false,
            focus_center,
            focus_radius,
        );
    };

    let mut span_x = (ls_max.x - ls_min.x).max(2.0);
    let mut span_y = (ls_max.y - ls_min.y).max(2.0);
    let xy_pad = (span_x.max(span_y) * 0.08).max(2.0);
    ls_min.x -= xy_pad;
    ls_max.x += xy_pad;
    ls_min.y -= xy_pad;
    ls_max.y += xy_pad;
    span_x = (ls_max.x - ls_min.x).max(2.0);
    span_y = (ls_max.y - ls_min.y).max(2.0);

    // Snap projection center in light-space texels for temporal stability.
    let wupt_x = (span_x / SHADOW_MAP_SIZE as f32).max(1.0e-6);
    let wupt_y = (span_y / SHADOW_MAP_SIZE as f32).max(1.0e-6);
    let center_ls_x = (ls_min.x + ls_max.x) * 0.5;
    let center_ls_y = (ls_min.y + ls_max.y) * 0.5;
    let snapped_ls_x = (center_ls_x / wupt_x).round() * wupt_x;
    let snapped_ls_y = (center_ls_y / wupt_y).round() * wupt_y;
    let center_delta =
        right_axis * (snapped_ls_x - center_ls_x) + up_axis * (snapped_ls_y - center_ls_y);
    focus_center += center_delta;
    eye += center_delta;
    view = Mat4::look_at_rh(eye, focus_center, up);

    let Some((mut ls_min, mut ls_max)) = light_space_bounds(&frustum_corners, view) else {
        return (
            shadow_scene,
            shadow_uniform,
            false,
            focus_center,
            focus_radius,
        );
    };
    let span_x = (ls_max.x - ls_min.x).max(2.0);
    let span_y = (ls_max.y - ls_min.y).max(2.0);
    let xy_pad = (span_x.max(span_y) * 0.08).max(2.0);
    ls_min.x -= xy_pad;
    ls_max.x += xy_pad;
    ls_min.y -= xy_pad;
    ls_max.y += xy_pad;

    let z_pad = (focus_radius * 0.45).max(12.0);
    let near = (-ls_max.z - z_pad).max(0.1);
    let far = (-ls_min.z + z_pad).max(near + 1.0);
    let proj = Mat4::orthographic_rh(ls_min.x, ls_max.x, ls_min.y, ls_max.y, near, far);
    let light_view_proj = proj * view;
    if !light_view_proj.is_finite() {
        return (
            shadow_scene,
            shadow_uniform,
            false,
            focus_center,
            focus_radius,
        );
    }

    shadow_scene.view_proj = light_view_proj.to_cols_array_2d();
    shadow_uniform.light_view_proj = shadow_scene.view_proj;
    // No falloff debug mode: very small constant bias for contact shadows.
    // params0 = [enabled, strength, depth_bias, normal_bias]
    shadow_uniform.params0 = [1.0, 1.0, 0.00002, 0.0];

    (
        shadow_scene,
        shadow_uniform,
        true,
        focus_center,
        focus_radius,
    )
}

pub(super) fn camera_frustum_corners_world(
    camera: &Camera3DState,
    width: u32,
    height: u32,
) -> Option<Vec<Vec3>> {
    let view_proj = compute_view_proj_mat(camera, width, height);
    if !view_proj.is_finite() {
        return None;
    }
    let inv = view_proj.inverse();
    if !inv.is_finite() {
        return None;
    }
    let mut corners = Vec::with_capacity(8);
    for z in [-1.0f32, 1.0f32] {
        for y in [-1.0f32, 1.0f32] {
            for x in [-1.0f32, 1.0f32] {
                let clip = Vec4::new(x, y, z, 1.0);
                let world_h = inv * clip;
                if !world_h.is_finite() || world_h.w.abs() <= 1.0e-6 {
                    return None;
                }
                corners.push(world_h.truncate() / world_h.w);
            }
        }
    }
    Some(corners)
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
        if batch.draw_on_top || !batch.casts_shadows {
            continue;
        }
        let start = batch.instance_start as usize;
        let end = (batch.instance_start + batch.instance_count) as usize;
        for inst in staged_instances.get(start..end).unwrap_or(&[]).iter() {
            let model_cols = model_cols_from_affine_rows(inst);
            let model = Mat4::from_cols_array_2d(&model_cols);
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
            let scale = sx.max(sy).max(sz).max(1.0e-6);
            let radius_world = (batch.local_radius.max(0.0) * scale).max(0.25);
            let r = Vec3::splat(radius_world);
            min = min.min(center_world - r);
            max = max.max(center_world + r);
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
