use crate::{StaticPipelineError, asset_uri, escape_rust_str, static_dir};
use perro_csv::CsvBuf;
use perro_ids::string_to_u64;
use perro_project::ProjectConfig;
use std::{collections::HashMap, fmt::Write as _, fs, path::Path};

const ARRAY_ITEMS_PER_LINE: usize = 25;

pub fn generate_static_localizations(
    project_root: &Path,
    config: &ProjectConfig,
) -> Result<(), StaticPipelineError> {
    let static_dir = static_dir(project_root);
    fs::create_dir_all(&static_dir)?;

    let Some(localization) = config.localization.as_ref() else {
        return write_empty_localizations(&static_dir);
    };

    let source_path = project_root.join(&localization.source_csv);
    let bytes = fs::read(&source_path).map_err(|err| {
        StaticPipelineError::SceneParse(format!(
            "failed to read localization source `{}`: {err}",
            localization.source_csv
        ))
    })?;

    let table = CsvBuf::from_bytes(&bytes).map_err(|err| {
        StaticPipelineError::SceneParse(format!(
            "failed to read localization csv header `{}`: {err}",
            localization.source_csv
        ))
    })?;
    let headers = table.headers();

    let key_idx = find_key_header_index(headers, &localization.key_column).ok_or_else(|| {
        StaticPipelineError::SceneParse(format!(
            "localization csv `{}` must use `{}` as first column",
            localization.source_csv, localization.key_column
        ))
    })?;

    let mut locale_column_by_code: HashMap<String, usize> = HashMap::new();
    let mut active_locales = Vec::<ActiveLocale>::new();
    active_locales.push(ActiveLocale::new("en".to_string(), 0));
    for (i, name) in headers.iter().enumerate() {
        if i == key_idx {
            continue;
        }
        let locale = name.trim().to_ascii_lowercase();
        if locale.is_empty() {
            continue;
        }
        if locale_column_by_code.contains_key(&locale) {
            continue;
        }
        locale_column_by_code.insert(locale.clone(), i);
        if locale != "en" {
            let idx = active_locales.len();
            active_locales.push(ActiveLocale::new(locale, idx));
        }
    }
    let mut key_hashes: HashMap<u64, String> = HashMap::new();
    let mut key_hash_order: Vec<u64> = Vec::new();
    let mut key_names_by_index: Vec<String> = Vec::new();
    let mut key_index_by_hash: HashMap<u64, usize> = HashMap::new();
    let mut locale_tables: Vec<Vec<Option<String>>> = vec![Vec::new(); active_locales.len()];

    for row in table.rows() {
        let key = row.get(key_idx).map(String::as_str).unwrap_or("").trim();
        if key.is_empty() {
            continue;
        }
        let hash = string_to_u64(key);
        if let Some(existing_key) = key_hashes.get(&hash)
            && existing_key != key
        {
            return Err(StaticPipelineError::SceneParse(format!(
                "localization hash collision in `{}` between keys `{existing_key}` and `{key}`",
                localization.source_csv
            )));
        }
        let key_index = if let Some(existing) = key_index_by_hash.get(&hash).copied() {
            existing
        } else {
            let new_index = key_hash_order.len();
            key_hash_order.push(hash);
            key_hashes.insert(hash, key.to_string());
            key_names_by_index.push(key.to_string());
            key_index_by_hash.insert(hash, new_index);
            for table in &mut locale_tables {
                table.push(None);
            }
            new_index
        };

        for (locale_idx, locale) in active_locales.iter().enumerate() {
            let Some(column_idx) = locale_column_by_code.get(&locale.code).copied() else {
                continue;
            };
            let value = row.get(column_idx).map(String::as_str).unwrap_or("").trim();
            if value.is_empty() {
                locale_tables[locale_idx][key_index] = None;
            } else {
                locale_tables[locale_idx][key_index] = Some(value.to_string());
            }
        }
    }

    let key_count = key_hash_order.len();
    let key_index_type = select_index_type(key_count, "key indices")?;

    let en_locale_index = active_locales
        .iter()
        .position(|locale| locale.code == "en")
        .ok_or_else(|| {
            StaticPipelineError::SceneParse(
                "internal localization generation error: english locale table missing".to_string(),
            )
        })?;

    let english_values: Vec<String> = (0..key_count)
        .map(|key_index| {
            locale_tables[en_locale_index][key_index]
                .clone()
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| key_names_by_index[key_index].clone())
        })
        .collect();

    let dense_locale_tables: Vec<Vec<String>> = (0..active_locales.len())
        .map(|locale_index| {
            (0..key_count)
                .map(|key_index| {
                    locale_tables[locale_index][key_index]
                        .clone()
                        .filter(|v| !v.is_empty())
                        .unwrap_or_else(|| english_values[key_index].clone())
                })
                .collect()
        })
        .collect();

    let mut hash_rows: Vec<(u64, usize)> = key_hash_order
        .iter()
        .copied()
        .enumerate()
        .map(|(key_index, hash)| (hash, key_index))
        .collect();
    hash_rows.sort_by_key(|(hash, _)| *hash);

    let mut string_index_by_value = HashMap::<String, usize>::new();
    let mut strings = Vec::<String>::new();
    for table in &dense_locale_tables {
        for value in table {
            if !string_index_by_value.contains_key(value) {
                let idx = strings.len();
                string_index_by_value.insert(value.clone(), idx);
                strings.push(value.clone());
            }
        }
    }

    let string_index_type = select_index_type(strings.len(), "string pool indices")?;

    let locale_string_indices: Vec<Vec<usize>> = dense_locale_tables
        .iter()
        .map(|table| {
            table
                .iter()
                .map(|value| {
                    string_index_by_value
                        .get(value)
                        .copied()
                        .expect("string index missing")
                })
                .collect()
        })
        .collect();

    let mut out = String::new();
    out.push_str("// Auto-generated by Perro Static Pipeline. Do not edit.\n");
    out.push_str("#![allow(unused_imports)]\n\n");
    out.push_str("type RuntimeLocale = perro_api::resource_api::sub_apis::Locale;\n");
    out.push_str(&format!("type KeyIndex = {key_index_type};\n\n"));
    out.push_str(&format!("type StringIndex = {string_index_type};\n\n"));

    let _ = writeln!(out, "const STRINGS: [&str; {}] = [", strings.len());
    for chunk in strings.chunks(ARRAY_ITEMS_PER_LINE) {
        out.push_str("    ");
        for value in chunk {
            let _ = write!(out, "\"{}\", ", escape_str(value));
        }
        out.push('\n');
    }
    out.push_str("];\n\n");

    let _ = writeln!(out, "const KEY_HASHES: [u64; {key_count}] = [");
    for chunk in hash_rows.chunks(ARRAY_ITEMS_PER_LINE) {
        out.push_str("    ");
        for (hash, _key_index) in chunk {
            let key = key_hashes
                .get(hash)
                .expect("localization key missing for hash");
            let _ = write!(out, "perro_ids::hash_str!(\"{}\"), ", escape_rust_str(key));
        }
        out.push('\n');
    }
    out.push_str("];\n\n");

    let _ = writeln!(out, "const KEY_INDICES: [KeyIndex; {key_count}] = [");
    for chunk in hash_rows.chunks(ARRAY_ITEMS_PER_LINE) {
        out.push_str("    ");
        for (_hash, key_index) in chunk {
            let _ = write!(out, "{}{}, ", key_index, key_index_type);
        }
        out.push('\n');
    }
    out.push_str("];\n\n");

    out.push_str("#[inline(always)]\n");
    out.push_str("const fn key_hash_to_index(key_hash: u64) -> Option<KeyIndex> {\n");
    out.push_str("    let mut left = 0usize;\n");
    out.push_str("    let mut right = KEY_HASHES.len();\n");
    out.push_str("    while left < right {\n");
    out.push_str("        let mid = left + ((right - left) / 2);\n");
    out.push_str("        let mid_hash = KEY_HASHES[mid];\n");
    out.push_str("        if key_hash < mid_hash {\n");
    out.push_str("            right = mid;\n");
    out.push_str("        } else if key_hash > mid_hash {\n");
    out.push_str("            left = mid + 1;\n");
    out.push_str("        } else {\n");
    out.push_str("            return Some(KEY_INDICES[mid]);\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("    None\n");
    out.push_str("}\n\n");

    out.push_str("#[inline(always)]\n");
    out.push_str("fn runtime_locale_index(locale: RuntimeLocale) -> usize {\n");
    out.push_str("    match locale {\n");
    for (active_idx, locale) in active_locales.iter().enumerate() {
        if let Some(runtime_variant) = locale_runtime_variant(&locale.code) {
            let _ = writeln!(
                out,
                "        RuntimeLocale::{runtime_variant} => {active_idx},"
            );
        } else {
            let _ = writeln!(
                out,
                "        RuntimeLocale::Custom(\"{}\") => {active_idx},",
                escape_rust_str(&locale.code)
            );
        }
    }
    let _ = writeln!(out, "        _ => {en_locale_index},");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    for (locale_idx, locale) in active_locales.iter().enumerate() {
        let table_name = &locale.table_ident;
        let _ = writeln!(out, "const {table_name}: [StringIndex; {key_count}] = [");
        for chunk in locale_string_indices[locale_idx].chunks(ARRAY_ITEMS_PER_LINE) {
            out.push_str("    ");
            for str_index in chunk {
                let _ = write!(out, "{}{}, ", str_index, string_index_type);
            }
            out.push('\n');
        }
        out.push_str("];\n\n");
    }

    out.push_str(&format!(
        "const LOCALES: [[StringIndex; {key_count}]; {}] = [\n",
        active_locales.len()
    ));
    for chunk in active_locales.chunks(ARRAY_ITEMS_PER_LINE) {
        out.push_str("    ");
        for locale in chunk {
            let _ = write!(out, "{}, ", locale.table_ident);
        }
        out.push('\n');
    }
    out.push_str("];\n\n");

    out.push_str(
        "pub fn lookup_localized_string(\n    locale: RuntimeLocale,\n    key_hash: u64,\n) -> &'static str {\n",
    );
    out.push_str("    let Some(key_index) = key_hash_to_index(key_hash) else {\n");
    out.push_str("        return \"\";\n");
    out.push_str("    };\n");
    out.push_str("    let key_index = key_index as usize;\n");
    out.push_str("    let locale_index = runtime_locale_index(locale);\n");
    out.push_str("    let string_index = LOCALES[locale_index][key_index] as usize;\n");
    out.push_str("    STRINGS[string_index]\n");
    out.push_str("}\n");

    fs::write(static_dir.join("localizations.rs"), out)?;
    let path = asset_uri(localization.source_csv.trim_start_matches("res/"));
    crate::record_static_assets(
        perro_asset_formats::dlc::DlcAssetKind::LOCALIZATION,
        perro_asset_formats::dlc::DlcAssetAccess::ENGINE_LOCAL,
        [(path.as_str(), false)],
    );
    Ok(())
}

