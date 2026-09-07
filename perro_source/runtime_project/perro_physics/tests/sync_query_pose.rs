//! Regression fixture compatible with the audit baseline, independent of
//! Runtime's per-world epoch optimization.
use perro_ids::NodeID;
use perro_nodes::{Shape2D, Shape3D};
use perro_physics::{
    BodyDesc2D, BodyDesc3D, BodyKind, PhysicsAssetContext, PhysicsProviderMode, PhysicsSystem,
    ShapeDesc2D, ShapeDesc3D, ShapeKind2D, ShapeKind3D,
};
use perro_runtime_api::sub_apis::PhysicsQueryFilter;
use perro_structs::{BitMask, Transform2D, Transform3D, Vector2, Vector3};

#[test]
fn sync_pose_change_is_query_visible_before_step_2d() {
    for kind in [BodyKind::Static, BodyKind::Rigid, BodyKind::Character] {
        let mut physics = PhysicsSystem::new();
        let mut body = BodyDesc2D {
            id: NodeID::new(1),
            kind,
            enabled: true,
            global: Transform2D::IDENTITY,
            rigid: None,
            sync_signature: 1,
            shape_signature: 1,
            shapes: vec![ShapeDesc2D {
                local: Transform2D::IDENTITY,
                shape: ShapeKind2D::Primitive(Shape2D::Circle { radius: 0.5 }),
                sensor: false,
                collision_layers: BitMask::ALL,
                collision_mask: BitMask::ALL,
                friction: 0.7,
                restitution: 0.0,
                density: 1.0,
            }],
        };
        physics.sync_world_2d(std::slice::from_ref(&body), |_, _| {});
        assert_eq!(
            physics
                .raycast_2d(
                    Vector2::new(0.0, -5.0),
                    Vector2::new(0.0, 1.0),
                    10.0,
                    &PhysicsQueryFilter::default()
                )
                .expect("initial 2D body pose is query visible")
                .node,
            body.id
        );
        body.global.position.x = 20.0;
        body.sync_signature += 1;
        physics.sync_world_2d(std::slice::from_ref(&body), |_, _| {});
        assert_eq!(
            physics
                .raycast_2d(
                    Vector2::new(20.0, -5.0),
                    Vector2::new(0.0, 1.0),
                    10.0,
                    &PhysicsQueryFilter::default()
                )
                .expect("new pose before step")
                .node,
            body.id
        );
        assert!(
            physics
                .raycast_2d(
                    Vector2::new(0.0, -5.0),
                    Vector2::new(0.0, 1.0),
                    10.0,
                    &PhysicsQueryFilter::default()
                )
                .is_none()
        );
    }
}

#[test]
fn sync_pose_change_is_query_visible_before_step_3d() {
    for kind in [BodyKind::Static, BodyKind::Rigid, BodyKind::Character] {
        let mut physics = PhysicsSystem::new();
        let assets = PhysicsAssetContext {
            provider_mode: PhysicsProviderMode::Dynamic,
            static_mesh_lookup: None,
            static_collision_trimesh_lookup: None,
        };
        let mut body = BodyDesc3D {
            id: NodeID::new(1),
            kind,
            enabled: true,
            global: Transform3D::IDENTITY,
            rigid: None,
            sync_signature: 1,
            shape_signature: 1,
            shapes: vec![ShapeDesc3D {
                local: Transform3D::IDENTITY,
                shape: ShapeKind3D::Primitive(Shape3D::Sphere { radius: 0.5 }),
                sensor: false,
                collision_layers: BitMask::ALL,
                collision_mask: BitMask::ALL,
                friction: 0.7,
                restitution: 0.0,
                density: 1.0,
            }],
        };
        physics.sync_world_3d(std::slice::from_ref(&body), assets, |_, _| {});
        assert_eq!(
            physics
                .raycast_3d(
                    Vector3::new(0.0, 0.0, -5.0),
                    Vector3::new(0.0, 0.0, 1.0),
                    10.0,
                    false
                )
                .expect("initial 3D body pose is query visible")
                .node,
            body.id
        );
        body.global.position.x = 20.0;
        body.sync_signature += 1;
        physics.sync_world_3d(std::slice::from_ref(&body), assets, |_, _| {});
        assert_eq!(
            physics
                .raycast_3d(
                    Vector3::new(20.0, 0.0, -5.0),
                    Vector3::new(0.0, 0.0, 1.0),
                    10.0,
                    false
                )
                .expect("new pose before step")
                .node,
            body.id
        );
        assert!(
            physics
                .raycast_3d(
                    Vector3::new(0.0, 0.0, -5.0),
                    Vector3::new(0.0, 0.0, 1.0),
                    10.0,
                    false
                )
                .is_none()
        );
    }
}
