use perro_nodes::prelude::*;
use perro_structs::prelude::*;
use perro_ids::prelude::*;
use perro_modules::prelude::*;
use perro_scripting::prelude::*;

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

        let (speed, look_sensitivity) = with_state!(ctx, CameraState, node, |state| {
            (state.move_speed, state.look_sensitivity)
        }).unwrap_or_default();
        log_info!(format!("camera move_speed={speed} look_sensitivity={look_sensitivity}"));
    }

    fn on_all_init(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        _self: NodeID,
    ) {}

    fn on_update(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        ipt: &InputContext<'_, IP>,
        node: NodeID,
    ) {
        let dt = delta_time!(ctx);
        let middle_down = ipt.Mouse().down(MouseButton::Middle);
        let (mouse_dx, mouse_dy) = ipt.Mouse().delta();
        let (_wheel_x, wheel_y) = ipt.Mouse().wheel();
        let (move_speed, yaw, pitch) = with_state_mut!(ctx, CameraState, node, |state| {
            if wheel_y != 0.0 {
                // Scroll up speeds up, scroll down slows down.
                state.move_speed = (state.move_speed + wheel_y * 2.0).clamp(0.5, 300.0);
            }
            if middle_down {
                // Inverted drag direction for both axes.
                state.yaw -= mouse_dx * state.look_sensitivity;
                state.pitch -= mouse_dy * state.look_sensitivity;
                let pitch_limit = deg_to_rad!(89.0);
                if state.pitch > pitch_limit {
                    state.pitch = pitch_limit;
                } else if state.pitch < -pitch_limit {
                    state.pitch = -pitch_limit;
                }
            }

            (state.move_speed, state.yaw, state.pitch)
        }).unwrap_or((8.0, 0.0, 0.0));

        // Ground movement is yaw-only (Minecraft style): pitch never curves movement.
        let basis_forward_x = -yaw.sin();
        let basis_forward_z = -yaw.cos();
        let basis_right_x = -basis_forward_z;
        let basis_right_z = basis_forward_x;

        let mut move_forward = 0.0_f32;
        let mut move_right = 0.0_f32;
        let mut y = 0.0_f32;

        if ipt.Keys().down(KeyCode::KeyW) {
            move_forward += 1.0;
        }
        if ipt.Keys().down(KeyCode::KeyS) {
            move_forward -= 1.0;
        }
        if ipt.Keys().down(KeyCode::KeyA) {
            move_right -= 1.0;
        }
        if ipt.Keys().down(KeyCode::KeyD) {
            move_right += 1.0;
        }
        if ipt.Keys().down(KeyCode::Space) {
            y += 1.0;
        }
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
            // Compose as world-yaw then local-pitch: no roll, stable FPS camera behavior.
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
    ) {}

    fn on_removal(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        _self: NodeID,
    ) {}
});

methods!({
    fn bob(&self, ctx: &mut RuntimeContext<'_, RT>, _res: &ResourceContext<'_, RS>, _ipt: &InputContext<'_, IP>, node: NodeID, _param1: i32, _j: &str) {
        let _j = with_state_mut!(ctx, CameraState, node, |state| {
            state.move_speed += 1.0;
            state.move_speed
        }).unwrap_or_default();
    }
});
