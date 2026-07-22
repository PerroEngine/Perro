use perro_api::prelude::*;

type SelfNodeType = Node3D;

const PROJECTILE_SCENE_PATH: &ResPath = res_path!("res://scenes/demos/physics_bone_projectile.scn");

#[State]
struct PhysicsBonesDemoState {
    #[default = NodeID::nil()]
    pub overlay: NodeID,
    #[default = NodeID::nil()]
    pub camera: NodeID,
    #[default = NodeID::nil()]
    pub projectiles: NodeID,
    #[default = 0.38]
    pub radius: f32,
    #[default = 18.0]
    pub speed: f32,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        with_state_mut!(ctx.run, PhysicsBonesDemoState, ctx.id, |state| {
            state.overlay = NodeID::nil();
        });
        self.push_overlay(ctx);
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        if mouse_mode!(ctx.ipt) != MouseMode::Captured {
            return;
        }

        let wheel = mouse_wheel!(ctx.ipt).y;
        if wheel.abs() > 0.001 {
            with_state_mut!(ctx.run, PhysicsBonesDemoState, ctx.id, |state| {
                state.radius = (state.radius + wheel * 0.05).clamp(0.15, 0.85);
            });
        }

        if mouse_pressed!(ctx.ipt, MouseButton::Left) {
            self.fire(ctx);
        }
        self.push_overlay(ctx);
    }
});

methods!({
    fn set_info_overlay(&self, ctx: &mut ScriptContext<'_, API>, overlay: NodeID) {
        with_state_mut!(ctx.run, PhysicsBonesDemoState, ctx.id, |state| {
            state.overlay = overlay;
        });
        self.push_overlay(ctx);
    }

    fn fire(&self, ctx: &mut ScriptContext<'_, API>) {
        let (camera, projectiles, radius, speed) =
            with_state!(ctx.run, PhysicsBonesDemoState, ctx.id, |state| {
                (state.camera, state.projectiles, state.radius, state.speed)
            }).unwrap_or_default();
        if camera.is_nil() || projectiles.is_nil() {
            return;
        }

        let Some(camera_world) = get_global_transform_3d!(ctx.run, camera) else {
            return;
        };
        let forward = camera_world
            .rotation
            .rotate_vector3(Vector3::new(0.0, 0.0, -1.0))
            .normalized();
        let spawn_pos = camera_world.position + forward * (radius + 0.9);

        let root = match scene_load!(ctx.run, PROJECTILE_SCENE_PATH) {
            Ok(id) => id,
            Err(err) => {
                log_error!("[PhysicsBonesDemo] projectile load fail: {:?}", err);
                return;
            }
        };
        reparent!(ctx.run, projectiles, root);
        let _ = set_global_pos_3d!(ctx.run, root, spawn_pos);
        let _ = call_method!(
            ctx.run,
            root,
            func!("launch"),
            params![forward * speed, radius]
        );
        self.push_overlay(ctx);
    }

    fn push_overlay(&self, ctx: &mut ScriptContext<'_, API>) {
        let (overlay, radius, projectiles) =
            with_state!(ctx.run, PhysicsBonesDemoState, ctx.id, |state| {
                (state.overlay, state.radius, state.projectiles)
            }).unwrap_or_default();
        if overlay.is_nil() {
            return;
        }
        let projectile_cnt = if projectiles.is_nil() {
            0
        } else {
            query!(
                ctx.run,
                all(node_type[RigidBody3D]),
                in_subtree(projectiles)
            )
            .len()
        };
        let chains = query!(
            ctx.run,
            all(node_type[PhysicsBoneChain3D]),
            in_subtree(ctx.id)
        )
        .len();
        let body = format!(
            "bone chains {}\nprojectiles {} | radius {:.2}",
            chains, projectile_cnt, radius
        );
        let _ = call_method!(
            ctx.run,
            overlay,
            func!("set_content"),
            params!["Physics Bones".to_string(), body]
        );
    }
});
