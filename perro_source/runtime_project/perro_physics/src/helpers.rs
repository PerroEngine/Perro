use ahash::{AHashMap, AHashSet};
use perro_asset_formats::pmesh::{
    FLAG_INDEX_U16 as PMESH_FLAG_INDEX_U16, FLAG_PAYLOAD_RAW as PMESH_FLAG_PAYLOAD_RAW,
    VERSION as PMESH_VERSION,
};
use perro_ids::{NodeID, parse_hashed_source_uri, string_to_u64};
use perro_io::{decompress_zlib, load_asset};
use perro_nodes::{
    CollisionShape2D, CollisionShape3D, Shape2D, Shape3D, TileMap2D, Triangle2DKind,
};
use perro_render_bridge::{
    TileSet2D as ParsedTileset2D, TileSetCollisionShape2D as ParsedTileCollisionShape2D,
    TileSetShape2D,
};
use perro_runtime_context::sub_apis::{PhysicsQueryFilter, PhysicsRayHit2D, PhysicsRayHit3D};
use perro_structs::{Transform2D, Transform3D, Vector2, Vector3};

use crate::{
    BodyDesc2D, BodyDesc3D, BodyKind, JointDesc2D, JointDesc3D, JointKind2D, JointKind3D,
    PhysicsWorld2D, PhysicsWorld3D, ShapeDesc2D, ShapeDesc3D, ShapeKind2D, ShapeKind3D,
    TriMeshData, na2, na3, r2, r3,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PhysicsProviderMode {
    Dynamic,
    Static,
}

pub type StaticBytesLookup = fn(u64) -> &'static [u8];

#[derive(Clone, Copy, Debug)]
pub struct PhysicsAssetContext {
    pub provider_mode: PhysicsProviderMode,
    pub static_mesh_lookup: Option<StaticBytesLookup>,
    pub static_collision_trimesh_lookup: Option<StaticBytesLookup>,
}

pub fn body_signature_seed(kind: BodyKind) -> u64 {
    match kind {
        BodyKind::Static => 0xA91B_D58C_24F1_7E31,
        BodyKind::Area => 0xCC42_83B7_9E20_11DD,
        BodyKind::Rigid => 0x6D1E_93A4_F02C_B871,
    }
}

pub fn hash_u64(mut state: u64, value: u64) -> u64 {
    state ^= value.wrapping_mul(0x9E37_79B1_85EB_CA87);
    state.rotate_left(17)
}

pub fn hash_f32(state: u64, bits: u32) -> u64 {
    hash_u64(state, bits as u64)
}

pub fn hash_u32(state: u64, value: u32) -> u64 {
    hash_u64(state, value as u64)
}

pub fn hash_transform_2d(mut state: u64, transform: Transform2D) -> u64 {
    state = hash_f32(state, transform.position.x.to_bits());
    state = hash_f32(state, transform.position.y.to_bits());
    state = hash_f32(state, transform.rotation.to_bits());
    state = hash_f32(state, transform.scale.x.to_bits());
    hash_f32(state, transform.scale.y.to_bits())
}

pub fn hash_transform_3d(mut state: u64, transform: Transform3D) -> u64 {
    state = hash_f32(state, transform.position.x.to_bits());
    state = hash_f32(state, transform.position.y.to_bits());
    state = hash_f32(state, transform.position.z.to_bits());
    state = hash_f32(state, transform.rotation.x.to_bits());
    state = hash_f32(state, transform.rotation.y.to_bits());
    state = hash_f32(state, transform.rotation.z.to_bits());
    state = hash_f32(state, transform.rotation.w.to_bits());
    state = hash_f32(state, transform.scale.x.to_bits());
    state = hash_f32(state, transform.scale.y.to_bits());
    hash_f32(state, transform.scale.z.to_bits())
}

pub fn hash_shape_2d(state: u64, shape: Shape2D) -> u64 {
    match shape {
        Shape2D::Quad { width, height } => {
            let state = hash_u64(state, 1);
            let state = hash_f32(state, width.to_bits());
            hash_f32(state, height.to_bits())
        }
        Shape2D::Circle { radius } => {
            let state = hash_u64(state, 2);
            hash_f32(state, radius.to_bits())
        }
        Shape2D::Triangle {
            kind,
            width,
            height,
        } => {
            let state = hash_u64(state, 3);
            let kind_tag = match kind {
                Triangle2DKind::Equilateral => 1,
                Triangle2DKind::Right => 2,
                Triangle2DKind::Isosceles => 3,
            };
            let state = hash_u64(state, kind_tag);
            let state = hash_f32(state, width.to_bits());
            hash_f32(state, height.to_bits())
        }
    }
}

pub fn hash_shape_3d(state: u64, shape: &Shape3D) -> u64 {
    match shape {
        Shape3D::Cube { size } => {
            let state = hash_u64(state, 1);
            let state = hash_f32(state, size.x.to_bits());
            let state = hash_f32(state, size.y.to_bits());
            hash_f32(state, size.z.to_bits())
        }
        Shape3D::Sphere { radius } => {
            let state = hash_u64(state, 2);
            hash_f32(state, radius.to_bits())
        }
        Shape3D::Capsule {
            radius,
            half_height,
        } => {
            let state = hash_u64(state, 3);
            let state = hash_f32(state, radius.to_bits());
            hash_f32(state, half_height.to_bits())
        }
        Shape3D::Cylinder {
            radius,
            half_height,
        } => {
            let state = hash_u64(state, 4);
            let state = hash_f32(state, radius.to_bits());
            hash_f32(state, half_height.to_bits())
        }
        Shape3D::Cone {
            radius,
            half_height,
        } => {
            let state = hash_u64(state, 5);
            let state = hash_f32(state, radius.to_bits());
            hash_f32(state, half_height.to_bits())
        }
        Shape3D::TriPrism { size } => {
            let state = hash_u64(state, 6);
            let state = hash_f32(state, size.x.to_bits());
            let state = hash_f32(state, size.y.to_bits());
            hash_f32(state, size.z.to_bits())
        }
        Shape3D::TriangularPyramid { size } => {
            let state = hash_u64(state, 7);
            let state = hash_f32(state, size.x.to_bits());
            let state = hash_f32(state, size.y.to_bits());
            hash_f32(state, size.z.to_bits())
        }
        Shape3D::SquarePyramid { size } => {
            let state = hash_u64(state, 8);
            let state = hash_f32(state, size.x.to_bits());
            let state = hash_f32(state, size.y.to_bits());
            hash_f32(state, size.z.to_bits())
        }
        Shape3D::TriMesh { source } => {
            let mut state = hash_u64(state, 9);
            for b in source.as_bytes() {
                state = hash_u64(state, *b as u64);
            }
            state
        }
    }
}

pub fn hash_collision_shape_2d(state: u64, shape: &CollisionShape2D, kind: BodyKind) -> u64 {
    let mut state = hash_u64(state, (kind == BodyKind::Area) as u64);
    state = hash_transform_2d(state, shape.base.transform);
    hash_shape_2d(state, shape.shape)
}

pub fn hash_tilemap_2d(mut state: u64, tilemap: &TileMap2D) -> u64 {
    state = hash_u32(state, tilemap.width);
    state = hash_u32(state, tilemap.height);
    state = hash_u64(state, tilemap.empty_tile as u64);
    for tile in &tilemap.tiles {
        state = hash_u64(state, *tile as u64);
    }
    for b in tilemap.tileset.as_bytes() {
        state = hash_u64(state, *b as u64);
    }
    state
}

pub fn hash_tile_collision_shape_2d(mut state: u64, shape: ParsedTileCollisionShape2D) -> u64 {
    match shape {
        ParsedTileCollisionShape2D::Auto => hash_u32(state, 1),
        ParsedTileCollisionShape2D::Shape { shape, offset } => {
            state = hash_u32(state, 2);
            state = hash_shape_2d(state, tile_set_shape_to_shape_2d(shape));
            state = hash_f32(state, offset[0].to_bits());
            hash_f32(state, offset[1].to_bits())
        }
        ParsedTileCollisionShape2D::Polygon { points, offset } => {
            state = hash_u32(state, 3);
            state = hash_f32(state, offset[0].to_bits());
            state = hash_f32(state, offset[1].to_bits());
            for point in points.iter() {
                state = hash_f32(state, point.x.to_bits());
                state = hash_f32(state, point.y.to_bits());
            }
            state
        }
    }
}

pub fn tile_set_shape_to_shape_2d(shape: TileSetShape2D) -> Shape2D {
    match shape {
        TileSetShape2D::Rect { width, height } => Shape2D::Quad { width, height },
        TileSetShape2D::Circle { radius } => Shape2D::Circle { radius },
        TileSetShape2D::Triangle { width, height } => Shape2D::Triangle {
            kind: Triangle2DKind::Isosceles,
            width,
            height,
        },
    }
}

pub fn tilemap_shape_descs_2d(
    tilemap: &TileMap2D,
    layer: u32,
    mask: u32,
    friction: f32,
    restitution: f32,
    tileset: Option<&ParsedTileset2D>,
) -> Vec<ShapeDesc2D> {
    let Some(tileset) = tileset else {
        return Vec::new();
    };
    let width = tilemap.width as usize;
    let height = tilemap.height as usize;
    if width == 0 || height == 0 {
        return Vec::new();
    }
    let tw = tileset.tile_size[0];
    let th = tileset.tile_size[1];

    let mut solid = vec![false; width.saturating_mul(height)];
    let mut explicit = Vec::new();
    for (idx, tile_id) in tilemap.tiles.iter().take(solid.len()).copied().enumerate() {
        if tile_id == tilemap.empty_tile {
            continue;
        }
        let Some(tile) = tileset.tile(tile_id) else {
            continue;
        };
        if !tile.collision {
            continue;
        }
        match tile.collision_shape.clone() {
            ParsedTileCollisionShape2D::Auto => solid[idx] = true,
            ParsedTileCollisionShape2D::Shape { shape, offset } => {
                explicit.push((
                    idx,
                    ShapeKind2D::Primitive(tile_set_shape_to_shape_2d(shape)),
                    offset,
                ));
            }
            ParsedTileCollisionShape2D::Polygon { points, offset } => {
                explicit.push((idx, ShapeKind2D::Polygon(points.to_vec()), offset));
            }
        }
    }

    let mut out = Vec::new();
    let mut used = vec![false; solid.len()];
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if !solid[idx] || used[idx] {
                continue;
            }
            let mut run_w = 1usize;
            while x + run_w < width && solid[idx + run_w] && !used[idx + run_w] {
                run_w += 1;
            }
            let mut run_h = 1usize;
            'grow: while y + run_h < height {
                for ox in 0..run_w {
                    let n = (y + run_h) * width + x + ox;
                    if !solid[n] || used[n] {
                        break 'grow;
                    }
                }
                run_h += 1;
            }
            for yy in y..(y + run_h) {
                for xx in x..(x + run_w) {
                    used[yy * width + xx] = true;
                }
            }
            let w = run_w as f32 * tw;
            let h = run_h as f32 * th;
            out.push(ShapeDesc2D {
                local: Transform2D::new(
                    Vector2::new(x as f32 * tw + w * 0.5, -(y as f32 * th + h * 0.5)),
                    0.0,
                    Vector2::ONE,
                ),
                shape: ShapeKind2D::Primitive(Shape2D::Quad {
                    width: w,
                    height: h,
                }),
                sensor: false,
                collision_layer: layer,
                collision_mask: mask,
                friction,
                restitution,
            });
        }
    }
    for (idx, shape, offset) in explicit {
        let x = idx % width;
        let y = idx / width;
        out.push(ShapeDesc2D {
            local: Transform2D::new(
                Vector2::new(
                    x as f32 * tw + tw * 0.5 + offset[0],
                    -(y as f32 * th + th * 0.5 + offset[1]),
                ),
                0.0,
                Vector2::ONE,
            ),
            shape,
            sensor: false,
            collision_layer: layer,
            collision_mask: mask,
            friction,
            restitution,
        });
    }
    out
}

