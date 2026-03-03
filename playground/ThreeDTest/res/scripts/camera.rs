use perro_nodes::prelude::*;
use perro_structs::prelude::*;
use perro_ids::prelude::*;
use perro_modules::prelude::*;
use perro_scripting::prelude::*;
use perro_terrain::{BrushOp, BrushShape};

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

    #[default = 8.0]
    editor_size: f32,

    #[default = 0.5]
    editor_basis: f32,

    #[default = 0.5]
    editor_delta: f32,

    #[default = 0.35]
    editor_smooth_strength: f32,

    #[default = 0.0]
    editor_set_height_y: f32,

    #[default = 1]
    editor_mode: i32,

    #[default = 1]
    editor_shape: i32,
}


lifecycle!({
    fn on_init(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        node: NodeID,
    ) {
        let _ = self.ensure_preview_emitter(ctx, _res, _ipt, node);
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
        res: &ResourceContext<'_, RS>,
        ipt: &InputContext<'_, IP>,
        node: NodeID,
    ) {
        let dt = delta_time!(ctx);
        let middle_down = ipt.Mouse().down(MouseButton::Middle);
        let mouse_delta = ipt.Mouse().delta();
        let wheel = ipt.Mouse().wheel();
        let mouse_pos = ipt.Mouse().position();
        let viewport = ipt.Mouse().viewport_size();
        let mouse_dx = mouse_delta.x;
        let mouse_dy = mouse_delta.y;
        let wheel_y = wheel.y;
        let size_scroll_down = ipt.Keys().down(KeyCode::KeyR);
        let basis_scroll_down = ipt.Keys().down(KeyCode::KeyB);
        let delta_scroll_down = ipt.Keys().down(KeyCode::KeyG);
        let smooth_scroll_down = ipt.Keys().down(KeyCode::KeyF);
        let setheight_scroll_down = ipt.Keys().down(KeyCode::KeyH);

        let mut new_mode = None;
        let mut new_shape = None;

        if ipt.Keys().pressed(KeyCode::Digit1) { new_mode = Some(1); }
        if ipt.Keys().pressed(KeyCode::Digit2) { new_mode = Some(2); }
        if ipt.Keys().pressed(KeyCode::Digit3) { new_mode = Some(3); }
        if ipt.Keys().pressed(KeyCode::Digit4) { new_mode = Some(4); }
        if ipt.Keys().pressed(KeyCode::Digit5) { new_mode = Some(5); }

        if ipt.Keys().pressed(KeyCode::Digit6) { new_shape = Some(1); }
        if ipt.Keys().pressed(KeyCode::Digit7) { new_shape = Some(2); }
        if ipt.Keys().pressed(KeyCode::Digit8) { new_shape = Some(3); }

        if new_mode.is_some() || new_shape.is_some() {
            with_state_mut!(ctx, CameraState, node, |state| {
                if let Some(m) = new_mode {
                    state.editor_mode = m;
                }
                if let Some(s) = new_shape {
                    state.editor_shape = s;
                }
            });
        }

        let (move_speed, yaw, pitch, editor_size, editor_basis, editor_delta, editor_smooth_strength, editor_set_height_y, editor_mode, editor_shape) = with_state_mut!(ctx, CameraState, node, |state| {
            if wheel_y != 0.0 {
                if size_scroll_down {
                    state.editor_size = (state.editor_size + wheel_y).clamp(0.25, 128.0);
                } else if basis_scroll_down {
                    state.editor_basis = (state.editor_basis + wheel_y * 0.05).clamp(0.05, 16.0);
                } else if delta_scroll_down {
                    state.editor_delta = (state.editor_delta + wheel_y * 0.05).clamp(0.01, 32.0);
                } else if smooth_scroll_down {
                    state.editor_smooth_strength =
                        (state.editor_smooth_strength + wheel_y * 0.05).clamp(0.0, 1.0);
                } else if setheight_scroll_down {
                    state.editor_set_height_y =
                        (state.editor_set_height_y + wheel_y * 0.5).clamp(-256.0, 256.0);
                } else {
                    // Scroll up speeds up, scroll down slows down.
                    state.move_speed = (state.move_speed + wheel_y * 2.0).clamp(0.5, 300.0);
                }
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

            (
                state.move_speed,
                state.yaw,
                state.pitch,
                state.editor_size,
                state.editor_basis,
                state.editor_delta,
                state.editor_smooth_strength,
                state.editor_set_height_y,
                state.editor_mode,
                state.editor_shape,
            )
        }).unwrap_or((8.0, 0.0, 0.0, 8.0, 0.5, 0.5, 0.35, 0.0, 1, 1));

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

        let terrain_node = query!(ctx, all(is[TerrainInstance3D]))
            .first()
            .copied()
            .unwrap_or(NodeID::nil());
        let terrain_id = with_node!(ctx, TerrainInstance3D, terrain_node, |terrain| terrain.terrain);
        let preview = self.ensure_preview_emitter(ctx, res, ipt, node);

        if terrain_id.is_nil() || viewport.x <= 0.0 || viewport.y <= 0.0 {
            with_node_mut!(ctx, ParticleEmitter3D, preview, |emitter| {
                emitter.active = false;
            });
            return;
        }

        let camera_position = with_node!(ctx, SelfNodeType, node, |camera| camera.position);
        let fov_deg = with_node!(ctx, SelfNodeType, node, |camera| match camera.projection {
            CameraProjection::Perspective { fov_y_degrees, .. } => fov_y_degrees,
            _ => 60.0,
        });

        let ray_dir = camera_ray_dir(
            mouse_pos.x,
            mouse_pos.y,
            viewport.x,
            viewport.y,
            yaw, 
            pitch,
            deg_to_rad!(fov_deg),
        );

        let hit = res
            .Terrain()
            .raycast(terrain_id, camera_position, ray_dir, 5000.0);

        if let Some(hit) = hit {
            with_node_mut!(ctx, ParticleEmitter3D, preview, |emitter| {
                emitter.active = true;
                emitter.position = hit.position_world;
                emitter.params = vec![
                    editor_size,
                    editor_basis,
                    editor_delta,
                    editor_smooth_strength,
                    editor_set_height_y,
                ];
            });

            if ipt.Mouse().down(MouseButton::Left) {
                let _ = res.Terrain().brush_op(
                    terrain_id,
                    hit.position_world,
                    editor_size,
                    brush_shape_from_index(editor_shape),
                    brush_op_from_mode(
                        editor_mode,
                        editor_basis,
                        editor_delta,
                        editor_smooth_strength,
                        editor_set_height_y,
                    ),
                );
            }
        } else {
            with_node_mut!(ctx, ParticleEmitter3D, preview, |emitter| {
                emitter.active = false;
            });
        }
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

    fn ensure_preview_emitter(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        camera_node: NodeID,
    ) -> NodeID {
        if let Some(id) = query!(ctx, all(is[ParticleEmitter3D], tags["terrain_editor_preview"]))
            .first()
            .copied()
        {
            return id;
        }

        let parent_id = get_node_parent_id!(ctx, camera_node).unwrap_or(NodeID::nil());
        let emitter_id = create_node!(
            ctx,
            ParticleEmitter3D,
            "terrain_preview_emitter",
            tags!["terrain_editor_preview"],
            parent_id
        );
        let brush_size = with_state!(ctx, CameraState, camera_node, |s| {
            s.editor_size
        }).unwrap_or(0.0);

        with_node_mut!(ctx, ParticleEmitter3D, emitter_id, |emitter| {
            emitter.profile = "res://particles/test.ppart".to_string();
            emitter.render_mode = ParticleType::Billboard;
            emitter.looping = true;
            emitter.prewarm = true;
            emitter.spawn_rate = 5000.0;
            emitter.active = false;
            emitter.params = vec![brush_size, 0.5];
        });
        emitter_id
    }

});

fn brush_shape_from_index(shape: i32) -> BrushShape {
    match shape {
        2 => BrushShape::Circle,
        3 => BrushShape::Triangle,
        _ => BrushShape::Square,
    }
}

fn brush_op_from_mode(
    mode: i32,
    basis: f32,
    delta: f32,
    smooth_strength: f32,
    set_height_y: f32,
) -> BrushOp {
    match mode {
        2 => BrushOp::Remove { delta },
        3 => BrushOp::Smooth {
            strength: smooth_strength,
        },
        4 => BrushOp::Decimate { basis },
        5 => BrushOp::SetHeight {
            y: set_height_y,
            feature_offset: 0.1,
        },
        _ => BrushOp::Add { delta },
    }
}

fn camera_ray_dir(
    mouse_x: f32,
    mouse_y: f32,
    viewport_w: f32,
    viewport_h: f32,
    yaw: f32,
    pitch: f32,
    fov_y_radians: f32,
) -> Vector3 {
    let ndc_x = (mouse_x / viewport_w) * 2.0 - 1.0;
    let ndc_y = 1.0 - (mouse_y / viewport_h) * 2.0;
    let aspect = viewport_w / viewport_h.max(1.0);
    let tan_half = (fov_y_radians * 0.5).tan();

    let cy = yaw.cos();
    let sy = yaw.sin();
    let cp = pitch.cos();
    let sp = pitch.sin();

    let forward = Vector3::new(-sy * cp, sp, -cy * cp);
    let right = Vector3::new(cy, 0.0, -sy);
    let up = right.cross(forward).normalized();

    Vector3::new(
        forward.x + right.x * ndc_x * aspect * tan_half + up.x * ndc_y * tan_half,
        forward.y + right.y * ndc_x * aspect * tan_half + up.y * ndc_y * tan_half,
        forward.z + right.z * ndc_x * aspect * tan_half + up.z * ndc_y * tan_half,
    )
    .normalized()
}
