#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Locale {
    /// Chinese (中文)
    ZH,
    /// English (English)
    EN,
    /// Russian (Русский)
    RU,
    /// Spanish (Español)
    ES,
    /// Portuguese (Português)
    PT,
    /// German (Deutsch)
    DE,
    /// Japanese (日本語)
    JA,
    /// French (Français)
    FR,
    /// Korean (한국어)
    KO,
    /// Polish (Polski)
    PL,
    /// Turkish (Türkçe)
    TR,
    /// Italian (Italiano)
    IT,
    /// Dutch (Nederlands)
    NL,
    /// Vietnamese (Tiếng Việt)
    VI,
    /// Indonesian (Bahasa Indonesia)
    ID,
    /// Arabic (العربية)
    AR,
    /// Hindi (हिन्दी)
    HI,
    /// Bengali (বাংলা)
    BN,
    /// Urdu (اردو)
    UR,
    /// Persian (فارسی)
    FA,
    /// Swahili (Kiswahili)
    SW,
    /// Custom locale code, for example `Locale::Custom("pt-br")`
    Custom(&'static str),
}

impl Locale {
    pub const fn code(self) -> &'static str {
        match self {
            Self::ZH => "zh",
            Self::EN => "en",
            Self::RU => "ru",
            Self::ES => "es",
            Self::PT => "pt",
            Self::DE => "de",
            Self::JA => "ja",
            Self::FR => "fr",
            Self::KO => "ko",
            Self::PL => "pl",
            Self::TR => "tr",
            Self::IT => "it",
            Self::NL => "nl",
            Self::VI => "vi",
            Self::ID => "id",
            Self::AR => "ar",
            Self::HI => "hi",
            Self::BN => "bn",
            Self::UR => "ur",
            Self::FA => "fa",
            Self::SW => "sw",
            Self::Custom(code) => code,
        }
    }
}

pub trait LocalizationAPI {
    fn localization_set_locale(&self, locale: Locale) -> bool;
    fn localization_get_locale(&self) -> Locale;
    fn localization_get(&self, key: &str) -> Option<&'static str>;
    fn localization_get_by_hash(&self, key_hash: u64) -> Option<&'static str>;
    fn localization_get_for_locale(&self, locale: Locale, key: &str) -> Option<&'static str>;
    fn localization_get_for_locale_by_hash(&self, locale: Locale, key_hash: u64)
        -> Option<&'static str>;
}

pub struct LocalizationModule<'res, R: LocalizationAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: LocalizationAPI + ?Sized> LocalizationModule<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    #[inline]
    pub fn set_locale(&self, locale: Locale) -> bool {
        self.api.localization_set_locale(locale)
    }

    #[inline]
    pub fn locale(&self) -> Locale {
        self.api.localization_get_locale()
    }

    #[inline]
    pub fn get<S: AsRef<str>>(&self, key: S) -> Option<&'static str> {
        self.api.localization_get(key.as_ref())
    }

    #[inline]
    pub fn get_by_hash(&self, key_hash: u64) -> Option<&'static str> {
        self.api.localization_get_by_hash(key_hash)
    }

    #[inline]
    pub fn get_for_locale<S: AsRef<str>>(&self, locale: Locale, key: S) -> Option<&'static str> {
        self.api.localization_get_for_locale(locale, key.as_ref())
    }

    #[inline]
    pub fn get_for_locale_by_hash(&self, locale: Locale, key_hash: u64) -> Option<&'static str> {
        self.api.localization_get_for_locale_by_hash(locale, key_hash)
    }
}

#[macro_export]
macro_rules! locale_set {
    ($res:expr, $locale:expr) => {
        $res.Localization().set_locale($locale)
    };
}

#[macro_export]
macro_rules! locale_get_current {
    ($res:expr) => {
        $res.Localization().locale()
    };
}

#[macro_export]
macro_rules! locale {
    ($res:expr, $key:literal) => {{
        const __KEY_HASH: u64 = $crate::__perro_string_to_u64($key);
        $res.Localization()
            .get_by_hash(__KEY_HASH)
            .unwrap_or($key)
    }};
    ($res:expr, $key:expr) => {{
        let __key = $key;
        $res.Localization().get(&__key).unwrap_or(__key.as_ref())
    }};
}

#[macro_export]
macro_rules! locale_in {
    ($res:expr, $locale:expr, $key:literal) => {{
        const __KEY_HASH: u64 = $crate::__perro_string_to_u64($key);
        $res.Localization()
            .get_for_locale_by_hash($locale, __KEY_HASH)
            .unwrap_or($key)
    }};
    ($res:expr, $locale:expr, $key:expr) => {{
        let __key = $key;
        $res.Localization()
            .get_for_locale($locale, &__key)
            .unwrap_or(__key.as_ref())
    }};
}
