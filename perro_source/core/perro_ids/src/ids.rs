//! Type-safe generational identifiers (slotmap-style) for arenas.
//! All IDs use u64 = index (low 32 bits) | generation (high 32 bits). Index 0 = nil.
//! IDs are created by their owning arena/manager; slot reuse bumps generation so stale IDs are invalid.

use std::{borrow::Cow, fmt, ops::Deref};

const STRING_HASH_SEED: u64 = 0xA0761D6478BD642F;
const STRING_HASH_PRIME: u64 = 0xE7037ED1A0B428DB;
const EMPTY_STRING_HASH: u64 = mix64(STRING_HASH_SEED.wrapping_mul(STRING_HASH_SEED));

#[inline]
pub const fn string_to_u64(s: &str) -> u64 {
    let bytes = s.as_bytes();
    let len = bytes.len();
    if len == 0 {
        return EMPTY_STRING_HASH;
    }

    let mut hash = STRING_HASH_SEED ^ (len as u64).wrapping_mul(STRING_HASH_PRIME);
    let mut i = 0usize;

    while i + 8 <= len {
        hash ^= read_u64_le(bytes, i);
        hash = hash.wrapping_mul(STRING_HASH_PRIME);
        hash ^= hash >> 32;
        i += 8;
    }

    let mut tail = 0u64;
    let mut shift = 0u32;
    while i < len {
        tail |= (bytes[i] as u64) << shift;
        shift += 8;
        i += 1;
    }

    hash ^= tail;
    hash = hash.wrapping_mul(STRING_HASH_SEED);
    mix64(hash)
}

const fn read_u64_le(bytes: &[u8], offset: usize) -> u64 {
    (bytes[offset] as u64)
        | ((bytes[offset + 1] as u64) << 8)
        | ((bytes[offset + 2] as u64) << 16)
        | ((bytes[offset + 3] as u64) << 24)
        | ((bytes[offset + 4] as u64) << 32)
        | ((bytes[offset + 5] as u64) << 40)
        | ((bytes[offset + 6] as u64) << 48)
        | ((bytes[offset + 7] as u64) << 56)
}

pub fn parse_hashed_source_uri(s: &str) -> Option<u64> {
    if s.as_bytes().iter().all(|b| b.is_ascii_digit()) {
        return s.parse::<u64>().ok();
    }
    None
}

pub const fn mix64(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58476D1CE4E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D049BB133111EB);
    x ^= x >> 31;
    x
}

/// Error returned when a generational ID string has invalid hex syntax.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseGenerationalIDError {
    /// No hex digits remain after the optional prefix.
    Empty,
    /// An unseparated value contains more than 16 hex digits.
    TooLong,
    /// A hyphen does not split two groups of exactly 8 hex digits.
    MisplacedSeparator,
    /// A group contains a non-hex digit.
    InvalidHex,
}

impl fmt::Display for ParseGenerationalIDError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Empty => "ID string is empty",
            Self::TooLong => "ID string has more than 16 hex digits",
            Self::MisplacedSeparator => "ID separator must split two groups of 8 hex digits",
            Self::InvalidHex => "ID string contains a non-hex digit",
        };
        f.write_str(message)
    }
}

impl std::error::Error for ParseGenerationalIDError {}

/// Parse a generational ID from 1-16 hex digits.
///
/// A lowercase `0x` prefix is optional. The legacy `xxxxxxxx-xxxxxxxx` form is
/// accepted; a hyphen at any other position is rejected.
pub fn parse_generational_id(s: &str) -> Result<u64, ParseGenerationalIDError> {
    let hex = s.strip_prefix("0x").unwrap_or(s);
    if hex.is_empty() {
        return Err(ParseGenerationalIDError::Empty);
    }

    if let Some((high, low)) = hex.split_once('-') {
        if high.len() != 8 || low.len() != 8 || low.contains('-') {
            return Err(ParseGenerationalIDError::MisplacedSeparator);
        }
        let high = parse_hex_group(high)?;
        let low = parse_hex_group(low)?;
        return Ok((high << 32) | low);
    }

    if hex.len() > 16 {
        return Err(ParseGenerationalIDError::TooLong);
    }
    parse_hex_group(hex)
}

fn parse_hex_group(hex: &str) -> Result<u64, ParseGenerationalIDError> {
    u64::from_str_radix(hex, 16).map_err(|_| ParseGenerationalIDError::InvalidHex)
}

// ---- Generational ID: base encoding ----
// u64 layout: low 32 = index (0 = nil, 1.. = slot), high 32 = generation.
// When a slot is reused, generation is bumped so old IDs no longer match.

