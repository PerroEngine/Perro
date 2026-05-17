use perro_api::prelude::*;

#[test]
fn prelude_exports_bitmask_type_and_macro() {
    const EMPTY: BitMask = bitmask!([]);
    const LAYERS: BitMask = bitmask!([1, 3]);

    assert_eq!(EMPTY, BitMask::NONE);
    assert_eq!(LAYERS.bits(), 0b101);
}

#[test]
fn prelude_exports_input_action_macro() {
    let input = InputSnapshot::new();
    let ctx = InputWindow::new(&input);

    assert!(!action_pressed!(&ctx, "jump"));
}
