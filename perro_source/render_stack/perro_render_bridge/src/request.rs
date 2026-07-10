#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderRequestID(pub u128);

impl RenderRequestID {
    #[inline]
    pub const fn new(raw: u64) -> Self {
        Self(raw as u128)
    }

    #[inline]
    pub const fn from_u128(raw: u128) -> Self {
        Self(raw)
    }
}