pub fn hash_collision_shape_3d(
    state: u64,
    shape: &CollisionShape3D,
    kind: BodyKind,
    inherited_scale: Vector3,
) -> u64 {
    let mut state = hash_u64(state, (kind == BodyKind::Area) as u64);
    let mut transform = shape.base.transform;
    transform.scale = Vector3::new(
        transform.scale.x * inherited_scale.x,
        transform.scale.y * inherited_scale.y,
        transform.scale.z * inherited_scale.z,
    );
    state = hash_transform_3d(state, transform);
    hash_shape_3d(state, &shape.shape)
}

pub fn shape_desc_2d(shape: &CollisionShape2D, friction: f32, restitution: f32) -> ShapeDesc2D {
    ShapeDesc2D {
        local: shape.base.transform,
        shape: ShapeKind2D::Primitive(shape.shape),
        sensor: false,
        collision_layer: 1,
        collision_mask: u32::MAX,
        friction,
        restitution,
    }
}

pub fn shape_desc_3d(shape: &CollisionShape3D, friction: f32, restitution: f32) -> ShapeDesc3D {
    ShapeDesc3D {
        local: shape.base.transform,
        shape: match &shape.shape {
            Shape3D::TriMesh { source } => ShapeKind3D::TriMesh {
                source: source.clone(),
            },
            _ => ShapeKind3D::Primitive(shape.shape.clone()),
        },
        sensor: false,
        collision_layer: 1,
        collision_mask: u32::MAX,
        friction,
        restitution,
    }
}

pub fn approx_eq_f32(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.000_01
}

pub fn clamp_rb_speed_2d(rb: &mut r2::RigidBody, max_speed: f32) {
    if max_speed <= 0.0 {
        return;
    }
    let current = *rb.linvel();
    let speed_sq = current.norm_squared();
    let max_sq = max_speed * max_speed;
    if speed_sq <= max_sq || speed_sq <= 0.0 {
        return;
    }
    let scale = max_speed / speed_sq.sqrt();
    rb.set_linvel(current * scale, true);
}

