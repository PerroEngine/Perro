use super::{
    scalar_add_assign, scalar_add_assign_generic, scalar_scale_assign, scalar_scale_assign_generic,
    scalar_sub_assign, scalar_sub_assign_generic,
};
use std::arch::aarch64::{
    float32x4_t, float64x2_t, int8x16_t, int16x8_t, int32x4_t, int64x2_t, vaddq_f32, vaddq_f64,
    vaddq_s8, vaddq_s16, vaddq_s32, vaddq_s64, vdupq_n_f32, vdupq_n_f64, vdupq_n_s16, vdupq_n_s32,
    vld1q_f32, vld1q_f64, vld1q_s8, vld1q_s16, vld1q_s32, vld1q_s64, vmulq_f32, vmulq_f64,
    vmulq_s16, vmulq_s32, vst1q_f32, vst1q_f64, vst1q_s8, vst1q_s16, vst1q_s32, vst1q_s64,
    vsubq_f32, vsubq_f64, vsubq_s8, vsubq_s16, vsubq_s32, vsubq_s64,
};

macro_rules! impl_vec_binop {
    ($name:ident, $ty:ty, $lanes:expr, $vec:ty, $load:ident, $store:ident, $op:ident, $tail:path) => {
        #[inline]
        pub(super) fn $name(out: &mut [$ty], rhs: &[$ty]) -> bool {
            let chunks = out.len() / $lanes;
            for i in 0..chunks {
                let offset = i * $lanes;
                // SAFETY: chunk count keeps offset + lanes within both slices; NEON loads permit unaligned ptrs.
                unsafe {
                    let lhs_v: $vec = $load(out.as_ptr().add(offset));
                    let rhs_v: $vec = $load(rhs.as_ptr().add(offset));
                    $store(out.as_mut_ptr().add(offset), $op(lhs_v, rhs_v));
                }
            }
            let tail = chunks * $lanes;
            $tail(&mut out[tail..], &rhs[tail..]);
            true
        }
    };
}

macro_rules! impl_vec_scale {
    ($name:ident, $ty:ty, $lanes:expr, $vec:ty, $load:ident, $store:ident, $splat:ident, $mul:ident, $tail:path) => {
        #[inline]
        pub(super) fn $name(out: &mut [$ty], rhs: $ty) -> bool {
            let chunks = out.len() / $lanes;
            // SAFETY: duplicating a scalar into a NEON vector has no pointer or aliasing preconditions.
            let rhs_v: $vec = unsafe { $splat(rhs) };
            for i in 0..chunks {
                let offset = i * $lanes;
                // SAFETY: chunk count keeps offset + lanes within out; NEON loads permit unaligned ptrs.
                unsafe {
                    let lhs_v: $vec = $load(out.as_ptr().add(offset));
                    $store(out.as_mut_ptr().add(offset), $mul(lhs_v, rhs_v));
                }
            }
            $tail(&mut out[(chunks * $lanes)..], rhs);
            true
        }
    };
}

impl_vec_binop!(
    try_add_assign_f32,
    f32,
    4,
    float32x4_t,
    vld1q_f32,
    vst1q_f32,
    vaddq_f32,
    scalar_add_assign
);
impl_vec_binop!(
    try_sub_assign_f32,
    f32,
    4,
    float32x4_t,
    vld1q_f32,
    vst1q_f32,
    vsubq_f32,
    scalar_sub_assign
);
impl_vec_scale!(
    try_scale_assign_f32,
    f32,
    4,
    float32x4_t,
    vld1q_f32,
    vst1q_f32,
    vdupq_n_f32,
    vmulq_f32,
    scalar_scale_assign
);

impl_vec_binop!(
    try_add_assign_f64,
    f64,
    2,
    float64x2_t,
    vld1q_f64,
    vst1q_f64,
    vaddq_f64,
    scalar_add_assign_generic
);
impl_vec_binop!(
    try_sub_assign_f64,
    f64,
    2,
    float64x2_t,
    vld1q_f64,
    vst1q_f64,
    vsubq_f64,
    scalar_sub_assign_generic
);
impl_vec_scale!(
    try_scale_assign_f64,
    f64,
    2,
    float64x2_t,
    vld1q_f64,
    vst1q_f64,
    vdupq_n_f64,
    vmulq_f64,
    scalar_scale_assign_generic
);

impl_vec_binop!(
    try_add_assign_i32,
    i32,
    4,
    int32x4_t,
    vld1q_s32,
    vst1q_s32,
    vaddq_s32,
    scalar_add_assign_generic
);
impl_vec_binop!(
    try_sub_assign_i32,
    i32,
    4,
    int32x4_t,
    vld1q_s32,
    vst1q_s32,
    vsubq_s32,
    scalar_sub_assign_generic
);
impl_vec_scale!(
    try_scale_assign_i32,
    i32,
    4,
    int32x4_t,
    vld1q_s32,
    vst1q_s32,
    vdupq_n_s32,
    vmulq_s32,
    scalar_scale_assign_generic
);

impl_vec_binop!(
    try_add_assign_i16,
    i16,
    8,
    int16x8_t,
    vld1q_s16,
    vst1q_s16,
    vaddq_s16,
    scalar_add_assign_generic
);
impl_vec_binop!(
    try_sub_assign_i16,
    i16,
    8,
    int16x8_t,
    vld1q_s16,
    vst1q_s16,
    vsubq_s16,
    scalar_sub_assign_generic
);
impl_vec_scale!(
    try_scale_assign_i16,
    i16,
    8,
    int16x8_t,
    vld1q_s16,
    vst1q_s16,
    vdupq_n_s16,
    vmulq_s16,
    scalar_scale_assign_generic
);

impl_vec_binop!(
    try_add_assign_i8,
    i8,
    16,
    int8x16_t,
    vld1q_s8,
    vst1q_s8,
    vaddq_s8,
    scalar_add_assign_generic
);
impl_vec_binop!(
    try_sub_assign_i8,
    i8,
    16,
    int8x16_t,
    vld1q_s8,
    vst1q_s8,
    vsubq_s8,
    scalar_sub_assign_generic
);

impl_vec_binop!(
    try_add_assign_i64,
    i64,
    2,
    int64x2_t,
    vld1q_s64,
    vst1q_s64,
    vaddq_s64,
    scalar_add_assign_generic
);
impl_vec_binop!(
    try_sub_assign_i64,
    i64,
    2,
    int64x2_t,
    vld1q_s64,
    vst1q_s64,
    vsubq_s64,
    scalar_sub_assign_generic
);

#[inline]
pub(super) fn try_dot_f32(lhs: &[f32], rhs: &[f32]) -> Option<f32> {
    let chunks = lhs.len() / 4;
    let mut acc = [0.0; 4];
    for i in 0..chunks {
        let offset = i * 4;
        // SAFETY: chunk count keeps offset + 4 within both slices; acc has 4 lanes.
        unsafe {
            let lhs_v = vld1q_f32(lhs.as_ptr().add(offset));
            let rhs_v = vld1q_f32(rhs.as_ptr().add(offset));
            vst1q_f32(
                acc.as_mut_ptr(),
                vaddq_f32(vld1q_f32(acc.as_ptr()), vmulq_f32(lhs_v, rhs_v)),
            );
        }
    }
    let mut sum = acc[0] + acc[1] + acc[2] + acc[3];
    for i in (chunks * 4)..lhs.len() {
        sum += lhs[i] * rhs[i];
    }
    Some(sum)
}
