use crate::BitMask;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CollisionPolicy {
    pub layers: BitMask,
    pub mask: BitMask,
}

impl CollisionPolicy {
    pub const DEFAULT: Self = Self {
        layers: BitMask::ALL,
        mask: BitMask::NONE,
    };
    pub const NONE: Self = Self {
        layers: BitMask::NONE,
        mask: BitMask::NONE,
    };

    pub const fn new(layers: BitMask, mask: BitMask) -> Self {
        Self { layers, mask }
    }

    pub const fn from_bits(layers: u32, mask: u32) -> Self {
        Self {
            layers: BitMask::from_bits(layers),
            mask: BitMask::from_bits(mask),
        }
    }

    pub const fn layer(layer: u8) -> BitMask {
        BitMask::layer(layer)
    }

    pub const fn layers<const N: usize>(layers: [u8; N]) -> BitMask {
        BitMask::with(layers)
    }

    pub const fn on(layer_bit: u8, mask: BitMask) -> Self {
        Self {
            layers: Self::layer(layer_bit),
            mask,
        }
    }

    pub const fn all_on(layer_bit: u8) -> Self {
        Self::on(layer_bit, BitMask::NONE)
    }

    pub const fn can_collide(self, other: Self) -> bool {
        !self.mask.intersects(other.layers) && !other.mask.intersects(self.layers)
    }
}

impl Default for CollisionPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::CollisionPolicy;
    use crate::BitMask;

    #[test]
    fn can_collide_uses_policy_mask_as_ignored_layers() {
        let a = CollisionPolicy::new(BitMask::with([1]), BitMask::NONE);
        let b = CollisionPolicy::new(BitMask::with([2]), BitMask::NONE);
        assert!(a.can_collide(b));

        let a_ignores_b = CollisionPolicy::new(BitMask::with([1]), BitMask::with([2]));
        assert!(!a_ignores_b.can_collide(b));

        let b_ignores_a = CollisionPolicy::new(BitMask::with([2]), BitMask::with([1]));
        assert!(!a.can_collide(b_ignores_a));
    }
}