/// Defines a generational ID type (NodeID, TextureID, MaterialID, etc.).
/// All such IDs use index + generation for safe arena slot reuse.
macro_rules! define_generational {
    ($type_name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $type_name(pub u64);

        impl $type_name {
            #[inline]
            pub const fn new(id: u32) -> Self {
                Self::from_parts(id, 0)
            }

            #[inline]
            pub const fn nil() -> Self {
                Self(0)
            }

            #[inline]
            pub const fn index(self) -> u32 {
                (self.0 & 0xFFFF_FFFF) as u32
            }

            #[inline]
            pub const fn generation(self) -> u32 {
                (self.0 >> 32) as u32
            }

            #[inline]
            pub const fn from_parts(index: u32, generation: u32) -> Self {
                Self((index as u64) | ((generation as u64) << 32))
            }

            #[inline]
            pub const fn as_u64(self) -> u64 {
                self.0
            }

            #[inline]
            pub const fn from_u64(value: u64) -> Self {
                Self(value)
            }

            #[inline]
            pub const fn is_nil(self) -> bool {
                self.0 == 0
            }

            /// Legacy: index in low 32, generation 0 (e.g. deserialization).
            #[inline]
            pub const fn from_u32(index: u32) -> Self {
                Self::from_parts(index, 0)
            }
        }

        impl Default for $type_name {
            fn default() -> Self {
                Self::nil()
            }
        }

        impl fmt::Debug for $type_name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(
                    f,
                    concat!(stringify!($type_name), "({}:{})"),
                    self.index(),
                    self.generation()
                )
            }
        }

        impl fmt::Display for $type_name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}:{}", self.index(), self.generation())
            }
        }

        impl std::str::FromStr for $type_name {
            type Err = ParseGenerationalIDError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                parse_generational_id(s).map(Self::from_u64)
            }
        }
    };
}

define_generational!(
    NodeID,
    "Node ID — allocated by NodeArena. Index + generation."
);
define_generational!(
    TextureID,
    "Texture ID — allocated by TextureManager. Index + generation."
);
define_generational!(
    WebcamID,
    "Webcam ID - allocated by webcam capture system. Index + generation."
);
define_generational!(
    MaterialID,
    "Material ID — allocated by material system. Index + generation."
);
define_generational!(
    MeshID,
    "Mesh ID — allocated by mesh system. Index + generation."
);
define_generational!(
    AnimationID,
    "Animation ID - allocated by animation system. Index + generation."
);
define_generational!(
    TileSetID,
    "Tile set ID - stable hashed tile set resource key. Index + generation."
);
define_generational!(
    ParticleProfileID,
    "Particle profile ID - stable hashed particle profile resource key."
);
define_generational!(
    AnimationTreeID,
    "Animation tree ID - allocated by animation tree system. Index + generation."
);
define_generational!(
    LightID,
    "Light ID — allocated by light system. Index + generation."
);
define_generational!(
    SignalID,
    "Signal ID - hash of signal name (or generational). Used for connect/emit."
);
define_generational!(
    AudioBusID,
    "Bus ID - deterministic ID from bus name. Used for audio routing."
);
define_generational!(
    SoundFontID,
    "Sound font ID - deterministic ID from soundfont asset path."
);
define_generational!(
    TagID,
    "Tag ID - deterministic ID from tag name. Used for scene node tags and queries."
);
define_generational!(
    PreloadedSceneID,
    "Preloaded scene ID - runtime handle for a retained parsed scene."
);

impl NodeID {
    pub const ROOT: NodeID = Self::new(1);
    /// Parse 1-16 hex digits, optionally prefixed with `0x`.
    pub fn parse_str(s: &str) -> Result<Self, ParseGenerationalIDError> {
        s.parse()
    }
}

impl TextureID {
    /// Parse 1-16 hex digits, optionally prefixed with `0x`.
    pub fn parse_str(s: &str) -> Result<Self, ParseGenerationalIDError> {
        s.parse()
    }
}

impl SignalID {
    /// Deterministic ID from signal name. Uses hash; generation 0.
    pub const fn from_string(s: &str) -> Self {
        Self::from_u64(string_to_u64(s))
    }
}

impl AudioBusID {
    /// Deterministic ID from bus name. Uses hash; generation 0.
    pub const fn from_string(s: &str) -> Self {
        Self::from_u64(string_to_u64(s))
    }
}

impl SoundFontID {
    /// Deterministic ID from soundfont asset path. Uses hash; generation 0.
    pub const fn from_string(s: &str) -> Self {
        Self::from_u64(string_to_u64(s))
    }
}

impl TileSetID {
    /// Deterministic ID from tile set asset path. Uses hash; generation 0.
    pub const fn from_string(s: &str) -> Self {
        Self::from_u64(string_to_u64(s))
    }
}

impl ParticleProfileID {
    /// Deterministic ID from particle profile asset path. Uses hash; generation 0.
    pub const fn from_string(s: &str) -> Self {
        Self::from_u64(string_to_u64(s))
    }
}

