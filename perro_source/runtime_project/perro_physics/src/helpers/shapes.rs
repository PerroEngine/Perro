use super::*;

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
    layer: BitMask,
    mask: BitMask,
    friction: f32,
    restitution: f32,
    density: f32,
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
                collision_layers: layer,
                collision_mask: mask,
                friction,
                restitution,
                density,
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
            collision_layers: layer,
            collision_mask: mask,
            friction,
            restitution,
            density,
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
        collision_layers: BitMask::with([1]),
        collision_mask: BitMask::ALL,
        friction,
        restitution,
        density: 1.0,
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
        collision_layers: BitMask::with([1]),
        collision_mask: BitMask::ALL,
        friction,
        restitution,
        density: 1.0,
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
            .additional_mass(rigid.mass.max(0.0))
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
                desc.collision_layers,
                desc.collision_mask,
            ))
            .friction(desc.friction)
            .restitution(desc.restitution)
            .density(desc.density.max(0.0))
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
                desc.collision_layers,
                desc.collision_mask,
            ))
            .friction(desc.friction)
            .restitution(desc.restitution)
            .density(desc.density.max(0.0))
            .build(),
    )
}

pub fn interaction_groups_2d(layer: BitMask, mask: BitMask) -> r2::InteractionGroups {
    r2::InteractionGroups::new(
        r2::Group::from_bits_truncate(layer.bits()),
        r2::Group::from_bits_truncate(mask.bits()),
    )
}

pub fn interaction_groups_3d(layer: BitMask, mask: BitMask) -> r3::InteractionGroups {
    r3::InteractionGroups::new(
        r3::Group::from_bits_truncate(layer.bits()),
        r3::Group::from_bits_truncate(mask.bits()),
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
