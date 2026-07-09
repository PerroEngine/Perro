use super::*;

#[test]
fn node_id_nil() {
    let nil = NodeID::nil();
    assert!(nil.is_nil());
    assert_eq!(nil.index(), 0);
    assert_eq!(nil.generation(), 0);
}

#[test]
fn node_id_parts() {
    let id = NodeID::from_parts(5, 2);
    assert_eq!(id.index(), 5);
    assert_eq!(id.generation(), 2);
    assert!(!id.is_nil());
}

#[test]
fn node_id_roundtrip_u64_various() {
    // Broad sanity coverage without assuming internal bit layout.
    let cases: &[(u32, u32)] = &[
        (0, 0),
        (1, 0),
        (0, 1),
        (1, 1),
        (5, 2),
        (12345, 77),
        (u32::MAX, 0),
        (0, u32::MAX),
        (u32::MAX, u32::MAX),
    ];

    for &(i, g) in cases {
        let id = NodeID::from_parts(i, g);
        let packed = id.as_u64();
        let unpacked = NodeID::from_u64(packed);
        assert_eq!(
            unpacked, id,
            "roundtrip failed for index={i} generation={g} packed={packed}"
        );
    }
}

#[test]
fn node_id_nil_roundtrip_u64() {
    let nil = NodeID::nil();
    assert_eq!(NodeID::from_u64(nil.as_u64()), nil);
}

#[test]
fn texture_id_nil_invariants() {
    let nil = TextureID::nil();
    assert!(nil.is_nil());
    // If your TextureID defines these invariants, keep them.
    // If not, remove these two asserts.
    assert_eq!(nil.index(), 0);
    assert_eq!(nil.generation(), 0);
}

#[test]
fn texture_id_generational() {
    let id = TextureID::from_parts(3, 1);
    assert_eq!(id.index(), 3);
    assert_eq!(id.generation(), 1);
    assert!(!id.is_nil());
}

#[test]
fn generational_id_parse_accepts_documented_hex_forms() {
    assert_eq!("1".parse::<NodeID>(), Ok(NodeID::from_u64(1)));
    assert_eq!(
        NodeID::parse_str("0x0123456789abcdef"),
        Ok(NodeID::from_u64(0x0123_4567_89ab_cdef))
    );
    assert_eq!(
        "01234567-89ABCDEF".parse::<TextureID>(),
        Ok(TextureID::from_u64(0x0123_4567_89ab_cdef))
    );
    assert_eq!(
        "ffffffffffffffff".parse::<MaterialID>(),
        Ok(MaterialID::from_u64(u64::MAX))
    );
}

#[test]
fn generational_id_parse_rejects_malformed_input() {
    for malformed in [
        "",
        "0x",
        "10000000000000000",
        "0x10000000000000000",
        "1-2",
        "-0123456789abcdef",
        "0123456789abcdef-",
        "01234567--89abcdef",
        "01234567-89abcdef-",
        "0123456g",
    ] {
        assert!(
            malformed.parse::<NodeID>().is_err(),
            "accepted malformed ID: {malformed}"
        );
        assert!(
            TextureID::parse_str(malformed).is_err(),
            "accepted malformed ID: {malformed}"
        );
    }

    assert_eq!("".parse::<NodeID>(), Err(ParseGenerationalIDError::Empty));
    assert_eq!(
        "10000000000000000".parse::<NodeID>(),
        Err(ParseGenerationalIDError::TooLong)
    );
    assert_eq!(
        "1-2".parse::<NodeID>(),
        Err(ParseGenerationalIDError::MisplacedSeparator)
    );
    assert_eq!(
        "not-hex".parse::<NodeID>(),
        Err(ParseGenerationalIDError::MisplacedSeparator)
    );
    assert_eq!(
        "xyz".parse::<NodeID>(),
        Err(ParseGenerationalIDError::InvalidHex)
    );
}

#[test]
fn signal_id_from_string_deterministic() {
    let a = SignalID::from_string("on_damage");
    let b = SignalID::from_string("on_damage");
    let c = SignalID::from_string("on_heal");
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn tag_id_from_string_deterministic() {
    let a = TagID::from_string("enemy");
    let b = TagID::from_string("enemy");
    let c = TagID::from_string("ally");
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn tags_macro_builds_slice() {
    let built = tags!["enemy", "boss"];
    assert_eq!(built.len(), 2);
    assert_eq!(built[0].id(), TagID::from_string("enemy"));
    assert_eq!(built[0].name(), "enemy");
    assert_eq!(built[1].id(), TagID::from_string("boss"));
    assert_eq!(built[1].name(), "boss");
}
