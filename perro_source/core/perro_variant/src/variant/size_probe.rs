//! Size regression guard: `Variant` stride dictates the footprint of every
//! `Vec<Variant>` element and object node engine-wide.

use super::*;

#[test]
fn variant_stride_stays_small() {
    // Print the full breakdown so a failure is self-diagnosing.
    println!("Variant      = {}", size_of::<Variant>());
    println!("EngineStruct = {}", size_of::<EngineStruct>());
    println!("Number       = {}", size_of::<Number>());
    println!("IDs          = {}", size_of::<IDs>());
    println!("Vector2      = {}", size_of::<Vector2>());
    println!("Vector3      = {}", size_of::<Vector3>());
    println!("Vector4      = {}", size_of::<Vector4>());
    println!("IVector4     = {}", size_of::<IVector4>());
    println!("UVector4     = {}", size_of::<UVector4>());
    println!("UnitVector2  = {}", size_of::<UnitVector2>());
    println!("UnitVector3  = {}", size_of::<UnitVector3>());
    println!("UnitVector4  = {}", size_of::<UnitVector4>());
    println!("Matrix2      = {}", size_of::<Matrix2>());
    println!("Transform2D  = {}", size_of::<Transform2D>());
    println!("Quaternion   = {}", size_of::<Quaternion>());
    println!("VisualAcc    = {}", size_of::<VisualAccessibilitySettings>());
    println!("Vec<Variant> = {}", size_of::<Vec<Variant>>());
    println!(
        "BTreeMap     = {}",
        size_of::<std::collections::BTreeMap<std::sync::Arc<str>, Variant>>()
    );

    assert!(
        size_of::<Variant>() <= 32,
        "Variant grew past 32 bytes; box the new large member"
    );
}
