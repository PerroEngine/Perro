use super::*;

pub(super) fn compute_view_proj_mat(camera: &Camera3DState, width: u32, height: u32) -> Mat4 {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let aspect = w / h;

    let proj = projection_matrix(camera.projection, aspect);

    let pos = Vec3::from(camera.position);
    let rot_raw = Quat::from_xyzw(
        camera.rotation[0],
        camera.rotation[1],
        camera.rotation[2],
        camera.rotation[3],
    );
    let rot = if rot_raw.is_finite() && rot_raw.length_squared() > 1.0e-6 {
        rot_raw.normalize()
    } else {
        Quat::IDENTITY
    };
    let world = Mat4::from_rotation_translation(rot, pos);
    let view = world.inverse();
    proj * view
}

pub(super) fn projection_matrix(projection: CameraProjectionState, aspect: f32) -> Mat4 {
    match projection {
        CameraProjectionState::Perspective {
            fov_y_degrees,
            near,
            far,
        } => {
            let fov_y_radians = perspective_fov_y_radians(fov_y_degrees);
            let near = sanitize_near(near);
            let far = sanitize_far(far, near);
            Mat4::perspective_rh(fov_y_radians, aspect.max(1.0e-6), near, far)
        }
        CameraProjectionState::Orthographic { size, near, far } => {
            let half_h = if size.is_finite() {
                (size.abs() * 0.5).max(1.0e-3)
            } else {
                5.0
            };
            let half_w = half_h * aspect.max(1.0e-6);
            let near = sanitize_near(near);
            let far = sanitize_far(far, near);
            Mat4::orthographic_rh(-half_w, half_w, -half_h, half_h, near, far)
        }
        CameraProjectionState::Frustum {
            left,
            right,
            bottom,
            top,
            near,
            far,
        } => {
            let near = sanitize_near(near);
            let far = sanitize_far(far, near);
            let (left, right) = sanitize_range(left, right, -1.0, 1.0);
            let (bottom, top) = sanitize_range(bottom, top, -1.0, 1.0);
            Mat4::frustum_rh(left, right, bottom, top, near, far)
        }
    }
}

pub(super) fn projection_y_scale_from_projection(projection: CameraProjectionState) -> f32 {
    match projection {
        CameraProjectionState::Perspective { fov_y_degrees, .. } => {
            let fov_y_radians = perspective_fov_y_radians(fov_y_degrees);
            1.0 / (fov_y_radians * 0.5).tan().max(1.0e-6)
        }
        CameraProjectionState::Orthographic { size, .. } => {
            let half_h = if size.is_finite() {
                (size.abs() * 0.5).max(1.0e-3)
            } else {
                5.0
            };
            1.0 / half_h
        }
        CameraProjectionState::Frustum {
            bottom, top, near, ..
        } => {
            let near = sanitize_near(near);
            let (bottom, top) = sanitize_range(bottom, top, -1.0, 1.0);
            (2.0 * near / (top - bottom).abs().max(1.0e-6)).max(1.0e-6)
        }
    }
}

pub(super) fn perspective_fov_y_radians(fov_y_degrees: f32) -> f32 {
    if fov_y_degrees.is_finite() {
        fov_y_degrees
            .to_radians()
            .clamp(10.0f32.to_radians(), 120.0f32.to_radians())
    } else {
        60.0f32.to_radians()
    }
}

pub(super) fn sanitize_near(near: f32) -> f32 {
    if near.is_finite() {
        near.max(1.0e-3)
    } else {
        0.1
    }
}

pub(super) fn sanitize_far(far: f32, near: f32) -> f32 {
    if far.is_finite() {
        far.max(near + 1.0e-3)
    } else {
        (near + 1000.0).max(near + 1.0e-3)
    }
}

pub(super) fn sanitize_range(
    min: f32,
    max: f32,
    fallback_min: f32,
    fallback_max: f32,
) -> (f32, f32) {
    let mut a = if min.is_finite() { min } else { fallback_min };
    let mut b = if max.is_finite() { max } else { fallback_max };
    if (b - a).abs() < 1.0e-6 {
        a = fallback_min;
        b = fallback_max;
    }
    if b < a {
        std::mem::swap(&mut a, &mut b);
    }
    (a, b)
}

// Returns (gpu_occlusion_enabled, cpu_occlusion_enabled).
pub(super) fn occlusion_flags(mode: OcclusionCullingMode) -> (bool, bool) {
    match mode {
        OcclusionCullingMode::Cpu => (false, true),
        OcclusionCullingMode::Gpu => (true, false),
        OcclusionCullingMode::Off => (false, false),
    }
}

