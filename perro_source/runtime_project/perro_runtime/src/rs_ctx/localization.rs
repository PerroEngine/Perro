use super::RuntimeResourceApi;
use super::state::RuntimeLocalizationState;
use csv::StringRecord;
use perro_ids::string_to_u64;
use perro_resource_context::sub_apis::{Locale, LocalizationAPI};
use std::{
    collections::HashMap,
    sync::{OnceLock, RwLock},
};

impl LocalizationAPI for RuntimeResourceApi {
    fn localization_set_locale(&self, locale: Locale) -> bool {
        if self.static_localization_lookup.is_some() {
            let mut localization = self
                .localization
                .write()
                .expect("resource api localization rwlock poisoned");
            localization.current_locale = locale;
            localization.current_locale_code = intern_localization_str(locale.code());
            return true;
        }

        let locale_code = locale.code().trim().to_ascii_lowercase();
        if locale_code.is_empty() {
            return false;
        }
        let mut localization = self
            .localization
            .write()
            .expect("resource api localization rwlock poisoned");
        self.load_locale_into_state(&mut localization, &locale_code)
    }

    fn localization_get_locale(&self) -> Locale {
        let localization = self
            .localization
            .read()
            .expect("resource api localization rwlock poisoned");
        localization.current_locale
    }

    fn localization_get(&self, key: &str) -> Option<&'static str> {
        if let Some(lookup) = self.static_localization_lookup {
            let localization = self
                .localization
                .read()
                .expect("resource api localization rwlock poisoned");
            return Some(lookup(localization.current_locale, string_to_u64(key)));
        }
        let localization = self
            .localization
            .read()
            .expect("resource api localization rwlock poisoned");
        localization.active_by_key.get(key).copied()
    }

    fn localization_get_by_hash(&self, key_hash: u64) -> Option<&'static str> {
        if let Some(lookup) = self.static_localization_lookup {
            let localization = self
                .localization
                .read()
                .expect("resource api localization rwlock poisoned");
            return Some(lookup(localization.current_locale, key_hash));
        }
        let localization = self
            .localization
            .read()
            .expect("resource api localization rwlock poisoned");
        localization.active_by_hash.get(&key_hash).copied()
    }

    fn localization_get_for_locale(&self, locale: Locale, key: &str) -> Option<&'static str> {
        if let Some(lookup) = self.static_localization_lookup {
            return Some(lookup(locale, string_to_u64(key)));
        }

        let locale_code = locale.code().trim().to_ascii_lowercase();
        if locale_code.is_empty() {
            return None;
        }
        let localization = self
            .localization
            .read()
            .expect("resource api localization rwlock poisoned");
        let source = localization.source_csv.as_ref()?;
        let (by_key, _) =
            read_localization_csv(source, &localization.key_column, &locale_code).ok()?;
        by_key.get(key).copied()
    }

    fn localization_get_for_locale_by_hash(
        &self,
        locale: Locale,
        key_hash: u64,
    ) -> Option<&'static str> {
        if let Some(lookup) = self.static_localization_lookup {
            return Some(lookup(locale, key_hash));
        }

        let locale_code = locale.code().trim().to_ascii_lowercase();
        if locale_code.is_empty() {
            return None;
        }
        let localization = self
            .localization
            .read()
            .expect("resource api localization rwlock poisoned");
        let source = localization.source_csv.as_ref()?;
        let (_, by_hash) =
            read_localization_csv(source, &localization.key_column, &locale_code).ok()?;
        by_hash.get(&key_hash).copied()
    }
}

impl RuntimeResourceApi {
    pub(crate) fn initialize_localization(&self) {
        if self.static_localization_lookup.is_some() {
            return;
        }
        let mut localization = self
            .localization
            .write()
            .expect("resource api localization rwlock poisoned");
        let current = localization.current_locale_code;
        let _ = self.load_locale_into_state(&mut localization, current);
    }

    fn load_locale_into_state(
        &self,
        localization: &mut RuntimeLocalizationState,
        locale_code: &str,
    ) -> bool {
        let Some(source) = localization.source_csv.as_deref() else {
            return false;
        };
        let Ok((by_key, by_hash)) =
            read_localization_csv(source, &localization.key_column, locale_code)
        else {
            return false;
        };
        localization.current_locale_code = intern_localization_str(locale_code);
        localization.current_locale = locale_from_code(localization.current_locale_code);
        localization.active_by_key = by_key;
        localization.active_by_hash = by_hash;
        true
    }
}

type LocalizationByKey = HashMap<&'static str, &'static str>;
type LocalizationByHash = HashMap<u64, &'static str>;
type LocalizationLookupMaps = (LocalizationByKey, LocalizationByHash);

fn read_localization_csv(
    source: &str,
    key_column: &str,
    locale_code: &str,
) -> Result<LocalizationLookupMaps, String> {
    let bytes = perro_io::load_asset(source)
        .map_err(|err| format!("failed to read localization csv `{source}`: {err}"))?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(bytes.as_slice());
    let headers = reader
        .headers()
        .map_err(|err| format!("failed to parse csv headers in `{source}`: {err}"))?
        .clone();

    let key_idx = find_header_index(&headers, key_column).ok_or_else(|| {
        format!("csv `{source}` is missing key column `{key_column}` in header row")
    })?;
    let locale_idx = find_header_index(&headers, locale_code).ok_or_else(|| {
        format!("csv `{source}` is missing locale column `{locale_code}` in header row")
    })?;

    let mut by_key: LocalizationByKey = HashMap::new();
    let mut by_hash: LocalizationByHash = HashMap::new();

    for row in reader.records() {
        let row = row.map_err(|err| format!("failed to parse csv row in `{source}`: {err}"))?;
        let key = row.get(key_idx).unwrap_or("").trim();
        if key.is_empty() {
            continue;
        }
        let value = row.get(locale_idx).unwrap_or("").trim();
        let key_static = intern_localization_str(key);
        let value_static = intern_localization_str(value);
        by_hash.insert(string_to_u64(key), value_static);
        by_key.insert(key_static, value_static);
    }

    Ok((by_key, by_hash))
}

