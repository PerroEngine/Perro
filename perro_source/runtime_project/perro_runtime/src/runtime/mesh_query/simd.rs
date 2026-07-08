use glam::Vec3;

#[inline]
pub(super) fn aabb_distance2(p: Vec3, min: Vec3, max: Vec3) -> f32 {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::has_sse() {
        // SAFETY: runtime feature check gates SSE use.
        return unsafe { x86::aabb_distance2(p, min, max) };
    }

    #[cfg(target_arch = "aarch64")]
    {
        aarch64::aabb_distance2(p, min, max)
    }

    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    {
        wasm32::aabb_distance2(p, min, max)
    }

    #[cfg(not(any(
        target_arch = "aarch64",
        all(target_arch = "wasm32", target_feature = "simd128")
    )))]
    scalar_aabb_distance2(p, min, max)
}

#[inline]
pub(super) fn ray_aabb_tmin(
    origin: Vec3,
    dir: Vec3,
    min: Vec3,
    max: Vec3,
    max_t: f32,
) -> Option<f32> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::has_sse() {
        // SAFETY: runtime feature check gates SSE use.
        return unsafe { x86::ray_aabb_tmin(origin, dir, min, max, max_t) };
    }

    #[cfg(target_arch = "aarch64")]
    {
        aarch64::ray_aabb_tmin(origin, dir, min, max, max_t)
    }

    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    {
        wasm32::ray_aabb_tmin(origin, dir, min, max, max_t)
    }

    #[cfg(not(any(
        target_arch = "aarch64",
        all(target_arch = "wasm32", target_feature = "simd128")
    )))]
    scalar_ray_aabb_tmin(origin, dir, min, max, max_t)
}

#[inline]
#[cfg(not(any(
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
)))]
fn scalar_aabb_distance2(p: Vec3, min: Vec3, max: Vec3) -> f32 {
    let dx = if p.x < min.x {
        min.x - p.x
    } else if p.x > max.x {
        p.x - max.x
    } else {
        0.0
    };
    let dy = if p.y < min.y {
        min.y - p.y
    } else if p.y > max.y {
        p.y - max.y
    } else {
        0.0
    };
    let dz = if p.z < min.z {
        min.z - p.z
    } else if p.z > max.z {
        p.z - max.z
    } else {
        0.0
    };
    dx * dx + dy * dy + dz * dz
}

