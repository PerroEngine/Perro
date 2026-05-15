use perro_api::prelude::*;

type SelfNodeType = Camera2D;

const PAN_SPEED: f32 = 900.0;
const ZOOM_STEP: f32 = 0.08;
const MIN_ZOOM: f32 = -0.65;
const MAX_ZOOM: f32 = 1.25;

lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        let mut delta = Vector2::ZERO;

        if key_down!(ctx.ipt, KeyCode::KeyA) || key_down!(ctx.ipt, KeyCode::ArrowLeft) {
            delta.x -= 1.0;
        }
        if key_down!(ctx.ipt, KeyCode::KeyD) || key_down!(ctx.ipt, KeyCode::ArrowRight) {
            delta.x += 1.0;
        }
        if key_down!(ctx.ipt, KeyCode::KeyW) || key_down!(ctx.ipt, KeyCode::ArrowUp) {
            delta.y += 1.0;
        }
        if key_down!(ctx.ipt, KeyCode::KeyS) || key_down!(ctx.ipt, KeyCode::ArrowDown) {
            delta.y -= 1.0;
        }

        if delta.length_squared() > 0.0 {
            delta = delta.normalized() * PAN_SPEED * dt;
            let _ = with_base_node_mut!(ctx.run, Node2D, ctx.id, |node| {
                node.transform.position += delta;
            });
        }

        let wheel = mouse_wheel!(ctx.ipt).y;
        if wheel.abs() > 0.001 {
            let _ = with_node_mut!(ctx.run, Camera2D, ctx.id, |cam| {
                cam.zoom = (cam.zoom - wheel * ZOOM_STEP).clamp(MIN_ZOOM, MAX_ZOOM);
            });
        }
    }
});
