use super::{
    scalar_add_assign, scalar_add_assign_generic, scalar_scale_assign, scalar_scale_assign_generic,
    scalar_sub_assign, scalar_sub_assign_generic,
};

#[cfg(target_arch = "x86")]
use std::arch::x86::{
    __m128i, _mm_add_epi8, _mm_add_epi16, _mm_add_epi32, _mm_add_epi64, _mm_add_pd, _mm_add_ps,
    _mm_loadu_pd, _mm_loadu_ps, _mm_loadu_si128, _mm_mul_pd, _mm_mul_ps, _mm_mullo_epi16,
    _mm_mullo_epi32, _mm_set1_epi16, _mm_set1_epi32, _mm_set1_pd, _mm_set1_ps, _mm_setzero_ps,
    _mm_storeu_pd, _mm_storeu_ps, _mm_storeu_si128, _mm_sub_epi8, _mm_sub_epi16, _mm_sub_epi32,
    _mm_sub_epi64, _mm_sub_pd, _mm_sub_ps,
};

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::{
    __m128i, _mm_add_epi8, _mm_add_epi16, _mm_add_epi32, _mm_add_epi64, _mm_add_pd, _mm_add_ps,
    _mm_loadu_pd, _mm_loadu_ps, _mm_loadu_si128, _mm_mul_pd, _mm_mul_ps, _mm_mullo_epi16,
    _mm_mullo_epi32, _mm_set1_epi16, _mm_set1_epi32, _mm_set1_pd, _mm_set1_ps, _mm_setzero_ps,
    _mm_storeu_pd, _mm_storeu_ps, _mm_storeu_si128, _mm_sub_epi8, _mm_sub_epi16, _mm_sub_epi32,
    _mm_sub_epi64, _mm_sub_pd, _mm_sub_ps,
};

macro_rules! impl_try_binop {
    ($try_name:ident, $ty:ty, sse, $helper:ident) => {
        #[inline]
        pub(super) fn $try_name(out: &mut [$ty], rhs: &[$ty]) -> bool {
            if std::is_x86_feature_detected!("sse") {
                // SAFETY: runtime feature check gates helper use; helper keeps ptr math in bounds.
                unsafe { $helper(out, rhs) };
                return true;
            }
            false
        }
    };
    ($try_name:ident, $ty:ty, sse2, $helper:ident) => {
        #[inline]
        pub(super) fn $try_name(out: &mut [$ty], rhs: &[$ty]) -> bool {
            if std::is_x86_feature_detected!("sse2") {
                // SAFETY: runtime feature check gates helper use; helper keeps ptr math in bounds.
                unsafe { $helper(out, rhs) };
                return true;
            }
            false
        }
    };
}

macro_rules! impl_try_scale {
    ($try_name:ident, $ty:ty, sse, $helper:ident) => {
        #[inline]
        pub(super) fn $try_name(out: &mut [$ty], rhs: $ty) -> bool {
            if std::is_x86_feature_detected!("sse") {
                // SAFETY: runtime feature check gates helper use; helper keeps ptr math in bounds.
                unsafe { $helper(out, rhs) };
                return true;
            }
            false
        }
    };
    ($try_name:ident, $ty:ty, sse2, $helper:ident) => {
        #[inline]
        pub(super) fn $try_name(out: &mut [$ty], rhs: $ty) -> bool {
            if std::is_x86_feature_detected!("sse2") {
                // SAFETY: runtime feature check gates helper use; helper keeps ptr math in bounds.
                unsafe { $helper(out, rhs) };
                return true;
            }
            false
        }
    };
    ($try_name:ident, $ty:ty, sse41, $helper:ident) => {
        #[inline]
        pub(super) fn $try_name(out: &mut [$ty], rhs: $ty) -> bool {
            if std::is_x86_feature_detected!("sse4.1") {
                // SAFETY: runtime feature check gates helper use; helper keeps ptr math in bounds.
                unsafe { $helper(out, rhs) };
                return true;
            }
            false
        }
    };
}

macro_rules! impl_float_binop {
    ($name:ident, $ty:ty, $lanes:expr, $target:literal, $load:ident, $store:ident, $op:ident, $tail:path) => {
        #[target_feature(enable = $target)]
        unsafe fn $name(out: &mut [$ty], rhs: &[$ty]) {
            let chunks = out.len() / $lanes;
            for i in 0..chunks {
                let offset = i * $lanes;
                unsafe {
                    let lhs_v = $load(out.as_ptr().add(offset));
                    let rhs_v = $load(rhs.as_ptr().add(offset));
                    $store(out.as_mut_ptr().add(offset), $op(lhs_v, rhs_v));
                }
            }
            let tail = chunks * $lanes;
            $tail(&mut out[tail..], &rhs[tail..]);
        }
    };
}

