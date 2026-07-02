//! Localization resource API.
//!
//! Selects active locale and resolves localized strings.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Locale {
    AA,
    AB,
    AE,
    AF,
    AK,
    AM,
    AN,
    AR,
    AS,
    AV,
    AY,
    AZ,
    BA,
    BE,
    BG,
    BI,
    BM,
    BN,
    BO,
    BR,
    BS,
    CA,
    CE,
    CH,
    CO,
    CR,
    CS,
    CU,
    CV,
    CY,
    DA,
    DE,
    DV,
    DZ,
    EE,
    EL,
    EN,
    EO,
    ES,
    ET,
    EU,
    FA,
    FF,
    FI,
    FJ,
    FO,
    FR,
    FY,
    GA,
    GD,
    GL,
    GN,
    GU,
    GV,
    HA,
    HE,
    HI,
    HO,
    HR,
    HT,
    HU,
    HY,
    HZ,
    IA,
    ID,
    IE,
    IG,
    II,
    IK,
    IO,
    IS,
    IT,
    IU,
    JA,
    JV,
    KA,
    KG,
    KI,
    KJ,
    KK,
    KL,
    KM,
    KN,
    KO,
    KR,
    KS,
    KU,
    KV,
    KW,
    KY,
    LA,
    LB,
    LG,
    LI,
    LN,
    LO,
    LT,
    LU,
    LV,
    MG,
    MH,
    MI,
    MK,
    ML,
    MN,
    MR,
    MS,
    MT,
    MY,
    NA,
    NB,
    ND,
    NE,
    NG,
    NL,
    NN,
    NO,
    NR,
    NV,
    NY,
    OC,
    OJ,
    OM,
    OR,
    OS,
    PA,
    PI,
    PL,
    PS,
    PT,
    QU,
    RM,
    RN,
    RO,
    RU,
    RW,
    SA,
    SC,
    SD,
    SE,
    SG,
    SI,
    SK,
    SL,
    SM,
    SN,
    SO,
    SQ,
    SR,
    SS,
    ST,
    SU,
    SV,
    SW,
    TA,
    TE,
    TG,
    TH,
    TI,
    TK,
    TL,
    TN,
    TO,
    TR,
    TS,
    TT,
    TW,
    TY,
    UG,
    UK,
    UR,
    UZ,
    VE,
    VI,
    VO,
    WA,
    WO,
    XH,
    YI,
    YO,
    ZA,
    ZH,
    ZU,
    /// Custom locale code, for example `Locale::Custom("pt-br")`.
    Custom(&'static str),
}

