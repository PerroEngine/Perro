#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderRequestID(pub u64);

impl RenderRequestID {
    #[inline]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }
}
