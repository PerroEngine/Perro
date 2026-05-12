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

Setup:

- Put `localization.csv`, `locale.csv`, or `translations.csv` next to `project.toml`.
- Do not put localization CSV in `res/`.
- First CSV column must be `key`.
- Other columns use language codes.
- `project.toml` may set default locale:

```toml
[localization]
default_locale = "en"
```

- If unset, default locale is `en`.

How lookup works:

- `locale!(res, "literal.key")` uses a compile-time key hash and queries current locale.
- `locale_in!(...)` queries a specific locale without changing current locale.
- If key is missing, macros return the key itself as fallback.

Scene text binding:

- Text fields in `.scn` can bind directly to a localization key.
- Use `text = %loc: "menu.start"` for clear scene-side binding.
- String form also works: `text = "%loc:\"menu.start\""`.
- Literal escape: `text = "%%loc:not_a_key"` renders `%loc:not_a_key`.
- Bound text updates when `locale_set!` changes the current locale.
- Missing keys render the key itself.
- Supported fields:
  - `UiLabel.text`
  - `UiTextBox.text`
  - `UiTextBox.placeholder` / `hint`
  - `UiTextBlock.text`
  - `UiTextBlock.placeholder` / `hint`
- Runtime scripts can switch a node binding with `bind_locale_text!(ctx.run, node_id, "menu.alt")`.
- Runtime scripts can bind placeholders with `bind_locale_placeholder!(ctx.run, node_id, "player.name.placeholder")`.

Scene example:

```scn
[title]
[UiLabel]
    text = %loc: "menu.title"
[/UiLabel]
[/title]

[name_input]
[UiTextBox]
    placeholder = %loc: "player.name.placeholder"
[/UiTextBox]
[/name_input]
```

Ownership:

- `locale!` returns `&str`.

Example:

```rust
println!("[current {}]", locale_get_current!(res));
println!("{}", locale!(res, "camera.init"));

locale_set!(res, Locale::ES);
println!("{}", locale!(res, "camera.init"));
println!("{}", locale_in!(res, Locale::EN, "camera.init"));

let owned = locale!(res, "camera.init").to_string();
```
