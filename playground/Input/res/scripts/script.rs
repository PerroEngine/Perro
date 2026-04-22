use perro_api::prelude::*;

type SelfNodeType = Node2D;

#[State]
pub struct EmptyState {}

lifecycle!({
    fn on_init(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {}

    fn on_all_init(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {}

    fn on_update(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {
        if key_pressed!(ipt, KeyCode::Space) {
            println!("Space key pressed");
        }
        if mouse_pressed!(ipt, MouseButton::Left) {
            println!("Left mouse button pressed");
        }
        if joycon_pressed!(ipt, 0, JoyConButton::Bottom) {
            println!("JoyCon 0 Bottom button pressed");
        }
        if gamepad_pressed!(ipt, 0, GamepadButton::Bottom) {
            println!("Gamepad 0 Bottom button pressed");
        }
    }

    fn on_fixed_update(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {}

    fn on_removal(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {}
});

methods!({
    fn default_method(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {}
});