pub fn clamp_rb_speed_3d(rb: &mut r3::RigidBody, max_speed: f32) {
    if max_speed <= 0.0 {
        return;
    }
    let current = *rb.linvel();
    let speed_sq = current.norm_squared();
    let max_sq = max_speed * max_speed;
    if speed_sq <= max_sq || speed_sq <= 0.0 {
        return;
    }
    let scale = max_speed / speed_sq.sqrt();
    rb.set_linvel(current * scale, true);
}

pub fn build_rigid_body_2d(desc: &BodyDesc2D) -> r2::RigidBody {
    let mut builder = match desc.kind {
        BodyKind::Static => r2::RigidBodyBuilder::fixed(),
        BodyKind::Area => r2::RigidBodyBuilder::fixed(),
        BodyKind::Rigid => r2::RigidBodyBuilder::dynamic(),
    }
    .position(transform_to_iso2(desc.global))
    .enabled(desc.enabled);

    if let Some(rigid) = desc.rigid.as_ref() {
        builder = builder
            .linvel(na2::Vector2::new(
                rigid.linear_velocity.x,
                rigid.linear_velocity.y,
            ))
            .angvel(rigid.angular_velocity)
            .gravity_scale(rigid.gravity_scale)
            .linear_damping(rigid.linear_damping)
            .angular_damping(rigid.angular_damping)
            .ccd_enabled(rigid.continuous_collision_detection)
            .can_sleep(rigid.can_sleep)
            .enabled(rigid.enabled);
        if rigid.lock_rotation {
            builder = builder.lock_rotations();
        }
    }

    builder.build()
}

pub fn build_rigid_body_3d(desc: &BodyDesc3D) -> r3::RigidBody {
    let mut builder = match desc.kind {
        BodyKind::Static => r3::RigidBodyBuilder::fixed(),
        BodyKind::Area => r3::RigidBodyBuilder::fixed(),
        BodyKind::Rigid => r3::RigidBodyBuilder::dynamic(),
    }
    .position(transform_to_iso3(desc.global))
    .enabled(desc.enabled);

    if let Some(rigid) = desc.rigid.as_ref() {
        builder = builder
            .linvel(na3::Vector3::new(
                rigid.linear_velocity.x,
                rigid.linear_velocity.y,
                rigid.linear_velocity.z,
            ))
            .angvel(na3::Vector3::new(
                rigid.angular_velocity.x,
                rigid.angular_velocity.y,
                rigid.angular_velocity.z,
            ))
            .gravity_scale(rigid.gravity_scale)
            .linear_damping(rigid.linear_damping)
            .angular_damping(rigid.angular_damping)
            .additional_mass(rigid.mass.max(0.0))
            .ccd_enabled(rigid.continuous_collision_detection)
            .can_sleep(rigid.can_sleep)
            .enabled(rigid.enabled);
    }

    builder.build()
}

pub fn collider_builder_2d(desc: &ShapeDesc2D) -> Option<r2::Collider> {
    let sx = desc.local.scale.x.abs().max(0.0001);
    let sy = desc.local.scale.y.abs().max(0.0001);
    let shape = match &desc.shape {
        ShapeKind2D::Primitive(Shape2D::Quad { width, height }) => r2::ColliderBuilder::cuboid(
            width.abs().max(0.0001) * sx * 0.5,
            height.abs().max(0.0001) * sy * 0.5,
        ),
        ShapeKind2D::Primitive(Shape2D::Circle { radius }) => {
            let scale = sx.max(sy);
            r2::ColliderBuilder::ball(radius.abs().max(0.0001) * scale)
        }
        ShapeKind2D::Primitive(Shape2D::Triangle {
            kind,
            width,
            height,
        }) => {
            let points = triangle_points_2d(*kind, width * sx, height * sy)?;
            r2::ColliderBuilder::triangle(points[0], points[1], points[2])
        }
        ShapeKind2D::Polygon(points) => {
            let points = points
                .iter()
                .filter(|p| p.x.is_finite() && p.y.is_finite())
                .map(|p| na2::Point2::new(p.x * sx, p.y * sy))
                .collect::<Vec<_>>();
            r2::ColliderBuilder::convex_hull(&points)?
        }
    };

    Some(
        shape
            .position(na2::Isometry2::new(
                na2::Vector2::new(desc.local.position.x, desc.local.position.y),
                desc.local.rotation,
            ))
            .sensor(desc.sensor)
            .collision_groups(interaction_groups_2d(
                desc.collision_layer,
                desc.collision_mask,
            ))
            .friction(desc.friction)
            .restitution(desc.restitution)
            .build(),
    )
}

pub fn shared_shape_2d(shape: Shape2D) -> Option<r2::SharedShape> {
    match shape {
        Shape2D::Quad { width, height } => Some(r2::SharedShape::cuboid(
            width.abs().max(0.0001) * 0.5,
            height.abs().max(0.0001) * 0.5,
        )),
        Shape2D::Circle { radius } => Some(r2::SharedShape::ball(radius.abs().max(0.0001))),
        Shape2D::Triangle {
            kind,
            width,
            height,
        } => {
            let points = triangle_points_2d(kind, width, height)?;
            Some(r2::SharedShape::triangle(points[0], points[1], points[2]))
        }
    }
}

pub fn shared_shape_3d(shape: Shape3D) -> Option<r3::SharedShape> {
    match shape {
        Shape3D::Cube { size } => Some(r3::SharedShape::cuboid(
            size.x.abs().max(0.0001) * 0.5,
            size.y.abs().max(0.0001) * 0.5,
            size.z.abs().max(0.0001) * 0.5,
        )),
        Shape3D::Sphere { radius } => Some(r3::SharedShape::ball(radius.abs().max(0.0001))),
        Shape3D::Capsule {
            radius,
            half_height,
        } => Some(r3::SharedShape::capsule_y(
            half_height.abs().max(0.0001),
            radius.abs().max(0.0001),
        )),
        Shape3D::Cylinder {
            radius,
            half_height,
        } => Some(r3::SharedShape::cylinder(
            half_height.abs().max(0.0001),
            radius.abs().max(0.0001),
        )),
        Shape3D::Cone {
            radius,
            half_height,
        } => Some(r3::SharedShape::cone(
            half_height.abs().max(0.0001),
            radius.abs().max(0.0001),
        )),
        Shape3D::TriPrism { size } => {
            let points = tri_prism_points(size.x, size.y, size.z);
            r3::SharedShape::convex_hull(&points)
        }
        Shape3D::TriangularPyramid { size } => {
            let points = triangular_pyramid_points(size.x, size.y, size.z);
            r3::SharedShape::convex_hull(&points)
        }
        Shape3D::SquarePyramid { size } => {
            let points = square_pyramid_points(size.x, size.y, size.z);
            r3::SharedShape::convex_hull(&points)
        }
        Shape3D::TriMesh { .. } => None,
    }
}

