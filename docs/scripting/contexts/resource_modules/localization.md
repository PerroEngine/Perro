# Localization Module

Access:

- `res.Localization()`

Enum:

- `Locale::ZH` - Chinese (中文)
- `Locale::EN` - English (English)
- `Locale::RU` - Russian (Русский)
- `Locale::ES` - Spanish (Español)
- `Locale::PT` - Portuguese (Português)
- `Locale::DE` - German (Deutsch)
- `Locale::JA` - Japanese (日本語)
- `Locale::FR` - French (Français)
- `Locale::KO` - Korean (한국어)
- `Locale::PL` - Polish (Polski)
- `Locale::TR` - Turkish (Türkçe)
- `Locale::IT` - Italian (Italiano)
- `Locale::NL` - Dutch (Nederlands)
- `Locale::VI` - Vietnamese (Tiếng Việt)
- `Locale::ID` - Indonesian (Bahasa Indonesia)
- `Locale::AR` - Arabic (العربية)
- `Locale::HI` - Hindi (हिन्दी)
- `Locale::BN` - Bengali (বাংলা)
- `Locale::UR` - Urdu (اردو)
- `Locale::FA` - Persian (فارسی)
- `Locale::SW` - Swahili (Kiswahili)

Macros:

- `locale_set!(res, Locale::ES) -> bool`
- `locale_set_code!(res, "es") -> bool`
- `locale_get_current!(res) -> Arc<str>`
- `locale!(res, "camera.init") -> &str`
- `locale_in!(res, Locale::ES, "camera.init") -> &str`

Module methods:

- `res.Localization().set_locale(Locale::ES) -> bool`
- `res.Localization().set_locale_code("es") -> bool`
- `res.Localization().locale_code() -> Arc<str>`
- `res.Localization().get("camera.init") -> Option<&'static str>`
- `res.Localization().get_by_hash(key_hash) -> Option<&'static str>`
- `res.Localization().get_for_locale(Locale::ES, "camera.init") -> Option<&'static str>`
- `res.Localization().get_for_locale_by_hash(Locale::ES, key_hash) -> Option<&'static str>`

How lookup works:

- `locale!(res, "literal.key")` uses a compile-time key hash and queries current locale.
- `locale_in!(...)` queries a specific locale without changing current locale.
- If key is missing, macros return the key itself as fallback.

Ownership:

- `locale!` returns `&str`.
- If you need ownership, call `.to_string()`.

Example:

```rust
println!("[current {}]", locale_get_current!(res));
println!("{}", locale!(res, "camera.init"));

locale_set!(res, Locale::ES);
println!("{}", locale!(res, "camera.init"));
println!("{}", locale_in!(res, Locale::EN, "camera.init"));

let owned = locale!(res, "camera.init").to_string();
```
