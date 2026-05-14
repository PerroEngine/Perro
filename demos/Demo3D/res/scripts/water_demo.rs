use perro_api::prelude::*;

type SelfNodeType = Node3D;

const CANNON_BALL_SCENE_PATH: &ResPath = res_path!("res://scenes/demos/cannon_ball.scn");
const CAMERA_NODE_NAME: &str = "DemoCamera";
const PROJECTILES_NODE_NAME: &str = "Projectiles";
const BALL_MESH_NODE_NAME: &str = "CannonBallMesh";
const BALL_SHAPE_NODE_NAME: &str = "CannonBallShape";

#[State]
struct WaterDemoState {
    #[default = NodeID::nil()]
    pub camera: NodeID,
    #[default = NodeID::nil()]
    pub projectiles: NodeID,
    #[default = 0.45]
    pub radius: f32,
    #[default = 2.0]
    pub mass: f32,
    #[default = 34.0]
    pub speed: f32,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let camera = get_child!(ctx.run, ctx.id, CAMERA_NODE_NAME).unwrap_or(NodeID::nil());
        let projectiles =
            get_child!(ctx.run, ctx.id, PROJECTILES_NODE_NAME).unwrap_or(NodeID::nil());
        with_state_mut!(ctx.run, WaterDemoState, ctx.id, |state| {
            state.camera = camera;
            state.projectiles = projectiles;
        });
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        if mouse_mode!(ctx.ipt) != MouseMode::Captured {
            return;
        }

        let wheel = mouse_wheel!(ctx.ipt).y;
        if wheel.abs() > 0.001 {
            with_state_mut!(ctx.run, WaterDemoState, ctx.id, |state| {
                state.radius = (state.radius + wheel * 0.06).clamp(0.18, 1.25);
                state.mass = (state.radius * state.radius * state.radius * 22.0).clamp(0.4, 42.0);
            });
        }

        if mouse_pressed!(ctx.ipt, MouseButton::Left) {
            self.fire(ctx);
        }
    }
});

methods!({
    fn fire(&self, ctx: &mut ScriptContext<'_, API>) {
        let (camera, projectiles, radius, mass, speed) =
            with_state!(ctx.run, WaterDemoState, ctx.id, |state| {
                (
                    state.camera,
                    state.projectiles,
                    state.radius,
                    state.mass,
                    state.speed,
                )
            });

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
        let spawn_pos = camera_world.position + forward * (radius + 0.85);

        let root = match scene_load!(ctx.run, CANNON_BALL_SCENE_PATH) {
            Ok(id) => id,
            Err(err) => {
                log_error!("[WaterDemo] projectile load fail: {:?}", err);
                return;
            }
        };
        reparent!(ctx.run, projectiles, root);
        let _ = set_global_pos_3d!(ctx.run, root, spawn_pos);

        with_node_mut!(ctx.run, RigidBody3D, root, |body| {
            body.mass = mass;
            body.linear_velocity = forward * speed;
            body.angular_velocity = Vector3::new(0.0, 5.0 / radius.max(0.01), 0.0);
        });

        if let Some(mesh) = get_child!(ctx.run, root, BALL_MESH_NODE_NAME) {
            let diameter = radius * 2.0;
            let _ = set_local_scale_3d!(ctx.run, mesh, Vector3::new(diameter, diameter, diameter));
        }
        if let Some(shape) = get_child!(ctx.run, root, BALL_SHAPE_NODE_NAME) {
            with_node_mut!(ctx.run, CollisionShape3D, shape, |shape| {
                shape.shape = Shape3D::Sphere { radius };
            });
        }
    }
});