pub fn collider_builder_3d(
    desc: &ShapeDesc3D,
    provider_mode: PhysicsProviderMode,
    static_mesh_lookup: Option<StaticBytesLookup>,
    static_collision_trimesh_lookup: Option<StaticBytesLookup>,
    trimesh_cache: &mut AHashMap<u64, TriMeshData>,
) -> Option<r3::Collider> {
    let sx = desc.local.scale.x.abs().max(0.0001);
    let sy = desc.local.scale.y.abs().max(0.0001);
    let sz = desc.local.scale.z.abs().max(0.0001);
    let mut trimesh_load = TrimeshLoadCtx {
        provider_mode,
        static_mesh_lookup,
        static_collision_trimesh_lookup,
        trimesh_cache,
    };

    let shape = match &desc.shape {
        ShapeKind3D::Primitive(shape) => match shape {
            Shape3D::Cube { size } => r3::ColliderBuilder::cuboid(
                size.x.abs().max(0.0001) * sx * 0.5,
                size.y.abs().max(0.0001) * sy * 0.5,
                size.z.abs().max(0.0001) * sz * 0.5,
            ),
            Shape3D::Sphere { radius } => {
                let scale = sx.max(sy).max(sz);
                r3::ColliderBuilder::ball(radius.abs().max(0.0001) * scale)
            }
            Shape3D::Capsule {
                radius,
                half_height,
            } => {
                let scale = sx.max(sz);
                r3::ColliderBuilder::capsule_y(
                    half_height.abs().max(0.0001) * sy,
                    radius.abs().max(0.0001) * scale,
                )
            }
            Shape3D::Cylinder {
                radius,
                half_height,
            } => {
                let scale = sx.max(sz);
                r3::ColliderBuilder::cylinder(
                    half_height.abs().max(0.0001) * sy,
                    radius.abs().max(0.0001) * scale,
                )
            }
            Shape3D::Cone {
                radius,
                half_height,
            } => {
                let scale = sx.max(sz);
                r3::ColliderBuilder::cone(
                    half_height.abs().max(0.0001) * sy,
                    radius.abs().max(0.0001) * scale,
                )
            }
            Shape3D::TriPrism { size } => {
                let points = tri_prism_points(size.x * sx, size.y * sy, size.z * sz);
                r3::ColliderBuilder::convex_hull(&points)?
            }
            Shape3D::TriangularPyramid { size } => {
                let points = triangular_pyramid_points(size.x * sx, size.y * sy, size.z * sz);
                r3::ColliderBuilder::convex_hull(&points)?
            }
            Shape3D::SquarePyramid { size } => {
                let points = square_pyramid_points(size.x * sx, size.y * sy, size.z * sz);
                r3::ColliderBuilder::convex_hull(&points)?
            }
            Shape3D::TriMesh { source } => {
                let (vertices, triangles) =
                    load_trimesh_from_source(source, [sx, sy, sz], &mut trimesh_load)?;
                r3::ColliderBuilder::trimesh(vertices, triangles).ok()?
            }
        },
        ShapeKind3D::TriMesh { source } => {
            let (vertices, triangles) =
                load_trimesh_from_source(source, [sx, sy, sz], &mut trimesh_load)?;
            r3::ColliderBuilder::trimesh(vertices, triangles).ok()?
        }
    };

    Some(
        shape
            .position(transform_to_iso3(desc.local))
            .sensor(desc.sensor)
            .collision_groups(interaction_groups_3d(
                desc.collision_layer,
                desc.collision_mask,
            ))
            .friction(desc.friction)
            .restitution(desc.restitution)
            .build(),
    )
}

pub fn interaction_groups_2d(layer: u32, mask: u32) -> r2::InteractionGroups {
    r2::InteractionGroups::new(
        r2::Group::from_bits_truncate(layer),
        r2::Group::from_bits_truncate(mask),
    )
}

pub fn interaction_groups_3d(layer: u32, mask: u32) -> r3::InteractionGroups {
    r3::InteractionGroups::new(
        r3::Group::from_bits_truncate(layer),
        r3::Group::from_bits_truncate(mask),
    )
}

pub fn query_filter_2d(filter: &PhysicsQueryFilter) -> r2::QueryFilter<'_> {
    let mut query_filter = r2::QueryFilter::new();
    if !filter.include_areas {
        query_filter = query_filter.exclude_sensors();
    }
    query_filter
}

pub fn query_filter_3d(filter: &PhysicsQueryFilter) -> r3::QueryFilter<'_> {
    let mut query_filter = r3::QueryFilter::new();
    if !filter.include_areas {
        query_filter = query_filter.exclude_sensors();
    }
    query_filter
}

pub struct TrimeshLoadCtx<'a> {
    provider_mode: PhysicsProviderMode,
    static_mesh_lookup: Option<StaticBytesLookup>,
    static_collision_trimesh_lookup: Option<StaticBytesLookup>,
    trimesh_cache: &'a mut AHashMap<u64, TriMeshData>,
}

pub fn load_trimesh_from_source(
    source: &str,
    scale: [f32; 3],
    ctx: &mut TrimeshLoadCtx<'_>,
) -> Option<TriMeshData> {
    let source = source.trim();
    if source.is_empty() {
        return None;
    }
    let [sx, sy, sz] = scale;

    let cache_key = trimesh_cache_key(source, sx, sy, sz, ctx.provider_mode);
    if let Some(cached) = ctx.trimesh_cache.get(&cache_key) {
        return Some(cached.clone());
    }

    if ctx.provider_mode == PhysicsProviderMode::Static
        && let Some(lookup) = ctx.static_collision_trimesh_lookup
    {
        let source_hash = parse_hashed_source_uri(source).unwrap_or_else(|| string_to_u64(source));
        let bytes = lookup(source_hash);
        if !bytes.is_empty()
            && let Some(decoded) = decode_pmesh_trimesh(bytes, sx, sy, sz)
        {
            let simplified = simplify_trimesh_data(decoded.0, decoded.1)?;
            ctx.trimesh_cache.insert(cache_key, simplified.clone());
            return Some(simplified);
        }

        let normalized = normalize_source_slashes(source);
        if normalized.as_ref() != source {
            let bytes = lookup(string_to_u64(normalized.as_ref()));
            if !bytes.is_empty()
                && let Some(decoded) = decode_pmesh_trimesh(bytes, sx, sy, sz)
            {
                let simplified = simplify_trimesh_data(decoded.0, decoded.1)?;
                ctx.trimesh_cache.insert(cache_key, simplified.clone());
                return Some(simplified);
            }
        }
        if let Some(alias) = normalized_static_mesh_lookup_alias(source) {
            let bytes = lookup(string_to_u64(alias.as_str()));
            if !bytes.is_empty()
                && let Some(decoded) = decode_pmesh_trimesh(bytes, sx, sy, sz)
            {
                let simplified = simplify_trimesh_data(decoded.0, decoded.1)?;
                ctx.trimesh_cache.insert(cache_key, simplified.clone());
                return Some(simplified);
            }
        }
        if normalized.as_ref() != source
            && let Some(alias) = normalized_static_mesh_lookup_alias(normalized.as_ref())
        {
            let bytes = lookup(string_to_u64(alias.as_str()));
            if !bytes.is_empty()
                && let Some(decoded) = decode_pmesh_trimesh(bytes, sx, sy, sz)
            {
                let simplified = simplify_trimesh_data(decoded.0, decoded.1)?;
                ctx.trimesh_cache.insert(cache_key, simplified.clone());
                return Some(simplified);
            }
        }
    }

    if ctx.provider_mode == PhysicsProviderMode::Static
        && let Some(lookup) = ctx.static_mesh_lookup
    {
        let source_hash = parse_hashed_source_uri(source).unwrap_or_else(|| string_to_u64(source));
        let bytes = lookup(source_hash);
        if !bytes.is_empty()
            && let Some(decoded) = decode_pmesh_trimesh(bytes, sx, sy, sz)
        {
            let simplified = simplify_trimesh_data(decoded.0, decoded.1)?;
            ctx.trimesh_cache.insert(cache_key, simplified.clone());
            return Some(simplified);
        }
    }

    let (path, fragment) = split_source_fragment(source);
    let mesh_index = if fragment.is_some() {
        parse_fragment_index(fragment, "mesh")?
    } else {
        0
    };

    let bytes = load_asset(path).ok()?;
    if path.ends_with(".pmesh") {
        let loaded = decode_pmesh_trimesh(&bytes, sx, sy, sz)?;
        let simplified = simplify_trimesh_data(loaded.0, loaded.1)?;
        ctx.trimesh_cache.insert(cache_key, simplified.clone());
        return Some(simplified);
    }
    if path.ends_with(".glb") || path.ends_with(".gltf") {
        let loaded = load_trimesh_from_gltf_bytes(&bytes, mesh_index, sx, sy, sz)?;
        let simplified = simplify_trimesh_data(loaded.0, loaded.1)?;
        ctx.trimesh_cache.insert(cache_key, simplified.clone());
        return Some(simplified);
    }
    None
}

