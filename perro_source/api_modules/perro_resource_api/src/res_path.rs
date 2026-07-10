use std::{
    borrow::{Borrow, Cow},
    fmt,
    ops::Deref,
    str::FromStr,
};

use perro_variant::{DeriveVariant, Variant};

pub trait ResPathSource {
    fn as_res_path_str(&self) -> &str;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResPathKind {
    Res,
    Dlc,
    User,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResPathError {
    Empty,
    MissingScheme,
    UnknownScheme,
    EmptyPath,
    EmptyDlcName,
    InvalidDlcName,
    InvalidSeparator,
    Traversal,
    ControlCharacter,
}

impl fmt::Display for ResPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Empty => "resource path is empty",
            Self::MissingScheme => "resource path must start with res://, dlc://, or user://",
            Self::UnknownScheme => "resource path scheme must be res, dlc, or user",
            Self::EmptyPath => "resource path body is empty",
            Self::EmptyDlcName => "dlc resource path needs a dlc name or self",
            Self::InvalidDlcName => "dlc resource path name has invalid characters",
            Self::InvalidSeparator => "resource path must use / separators",
            Self::Traversal => "resource path cannot contain . or .. path segments",
            Self::ControlCharacter => "resource path cannot contain control characters",
        };
        f.write_str(message)
    }
}

impl std::error::Error for ResPathError {}

#[repr(transparent)]
pub struct ResPath(str);

impl ResPath {
    pub const fn new(path: &'static str) -> &'static Self {
        validate_const(path);
        // SAFETY: validate_const rejects paths outside ResPath grammar.
        unsafe { Self::new_unchecked(path) }
    }

    pub fn try_new(path: &str) -> Result<&Self, ResPathError> {
        validate(path)?;
        // SAFETY: validate rejects paths outside ResPath grammar.
        Ok(unsafe { Self::new_unchecked_borrowed(path) })
    }

    pub fn intern(path: &str) -> Result<&'static Self, ResPathError> {
        validate(path)?;
        let path: &'static str = Box::leak(path.to_owned().into_boxed_str());
        // SAFETY: validate rejects paths outside ResPath grammar and leaked str is static.
        Ok(unsafe { Self::new_unchecked(path) })
    }

