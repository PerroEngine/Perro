use perro_api::prelude::*;

type SelfNodeType = Camera3D;

const DEFAULT_MOUSE_SENSITIVITY: f32 = 0.00012;
const MAX_MOUSE_DELTA: f32 = 120.0;
const PITCH_LIMIT: f32 = 1.553343;
const CAPTURE_WARMUP_FRAMES: u8 = 2;
const WORLD_UP: Vector3 = Vector3::new(0.0, 1.0, 0.0);

#[State]
struct DemoFreecam3DState {
    #[default = 0.0]
    pub yaw: f32,
    #[default = 0.0]
    pub pitch: f32,
    #[default = 8.0]
    pub speed: f32,
    #[default = DEFAULT_MOUSE_SENSITIVITY]
    pub mouse_sensitivity: f32,
    #[default = CAPTURE_WARMUP_FRAMES]
    pub capture_warmup: u8,
    #[default = true]
    pub input_enabled: bool,
    #[default = 0]
    pub debug_frame: u32,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let (yaw, pitch) = with_state!(ctx.run, DemoFreecam3DState, ctx.id, |state| {
            (state.yaw, state.pitch)
        });
        let rot = freecam_rotation(yaw, pitch);
        let _ = with_node_mut!(ctx.run, Camera3D, ctx.id, |camera| {
            camera.transform.rotation = rot;
        });
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let mouse = mouse_delta!(ctx.ipt);

        let input_enabled = with_state!(ctx.run, DemoFreecam3DState, ctx.id, |state| {
            state.input_enabled
        });
        if !input_enabled {
            let _ = with_state_mut!(ctx.run, DemoFreecam3DState, ctx.id, |state| {
                state.capture_warmup = CAPTURE_WARMUP_FRAMES;
            });
            return;
        }

        let _ = with_state_mut!(ctx.run, DemoFreecam3DState, ctx.id, |state| {
            state.debug_frame = state.debug_frame.wrapping_add(1);
            if state.debug_frame % 30 == 0 {
                log_info!(
                    "freecam active mouse=({:.3},{:.3}) warmup={}",
                    mouse.x,
                    mouse.y,
                    state.capture_warmup
                );
            }
        });

        let dt = delta_time!(ctx.run);

        let (yaw, pitch, speed) = with_state_mut!(ctx.run, DemoFreecam3DState, ctx.id, |state| {
            if state.capture_warmup > 0 {
                state.capture_warmup -= 1;
                return (state.yaw, state.pitch, state.speed);
            }
            let sensitivity = state.mouse_sensitivity.max(0.000001);
            let look_x = mouse.x.clamp(-MAX_MOUSE_DELTA, MAX_MOUSE_DELTA);
            let look_y = mouse.y.clamp(-MAX_MOUSE_DELTA, MAX_MOUSE_DELTA);
            state.yaw -= look_x * sensitivity;
            state.pitch = (state.pitch + look_y * sensitivity).clamp(-PITCH_LIMIT, PITCH_LIMIT);
            if look_x.abs() > 0.001 || look_y.abs() > 0.001 {
                log_info!(
                    "freecam mouse raw=({:.3},{:.3}) look=({:.3},{:.3}) yaw={:.4} pitch={:.4}",
                    mouse.x,
                    mouse.y,
                    look_x,
                    look_y,
                    state.yaw,
                    state.pitch
                );
            }
            (state.yaw, state.pitch, state.speed)
        })
        .unwrap_or((0.0, 0.0, 8.0));

        let rot = freecam_rotation(yaw, pitch);
        let _ = with_node_mut!(ctx.run, Camera3D, ctx.id, |camera| {
            camera.transform.rotation = rot;
        });

        let forward = freecam_flat_forward(yaw);
        let right = freecam_flat_right(yaw);
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

        let delta = move_dir.normalized() * speed * dt;
        let _ = with_node_mut!(ctx.run, Camera3D, ctx.id, |camera| {
            camera.transform.position += delta;
        });
    }
});

methods!({});

fn freecam_rotation(yaw: f32, pitch: f32) -> Quaternion {
    Quaternion::looking_at(freecam_forward(yaw, pitch), WORLD_UP)
}

fn freecam_forward(yaw: f32, pitch: f32) -> Vector3 {
    let pitch_cos = pitch.cos();
    Vector3::new(-yaw.sin() * pitch_cos, pitch.sin(), -yaw.cos() * pitch_cos)
}

fn freecam_flat_forward(yaw: f32) -> Vector3 {
    Vector3::new(-yaw.sin(), 0.0, -yaw.cos())
}

fn freecam_flat_right(yaw: f32) -> Vector3 {
    Vector3::new(yaw.cos(), 0.0, -yaw.sin())
}