#[inline]
#[cfg(not(any(
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
)))]
fn scalar_ray_aabb_tmin(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3, max_t: f32) -> Option<f32> {
    let inv_x = if dir.x.abs() > 1e-8 {
        1.0 / dir.x
    } else {
        f32::INFINITY
    };
    let inv_y = if dir.y.abs() > 1e-8 {
        1.0 / dir.y
    } else {
        f32::INFINITY
    };
    let inv_z = if dir.z.abs() > 1e-8 {
        1.0 / dir.z
    } else {
        f32::INFINITY
    };

    let mut t1 = (min.x - origin.x) * inv_x;
    let mut t2 = (max.x - origin.x) * inv_x;
    if t1 > t2 {
        std::mem::swap(&mut t1, &mut t2);
    }
    let mut tmin = t1;
    let mut tmax = t2;

    t1 = (min.y - origin.y) * inv_y;
    t2 = (max.y - origin.y) * inv_y;
    if t1 > t2 {
        std::mem::swap(&mut t1, &mut t2);
    }
    tmin = tmin.max(t1);
    tmax = tmax.min(t2);

    t1 = (min.z - origin.z) * inv_z;
    t2 = (max.z - origin.z) * inv_z;
    if t1 > t2 {
        std::mem::swap(&mut t1, &mut t2);
    }
    tmin = tmin.max(t1);
    tmax = tmax.min(t2);

    if tmax < 0.0 || tmin > tmax || tmin > max_t {
        None
    } else {
        Some(tmin.max(0.0))
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[allow(dead_code)]
mod x86 {
    use glam::Vec3;

    #[cfg(target_arch = "x86")]
    use std::arch::x86::{
        _mm_add_ps, _mm_and_ps, _mm_blendv_ps, _mm_cmpgt_ps, _mm_div_ps, _mm_max_ps, _mm_min_ps,
        _mm_mul_ps, _mm_set_ps, _mm_set1_ps, _mm_storeu_ps, _mm_sub_ps,
    };

    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::{
        _mm_add_ps, _mm_and_ps, _mm_blendv_ps, _mm_cmpgt_ps, _mm_div_ps, _mm_max_ps, _mm_min_ps,
        _mm_mul_ps, _mm_set_ps, _mm_set1_ps, _mm_storeu_ps, _mm_sub_ps,
    };

    #[inline]
    pub(super) fn has_sse() -> bool {
        std::is_x86_feature_detected!("sse4.1")
    }

    #[target_feature(enable = "sse4.1")]
    pub(super) unsafe fn aabb_distance2(p: Vec3, min: Vec3, max: Vec3) -> f32 {
        let p_v = _mm_set_ps(0.0, p.z, p.y, p.x);
        let min_v = _mm_set_ps(0.0, min.z, min.y, min.x);
        let max_v = _mm_set_ps(0.0, max.z, max.y, max.x);
        let zero = _mm_set1_ps(0.0);
        let below = _mm_max_ps(_mm_sub_ps(min_v, p_v), zero);
        let above = _mm_max_ps(_mm_sub_ps(p_v, max_v), zero);
        let d = _mm_add_ps(below, above);
        let sq = _mm_mul_ps(d, d);
        let mut lanes = [0.0; 4];
        // SAFETY: lanes has four f32 slots and storeu permits unaligned writes.
        unsafe {
            _mm_storeu_ps(lanes.as_mut_ptr(), sq);
        }
        lanes[0] + lanes[1] + lanes[2]
    }

    #[target_feature(enable = "sse4.1")]
    pub(super) unsafe fn ray_aabb_tmin(
        origin: Vec3,
        dir: Vec3,
        min: Vec3,
        max: Vec3,
        max_t: f32,
    ) -> Option<f32> {
        let origin_v = _mm_set_ps(0.0, origin.z, origin.y, origin.x);
        let dir_v = _mm_set_ps(1.0, dir.z, dir.y, dir.x);
        let min_v = _mm_set_ps(0.0, min.z, min.y, min.x);
        let max_v = _mm_set_ps(0.0, max.z, max.y, max.x);
        let abs_dir = _mm_and_ps(dir_v, _mm_set1_ps(f32::from_bits(0x7fff_ffff)));
        let nonzero = _mm_cmpgt_ps(abs_dir, _mm_set1_ps(1.0e-8));
        let inv = _mm_blendv_ps(
            _mm_set1_ps(f32::INFINITY),
            _mm_div_ps(_mm_set1_ps(1.0), dir_v),
            nonzero,
        );
        let ta = _mm_mul_ps(_mm_sub_ps(min_v, origin_v), inv);
        let tb = _mm_mul_ps(_mm_sub_ps(max_v, origin_v), inv);
        let t1 = _mm_min_ps(ta, tb);
        let t2 = _mm_max_ps(ta, tb);
        let mut near = [0.0; 4];
        let mut far = [0.0; 4];
        // SAFETY: near/far each have four f32 slots and storeu permits unaligned writes.
        unsafe {
            _mm_storeu_ps(near.as_mut_ptr(), t1);
            _mm_storeu_ps(far.as_mut_ptr(), t2);
        }
        let tmin = near[0].max(near[1]).max(near[2]);
        let tmax = far[0].min(far[1]).min(far[2]);
        if tmax < 0.0 || tmin > tmax || tmin > max_t {
            None
        } else {
            Some(tmin.max(0.0))
        }
    }
}

#[cfg(target_arch = "aarch64")]
#[allow(dead_code)]
mod aarch64 {
    use glam::Vec3;
    use std::arch::aarch64::{
        float32x4_t, vabsq_f32, vaddq_f32, vaddvq_f32, vbslq_f32, vcgtq_f32, vdivq_f32,
        vdupq_n_f32, vgetq_lane_f32, vld1q_f32, vmaxq_f32, vminq_f32, vmulq_f32, vsubq_f32,
    };

    #[inline]
    fn vec4(v: Vec3, w: f32) -> float32x4_t {
        let lanes = [v.x, v.y, v.z, w];
        // SAFETY: lanes has four f32 slots; NEON load reads exactly that many values.
        unsafe { vld1q_f32(lanes.as_ptr()) }
    }

    #[inline]
    pub(super) fn aabb_distance2(p: Vec3, min: Vec3, max: Vec3) -> f32 {
        // SAFETY: NEON intrinsics operate on local values; vec4 builds valid lane vectors.
        unsafe {
            let p_v = vec4(p, 0.0);
            let min_v = vec4(min, 0.0);
            let max_v = vec4(max, 0.0);
            let zero = vdupq_n_f32(0.0);
            let below = vmaxq_f32(vsubq_f32(min_v, p_v), zero);
            let above = vmaxq_f32(vsubq_f32(p_v, max_v), zero);
            let d = vaddq_f32(below, above);
            vaddvq_f32(vmulq_f32(d, d))
        }
    }

    #[inline]
    pub(super) fn ray_aabb_tmin(
        origin: Vec3,
        dir: Vec3,
        min: Vec3,
        max: Vec3,
        max_t: f32,
    ) -> Option<f32> {
        // SAFETY: NEON intrinsics operate on local values; vec4 builds valid lane vectors.
        unsafe {
            let origin_v = vec4(origin, 0.0);
            let dir_v = vec4(dir, 1.0);
            let min_v = vec4(min, 0.0);
            let max_v = vec4(max, 0.0);
            let mask = vcgtq_f32(vabsq_f32(dir_v), vdupq_n_f32(1.0e-8));
            let inv = vbslq_f32(
                mask,
                vdivq_f32(vdupq_n_f32(1.0), dir_v),
                vdupq_n_f32(f32::INFINITY),
            );
            let ta = vmulq_f32(vsubq_f32(min_v, origin_v), inv);
            let tb = vmulq_f32(vsubq_f32(max_v, origin_v), inv);
            let near = vminq_f32(ta, tb);
            let far = vmaxq_f32(ta, tb);
            let tmin = vgetq_lane_f32::<0>(near)
                .max(vgetq_lane_f32::<1>(near))
                .max(vgetq_lane_f32::<2>(near));
            let tmax = vgetq_lane_f32::<0>(far)
                .min(vgetq_lane_f32::<1>(far))
                .min(vgetq_lane_f32::<2>(far));
            if tmax < 0.0 || tmin > tmax || tmin > max_t {
                None
            } else {
                Some(tmin.max(0.0))
            }
        }
    }
}

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
#[allow(dead_code)]
mod wasm32 {
    use glam::Vec3;
    use std::arch::wasm32::{
        f32x4_abs, f32x4_add, f32x4_div, f32x4_extract_lane, f32x4_gt, f32x4_max, f32x4_min,
        f32x4_mul, f32x4_replace_lane, f32x4_splat, f32x4_sub, v128, v128_bitselect,
    };

    #[inline]
    fn vec4(v: Vec3, w: f32) -> v128 {
        let out = f32x4_replace_lane::<0>(f32x4_splat(w), v.x);
        let out = f32x4_replace_lane::<1>(out, v.y);
        f32x4_replace_lane::<2>(out, v.z)
    }

    #[inline]
    pub(super) fn aabb_distance2(p: Vec3, min: Vec3, max: Vec3) -> f32 {
        let p_v = vec4(p, 0.0);
        let min_v = vec4(min, 0.0);
        let max_v = vec4(max, 0.0);
        let zero = f32x4_splat(0.0);
        let below = f32x4_max(f32x4_sub(min_v, p_v), zero);
        let above = f32x4_max(f32x4_sub(p_v, max_v), zero);
        let d = f32x4_add(below, above);
        let sq = f32x4_mul(d, d);
        f32x4_extract_lane::<0>(sq) + f32x4_extract_lane::<1>(sq) + f32x4_extract_lane::<2>(sq)
    }

    #[inline]
    pub(super) fn ray_aabb_tmin(
        origin: Vec3,
        dir: Vec3,
        min: Vec3,
        max: Vec3,
        max_t: f32,
    ) -> Option<f32> {
        let origin_v = vec4(origin, 0.0);
        let dir_v = vec4(dir, 1.0);
        let min_v = vec4(min, 0.0);
        let max_v = vec4(max, 0.0);
        let mask = f32x4_gt(f32x4_abs(dir_v), f32x4_splat(1.0e-8));
        let inv = v128_bitselect(
            f32x4_div(f32x4_splat(1.0), dir_v),
            f32x4_splat(f32::INFINITY),
            mask,
        );
        let ta = f32x4_mul(f32x4_sub(min_v, origin_v), inv);
        let tb = f32x4_mul(f32x4_sub(max_v, origin_v), inv);
        let near = f32x4_min(ta, tb);
        let far = f32x4_max(ta, tb);
        let tmin = f32x4_extract_lane::<0>(near)
            .max(f32x4_extract_lane::<1>(near))
            .max(f32x4_extract_lane::<2>(near));
        let tmax = f32x4_extract_lane::<0>(far)
            .min(f32x4_extract_lane::<1>(far))
            .min(f32x4_extract_lane::<2>(far));
        if tmax < 0.0 || tmin > tmax || tmin > max_t {
            None
        } else {
            Some(tmin.max(0.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simd_aabb_math_matches_expected() {
        let p = Vec3::new(3.0, -2.0, 0.5);
        let min = Vec3::new(0.0, 0.0, -1.0);
        let max = Vec3::new(2.0, 1.0, 1.0);
        assert_eq!(aabb_distance2(p, min, max), 5.0);
    }

    #[test]
    fn simd_ray_aabb_math_matches_expected() {
        let origin = Vec3::new(0.0, 2.0, 0.0);
        let dir = Vec3::new(0.0, -1.0, 0.0);
        let min = Vec3::new(-1.0, -1.0, -1.0);
        let max = Vec3::new(1.0, 1.0, 1.0);
        assert_eq!(ray_aabb_tmin(origin, dir, min, max, 10.0), Some(1.0));
        assert_eq!(ray_aabb_tmin(origin, dir, min, max, 0.5), None);
    }
}
