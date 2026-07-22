use perro_api::prelude::*;
use std::time::Duration;

type SelfNodeType = Node3D;

#[State]
struct PhysicsCollisionsDemoState {
    #[default = NodeID::nil()]
    pub overlay: NodeID,
    #[default = NodeID::nil()]
    pub area_mesh: NodeID,
    #[default = NodeID::nil()]
    pub area_probe: NodeID,
    #[default = NodeID::nil()]
    pub left_ball: NodeID,
    #[default = NodeID::nil()]
    pub right_ball: NodeID,
    #[default = NodeID::nil()]
    pub drop_ball: NodeID,
    #[default = MaterialID::nil()]
    pub area_idle_material: MaterialID,
    #[default = MaterialID::nil()]
    pub area_active_material: MaterialID,
    #[default = false]
    pub area_active: bool,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let idle = material_create!(
            ctx.res,
            area_material([0.08, 0.70, 0.95, 0.28], [0.02, 0.18, 0.30])
        );
        let active = material_create!(
            ctx.res,
            area_material([1.0, 0.22, 0.12, 0.52], [1.0, 0.08, 0.02])
        );

        with_state_mut!(ctx.run, PhysicsCollisionsDemoState, ctx.id, |state| {
            state.overlay = NodeID::nil();
            state.area_idle_material = idle;
            state.area_active_material = active;
        });

        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("SignalArea_Entered"),
            func!("on_area_entered")
        );
        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("SignalArea_Exited"),
            func!("on_area_exited")
        );
        signal_connect!(
            ctx.run,
            ctx.id,
            timer_finished!("physics_collisions_reset"),
            func!("on_reset_timer")
        );

        self.reset_bodies(ctx);
        self.set_area_active(ctx, false);
        self.push_overlay(ctx);
        timer_start!(ctx.run, Duration::from_secs(7), "physics_collisions_reset");
    }

    fn on_removal(&self, ctx: &mut ScriptContext<'_, API>) {
        timer_cancel!(ctx.run, "physics_collisions_reset");
    }
});

methods!({
    fn set_info_overlay(&self, ctx: &mut ScriptContext<'_, API>, overlay: NodeID) {
        with_state_mut!(ctx.run, PhysicsCollisionsDemoState, ctx.id, |state| {
            state.overlay = overlay;
        });
        self.push_overlay(ctx);
    }

    fn on_area_entered(&self, ctx: &mut ScriptContext<'_, API>, _area: NodeID, _other: NodeID) {
        self.set_area_active(ctx, true);
        self.push_overlay(ctx);
    }

    fn on_area_exited(&self, ctx: &mut ScriptContext<'_, API>, _area: NodeID, _other: NodeID) {
        self.set_area_active(ctx, false);
        self.push_overlay(ctx);
    }

    fn on_reset_timer(&self, ctx: &mut ScriptContext<'_, API>) {
        self.reset_bodies(ctx);
        self.set_area_active(ctx, false);
        self.push_overlay(ctx);
        timer_start!(ctx.run, Duration::from_secs(7), "physics_collisions_reset");
    }

    fn reset_bodies(&self, ctx: &mut ScriptContext<'_, API>) {
        let (area_probe, left_ball, right_ball, drop_ball) =
            with_state!(ctx.run, PhysicsCollisionsDemoState, ctx.id, |state| {
                (
                    state.area_probe,
                    state.left_ball,
                    state.right_ball,
                    state.drop_ball,
                )
            }).unwrap_or_default();

        reset_body(
            ctx,
            area_probe,
            Vector3::new(-8.5, 2.2, 2.2),
            Vector3::new(5.8, 0.0, 0.0),
        );
        reset_body(
            ctx,
            left_ball,
            Vector3::new(-5.2, 2.0, -3.2),
            Vector3::new(4.2, 0.0, 0.0),
        );
        reset_body(
            ctx,
            right_ball,
            Vector3::new(5.2, 2.0, -3.2),
            Vector3::new(-4.2, 0.0, 0.0),
        );
        reset_body(
            ctx,
            drop_ball,
            Vector3::new(0.0, 6.2, -0.4),
            Vector3::new(1.4, 0.0, 0.0),
        );
    }

    fn set_area_active(&self, ctx: &mut ScriptContext<'_, API>, active: bool) {
        let (mesh, idle, active_material) =
            with_state!(ctx.run, PhysicsCollisionsDemoState, ctx.id, |state| {
                (
                    state.area_mesh,
                    state.area_idle_material,
                    state.area_active_material,
                )
            }).unwrap_or_default();
        if mesh.is_nil() {
            return;
        }
        let material = if active { active_material } else { idle };
        if material.is_nil() {
            return;
        }
        with_node_mut!(ctx.run, MeshInstance3D, mesh, |node| {
            node.set_material(material);
        });
        with_state_mut!(ctx.run, PhysicsCollisionsDemoState, ctx.id, |state| {
            state.area_active = active;
        });
    }

    fn push_overlay(&self, ctx: &mut ScriptContext<'_, API>) {
        let overlay = with_state!(ctx.run, PhysicsCollisionsDemoState, ctx.id, |state| state
            .overlay).unwrap_or_default();
        if overlay.is_nil() {
            return;
        }
        let rigid = query!(ctx.run, all(node_type[RigidBody3D]), in_subtree(ctx.id)).len();
        let active = with_state!(ctx.run, PhysicsCollisionsDemoState, ctx.id, |state| state
            .area_active).unwrap_or_default();
        let body = format!(
            "rigid bodies {}\nsignal area {}",
            rigid,
            if active { "active" } else { "idle" }
        );
        let _ = call_method!(
            ctx.run,
            overlay,
            func!("set_content"),
            params!["Physics Collisions".to_string(), body]
        );
    }
});

fn reset_body<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    body: NodeID,
    pos: Vector3,
    velocity: Vector3,
) {
    if body.is_nil() {
        return;
    }
    let _ = set_global_pos_3d!(ctx.run, body, pos);
    with_node_mut!(ctx.run, RigidBody3D, body, |node| {
        node.linear_velocity = velocity;
        node.angular_velocity = Vector3::ZERO;
    });
}

fn area_material(color: [f32; 4], emissive: [f32; 3]) -> Material3D {
    let mut material = Material3D::default();
    if let Material3D::Standard(params) = &mut material {
        params.base_color_factor = color;
        params.emissive_factor = emissive;
        params.roughness_factor = 0.45;
        params.metallic_factor = 0.0;
        params.alpha_mode = 2;
        params.double_sided = true;
    }
    material
}
