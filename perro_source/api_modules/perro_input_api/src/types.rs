/// Requested mouse cursor behavior.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MouseMode {
    /// Cursor visible and free.
    #[default]
    Visible,
    /// Cursor hidden and free.
    Hidden,
    /// Cursor hidden and captured for relative movement.
    Captured,
    /// Cursor visible and confined to the window.
    Confined,
    /// Cursor hidden and confined to the window.
    ConfinedHidden,
}

impl MouseMode {
    /// Return whether this mode displays the OS cursor.
    #[inline]
    pub fn cursor_visible(self) -> bool {
        matches!(self, Self::Visible | Self::Confined)
    }

    /// Return whether this mode captures relative movement.
    #[inline]
    pub fn is_captured(self) -> bool {
        matches!(self, Self::Captured)
    }

    /// Return whether this mode confines the cursor to the window.
    #[inline]
    pub fn is_confined(self) -> bool {
        matches!(self, Self::Confined | Self::ConfinedHidden)
    }
}

/// Gamepad slot index.
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

/// Joy-Con slot index.
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

/// Low/high frequency rumble intensity request.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RumbleIntensity {
    /// Low-frequency motor intensity.
    pub low_frequency: f32,
    /// High-frequency motor intensity.
    pub high_frequency: f32,
}

impl RumbleIntensity {
    /// Create a rumble request.
    #[inline]
    pub fn new(low_frequency: f32, high_frequency: f32) -> Self {
        Self {
            low_frequency,
            high_frequency,
        }
    }
}

/// Joy-Con player indicator slot or mapped lamp pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlayerIndicatorSlot(pub u8);

/// Backward-compatible alias for [`PlayerIndicatorSlot`].
pub type PlayerIndicator = PlayerIndicatorSlot;

impl PlayerIndicatorSlot {
    /// Number of supported player indicator slots.
    pub const COUNT: u8 = 8;
    /// Lamp bit patterns indexed by player slot.
    pub const LAMP_PATTERNS: [u8; Self::COUNT as usize] = [
        0b0001, 0b0011, 0b0111, 0b1111, 0b1001, 0b1010, 0b1011, 0b0110,
    ];

    /// Convert zero-based slot to an indicator.
    #[inline]
    pub fn from_slot(slot: u8) -> Option<Self> {
        if slot < Self::COUNT {
            Some(Self(slot))
        } else {
            None
        }
    }

    /// Convert one-based player number to an indicator.
    #[inline]
    pub fn from_player_number(player_number: usize) -> Option<Self> {
        if (1..=(Self::COUNT as usize)).contains(&player_number) {
            Some(Self((player_number - 1) as u8))
        } else {
            None
        }
    }

    /// Convert a Joy-Con lamp bit pattern to an indicator slot.
    #[inline]
    pub fn from_lamp_pattern(pattern: u8) -> Option<Self> {
        Self::LAMP_PATTERNS
            .iter()
            .position(|&candidate| candidate == pattern)
            .map(|slot| Self(slot as u8))
    }

    /// Accept either a zero-based slot or an exact lamp bit pattern.
    #[inline]
    pub fn from_slot_or_lamp_pattern(value: u8) -> Option<Self> {
        Self::from_slot(value).or_else(|| Self::from_lamp_pattern(value))
    }

    /// Return the Joy-Con lamp bit pattern for this slot.
    #[inline]
    pub fn to_lamp_pattern(self) -> u8 {
        Self::LAMP_PATTERNS[self.0 as usize]
    }
}

/// Pending gamepad rumble command produced by script input APIs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GamepadRumbleRequest {
    /// Target gamepad slot.
    pub index: usize,
    /// Requested rumble intensity.
    pub rumble: RumbleIntensity,
}

/// Pending Joy-Con rumble command produced by script input APIs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JoyConRumbleRequest {
    /// Target Joy-Con slot.
    pub index: usize,
    /// Requested rumble intensity.
    pub rumble: RumbleIntensity,
}

/// Pending Joy-Con player indicator command produced by script input APIs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct JoyConIndicatorRequest {
    /// Target Joy-Con slot.
    pub index: usize,
    /// Requested indicator slot.
    pub indicator: PlayerIndicatorSlot,
}