    /// # Safety
    ///
    /// Caller must pass a valid resource path.
    pub const unsafe fn new_unchecked(path: &'static str) -> &'static Self {
        // SAFETY: ResPath is repr(transparent) over str; caller upholds path validity.
        unsafe { &*(path as *const str as *const Self) }
    }

    /// # Safety
    ///
    /// Caller must pass a valid resource path.
    pub unsafe fn new_unchecked_borrowed(path: &str) -> &Self {
        // SAFETY: ResPath is repr(transparent) over str; caller upholds path validity.
        unsafe { &*(path as *const str as *const Self) }
    }

    pub fn kind(&self) -> ResPathKind {
        if self.0.starts_with("res://") {
            ResPathKind::Res
        } else if self.0.starts_with("dlc://") {
            ResPathKind::Dlc
        } else {
            ResPathKind::User
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn to_res_path_buf(&self) -> ResPathBuf {
        ResPathBuf(Cow::Owned(self.0.to_owned()))
    }

    pub fn to_buf(&self) -> ResPathBuf {
        self.to_res_path_buf()
    }

    pub fn dlc_name(&self) -> Option<&str> {
        let rest = self.0.strip_prefix("dlc://")?;
        Some(rest.split_once('/').map_or(rest, |(name, _)| name))
    }

    pub fn body(&self) -> &str {
        match self.kind() {
            ResPathKind::Res => &self.0["res://".len()..],
            ResPathKind::User => &self.0["user://".len()..],
            ResPathKind::Dlc => {
                let rest = &self.0["dlc://".len()..];
                rest.split_once('/').map_or("", |(_, body)| body)
            }
        }
    }
}

impl fmt::Debug for ResPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ResPath").field(&self.as_str()).finish()
    }
}

impl fmt::Display for ResPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for ResPath {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl ResPathSource for ResPath {
    fn as_res_path_str(&self) -> &str {
        self.as_str()
    }
}

impl ResPathSource for &ResPath {
    fn as_res_path_str(&self) -> &str {
        self.as_str()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ResPathBuf(Cow<'static, str>);

impl ResPathBuf {
    pub const fn new(path: &'static str) -> Self {
        validate_const(path);
        Self(Cow::Borrowed(path))
    }

    pub fn try_new(path: impl Into<String>) -> Result<Self, ResPathError> {
        let path = path.into();
        validate(&path)?;
        Ok(Self(Cow::Owned(path)))
    }

    pub fn as_res_path(&self) -> &ResPath {
        // SAFETY: ResPathBuf constructors validate owned and borrowed storage.
        unsafe { ResPath::new_unchecked_borrowed(&self.0) }
    }

    pub fn into_string(self) -> String {
        self.0.into_owned()
    }
}

impl Deref for ResPathBuf {
    type Target = ResPath;

    fn deref(&self) -> &Self::Target {
        self.as_res_path()
    }
}

impl Borrow<ResPath> for ResPathBuf {
    fn borrow(&self) -> &ResPath {
        self.as_res_path()
    }
}

impl AsRef<ResPath> for ResPathBuf {
    fn as_ref(&self) -> &ResPath {
        self.as_res_path()
    }
}

impl AsRef<str> for ResPathBuf {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ResPathSource for ResPathBuf {
    fn as_res_path_str(&self) -> &str {
        self.as_str()
    }
}

impl ResPathSource for &ResPathBuf {
    fn as_res_path_str(&self) -> &str {
        self.as_str()
    }
}

impl ResPathSource for str {
    fn as_res_path_str(&self) -> &str {
        self
    }
}

impl ResPathSource for &str {
    fn as_res_path_str(&self) -> &str {
        self
    }
}

impl ResPathSource for String {
    fn as_res_path_str(&self) -> &str {
        self
    }
}

impl ResPathSource for &String {
    fn as_res_path_str(&self) -> &str {
        self
    }
}

impl ResPathSource for Cow<'static, str> {
    fn as_res_path_str(&self) -> &str {
        self
    }
}

impl ResPathSource for &Cow<'static, str> {
    fn as_res_path_str(&self) -> &str {
        self
    }
}

impl fmt::Display for ResPathBuf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for ResPathBuf {
    type Err = ResPathError;

    fn from_str(path: &str) -> Result<Self, Self::Err> {
        Self::try_new(path)
    }
}

impl TryFrom<String> for ResPathBuf {
    type Error = ResPathError;

    fn try_from(path: String) -> Result<Self, Self::Error> {
        Self::try_new(path)
    }
}

impl TryFrom<&str> for ResPathBuf {
    type Error = ResPathError;

    fn try_from(path: &str) -> Result<Self, Self::Error> {
        Self::try_new(path)
    }
}

impl From<ResPathBuf> for Variant {
    fn from(path: ResPathBuf) -> Self {
        Variant::from(path.into_string())
    }
}

impl From<&ResPathBuf> for Variant {
    fn from(path: &ResPathBuf) -> Self {
        Variant::from(path.as_str())
    }
}

impl From<&ResPath> for Variant {
    fn from(path: &ResPath) -> Self {
        Variant::from(path.as_str())
    }
}

impl DeriveVariant for ResPathBuf {
    fn from_variant(value: &Variant) -> Option<Self> {
        Self::try_new(value.as_str()?).ok()
    }

    fn from_owned_variant(value: Variant) -> Option<Self> {
        match value {
            Variant::String(path) => Self::try_new(path.to_string()).ok(),
            _ => None,
        }
    }

    fn to_variant(&self) -> Variant {
        Variant::from(self.as_str())
    }

    fn into_variant(self) -> Variant {
        Variant::from(self)
    }
}

impl DeriveVariant for &'static ResPath {
    fn from_variant(value: &Variant) -> Option<Self> {
        ResPath::intern(value.as_str()?).ok()
    }

    fn to_variant(&self) -> Variant {
        Variant::from(self.as_str())
    }
}

pub const fn validate_const(path: &str) {
    let bytes = path.as_bytes();
    if bytes.is_empty() {
        panic!("empty ResPath");
    }
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] < 0x20 || bytes[i] == 0x7f {
            panic!("ResPath has control character");
        }
        if bytes[i] == b'\\' {
            panic!("ResPath must use / separators");
        }
        i += 1;
    }