fn intern_localization_str(value: &str) -> &'static str {
    static INTERNER: OnceLock<RwLock<HashMap<String, &'static str>>> = OnceLock::new();
    let interner = INTERNER.get_or_init(|| RwLock::new(HashMap::new()));

    if let Some(existing) = interner
        .read()
        .expect("localization interner rwlock poisoned")
        .get(value)
        .copied()
    {
        return existing;
    }

    let mut write = interner
        .write()
        .expect("localization interner rwlock poisoned");
    if let Some(existing) = write.get(value).copied() {
        return existing;
    }
    let leaked: &'static str = Box::leak(value.to_string().into_boxed_str());
    write.insert(value.to_string(), leaked);
    leaked
}

fn find_header_index(headers: &StringRecord, expected: &str) -> Option<usize> {
    let expected = expected.trim().to_ascii_lowercase();
    headers
        .iter()
        .position(|header| header.trim().eq_ignore_ascii_case(&expected))
}

fn locale_from_code(code: &'static str) -> Locale {
    match code {
        "zh" => Locale::ZH,
        "en" => Locale::EN,
        "ru" => Locale::RU,
        "es" => Locale::ES,
        "pt" => Locale::PT,
        "de" => Locale::DE,
        "ja" => Locale::JA,
        "fr" => Locale::FR,
        "ko" => Locale::KO,
        "pl" => Locale::PL,
        "tr" => Locale::TR,
        "it" => Locale::IT,
        "nl" => Locale::NL,
        "vi" => Locale::VI,
        "id" => Locale::ID,
        "ar" => Locale::AR,
        "hi" => Locale::HI,
        "bn" => Locale::BN,
        "ur" => Locale::UR,
        "fa" => Locale::FA,
        "sw" => Locale::SW,
        _ => Locale::Custom(code),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use perro_io::{ProjectRoot, set_project_root};
    use perro_project::LocalizationConfig;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(label: &str) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("perro_runtime_{label}_{ts}"))
    }

    fn setup_csv_project() -> PathBuf {
        let root = unique_temp_dir("localization");
        let res = root.join("res");
        std::fs::create_dir_all(&res).expect("failed to create test res dir");
        std::fs::write(
            res.join("localization.csv"),
            "key,en,es,fr,ja,zh\nhello,Hello,Hola,Bonjour,こんにちは,你好\nbye,Bye,Adios,Au revoir,さようなら,再见\n",
        )
        .expect("failed to write localization csv");
        set_project_root(ProjectRoot::Disk {
            root: root.clone(),
            name: "LocalizationTest".to_string(),
        });
        root
    }

    #[test]
    fn localization_loads_default_locale_and_switches_active_map() {
        let _root = setup_csv_project();
        let api = RuntimeResourceApi::new(
            None,
            None,
            None,
            None,
            None,
            Some(LocalizationConfig {
                source_csv: "res://localization.csv".to_string(),
                source_csv_hash: None,
                key_column: "key".to_string(),
                default_locale: "en".to_string(),
            }),
        );

        assert_eq!(
            api.localization_get_locale(),
            Locale::EN,
            "default locale should load on startup"
        );
        assert_eq!(
            api.localization_get("hello"),
            Some("Hello"),
            "default locale value should resolve"
        );
        assert_eq!(
            api.localization_get_by_hash(string_to_u64("bye")),
            Some("Bye"),
            "hashed lookup should resolve with same key hash as macros"
        );

        assert!(
            api.localization_set_locale(Locale::ES),
            "switching to an existing locale should succeed"
        );
        assert_eq!(api.localization_get_locale(), Locale::ES);
        assert_eq!(api.localization_get("hello"), Some("Hola"));
        assert_eq!(
            api.localization_get_by_hash(string_to_u64("bye")),
            Some("Adios")
        );

        assert!(
            !api.localization_set_locale(Locale::Custom("xx")),
            "switching to a missing locale should fail"
        );
        assert_eq!(
            api.localization_get_locale(),
            Locale::ES,
            "failed switch must keep previous active locale"
        );
        assert_eq!(api.localization_get("hello"), Some("Hola"));
    }

    #[test]
    fn static_localization_switches_by_locale_code_even_with_configured_csv_source() {
        fn static_lookup(locale: Locale, key_hash: u64) -> &'static str {
            if key_hash != string_to_u64("camera.init") {
                return "";
            }
            match locale {
                Locale::EN => "Camera initialized",
                Locale::ES => "Camara inicializada",
                _ => "",
            }
        }

        let api = RuntimeResourceApi::new(
            None,
            None,
            None,
            None,
            Some(static_lookup),
            Some(LocalizationConfig {
                source_csv: "res://localization.csv".to_string(),
                source_csv_hash: None,
                key_column: "key".to_string(),
                default_locale: "en".to_string(),
            }),
        );

        assert_eq!(
            api.localization_get("camera.init"),
            Some("Camera initialized")
        );
        assert!(api.localization_set_locale(Locale::ES));
        assert_eq!(
            api.localization_get("camera.init"),
            Some("Camara inicializada")
        );
    }
}
