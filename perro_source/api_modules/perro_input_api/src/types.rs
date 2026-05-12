#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MouseMode {
    #[default]
    Visible,
    Hidden,
    Captured,
    Confined,
    ConfinedHidden,
}

impl MouseMode {
    #[inline]
    pub fn cursor_visible(self) -> bool {
        matches!(self, Self::Visible | Self::Confined)
    }

    #[inline]
    pub fn is_captured(self) -> bool {
        matches!(self, Self::Captured)
    }

    #[inline]
    pub fn is_confined(self) -> bool {
        matches!(self, Self::Confined | Self::ConfinedHidden)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GamepadIndex(pub usize);

impl From<usize> for GamepadIndex {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl From<GamepadIndex> for usize {
    fn from(value: GamepadIndex) -> Self {
        value.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JoyConIndex(pub usize);

impl From<usize> for JoyConIndex {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl From<JoyConIndex> for usize {
    fn from(value: JoyConIndex) -> Self {
        value.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RumbleIntensity {
    pub low_frequency: f32,
    pub high_frequency: f32,
}

impl RumbleIntensity {
    #[inline]
    pub fn new(low_frequency: f32, high_frequency: f32) -> Self {
        Self {
            low_frequency,
            high_frequency,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlayerIndicatorSlot(pub u8);
pub type PlayerIndicator = PlayerIndicatorSlot;

impl PlayerIndicatorSlot {
    pub const COUNT: u8 = 8;
    pub const LAMP_PATTERNS: [u8; Self::COUNT as usize] = [
        0b0001, 0b0011, 0b0111, 0b1111, 0b1001, 0b1010, 0b1011, 0b0110,
    ];

    #[inline]
    pub fn from_slot(slot: u8) -> Option<Self> {
        if slot < Self::COUNT {
            Some(Self(slot))
        } else {
            None
        }
    }

    #[inline]
    pub fn from_player_number(player_number: usize) -> Option<Self> {
        if (1..=(Self::COUNT as usize)).contains(&player_number) {
            Some(Self((player_number - 1) as u8))
        } else {
            None
        }
    }

    #[inline]
    pub fn from_lamp_pattern(pattern: u8) -> Option<Self> {
        Self::LAMP_PATTERNS
            .iter()
            .position(|&candidate| candidate == pattern)
            .map(|slot| Self(slot as u8))
    }

    #[inline]
    pub fn from_slot_or_lamp_pattern(value: u8) -> Option<Self> {
        Self::from_slot(value).or_else(|| Self::from_lamp_pattern(value))
    }

    #[inline]
    pub fn to_lamp_pattern(self) -> u8 {
        Self::LAMP_PATTERNS[self.0 as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GamepadRumbleRequest {
    pub index: usize,
    pub rumble: RumbleIntensity,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JoyConRumbleRequest {
    pub index: usize,
    pub rumble: RumbleIntensity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct JoyConIndicatorRequest {
    pub index: usize,
    pub indicator: PlayerIndicatorSlot,
}
