use perro::prelude::*;

type SelfNodeType = Camera3D;

#[State]
pub struct CameraState {
    #[default = 24.0]
    ///@Expose
    move_speed: f32,

    #[default = 0.0025]
    ///@Expose
    look_sensitivity: f32,

    #[default = 0.0]
    yaw: f32,

    #[default = 0.0]
    pitch: f32,
}

lifecycle!({
    fn on_init(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        node: NodeID,
    ) {
    }

    fn on_all_init(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        _self: NodeID,
    ) {
    }

    fn on_update(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        ipt: &InputContext<'_, IP>,
        node: NodeID,
    ) {
        let dt = delta_time!(ctx);
        let middle_down = ipt.Mouse().down(MouseButton::Middle);
        let mouse_delta = ipt.Mouse().delta();
        let wheel = ipt.Mouse().wheel();
        let mouse_dx = mouse_delta.x;
        let mouse_dy = mouse_delta.y;
        let wheel_y = wheel.y;

        let (move_speed, yaw, pitch) = with_state_mut!(ctx, CameraState, node, |state| {
            if wheel_y != 0.0 {
                state.move_speed = (state.move_speed + wheel_y * 2.0).clamp(0.5, 300.0);
            }
            if middle_down {
                state.yaw -= mouse_dx * state.look_sensitivity;
                state.pitch -= mouse_dy * state.look_sensitivity;
                let pitch_limit = deg_to_rad!(89.0);
                state.pitch = state.pitch.clamp(-pitch_limit, pitch_limit);
            }
            (state.move_speed, state.yaw, state.pitch)
        })
        .unwrap_or((24.0, 0.0, 0.0));

        let basis_forward_x = -yaw.sin();
        let basis_forward_z = -yaw.cos();
        let basis_right_x = -basis_forward_z;
        let basis_right_z = basis_forward_x;

        let mut move_forward = 0.0_f32;
        let mut move_right = 0.0_f32;
        let mut y = 0.0_f32;

        if ipt.Keys().down(KeyCode::KeyW) { move_forward += 1.0; }
        if ipt.Keys().down(KeyCode::KeyS) { move_forward -= 1.0; }
        if ipt.Keys().down(KeyCode::KeyA) { move_right -= 1.0; }
        if ipt.Keys().down(KeyCode::KeyD) { move_right += 1.0; }
        if ipt.Keys().down(KeyCode::Space) { y += 1.0; }
        if ipt.Keys().down(KeyCode::ShiftLeft) || ipt.Keys().down(KeyCode::ShiftRight) {
            y -= 1.0;
        }

        let mut move_x = basis_forward_x * move_forward + basis_right_x * move_right;
        let mut move_z = basis_forward_z * move_forward + basis_right_z * move_right;
        let planar_len = (move_x * move_x + move_z * move_z).sqrt();
        if planar_len > 1.0 {
            move_x /= planar_len;
            move_z /= planar_len;
        }

        let step = dt * move_speed;
        with_node_mut!(ctx, SelfNodeType, node, |camera| {
            camera.rotation = Quaternion::IDENTITY;
            camera.rotation.rotate_x(pitch);
            camera.rotation.rotate_y(yaw);

            camera.position.x += move_x * step;
            camera.position.y += y * step;
            camera.position.z += move_z * step;
        });
    }

    fn on_fixed_update(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        _self: NodeID,
    ) {
    }

    fn on_removal(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        _self: NodeID,
    ) {
    }
});

methods!({
   
});