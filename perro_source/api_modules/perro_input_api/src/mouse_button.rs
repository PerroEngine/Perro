#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MouseButton {
    Left = 0,
    Right = 1,
    Middle = 2,
    Back = 3,
    Forward = 4,
}

impl MouseButton {
    pub const COUNT: usize = 5;

    #[inline]
    pub const fn bit(self) -> u8 {
        1u8 << (self as u8)
    }

    #[inline]
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim() {
            "Left" | "MouseLeft" => Some(Self::Left),
            "Right" | "MouseRight" => Some(Self::Right),
            "Middle" | "MouseMiddle" => Some(Self::Middle),
            "Back" | "MouseBack" => Some(Self::Back),
            "Forward" | "MouseForward" => Some(Self::Forward),
            _ => None,
        }
    }
}