pub fn normalize_source_slashes(source: &str) -> std::borrow::Cow<'_, str> {
    if source.contains('\\') {
        std::borrow::Cow::Owned(source.replace('\\', "/"))
    } else {
        std::borrow::Cow::Borrowed(source)
    }
}

pub fn normalized_static_mesh_lookup_alias(source: &str) -> Option<String> {
    let (path, fragment) = split_source_fragment(source);
    if !(path.ends_with(".glb") || path.ends_with(".gltf")) {
        return None;
    }
    match parse_fragment_index(fragment, "mesh") {
        Some(0) => Some(path.to_string()),
        Some(_) => None,
        None => Some(format!("{path}:mesh[0]")),
    }
}

pub fn decode_pmesh_trimesh(bytes: &[u8], sx: f32, sy: f32, sz: f32) -> Option<TriMeshData> {
    if bytes.len() < 33 || &bytes[0..5] != b"PMESH" {
        return None;
    }
    let version = u32::from_le_bytes(bytes[5..9].try_into().ok()?);
    if version != PMESH_VERSION {
        return None;
    }
    if let Some(render_trimesh) = decode_render_pmesh_trimesh(bytes, sx, sy, sz) {
        return Some(render_trimesh);
    }
    let flags = u32::from_le_bytes(bytes[9..13].try_into().ok()?);
    let vertex_count = u32::from_le_bytes(bytes[13..17].try_into().ok()?) as usize;
    let index_count = u32::from_le_bytes(bytes[17..21].try_into().ok()?) as usize;
    let raw_len = u32::from_le_bytes(bytes[29..33].try_into().ok()?) as usize;
    let payload_start = 33usize;

    let raw = decode_pmesh_payload(flags, &bytes[payload_start..])?;
    if raw.len() != raw_len {
        return None;
    }

    let index_u16 = (flags & PMESH_FLAG_INDEX_U16) != 0;
    let vertex_stride = 12usize;
    let vertex_bytes = vertex_count.checked_mul(vertex_stride)?;
    let index_bytes = index_count.checked_mul(if index_u16 { 2 } else { 4 })?;
    if raw.len() < vertex_bytes + index_bytes {
        return None;
    }

    let mut vertices = Vec::with_capacity(vertex_count);
    for i in 0..vertex_count {
        let off = i * vertex_stride;
        let x = f32::from_le_bytes(raw[off..off + 4].try_into().ok()?);
        let y = f32::from_le_bytes(raw[off + 4..off + 8].try_into().ok()?);
        let z = f32::from_le_bytes(raw[off + 8..off + 12].try_into().ok()?);
        vertices.push(na3::Point3::new(x * sx, y * sy, z * sz));
    }

    let mut triangles = Vec::new();
    let index_start = vertex_bytes;
    for tri_idx in (0..index_count / 3).map(|i| i * 3) {
        let ia = read_trimesh_index(raw.as_slice(), index_start, tri_idx, index_u16)?;
        let ib = read_trimesh_index(raw.as_slice(), index_start, tri_idx + 1, index_u16)?;
        let ic = read_trimesh_index(raw.as_slice(), index_start, tri_idx + 2, index_u16)?;
        let a = ia as usize;
        let b = ib as usize;
        let c = ic as usize;
        if a >= vertices.len()
            || b >= vertices.len()
            || c >= vertices.len()
            || a == b
            || b == c
            || a == c
        {
            continue;
        }
        triangles.push([ia, ib, ic]);
    }

    if vertices.len() < 3 || triangles.is_empty() {
        return None;
    }
    Some((vertices, triangles))
}

