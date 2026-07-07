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

#[test]
fn prelude_exports_runtime_physics_move_api() {
    fn _uses_types(
        _filter: PhysicsQueryFilter,
        _move_2d: Option<PhysicsMoveResult2D>,
        _move_3d: Option<PhysicsMoveResult3D>,
    ) {
    }

    let _macro_2d = stringify!(physics_move_body_2d!(ctx.run, body, target));
    let _macro_3d = stringify!(physics_move_body_3d!(ctx.run, body, target));
}

#[test]
fn prelude_exports_world_label_and_sprite_nodes() {
    fn _uses_node_types(
        _label_2d: Option<Label2D>,
        _label_3d: Option<Label3D>,
        _sprite_3d: Option<Sprite3D>,
    ) {
    }

    let _node_type = NodeType::Label3D;
    let _data = SceneNodeData::Label3D(Label3D::new());
}
