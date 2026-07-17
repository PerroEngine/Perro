use super::*;

#[inline]
pub(super) fn simd_add_assign(out: &mut [f32], rhs: &[f32]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_add_assign_f32(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_add_assign_f32(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_add_assign_f32(out, rhs) {
        return;
    }
    scalar_add_assign(out, rhs);
}

#[inline]
pub(super) fn simd_sub_assign(out: &mut [f32], rhs: &[f32]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_sub_assign_f32(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_sub_assign_f32(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_sub_assign_f32(out, rhs) {
        return;
    }
    scalar_sub_assign(out, rhs);
}

#[inline]
pub(super) fn simd_scale_assign(out: &mut [f32], rhs: f32) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_scale_assign_f32(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_scale_assign_f32(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_scale_assign_f32(out, rhs) {
        return;
    }
    scalar_scale_assign(out, rhs);
}

#[inline]
pub(super) fn scalar_add_assign(out: &mut [f32], rhs: &[f32]) {
    for (dst, src) in out.iter_mut().zip(rhs) {
        *dst += *src;
    }
}

#[inline]
pub(super) fn scalar_sub_assign(out: &mut [f32], rhs: &[f32]) {
    for (dst, src) in out.iter_mut().zip(rhs) {
        *dst -= *src;
    }
}

#[inline]
pub(super) fn scalar_scale_assign(out: &mut [f32], rhs: f32) {
    for dst in out {
        *dst *= rhs;
    }
}

#[inline]
pub(super) fn scalar_dot_f32(lhs: &[f32], rhs: &[f32]) -> f32 {
    let mut sum = 0.0;
    for i in 0..lhs.len() {
        sum += lhs[i] * rhs[i];
    }
    sum
}

#[inline]
pub(super) fn simd_dot_f32(lhs: &[f32], rhs: &[f32]) -> f32 {
    debug_assert_eq!(lhs.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if let Some(sum) = x86::try_dot_f32(lhs, rhs) {
        return sum;
    }
    #[cfg(target_arch = "aarch64")]
    if let Some(sum) = aarch64::try_dot_f32(lhs, rhs) {
        return sum;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if let Some(sum) = wasm32::try_dot_f32(lhs, rhs) {
        return sum;
    }
    scalar_dot_f32(lhs, rhs)
}

#[inline]
pub(super) fn scalar_add_assign_generic<T>(out: &mut [T], rhs: &[T])
where
    T: Copy + AddAssign,
{
    for (dst, src) in out.iter_mut().zip(rhs) {
        *dst += *src;
    }
}

#[inline]
pub(super) fn scalar_sub_assign_generic<T>(out: &mut [T], rhs: &[T])
where
    T: Copy + SubAssign,
{
    for (dst, src) in out.iter_mut().zip(rhs) {
        *dst -= *src;
    }
}

#[inline]
pub(super) fn scalar_scale_assign_generic<T>(out: &mut [T], rhs: T)
where
    T: Copy + MulAssign,
{
    for dst in out {
        *dst *= rhs;
    }
}

#[inline]
pub(super) fn scalar_div_assign_generic<T>(out: &mut [T], rhs: T)
where
    T: Copy + DivAssign,
{
    for dst in out {
        *dst /= rhs;
    }
}

#[inline]
pub(super) fn scalar_shl_assign_generic<T>(out: &mut [T], rhs: u32)
where
    T: ShlAssign<u32>,
{
    for dst in out {
        *dst <<= rhs;
    }
}

#[inline]
pub(super) fn scalar_shr_assign_generic<T>(out: &mut [T], rhs: u32)
where
    T: ShrAssign<u32>,
{
    for dst in out {
        *dst >>= rhs;
    }
}

#[inline]
pub(super) fn simd_add_assign_f64(out: &mut [f64], rhs: &[f64]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_add_assign_f64(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_add_assign_f64(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_add_assign_f64(out, rhs) {
        return;
    }
    scalar_add_assign_generic(out, rhs);
}

#[inline]
pub(super) fn simd_sub_assign_f64(out: &mut [f64], rhs: &[f64]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_sub_assign_f64(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_sub_assign_f64(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_sub_assign_f64(out, rhs) {
        return;
    }
    scalar_sub_assign_generic(out, rhs);
}

#[inline]
pub(super) fn simd_scale_assign_f64(out: &mut [f64], rhs: f64) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_scale_assign_f64(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_scale_assign_f64(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_scale_assign_f64(out, rhs) {
        return;
    }
    scalar_scale_assign_generic(out, rhs);
}

#[inline]
pub(super) fn simd_add_assign_i32(out: &mut [i32], rhs: &[i32]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_add_assign_i32(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_add_assign_i32(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_add_assign_i32(out, rhs) {
        return;
    }
    scalar_add_assign_generic(out, rhs);
}

#[inline]
pub(super) fn simd_sub_assign_i32(out: &mut [i32], rhs: &[i32]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_sub_assign_i32(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_sub_assign_i32(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_sub_assign_i32(out, rhs) {
        return;
    }
    scalar_sub_assign_generic(out, rhs);
}

#[inline]
pub(super) fn simd_scale_assign_i32(out: &mut [i32], rhs: i32) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_scale_assign_i32(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_scale_assign_i32(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_scale_assign_i32(out, rhs) {
        return;
    }
    scalar_scale_assign_generic(out, rhs);
}

#[inline]
pub(super) fn simd_add_assign_i8(out: &mut [i8], rhs: &[i8]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_add_assign_i8(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_add_assign_i8(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_add_assign_i8(out, rhs) {
        return;
    }
    scalar_add_assign_generic(out, rhs);
}

#[inline]
pub(super) fn simd_sub_assign_i8(out: &mut [i8], rhs: &[i8]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_sub_assign_i8(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_sub_assign_i8(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_sub_assign_i8(out, rhs) {
        return;
    }
    scalar_sub_assign_generic(out, rhs);
}

#[inline]
pub(super) fn simd_add_assign_i16(out: &mut [i16], rhs: &[i16]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_add_assign_i16(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_add_assign_i16(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_add_assign_i16(out, rhs) {
        return;
    }
    scalar_add_assign_generic(out, rhs);
}

#[inline]
pub(super) fn simd_sub_assign_i16(out: &mut [i16], rhs: &[i16]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_sub_assign_i16(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_sub_assign_i16(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_sub_assign_i16(out, rhs) {
        return;
    }
    scalar_sub_assign_generic(out, rhs);
}

#[inline]
pub(super) fn simd_scale_assign_i16(out: &mut [i16], rhs: i16) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_scale_assign_i16(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_scale_assign_i16(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_scale_assign_i16(out, rhs) {
        return;
    }
    scalar_scale_assign_generic(out, rhs);
}

#[inline]
pub(super) fn simd_add_assign_i64(out: &mut [i64], rhs: &[i64]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_add_assign_i64(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_add_assign_i64(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_add_assign_i64(out, rhs) {
        return;
    }
    scalar_add_assign_generic(out, rhs);
}

#[inline]
pub(super) fn simd_sub_assign_i64(out: &mut [i64], rhs: &[i64]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_sub_assign_i64(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_sub_assign_i64(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_sub_assign_i64(out, rhs) {
        return;
    }
    scalar_sub_assign_generic(out, rhs);
}

#[inline]
pub(super) fn simd_add_assign_u32(out: &mut [u32], rhs: &[u32]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_add_assign_i32(cast_u32_mut(out), cast_u32(rhs)) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_add_assign_i32(cast_u32_mut(out), cast_u32(rhs)) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_add_assign_i32(cast_u32_mut(out), cast_u32(rhs)) {
        return;
    }
    scalar_add_assign_generic(out, rhs);
}

#[inline]
pub(super) fn simd_sub_assign_u32(out: &mut [u32], rhs: &[u32]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_sub_assign_i32(cast_u32_mut(out), cast_u32(rhs)) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_sub_assign_i32(cast_u32_mut(out), cast_u32(rhs)) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_sub_assign_i32(cast_u32_mut(out), cast_u32(rhs)) {
        return;
    }
    scalar_sub_assign_generic(out, rhs);
}

#[inline]
pub(super) fn simd_scale_assign_u32(out: &mut [u32], rhs: u32) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_scale_assign_i32(cast_u32_mut(out), rhs as i32) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_scale_assign_i32(cast_u32_mut(out), rhs as i32) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_scale_assign_i32(cast_u32_mut(out), rhs as i32) {
        return;
    }
    scalar_scale_assign_generic(out, rhs);
}

#[inline]
pub(super) fn simd_add_assign_u8(out: &mut [u8], rhs: &[u8]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_add_assign_i8(cast_u8_mut(out), cast_u8(rhs)) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_add_assign_i8(cast_u8_mut(out), cast_u8(rhs)) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_add_assign_i8(cast_u8_mut(out), cast_u8(rhs)) {
        return;
    }
    scalar_add_assign_generic(out, rhs);
}

#[inline]
pub(super) fn simd_sub_assign_u8(out: &mut [u8], rhs: &[u8]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_sub_assign_i8(cast_u8_mut(out), cast_u8(rhs)) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_sub_assign_i8(cast_u8_mut(out), cast_u8(rhs)) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_sub_assign_i8(cast_u8_mut(out), cast_u8(rhs)) {
        return;
    }
    scalar_sub_assign_generic(out, rhs);
}

#[inline]
pub(super) fn simd_add_assign_u16(out: &mut [u16], rhs: &[u16]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_add_assign_i16(cast_u16_mut(out), cast_u16(rhs)) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_add_assign_i16(cast_u16_mut(out), cast_u16(rhs)) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_add_assign_i16(cast_u16_mut(out), cast_u16(rhs)) {
        return;
    }
    scalar_add_assign_generic(out, rhs);
}

#[inline]
pub(super) fn simd_sub_assign_u16(out: &mut [u16], rhs: &[u16]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_sub_assign_i16(cast_u16_mut(out), cast_u16(rhs)) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_sub_assign_i16(cast_u16_mut(out), cast_u16(rhs)) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_sub_assign_i16(cast_u16_mut(out), cast_u16(rhs)) {
        return;
    }
    scalar_sub_assign_generic(out, rhs);
}

#[inline]
pub(super) fn simd_scale_assign_u16(out: &mut [u16], rhs: u16) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_scale_assign_i16(cast_u16_mut(out), rhs as i16) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_scale_assign_i16(cast_u16_mut(out), rhs as i16) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_scale_assign_i16(cast_u16_mut(out), rhs as i16) {
        return;
    }
    scalar_scale_assign_generic(out, rhs);
}

#[inline]
pub(super) fn simd_add_assign_u64(out: &mut [u64], rhs: &[u64]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_add_assign_i64(cast_u64_mut(out), cast_u64(rhs)) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_add_assign_i64(cast_u64_mut(out), cast_u64(rhs)) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_add_assign_i64(cast_u64_mut(out), cast_u64(rhs)) {
        return;
    }
    scalar_add_assign_generic(out, rhs);
}

#[inline]
pub(super) fn simd_sub_assign_u64(out: &mut [u64], rhs: &[u64]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_sub_assign_i64(cast_u64_mut(out), cast_u64(rhs)) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_sub_assign_i64(cast_u64_mut(out), cast_u64(rhs)) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_sub_assign_i64(cast_u64_mut(out), cast_u64(rhs)) {
        return;
    }
    scalar_sub_assign_generic(out, rhs);
}

#[inline]
#[cfg(any(
    target_arch = "x86",
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
pub(super) fn cast_u8(value: &[u8]) -> &[i8] {
    // SAFETY: u8 and i8 have same size/alignment; length unchanged.
    unsafe { std::slice::from_raw_parts(value.as_ptr().cast(), value.len()) }
}

#[inline]
#[cfg(any(
    target_arch = "x86",
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
pub(super) fn cast_u8_mut(value: &mut [u8]) -> &mut [i8] {
    // SAFETY: u8 and i8 have same size/alignment; length unchanged.
    unsafe { std::slice::from_raw_parts_mut(value.as_mut_ptr().cast(), value.len()) }
}

#[inline]
#[cfg(any(
    target_arch = "x86",
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
pub(super) fn cast_u16(value: &[u16]) -> &[i16] {
    // SAFETY: u16 and i16 have same size/alignment; length unchanged.
    unsafe { std::slice::from_raw_parts(value.as_ptr().cast(), value.len()) }
}

#[inline]
#[cfg(any(
    target_arch = "x86",
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
pub(super) fn cast_u16_mut(value: &mut [u16]) -> &mut [i16] {
    // SAFETY: u16 and i16 have same size/alignment; length unchanged.
    unsafe { std::slice::from_raw_parts_mut(value.as_mut_ptr().cast(), value.len()) }
}

#[inline]
#[cfg(any(
    target_arch = "x86",
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
pub(super) fn cast_u32(value: &[u32]) -> &[i32] {
    // SAFETY: u32 and i32 have same size/alignment; length unchanged.
    unsafe { std::slice::from_raw_parts(value.as_ptr().cast(), value.len()) }
}

#[inline]
#[cfg(any(
    target_arch = "x86",
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
pub(super) fn cast_u32_mut(value: &mut [u32]) -> &mut [i32] {
    // SAFETY: u32 and i32 have same size/alignment; length unchanged.
    unsafe { std::slice::from_raw_parts_mut(value.as_mut_ptr().cast(), value.len()) }
}

#[inline]
#[cfg(any(
    target_arch = "x86",
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
pub(super) fn cast_u64(value: &[u64]) -> &[i64] {
    // SAFETY: u64 and i64 have same size/alignment; length unchanged.
    unsafe { std::slice::from_raw_parts(value.as_ptr().cast(), value.len()) }
}

#[inline]
#[cfg(any(
    target_arch = "x86",
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
pub(super) fn cast_u64_mut(value: &mut [u64]) -> &mut [i64] {
    // SAFETY: u64 and i64 have same size/alignment; length unchanged.
    unsafe { std::slice::from_raw_parts_mut(value.as_mut_ptr().cast(), value.len()) }
}

#[inline]
pub(super) const fn static_assert_square<const ROWS: usize, const COLS: usize>() {
    assert!(ROWS == COLS, "matrix must be square");
}

#[inline]
pub(super) fn matrix_rows_2<const ROWS: usize, const COLS: usize>(
    rows: [[f32; COLS]; ROWS],
) -> [[f32; 2]; 2] {
    [[rows[0][0], rows[0][1]], [rows[1][0], rows[1][1]]]
}

#[inline]
pub(super) fn matrix_rows_3<const ROWS: usize, const COLS: usize>(
    rows: [[f32; COLS]; ROWS],
) -> [[f32; 3]; 3] {
    [
        [rows[0][0], rows[0][1], rows[0][2]],
        [rows[1][0], rows[1][1], rows[1][2]],
        [rows[2][0], rows[2][1], rows[2][2]],
    ]
}

#[inline]
pub(super) fn matrix_rows_4<const ROWS: usize, const COLS: usize>(
    rows: [[f32; COLS]; ROWS],
) -> [[f32; 4]; 4] {
    [
        [rows[0][0], rows[0][1], rows[0][2], rows[0][3]],
        [rows[1][0], rows[1][1], rows[1][2], rows[1][3]],
        [rows[2][0], rows[2][1], rows[2][2], rows[2][3]],
        [rows[3][0], rows[3][1], rows[3][2], rows[3][3]],
    ]
}

#[inline]
pub(super) fn matrix_from_rows_2<const ROWS: usize, const COLS: usize>(
    rows: [[f32; 2]; 2],
) -> [[f32; COLS]; ROWS] {
    let mut out = [[0.0; COLS]; ROWS];
    out[0][0] = rows[0][0];
    out[0][1] = rows[0][1];
    out[1][0] = rows[1][0];
    out[1][1] = rows[1][1];
    out
}

#[inline]
pub(super) fn matrix_from_rows_3<const ROWS: usize, const COLS: usize>(
    rows: [[f32; 3]; 3],
) -> [[f32; COLS]; ROWS] {
    let mut out = [[0.0; COLS]; ROWS];
    for r in 0..3 {
        for c in 0..3 {
            out[r][c] = rows[r][c];
        }
    }
    out
}

#[inline]
pub(super) fn matrix_from_rows_4<const ROWS: usize, const COLS: usize>(
    rows: [[f32; 4]; 4],
) -> [[f32; COLS]; ROWS] {
    let mut out = [[0.0; COLS]; ROWS];
    for r in 0..4 {
        for c in 0..4 {
            out[r][c] = rows[r][c];
        }
    }
    out
}
