use crate::BitMask;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CollisionMasks {
    pub layers: BitMask,
    pub mask: BitMask,
}

impl CollisionMasks {
    pub const DEFAULT: Self = Self {
        layers: BitMask::with([1]),
        mask: BitMask::ALL,
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
        Self::on(layer_bit, BitMask::ALL)
    }

    pub const fn can_collide(self, other: Self) -> bool {
        self.mask.intersects(other.layers) && other.mask.intersects(self.layers)
    }
}

impl Default for CollisionMasks {
    fn default() -> Self {
        Self::DEFAULT
    }
}