impl Locale {
    pub const fn code(self) -> &'static str {
        match self {
            Self::AA => "aa",
            Self::AB => "ab",
            Self::AE => "ae",
            Self::AF => "af",
            Self::AK => "ak",
            Self::AM => "am",
            Self::AN => "an",
            Self::AR => "ar",
            Self::AS => "as",
            Self::AV => "av",
            Self::AY => "ay",
            Self::AZ => "az",
            Self::BA => "ba",
            Self::BE => "be",
            Self::BG => "bg",
            Self::BI => "bi",
            Self::BM => "bm",
            Self::BN => "bn",
            Self::BO => "bo",
            Self::BR => "br",
            Self::BS => "bs",
            Self::CA => "ca",
            Self::CE => "ce",
            Self::CH => "ch",
            Self::CO => "co",
            Self::CR => "cr",
            Self::CS => "cs",
            Self::CU => "cu",
            Self::CV => "cv",
            Self::CY => "cy",
            Self::DA => "da",
            Self::DE => "de",
            Self::DV => "dv",
            Self::DZ => "dz",
            Self::EE => "ee",
            Self::EL => "el",
            Self::EN => "en",
            Self::EO => "eo",
            Self::ES => "es",
            Self::ET => "et",
            Self::EU => "eu",
            Self::FA => "fa",
            Self::FF => "ff",
            Self::FI => "fi",
            Self::FJ => "fj",
            Self::FO => "fo",
            Self::FR => "fr",
            Self::FY => "fy",
            Self::GA => "ga",
            Self::GD => "gd",
            Self::GL => "gl",
            Self::GN => "gn",
            Self::GU => "gu",
            Self::GV => "gv",
            Self::HA => "ha",
            Self::HE => "he",
            Self::HI => "hi",
            Self::HO => "ho",
            Self::HR => "hr",
            Self::HT => "ht",
            Self::HU => "hu",
            Self::HY => "hy",
            Self::HZ => "hz",
            Self::IA => "ia",
            Self::ID => "id",
            Self::IE => "ie",
            Self::IG => "ig",
            Self::II => "ii",
            Self::IK => "ik",
            Self::IO => "io",
            Self::IS => "is",
            Self::IT => "it",
            Self::IU => "iu",
            Self::JA => "ja",
            Self::JV => "jv",
            Self::KA => "ka",
            Self::KG => "kg",
            Self::KI => "ki",
            Self::KJ => "kj",
            Self::KK => "kk",
            Self::KL => "kl",
            Self::KM => "km",
            Self::KN => "kn",
            Self::KO => "ko",
            Self::KR => "kr",
            Self::KS => "ks",
            Self::KU => "ku",
            Self::KV => "kv",
            Self::KW => "kw",
            Self::KY => "ky",
            Self::LA => "la",
            Self::LB => "lb",
            Self::LG => "lg",
            Self::LI => "li",
            Self::LN => "ln",
            Self::LO => "lo",
            Self::LT => "lt",
            Self::LU => "lu",
            Self::LV => "lv",
            Self::MG => "mg",
            Self::MH => "mh",
            Self::MI => "mi",
            Self::MK => "mk",
            Self::ML => "ml",
            Self::MN => "mn",
            Self::MR => "mr",
            Self::MS => "ms",
            Self::MT => "mt",
            Self::MY => "my",
            Self::NA => "na",
            Self::NB => "nb",
            Self::ND => "nd",
            Self::NE => "ne",
            Self::NG => "ng",
            Self::NL => "nl",
            Self::NN => "nn",
            Self::NO => "no",
            Self::NR => "nr",
            Self::NV => "nv",
            Self::NY => "ny",
            Self::OC => "oc",
            Self::OJ => "oj",
            Self::OM => "om",
            Self::OR => "or",
            Self::OS => "os",
            Self::PA => "pa",
            Self::PI => "pi",
            Self::PL => "pl",
            Self::PS => "ps",
            Self::PT => "pt",
            Self::QU => "qu",
            Self::RM => "rm",
            Self::RN => "rn",
            Self::RO => "ro",
            Self::RU => "ru",
            Self::RW => "rw",
            Self::SA => "sa",
            Self::SC => "sc",
            Self::SD => "sd",
            Self::SE => "se",
            Self::SG => "sg",
            Self::SI => "si",
            Self::SK => "sk",
            Self::SL => "sl",
            Self::SM => "sm",
            Self::SN => "sn",
            Self::SO => "so",
            Self::SQ => "sq",
            Self::SR => "sr",
            Self::SS => "ss",
            Self::ST => "st",
            Self::SU => "su",
            Self::SV => "sv",
            Self::SW => "sw",
            Self::TA => "ta",
            Self::TE => "te",
            Self::TG => "tg",
            Self::TH => "th",
            Self::TI => "ti",
            Self::TK => "tk",
            Self::TL => "tl",
            Self::TN => "tn",
            Self::TO => "to",
            Self::TR => "tr",
            Self::TS => "ts",
            Self::TT => "tt",
            Self::TW => "tw",
            Self::TY => "ty",
            Self::UG => "ug",
            Self::UK => "uk",
            Self::UR => "ur",
            Self::UZ => "uz",
            Self::VE => "ve",
            Self::VI => "vi",
            Self::VO => "vo",
            Self::WA => "wa",
            Self::WO => "wo",
            Self::XH => "xh",
            Self::YI => "yi",
            Self::YO => "yo",
            Self::ZA => "za",
            Self::ZH => "zh",
            Self::ZU => "zu",
            Self::Custom(code) => code,
        }
    }

    pub fn from_code(code: &'static str) -> Self {
        match code {
            "aa" => Self::AA,
            "ab" => Self::AB,
            "ae" => Self::AE,
            "af" => Self::AF,
            "ak" => Self::AK,
            "am" => Self::AM,
            "an" => Self::AN,
            "ar" => Self::AR,
            "as" => Self::AS,
            "av" => Self::AV,
            "ay" => Self::AY,
            "az" => Self::AZ,
            "ba" => Self::BA,
            "be" => Self::BE,
            "bg" => Self::BG,
            "bi" => Self::BI,
            "bm" => Self::BM,
            "bn" => Self::BN,
            "bo" => Self::BO,
            "br" => Self::BR,
            "bs" => Self::BS,
            "ca" => Self::CA,
            "ce" => Self::CE,
            "ch" => Self::CH,
            "co" => Self::CO,
            "cr" => Self::CR,
            "cs" => Self::CS,
            "cu" => Self::CU,
            "cv" => Self::CV,
            "cy" => Self::CY,
            "da" => Self::DA,
            "de" => Self::DE,
            "dv" => Self::DV,
            "dz" => Self::DZ,
            "ee" => Self::EE,
            "el" => Self::EL,
            "en" => Self::EN,
            "eo" => Self::EO,
            "es" => Self::ES,
            "et" => Self::ET,
            "eu" => Self::EU,
            "fa" => Self::FA,
            "ff" => Self::FF,
            "fi" => Self::FI,
            "fj" => Self::FJ,
            "fo" => Self::FO,
            "fr" => Self::FR,
            "fy" => Self::FY,
            "ga" => Self::GA,
            "gd" => Self::GD,
            "gl" => Self::GL,
            "gn" => Self::GN,
            "gu" => Self::GU,
            "gv" => Self::GV,
            "ha" => Self::HA,
            "he" => Self::HE,
            "hi" => Self::HI,
            "ho" => Self::HO,
            "hr" => Self::HR,
            "ht" => Self::HT,
            "hu" => Self::HU,
            "hy" => Self::HY,
            "hz" => Self::HZ,
            "ia" => Self::IA,
            "id" => Self::ID,
            "ie" => Self::IE,
            "ig" => Self::IG,
            "ii" => Self::II,
            "ik" => Self::IK,
            "io" => Self::IO,
            "is" => Self::IS,
            "it" => Self::IT,
            "iu" => Self::IU,
            "ja" => Self::JA,
            "jv" => Self::JV,
            "ka" => Self::KA,
            "kg" => Self::KG,
            "ki" => Self::KI,
            "kj" => Self::KJ,
            "kk" => Self::KK,
            "kl" => Self::KL,
            "km" => Self::KM,
            "kn" => Self::KN,
            "ko" => Self::KO,
            "kr" => Self::KR,
            "ks" => Self::KS,
            "ku" => Self::KU,
            "kv" => Self::KV,
            "kw" => Self::KW,
            "ky" => Self::KY,
            "la" => Self::LA,
            "lb" => Self::LB,
            "lg" => Self::LG,
            "li" => Self::LI,
            "ln" => Self::LN,
            "lo" => Self::LO,
            "lt" => Self::LT,
            "lu" => Self::LU,
            "lv" => Self::LV,
            "mg" => Self::MG,
            "mh" => Self::MH,
            "mi" => Self::MI,
            "mk" => Self::MK,
            "ml" => Self::ML,
            "mn" => Self::MN,
            "mr" => Self::MR,
            "ms" => Self::MS,
            "mt" => Self::MT,
            "my" => Self::MY,
            "na" => Self::NA,
            "nb" => Self::NB,
            "nd" => Self::ND,
            "ne" => Self::NE,
            "ng" => Self::NG,
            "nl" => Self::NL,
            "nn" => Self::NN,
            "no" => Self::NO,
            "nr" => Self::NR,
            "nv" => Self::NV,
            "ny" => Self::NY,
            "oc" => Self::OC,
            "oj" => Self::OJ,
            "om" => Self::OM,
            "or" => Self::OR,
            "os" => Self::OS,
            "pa" => Self::PA,
            "pi" => Self::PI,
            "pl" => Self::PL,
            "ps" => Self::PS,
            "pt" => Self::PT,
            "qu" => Self::QU,
            "rm" => Self::RM,
            "rn" => Self::RN,
            "ro" => Self::RO,
            "ru" => Self::RU,
            "rw" => Self::RW,
            "sa" => Self::SA,
            "sc" => Self::SC,
            "sd" => Self::SD,
            "se" => Self::SE,
            "sg" => Self::SG,
            "si" => Self::SI,
            "sk" => Self::SK,
            "sl" => Self::SL,
            "sm" => Self::SM,
            "sn" => Self::SN,
            "so" => Self::SO,
            "sq" => Self::SQ,
            "sr" => Self::SR,
            "ss" => Self::SS,
            "st" => Self::ST,
            "su" => Self::SU,
            "sv" => Self::SV,
            "sw" => Self::SW,
            "ta" => Self::TA,
            "te" => Self::TE,
            "tg" => Self::TG,
            "th" => Self::TH,
            "ti" => Self::TI,
            "tk" => Self::TK,
            "tl" => Self::TL,
            "tn" => Self::TN,
            "to" => Self::TO,
            "tr" => Self::TR,
            "ts" => Self::TS,
            "tt" => Self::TT,
            "tw" => Self::TW,
            "ty" => Self::TY,
            "ug" => Self::UG,
            "uk" => Self::UK,
            "ur" => Self::UR,
            "uz" => Self::UZ,
            "ve" => Self::VE,
            "vi" => Self::VI,
            "vo" => Self::VO,
            "wa" => Self::WA,
            "wo" => Self::WO,
            "xh" => Self::XH,
            "yi" => Self::YI,
            "yo" => Self::YO,
            "za" => Self::ZA,
            "zh" => Self::ZH,
            "zu" => Self::ZU,
            _ => Self::Custom(code),
        }
    }
}