macro_rules! impl_float_scale {
    ($name:ident, $ty:ty, $lanes:expr, $target:literal, $load:ident, $store:ident, $splat:ident, $op:ident, $tail:path) => {
        #[target_feature(enable = $target)]
        unsafe fn $name(out: &mut [$ty], rhs: $ty) {
            let chunks = out.len() / $lanes;
            let rhs_v = $splat(rhs);
            for i in 0..chunks {
                let offset = i * $lanes;
                unsafe {
                    let lhs_v = $load(out.as_ptr().add(offset));
                    $store(out.as_mut_ptr().add(offset), $op(lhs_v, rhs_v));
                }
            }
            $tail(&mut out[(chunks * $lanes)..], rhs);
        }
    };
}

macro_rules! impl_int_binop {
    ($name:ident, $ty:ty, $lanes:expr, $target:literal, $op:ident, $tail:path) => {
        #[target_feature(enable = $target)]
        unsafe fn $name(out: &mut [$ty], rhs: &[$ty]) {
            let chunks = out.len() / $lanes;
            for i in 0..chunks {
                let offset = i * $lanes;
                unsafe {
                    let lhs_v = _mm_loadu_si128(out.as_ptr().add(offset).cast::<__m128i>());
                    let rhs_v = _mm_loadu_si128(rhs.as_ptr().add(offset).cast::<__m128i>());
                    _mm_storeu_si128(
                        out.as_mut_ptr().add(offset).cast::<__m128i>(),
                        $op(lhs_v, rhs_v),
                    );
                }
            }
            let tail = chunks * $lanes;
            $tail(&mut out[tail..], &rhs[tail..]);
        }
    };
}

macro_rules! impl_int_scale {
    ($name:ident, $ty:ty, $lanes:expr, $target:literal, $splat:ident, $op:ident, $tail:path) => {
        #[target_feature(enable = $target)]
        unsafe fn $name(out: &mut [$ty], rhs: $ty) {
            let chunks = out.len() / $lanes;
            let rhs_v = $splat(rhs);
            for i in 0..chunks {
                let offset = i * $lanes;
                unsafe {
                    let lhs_v = _mm_loadu_si128(out.as_ptr().add(offset).cast::<__m128i>());
                    _mm_storeu_si128(
                        out.as_mut_ptr().add(offset).cast::<__m128i>(),
                        $op(lhs_v, rhs_v),
                    );
                }
            }
            $tail(&mut out[(chunks * $lanes)..], rhs);
        }
    };
}

impl_try_binop!(try_add_assign_f32, f32, sse, simd_add_assign_sse);
impl_try_binop!(try_sub_assign_f32, f32, sse, simd_sub_assign_sse);
impl_try_scale!(try_scale_assign_f32, f32, sse, simd_scale_assign_sse);

impl_try_binop!(try_add_assign_f64, f64, sse2, simd_add_assign_f64_sse2);
impl_try_binop!(try_sub_assign_f64, f64, sse2, simd_sub_assign_f64_sse2);
impl_try_scale!(try_scale_assign_f64, f64, sse2, simd_scale_assign_f64_sse2);

impl_try_binop!(try_add_assign_i32, i32, sse2, simd_add_assign_i32_sse2);
impl_try_binop!(try_sub_assign_i32, i32, sse2, simd_sub_assign_i32_sse2);
impl_try_scale!(
    try_scale_assign_i32,
    i32,
    sse41,
    simd_scale_assign_i32_sse41
);

impl_try_binop!(try_add_assign_i16, i16, sse2, simd_add_assign_i16_sse2);
impl_try_binop!(try_sub_assign_i16, i16, sse2, simd_sub_assign_i16_sse2);
impl_try_scale!(try_scale_assign_i16, i16, sse2, simd_scale_assign_i16_sse2);

impl_try_binop!(try_add_assign_i8, i8, sse2, simd_add_assign_i8_sse2);
impl_try_binop!(try_sub_assign_i8, i8, sse2, simd_sub_assign_i8_sse2);

impl_try_binop!(try_add_assign_i64, i64, sse2, simd_add_assign_i64_sse2);
impl_try_binop!(try_sub_assign_i64, i64, sse2, simd_sub_assign_i64_sse2);

