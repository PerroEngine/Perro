#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BitMask(u32);

pub trait IntoBitMaskLayer {
    fn into_bitmask_layer(self) -> Option<u8>;
}

pub trait IntoBitMaskLayers {
    fn into_bitmask(self) -> Option<BitMask>;
}

macro_rules! impl_into_bitmask_layer_unsigned {
    ($($ty:ty),* $(,)?) => {
        $(
            impl IntoBitMaskLayer for $ty {
                #[inline]
                fn into_bitmask_layer(self) -> Option<u8> {
                    if (1..=32).contains(&self) {
                        Some(self as u8)
                    } else {
                        None
                    }
                }
            }

            impl IntoBitMaskLayer for &$ty {
                #[inline]
                fn into_bitmask_layer(self) -> Option<u8> {
                    (*self).into_bitmask_layer()
                }
            }

            impl IntoBitMaskLayers for $ty {
                #[inline]
                fn into_bitmask(self) -> Option<BitMask> {
                    BitMask::try_layer(self.into_bitmask_layer()?)
                }
            }

            impl IntoBitMaskLayers for &$ty {
                #[inline]
                fn into_bitmask(self) -> Option<BitMask> {
                    (*self).into_bitmask()
                }
            }
        )*
    };
}

impl_into_bitmask_layer_unsigned!(u8, u16, u32, u64, usize);

macro_rules! impl_into_bitmask_layer_signed {
    ($($ty:ty),* $(,)?) => {
        $(
            impl IntoBitMaskLayer for $ty {
                #[inline]
                fn into_bitmask_layer(self) -> Option<u8> {
                    if (1..=32).contains(&self) {
                        Some(self as u8)
                    } else {
                        None
                    }
                }
            }

            impl IntoBitMaskLayer for &$ty {
                #[inline]
                fn into_bitmask_layer(self) -> Option<u8> {
                    (*self).into_bitmask_layer()
                }
            }

            impl IntoBitMaskLayers for $ty {
                #[inline]
                fn into_bitmask(self) -> Option<BitMask> {
                    BitMask::try_layer(self.into_bitmask_layer()?)
                }
            }

            impl IntoBitMaskLayers for &$ty {
                #[inline]
                fn into_bitmask(self) -> Option<BitMask> {
                    (*self).into_bitmask()
                }
            }
        )*
    };
}

impl_into_bitmask_layer_signed!(i8, i16, i32, i64, isize);

impl<L, const N: usize> IntoBitMaskLayers for [L; N]
where
    L: IntoBitMaskLayer,
{
    #[inline]
    fn into_bitmask(self) -> Option<BitMask> {
        BitMask::try_from_layers(self)
    }
}

impl<L> IntoBitMaskLayers for &[L]
where
    for<'a> &'a L: IntoBitMaskLayer,
{
    #[inline]
    fn into_bitmask(self) -> Option<BitMask> {
        BitMask::try_from_layers(self)
    }
}

impl<L> IntoBitMaskLayers for &Vec<L>
where
    for<'a> &'a L: IntoBitMaskLayer,
{
    #[inline]
    fn into_bitmask(self) -> Option<BitMask> {
        BitMask::try_from_layers(self)
    }
}

impl BitMask {
    pub const NONE: Self = Self(0);
    pub const ALL: Self = Self(u32::MAX);

    #[inline]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    #[inline]
    pub const fn bits(self) -> u32 {
        self.0
    }

    #[inline]
    pub const fn bit(bit: u8) -> Self {
        if bit < u32::BITS as u8 {
            Self(1u32 << bit)
        } else {
            Self::NONE
        }
    }

    #[inline]
    pub const fn layer(layer: u8) -> Self {
        assert!(layer >= 1 && layer <= 32, "BitMask layer must be 1..=32");
        Self::bit(layer - 1)
    }

    #[inline]
    pub const fn try_layer(layer: u8) -> Option<Self> {
        if layer >= 1 && layer <= 32 {
            Some(Self::bit(layer - 1))
        } else {
            None
        }
    }

    #[inline]
    pub const fn with<const N: usize>(layers: [u8; N]) -> Self {
        let mut bits = 0u32;
        let mut i = 0usize;
        while i < N {
            bits |= Self::layer(layers[i]).bits();
            i += 1;
        }
        Self(bits)
    }

    #[inline]
    pub fn from_layers<I, L>(layers: I) -> Self
    where
        I: IntoIterator<Item = L>,
        L: IntoBitMaskLayer,
    {
        Self::try_from_layers(layers).expect("BitMask layer must be 1..=32")
    }

