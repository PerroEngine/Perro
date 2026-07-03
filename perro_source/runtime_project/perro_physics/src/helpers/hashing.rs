use super::*;

pub fn body_signature_seed(kind: BodyKind) -> u64 {
    match kind {
        BodyKind::Static => 0xA91B_D58C_24F1_7E31,
        BodyKind::Area => 0xCC42_83B7_9E20_11DD,
        BodyKind::Rigid => 0x6D1E_93A4_F02C_B871,
        BodyKind::Character => 0x51F8_0A6E_D3B4_29C5,
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

pub fn hash_collision_shape_flip_3d(mut state: u64, shape: &CollisionShape3D) -> u64 {
    state = hash_u64(state, shape.flip_x as u64);
    state = hash_u64(state, shape.flip_y as u64);
    hash_u64(state, shape.flip_z as u64)
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

pub fn hash_tile_collision_shape_2d(mut state: u64, shape: &ParsedTileCollisionShape2D) -> u64 {
    match shape {
        ParsedTileCollisionShape2D::Auto => hash_u32(state, 1),
        ParsedTileCollisionShape2D::Shape { shape, offset } => {
            state = hash_u32(state, 2);
            state = hash_shape_2d(state, tile_set_shape_to_shape_2d(*shape));
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
