use super::should_skip;
use std::collections::HashSet;

#[test]
fn pmat_is_skipped_as_compiled_resource() {
    let extra = HashSet::new();
    assert!(should_skip("materials/mat.pmat", &extra));
    assert!(should_skip("particles/fire.ppart", &extra));
    assert!(should_skip("animations/run.panim", &extra));
    assert!(should_skip("terrain/0_0.ptchunk", &extra));
    assert!(should_skip("terrain/settings.pterr", &extra));
    assert!(should_skip("scene/main.scn", &extra));
    assert!(should_skip("mesh/robot.glb", &extra));
    assert!(should_skip("audio/music.ogg", &extra));
    assert!(should_skip("shaders/custom.wgsl", &extra));
}
