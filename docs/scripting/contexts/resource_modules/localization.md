# Localization Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
| CSV Format | [CSV Format](#csv-format) |
| Locale Codes | [Locale Codes](#locale-codes) |
| Set Locale | [Set Locale](#set-locale) |
| API Reference | [API Reference](#api-reference) |
| `set_locale` | [`set_locale`](#set_locale) |
| `locale` | [`locale`](#locale) |
| `get` | [`get`](#get) |
| `get_by_hash` | [`get_by_hash`](#get_by_hash) |
| `get_for_locale` | [`get_for_locale`](#get_for_locale) |
| `get_for_locale_by_hash` | [`get_for_locale_by_hash`](#get_for_locale_by_hash) |
| `locale_set` | [`locale_set`](#locale_set) |
| `locale_get_current` | [`locale_get_current`](#locale_get_current) |
| `locale` | [`locale`](#locale) |
| `locale_in` | [`locale_in`](#locale_in) |

## Overview

This resource module belongs to `ctx.res` and documents localization calls.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res.Localization()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## CSV Format

Put `localization.csv`, `locale.csv`, or `translations.csv` next to `project.toml`.

First column must be `key`.

Other columns are locale codes.

```csv
key,en,es,pt-br
menu.start,Start,Iniciar,Comecar
menu.quit,Quit,Salir,Sair
```

- `key`: lookup id.
- Built-in language cols: ISO 639-1 codes like `en`, `es`, `ja`, `ga`.
- Region/script cols: custom tags like `pt-br`, `zh-hant`.
- Empty cell: falls back to `en`.
- Missing locale col: dynamic `set_locale` returns `false`.

## Locale Codes

Built-in locales use ISO 639-1 two-letter codes.

Use codes, not language names.

`Spanish` is not a locale code.

`es` is a locale code.

Built-in enum variants use upper case.

`es` => `Locale::ES`

`ga` => `Locale::GA`

### Built-in ISO table

| Code | ISO name | Locale |
| --- | --- | --- |
| `aa` | Afar | `Locale::AA` |
| `ab` | Abkhazian | `Locale::AB` |
| `ae` | Avestan | `Locale::AE` |
| `af` | Afrikaans | `Locale::AF` |
| `ak` | Akan | `Locale::AK` |
| `am` | Amharic | `Locale::AM` |
| `an` | Aragonese | `Locale::AN` |
| `ar` | Arabic | `Locale::AR` |
| `as` | Assamese | `Locale::AS` |
| `av` | Avaric | `Locale::AV` |
| `ay` | Aymara | `Locale::AY` |
| `az` | Azerbaijani | `Locale::AZ` |
| `ba` | Bashkir | `Locale::BA` |
| `be` | Belarusian | `Locale::BE` |
| `bg` | Bulgarian | `Locale::BG` |
| `bi` | Bislama | `Locale::BI` |
| `bm` | Bambara | `Locale::BM` |
| `bn` | Bengali | `Locale::BN` |
| `bo` | Tibetan | `Locale::BO` |
| `br` | Breton | `Locale::BR` |
| `bs` | Bosnian | `Locale::BS` |
| `ca` | Catalan; Valencian | `Locale::CA` |
| `ce` | Chechen | `Locale::CE` |
| `ch` | Chamorro | `Locale::CH` |
| `co` | Corsican | `Locale::CO` |
| `cr` | Cree | `Locale::CR` |
| `cs` | Czech | `Locale::CS` |
| `cu` | Church Slavic; Old Slavonic; Church Slavonic; Old Bulgarian; Old Church Slavonic | `Locale::CU` |
| `cv` | Chuvash | `Locale::CV` |
| `cy` | Welsh | `Locale::CY` |
| `da` | Danish | `Locale::DA` |
| `de` | German | `Locale::DE` |
| `dv` | Divehi; Dhivehi; Maldivian | `Locale::DV` |
| `dz` | Dzongkha | `Locale::DZ` |
| `ee` | Ewe | `Locale::EE` |
| `el` | Modern Greek (1453-) | `Locale::EL` |
| `en` | English | `Locale::EN` |
| `eo` | Esperanto | `Locale::EO` |
| `es` | Spanish; Castilian | `Locale::ES` |
| `et` | Estonian | `Locale::ET` |
| `eu` | Basque | `Locale::EU` |
| `fa` | Persian | `Locale::FA` |
| `ff` | Fulah | `Locale::FF` |
| `fi` | Finnish | `Locale::FI` |
| `fj` | Fijian | `Locale::FJ` |
| `fo` | Faroese | `Locale::FO` |
| `fr` | French | `Locale::FR` |
| `fy` | Western Frisian | `Locale::FY` |
| `ga` | Irish | `Locale::GA` |
| `gd` | Gaelic; Scottish Gaelic | `Locale::GD` |
| `gl` | Galician | `Locale::GL` |
| `gn` | Guarani | `Locale::GN` |
| `gu` | Gujarati | `Locale::GU` |
| `gv` | Manx | `Locale::GV` |
| `ha` | Hausa | `Locale::HA` |
| `he` | Hebrew | `Locale::HE` |
| `hi` | Hindi | `Locale::HI` |
| `ho` | Hiri Motu | `Locale::HO` |
| `hr` | Croatian | `Locale::HR` |
| `ht` | Haitian; Haitian Creole | `Locale::HT` |
| `hu` | Hungarian | `Locale::HU` |
| `hy` | Armenian | `Locale::HY` |
| `hz` | Herero | `Locale::HZ` |
| `ia` | Interlingua (International Auxiliary Language Association) | `Locale::IA` |
| `id` | Indonesian | `Locale::ID` |
| `ie` | Interlingue; Occidental | `Locale::IE` |
| `ig` | Igbo | `Locale::IG` |
| `ii` | Sichuan Yi; Nuosu | `Locale::II` |
| `ik` | Inupiaq | `Locale::IK` |
| `io` | Ido | `Locale::IO` |
| `is` | Icelandic | `Locale::IS` |
| `it` | Italian | `Locale::IT` |
| `iu` | Inuktitut | `Locale::IU` |
| `ja` | Japanese | `Locale::JA` |
| `jv` | Javanese | `Locale::JV` |
| `ka` | Georgian | `Locale::KA` |
| `kg` | Kongo | `Locale::KG` |
| `ki` | Kikuyu; Gikuyu | `Locale::KI` |
| `kj` | Kuanyama; Kwanyama | `Locale::KJ` |
| `kk` | Kazakh | `Locale::KK` |
| `kl` | Kalaallisut; Greenlandic | `Locale::KL` |
| `km` | Central Khmer | `Locale::KM` |
| `kn` | Kannada | `Locale::KN` |
| `ko` | Korean | `Locale::KO` |
| `kr` | Kanuri | `Locale::KR` |
| `ks` | Kashmiri | `Locale::KS` |
| `ku` | Kurdish | `Locale::KU` |
| `kv` | Komi | `Locale::KV` |
| `kw` | Cornish | `Locale::KW` |
| `ky` | Kirghiz; Kyrgyz | `Locale::KY` |
| `la` | Latin | `Locale::LA` |
| `lb` | Luxembourgish; Letzeburgesch | `Locale::LB` |
| `lg` | Ganda | `Locale::LG` |
| `li` | Limburgan; Limburger; Limburgish | `Locale::LI` |
| `ln` | Lingala | `Locale::LN` |
| `lo` | Lao | `Locale::LO` |
| `lt` | Lithuanian | `Locale::LT` |
| `lu` | Luba-Katanga | `Locale::LU` |
| `lv` | Latvian | `Locale::LV` |
| `mg` | Malagasy | `Locale::MG` |
| `mh` | Marshallese | `Locale::MH` |
| `mi` | Maori | `Locale::MI` |
| `mk` | Macedonian | `Locale::MK` |
| `ml` | Malayalam | `Locale::ML` |
| `mn` | Mongolian | `Locale::MN` |
| `mr` | Marathi | `Locale::MR` |
| `ms` | Malay | `Locale::MS` |
| `mt` | Maltese | `Locale::MT` |
| `my` | Burmese | `Locale::MY` |
| `na` | Nauru | `Locale::NA` |
| `nb` | Norwegian Bokmal | `Locale::NB` |
| `nd` | North Ndebele | `Locale::ND` |
| `ne` | Nepali | `Locale::NE` |
| `ng` | Ndonga | `Locale::NG` |
| `nl` | Dutch; Flemish | `Locale::NL` |
| `nn` | Norwegian Nynorsk | `Locale::NN` |
| `no` | Norwegian | `Locale::NO` |
| `nr` | South Ndebele | `Locale::NR` |
| `nv` | Navajo; Navaho | `Locale::NV` |
| `ny` | Chichewa; Chewa; Nyanja | `Locale::NY` |
| `oc` | Occitan (post 1500) | `Locale::OC` |
| `oj` | Ojibwa | `Locale::OJ` |
| `om` | Oromo | `Locale::OM` |
| `or` | Oriya | `Locale::OR` |
| `os` | Ossetian; Ossetic | `Locale::OS` |
| `pa` | Panjabi; Punjabi | `Locale::PA` |
| `pi` | Pali | `Locale::PI` |
| `pl` | Polish | `Locale::PL` |
| `ps` | Pushto; Pashto | `Locale::PS` |
| `pt` | Portuguese | `Locale::PT` |
| `qu` | Quechua | `Locale::QU` |
| `rm` | Romansh | `Locale::RM` |
| `rn` | Rundi | `Locale::RN` |
| `ro` | Romanian; Moldavian; Moldovan | `Locale::RO` |
| `ru` | Russian | `Locale::RU` |
| `rw` | Kinyarwanda | `Locale::RW` |
| `sa` | Sanskrit | `Locale::SA` |
| `sc` | Sardinian | `Locale::SC` |
| `sd` | Sindhi | `Locale::SD` |
| `se` | Northern Sami | `Locale::SE` |
| `sg` | Sango | `Locale::SG` |
| `si` | Sinhala; Sinhalese | `Locale::SI` |
| `sk` | Slovak | `Locale::SK` |
| `sl` | Slovenian | `Locale::SL` |
| `sm` | Samoan | `Locale::SM` |
| `sn` | Shona | `Locale::SN` |
| `so` | Somali | `Locale::SO` |
| `sq` | Albanian | `Locale::SQ` |
| `sr` | Serbian | `Locale::SR` |
| `ss` | Swati | `Locale::SS` |
| `st` | Sotho, Southern | `Locale::ST` |
| `su` | Sundanese | `Locale::SU` |
| `sv` | Swedish | `Locale::SV` |
| `sw` | Swahili | `Locale::SW` |
| `ta` | Tamil | `Locale::TA` |
| `te` | Telugu | `Locale::TE` |
| `tg` | Tajik | `Locale::TG` |
| `th` | Thai | `Locale::TH` |
| `ti` | Tigrinya | `Locale::TI` |
| `tk` | Turkmen | `Locale::TK` |
| `tl` | Tagalog | `Locale::TL` |
| `tn` | Tswana | `Locale::TN` |
| `to` | Tonga (Tonga Islands) | `Locale::TO` |
| `tr` | Turkish | `Locale::TR` |
| `ts` | Tsonga | `Locale::TS` |
| `tt` | Tatar | `Locale::TT` |
| `tw` | Twi | `Locale::TW` |
| `ty` | Tahitian | `Locale::TY` |
| `ug` | Uighur; Uyghur | `Locale::UG` |
| `uk` | Ukrainian | `Locale::UK` |
| `ur` | Urdu | `Locale::UR` |
| `uz` | Uzbek | `Locale::UZ` |
| `ve` | Venda | `Locale::VE` |
| `vi` | Vietnamese | `Locale::VI` |
| `vo` | Volapuk | `Locale::VO` |
| `wa` | Walloon | `Locale::WA` |
| `wo` | Wolof | `Locale::WO` |
| `xh` | Xhosa | `Locale::XH` |
| `yi` | Yiddish | `Locale::YI` |
| `yo` | Yoruba | `Locale::YO` |
| `za` | Zhuang; Chuang | `Locale::ZA` |
| `zh` | Chinese | `Locale::ZH` |
| `zu` | Zulu | `Locale::ZU` |

### Custom codes

Custom codes are manual tags for regions, scripts, dialects, or project-specific variants.

Use lower-case tags with no spaces.

CSV header and script value must match.

Good custom tags:

- `pt-br`
- `zh-hant`
- `en-pirate`

```csv
key,en,pt-br,en-pirate
menu.start,Start,Comecar,Set Sail
```

```rust
ctx.res.Localization().set_locale(Locale::Custom("pt-br"));
```

## Set Locale

```rust
ctx.res.Localization().set_locale(Locale::ES);
```

```rust
ctx.res.Localization().set_locale(Locale::from_code("ga"));
```

```rust
ctx.res.Localization().set_locale("ga");
```

```rust
ctx.res.Localization().set_locale(Locale::Custom("pt-br"));
```

```rust
locale_set!(ctx.res.res, "es");
```

```rust
locale_set!(ctx.res.res, "pt-br");
```

`locale_set!` turns built-in string literals like `"es"` into `Locale::ES`.

Unknown string literals become `Locale::Custom(...)`.

`set_locale` returns `true` when the locale exists.

Use `locale!(ctx.res.res, "menu.start")` to read active text.

Use `locale_in!(ctx.res.res, Locale::ES, "menu.start")` to read a specific locale.

## API Reference

### `set_locale`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Localization()` |
| Signature | `pub fn set_locale<L: IntoLocale>(&self, locale: L) -> bool` |
| Params | `&self, locale: Locale` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `locale`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Localization()` |
| Signature | `pub fn locale(&self) -> Locale` |
| Params | `&self` |
| Returns | `Locale` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Localization()` |
| Signature | `pub fn get<S: AsRef<str>>(&self, key: S) -> Option<&'static str>` |
| Params | `&self, key: S` |
| Returns | `Option<&'static str>` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_by_hash`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Localization()` |
| Signature | `pub fn get_by_hash(&self, key_hash: u64) -> Option<&'static str>` |
| Params | `&self, key_hash: u64` |
| Returns | `Option<&'static str>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_for_locale`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Localization()` |
| Signature | `pub fn get_for_locale<S: AsRef<str>>(&self, locale: Locale, key: S) -> Option<&'static str>` |
| Params | `&self, locale: Locale, key: S` |
| Returns | `Option<&'static str>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_for_locale_by_hash`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Localization()` |
| Signature | `pub fn get_for_locale_by_hash(&self, locale: Locale, key_hash: u64) -> Option<&'static str>` |
| Params | `&self, locale: Locale, key_hash: u64` |
| Returns | `Option<&'static str>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `locale_set`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Localization()` |
| Signature | `locale_set!(ctx.res.res, locale)` |
| Params | `ctx.res, locale` |
| Returns | `same as backing method` |
| Use when | Use when setting locale by enum or string literal. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `locale_get_current`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Localization()` |
| Signature | `locale_get_current!(ctx.res.res)` |
| Params | `ctx.res` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `locale`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Localization()` |
| Signature | `locale!(ctx.res.res, key)` |
| Params | `ctx.res, key` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `locale_in`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Localization()` |
| Signature | `locale_in!(ctx.res.res, locale, key)` |
| Params | `ctx.res, locale, key` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

