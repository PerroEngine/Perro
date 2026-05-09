use perro_ids::NodeID;
use perro_structs::{Vector2, Vector3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsRayHit3D {
    pub node: NodeID,
    pub point: Vector3,
    pub normal: Vector3,
    pub distance: f32,
}

pub trait PhysicsAPI {
    fn apply_force_2d(&mut self, body_id: NodeID, force: Vector2) -> bool;
    fn apply_force_3d(&mut self, body_id: NodeID, force: Vector3) -> bool;
    fn apply_impulse_2d(&mut self, body_id: NodeID, impulse: Vector2) -> bool;
    fn apply_impulse_3d(&mut self, body_id: NodeID, impulse: Vector3) -> bool;
    fn raycast_3d(
        &mut self,
        origin: Vector3,
        direction: Vector3,
        max_distance: f32,
        include_areas: bool,
    ) -> Option<PhysicsRayHit3D>;
    fn physics_pause(&mut self, paused: bool);
    fn physics_is_paused(&mut self) -> bool;
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

    pub fn apply_force_3d(&mut self, body_id: NodeID, force: Vector3) -> bool {
        self.rt.apply_force_3d(body_id, force)
    }

    pub fn apply_impulse_2d(&mut self, body_id: NodeID, impulse: Vector2) -> bool {
        self.rt.apply_impulse_2d(body_id, impulse)
    }

    pub fn apply_impulse_3d(&mut self, body_id: NodeID, impulse: Vector3) -> bool {
        self.rt.apply_impulse_3d(body_id, impulse)
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

    pub fn pause(&mut self, paused: bool) {
        self.rt.physics_pause(paused);
    }

    pub fn is_paused(&mut self) -> bool {
        self.rt.physics_is_paused()
    }
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
