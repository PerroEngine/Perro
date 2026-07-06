use super::*;

#[test]
fn ui_rect_converts_center_origin_y_up_to_screen_rect() {
    let rect = UiRectState {
        center: [300.0, 0.0],
        size: [200.0, 100.0],
        pivot: [0.5, 0.5],
        rotation_radians: 0.0,
        z_index: 0,
    };

    let (min, max) = rect.screen_min_max([800.0, 600.0]);

    assert_eq!(min, [600.0, 250.0]);
    assert_eq!(max, [800.0, 350.0]);
}

#[test]
fn tileset_binary_roundtrip_keeps_collision_shapes() {
    let tileset = parse_ptileset_source(
            r#"
            texture = "res://tiles/world.png"
            tile_size = (16, 16)
            columns = 2
            rows = 1
            tiles = [
                { id = 1 atlas = (0, 0) collision = true collision_shape = "auto" },
                { id = 2 atlas = (1, 0) collision = true collision_shape = { polygon = { points = [(0, 0), (16, 0), (8, 16)] offset = (1, -2) } } },
            ]
            "#,
        )
        .expect("tileset parses");

    let bytes = encode_tileset_2d_binary(&tileset);
    assert_eq!(&bytes[0..5], b"PTSET");
    assert_eq!(u32::from_le_bytes(bytes[5..9].try_into().unwrap()), 1);
    let decoded = decode_tileset_2d_binary(&bytes).expect("tileset decodes");

    assert_eq!(decoded, tileset);
}

#[test]
fn tileset_binary_rejects_hostile_tile_count_no_huge_alloc() {
    // tile_count = u32::MAX but no tile data follow.
    // clamp -> with_capacity stay small, decode ret None on exhausted bytes.
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"PTSET");
    bytes.extend_from_slice(&1u32.to_le_bytes()); // version
    bytes.extend_from_slice(&3u32.to_le_bytes()); // texture_len
    bytes.extend_from_slice(b"abc"); // texture
    bytes.extend_from_slice(&16.0f32.to_le_bytes()); // tile_size.x
    bytes.extend_from_slice(&16.0f32.to_le_bytes()); // tile_size.y
    bytes.extend_from_slice(&2u32.to_le_bytes()); // columns
    bytes.extend_from_slice(&1u32.to_le_bytes()); // rows
    bytes.extend_from_slice(&u32::MAX.to_le_bytes()); // hostile tile_count

    assert!(decode_tileset_2d_binary(&bytes).is_none());
}
