use crate::{StaticPipelineError, res_dir, static_dir};
use csv::StringRecord;
use perro_ids::string_to_u64;
use perro_project::ProjectConfig;
use std::{
    collections::{BTreeMap, HashMap},
    fmt::Write as _,
    fs,
    path::Path,
};

pub fn generate_static_localizations(
    project_root: &Path,
    config: &ProjectConfig,
) -> Result<(), StaticPipelineError> {
    let static_dir = static_dir(project_root);
    fs::create_dir_all(&static_dir)?;

    let Some(localization) = config.localization.as_ref() else {
        return write_empty_localizations(&static_dir);
    };

    let source_rel = localization
        .source_csv
        .strip_prefix("res://")
        .ok_or_else(|| {
            StaticPipelineError::SceneParse(format!(
                "localization source must start with `res://`: {}",
                localization.source_csv
            ))
        })?
        .replace('\\', "/");
    let source_path = res_dir(project_root).join(&source_rel);
    let bytes = fs::read(&source_path).map_err(|err| {
        StaticPipelineError::SceneParse(format!(
            "failed to read localization source `{}`: {err}",
            localization.source_csv
        ))
    })?;

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(bytes.as_slice());
    let headers = reader.headers().map_err(|err| {
        StaticPipelineError::SceneParse(format!(
            "failed to read localization csv header `{}`: {err}",
            localization.source_csv
        ))
    })?;

    let key_idx = find_header_index(headers, &localization.key_column).ok_or_else(|| {
        StaticPipelineError::SceneParse(format!(
            "localization csv `{}` missing key column `{}`",
            localization.source_csv, localization.key_column
        ))
    })?;

    let locale_column_by_code: HashMap<String, usize> = headers
        .iter()
        .enumerate()
        .filter_map(|(i, name)| {
            if i == key_idx {
                return None;
            }
            let locale = name.trim().to_ascii_lowercase();
            if locale.is_empty() {
                return None;
            }
            Some((locale, i))
        })
        .collect();

    let supported_locales = supported_locales();
    let mut active_locales = Vec::<(&'static str, &'static str)>::new();
    for (code, runtime_variant) in &supported_locales {
        if *code == "en" || locale_column_by_code.contains_key(*code) {
            active_locales.push((*code, *runtime_variant));
        }
    }
    let mut key_hashes: HashMap<u64, String> = HashMap::new();
    let mut key_hash_order: Vec<u64> = Vec::new();
    let mut key_names_by_index: Vec<String> = Vec::new();
    let mut key_index_by_hash: HashMap<u64, usize> = HashMap::new();
    let mut locale_tables: Vec<Vec<Option<String>>> = vec![Vec::new(); active_locales.len()];

    for row in reader.records() {
        let row = row.map_err(|err| {
            StaticPipelineError::SceneParse(format!(
                "failed to parse localization csv row `{}`: {err}",
                localization.source_csv
            ))
        })?;
        let key = row.get(key_idx).unwrap_or("").trim();
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

        for (locale_idx, (code, _runtime_variant)) in active_locales.iter().enumerate() {
            let Some(column_idx) = locale_column_by_code.get(*code).copied() else {
                continue;
            };
            let value = row.get(column_idx).unwrap_or("").trim();
            if value.is_empty() {
                locale_tables[locale_idx][key_index] = None;
            } else {
                locale_tables[locale_idx][key_index] = Some(value.to_string());
            }
        }
    }

    let key_count = key_hash_order.len();
    let key_index_type = if key_count <= u16::MAX as usize {
        "u16"
    } else {
        "u32"
    };

    let mut interned_strings = BTreeMap::<String, String>::new();
    let mut next_string_id: u32 = 0;
    for table in &locale_tables {
        for value in table.iter().flatten() {
            interned_strings.entry(value.clone()).or_insert_with(|| {
                let ident = format!("S_{next_string_id:05}");
                next_string_id += 1;
                ident
            });
        }
    }

    let mut out = String::new();
    out.push_str("// Auto-generated by Perro Static Pipeline. Do not edit.\n");
    out.push_str("#![allow(unused_imports)]\n\n");
    out.push_str("type RuntimeLocale = perro::resource_context::sub_apis::Locale;\n");
    out.push_str(&format!("type KeyIndex = {key_index_type};\n\n"));

    for (idx, key) in key_names_by_index.iter().enumerate() {
        let const_name = key_hash_const_name(key, idx);
        let _ = writeln!(
            out,
            "const {const_name}: u64 = perro::ids::string_to_u64(\"{}\");",
            escape_str(key)
        );
    }
    if !key_names_by_index.is_empty() {
        out.push('\n');
    }

    out.push_str("const fn key_hash_to_index(key_hash: u64) -> Option<KeyIndex> {\n");
    out.push_str("    match key_hash {\n");
    for (idx, _hash) in key_hash_order.iter().enumerate() {
        let key = &key_names_by_index[idx];
        let const_name = key_hash_const_name(key, idx);
        let _ = writeln!(
            out,
            "        {const_name} => Some({idx}{key_index_type}), // {}",
            escape_comment(key)
        );
    }
    out.push_str("        _ => None,\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    out.push_str("#[inline]\n");
    let mut active_index_by_code: HashMap<&'static str, usize> = HashMap::new();
    for (idx, (code, _variant)) in active_locales.iter().enumerate() {
        active_index_by_code.insert(*code, idx);
    }

    out.push_str("const fn runtime_locale_index(locale: RuntimeLocale) -> Option<usize> {\n");
    out.push_str("    match locale {\n");
    for (code, runtime_variant) in &supported_locales {
        if let Some(active_idx) = active_index_by_code.get(code).copied() {
            let _ = writeln!(
                out,
                "        RuntimeLocale::{runtime_variant} => Some({active_idx}),"
            );
        } else {
            let _ = writeln!(out, "        RuntimeLocale::{runtime_variant} => None,");
        }
    }
    out.push_str("        RuntimeLocale::Custom(_) => None,\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    for (value, const_name) in &interned_strings {
        let _ = writeln!(out, "static {const_name}: &str = \"{}\";", escape_str(value));
    }
    if !interned_strings.is_empty() {
        out.push('\n');
    }

    for (locale_idx, (_code, runtime_variant)) in active_locales.iter().enumerate() {
        let table_name = format!("LOCALE_{runtime_variant}");
        let _ = writeln!(
            out,
            "static {table_name}: [Option<&'static str>; {key_count}] = ["
        );
        if key_count == 0 {
            out.push_str("];\n\n");
            continue;
        }
        for (row_idx, value) in locale_tables[locale_idx].iter().enumerate() {
            let key = &key_names_by_index[row_idx];
            match value {
                Some(v) => {
                    let const_name = interned_strings.get(v).expect("interned string missing");
                    let _ = writeln!(
                        out,
                        "    Some({const_name}), // {} => {}",
                        escape_comment(key),
                        escape_comment(v)
                    );
                }
                None => {
                    let _ = writeln!(out, "    None, // {} => <missing>", escape_comment(key));
                }
            }
        }
        out.push_str("];\n\n");
    }

    out.push_str(&format!(
        "static LOCALES: [&[Option<&'static str>; {key_count}]; {}] = [\n",
        active_locales.len()
    ));
    for (_, runtime_variant) in &active_locales {
        let table_name = format!("LOCALE_{runtime_variant}");
        let _ = writeln!(out, "    &{table_name},");
    }
    out.push_str("];\n\n");
    out.push_str("const LOCALE_EN_INDEX: usize = 0;\n\n");

    out.push_str(
        "pub const fn lookup_localized_string(\n    locale: RuntimeLocale,\n    key_hash: u64,\n) -> Option<&'static str> {\n",
    );
    out.push_str("    let Some(key_index) = key_hash_to_index(key_hash) else {\n");
    out.push_str("        return None;\n");
    out.push_str("    };\n");
    out.push_str("    let key_index = key_index as usize;\n");
    out.push_str("    let locale_index = match runtime_locale_index(locale) {\n");
    out.push_str("        Some(index) => index,\n");
    out.push_str("        None => LOCALE_EN_INDEX,\n");
    out.push_str("    };\n");
    out.push_str("    match LOCALES[locale_index][key_index] {\n");
    out.push_str("        Some(value) => Some(value),\n");
    out.push_str("        None => LOCALE_EN[key_index],\n");
    out.push_str("    }\n");
    out.push_str("}\n");

    fs::write(static_dir.join("localizations.rs"), out)?;
    Ok(())
}

fn write_empty_localizations(static_dir: &Path) -> Result<(), StaticPipelineError> {
    fs::write(
        static_dir.join("localizations.rs"),
        "// Auto-generated by Perro Static Pipeline. Do not edit.\n\
#![allow(unused_imports)]\n\n\
pub fn lookup_localized_string(_locale: perro::resource_context::sub_apis::Locale, _key_hash: u64) -> Option<&'static str> {\n\
    None\n\
}\n",
    )?;
    Ok(())
}

fn find_header_index(headers: &StringRecord, expected: &str) -> Option<usize> {
    headers
        .iter()
        .position(|header| header.trim().eq_ignore_ascii_case(expected))
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

fn escape_comment(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

fn key_hash_const_name(key: &str, index: usize) -> String {
    format!(
        "KEY_HASH_{}_{}",
        sanitize_ident(key).to_ascii_uppercase(),
        index
    )
}

fn sanitize_ident(input: &str) -> String {
    let mut out = String::new();
    for c in input.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        return "x".to_string();
    }
    if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

fn supported_locales() -> Vec<(&'static str, &'static str)> {
    vec![
        ("en", "EN"),
        ("es", "ES"),
        ("pt", "PT"),
        ("fr", "FR"),
        ("it", "IT"),
        ("de", "DE"),
        ("tr", "TR"),
        ("pl", "PL"),
        ("ru", "RU"),
        ("zh", "ZH"),
        ("ja", "JA"),
        ("ko", "KO"),
        ("ar", "AR"),
        ("nl", "NL"),
        ("vi", "VI"),
        ("id", "ID"),
        ("hi", "HI"),
        ("bn", "BN"),
        ("ur", "UR"),
        ("fa", "FA"),
        ("sw", "SW"),
    ]
}