pub fn generate_empty_localizations(project_root: &Path) -> Result<(), StaticPipelineError> {
    let static_dir = static_dir(project_root);
    fs::create_dir_all(&static_dir)?;
    write_empty_localizations(&static_dir)
}

fn write_empty_localizations(static_dir: &Path) -> Result<(), StaticPipelineError> {
    fs::write(
        static_dir.join("localizations.rs"),
        "// Auto-generated by Perro Static Pipeline. Do not edit.\n\
#![allow(unused_imports)]\n\n\
pub const fn lookup_localized_string(_locale: perro_api::resource_api::sub_apis::Locale, _key_hash: u64) -> &'static str {\n\
    \"\"\n\
}\n",
    )?;
    Ok(())
}

fn find_key_header_index(headers: &[String], expected: &str) -> Option<usize> {
    headers
        .first()
        .is_some_and(|header| header.trim().eq_ignore_ascii_case(expected))
        .then_some(0)
}

fn escape_str(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

struct ActiveLocale {
    code: String,
    table_ident: String,
}

impl ActiveLocale {
    fn new(code: String, index: usize) -> Self {
        let mut ident = String::new();
        for ch in code.chars() {
            if ch.is_ascii_alphanumeric() {
                ident.push(ch.to_ascii_uppercase());
            } else {
                ident.push('_');
            }
        }
        Self {
            code,
            table_ident: format!("LOCALE_{index}_{ident}"),
        }
    }
}

fn locale_runtime_variant(code: &str) -> Option<&'static str> {
    match code {
        "aa" => Some("AA"),
        "ab" => Some("AB"),
        "ae" => Some("AE"),
        "af" => Some("AF"),
        "ak" => Some("AK"),
        "am" => Some("AM"),
        "an" => Some("AN"),
        "ar" => Some("AR"),
        "as" => Some("AS"),
        "av" => Some("AV"),
        "ay" => Some("AY"),
        "az" => Some("AZ"),
        "ba" => Some("BA"),
        "be" => Some("BE"),
        "bg" => Some("BG"),
        "bi" => Some("BI"),
        "bm" => Some("BM"),
        "bn" => Some("BN"),
        "bo" => Some("BO"),
        "br" => Some("BR"),
        "bs" => Some("BS"),
        "ca" => Some("CA"),
        "ce" => Some("CE"),
        "ch" => Some("CH"),
        "co" => Some("CO"),
        "cr" => Some("CR"),
        "cs" => Some("CS"),
        "cu" => Some("CU"),
        "cv" => Some("CV"),
        "cy" => Some("CY"),
        "da" => Some("DA"),
        "de" => Some("DE"),
        "dv" => Some("DV"),
        "dz" => Some("DZ"),
        "ee" => Some("EE"),
        "el" => Some("EL"),
        "en" => Some("EN"),
        "eo" => Some("EO"),
        "es" => Some("ES"),
        "et" => Some("ET"),
        "eu" => Some("EU"),
        "fa" => Some("FA"),
        "ff" => Some("FF"),
        "fi" => Some("FI"),
        "fj" => Some("FJ"),
        "fo" => Some("FO"),
        "fr" => Some("FR"),
        "fy" => Some("FY"),
        "ga" => Some("GA"),
        "gd" => Some("GD"),
        "gl" => Some("GL"),
        "gn" => Some("GN"),
        "gu" => Some("GU"),
        "gv" => Some("GV"),
        "ha" => Some("HA"),
        "he" => Some("HE"),
        "hi" => Some("HI"),
        "ho" => Some("HO"),
        "hr" => Some("HR"),
        "ht" => Some("HT"),
        "hu" => Some("HU"),
        "hy" => Some("HY"),
        "hz" => Some("HZ"),
        "ia" => Some("IA"),
        "id" => Some("ID"),
        "ie" => Some("IE"),
        "ig" => Some("IG"),
        "ii" => Some("II"),
        "ik" => Some("IK"),
        "io" => Some("IO"),
        "is" => Some("IS"),
        "it" => Some("IT"),
        "iu" => Some("IU"),
        "ja" => Some("JA"),
        "jv" => Some("JV"),
        "ka" => Some("KA"),
        "kg" => Some("KG"),
        "ki" => Some("KI"),
        "kj" => Some("KJ"),
        "kk" => Some("KK"),
        "kl" => Some("KL"),
        "km" => Some("KM"),
        "kn" => Some("KN"),
        "ko" => Some("KO"),
        "kr" => Some("KR"),
        "ks" => Some("KS"),
        "ku" => Some("KU"),
        "kv" => Some("KV"),
        "kw" => Some("KW"),
        "ky" => Some("KY"),
        "la" => Some("LA"),
        "lb" => Some("LB"),
        "lg" => Some("LG"),
        "li" => Some("LI"),
        "ln" => Some("LN"),
        "lo" => Some("LO"),
        "lt" => Some("LT"),
        "lu" => Some("LU"),
        "lv" => Some("LV"),
        "mg" => Some("MG"),
        "mh" => Some("MH"),
        "mi" => Some("MI"),
        "mk" => Some("MK"),
        "ml" => Some("ML"),
        "mn" => Some("MN"),
        "mr" => Some("MR"),
        "ms" => Some("MS"),
        "mt" => Some("MT"),
        "my" => Some("MY"),
        "na" => Some("NA"),
        "nb" => Some("NB"),
        "nd" => Some("ND"),
        "ne" => Some("NE"),
        "ng" => Some("NG"),
        "nl" => Some("NL"),
        "nn" => Some("NN"),
        "no" => Some("NO"),
        "nr" => Some("NR"),
        "nv" => Some("NV"),
        "ny" => Some("NY"),
        "oc" => Some("OC"),
        "oj" => Some("OJ"),
        "om" => Some("OM"),
        "or" => Some("OR"),
        "os" => Some("OS"),
        "pa" => Some("PA"),
        "pi" => Some("PI"),
        "pl" => Some("PL"),
        "ps" => Some("PS"),
        "pt" => Some("PT"),
        "qu" => Some("QU"),
        "rm" => Some("RM"),
        "rn" => Some("RN"),
        "ro" => Some("RO"),
        "ru" => Some("RU"),
        "rw" => Some("RW"),
        "sa" => Some("SA"),
        "sc" => Some("SC"),
        "sd" => Some("SD"),
        "se" => Some("SE"),
        "sg" => Some("SG"),
        "si" => Some("SI"),
        "sk" => Some("SK"),
        "sl" => Some("SL"),
        "sm" => Some("SM"),
        "sn" => Some("SN"),
        "so" => Some("SO"),
        "sq" => Some("SQ"),
        "sr" => Some("SR"),
        "ss" => Some("SS"),
        "st" => Some("ST"),
        "su" => Some("SU"),
        "sv" => Some("SV"),
        "sw" => Some("SW"),
        "ta" => Some("TA"),
        "te" => Some("TE"),
        "tg" => Some("TG"),
        "th" => Some("TH"),
        "ti" => Some("TI"),
        "tk" => Some("TK"),
        "tl" => Some("TL"),
        "tn" => Some("TN"),
        "to" => Some("TO"),
        "tr" => Some("TR"),
        "ts" => Some("TS"),
        "tt" => Some("TT"),
        "tw" => Some("TW"),
        "ty" => Some("TY"),
        "ug" => Some("UG"),
        "uk" => Some("UK"),
        "ur" => Some("UR"),
        "uz" => Some("UZ"),
        "ve" => Some("VE"),
        "vi" => Some("VI"),
        "vo" => Some("VO"),
        "wa" => Some("WA"),
        "wo" => Some("WO"),
        "xh" => Some("XH"),
        "yi" => Some("YI"),
        "yo" => Some("YO"),
        "za" => Some("ZA"),
        "zh" => Some("ZH"),
        "zu" => Some("ZU"),
        _ => None,
    }
}

