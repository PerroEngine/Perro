use criterion::{Criterion, black_box, criterion_group, criterion_main};
use perro_ids::NodeID;
use perro_nodes::{PhysicsForceEmitter2D, PhysicsForceEmitter3D, Shape2D, Shape3D};
use perro_runtime_api::sub_apis::{
    PhysicsAPI, PhysicsBodyPrediction2D, PhysicsBodyPrediction3D, PhysicsContact2D,
    PhysicsContact3D, PhysicsModule, PhysicsQueryFilter, PhysicsRayHit2D, PhysicsRayHit3D,
    PhysicsShapeHit2D, PhysicsShapeHit3D,
};
use perro_structs::{Quaternion, Vector2, Vector3};

struct BenchPhysics {
    gravity: f32,
    coefficient: f32,
    calls: u64,
}

impl PhysicsAPI for BenchPhysics {
    fn get_gravity(&mut self) -> f32 {
        self.gravity
    }

    fn set_gravity(&mut self, gravity: f32) {
        self.gravity = gravity;
    }

    fn get_coefficient(&mut self) -> f32 {
        self.coefficient
    }

    fn set_coefficient(&mut self, coefficient: f32) {
        self.coefficient = coefficient;
    }

    fn apply_force_2d(&mut self, _body_id: NodeID, _force: Vector2) -> bool {
        self.calls = self.calls.wrapping_add(1);
        true
    }

    fn apply_force_3d(&mut self, _body_id: NodeID, _force: Vector3) -> bool {
        self.calls = self.calls.wrapping_add(1);
        true
    }

    fn apply_impulse_2d(&mut self, _body_id: NodeID, _impulse: Vector2) -> bool {
        self.calls = self.calls.wrapping_add(1);
        true
    }

    fn apply_impulse_3d(&mut self, _body_id: NodeID, _impulse: Vector3) -> bool {
        self.calls = self.calls.wrapping_add(1);
        true
    }

    fn emit_force_2d(&mut self, _emitter: PhysicsForceEmitter2D) -> bool {
        self.calls = self.calls.wrapping_add(1);
        true
    }

    fn emit_force_3d(&mut self, _emitter: PhysicsForceEmitter3D) -> bool {
        self.calls = self.calls.wrapping_add(1);
        true
    }

    fn raycast_3d(
        &mut self,
        _origin: Vector3,
        _direction: Vector3,
        _max_distance: f32,
        _include_areas: bool,
    ) -> Option<PhysicsRayHit3D> {
        None
    }

    fn raycast_2d(
        &mut self,
        _origin: Vector2,
        _direction: Vector2,
        _max_distance: f32,
        _filter: PhysicsQueryFilter,
    ) -> Option<PhysicsRayHit2D> {
        None
    }

    fn shape_cast_2d(
        &mut self,
        _shape: Shape2D,
        _origin: Vector2,
        _direction: Vector2,
        _max_distance: f32,
        _filter: PhysicsQueryFilter,
    ) -> Option<PhysicsShapeHit2D> {
        None
    }

    fn shape_cast_3d(
        &mut self,
        _shape: Shape3D,
        _origin: Vector3,
        _direction: Vector3,
        _max_distance: f32,
        _filter: PhysicsQueryFilter,
    ) -> Option<PhysicsShapeHit3D> {
        None
    }

    fn contacts_2d(&mut self, _body_id: NodeID) -> Vec<PhysicsContact2D> {
        Vec::new()
    }

    fn contacts_3d(&mut self, _body_id: NodeID) -> Vec<PhysicsContact3D> {
        Vec::new()
    }

    fn physics_pause(&mut self, _paused: bool) {}

    fn physics_is_paused(&mut self) -> bool {
        false
    }

    fn predict_body_2d(
        &mut self,
        _body_id: NodeID,
        time: f32,
        drift: Vector2,
    ) -> Option<PhysicsBodyPrediction2D> {
        let gravity = Vector2::new(0.0, self.get_effective_gravity());
        Some(PhysicsBodyPrediction2D {
            position: drift * time + gravity * (0.5 * time * time),
            rotation: time,
            velocity: drift + gravity * time,
            angular_velocity: 1.0,
        })
    }

    fn predict_body_3d(
        &mut self,
        _body_id: NodeID,
        time: f32,
        drift: Vector3,
    ) -> Option<PhysicsBodyPrediction3D> {
        let gravity = Vector3::new(0.0, self.get_effective_gravity(), 0.0);
        Some(PhysicsBodyPrediction3D {
            position: drift * time + gravity * (0.5 * time * time),
            rotation: Quaternion::IDENTITY,
            velocity: drift + gravity * time,
            angular_velocity: Vector3::new(0.0, time, 0.0),
        })
    }
}

fn bench_api_force_impulse_emit_dispatch(c: &mut Criterion) {
    c.bench_function("physics_api/force_impulse_emit_dispatch_4096", |b| {
        let ids = (0..4096).map(NodeID::new).collect::<Vec<_>>();
        b.iter(|| {
            let mut runtime = BenchPhysics {
                gravity: -9.81,
                coefficient: 1.0,
                calls: 0,
            };
            for &id in black_box(&ids) {
                let mut physics = PhysicsModule::new(&mut runtime);
                black_box(physics.apply_force(id, Vector2::new(1.0, 0.5)));
                black_box(physics.apply_force(id, Vector3::new(1.0, 0.5, -0.25)));
                black_box(physics.apply_impulse(id, Vector2::new(0.2, 0.1)));
                black_box(physics.apply_impulse(id, Vector3::new(0.2, 0.1, -0.05)));
                black_box(physics.emit_force_2d(PhysicsForceEmitter2D::new()));
                black_box(physics.emit_force_3d(PhysicsForceEmitter3D::new()));
            }
            black_box(runtime.calls)
        })
    });
}