pub(super) fn extract_frustum_planes(view_proj: Mat4) -> [Vec4; 6] {
    let r0 = Vec4::new(
        view_proj.x_axis.x,
        view_proj.y_axis.x,
        view_proj.z_axis.x,
        view_proj.w_axis.x,
    );
    let r1 = Vec4::new(
        view_proj.x_axis.y,
        view_proj.y_axis.y,
        view_proj.z_axis.y,
        view_proj.w_axis.y,
    );
    let r2 = Vec4::new(
        view_proj.x_axis.z,
        view_proj.y_axis.z,
        view_proj.z_axis.z,
        view_proj.w_axis.z,
    );
    let r3 = Vec4::new(
        view_proj.x_axis.w,
        view_proj.y_axis.w,
        view_proj.z_axis.w,
        view_proj.w_axis.w,
    );
    [
        normalize_plane(r3 + r0),
        normalize_plane(r3 - r0),
        normalize_plane(r3 + r1),
        normalize_plane(r3 - r1),
        normalize_plane(r3 + r2),
        normalize_plane(r3 - r2),
    ]
}

#[inline]
pub(super) fn normalize_plane(plane: Vec4) -> Vec4 {
    let n = plane.truncate();
    let len = n.length();
    if len > 1.0e-6 && len.is_finite() {
        plane / len
    } else {
        plane
    }
}

pub(super) fn bounds_in_frustum(
    model: [[f32; 4]; 4],
    local_center: [f32; 3],
    local_radius: f32,
    planes: &[Vec4; 6],
) -> bool {
    let model = Mat4::from_cols_array_2d(&model);
    if !model.is_finite() {
        return false;
    }
    let center_local = Vec4::new(local_center[0], local_center[1], local_center[2], 1.0);
    let center_world = model * center_local;
    if !center_world.is_finite() {
        return false;
    }
    let sx = Vec3::new(model.x_axis.x, model.x_axis.y, model.x_axis.z).length();
    let sy = Vec3::new(model.y_axis.x, model.y_axis.y, model.y_axis.z).length();
    let sz = Vec3::new(model.z_axis.x, model.z_axis.y, model.z_axis.z).length();
    let scale = sx.max(sy).max(sz).max(1.0e-6);
    let radius_world = local_radius.max(0.0) * scale;
    let center = center_world.truncate();

    for plane in planes {
        let d = plane.truncate().dot(center) + plane.w;
        if d < -radius_world {
            return false;
        }
    }
    true
}