    if starts_with(bytes, b"res://") {
        validate_body_const(bytes, 6);
    } else if starts_with(bytes, b"user://") {
        validate_body_const(bytes, 7);
    } else if starts_with(bytes, b"dlc://") {
        validate_dlc_const(bytes);
    } else if contains_scheme(bytes) {
        panic!("ResPath has unsupported scheme; use res://, dlc://, or user://");
    } else {
        panic!("ResPath missing scheme; start path with res://, dlc://, or user://");
    }
}

const fn validate_dlc_const(bytes: &[u8]) {
    let mut slash = 6;
    while slash < bytes.len() && bytes[slash] != b'/' {
        slash += 1;
    }
    if slash == bytes.len() {
        panic!("dlc ResPath needs body");
    }
    if slash == 6 {
        panic!("dlc ResPath needs name");
    }
    let name_len = slash - 6;
    if (name_len == 1 && bytes[6] == b'.')
        || (name_len == 2 && bytes[6] == b'.' && bytes[7] == b'.')
    {
        panic!("dlc ResPath name cannot be . or ..");
    }
    let mut i = 6;
    while i < slash {
        let byte = bytes[i];
        if !(byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-' || byte == b'.') {
            panic!("dlc ResPath name has invalid character");
        }
        i += 1;
    }
    validate_body_const(bytes, slash + 1);
}

const fn validate_body_const(bytes: &[u8], start: usize) {
    if start >= bytes.len() {
        panic!("ResPath body is empty");
    }
    let mut segment_start = start;
    let mut i = start;
    while i <= bytes.len() {
        if i == bytes.len() || bytes[i] == b'/' {
            let len = i - segment_start;
            if len == 1 && bytes[segment_start] == b'.' {
                panic!("ResPath cannot contain . segment");
            }
            if len == 2 && bytes[segment_start] == b'.' && bytes[segment_start + 1] == b'.' {
                panic!("ResPath cannot contain .. segment");
            }
            segment_start = i + 1;
        }
        i += 1;
    }
}

const fn starts_with(bytes: &[u8], prefix: &[u8]) -> bool {
    if bytes.len() < prefix.len() {
        return false;
    }
    let mut i = 0;
    while i < prefix.len() {
        if bytes[i] != prefix[i] {
            return false;
        }
        i += 1;
    }
    true
}

const fn contains_scheme(bytes: &[u8]) -> bool {
    let mut i = 0;
    while i + 2 < bytes.len() {
        if bytes[i] == b':' && bytes[i + 1] == b'/' && bytes[i + 2] == b'/' {
            return true;
        }
        i += 1;
    }
    false
}

#[macro_export]
macro_rules! res_path {
    ($path:literal) => {
        $crate::ResPath::new($path)
    };
    ($path:expr) => {
        compile_error!(
            "res_path! requires a string literal; use ResPath::try_new for dynamic paths"
        )
    };
}

#[macro_export]
macro_rules! res_path_buf {
    ($path:literal) => {
        $crate::ResPathBuf::new($path)
    };
    ($path:expr) => {
        compile_error!(
            "res_path_buf! requires a string literal; use ResPathBuf::try_new for dynamic paths"
        )
    };
}

fn validate(path: &str) -> Result<(), ResPathError> {
    if path.is_empty() {
        return Err(ResPathError::Empty);
    }
    if path.bytes().any(|byte| byte < 0x20 || byte == 0x7f) {
        return Err(ResPathError::ControlCharacter);
    }
    if path.contains('\\') {
        return Err(ResPathError::InvalidSeparator);
    }
    if let Some(path) = path.strip_prefix("res://") {
        validate_body(path)
    } else if let Some(path) = path.strip_prefix("user://") {
        validate_body(path)
    } else if let Some(path) = path.strip_prefix("dlc://") {
        validate_dlc(path)
    } else if path.contains("://") {
        Err(ResPathError::UnknownScheme)
    } else {
        Err(ResPathError::MissingScheme)
    }
}

fn validate_dlc(rest: &str) -> Result<(), ResPathError> {
    let (name, body) = rest.split_once('/').ok_or(ResPathError::EmptyPath)?;
    if name.is_empty() {
        return Err(ResPathError::EmptyDlcName);
    }
    if matches!(name, "." | "..") {
        return Err(ResPathError::InvalidDlcName);
    }
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(ResPathError::InvalidDlcName);
    }
    validate_body(body)
}

