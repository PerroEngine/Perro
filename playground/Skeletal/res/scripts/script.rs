use perro::prelude::*;


type SelfNodeType = Skeleton3D;

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
    ctx: &mut RuntimeContext<'_, RT>,
    _res: &ResourceContext<'_, RS>,
    ipt: &InputContext<'_, IP>,
    self_id: NodeID,
) {
    let dt = delta_time!(ctx);
    let speed = 1.5; // radians/sec

    let mut dir = 0.0;
    if ipt.Keys().down(KeyCode::KeyA) {
        dir += 1.0;
    }
    if ipt.Keys().down(KeyCode::KeyD) {
        dir -= 1.0;
    }


if dir != 0.0 {
    if let Some(rot) = with_node_mut!(ctx, SelfNodeType, self_id, |node| {
        node.bones.get_mut(1).map(|bone| {
            bone.rest.rotation.rotate_y(speed * dt * dir);
            bone.rest.rotation
        })
    }) {
        log_info!(format!(
            "bone[1] rot = {:?}",
            rot.unwrap_or_default().to_quat()
        ));
    }
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
