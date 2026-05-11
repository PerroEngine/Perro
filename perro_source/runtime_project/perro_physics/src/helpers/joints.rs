use super::*;

pub fn joint_signature_2d(
    body_a: NodeID,
    body_b: NodeID,
    anchor_a: Vector2,
    anchor_b: Vector2,
    enabled: bool,
    collide_connected: bool,
    kind: JointKind2D,
) -> u64 {
    let mut hash = body_a.as_u64().wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ body_b.as_u64();
    hash = hash_u32(hash, anchor_a.x.to_bits());
    hash = hash_u32(hash, anchor_a.y.to_bits());
    hash = hash_u32(hash, anchor_b.x.to_bits());
    hash = hash_u32(hash, anchor_b.y.to_bits());
    hash = hash_u32(hash, enabled as u32);
    hash = hash_u32(hash, collide_connected as u32);
    match kind {
        JointKind2D::Pin => hash_u32(hash, 1),
        JointKind2D::Distance { min, max } => {
            let hash = hash_u32(hash, 2);
            let hash = hash_u32(hash, min.to_bits());
            hash_u32(hash, max.to_bits())
        }
        JointKind2D::Fixed => hash_u32(hash, 3),
    }
}

pub fn joint_signature_3d(
    body_a: NodeID,
    body_b: NodeID,
    anchor_a: Vector3,
    anchor_b: Vector3,
    enabled: bool,
    collide_connected: bool,
    kind: JointKind3D,
) -> u64 {
    let mut hash = body_a.as_u64().wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ body_b.as_u64();
    hash = hash_u32(hash, anchor_a.x.to_bits());
    hash = hash_u32(hash, anchor_a.y.to_bits());
    hash = hash_u32(hash, anchor_a.z.to_bits());
    hash = hash_u32(hash, anchor_b.x.to_bits());
    hash = hash_u32(hash, anchor_b.y.to_bits());
    hash = hash_u32(hash, anchor_b.z.to_bits());
    hash = hash_u32(hash, enabled as u32);
    hash = hash_u32(hash, collide_connected as u32);
    match kind {
        JointKind3D::Ball => hash_u32(hash, 1),
        JointKind3D::Hinge { axis } => {
            let hash = hash_u32(hash, 2);
            let hash = hash_u32(hash, axis.x.to_bits());
            let hash = hash_u32(hash, axis.y.to_bits());
            hash_u32(hash, axis.z.to_bits())
        }
        JointKind3D::Fixed => hash_u32(hash, 3),
    }
}

pub fn build_joint_2d(desc: &JointDesc2D) -> r2::GenericJoint {
    let anchor_a = na2::Point2::new(desc.anchor_a.x, desc.anchor_a.y);
    let anchor_b = na2::Point2::new(desc.anchor_b.x, desc.anchor_b.y);
    match desc.kind {
        JointKind2D::Pin => r2::RevoluteJointBuilder::new()
            .contacts_enabled(desc.collide_connected)
            .local_anchor1(anchor_a)
            .local_anchor2(anchor_b)
            .into(),
        JointKind2D::Distance { min, max } => {
            let min = min.max(0.0);
            let max = max.max(min).max(0.0001);
            r2::GenericJointBuilder::new(r2::JointAxesMask::empty())
                .coupled_axes(r2::JointAxesMask::LIN_AXES)
                .limits(r2::JointAxis::LinX, [min, max])
                .contacts_enabled(desc.collide_connected)
                .local_anchor1(anchor_a)
                .local_anchor2(anchor_b)
                .into()
        }
        JointKind2D::Fixed => r2::FixedJointBuilder::new()
            .contacts_enabled(desc.collide_connected)
            .local_anchor1(anchor_a)
            .local_anchor2(anchor_b)
            .into(),
    }
}

pub fn build_joint_3d(desc: &JointDesc3D) -> r3::GenericJoint {
    let anchor_a = na3::Point3::new(desc.anchor_a.x, desc.anchor_a.y, desc.anchor_a.z);
    let anchor_b = na3::Point3::new(desc.anchor_b.x, desc.anchor_b.y, desc.anchor_b.z);
    match desc.kind {
        JointKind3D::Ball => r3::SphericalJointBuilder::new()
            .contacts_enabled(desc.collide_connected)
            .local_anchor1(anchor_a)
            .local_anchor2(anchor_b)
            .into(),
        JointKind3D::Hinge { axis } => {
            let axis = if axis.x * axis.x + axis.y * axis.y + axis.z * axis.z <= 0.000_001 {
                na3::Vector3::y_axis()
            } else {
                na3::Unit::new_normalize(na3::Vector3::new(axis.x, axis.y, axis.z))
            };
            r3::RevoluteJointBuilder::new(axis)
                .contacts_enabled(desc.collide_connected)
                .local_anchor1(anchor_a)
                .local_anchor2(anchor_b)
                .into()
        }
        JointKind3D::Fixed => r3::FixedJointBuilder::new()
            .contacts_enabled(desc.collide_connected)
            .local_anchor1(anchor_a)
            .local_anchor2(anchor_b)
            .into(),
    }
}

pub fn remove_joint_2d(world: &mut PhysicsWorld2D, id: NodeID) {
    if let Some(state) = world.joint_map.remove(&id) {
        let _ = world.impulse_joints.remove(state.handle, true);
    }
}

pub fn remove_joint_3d(world: &mut PhysicsWorld3D, id: NodeID) {
    if let Some(state) = world.joint_map.remove(&id) {
        let _ = world.impulse_joints.remove(state.handle, true);
    }
}
