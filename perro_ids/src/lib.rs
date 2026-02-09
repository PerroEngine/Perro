pub mod ids;

pub use ids::*;

#[cfg(test)]
mod tests {
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
    fn ui_element_from_string_deterministic() {
        let a = UIElementID::from_string("x-border");
        let b = UIElementID::from_string("x-border");
        assert_eq!(a, b);
    }

    #[test]
    fn ui_element_from_string_distinguishes_common_cases() {
        let a = UIElementID::from_string("x-border");
        let b = UIElementID::from_string("x-border2");
        let c = UIElementID::from_string("X-BORDER");

        assert_ne!(a, b);
        assert_ne!(a, c);
    }
}
