use super::should_skip;

#[test]
fn pmat_is_skipped_as_compiled_resource() {
    assert!(should_skip("materials/mat.pmat"));
    assert!(should_skip("particles/fire.ppart"));
    assert!(should_skip("scene/main.scn"));
    assert!(should_skip("mesh/robot.glb"));
    assert!(should_skip("audio/music.ogg"));
    assert!(should_skip("shaders/custom.wgsl"));
}