fn validate_body(body: &str) -> Result<(), ResPathError> {
    if body.is_empty() {
        return Err(ResPathError::EmptyPath);
    }
    if body.split('/').any(|segment| matches!(segment, "." | "..")) {
        return Err(ResPathError::Traversal);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_supported_schemes() {
        assert_eq!(
            ResPath::new("res://textures/player.png").kind(),
            ResPathKind::Res
        );
        let dlc = ResPath::new("dlc://winter_pack/levels/one.scn");
        assert_eq!(dlc.kind(), ResPathKind::Dlc);
        assert_eq!(dlc.dlc_name(), Some("winter_pack"));
        assert_eq!(dlc.body(), "levels/one.scn");
        assert_eq!(
            ResPath::new("dlc://self/player.png").dlc_name(),
            Some("self")
        );
        assert_eq!(
            ResPath::new("user://saves/slot1.scn").kind(),
            ResPathKind::User
        );
    }

    #[test]
    fn rejects_invalid_paths() {
        assert_eq!(
            ResPath::try_new("textures/player.png").unwrap_err(),
            ResPathError::MissingScheme
        );
        assert_eq!(
            ResPath::try_new("http://site/file").unwrap_err(),
            ResPathError::UnknownScheme
        );
        assert_eq!(
            ResPath::try_new("res://").unwrap_err(),
            ResPathError::EmptyPath
        );
        assert_eq!(
            ResPath::try_new("dlc://bad name/file").unwrap_err(),
            ResPathError::InvalidDlcName
        );
        assert_eq!(
            ResPath::try_new("res://../secret").unwrap_err(),
            ResPathError::Traversal
        );
        assert_eq!(
            ResPath::try_new("user://save\\slot").unwrap_err(),
            ResPathError::InvalidSeparator
        );
    }

    #[test]
    fn dlc_dot_names_match_const_and_runtime_grammar() {
        for name in [".", ".."] {
            let path = format!("dlc://{name}/file.txt");
            assert_eq!(
                ResPath::try_new(&path).unwrap_err(),
                ResPathError::InvalidDlcName
            );
            assert!(std::panic::catch_unwind(|| validate_const(&path)).is_err());
        }

        for path in ["dlc://v1.2/file.txt", "dlc://a.b/file.txt"] {
            assert!(ResPath::try_new(path).is_ok());
            validate_const(path);
        }
    }

    #[test]
    fn dlc_name_separator_and_control_errors_stay_distinct() {
        assert_eq!(
            ResPath::try_new("dlc://bad\\name/file.txt").unwrap_err(),
            ResPathError::InvalidSeparator
        );
        assert_eq!(
            ResPath::try_new("dlc://bad\nname/file.txt").unwrap_err(),
            ResPathError::ControlCharacter
        );
    }

    #[test]
    fn owned_path_derefs_to_borrowed_path() {
        let owned = ResPathBuf::try_new(String::from("res://audio/theme.ogg")).unwrap();
        let borrowed: &ResPath = &owned;
        assert_eq!(borrowed.as_str(), "res://audio/theme.ogg");

        let promoted = borrowed.to_buf();
        assert_eq!(promoted.as_str(), "res://audio/theme.ogg");
    }

    #[test]
    fn res_path_source_accepts_owned_and_borrowed_inputs() {
        fn path_str<P: ResPathSource>(path: P) -> String {
            path.as_res_path_str().to_string()
        }

        let owned = ResPathBuf::new("res://audio/theme.ogg");
        let dynamic = String::from("user://saves/slot1.scn");
        let cow = Cow::Borrowed("dlc://self/a.txt");

        assert_eq!(
            path_str(ResPath::new("res://textures/player.png")),
            "res://textures/player.png"
        );
        assert_eq!(path_str(&owned), "res://audio/theme.ogg");
        assert_eq!(path_str(dynamic), "user://saves/slot1.scn");
        assert_eq!(path_str(cow), "dlc://self/a.txt");
    }

    #[test]
    fn variant_round_trip_uses_string_storage() {
        let path = ResPathBuf::new("user://saves/slot1.scn");
        let variant = path.clone().into_variant();
        assert_eq!(variant.as_str(), Some("user://saves/slot1.scn"));
        assert_eq!(ResPathBuf::from_variant(&variant), Some(path));
        assert!(ResPathBuf::from_variant(&Variant::from("plain/path")).is_none());
    }

    #[test]
    fn static_res_path_round_trip_uses_interned_string() {
        let path = ResPath::new("res://textures/player.png");
        let variant = path.into_variant();
        let parsed = <&'static ResPath>::from_variant(&variant).unwrap();
        assert_eq!(parsed.as_str(), "res://textures/player.png");
        assert_eq!(
            <&'static ResPath>::from_variant(&variant).unwrap().as_str(),
            parsed.as_str()
        );
    }

    #[test]
    fn macros_create_borrowed_and_owned_paths() {
        let borrowed = crate::res_path!("res://textures/player.png");
        let owned = crate::res_path_buf!("dlc://self/textures/player.png");
        assert_eq!(borrowed.as_str(), "res://textures/player.png");
        assert_eq!(owned.as_str(), "dlc://self/textures/player.png");
    }
}