pub trait IntoLocale {
    fn into_locale(self) -> Locale;
}

impl IntoLocale for Locale {
    #[inline]
    fn into_locale(self) -> Locale {
        self
    }
}

impl IntoLocale for &'static str {
    #[inline]
    fn into_locale(self) -> Locale {
        Locale::from_code(self)
    }
}

pub trait LocalizationAPI {
    fn localization_set_locale(&self, locale: Locale) -> bool;
    fn localization_get_locale(&self) -> Locale;
    fn localization_get(&self, key: &str) -> Option<&'static str>;
    fn localization_get_by_hash(&self, key_hash: u64) -> Option<&'static str>;
    fn localization_get_for_locale(&self, locale: Locale, key: &str) -> Option<&'static str>;
    fn localization_get_for_locale_by_hash(
        &self,
        locale: Locale,
        key_hash: u64,
    ) -> Option<&'static str>;
}

pub struct LocalizationModule<'res, R: LocalizationAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: LocalizationAPI + ?Sized> LocalizationModule<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    #[inline]
    pub fn set_locale<L: IntoLocale>(&self, locale: L) -> bool {
        self.api.localization_set_locale(locale.into_locale())
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
        self.api
            .localization_get_for_locale_by_hash(locale, key_hash)
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! __perro_locale_from_literal {
    ("aa") => {
        $crate::sub_apis::Locale::AA
    };
    ("ab") => {
        $crate::sub_apis::Locale::AB
    };
    ("ae") => {
        $crate::sub_apis::Locale::AE
    };
    ("af") => {
        $crate::sub_apis::Locale::AF
    };
    ("ak") => {
        $crate::sub_apis::Locale::AK
    };
    ("am") => {
        $crate::sub_apis::Locale::AM
    };
    ("an") => {
        $crate::sub_apis::Locale::AN
    };
    ("ar") => {
        $crate::sub_apis::Locale::AR
    };
    ("as") => {
        $crate::sub_apis::Locale::AS
    };
    ("av") => {
        $crate::sub_apis::Locale::AV
    };
    ("ay") => {
        $crate::sub_apis::Locale::AY
    };
    ("az") => {
        $crate::sub_apis::Locale::AZ
    };
    ("ba") => {
        $crate::sub_apis::Locale::BA
    };
    ("be") => {
        $crate::sub_apis::Locale::BE
    };
    ("bg") => {
        $crate::sub_apis::Locale::BG
    };
    ("bi") => {
        $crate::sub_apis::Locale::BI
    };
    ("bm") => {
        $crate::sub_apis::Locale::BM
    };
    ("bn") => {
        $crate::sub_apis::Locale::BN
    };
    ("bo") => {
        $crate::sub_apis::Locale::BO
    };
    ("br") => {
        $crate::sub_apis::Locale::BR
    };
    ("bs") => {
        $crate::sub_apis::Locale::BS
    };
    ("ca") => {
        $crate::sub_apis::Locale::CA
    };
    ("ce") => {
        $crate::sub_apis::Locale::CE
    };
    ("ch") => {
        $crate::sub_apis::Locale::CH
    };
    ("co") => {
        $crate::sub_apis::Locale::CO
    };
    ("cr") => {
        $crate::sub_apis::Locale::CR
    };
    ("cs") => {
        $crate::sub_apis::Locale::CS
    };
    ("cu") => {
        $crate::sub_apis::Locale::CU
    };
    ("cv") => {
        $crate::sub_apis::Locale::CV
    };
    ("cy") => {
        $crate::sub_apis::Locale::CY
    };
    ("da") => {
        $crate::sub_apis::Locale::DA
    };
    ("de") => {
        $crate::sub_apis::Locale::DE
    };
    ("dv") => {
        $crate::sub_apis::Locale::DV
    };
    ("dz") => {
        $crate::sub_apis::Locale::DZ
    };
    ("ee") => {
        $crate::sub_apis::Locale::EE
    };
    ("el") => {
        $crate::sub_apis::Locale::EL
    };
    ("en") => {
        $crate::sub_apis::Locale::EN
    };
    ("eo") => {
        $crate::sub_apis::Locale::EO
    };
    ("es") => {
        $crate::sub_apis::Locale::ES
    };
    ("et") => {
        $crate::sub_apis::Locale::ET
    };
    ("eu") => {
        $crate::sub_apis::Locale::EU
    };
    ("fa") => {
        $crate::sub_apis::Locale::FA
    };
    ("ff") => {
        $crate::sub_apis::Locale::FF
    };
    ("fi") => {
        $crate::sub_apis::Locale::FI
    };
    ("fj") => {
        $crate::sub_apis::Locale::FJ
    };
    ("fo") => {
        $crate::sub_apis::Locale::FO
    };
    ("fr") => {
        $crate::sub_apis::Locale::FR
    };
    ("fy") => {
        $crate::sub_apis::Locale::FY
    };
    ("ga") => {
        $crate::sub_apis::Locale::GA
    };
    ("gd") => {
        $crate::sub_apis::Locale::GD
    };
    ("gl") => {
        $crate::sub_apis::Locale::GL
    };
    ("gn") => {
        $crate::sub_apis::Locale::GN
    };
    ("gu") => {
        $crate::sub_apis::Locale::GU
    };
    ("gv") => {
        $crate::sub_apis::Locale::GV
    };
    ("ha") => {
        $crate::sub_apis::Locale::HA
    };
    ("he") => {
        $crate::sub_apis::Locale::HE
    };
    ("hi") => {
        $crate::sub_apis::Locale::HI
    };
    ("ho") => {
        $crate::sub_apis::Locale::HO
    };
    ("hr") => {
        $crate::sub_apis::Locale::HR
    };
    ("ht") => {
        $crate::sub_apis::Locale::HT
    };
    ("hu") => {
        $crate::sub_apis::Locale::HU
    };
    ("hy") => {
        $crate::sub_apis::Locale::HY
    };
    ("hz") => {
        $crate::sub_apis::Locale::HZ
    };
    ("ia") => {
        $crate::sub_apis::Locale::IA
    };
    ("id") => {
        $crate::sub_apis::Locale::ID
    };
    ("ie") => {
        $crate::sub_apis::Locale::IE
    };
    ("ig") => {
        $crate::sub_apis::Locale::IG
    };
    ("ii") => {
        $crate::sub_apis::Locale::II
    };
    ("ik") => {
        $crate::sub_apis::Locale::IK
    };
    ("io") => {
        $crate::sub_apis::Locale::IO
    };
    ("is") => {
        $crate::sub_apis::Locale::IS
    };
    ("it") => {
        $crate::sub_apis::Locale::IT
    };
    ("iu") => {
        $crate::sub_apis::Locale::IU
    };
    ("ja") => {
        $crate::sub_apis::Locale::JA
    };
    ("jv") => {
        $crate::sub_apis::Locale::JV
    };
    ("ka") => {
        $crate::sub_apis::Locale::KA
    };
    ("kg") => {
        $crate::sub_apis::Locale::KG
    };
    ("ki") => {
        $crate::sub_apis::Locale::KI
    };
    ("kj") => {
        $crate::sub_apis::Locale::KJ
    };
    ("kk") => {
        $crate::sub_apis::Locale::KK
    };
    ("kl") => {
        $crate::sub_apis::Locale::KL
    };
    ("km") => {
        $crate::sub_apis::Locale::KM
    };
    ("kn") => {
        $crate::sub_apis::Locale::KN
    };
    ("ko") => {
        $crate::sub_apis::Locale::KO
    };
    ("kr") => {
        $crate::sub_apis::Locale::KR
    };
    ("ks") => {
        $crate::sub_apis::Locale::KS
    };
    ("ku") => {
        $crate::sub_apis::Locale::KU
    };
    ("kv") => {
        $crate::sub_apis::Locale::KV
    };
    ("kw") => {
        $crate::sub_apis::Locale::KW
    };
    ("ky") => {
        $crate::sub_apis::Locale::KY
    };
    ("la") => {
        $crate::sub_apis::Locale::LA
    };
    ("lb") => {
        $crate::sub_apis::Locale::LB
    };
    ("lg") => {
        $crate::sub_apis::Locale::LG
    };
    ("li") => {
        $crate::sub_apis::Locale::LI
    };
    ("ln") => {
        $crate::sub_apis::Locale::LN
    };
    ("lo") => {
        $crate::sub_apis::Locale::LO
    };
    ("lt") => {
        $crate::sub_apis::Locale::LT
    };
    ("lu") => {
        $crate::sub_apis::Locale::LU
    };
    ("lv") => {
        $crate::sub_apis::Locale::LV
    };
    ("mg") => {
        $crate::sub_apis::Locale::MG
    };
    ("mh") => {
        $crate::sub_apis::Locale::MH
    };
    ("mi") => {
        $crate::sub_apis::Locale::MI
    };
    ("mk") => {
        $crate::sub_apis::Locale::MK
    };
    ("ml") => {
        $crate::sub_apis::Locale::ML
    };
    ("mn") => {
        $crate::sub_apis::Locale::MN
    };
    ("mr") => {
        $crate::sub_apis::Locale::MR
    };
    ("ms") => {
        $crate::sub_apis::Locale::MS
    };
    ("mt") => {
        $crate::sub_apis::Locale::MT
    };
    ("my") => {
        $crate::sub_apis::Locale::MY
    };
    ("na") => {
        $crate::sub_apis::Locale::NA
    };
    ("nb") => {
        $crate::sub_apis::Locale::NB
    };
    ("nd") => {
        $crate::sub_apis::Locale::ND
    };
    ("ne") => {
        $crate::sub_apis::Locale::NE
    };
    ("ng") => {
        $crate::sub_apis::Locale::NG
    };
    ("nl") => {
        $crate::sub_apis::Locale::NL
    };
    ("nn") => {
        $crate::sub_apis::Locale::NN
    };
    ("no") => {
        $crate::sub_apis::Locale::NO
    };
    ("nr") => {
        $crate::sub_apis::Locale::NR
    };
    ("nv") => {
        $crate::sub_apis::Locale::NV
    };
    ("ny") => {
        $crate::sub_apis::Locale::NY
    };
    ("oc") => {
        $crate::sub_apis::Locale::OC
    };
    ("oj") => {
        $crate::sub_apis::Locale::OJ
    };
    ("om") => {
        $crate::sub_apis::Locale::OM
    };
    ("or") => {
        $crate::sub_apis::Locale::OR
    };
    ("os") => {
        $crate::sub_apis::Locale::OS
    };
    ("pa") => {
        $crate::sub_apis::Locale::PA
    };
    ("pi") => {
        $crate::sub_apis::Locale::PI
    };
    ("pl") => {
        $crate::sub_apis::Locale::PL
    };
    ("ps") => {
        $crate::sub_apis::Locale::PS
    };
    ("pt") => {
        $crate::sub_apis::Locale::PT
    };
    ("qu") => {
        $crate::sub_apis::Locale::QU
    };
    ("rm") => {
        $crate::sub_apis::Locale::RM
    };
    ("rn") => {
        $crate::sub_apis::Locale::RN
    };
    ("ro") => {
        $crate::sub_apis::Locale::RO
    };
    ("ru") => {
        $crate::sub_apis::Locale::RU
    };
    ("rw") => {
        $crate::sub_apis::Locale::RW
    };
    ("sa") => {
        $crate::sub_apis::Locale::SA
    };
    ("sc") => {
        $crate::sub_apis::Locale::SC
    };
    ("sd") => {
        $crate::sub_apis::Locale::SD
    };
    ("se") => {
        $crate::sub_apis::Locale::SE
    };
    ("sg") => {
        $crate::sub_apis::Locale::SG
    };
    ("si") => {
        $crate::sub_apis::Locale::SI
    };
    ("sk") => {
        $crate::sub_apis::Locale::SK
    };
    ("sl") => {
        $crate::sub_apis::Locale::SL
    };
    ("sm") => {
        $crate::sub_apis::Locale::SM
    };
    ("sn") => {
        $crate::sub_apis::Locale::SN
    };
    ("so") => {
        $crate::sub_apis::Locale::SO
    };
    ("sq") => {
        $crate::sub_apis::Locale::SQ
    };
    ("sr") => {
        $crate::sub_apis::Locale::SR
    };
    ("ss") => {
        $crate::sub_apis::Locale::SS
    };
    ("st") => {
        $crate::sub_apis::Locale::ST
    };
    ("su") => {
        $crate::sub_apis::Locale::SU
    };
    ("sv") => {
        $crate::sub_apis::Locale::SV
    };
    ("sw") => {
        $crate::sub_apis::Locale::SW
    };
    ("ta") => {
        $crate::sub_apis::Locale::TA
    };
    ("te") => {
        $crate::sub_apis::Locale::TE
    };
    ("tg") => {
        $crate::sub_apis::Locale::TG
    };
    ("th") => {
        $crate::sub_apis::Locale::TH
    };
    ("ti") => {
        $crate::sub_apis::Locale::TI
    };
    ("tk") => {
        $crate::sub_apis::Locale::TK
    };
    ("tl") => {
        $crate::sub_apis::Locale::TL
    };
    ("tn") => {
        $crate::sub_apis::Locale::TN
    };
    ("to") => {
        $crate::sub_apis::Locale::TO
    };
    ("tr") => {
        $crate::sub_apis::Locale::TR
    };
    ("ts") => {
        $crate::sub_apis::Locale::TS
    };
    ("tt") => {
        $crate::sub_apis::Locale::TT
    };
    ("tw") => {
        $crate::sub_apis::Locale::TW
    };
    ("ty") => {
        $crate::sub_apis::Locale::TY
    };
    ("ug") => {
        $crate::sub_apis::Locale::UG
    };
    ("uk") => {
        $crate::sub_apis::Locale::UK
    };
    ("ur") => {
        $crate::sub_apis::Locale::UR
    };
    ("uz") => {
        $crate::sub_apis::Locale::UZ
    };
    ("ve") => {
        $crate::sub_apis::Locale::VE
    };
    ("vi") => {
        $crate::sub_apis::Locale::VI
    };
    ("vo") => {
        $crate::sub_apis::Locale::VO
    };
    ("wa") => {
        $crate::sub_apis::Locale::WA
    };
    ("wo") => {
        $crate::sub_apis::Locale::WO
    };
    ("xh") => {
        $crate::sub_apis::Locale::XH
    };
    ("yi") => {
        $crate::sub_apis::Locale::YI
    };
    ("yo") => {
        $crate::sub_apis::Locale::YO
    };
    ("za") => {
        $crate::sub_apis::Locale::ZA
    };
    ("zh") => {
        $crate::sub_apis::Locale::ZH
    };
    ("zu") => {
        $crate::sub_apis::Locale::ZU
    };
    ($locale:literal) => {
        $crate::sub_apis::Locale::Custom($locale)
    };
}
#[macro_export]
macro_rules! locale_set {
    ($res:expr, $locale:literal) => {
        $res.Localization()
            .set_locale($crate::__perro_locale_from_literal!($locale))
    };
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
        $res.Localization().get_by_hash(__KEY_HASH).unwrap_or($key)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_literal_macro_maps_known_codes_to_variants() {
        assert_eq!(crate::__perro_locale_from_literal!("es"), Locale::ES);
        assert_eq!(crate::__perro_locale_from_literal!("ga"), Locale::GA);
    }

    #[test]
    fn locale_literal_macro_maps_unknown_codes_to_custom() {
        assert_eq!(
            crate::__perro_locale_from_literal!("pt-br"),
            Locale::Custom("pt-br")
        );
        assert_eq!(
            crate::__perro_locale_from_literal!("en-pirate"),
            Locale::Custom("en-pirate")
        );
    }

    #[test]
    fn into_locale_accepts_locale_or_static_str() {
        assert_eq!(Locale::ES.into_locale(), Locale::ES);
        assert_eq!("ga".into_locale(), Locale::GA);
        assert_eq!("pt-br".into_locale(), Locale::Custom("pt-br"));
    }
}