impl_float_binop!(
    simd_add_assign_sse,
    f32,
    4,
    "sse",
    _mm_loadu_ps,
    _mm_storeu_ps,
    _mm_add_ps,
    scalar_add_assign
);
impl_float_binop!(
    simd_sub_assign_sse,
    f32,
    4,
    "sse",
    _mm_loadu_ps,
    _mm_storeu_ps,
    _mm_sub_ps,
    scalar_sub_assign
);
impl_float_scale!(
    simd_scale_assign_sse,
    f32,
    4,
    "sse",
    _mm_loadu_ps,
    _mm_storeu_ps,
    _mm_set1_ps,
    _mm_mul_ps,
    scalar_scale_assign
);

impl_float_binop!(
    simd_add_assign_f64_sse2,
    f64,
    2,
    "sse2",
    _mm_loadu_pd,
    _mm_storeu_pd,
    _mm_add_pd,
    scalar_add_assign_generic
);
impl_float_binop!(
    simd_sub_assign_f64_sse2,
    f64,
    2,
    "sse2",
    _mm_loadu_pd,
    _mm_storeu_pd,
    _mm_sub_pd,
    scalar_sub_assign_generic
);
impl_float_scale!(
    simd_scale_assign_f64_sse2,
    f64,
    2,
    "sse2",
    _mm_loadu_pd,
    _mm_storeu_pd,
    _mm_set1_pd,
    _mm_mul_pd,
    scalar_scale_assign_generic
);

impl_int_binop!(
    simd_add_assign_i32_sse2,
    i32,
    4,
    "sse2",
    _mm_add_epi32,
    scalar_add_assign_generic
);
impl_int_binop!(
    simd_sub_assign_i32_sse2,
    i32,
    4,
    "sse2",
    _mm_sub_epi32,
    scalar_sub_assign_generic
);
impl_int_scale!(
    simd_scale_assign_i32_sse41,
    i32,
    4,
    "sse4.1",
    _mm_set1_epi32,
    _mm_mullo_epi32,
    scalar_scale_assign_generic
);

impl_int_binop!(
    simd_add_assign_i16_sse2,
    i16,
    8,
    "sse2",
    _mm_add_epi16,
    scalar_add_assign_generic
);
impl_int_binop!(
    simd_sub_assign_i16_sse2,
    i16,
    8,
    "sse2",
    _mm_sub_epi16,
    scalar_sub_assign_generic
);
impl_int_scale!(
    simd_scale_assign_i16_sse2,
    i16,
    8,
    "sse2",
    _mm_set1_epi16,
    _mm_mullo_epi16,
    scalar_scale_assign_generic
);

impl_int_binop!(
    simd_add_assign_i8_sse2,
    i8,
    16,
    "sse2",
    _mm_add_epi8,
    scalar_add_assign_generic
);
impl_int_binop!(
    simd_sub_assign_i8_sse2,
    i8,
    16,
    "sse2",
    _mm_sub_epi8,
    scalar_sub_assign_generic
);

impl_int_binop!(
    simd_add_assign_i64_sse2,
    i64,
    2,
    "sse2",
    _mm_add_epi64,
    scalar_add_assign_generic
);
impl_int_binop!(
    simd_sub_assign_i64_sse2,
    i64,
    2,
    "sse2",
    _mm_sub_epi64,
    scalar_sub_assign_generic
);

#[inline]
pub(super) fn try_dot_f32(lhs: &[f32], rhs: &[f32]) -> Option<f32> {
    if std::is_x86_feature_detected!("sse") {
        // SAFETY: runtime feature check gates SSE use; helper only reads in-bounds lanes.
        return Some(unsafe { simd_dot_f32_sse(lhs, rhs) });
    }
    None
}

#[target_feature(enable = "sse")]
unsafe fn simd_dot_f32_sse(lhs: &[f32], rhs: &[f32]) -> f32 {
    let chunks = lhs.len() / 4;
    let mut acc = _mm_setzero_ps();
    for i in 0..chunks {
        let offset = i * 4;
        unsafe {
            let lhs_v = _mm_loadu_ps(lhs.as_ptr().add(offset));
            let rhs_v = _mm_loadu_ps(rhs.as_ptr().add(offset));
            acc = _mm_add_ps(acc, _mm_mul_ps(lhs_v, rhs_v));
        }
    }

    let mut lanes = [0.0; 4];
    unsafe {
        _mm_storeu_ps(lanes.as_mut_ptr(), acc);
    }
    let mut sum = lanes[0] + lanes[1] + lanes[2] + lanes[3];
    for i in (chunks * 4)..lhs.len() {
        sum += lhs[i] * rhs[i];
    }
    sum
}