    #[inline]
    pub fn try_from_layers<I, L>(layers: I) -> Option<Self>
    where
        I: IntoIterator<Item = L>,
        L: IntoBitMaskLayer,
    {
        let mut bits = 0u32;
        for layer in layers {
            let layer = layer.into_bitmask_layer()?;
            bits |= Self::layer(layer).bits();
        }
        Some(Self(bits))
    }

    #[inline]
    pub const fn with_bits<const N: usize>(bits_in: [u8; N]) -> Self {
        let mut bits = 0u32;
        let mut i = 0usize;
        while i < N {
            bits |= Self::bit(bits_in[i]).bits();
            i += 1;
        }
        Self(bits)
    }

    #[inline]
    pub const fn without_layers<const N: usize>(self, layers: [u8; N]) -> Self {
        Self(self.bits() & !Self::with(layers).bits())
    }

    #[inline]
    pub fn push<L>(&mut self, layers: L)
    where
        L: IntoBitMaskLayers,
    {
        *self = self.pushed(layers);
    }

    #[inline]
    pub fn pushed<L>(self, layers: L) -> Self
    where
        L: IntoBitMaskLayers,
    {
        Self(
            self.bits()
                | layers
                    .into_bitmask()
                    .expect("BitMask layer must be 1..=32")
                    .bits(),
        )
    }

    #[inline]
    pub fn pop<L>(&mut self, layers: L)
    where
        L: IntoBitMaskLayers,
    {
        *self = self.popped(layers);
    }

    #[inline]
    pub fn popped<L>(self, layers: L) -> Self
    where
        L: IntoBitMaskLayers,
    {
        Self(
            self.bits()
                & !layers
                    .into_bitmask()
                    .expect("BitMask layer must be 1..=32")
                    .bits(),
        )
    }

    #[inline]
    pub fn without<L>(layers: L) -> Self
    where
        L: IntoBitMaskLayers,
    {
        Self::ALL.popped(layers)
    }

    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.bits() | other.bits())
    }

    #[inline]
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.bits() & other.bits())
    }

    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.bits() & other.bits()) == other.bits()
    }

    #[inline]
    pub const fn intersects(self, other: Self) -> bool {
        (self.bits() & other.bits()) != 0
    }

    #[inline]
    pub const fn is_empty(self) -> bool {
        self.bits() == 0
    }
}

impl Default for BitMask {
    #[inline]
    fn default() -> Self {
        Self::ALL
    }
}

impl From<u32> for BitMask {
    #[inline]
    fn from(value: u32) -> Self {
        Self::from_bits(value)
    }
}

impl From<BitMask> for u32 {
    #[inline]
    fn from(value: BitMask) -> Self {
        value.bits()
    }
}

#[macro_export]
macro_rules! bitmask {
    ([]) => {
        $crate::BitMask::NONE
    };
    () => {
        $crate::BitMask::NONE
    };
    ([$($layer:expr),+ $(,)?]) => {
        $crate::BitMask::with([$($layer),+])
    };
}

#[cfg(test)]
mod tests {
    use super::BitMask;

    #[test]
    fn with_uses_one_based_layers() {
        let mask = BitMask::with([1, 3, 4]);

        assert_eq!(mask.bits(), 0b1101);
        assert!(mask.contains(BitMask::with([1, 3])));
        assert!(!mask.contains(BitMask::with([2])));
    }

    #[test]
    fn without_removes_layers() {
        let mask = BitMask::without([2, 4]);

        assert!(mask.intersects(BitMask::with([1])));
        assert!(!mask.intersects(BitMask::with([2])));
        assert!(!mask.intersects(BitMask::with([4])));
    }

    #[test]
    fn without_accepts_single_layer() {
        let mask = BitMask::without(1);

        assert!(!mask.intersects(BitMask::with([1])));
        assert!(mask.intersects(BitMask::with([2])));
        assert!(mask.intersects(BitMask::with([32])));
    }

    #[test]
    fn push_and_pop_mutate_layers() {
        let mut mask = BitMask::NONE;

        mask.push(5);
        assert_eq!(mask.bits(), 1u32 << 4);

        mask.push([1, 32]);
        assert!(mask.intersects(BitMask::with([1])));
        assert!(mask.intersects(BitMask::with([5])));
        assert!(mask.intersects(BitMask::with([32])));

        mask.pop(5);
        assert!(mask.intersects(BitMask::with([1])));
        assert!(!mask.intersects(BitMask::with([5])));
        assert!(mask.intersects(BitMask::with([32])));
    }

