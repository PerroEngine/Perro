use perro_ids::NodeID;
use perro_structs::{Vector2, Vector3};

pub trait PhysicsAPI {
    fn apply_force_2d(&mut self, body_id: NodeID, direction: Vector2, amount: f32) -> bool;
    fn apply_force_3d(&mut self, body_id: NodeID, direction: Vector3, amount: f32) -> bool;
}

pub trait IntoImpulseDirection {
    fn apply_force<R: PhysicsAPI + ?Sized>(
        self,
        physics: &mut PhysicsModule<'_, R>,
        body_id: NodeID,
        amount: f32,
    ) -> bool;
}

impl IntoImpulseDirection for Vector2 {
    fn apply_force<R: PhysicsAPI + ?Sized>(
        self,
        physics: &mut PhysicsModule<'_, R>,
        body_id: NodeID,
        amount: f32,
    ) -> bool {
        physics.apply_force_2d(body_id, self, amount)
    }
}

impl IntoImpulseDirection for Vector3 {
    fn apply_force<R: PhysicsAPI + ?Sized>(
        self,
        physics: &mut PhysicsModule<'_, R>,
        body_id: NodeID,
        amount: f32,
    ) -> bool {
        physics.apply_force_3d(body_id, self, amount)
    }
}

pub struct PhysicsModule<'rt, R: PhysicsAPI + ?Sized> {
    rt: &'rt mut R,
}

impl<'rt, R: PhysicsAPI + ?Sized> PhysicsModule<'rt, R> {
    pub fn new(rt: &'rt mut R) -> Self {
        Self { rt }
    }

    pub fn apply_force_2d(&mut self, body_id: NodeID, direction: Vector2, amount: f32) -> bool {
        self.rt.apply_force_2d(body_id, direction, amount)
    }

    pub fn apply_force_3d(&mut self, body_id: NodeID, direction: Vector3, amount: f32) -> bool {
        self.rt.apply_force_3d(body_id, direction, amount)
    }

    pub fn apply_force<D>(&mut self, body_id: NodeID, direction: D, amount: f32) -> bool
    where
        D: IntoImpulseDirection,
    {
        direction.apply_force(self, body_id, amount)
    }
}

/// Applies a directional impulse to a rigidbody.
///
/// Behavior:
/// - `direction` can be `Vector2` (2D body) or `Vector3` (3D body)
/// - `amount` is scalar magnitude; final impulse is `normalize(direction) * amount`
/// - call once for one-shot impulse, or each update/fixed-update for sustained acceleration
#[macro_export]
macro_rules! apply_force {
    ($ctx:expr, $body_id:expr, $direction:expr, $amount:expr) => {
        $ctx.Physics().apply_force($body_id, $direction, $amount)
    };
}
