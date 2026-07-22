use perro_api::prelude::*;

type SelfNodeType = Camera3D;

const MOVE_SPEED: f32 = 8.0;
const MAX_MOVE_DT: f32 = 1.0 / 45.0;

#[State]
struct DemoFreecam3DState {
    #[default = MOVE_SPEED]
    pub speed: f32,
    #[default = true]
    pub input_enabled: bool,
}

lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let input_enabled = with_state!(ctx.run, DemoFreecam3DState, ctx.id, |state| {
            state.input_enabled
        });

        if !input_enabled {
            return;
        }

        let dt = delta_time!(ctx.run).clamp(0.0, MAX_MOVE_DT);

        let speed = with_state!(ctx.run, DemoFreecam3DState, ctx.id, |state| { state.speed });

        let mut move_dir = Vector3::ZERO;

        if key_down!(ctx.ipt, KeyCode::KeyW) {
            move_dir.z -= 1.0;
        }
        if key_down!(ctx.ipt, KeyCode::KeyS) {
            move_dir.z += 1.0;
        }
        if key_down!(ctx.ipt, KeyCode::KeyD) {
            move_dir.x += 1.0;
        }
        if key_down!(ctx.ipt, KeyCode::KeyA) {
            move_dir.x -= 1.0;
        }
        if key_down!(ctx.ipt, KeyCode::Space) {
            move_dir.y += 1.0;
        }
        if key_down!(ctx.ipt, KeyCode::ShiftLeft) || key_down!(ctx.ipt, KeyCode::ShiftRight) {
            move_dir.y -= 1.0;
        }

        if move_dir.length_squared() <= 0.000001 {
            return;
        }

        // `move_dir` is camera-local: x = right, -z = forward, y = world up.
        let _ = with_node_mut!(ctx.run, Camera3D, ctx.id, |camera| {
            let forward = camera.transform.forward();
            let right = camera.transform.right();
            let mut world = right * move_dir.x + forward * (-move_dir.z);
            world.y += move_dir.y;
            if world.length_squared() > 0.000001 {
                camera.transform.position += world.normalized() * speed * dt;
            }
        });
    }
});

methods!({
    fn set_speed(&self, ctx: &mut ScriptContext<'_, API>, speed: f32) {
        with_state_mut!(ctx.run, DemoFreecam3DState, ctx.id, |state| {
            state.speed = speed.max(0.0);
        });
    }
});
