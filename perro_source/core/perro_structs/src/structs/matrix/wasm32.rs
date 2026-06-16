use super::{
    scalar_add_assign, scalar_add_assign_generic, scalar_scale_assign, scalar_scale_assign_generic,
    scalar_sub_assign, scalar_sub_assign_generic,
};
use std::arch::wasm32::{
    f32x4_add, f32x4_extract_lane, f32x4_mul, f32x4_splat, f32x4_sub, f64x2_add, f64x2_mul,
    f64x2_splat, f64x2_sub, i8x16_add, i8x16_sub, i16x8_add, i16x8_mul, i16x8_splat, i16x8_sub,
    i32x4_add, i32x4_mul, i32x4_splat, i32x4_sub, i64x2_add, i64x2_sub, v128, v128_load,
    v128_store,
};

macro_rules! impl_vec_binop {
    ($name:ident, $ty:ty, $lanes:expr, $op:ident, $tail:path) => {
        #[inline]
        pub(super) fn $name(out: &mut [$ty], rhs: &[$ty]) -> bool {
            let chunks = out.len() / $lanes;
            for i in 0..chunks {
                let offset = i * $lanes;
                unsafe {
                    let lhs_v = v128_load(out.as_ptr().add(offset).cast::<v128>());
                    let rhs_v = v128_load(rhs.as_ptr().add(offset).cast::<v128>());
                    v128_store(
                        out.as_mut_ptr().add(offset).cast::<v128>(),
                        $op(lhs_v, rhs_v),
                    );
                }
            }
            let tail = chunks * $lanes;
            $tail(&mut out[tail..], &rhs[tail..]);
            true
        }
    };
}

macro_rules! impl_vec_scale {
    ($name:ident, $ty:ty, $lanes:expr, $splat:ident, $mul:ident, $tail:path) => {
        #[inline]
        pub(super) fn $name(out: &mut [$ty], rhs: $ty) -> bool {
            let chunks = out.len() / $lanes;
            let rhs_v = $splat(rhs);
            for i in 0..chunks {
                let offset = i * $lanes;
                unsafe {
                    let lhs_v = v128_load(out.as_ptr().add(offset).cast::<v128>());
                    v128_store(
                        out.as_mut_ptr().add(offset).cast::<v128>(),
                        $mul(lhs_v, rhs_v),
                    );
                }
            }
            $tail(&mut out[(chunks * $lanes)..], rhs);
            true
        }
    };
}

impl_vec_binop!(try_add_assign_f32, f32, 4, f32x4_add, scalar_add_assign);
impl_vec_binop!(try_sub_assign_f32, f32, 4, f32x4_sub, scalar_sub_assign);
impl_vec_scale!(
    try_scale_assign_f32,
    f32,
    4,
    f32x4_splat,
    f32x4_mul,
    scalar_scale_assign
);

impl_vec_binop!(
    try_add_assign_f64,
    f64,
    2,
    f64x2_add,
    scalar_add_assign_generic
);
impl_vec_binop!(
    try_sub_assign_f64,
    f64,
    2,
    f64x2_sub,
    scalar_sub_assign_generic
);
impl_vec_scale!(
    try_scale_assign_f64,
    f64,
    2,
    f64x2_splat,
    f64x2_mul,
    scalar_scale_assign_generic
);

impl_vec_binop!(
    try_add_assign_i32,
    i32,
    4,
    i32x4_add,
    scalar_add_assign_generic
);
impl_vec_binop!(
    try_sub_assign_i32,
    i32,
    4,
    i32x4_sub,
    scalar_sub_assign_generic
);
impl_vec_scale!(
    try_scale_assign_i32,
    i32,
    4,
    i32x4_splat,
    i32x4_mul,
    scalar_scale_assign_generic
);

impl_vec_binop!(
    try_add_assign_i16,
    i16,
    8,
    i16x8_add,
    scalar_add_assign_generic
);
impl_vec_binop!(
    try_sub_assign_i16,
    i16,
    8,
    i16x8_sub,
    scalar_sub_assign_generic
);
impl_vec_scale!(
    try_scale_assign_i16,
    i16,
    8,
    i16x8_splat,
    i16x8_mul,
    scalar_scale_assign_generic
);

impl_vec_binop!(
    try_add_assign_i8,
    i8,
    16,
    i8x16_add,
    scalar_add_assign_generic
);
impl_vec_binop!(
    try_sub_assign_i8,
    i8,
    16,
    i8x16_sub,
    scalar_sub_assign_generic
);

impl_vec_binop!(
    try_add_assign_i64,
    i64,
    2,
    i64x2_add,
    scalar_add_assign_generic
);
impl_vec_binop!(
    try_sub_assign_i64,
    i64,
    2,
    i64x2_sub,
    scalar_sub_assign_generic
);

#[inline]
pub(super) fn try_dot_f32(lhs: &[f32], rhs: &[f32]) -> Option<f32> {
    let chunks = lhs.len() / 4;
    let mut acc = f32x4_splat(0.0);
    for i in 0..chunks {
        let offset = i * 4;
        unsafe {
            let lhs_v = v128_load(lhs.as_ptr().add(offset).cast::<v128>());
            let rhs_v = v128_load(rhs.as_ptr().add(offset).cast::<v128>());
            acc = f32x4_add(acc, f32x4_mul(lhs_v, rhs_v));
        }
    }

    let mut sum = f32x4_extract_lane::<0>(acc)
        + f32x4_extract_lane::<1>(acc)
        + f32x4_extract_lane::<2>(acc)
        + f32x4_extract_lane::<3>(acc);
    for i in (chunks * 4)..lhs.len() {
        sum += lhs[i] * rhs[i];
    }
    Some(sum)
}
