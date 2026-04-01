use perro::{ids::NodeID, prelude::*};
use std::borrow::Cow;

type SelfNodeType = Camera3D;

#[State]
pub struct CameraState {
    #[default = 24.0]
     
    move_speed: f32,

    #[default = 0.0025]
     
    look_sensitivity: f32,

    #[default = 0.0]
    yaw: f32,

    #[default = 0.0]
    pitch: f32,

    mesh: NodeID,

    print_name: String,
}

lifecycle!({
    fn on_init(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        node: NodeID,
    ) {
        //enable_colorblind_filter!(res, ColorBlindFilter::Protan, 0.8);
        //enable_colorblind_filter!(res, ColorBlindFilter::Deuteran, 0.8);
        //enable_colorblind_filter!(res, ColorBlindFilter::Tritan, 0.8);
        //enable_colorblind_filter!(res, ColorBlindFilter::Achroma, 0.8);
        let (mesh_id, print_name) = with_state!(ctx, CameraState, node, |state| {
            (state.mesh, state.print_name.clone())
        });
        let mesh_id = if mesh_id.is_nil() {
            println!(
                "Camera node {} has no mesh exposed variable, defaulting to self",
                node
            );
            node
        } else {
            mesh_id
        };

        let name = get_node_name!(ctx, mesh_id);
        println!("Camera node {} has external mesh exposed variable named '{:?}'", node, name);
        println!("Camera node {} has external print_name variable named '{}'", node, print_name);

        let en_init = locale!(res, "camera.init");
        let en_prompt = locale!(res, "camera.prompt");
        println!("[locale en] {}", en_init);
        println!("[locale en] {}", en_prompt);

        locale_set!(res, Locale::ES);
        let es_init = locale!(res, "camera.init");
        let es_prompt = locale!(res, "camera.prompt");
        println!("[locale es] {}", es_init);
        println!("[locale es] {}", es_prompt);

        locale_set!(res, Locale::EN);
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

        let mut requested_post: Option<Cow<'static, [PostProcessEffect]>> = None;

        if ipt.Keys().pressed(KeyCode::Digit0) {
            requested_post = Some(Cow::Borrowed(&[]));
        }
        if ipt.Keys().pressed(KeyCode::Digit1) {
            requested_post = Some(Cow::Owned(vec![PostProcessEffect::Blur { strength: 4.0 }]));
        }
        if ipt.Keys().pressed(KeyCode::Digit2) {
            requested_post = Some(Cow::Owned(vec![PostProcessEffect::Pixelate { size: 6.0 }]));
        }
        if ipt.Keys().pressed(KeyCode::Digit3) {
            requested_post = Some(Cow::Owned(vec![PostProcessEffect::Warp {
                waves: 12.0,
                strength: 6.0,
            }]));
        }
        if ipt.Keys().pressed(KeyCode::Digit4) {
            requested_post = Some(Cow::Owned(vec![PostProcessEffect::Vignette {
                strength: 0.5,
                radius: 0.35,
                softness: 0.1,
            }]));
        }
        if ipt.Keys().pressed(KeyCode::Digit5) {
            requested_post = Some(Cow::Owned(vec![PostProcessEffect::Crt {
                scanline_strength: 0.85,
                curvature: 0.35,
                chromatic: 5.0,
                vignette: 0.2,
            }]));
        }
        if ipt.Keys().pressed(KeyCode::Digit6) {
            requested_post = Some(Cow::Owned(vec![PostProcessEffect::ColorFilter {
                color: [1.0, 0.8, 0.6],
                strength: 0.95,
            }]));
        }
        if ipt.Keys().pressed(KeyCode::Digit7) {
            requested_post = Some(Cow::Owned(vec![PostProcessEffect::ReverseFilter {
                color: [0.1, 0.8, 0.2],
                strength: 0.98,
                softness: 0.3,
            }]));
        }
        if ipt.Keys().pressed(KeyCode::Digit8) {
            requested_post = Some(Cow::Owned(vec![PostProcessEffect::BlackWhite {
                amount: 1.0,
            }]));
        }
        if ipt.Keys().pressed(KeyCode::Digit9) {
            requested_post = Some(Cow::Owned(vec![PostProcessEffect::Custom {
                shader_path: Cow::Borrowed("res://shaders/post_rgb_split.wgsl"),
                params: Cow::Owned(vec![
                    CustomPostParam::unnamed(CustomPostParamValue::F32(12.0)),
                    CustomPostParam::unnamed(CustomPostParamValue::F32(0.6)),
                    CustomPostParam::unnamed(CustomPostParamValue::F32(1.4)),
                    CustomPostParam::unnamed(CustomPostParamValue::F32(0.2)),
                ]),
            }]));
        }

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
            camera.rotation = Quaternion::IDENTITY;
            camera.rotation.rotate_x(pitch);
            camera.rotation.rotate_y(yaw);

            camera.position.x += move_x * step;
            camera.position.y += y * step;
            camera.position.z += move_z * step;

            if let Some(post) = requested_post {
                camera.post_processing = PostProcessSet::from_effects(post.into_owned());
            }
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

methods!({});
