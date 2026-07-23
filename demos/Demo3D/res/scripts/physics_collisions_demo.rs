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
    #[default = 0.0]
    pub spawn_time: f32,
    #[default = 0]
    pub spawn_index: u32,
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

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run).max(0.0);
        let spawn = with_state_mut!(ctx.run, PhysicsCollisionsDemoState, ctx.id, |state| {
            state.spawn_time += dt;
            if state.spawn_time < 0.14 {
                return false;
            }
            state.spawn_time -= 0.14;
            true
        })
        .unwrap_or(false);
        if spawn {
            self.spawn_rain_ball(ctx);
        }
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
        for ball in query!(ctx.run, all(tags["physics_rain_ball"]), in_subtree(ctx.id)) {
            let _ = remove_node!(ctx.run, ball);
        }
        with_state_mut!(ctx.run, PhysicsCollisionsDemoState, ctx.id, |state| {
            state.spawn_time = 0.0;
        });

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
        for _ in 0..28 {
            self.spawn_rain_ball(ctx);
        }
    }

    fn spawn_rain_ball(&self, ctx: &mut ScriptContext<'_, API>) {
        let balls = query!(ctx.run, all(tags["physics_rain_ball"]), in_subtree(ctx.id));
        if balls.len() >= 110 {
            let _ = remove_node!(ctx.run, balls[0]);
        }

        let index = with_state_mut!(ctx.run, PhysicsCollisionsDemoState, ctx.id, |state| {
            let index = state.spawn_index;
            state.spawn_index = index.wrapping_add(1);
            index
        })
        .unwrap_or(0);
        let hash = index.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let x = (hash % 1501) as f32 * 0.01 - 7.5;
        let z = ((hash >> 11) % 851) as f32 * 0.01 - 4.25;
        let y = 8.0 + ((hash >> 21) % 500) as f32 * 0.01;
        let radius = 0.24 + ((hash >> 16) % 43) as f32 * 0.01;

        let body = create_node!(
            ctx.run,
            RigidBody3D,
            "rain_ball",
            tags!["physics_rain_ball"],
            ctx.id
        );
        let _ = with_node_mut!(ctx.run, RigidBody3D, body, |node| {
            node.transform.position = Vector3::new(x, y, z);
            node.mass = 0.65 + radius * 1.8;
            node.gravity_scale = 1.0;
            node.friction = 0.42;
            node.restitution = 0.62;
            node.continuous_collision_detection = true;
            node.linear_velocity = Vector3::new(
                ((hash >> 5) % 100) as f32 * 0.006 - 0.3,
                0.0,
                ((hash >> 14) % 100) as f32 * 0.006 - 0.3,
            );
        });

        let shape = create_node!(
            ctx.run,
            CollisionShape3D,
            "rain_ball_shape",
            tags!["physics_rain_part"],
            body
        );
        let _ = with_node_mut!(ctx.run, CollisionShape3D, shape, |node| {
            node.shape = Shape3D::Sphere { radius };
        });

        let mesh = create_node!(
            ctx.run,
            MeshInstance3D,
            "rain_ball_mesh",
            tags!["physics_rain_part"],
            body
        );
        let color = palette_color((index as f32 * 0.618_034).fract());
        let material = material_create!(ctx.res, ball_material(color));
        let sphere = mesh_load!(ctx.res, "__sphere__");
        let _ = with_node_mut!(ctx.run, MeshInstance3D, mesh, |node| {
            node.mesh = sphere;
            node.transform.scale = Vector3::new(radius, radius, radius);
            node.set_material(material);
        });
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

fn palette_color(t: f32) -> [f32; 4] {
    let a = std::f32::consts::TAU * t;
    [
        0.5 + 0.5 * a.cos(),
        0.5 + 0.5 * (a + 2.094).cos(),
        0.5 + 0.5 * (a + 4.188).cos(),
        1.0,
    ]
}

fn ball_material(color: [f32; 4]) -> Material3D {
    let mut material = Material3D::default();
    if let Material3D::Standard(params) = &mut material {
        params.base_color_factor = color;
        params.roughness_factor = 0.32;
        params.metallic_factor = 0.05;
    }
    material
}
