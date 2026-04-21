pub mod file;
pub mod json;
pub mod log;
pub mod math;
pub mod random;

pub mod prelude {
    pub use crate::file as FileMod;
    pub use crate::json as JSONMod;
    pub use crate::log as LogMod;
    pub use crate::math as MathMod;
    pub use crate::math::{
        angle_diff_deg, angle_diff_rad, approach, clamp01, damp, deg_to_rad, ilerp, islerp,
        ismoothstep, lerp, lerp_angle_deg, lerp_angle_rad, nearly_eq, ping_pong, rad_to_deg, remap,
        repeat, slerp, smooth_damp, smoothstep, wrap_angle_deg, wrap_angle_rad,
    };
    pub use crate::random as RandomMod;
    pub use crate::random::{
        chance, choose_index, hash, hash2_u32, hash3_u32, hash64_bytes, hash64_str, hash64_u128,
        hash64_u32, hash64_u64, hash_bool, hash_bytes, hash_combine, hash_combine3, hash_combine4,
        hash_f32, hash_i32, hash_i64, hash_str, hash_u128, hash_u32, hash_u64, rand01,
        rand01_stream, rand11, rand11_stream, rand_in_circle, rand_range, rand_range_f32,
        rand_range_i32, rand_range_u32, rand_u32, rand_u32_stream, rand_unit_vec2, rand_unit_vec3,
        shuffle, HashToU32, RandRangeValue, SeededRng,
    };
    pub use crate::{log_error, log_info, log_print, log_warn};
}

#[cfg(test)]
#[path = "../tests/unit/lib_tests.rs"]
mod tests;