pub fn decode_render_pmesh_trimesh(bytes: &[u8], sx: f32, sy: f32, sz: f32) -> Option<TriMeshData> {
    if bytes.len() < 37 {
        return None;
    }
    let flags = u32::from_le_bytes(bytes[9..13].try_into().ok()?);
    let vertex_count = u32::from_le_bytes(bytes[13..17].try_into().ok()?) as usize;
    let index_count = u32::from_le_bytes(bytes[17..21].try_into().ok()?) as usize;
    let surface_count = u32::from_le_bytes(bytes[21..25].try_into().ok()?) as usize;
    let meshlet_count = u32::from_le_bytes(bytes[25..29].try_into().ok()?) as usize;
    let lod_count = u32::from_le_bytes(bytes[29..33].try_into().ok()?) as usize;
    let raw_len = u32::from_le_bytes(bytes[33..37].try_into().ok()?) as usize;
    let raw = decode_pmesh_payload(flags, &bytes[37..])?;
    if raw.len() != raw_len {
        return None;
    }
    let has_normal = (flags & (1 << 0)) != 0;
    let has_uv0 = (flags & (1 << 1)) != 0;
    let has_joints = (flags & (1 << 2)) != 0;
    let has_weights = (flags & (1 << 3)) != 0;
    let stride = 12
        + if has_normal { 12 } else { 0 }
        + if has_uv0 { 8 } else { 0 }
        + if has_joints { 8 } else { 0 }
        + if has_weights { 16 } else { 0 };
    let vertex_bytes = vertex_count.checked_mul(stride)?;
    let index_bytes = index_count.checked_mul(4)?;
    let surface_bytes = surface_count.checked_mul(8)?;
    let meshlet_bytes = meshlet_count.checked_mul(24)?;
    let lod_start = vertex_bytes
        .checked_add(index_bytes)?
        .checked_add(surface_bytes)?
        .checked_add(meshlet_bytes)?;
    if raw.len() < lod_start {
        return None;
    }
    let (lod_index_start, lod_index_count) = if lod_count > 0 && raw.len() >= lod_start + 24 {
        (
            u32::from_le_bytes(raw[lod_start..lod_start + 4].try_into().ok()?) as usize,
            u32::from_le_bytes(raw[lod_start + 4..lod_start + 8].try_into().ok()?) as usize,
        )
    } else {
        (0, index_count)
    };
    let mut vertices = Vec::with_capacity(vertex_count);
    for i in 0..vertex_count {
        let off = i * stride;
        vertices.push(na3::Point3::new(
            f32::from_le_bytes(raw[off..off + 4].try_into().ok()?) * sx,
            f32::from_le_bytes(raw[off + 4..off + 8].try_into().ok()?) * sy,
            f32::from_le_bytes(raw[off + 8..off + 12].try_into().ok()?) * sz,
        ));
    }
    let index_start = vertex_bytes + lod_index_start.saturating_mul(4);
    let index_end = index_start
        .saturating_add(lod_index_count.saturating_mul(4))
        .min(vertex_bytes + index_bytes);
    let mut triangles = Vec::new();
    for off in (index_start..index_end).step_by(12) {
        if off + 12 > raw.len() {
            break;
        }
        let ia = u32::from_le_bytes(raw[off..off + 4].try_into().ok()?);
        let ib = u32::from_le_bytes(raw[off + 4..off + 8].try_into().ok()?);
        let ic = u32::from_le_bytes(raw[off + 8..off + 12].try_into().ok()?);
        let a = ia as usize;
        let b = ib as usize;
        let c = ic as usize;
        if a < vertices.len()
            && b < vertices.len()
            && c < vertices.len()
            && a != b
            && b != c
            && a != c
        {
            triangles.push([ia, ib, ic]);
        }
    }
    if vertices.len() < 3 || triangles.is_empty() {
        return None;
    }
    Some((vertices, triangles))
}

pub fn decode_pmesh_payload(flags: u32, payload: &[u8]) -> Option<Vec<u8>> {
    if (flags & PMESH_FLAG_PAYLOAD_RAW) != 0 {
        Some(payload.to_vec())
    } else {
        decompress_zlib(payload).ok()
    }
}

pub fn read_trimesh_index(
    raw: &[u8],
    index_start: usize,
    index: usize,
    index_u16: bool,
) -> Option<u32> {
    if index_u16 {
        let off = index_start + index * 2;
        Some(u16::from_le_bytes(raw[off..off + 2].try_into().ok()?) as u32)
    } else {
        let off = index_start + index * 4;
        Some(u32::from_le_bytes(raw[off..off + 4].try_into().ok()?))
    }
}

pub fn load_trimesh_from_gltf_bytes(
    bytes: &[u8],
    mesh_index: usize,
    sx: f32,
    sy: f32,
    sz: f32,
) -> Option<TriMeshData> {
    let (doc, buffers, _images) = gltf::import_slice(bytes).ok()?;
    let mesh = doc.meshes().nth(mesh_index)?;

    let mut vertices = Vec::<na3::Point3<f32>>::new();
    let mut triangles = Vec::<[u32; 3]>::new();

    for primitive in mesh.primitives() {
        let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(|d| d.0.as_slice()));
        let Some(pos_iter) = reader.read_positions() else {
            continue;
        };

        let local_positions: Vec<[f32; 3]> = pos_iter.collect();
        if local_positions.len() < 3 {
            continue;
        }

        let Ok(base) = u32::try_from(vertices.len()) else {
            return None;
        };
        for p in &local_positions {
            vertices.push(na3::Point3::new(p[0] * sx, p[1] * sy, p[2] * sz));
        }

        if let Some(indices_reader) = reader.read_indices() {
            let mut flat: Vec<u32> = indices_reader.into_u32().collect();
            let tri_len = flat.len() / 3 * 3;
            flat.truncate(tri_len);
            for tri in flat.chunks_exact(3) {
                let ia = tri[0] as usize;
                let ib = tri[1] as usize;
                let ic = tri[2] as usize;
                if ia >= local_positions.len()
                    || ib >= local_positions.len()
                    || ic >= local_positions.len()
                {
                    continue;
                }
                let a = base + tri[0];
                let b = base + tri[1];
                let c = base + tri[2];
                if a != b && b != c && a != c {
                    triangles.push([a, b, c]);
                }
            }
        } else {
            for i in (0..local_positions.len() / 3 * 3).step_by(3) {
                let a = base + i as u32;
                let b = base + i as u32 + 1;
                let c = base + i as u32 + 2;
                triangles.push([a, b, c]);
            }
        }
    }

    if vertices.len() < 3 || triangles.is_empty() {
        return None;
    }
    Some((vertices, triangles))
}

pub fn trimesh_cache_key(
    source: &str,
    sx: f32,
    sy: f32,
    sz: f32,
    provider_mode: PhysicsProviderMode,
) -> u64 {
    string_to_u64(&format!(
        "{source}|{:08x}|{:08x}|{:08x}|{}",
        sx.to_bits(),
        sy.to_bits(),
        sz.to_bits(),
        provider_mode as u8
    ))
}

pub fn simplify_trimesh_data(
    vertices: Vec<na3::Point3<f32>>,
    triangles: Vec<[u32; 3]>,
) -> Option<TriMeshData> {
    let (vertices, triangles) = weld_and_filter_mesh(vertices, triangles)?;
    if let Some((reduced_vertices, reduced_triangles)) =
        simplify_coplanar_mesh(&vertices, &triangles)
    {
        return weld_and_filter_mesh(reduced_vertices, reduced_triangles);
    }
    Some((vertices, triangles))
}

