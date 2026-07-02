//! Runtime physics API.
//!
//! Controls global physics state and exposes raycasts, shape queries, force
//! application, and launch/trajectory helpers.

use perro_ids::NodeID;
use perro_nodes::{PhysicsForceEmitter2D, PhysicsForceEmitter3D, Shape2D, Shape3D};
use perro_structs::{BitMask, Quaternion, Vector2, Vector3};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhysicsQueryFilter {
    /// Hit layer membership filter. Default is all layers.
    pub layers: BitMask,
    /// Ignore layer filter. Matching collider layers are skipped.
    pub mask: BitMask,
    pub include_areas: bool,
    pub exclude_nodes: Vec<NodeID>,
}

impl Default for PhysicsQueryFilter {
    fn default() -> Self {
        Self {
            layers: BitMask::ALL,
            mask: BitMask::NONE,
            include_areas: true,
            exclude_nodes: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsRayHit2D {
    pub node: NodeID,
    pub point: Vector2,
    pub normal: Vector2,
    pub distance: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsRayHit3D {
    pub node: NodeID,
    pub point: Vector3,
    pub normal: Vector3,
    pub distance: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsShapeHit2D {
    pub node: NodeID,
    pub point: Vector2,
    pub normal: Vector2,
    pub distance: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsShapeHit3D {
    pub node: NodeID,
    pub point: Vector3,
    pub normal: Vector3,
    pub distance: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsMoveResult2D {
    pub position: Vector2,
    pub hit: Option<PhysicsShapeHit2D>,
    pub clipped: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsMoveResult3D {
    pub position: Vector3,
    pub hit: Option<PhysicsShapeHit3D>,
    pub clipped: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsContact2D {
    pub node: NodeID,
    pub point: Vector2,
    pub normal: Vector2,
    pub impulse: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsContact3D {
    pub node: NodeID,
    pub point: Vector3,
    pub normal: Vector3,
    pub impulse: f32,
}

/// Low and high 2D launch velocities for a fixed-speed projectile solve.
///
/// `low` is the shortest valid flight-time arc.
/// `high` is the longest valid flight-time arc.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsLaunchSolution2D {
    pub low: Vector2,
    pub high: Vector2,
}

/// Low and high 3D launch velocities for a fixed-speed projectile solve.
///
/// `low` is the shortest valid flight-time arc.
/// `high` is the longest valid flight-time arc.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsLaunchSolution3D {
    pub low: Vector3,
    pub high: Vector3,
}

/// Predicted 2D rigidbody state from cheap kinematic integration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsBodyPrediction2D {
    pub position: Vector2,
    pub rotation: f32,
    pub velocity: Vector2,
    pub angular_velocity: f32,
}

/// Predicted 3D rigidbody state from cheap kinematic integration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsBodyPrediction3D {
    pub position: Vector3,
    pub rotation: Quaternion,
    pub velocity: Vector3,
    pub angular_velocity: Vector3,
}

pub trait PhysicsAPI {
    fn get_gravity(&mut self) -> f32;
    fn set_gravity(&mut self, gravity: f32);
    fn get_coefficient(&mut self) -> f32;
    fn set_coefficient(&mut self, coefficient: f32);
    fn apply_force_2d(&mut self, body_id: NodeID, force: Vector2) -> bool;
    fn apply_force_3d(&mut self, body_id: NodeID, force: Vector3) -> bool;
    fn apply_impulse_2d(&mut self, body_id: NodeID, impulse: Vector2) -> bool;
    fn apply_impulse_3d(&mut self, body_id: NodeID, impulse: Vector3) -> bool;
    fn emit_force_2d(&mut self, _emitter: PhysicsForceEmitter2D) -> bool {
        false
    }
    fn emit_force_3d(&mut self, _emitter: PhysicsForceEmitter3D) -> bool {
        false
    }
    fn raycast_3d(
        &mut self,
        origin: Vector3,
        direction: Vector3,
        max_distance: f32,
        include_areas: bool,
    ) -> Option<PhysicsRayHit3D>;
    fn raycast_3d_filtered(
        &mut self,
        origin: Vector3,
        direction: Vector3,
        max_distance: f32,
        filter: PhysicsQueryFilter,
    ) -> Option<PhysicsRayHit3D> {
        self.raycast_3d(origin, direction, max_distance, filter.include_areas)
    }
    fn raycast_2d(
        &mut self,
        origin: Vector2,
        direction: Vector2,
        max_distance: f32,
        filter: PhysicsQueryFilter,
    ) -> Option<PhysicsRayHit2D>;
    fn shape_cast_2d(
        &mut self,
        shape: Shape2D,
        origin: Vector2,
        direction: Vector2,
        max_distance: f32,
        filter: PhysicsQueryFilter,
    ) -> Option<PhysicsShapeHit2D>;
    fn shape_cast_3d(
        &mut self,
        shape: Shape3D,
        origin: Vector3,
        direction: Vector3,
        max_distance: f32,
        filter: PhysicsQueryFilter,
    ) -> Option<PhysicsShapeHit3D>;
    fn move_body_2d(
        &mut self,
        body_id: NodeID,
        target: Vector2,
        margin: f32,
        filter: PhysicsQueryFilter,
    ) -> Option<PhysicsMoveResult2D> {
        let _ = (body_id, target, margin, filter);
        None
    }
    fn move_body_3d(
        &mut self,
        body_id: NodeID,
        target: Vector3,
        margin: f32,
        filter: PhysicsQueryFilter,
    ) -> Option<PhysicsMoveResult3D> {
        let _ = (body_id, target, margin, filter);
        None
    }
    fn contacts_2d(&mut self, body_id: NodeID) -> Vec<PhysicsContact2D>;
    fn contacts_3d(&mut self, body_id: NodeID) -> Vec<PhysicsContact3D>;
    fn physics_pause(&mut self, paused: bool);
    fn physics_is_paused(&mut self) -> bool;

    /// Returns the initial 2D velocity needed to hit `target` after `time`.
    ///
    /// Uses effective runtime gravity (`gravity * coefficient`) and treats `drift`
    /// as a constant velocity offset from wind, water, or gameplay current.
    /// Returns `None` for invalid input, non-positive time, or same origin/target.
    fn solve_velocity_to_target_2d(
        &mut self,
        origin: Vector2,
        target: Vector2,
        time: f32,
        drift: Vector2,
    ) -> Option<Vector2> {
        solve_velocity_to_target_2d_core(origin, target, time, drift, self.get_effective_gravity())
    }

    /// Returns the initial 3D velocity needed to hit `target` after `time`.
    ///
    /// Uses effective runtime gravity (`gravity * coefficient`) on world Y and
    /// treats `drift` as a constant velocity offset.
    /// Returns `None` for invalid input, non-positive time, or same origin/target.
    fn solve_velocity_to_target_3d(
        &mut self,
        origin: Vector3,
        target: Vector3,
        time: f32,
        drift: Vector3,
    ) -> Option<Vector3> {
        solve_velocity_to_target_3d_core(origin, target, time, drift, self.get_effective_gravity())
    }

    /// Returns low and high 2D launch arcs for a projectile with fixed `speed`.
    ///
    /// Uses an analytic solve when `drift` is zero.
    /// Otherwise, scans flight times up to `max_time` and refines valid roots.
    /// Returns `None` when target is unreachable or input is invalid.
    fn solve_launch_velocity_2d(
        &mut self,
        origin: Vector2,
        target: Vector2,
        speed: f32,
        max_time: f32,
        drift: Vector2,
    ) -> Option<PhysicsLaunchSolution2D> {
        solve_launch_velocity_2d_core(
            origin,
            target,
            speed,
            max_time,
            drift,
            self.get_effective_gravity(),
        )
    }

    /// Returns low and high 3D launch arcs for a projectile with fixed `speed`.
    ///
    /// Uses an analytic solve when `drift` is zero.
    /// Otherwise, scans flight times up to `max_time` and refines valid roots.
    /// Returns `None` when target is unreachable or input is invalid.
    fn solve_launch_velocity_3d(
        &mut self,
        origin: Vector3,
        target: Vector3,
        speed: f32,
        max_time: f32,
        drift: Vector3,
    ) -> Option<PhysicsLaunchSolution3D> {
        solve_launch_velocity_3d_core(
            origin,
            target,
            speed,
            max_time,
            drift,
            self.get_effective_gravity(),
        )
    }

    fn get_effective_gravity(&mut self) -> f32 {
        self.get_gravity() * self.get_coefficient()
    }

    /// Predicts a 2D rigidbody position/velocity after `time`.
    ///
    /// Implementations should read current body position, velocity, and gravity scale.
    /// This is a cheap kinematic prediction and does not step collisions or mutate state.
    fn predict_body_2d(
        &mut self,
        _body_id: NodeID,
        _time: f32,
        _drift: Vector2,
    ) -> Option<PhysicsBodyPrediction2D> {
        None
    }

    /// Predicts a 3D rigidbody position/velocity after `time`.
    ///
    /// Implementations should read current body position, velocity, and gravity scale.
    /// This is a cheap kinematic prediction and does not step collisions or mutate state.
    fn predict_body_3d(
        &mut self,
        _body_id: NodeID,
        _time: f32,
        _drift: Vector3,
    ) -> Option<PhysicsBodyPrediction3D> {
        None
    }
}

pub trait IntoImpulseDirection {
    fn apply_force<R: PhysicsAPI + ?Sized>(
        self,
        physics: &mut PhysicsModule<'_, R>,
        body_id: NodeID,
    ) -> bool;

    fn apply_impulse<R: PhysicsAPI + ?Sized>(
        self,
        physics: &mut PhysicsModule<'_, R>,
        body_id: NodeID,
    ) -> bool;
}

impl IntoImpulseDirection for Vector2 {
    fn apply_force<R: PhysicsAPI + ?Sized>(
        self,
        physics: &mut PhysicsModule<'_, R>,
        body_id: NodeID,
    ) -> bool {
        physics.apply_force_2d(body_id, self)
    }

    fn apply_impulse<R: PhysicsAPI + ?Sized>(
        self,
        physics: &mut PhysicsModule<'_, R>,
        body_id: NodeID,
    ) -> bool {
        physics.apply_impulse_2d(body_id, self)
    }
}

impl IntoImpulseDirection for Vector3 {
    fn apply_force<R: PhysicsAPI + ?Sized>(
        self,
        physics: &mut PhysicsModule<'_, R>,
        body_id: NodeID,
    ) -> bool {
        physics.apply_force_3d(body_id, self)
    }

    fn apply_impulse<R: PhysicsAPI + ?Sized>(
        self,
        physics: &mut PhysicsModule<'_, R>,
        body_id: NodeID,
    ) -> bool {
        physics.apply_impulse_3d(body_id, self)
    }
}

pub struct PhysicsModule<'rt, R: PhysicsAPI + ?Sized> {
    rt: &'rt mut R,
}

impl<'rt, R: PhysicsAPI + ?Sized> PhysicsModule<'rt, R> {
    pub fn new(rt: &'rt mut R) -> Self {
        Self { rt }
    }

    pub fn apply_force_2d(&mut self, body_id: NodeID, force: Vector2) -> bool {
        self.rt.apply_force_2d(body_id, force)
    }

    pub fn get_gravity(&mut self) -> f32 {
        self.rt.get_gravity()
    }

    pub fn set_gravity(&mut self, gravity: f32) {
        self.rt.set_gravity(gravity);
    }

    pub fn get_coefficient(&mut self) -> f32 {
        self.rt.get_coefficient()
    }

    pub fn set_coefficient(&mut self, coefficient: f32) {
        self.rt.set_coefficient(coefficient);
    }

    pub fn apply_force_3d(&mut self, body_id: NodeID, force: Vector3) -> bool {
        self.rt.apply_force_3d(body_id, force)
    }

    pub fn apply_impulse_2d(&mut self, body_id: NodeID, impulse: Vector2) -> bool {
        self.rt.apply_impulse_2d(body_id, impulse)
    }

    pub fn apply_impulse_3d(&mut self, body_id: NodeID, impulse: Vector3) -> bool {
        self.rt.apply_impulse_3d(body_id, impulse)
    }

    pub fn emit_force_2d(&mut self, emitter: PhysicsForceEmitter2D) -> bool {
        self.rt.emit_force_2d(emitter)
    }

    pub fn emit_force_3d(&mut self, emitter: PhysicsForceEmitter3D) -> bool {
        self.rt.emit_force_3d(emitter)
    }

    pub fn apply_force<D>(&mut self, body_id: NodeID, force: D) -> bool
    where
        D: IntoImpulseDirection,
    {
        force.apply_force(self, body_id)
    }

    pub fn apply_impulse<D>(&mut self, body_id: NodeID, impulse: D) -> bool
    where
        D: IntoImpulseDirection,
    {
        impulse.apply_impulse(self, body_id)
    }

    pub fn raycast_3d(
        &mut self,
        origin: Vector3,
        direction: Vector3,
        max_distance: f32,
    ) -> Option<PhysicsRayHit3D> {
        self.rt.raycast_3d(origin, direction, max_distance, true)
    }

    pub fn raycast_3d_with_areas(
        &mut self,
        origin: Vector3,
        direction: Vector3,
        max_distance: f32,
    ) -> Option<PhysicsRayHit3D> {
        self.rt.raycast_3d(origin, direction, max_distance, true)
    }

    pub fn raycast_3d_without_areas(
        &mut self,
        origin: Vector3,
        direction: Vector3,
        max_distance: f32,
    ) -> Option<PhysicsRayHit3D> {
        self.rt.raycast_3d(origin, direction, max_distance, false)
    }

    pub fn raycast_3d_filtered(
        &mut self,
        origin: Vector3,
        direction: Vector3,
        max_distance: f32,
        filter: PhysicsQueryFilter,
    ) -> Option<PhysicsRayHit3D> {
        self.rt
            .raycast_3d_filtered(origin, direction, max_distance, filter)
    }

    pub fn raycast_2d(
        &mut self,
        origin: Vector2,
        direction: Vector2,
        max_distance: f32,
    ) -> Option<PhysicsRayHit2D> {
        self.rt.raycast_2d(
            origin,
            direction,
            max_distance,
            PhysicsQueryFilter::default(),
        )
    }

    pub fn raycast_2d_filtered(
        &mut self,
        origin: Vector2,
        direction: Vector2,
        max_distance: f32,
        filter: PhysicsQueryFilter,
    ) -> Option<PhysicsRayHit2D> {
        self.rt.raycast_2d(origin, direction, max_distance, filter)
    }

    pub fn shape_cast_2d(
        &mut self,
        shape: Shape2D,
        origin: Vector2,
        direction: Vector2,
        max_distance: f32,
        filter: PhysicsQueryFilter,
    ) -> Option<PhysicsShapeHit2D> {
        self.rt
            .shape_cast_2d(shape, origin, direction, max_distance, filter)
    }

    pub fn shape_cast_3d(
        &mut self,
        shape: Shape3D,
        origin: Vector3,
        direction: Vector3,
        max_distance: f32,
        filter: PhysicsQueryFilter,
    ) -> Option<PhysicsShapeHit3D> {
        self.rt
            .shape_cast_3d(shape, origin, direction, max_distance, filter)
    }

    pub fn move_body_2d(
        &mut self,
        body_id: NodeID,
        target: Vector2,
        margin: f32,
        filter: PhysicsQueryFilter,
    ) -> Option<PhysicsMoveResult2D> {
        self.rt.move_body_2d(body_id, target, margin, filter)
    }

    pub fn move_body_3d(
        &mut self,
        body_id: NodeID,
        target: Vector3,
        margin: f32,
        filter: PhysicsQueryFilter,
    ) -> Option<PhysicsMoveResult3D> {
        self.rt.move_body_3d(body_id, target, margin, filter)
    }

    pub fn contacts_2d(&mut self, body_id: NodeID) -> Vec<PhysicsContact2D> {
        self.rt.contacts_2d(body_id)
    }

    pub fn contacts_3d(&mut self, body_id: NodeID) -> Vec<PhysicsContact3D> {
        self.rt.contacts_3d(body_id)
    }

    /// Returns the initial 2D velocity needed to hit `target` after `time`.
    pub fn solve_velocity_to_target_2d(
        &mut self,
        origin: Vector2,
        target: Vector2,
        time: f32,
        drift: Vector2,
    ) -> Option<Vector2> {
        self.rt
            .solve_velocity_to_target_2d(origin, target, time, drift)
    }

    /// Returns the initial 3D velocity needed to hit `target` after `time`.
    pub fn solve_velocity_to_target_3d(
        &mut self,
        origin: Vector3,
        target: Vector3,
        time: f32,
        drift: Vector3,
    ) -> Option<Vector3> {
        self.rt
            .solve_velocity_to_target_3d(origin, target, time, drift)
    }

    /// Returns low and high 2D launch arcs for a projectile with fixed `speed`.
    pub fn solve_launch_velocity_2d(
        &mut self,
        origin: Vector2,
        target: Vector2,
        speed: f32,
        max_time: f32,
        drift: Vector2,
    ) -> Option<PhysicsLaunchSolution2D> {
        self.rt
            .solve_launch_velocity_2d(origin, target, speed, max_time, drift)
    }

    /// Returns low and high 3D launch arcs for a projectile with fixed `speed`.
    pub fn solve_launch_velocity_3d(
        &mut self,
        origin: Vector3,
        target: Vector3,
        speed: f32,
        max_time: f32,
        drift: Vector3,
    ) -> Option<PhysicsLaunchSolution3D> {
        self.rt
            .solve_launch_velocity_3d(origin, target, speed, max_time, drift)
    }

    /// Predicts a 2D rigidbody position/velocity after `time`.
    pub fn predict_body_2d(
        &mut self,
        body_id: NodeID,
        time: f32,
        drift: Vector2,
    ) -> Option<PhysicsBodyPrediction2D> {
        self.rt.predict_body_2d(body_id, time, drift)
    }

    /// Predicts a 3D rigidbody position/velocity after `time`.
    pub fn predict_body_3d(
        &mut self,
        body_id: NodeID,
        time: f32,
        drift: Vector3,
    ) -> Option<PhysicsBodyPrediction3D> {
        self.rt.predict_body_3d(body_id, time, drift)
    }

    pub fn pause(&mut self, paused: bool) {
        self.rt.physics_pause(paused);
    }

    pub fn is_paused(&mut self) -> bool {
        self.rt.physics_is_paused()
    }
}

const TRAJECTORY_EPS: f32 = 1.0e-5;
const TRAJECTORY_ROOT_EPS: f32 = 1.0e-4;
const TRAJECTORY_SCAN_STEPS: usize = 512;
const TRAJECTORY_REFINE_STEPS: usize = 32;

fn finite2(v: Vector2) -> bool {
    v.x.is_finite() && v.y.is_finite()
}

fn finite3(v: Vector3) -> bool {
    v.x.is_finite() && v.y.is_finite() && v.z.is_finite()
}

fn solve_velocity_to_target_2d_core(
    origin: Vector2,
    target: Vector2,
    time: f32,
    drift: Vector2,
    gravity_y: f32,
) -> Option<Vector2> {
    if !finite2(origin)
        || !finite2(target)
        || !finite2(drift)
        || !time.is_finite()
        || !gravity_y.is_finite()
        || time <= 0.0
        || (target - origin).length_squared() <= TRAJECTORY_EPS * TRAJECTORY_EPS
    {
        return None;
    }

    let gravity = Vector2::new(0.0, gravity_y);
    Some((target - origin - drift * time - gravity * (0.5 * time * time)) / time)
}

fn solve_velocity_to_target_3d_core(
    origin: Vector3,
    target: Vector3,
    time: f32,
    drift: Vector3,
    gravity_y: f32,
) -> Option<Vector3> {
    if !finite3(origin)
        || !finite3(target)
        || !finite3(drift)
        || !time.is_finite()
        || !gravity_y.is_finite()
        || time <= 0.0
        || (target - origin).length_squared() <= TRAJECTORY_EPS * TRAJECTORY_EPS
    {
        return None;
    }

    let gravity = Vector3::new(0.0, gravity_y, 0.0);
    Some((target - origin - drift * time - gravity * (0.5 * time * time)) / time)
}

fn solve_launch_velocity_2d_core(
    origin: Vector2,
    target: Vector2,
    speed: f32,
    max_time: f32,
    drift: Vector2,
    gravity_y: f32,
) -> Option<PhysicsLaunchSolution2D> {
    if !finite2(origin)
        || !finite2(target)
        || !finite2(drift)
        || !speed.is_finite()
        || !max_time.is_finite()
        || !gravity_y.is_finite()
        || speed <= 0.0
        || max_time <= 0.0
        || (target - origin).length_squared() <= TRAJECTORY_EPS * TRAJECTORY_EPS
    {
        return None;
    }

    if drift.length_squared() <= TRAJECTORY_EPS * TRAJECTORY_EPS {
        return solve_launch_velocity_2d_no_drift(origin, target, speed, max_time, gravity_y);
    }

    let mut f = |time: f32| -> Option<f32> {
        let v = solve_velocity_to_target_2d_core(origin, target, time, drift, gravity_y)?;
        Some(v.length_squared() - speed * speed)
    };
    let roots = find_launch_time_range(max_time, &mut f);
    let low_t = roots.low?;
    let high_t = roots.high.unwrap_or(low_t);
    Some(PhysicsLaunchSolution2D {
        low: solve_velocity_to_target_2d_core(origin, target, low_t, drift, gravity_y)?,
        high: solve_velocity_to_target_2d_core(origin, target, high_t, drift, gravity_y)?,
    })
}

fn solve_launch_velocity_3d_core(
    origin: Vector3,
    target: Vector3,
    speed: f32,
    max_time: f32,
    drift: Vector3,
    gravity_y: f32,
) -> Option<PhysicsLaunchSolution3D> {
    if !finite3(origin)
        || !finite3(target)
        || !finite3(drift)
        || !speed.is_finite()
        || !max_time.is_finite()
        || !gravity_y.is_finite()
        || speed <= 0.0
        || max_time <= 0.0
        || (target - origin).length_squared() <= TRAJECTORY_EPS * TRAJECTORY_EPS
    {
        return None;
    }

    if drift.length_squared() <= TRAJECTORY_EPS * TRAJECTORY_EPS {
        return solve_launch_velocity_3d_no_drift(origin, target, speed, max_time, gravity_y);
    }

    let mut f = |time: f32| -> Option<f32> {
        let v = solve_velocity_to_target_3d_core(origin, target, time, drift, gravity_y)?;
        Some(v.length_squared() - speed * speed)
    };
    let roots = find_launch_time_range(max_time, &mut f);
    let low_t = roots.low?;
    let high_t = roots.high.unwrap_or(low_t);
    Some(PhysicsLaunchSolution3D {
        low: solve_velocity_to_target_3d_core(origin, target, low_t, drift, gravity_y)?,
        high: solve_velocity_to_target_3d_core(origin, target, high_t, drift, gravity_y)?,
    })
}

fn solve_launch_velocity_2d_no_drift(
    origin: Vector2,
    target: Vector2,
    speed: f32,
    max_time: f32,
    gravity_y: f32,
) -> Option<PhysicsLaunchSolution2D> {
    let d = target - origin;
    let accel = Vector2::new(0.0, gravity_y);
    let times = solve_no_drift_times(d.length_squared(), d.y * gravity_y, gravity_y, speed);
    let low_t = times.low.filter(|time| *time <= max_time)?;
    let high_t = times.high.filter(|time| *time <= max_time).unwrap_or(low_t);
    Some(PhysicsLaunchSolution2D {
        low: (d - accel * (0.5 * low_t * low_t)) / low_t,
        high: (d - accel * (0.5 * high_t * high_t)) / high_t,
    })
}

fn solve_launch_velocity_3d_no_drift(
    origin: Vector3,
    target: Vector3,
    speed: f32,
    max_time: f32,
    gravity_y: f32,
) -> Option<PhysicsLaunchSolution3D> {
    let d = target - origin;
    let accel = Vector3::new(0.0, gravity_y, 0.0);
    let times = solve_no_drift_times(d.length_squared(), d.y * gravity_y, gravity_y, speed);
    let low_t = times.low.filter(|time| *time <= max_time)?;
    let high_t = times.high.filter(|time| *time <= max_time).unwrap_or(low_t);
    Some(PhysicsLaunchSolution3D {
        low: (d - accel * (0.5 * low_t * low_t)) / low_t,
        high: (d - accel * (0.5 * high_t * high_t)) / high_t,
    })
}

fn solve_no_drift_times(
    distance_sq: f32,
    displacement_dot_accel: f32,
    gravity_y: f32,
    speed: f32,
) -> LaunchTimeRange {
    let a = 0.25 * gravity_y * gravity_y;
    let b = displacement_dot_accel - speed * speed;
    let c = distance_sq;
    if a <= TRAJECTORY_EPS {
        if b >= -TRAJECTORY_EPS {
            return LaunchTimeRange::default();
        }
        let time_sq = -c / b;
        if time_sq > 0.0 && time_sq.is_finite() {
            let time = time_sq.sqrt();
            return LaunchTimeRange {
                low: Some(time),
                high: Some(time),
            };
        }
        return LaunchTimeRange::default();
    }

    let disc = b.mul_add(b, -4.0 * a * c);
    if disc < -TRAJECTORY_ROOT_EPS {
        return LaunchTimeRange::default();
    }
    let disc_sqrt = disc.max(0.0).sqrt();
    let inv = 0.5 / a;
    let mut out = LaunchTimeRange::default();
    for time_sq in [(-b - disc_sqrt) * inv, (-b + disc_sqrt) * inv] {
        if time_sq > 0.0 && time_sq.is_finite() {
            out.push(time_sq.sqrt());
        }
    }
    out
}

#[derive(Clone, Copy, Default)]
struct LaunchTimeRange {
    low: Option<f32>,
    high: Option<f32>,
}

fn find_launch_time_range<F>(max_time: f32, f: &mut F) -> LaunchTimeRange
where
    F: FnMut(f32) -> Option<f32>,
{
    let mut roots = LaunchTimeRange::default();
    let dt = max_time / TRAJECTORY_SCAN_STEPS as f32;
    let mut prev_time = dt.max(TRAJECTORY_EPS);
    let mut prev = match f(prev_time) {
        Some(v) if v.is_finite() => v,
        _ => return roots,
    };

    if prev.abs() <= TRAJECTORY_ROOT_EPS {
        roots.push(prev_time);
    }

    for step in 2..=TRAJECTORY_SCAN_STEPS {
        let time = dt * step as f32;
        let Some(value) = f(time).filter(|value| value.is_finite()) else {
            prev_time = time;
            continue;
        };

        if value.abs() <= TRAJECTORY_ROOT_EPS {
            roots.push(time);
        } else if (prev < 0.0 && value > 0.0) || (prev > 0.0 && value < 0.0) {
            roots.push(refine_launch_time(prev_time, time, f));
        }

        prev_time = time;
        prev = value;
    }

    roots
}

impl LaunchTimeRange {
    fn push(&mut self, time: f32) {
        if self
            .high
            .or(self.low)
            .is_some_and(|prev| (time - prev).abs() <= TRAJECTORY_ROOT_EPS)
        {
            return;
        }
        if self.low.is_none() {
            self.low = Some(time);
        }
        self.high = Some(time);
    }
}

fn refine_launch_time<F>(mut lo: f32, mut hi: f32, f: &mut F) -> f32
where
    F: FnMut(f32) -> Option<f32>,
{
    let Some(mut lo_value) = f(lo).filter(|value| value.is_finite()) else {
        return lo;
    };

    for _ in 0..TRAJECTORY_REFINE_STEPS {
        let mid = (lo + hi) * 0.5;
        let Some(mid_value) = f(mid).filter(|value| value.is_finite()) else {
            break;
        };
        if mid_value.abs() <= TRAJECTORY_ROOT_EPS {
            return mid;
        }
        if (lo_value < 0.0 && mid_value > 0.0) || (lo_value > 0.0 && mid_value < 0.0) {
            hi = mid;
        } else {
            lo = mid;
            lo_value = mid_value;
        }
    }

    (lo + hi) * 0.5
}

/// Applies a force vector to a rigidbody.
///
/// Behavior:
/// - `force` can be `Vector2` (2D body) or `Vector3` (3D body)
/// - force is integrated using fixed-step dt (`impulse = force * dt`)
#[macro_export]
macro_rules! apply_force {
    ($ctx:expr, $body_id:expr, $force:expr) => {
        $ctx.Physics().apply_force($body_id, $force)
    };
}

#[macro_export]
macro_rules! physics_get_gravity {
    ($ctx:expr) => {
        $ctx.Physics().get_gravity()
    };
}

#[macro_export]
macro_rules! physics_set_gravity {
    ($ctx:expr, $gravity:expr) => {
        $ctx.Physics().set_gravity($gravity)
    };
}

#[macro_export]
macro_rules! physics_get_coefficient {
    ($ctx:expr) => {
        $ctx.Physics().get_coefficient()
    };
}

#[macro_export]
macro_rules! physics_set_coefficient {
    ($ctx:expr, $coefficient:expr) => {
        $ctx.Physics().set_coefficient($coefficient)
    };
}

#[macro_export]
macro_rules! physics_solve_velocity_to_target_2d {
    ($ctx:expr, $origin:expr, $target:expr, $time:expr) => {
        $ctx.Physics().solve_velocity_to_target_2d(
            $origin,
            $target,
            $time,
            $crate::perro_structs::Vector2::ZERO,
        )
    };
    ($ctx:expr, $origin:expr, $target:expr, $time:expr, $drift:expr) => {
        $ctx.Physics()
            .solve_velocity_to_target_2d($origin, $target, $time, $drift)
    };
}

#[macro_export]
macro_rules! physics_solve_velocity_to_target_3d {
    ($ctx:expr, $origin:expr, $target:expr, $time:expr) => {
        $ctx.Physics().solve_velocity_to_target_3d(
            $origin,
            $target,
            $time,
            $crate::perro_structs::Vector3::ZERO,
        )
    };
    ($ctx:expr, $origin:expr, $target:expr, $time:expr, $drift:expr) => {
        $ctx.Physics()
            .solve_velocity_to_target_3d($origin, $target, $time, $drift)
    };
}

#[macro_export]
macro_rules! physics_solve_launch_velocity_2d {
    ($ctx:expr, $origin:expr, $target:expr, $speed:expr, $max_time:expr) => {
        $ctx.Physics().solve_launch_velocity_2d(
            $origin,
            $target,
            $speed,
            $max_time,
            $crate::perro_structs::Vector2::ZERO,
        )
    };
    ($ctx:expr, $origin:expr, $target:expr, $speed:expr, $max_time:expr, $drift:expr) => {
        $ctx.Physics()
            .solve_launch_velocity_2d($origin, $target, $speed, $max_time, $drift)
    };
}

#[macro_export]
macro_rules! physics_solve_launch_velocity_3d {
    ($ctx:expr, $origin:expr, $target:expr, $speed:expr, $max_time:expr) => {
        $ctx.Physics().solve_launch_velocity_3d(
            $origin,
            $target,
            $speed,
            $max_time,
            $crate::perro_structs::Vector3::ZERO,
        )
    };
    ($ctx:expr, $origin:expr, $target:expr, $speed:expr, $max_time:expr, $drift:expr) => {
        $ctx.Physics()
            .solve_launch_velocity_3d($origin, $target, $speed, $max_time, $drift)
    };
}

#[macro_export]
macro_rules! physics_predict_body_2d {
    ($ctx:expr, $body_id:expr, $time:expr) => {
        $ctx.Physics()
            .predict_body_2d($body_id, $time, $crate::perro_structs::Vector2::ZERO)
    };
    ($ctx:expr, $body_id:expr, $time:expr, $drift:expr) => {
        $ctx.Physics().predict_body_2d($body_id, $time, $drift)
    };
}

#[macro_export]
macro_rules! physics_predict_body_3d {
    ($ctx:expr, $body_id:expr, $time:expr) => {
        $ctx.Physics()
            .predict_body_3d($body_id, $time, $crate::perro_structs::Vector3::ZERO)
    };
    ($ctx:expr, $body_id:expr, $time:expr, $drift:expr) => {
        $ctx.Physics().predict_body_3d($body_id, $time, $drift)
    };
}

/// Applies an impulse vector to a rigidbody.
///
/// Behavior:
/// - `impulse` can be `Vector2` (2D body) or `Vector3` (3D body)
/// - call once for one-shot momentum changes
#[macro_export]
macro_rules! apply_impulse {
    ($ctx:expr, $body_id:expr, $impulse:expr) => {
        $ctx.Physics().apply_impulse($body_id, $impulse)
    };
}

#[macro_export]
macro_rules! physics_raycast_3d {
    ($ctx:expr, $origin:expr, $direction:expr, $max_distance:expr) => {
        $ctx.Physics()
            .raycast_3d($origin, $direction, $max_distance)
    };
    ($ctx:expr, $origin:expr, $direction:expr, $max_distance:expr, $filter:expr) => {
        $ctx.Physics()
            .raycast_3d_filtered($origin, $direction, $max_distance, $filter)
    };
}

#[macro_export]
macro_rules! physics_raycast_3d_with_areas {
    ($ctx:expr, $origin:expr, $direction:expr, $max_distance:expr) => {
        $ctx.Physics()
            .raycast_3d_with_areas($origin, $direction, $max_distance)
    };
}

#[macro_export]
macro_rules! physics_raycast_3d_without_areas {
    ($ctx:expr, $origin:expr, $direction:expr, $max_distance:expr) => {
        $ctx.Physics()
            .raycast_3d_without_areas($origin, $direction, $max_distance)
    };
}

#[macro_export]
macro_rules! physics_raycast_2d {
    ($ctx:expr, $origin:expr, $direction:expr, $max_distance:expr) => {
        $ctx.Physics()
            .raycast_2d($origin, $direction, $max_distance)
    };
    ($ctx:expr, $origin:expr, $direction:expr, $max_distance:expr, $filter:expr) => {
        $ctx.Physics()
            .raycast_2d_filtered($origin, $direction, $max_distance, $filter)
    };
}

#[macro_export]
macro_rules! physics_shape_cast_2d {
    ($ctx:expr, $shape:expr, $origin:expr, $direction:expr, $max_distance:expr, $filter:expr) => {
        $ctx.Physics()
            .shape_cast_2d($shape, $origin, $direction, $max_distance, $filter)
    };
}

#[macro_export]
macro_rules! physics_shape_cast_3d {
    ($ctx:expr, $shape:expr, $origin:expr, $direction:expr, $max_distance:expr, $filter:expr) => {
        $ctx.Physics()
            .shape_cast_3d($shape, $origin, $direction, $max_distance, $filter)
    };
}

#[macro_export]
macro_rules! physics_move_body_2d {
    ($ctx:expr, $body_id:expr, $target:expr) => {
        $ctx.Physics().move_body_2d(
            $body_id,
            $target,
            0.001,
            $crate::sub_apis::PhysicsQueryFilter::default(),
        )
    };
    ($ctx:expr, $body_id:expr, $target:expr, $margin:expr, $filter:expr) => {
        $ctx.Physics()
            .move_body_2d($body_id, $target, $margin, $filter)
    };
}

#[macro_export]
macro_rules! physics_move_body_3d {
    ($ctx:expr, $body_id:expr, $target:expr) => {
        $ctx.Physics().move_body_3d(
            $body_id,
            $target,
            0.001,
            $crate::sub_apis::PhysicsQueryFilter::default(),
        )
    };
    ($ctx:expr, $body_id:expr, $target:expr, $margin:expr, $filter:expr) => {
        $ctx.Physics()
            .move_body_3d($body_id, $target, $margin, $filter)
    };
}

#[macro_export]
macro_rules! physics_contacts_2d {
    ($ctx:expr, $body_id:expr) => {
        $ctx.Physics().contacts_2d($body_id)
    };
}

#[macro_export]
macro_rules! physics_contacts_3d {
    ($ctx:expr, $body_id:expr) => {
        $ctx.Physics().contacts_3d($body_id)
    };
}

#[macro_export]
macro_rules! physics_pause {
    ($ctx:expr, $paused:expr) => {
        $ctx.Physics().pause($paused)
    };
}

#[macro_export]
macro_rules! physics_is_paused {
    ($ctx:expr) => {
        $ctx.Physics().is_paused()
    };
}
