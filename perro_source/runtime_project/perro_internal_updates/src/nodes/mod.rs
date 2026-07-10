pub mod animated_sprite_2d;
pub mod animation_player;
pub mod animation_tree;
pub mod bone_attachment_2d;
pub mod bone_attachment_3d;
pub mod ik_target_2d;
pub mod ik_target_3d;
pub mod particle_emitter_2d;
pub mod particle_emitter_3d;
pub mod physics_bone_chain_2d;
pub mod physics_bone_chain_3d;
pub mod ui_animated_image;
pub mod video_player;

#[inline]
pub(super) fn bounded_solver_iterations(iterations: u32) -> usize {
    iterations.min(perro_runtime_api::perro_structs::MAX_SKELETAL_SOLVER_ITERATIONS) as usize
}

#[cfg(test)]
mod solver_limit_tests {
    use super::bounded_solver_iterations;

    #[test]
    fn solver_iterations_cap_raw_values() {
        assert_eq!(bounded_solver_iterations(0), 0);
        assert_eq!(bounded_solver_iterations(u32::MAX), 64);
    }
}