fn select_index_type(count: usize, label: &str) -> Result<&'static str, StaticPipelineError> {
    if count <= u16::MAX as usize {
        return Ok("u16");
    }
    if count <= u32::MAX as usize {
        return Ok("u32");
    }
    Err(StaticPipelineError::SceneParse(format!(
        "localization {label} exceed supported limit: {count} > {}",
        u32::MAX
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use perro_project::{LocalizationConfig, ProjectConfig};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(label: &str) -> std::path::PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("perro_static_pipeline_{label}_{ts}"))
    }

    #[test]
    fn static_localizations_emit_custom_locale_columns() {
        let root = unique_temp_dir("custom_locales");
        std::fs::create_dir_all(&root).expect("create temp root");
        std::fs::write(
            root.join("locale.csv"),
            "key,en,ga,pt-br\nmenu.start,Start,Tosach,Comecar\n",
        )
        .expect("write locale csv");

        let mut config = ProjectConfig::default_for_name("CustomLocaleTest");
        config.localization = Some(LocalizationConfig {
            source_csv: "locale.csv".to_string(),
            key_column: "key".to_string(),
            default_locale: "ga".to_string(),
        });

        generate_static_localizations(&root, &config).expect("generate static localizations");
        let generated = std::fs::read_to_string(
            root.join(".perro")
                .join("project")
                .join("src")
                .join("static")
                .join("localizations.rs"),
        )
        .expect("read generated localizations");

        assert!(generated.contains("RuntimeLocale::EN => 0"));
        assert!(generated.contains("RuntimeLocale::GA => 1"));
        assert!(generated.contains("RuntimeLocale::Custom(\"pt-br\") => 2"));
        assert!(generated.contains("const LOCALE_1_GA"));
        assert!(generated.contains("const LOCALE_2_PT_BR"));
    }
}