pub fn weld_and_filter_mesh(
    vertices: Vec<na3::Point3<f32>>,
    triangles: Vec<[u32; 3]>,
) -> Option<TriMeshData> {
    let mut remap = vec![0u32; vertices.len()];
    let mut map = AHashMap::<(i64, i64, i64), u32>::default();
    let mut out_vertices = Vec::<na3::Point3<f32>>::new();
    let eps = 0.0001f32;
    for (idx, v) in vertices.iter().enumerate() {
        let key = (
            (v.x / eps).round() as i64,
            (v.y / eps).round() as i64,
            (v.z / eps).round() as i64,
        );
        let out_idx = if let Some(existing) = map.get(&key) {
            *existing
        } else {
            let next = out_vertices.len() as u32;
            map.insert(key, next);
            out_vertices.push(*v);
            next
        };
        remap[idx] = out_idx;
    }

    let mut unique = AHashSet::<(u32, u32, u32)>::default();
    let mut out_triangles = Vec::<[u32; 3]>::new();
    for tri in triangles {
        let a = remap.get(tri[0] as usize).copied()?;
        let b = remap.get(tri[1] as usize).copied()?;
        let c = remap.get(tri[2] as usize).copied()?;
        if a == b || b == c || a == c {
            continue;
        }
        let pa = out_vertices[a as usize];
        let pb = out_vertices[b as usize];
        let pc = out_vertices[c as usize];
        if triangle_area_sq(pa, pb, pc) <= 1.0e-12 {
            continue;
        }
        let mut ord = [a, b, c];
        ord.sort_unstable();
        if !unique.insert((ord[0], ord[1], ord[2])) {
            continue;
        }
        out_triangles.push([a, b, c]);
    }

    if out_vertices.len() < 3 || out_triangles.is_empty() {
        return None;
    }
    Some((out_vertices, out_triangles))
}

pub fn simplify_coplanar_mesh(
    vertices: &[na3::Point3<f32>],
    triangles: &[[u32; 3]],
) -> Option<TriMeshData> {
    if triangles.len() < 16 {
        return None;
    }
    let first = triangles[0];
    let p0 = vertices[first[0] as usize];
    let p1 = vertices[first[1] as usize];
    let p2 = vertices[first[2] as usize];
    let n = (p1 - p0).cross(&(p2 - p0));
    let n_len = n.norm();
    if n_len <= 1.0e-6 {
        return None;
    }
    let n = n / n_len;
    let plane_d = n.dot(&p0.coords);
    let plane_eps = 0.0025f32;
    for p in vertices {
        let dist = (n.dot(&p.coords) - plane_d).abs();
        if dist > plane_eps {
            return None;
        }
    }

    let axis = dominant_axis_3d(n.x, n.y, n.z);
    let mut pts2d = Vec::<[f32; 2]>::with_capacity(vertices.len());
    for p in vertices {
        pts2d.push(project_axis_3d(*p, axis));
    }

    let mut unique_2d = pts2d.clone();
    unique_2d.sort_by(|a, b| {
        a[0].partial_cmp(&b[0])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a[1].partial_cmp(&b[1]).unwrap_or(std::cmp::Ordering::Equal))
    });
    unique_2d.dedup_by(|a, b| (a[0] - b[0]).abs() <= 1.0e-5 && (a[1] - b[1]).abs() <= 1.0e-5);
    if unique_2d.len() < 3 {
        return None;
    }

    let hull = convex_hull_2d(&unique_2d);
    if hull.len() < 3 {
        return None;
    }

    let hull_area = polygon_area_abs(&hull);
    if hull_area <= 1.0e-6 {
        return None;
    }
    let mut tri_area_sum = 0.0f32;
    for tri in triangles {
        let a = pts2d[tri[0] as usize];
        let b = pts2d[tri[1] as usize];
        let c = pts2d[tri[2] as usize];
        tri_area_sum += ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() * 0.5;
    }
    if tri_area_sum <= 1.0e-6 {
        return None;
    }
    if hull_area > tri_area_sum * 1.1 {
        return None;
    }

    let mut new_vertices = Vec::<na3::Point3<f32>>::with_capacity(hull.len());
    for p in &hull {
        new_vertices.push(unproject_axis_on_plane(*p, axis, n, plane_d));
    }
    let mut new_triangles = Vec::<[u32; 3]>::new();
    for i in 1..hull.len() - 1 {
        new_triangles.push([0, i as u32, (i + 1) as u32]);
    }
    Some((new_vertices, new_triangles))
}

pub fn dominant_axis_3d(x: f32, y: f32, z: f32) -> usize {
    let ax = x.abs();
    let ay = y.abs();
    let az = z.abs();
    if ax >= ay && ax >= az {
        0
    } else if ay >= az {
        1
    } else {
        2
    }
}

pub fn project_axis_3d(p: na3::Point3<f32>, axis: usize) -> [f32; 2] {
    match axis {
        0 => [p.y, p.z],
        1 => [p.x, p.z],
        _ => [p.x, p.y],
    }
}

pub fn unproject_axis_on_plane(
    p: [f32; 2],
    axis: usize,
    n: na3::Vector3<f32>,
    d: f32,
) -> na3::Point3<f32> {
    match axis {
        0 => {
            let y = p[0];
            let z = p[1];
            let x = (d - n.y * y - n.z * z) / n.x.max(1.0e-6).copysign(n.x);
            na3::Point3::new(x, y, z)
        }
        1 => {
            let x = p[0];
            let z = p[1];
            let y = (d - n.x * x - n.z * z) / n.y.max(1.0e-6).copysign(n.y);
            na3::Point3::new(x, y, z)
        }
        _ => {
            let x = p[0];
            let y = p[1];
            let z = (d - n.x * x - n.y * y) / n.z.max(1.0e-6).copysign(n.z);
            na3::Point3::new(x, y, z)
        }
    }
}