pub(super) fn build_scene_uniform(
    camera: &Camera3DState,
    lighting: &Lighting3DState,
    width: u32,
    height: u32,
) -> Scene3DUniform {
    let view_proj = compute_view_proj_mat(camera, width, height);
    let inv_view_proj = view_proj.inverse();
    let mut scene = Scene3DUniform {
        view_proj: view_proj.to_cols_array_2d(),
        ambient_and_counts: [0.0, 0.0, 0.0, 0.0],
        camera_pos: [
            camera.position[0],
            camera.position[1],
            camera.position[2],
            0.0,
        ],
        ambient_color: [1.0, 1.0, 1.0, 0.0],
        ray_light: RayLightGpu {
            direction: [0.0, 0.0, -1.0, 0.0],
            color_intensity: [1.0, 1.0, 1.0, 0.0],
        },
        ray_lights: [RayLightGpu {
            direction: [0.0, 0.0, -1.0, 0.0],
            color_intensity: [1.0, 1.0, 1.0, 0.0],
        }; MAX_RAY_LIGHTS],
        point_lights: [PointLightGpu {
            position_range: [0.0, 0.0, 0.0, 1.0],
            color_intensity: [0.0, 0.0, 0.0, 0.0],
        }; MAX_POINT_LIGHTS],
        spot_lights: [SpotLightGpu {
            position_range: [0.0, 0.0, 0.0, 1.0],
            direction_outer_cos: [0.0, 0.0, -1.0, -1.0],
            color_intensity: [0.0, 0.0, 0.0, 0.0],
            inner_cos_pad: [1.0, 0.0, 0.0, 0.0],
        }; MAX_SPOT_LIGHTS],
        inv_view_proj: if inv_view_proj.is_finite() {
            inv_view_proj.to_cols_array_2d()
        } else {
            Mat4::IDENTITY.to_cols_array_2d()
        },
    };

    if let Some(sky) = lighting.sky.as_ref() {
        let day_color = sample_gradient(sky.day_colors.as_ref(), 0.55);
        let evening_color = sample_gradient(sky.evening_colors.as_ref(), 0.55);
        let night_color = sample_gradient(sky.night_colors.as_ref(), 0.55);
        let t_day = day_weight_from_time(sky.time.time_of_day);
        let t_evening = evening_weight_from_time(sky.time.time_of_day);
        let ambient_rgb = lerp3(
            lerp3(night_color, day_color, t_day),
            evening_color,
            t_evening,
        );
        let ambient_strength = (0.08 + 0.32 * t_day).max(0.0);
        let ambient_lin = crate::srgb_to_linear_rgb(ambient_rgb);
        scene.ambient_color = [
            ambient_lin[0].max(0.0),
            ambient_lin[1].max(0.0),
            ambient_lin[2].max(0.0),
            ambient_strength,
        ];
    }

    // Zero-intensity ambient nodes must not wipe out sky-derived ambient.
    if let Some(ambient) = lighting.ambient_light.filter(|a| a.intensity > 0.0) {
        let ambient_lin = crate::srgb_to_linear_rgb(ambient.color);
        scene.ambient_color = [
            ambient_lin[0],
            ambient_lin[1],
            ambient_lin[2],
            ambient.intensity.max(0.0),
        ];
    }

    let mut ray_count = 0usize;
    let mut push_ray = |dir: Vec3, color: [f32; 3], intensity: f32| {
        if ray_count >= MAX_RAY_LIGHTS {
            return;
        }
        if intensity <= 1.0e-4 {
            return;
        }
        let d = dir.normalize_or_zero();
        if d.length_squared() <= 1.0e-6 || !d.is_finite() {
            return;
        }
        let color_lin = crate::srgb_to_linear_rgb(color);
        scene.ray_lights[ray_count] = RayLightGpu {
            direction: [d.x, d.y, d.z, 0.0],
            color_intensity: [
                color_lin[0],
                color_lin[1],
                color_lin[2],
                intensity.max(0.0),
            ],
        };
        ray_count += 1;
    };

    if DEBUG_FORCE_WORLD_SUN_DIR {
        let d = Vec3::new(
            DEBUG_WORLD_SUN_DIR[0],
            DEBUG_WORLD_SUN_DIR[1],
            DEBUG_WORLD_SUN_DIR[2],
        )
        .normalize_or_zero();
        push_ray(d, [1.0, 0.98, 0.92], 1.0);
    }

    // Prefer authored directional lights when present.
    if !DEBUG_FORCE_WORLD_SUN_DIR {
        for ray in lighting.ray_lights.iter().flatten() {
            if !ray.cast_shadows {
                continue;
            }
            push_ray(Vec3::from(ray.direction), ray.color, ray.intensity);
        }
        for ray in lighting.ray_lights.iter().flatten() {
            if ray.cast_shadows {
                continue;
            }
            push_ray(Vec3::from(ray.direction), ray.color, ray.intensity);
        }
    }

    scene.ambient_and_counts[0] = ray_count as f32;
    scene.ambient_and_counts[3] = if ray_count > 0 { 1.0 } else { 0.0 };
    if ray_count > 0 {
        scene.ray_light = scene.ray_lights[0];
    }

    let mut point_count = 0.0f32;
    for (dst, src) in scene
        .point_lights
        .iter_mut()
        .zip(lighting.point_lights.iter().flatten())
    {
        dst.position_range = [
            src.position[0],
            src.position[1],
            src.position[2],
            src.range.max(0.001),
        ];
        let color_lin = crate::srgb_to_linear_rgb(src.color);
        dst.color_intensity = [
            color_lin[0],
            color_lin[1],
            color_lin[2],
            src.intensity.max(0.0),
        ];
        point_count += 1.0;
    }
    scene.ambient_and_counts[1] = point_count;

    let mut spot_count = 0.0f32;
    for (dst, src) in scene
        .spot_lights
        .iter_mut()
        .zip(lighting.spot_lights.iter().flatten())
    {
        let dir = Vec3::from(src.direction).normalize_or_zero();
        // Clamp to the same range as the spot shadow frustum
        // (spot_light_view_proj) so the lit cone never outgrows the map.
        let outer = src
            .outer_angle_radians
            .clamp(0.01, std::f32::consts::FRAC_PI_2);
        let inner = src.inner_angle_radians.clamp(0.0, outer - 1.0e-4);
        dst.position_range = [
            src.position[0],
            src.position[1],
            src.position[2],
            src.range.max(0.001),
        ];
        dst.direction_outer_cos = [dir.x, dir.y, dir.z, outer.cos()];
        let color_lin = crate::srgb_to_linear_rgb(src.color);
        dst.color_intensity = [
            color_lin[0],
            color_lin[1],
            color_lin[2],
            src.intensity.max(0.0),
        ];
        dst.inner_cos_pad = [inner.cos(), 0.0, 0.0, 0.0];
        spot_count += 1.0;
    }
    scene.ambient_and_counts[2] = spot_count;

    scene
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_uniform_no_sky_or_lights_has_no_ambient_or_light_counts() {
        let scene = build_scene_uniform(
            &Camera3DState::default(),
            &Lighting3DState::default(),
            1280,
            720,
        );

        assert_eq!(scene.ambient_color, [1.0, 1.0, 1.0, 0.0]);
        assert_eq!(scene.ambient_and_counts, [0.0, 0.0, 0.0, 0.0]);
        assert_eq!(scene.ray_light.color_intensity[3], 0.0);
    }
}