fn bench_api_prediction_dispatch(c: &mut Criterion) {
    c.bench_function("physics_api/prediction_dispatch_4096", |b| {
        let ids = (0..4096).map(NodeID::new).collect::<Vec<_>>();
        b.iter(|| {
            let mut runtime = BenchPhysics {
                gravity: -9.81,
                coefficient: 1.0,
                calls: 0,
            };
            let mut acc = Vector3::ZERO;
            for (i, &id) in black_box(&ids).iter().enumerate() {
                let f = i as f32;
                let mut physics = PhysicsModule::new(&mut runtime);
                if let Some(predicted) =
                    physics.predict_body_2d(id, 0.25 + f * 0.0001, Vector2::new(0.1, -0.2))
                {
                    acc.x += predicted.position.x;
                    acc.y += predicted.velocity.y;
                }
                if let Some(predicted) =
                    physics.predict_body_3d(id, 0.25 + f * 0.0001, Vector3::new(0.1, -0.2, 0.3))
                {
                    acc += predicted.position + predicted.velocity;
                }
            }
            black_box(acc)
        })
    });
}

fn bench_fixed_time_2d(c: &mut Criterion) {
    let cases = (0..1024)
        .map(|i| {
            let f = i as f32;
            (
                Vector2::new(f * 0.03125, f * 0.015625),
                Vector2::new(8.0 + f * 0.02, 2.0 + (i % 17) as f32 * 0.1),
                0.25 + (i % 96) as f32 * 0.01,
                Vector2::new((i % 5) as f32 * 0.1, 0.0),
            )
        })
        .collect::<Vec<_>>();

    c.bench_function("physics_trajectory/fixed_time_2d_1024", |b| {
        let mut runtime = BenchPhysics {
            gravity: -9.81,
            coefficient: 1.0,
            calls: 0,
        };
        b.iter(|| {
            let mut acc = Vector2::ZERO;
            for &(origin, target, time, drift) in black_box(&cases) {
                if let Some(v) = PhysicsModule::new(&mut runtime)
                    .solve_velocity_to_target_2d(origin, target, time, drift)
                {
                    acc += v;
                }
            }
            black_box(acc)
        })
    });
}

fn bench_fixed_speed_3d_no_drift(c: &mut Criterion) {
    let cases = (0..256)
        .map(|i| {
            let f = i as f32;
            (
                Vector3::new(0.0, 0.5 + (i % 7) as f32 * 0.1, 0.0),
                Vector3::new(8.0 + f * 0.05, 1.0 + (i % 11) as f32 * 0.1, -3.0),
                20.0 + (i % 9) as f32,
                5.0,
                Vector3::ZERO,
            )
        })
        .collect::<Vec<_>>();

    c.bench_function("physics_trajectory/fixed_speed_3d_no_drift_256", |b| {
        let mut runtime = BenchPhysics {
            gravity: -9.81,
            coefficient: 1.0,
            calls: 0,
        };
        b.iter(|| {
            let mut acc = Vector3::ZERO;
            for &(origin, target, speed, max_time, drift) in black_box(&cases) {
                if let Some(solution) = PhysicsModule::new(&mut runtime)
                    .solve_launch_velocity_3d(origin, target, speed, max_time, drift)
                {
                    acc += solution.low + solution.high;
                }
            }
            black_box(acc)
        })
    });
}

fn bench_fixed_speed_3d_with_drift(c: &mut Criterion) {
    let cases = (0..256)
        .map(|i| {
            let f = i as f32;
            (
                Vector3::new(0.0, 0.5 + (i % 7) as f32 * 0.1, 0.0),
                Vector3::new(8.0 + f * 0.05, 1.0 + (i % 11) as f32 * 0.1, -3.0),
                20.0 + (i % 9) as f32,
                5.0,
                Vector3::new(0.5, 0.0, -0.2),
            )
        })
        .collect::<Vec<_>>();

    c.bench_function("physics_trajectory/fixed_speed_3d_drift_256", |b| {
        let mut runtime = BenchPhysics {
            gravity: -9.81,
            coefficient: 1.0,
            calls: 0,
        };
        b.iter(|| {
            let mut acc = Vector3::ZERO;
            for &(origin, target, speed, max_time, drift) in black_box(&cases) {
                if let Some(solution) = PhysicsModule::new(&mut runtime)
                    .solve_launch_velocity_3d(origin, target, speed, max_time, drift)
                {
                    acc += solution.low + solution.high;
                }
            }
            black_box(acc)
        })
    });
}

criterion_group! {
    name = physics_trajectory_solver;
    config = Criterion::default().sample_size(20);
    targets = bench_api_force_impulse_emit_dispatch, bench_api_prediction_dispatch, bench_fixed_time_2d, bench_fixed_speed_3d_no_drift, bench_fixed_speed_3d_with_drift
}
criterion_main!(physics_trajectory_solver);