pub fn convex_hull_2d(points: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let mut pts = points.to_vec();
    pts.sort_by(|a, b| {
        a[0].partial_cmp(&b[0])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a[1].partial_cmp(&b[1]).unwrap_or(std::cmp::Ordering::Equal))
    });
    if pts.len() <= 3 {
        return pts;
    }
    let mut lower = Vec::<[f32; 2]>::new();
    for p in &pts {
        while lower.len() >= 2
            && cross2(
                sub2(lower[lower.len() - 1], lower[lower.len() - 2]),
                sub2(*p, lower[lower.len() - 1]),
            ) <= 0.0
        {
            lower.pop();
        }
        lower.push(*p);
    }
    let mut upper = Vec::<[f32; 2]>::new();
    for p in pts.iter().rev() {
        while upper.len() >= 2
            && cross2(
                sub2(upper[upper.len() - 1], upper[upper.len() - 2]),
                sub2(*p, upper[upper.len() - 1]),
            ) <= 0.0
        {
            upper.pop();
        }
        upper.push(*p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

pub fn polygon_area_abs(poly: &[[f32; 2]]) -> f32 {
    if poly.len() < 3 {
        return 0.0;
    }
    let mut area = 0.0f32;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        area += a[0] * b[1] - a[1] * b[0];
    }
    area.abs() * 0.5
}

pub fn sub2(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

pub fn cross2(a: [f32; 2], b: [f32; 2]) -> f32 {
    a[0] * b[1] - a[1] * b[0]
}

pub fn triangle_area_sq(a: na3::Point3<f32>, b: na3::Point3<f32>, c: na3::Point3<f32>) -> f32 {
    let ab = b - a;
    let ac = c - a;
    ab.cross(&ac).norm_squared() * 0.25
}

pub fn split_source_fragment(source: &str) -> (&str, Option<&str>) {
    let Some((path, selector)) = source.rsplit_once(':') else {
        return (source, None);
    };
    if path.is_empty() {
        return (source, None);
    }
    if selector.contains('/') || selector.contains('\\') {
        return (source, None);
    }
    if selector.contains('[') && selector.ends_with(']') {
        return (path, Some(selector));
    }
    (source, None)
}

pub fn parse_fragment_index(fragment: Option<&str>, key: &str) -> Option<usize> {
    let fragment = fragment?;
    let (name, rest) = fragment.split_once('[')?;
    if name.trim() != key {
        return None;
    }
    let value = rest.strip_suffix(']')?.trim();
    value.parse::<usize>().ok()
}

pub fn triangle_points_2d(
    kind: Triangle2DKind,
    width: f32,
    height: f32,
) -> Option<[na2::Point2<f32>; 3]> {
    let w = width.abs().max(0.0001);
    let mut h = height.abs().max(0.0001);
    let points = match kind {
        Triangle2DKind::Equilateral => {
            h = h.max((3.0f32).sqrt() * 0.5 * w);
            [
                na2::Point2::new(-w * 0.5, -h / 3.0),
                na2::Point2::new(w * 0.5, -h / 3.0),
                na2::Point2::new(0.0, 2.0 * h / 3.0),
            ]
        }
        Triangle2DKind::Right => [
            na2::Point2::new(-w / 3.0, -h / 3.0),
            na2::Point2::new(2.0 * w / 3.0, -h / 3.0),
            na2::Point2::new(-w / 3.0, 2.0 * h / 3.0),
        ],
        Triangle2DKind::Isosceles => [
            na2::Point2::new(-w * 0.5, -h * 0.5),
            na2::Point2::new(w * 0.5, -h * 0.5),
            na2::Point2::new(0.0, h * 0.5),
        ],
    };
    Some(points)
}

pub fn tri_prism_points(width: f32, height: f32, depth: f32) -> Vec<na3::Point3<f32>> {
    let hw = width.abs().max(0.0001) * 0.5;
    let hh = height.abs().max(0.0001) * 0.5;
    let hd = depth.abs().max(0.0001) * 0.5;
    vec![
        na3::Point3::new(-hw, -hh, -hd),
        na3::Point3::new(hw, -hh, -hd),
        na3::Point3::new(0.0, hh, -hd),
        na3::Point3::new(-hw, -hh, hd),
        na3::Point3::new(hw, -hh, hd),
        na3::Point3::new(0.0, hh, hd),
    ]
}

pub fn triangular_pyramid_points(width: f32, height: f32, depth: f32) -> Vec<na3::Point3<f32>> {
    let hw = width.abs().max(0.0001) * 0.5;
    let hh = height.abs().max(0.0001) * 0.5;
    let hd = depth.abs().max(0.0001) * 0.5;
    vec![
        na3::Point3::new(-hw, -hh, -hd),
        na3::Point3::new(hw, -hh, -hd),
        na3::Point3::new(0.0, -hh, hd),
        na3::Point3::new(0.0, hh, 0.0),
    ]
}

pub fn square_pyramid_points(width: f32, height: f32, depth: f32) -> Vec<na3::Point3<f32>> {
    let hw = width.abs().max(0.0001) * 0.5;
    let hh = height.abs().max(0.0001) * 0.5;
    let hd = depth.abs().max(0.0001) * 0.5;
    vec![
        na3::Point3::new(-hw, -hh, -hd),
        na3::Point3::new(hw, -hh, -hd),
        na3::Point3::new(hw, -hh, hd),
        na3::Point3::new(-hw, -hh, hd),
        na3::Point3::new(0.0, hh, 0.0),
    ]
}

pub fn transform_to_iso2(transform: Transform2D) -> na2::Isometry2<f32> {
    na2::Isometry2::new(
        na2::Vector2::new(transform.position.x, transform.position.y),
        transform.rotation,
    )
}

pub fn transform_to_iso3(transform: Transform3D) -> na3::Isometry3<f32> {
    let rotation = na3::UnitQuaternion::from_quaternion(na3::Quaternion::new(
        transform.rotation.w,
        transform.rotation.x,
        transform.rotation.y,
        transform.rotation.z,
    ));
    na3::Isometry3::from_parts(
        na3::Translation3::new(
            transform.position.x,
            transform.position.y,
            transform.position.z,
        ),
        rotation,
    )
}

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

pub fn prepared_audio_raycast_2d_in_world(
    world: &PhysicsWorld2D,
    origin: Vector2,
    direction: Vector2,
    max_distance: f32,
    mask: u32,
) -> Option<PhysicsRayHit2D> {
    if max_distance <= 0.0 || !max_distance.is_finite() {
        return None;
    }
    let dir = na2::Vector2::new(direction.x, direction.y);
    let dir_len = dir.norm();
    if dir_len <= 0.000_001 || !dir_len.is_finite() {
        return None;
    }
    let dir = dir / dir_len;
    let ray = r2::Ray::new(na2::Point2::new(origin.x, origin.y), dir);
    let predicate = |handle, collider: &r2::Collider| {
        (collider.collision_groups().memberships.bits() & mask) != 0
            && world.collider_owners.contains_key(&handle)
    };
    let query_filter = r2::QueryFilter::new()
        .exclude_sensors()
        .predicate(&predicate);
    let (collider, hit) = world.query_pipeline.cast_ray_and_get_normal(
        &world.bodies,
        &world.colliders,
        &ray,
        max_distance,
        true,
        query_filter,
    )?;
    let node = *world.collider_owners.get(&collider)?;
    let point = ray.point_at(hit.time_of_impact);
    Some(PhysicsRayHit2D {
        node,
        point: Vector2::new(point.x, point.y),
        normal: Vector2::new(hit.normal.x, hit.normal.y),
        distance: hit.time_of_impact,
    })
}

pub fn prepared_audio_raycast_3d_in_world(
    world: &PhysicsWorld3D,
    origin: Vector3,
    direction: Vector3,
    max_distance: f32,
    include_areas: bool,
) -> Option<PhysicsRayHit3D> {
    if max_distance <= 0.0 || !max_distance.is_finite() {
        return None;
    }
    let dir = na3::Vector3::new(direction.x, direction.y, direction.z);
    let dir_len = dir.norm();
    if dir_len <= 0.000_001 || !dir_len.is_finite() {
        return None;
    }
    let dir = dir / dir_len;
    let ray = r3::Ray::new(na3::Point3::new(origin.x, origin.y, origin.z), dir);
    let filter = if include_areas {
        r3::QueryFilter::new()
    } else {
        r3::QueryFilter::new().exclude_sensors()
    };
    let (collider, hit) = world.query_pipeline.cast_ray_and_get_normal(
        &world.bodies,
        &world.colliders,
        &ray,
        max_distance,
        true,
        filter,
    )?;
    let node = *world.collider_owners.get(&collider)?;
    let point = ray.point_at(hit.time_of_impact);
    Some(PhysicsRayHit3D {
        node,
        point: Vector3::new(point.x, point.y, point.z),
        normal: Vector3::new(hit.normal.x, hit.normal.y, hit.normal.z),
        distance: hit.time_of_impact,
    })
}
