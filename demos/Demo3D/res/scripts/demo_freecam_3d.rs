use perro_api::prelude::*;

type SelfNodeType = Camera3D;

const MOUSE_SENSITIVITY: f32 = 0.0025;
const PITCH_LIMIT: f32 = 1.553343;

#[State]
struct DemoFreecam3DState {
    #[default = 0.0]
    pub yaw: f32,
    #[default = 0.0]
    pub pitch: f32,
    #[default = 8.0]
    pub speed: f32,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let (yaw, pitch) = with_state!(ctx.run, DemoFreecam3DState, ctx.id, |state| {
            (state.yaw, state.pitch)
        });
        let rot = Quaternion::from_euler_xyz(pitch, yaw, 0.0);
        let _ = set_local_rot_3d!(ctx.run, ctx.id, rot);
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        if mouse_mode!(ctx.ipt) != MouseMode::Captured {
            return;
        }

        let dt = delta_time!(ctx.run);
        let mouse = mouse_delta!(ctx.ipt);

        let (yaw, pitch, speed) = with_state_mut!(ctx.run, DemoFreecam3DState, ctx.id, |state| {
            state.yaw -= mouse.x * MOUSE_SENSITIVITY;
            state.pitch =
                (state.pitch - mouse.y * MOUSE_SENSITIVITY).clamp(-PITCH_LIMIT, PITCH_LIMIT);
            (state.yaw, state.pitch, state.speed)
        })
        .unwrap_or((0.0, 0.0, 8.0));

        let rot = Quaternion::from_euler_xyz(pitch, yaw, 0.0);
        let _ = set_local_rot_3d!(ctx.run, ctx.id, rot);

        let forward = Vector3::new(yaw.sin(), 0.0, -yaw.cos());
        let right = Vector3::new(yaw.cos(), 0.0, yaw.sin());
        let mut move_dir = Vector3::ZERO;

        if key_down!(ctx.ipt, KeyCode::KeyW) {
            move_dir += forward;
        }
        if key_down!(ctx.ipt, KeyCode::KeyS) {
            move_dir -= forward;
        }
        if key_down!(ctx.ipt, KeyCode::KeyD) {
            move_dir += right;
        }
        if key_down!(ctx.ipt, KeyCode::KeyA) {
            move_dir -= right;
        }
        if key_down!(ctx.ipt, KeyCode::Space) {
            move_dir += Vector3::new(0.0, 1.0, 0.0);
        }
        if key_down!(ctx.ipt, KeyCode::ShiftLeft) || key_down!(ctx.ipt, KeyCode::ShiftRight) {
            move_dir -= Vector3::new(0.0, 1.0, 0.0);
        }

        if move_dir.length_squared() <= 0.000001 {
            return;
        }

        let pos = get_local_pos_3d!(ctx.run, ctx.id).unwrap_or(Vector3::ZERO);
        let next = pos + (move_dir.normalized() * speed * dt);
        let _ = set_local_pos_3d!(ctx.run, ctx.id, next);
    }
});

methods!({});