impl TagID {
    /// Deterministic ID from tag name. Uses hash; generation 0.
    pub const fn from_string(s: &str) -> Self {
        Self::from_u64(string_to_u64(s))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TileSetRef {
    pub id: TileSetID,
    pub source: Cow<'static, str>,
}

impl TileSetRef {
    pub const fn empty() -> Self {
        Self {
            id: TileSetID::nil(),
            source: Cow::Borrowed(""),
        }
    }

    pub const fn borrowed(source: &'static str) -> Self {
        Self {
            id: TileSetID::from_string(source),
            source: Cow::Borrowed(source),
        }
    }

    pub fn new<S>(source: S) -> Self
    where
        S: Into<Cow<'static, str>>,
    {
        let source = source.into();
        let id = if source.as_ref().is_empty() {
            TileSetID::nil()
        } else {
            parse_hashed_source_uri(source.as_ref())
                .map(TileSetID::from_u64)
                .unwrap_or_else(|| TileSetID::from_string(source.as_ref()))
        };
        Self { id, source }
    }

    pub const fn id(&self) -> TileSetID {
        self.id
    }

    pub fn source(&self) -> &str {
        self.source.as_ref()
    }

    pub fn as_str(&self) -> &str {
        self.source()
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.source().as_bytes()
    }

    pub fn is_empty(&self) -> bool {
        self.source().is_empty()
    }
}

impl AsRef<str> for TileSetRef {
    fn as_ref(&self) -> &str {
        self.source()
    }
}

impl Deref for TileSetRef {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.source()
    }
}

impl Default for TileSetRef {
    fn default() -> Self {
        Self::empty()
    }
}

impl From<&str> for TileSetRef {
    fn from(source: &str) -> Self {
        Self::new(source.to_string())
    }
}

impl From<String> for TileSetRef {
    fn from(source: String) -> Self {
        Self::new(source)
    }
}

impl From<&String> for TileSetRef {
    fn from(source: &String) -> Self {
        Self::new(source.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ParticleProfileRef {
    pub id: ParticleProfileID,
    pub source: Cow<'static, str>,
}

impl ParticleProfileRef {
    pub const fn empty() -> Self {
        Self {
            id: ParticleProfileID::nil(),
            source: Cow::Borrowed(""),
        }
    }

    pub const fn borrowed(source: &'static str) -> Self {
        Self {
            id: ParticleProfileID::from_string(source),
            source: Cow::Borrowed(source),
        }
    }

    pub fn new<S>(source: S) -> Self
    where
        S: Into<Cow<'static, str>>,
    {
        let source = source.into();
        let id = if source.as_ref().is_empty() {
            ParticleProfileID::nil()
        } else {
            parse_hashed_source_uri(source.as_ref())
                .map(ParticleProfileID::from_u64)
                .unwrap_or_else(|| ParticleProfileID::from_string(source.as_ref()))
        };
        Self { id, source }
    }

    pub const fn id(&self) -> ParticleProfileID {
        self.id
    }

    pub fn source(&self) -> &str {
        self.source.as_ref()
    }

    pub fn as_str(&self) -> &str {
        self.source()
    }

    pub fn is_empty(&self) -> bool {
        self.source().is_empty()
    }
}

impl AsRef<str> for ParticleProfileRef {
    fn as_ref(&self) -> &str {
        self.source()
    }
}

impl Deref for ParticleProfileRef {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.source()
    }
}

impl Default for ParticleProfileRef {
    fn default() -> Self {
        Self::empty()
    }
}

impl From<&str> for ParticleProfileRef {
    fn from(source: &str) -> Self {
        Self::new(source.to_string())
    }
}

impl From<String> for ParticleProfileRef {
    fn from(source: String) -> Self {
        Self::new(source)
    }
}

impl From<&String> for ParticleProfileRef {
    fn from(source: &String) -> Self {
        Self::new(source.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodeTag {
    pub id: TagID,
    pub name: Cow<'static, str>,
}

impl NodeTag {
    pub const fn borrowed(name: &'static str) -> Self {
        Self {
            id: TagID::from_string(name),
            name: Cow::Borrowed(name),
        }
    }

    pub fn new<S>(name: S) -> Self
    where
        S: Into<Cow<'static, str>>,
    {
        let name = name.into();
        Self {
            id: TagID::from_string(name.as_ref()),
            name,
        }
    }

    pub const fn id(&self) -> TagID {
        self.id
    }

    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
}

impl From<TagID> for NodeTag {
    fn from(id: TagID) -> Self {
        Self {
            id,
            name: Cow::Borrowed(""),
        }
    }
}

impl From<&TagID> for NodeTag {
    fn from(id: &TagID) -> Self {
        (*id).into()
    }
}

impl From<&str> for NodeTag {
    fn from(name: &str) -> Self {
        Self::new(name.to_string())
    }
}

impl From<String> for NodeTag {
    fn from(name: String) -> Self {
        Self::new(name)
    }
}

impl From<&String> for NodeTag {
    fn from(name: &String) -> Self {
        Self::new(name.clone())
    }
}

pub trait IntoTagID {
    fn into_tag_id(self) -> TagID;
}

impl IntoTagID for TagID {
    #[inline]
    fn into_tag_id(self) -> TagID {
        self
    }
}

impl IntoTagID for &TagID {
    #[inline]
    fn into_tag_id(self) -> TagID {
        *self
    }
}

impl IntoTagID for NodeTag {
    #[inline]
    fn into_tag_id(self) -> TagID {
        self.id
    }
}

impl IntoTagID for &NodeTag {
    #[inline]
    fn into_tag_id(self) -> TagID {
        self.id
    }
}

impl IntoTagID for &str {
    #[inline]
    fn into_tag_id(self) -> TagID {
        TagID::from_string(self)
    }
}

impl IntoTagID for String {
    #[inline]
    fn into_tag_id(self) -> TagID {
        TagID::from_string(self.as_str())
    }
}

impl IntoTagID for &String {
    #[inline]
    fn into_tag_id(self) -> TagID {
        TagID::from_string(self.as_str())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ScriptMemberID(pub u64);

impl ScriptMemberID {
    pub const fn from_string(s: &str) -> Self {
        Self(string_to_u64(s))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ParticleProfileID, ParticleProfileRef, TileSetID, TileSetRef, parse_hashed_source_uri,
        string_to_u64,
    };
    use std::{hint::black_box, time::Instant};

    #[test]
    fn parse_hashed_source_uri_accepts_decimal() {
        assert_eq!(parse_hashed_source_uri("42"), Some(42));
    }

    #[test]
    fn parse_hashed_source_uri_rejects_raw_dlc_path() {
        let dlc_path = "dlc://test/scripts/script.rs";
        assert_eq!(parse_hashed_source_uri(dlc_path), None);
    }

    #[test]
    fn asset_refs_keep_source_and_typed_id() {
        let tileset = TileSetRef::from("res://tiles.ptileset");
        assert_eq!(tileset.id(), TileSetID::from_string("res://tiles.ptileset"));
        assert_eq!(tileset.source(), "res://tiles.ptileset");
        assert_eq!(tileset.as_ref(), "res://tiles.ptileset");

        let profile = ParticleProfileRef::from("12345");
        assert_eq!(profile.id(), ParticleProfileID::from_u64(12345));
        assert_eq!(profile.source(), "12345");
    }

    #[test]
    fn hash_str_macro_matches_string_to_u64() {
        const HASH: u64 = crate::hash_str!("res://textures/player.png");
        assert_eq!(HASH, string_to_u64("res://textures/player.png"));
    }

    #[test]
    #[ignore = "bench-style timing test; run with --ignored --nocapture"]
    fn bench_string_to_u64_by_length() {
        const LENGTHS: &[usize] = &[
            0, 1, 4, 8, 15, 16, 32, 64, 128, 256, 512, 1024, 4096, 16_384,
        ];
        const TARGET_BYTES: usize = 64 * 1024 * 1024;

        println!("len,iters,total_ms,ns_per_hash,ns_per_byte,hash_xor");
        for &len in LENGTHS {
            let input = make_bench_string(len);
            let bytes_per_iter = len.max(1);
            let iters = (TARGET_BYTES / bytes_per_iter).max(1_000);
            let mut hash_xor = 0u64;

            let start = Instant::now();
            for _ in 0..iters {
                hash_xor ^= string_to_u64(black_box(input.as_str()));
            }
            let elapsed = start.elapsed();

            let total_ns = elapsed.as_nanos() as f64;
            let ns_per_hash = total_ns / iters as f64;
            let ns_per_byte = if len == 0 {
                0.0
            } else {
                total_ns / (iters as f64 * len as f64)
            };

            println!(
                "{len},{iters},{:.3},{:.3},{:.3},{hash_xor}",
                elapsed.as_secs_f64() * 1000.0,
                ns_per_hash,
                ns_per_byte
            );
        }
    }

    fn make_bench_string(len: usize) -> String {
        const PATTERN: &[u8] =
            b"res://Sports/Golf/holes/hole_01.scn:material[0123456789]/abcdefghijklmnopqrstuvwxyz";
        let mut out = String::with_capacity(len);
        while out.len() < len {
            let take = (len - out.len()).min(PATTERN.len());
            out.push_str(std::str::from_utf8(&PATTERN[..take]).unwrap());
        }
        out
    }
}