    #[test]
    fn pushed_and_popped_return_new_masks() {
        let old_mask = BitMask::with([1, 2]);
        let pushed = old_mask.pushed(5);
        let popped = pushed.popped([1, 5]);

        assert_eq!(old_mask.bits(), 0b11);
        assert_eq!(pushed.bits(), 0b1_0011);
        assert_eq!(popped.bits(), 0b10);
    }

    #[test]
    fn pop_then_push_restores_all_layers() {
        let original = BitMask::ALL;
        let mut mask = original;

        mask.pop(5);
        assert_ne!(mask, original);

        mask.push(5);
        assert_eq!(mask, original);
    }

    #[test]
    fn layer_helper_combos_match() {
        let with = BitMask::with([1, 3, 5]);
        let pushed = BitMask::NONE.pushed([1, 3]).pushed(5);
        let mut push_pop = BitMask::NONE;
        push_pop.push([1, 3, 5]);
        push_pop.pop(3);
        push_pop.push(3);

        assert_eq!(with, pushed);
        assert_eq!(with, push_pop);

        let without = BitMask::without([2, 4, 6]);
        let without_layers = BitMask::ALL.without_layers([2, 4, 6]);
        let popped = BitMask::ALL.popped([2, 4]).popped(6);
        let mut pop_mut = BitMask::ALL;
        pop_mut.pop([2, 4, 6]);

        assert_eq!(without, without_layers);
        assert_eq!(without, popped);
        assert_eq!(without, pop_mut);

        let rebuilt = without.pushed([2, 4, 6]);
        let mut rebuilt_mut = without;
        rebuilt_mut.push([2, 4, 6]);

        assert_eq!(rebuilt, BitMask::ALL);
        assert_eq!(rebuilt_mut, BitMask::ALL);
    }

    #[test]
    fn pushed_and_popped_accept_same_layer_inputs_as_with_without() {
        let slice_u8: &[u8] = &[1, 4];
        let vec_usize = vec![2usize, 5usize];
        let arr_usize = [3usize, 6usize];

        let built = BitMask::NONE
            .pushed(slice_u8)
            .pushed(&vec_usize)
            .pushed(arr_usize);

        assert_eq!(built, BitMask::with([1, 2, 3, 4, 5, 6]));

        let removed = BitMask::ALL
            .popped(slice_u8)
            .popped(&vec_usize)
            .popped(arr_usize);
        let without = BitMask::without([1, 2, 3, 4, 5, 6]);

        assert_eq!(removed, without);
    }

    #[test]
    fn without_layers_stays_const() {
        const MASK: BitMask = BitMask::ALL.without_layers([1, 32]);

        assert!(!MASK.intersects(BitMask::with([1])));
        assert!(MASK.intersects(BitMask::with([2])));
        assert!(!MASK.intersects(BitMask::with([32])));
    }

    #[test]
    fn invalid_layers_use_try_layer() {
        assert_eq!(BitMask::try_layer(0), None);
        assert_eq!(BitMask::try_layer(33), None);
        assert_eq!(BitMask::try_layer(32).map(BitMask::bits), Some(1u32 << 31));
    }

    #[test]
    fn bitmask_macro_builds_masks() {
        const EMPTY: BitMask = bitmask!([]);
        const ALSO_EMPTY: BitMask = bitmask!();
        const MIXED: BitMask = bitmask!([1, 3, 4]);

        assert_eq!(EMPTY, BitMask::NONE);
        assert_eq!(ALSO_EMPTY, BitMask::NONE);
        assert_eq!(MIXED.bits(), 0b1101);
    }

    #[test]
    fn from_layers_accepts_slices_vecs_and_usize() {
        let slice_u8: &[u8] = &[1, 3, 4];
        let vec_usize = vec![2usize, 5usize];
        let arr_usize = [1usize, 32usize];

        assert_eq!(BitMask::from_layers(slice_u8).bits(), 0b1101);
        assert_eq!(BitMask::from_layers(&vec_usize).bits(), 0b1_0010);
        assert_eq!(BitMask::from_layers(arr_usize).bits(), 1 | (1u32 << 31));
        assert_eq!(BitMask::try_from_layers([0usize]), None);
        assert_eq!(BitMask::try_from_layers([33u8]), None);
    }
}
