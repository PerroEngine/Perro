use crate::{StaticPipelineError, res_dir, static_dir};
use csv::StringRecord;
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
    let key_index_type = select_index_type(key_count, "key indices")?;

    let en_locale_index = active_locales
        .iter()
        .position(|(code, _)| *code == "en")
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
    out.push_str("type RuntimeLocale = perro::resource_context::sub_apis::Locale;\n");
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
            let _ = write!(out, "0x{hash:016X}u64, ");
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
    let mut active_index_by_code: HashMap<&'static str, usize> = HashMap::new();
    for (idx, (code, _variant)) in active_locales.iter().enumerate() {
        active_index_by_code.insert(*code, idx);
    }

    out.push_str("const fn runtime_locale_index(locale: RuntimeLocale) -> usize {\n");
    out.push_str("    match locale {\n");
    for (code, runtime_variant) in &supported_locales {
        let active_idx = active_index_by_code
            .get(code)
            .copied()
            .unwrap_or(en_locale_index);
        {
            let _ = writeln!(
                out,
                "        RuntimeLocale::{runtime_variant} => {active_idx},"
            );
        }
    }
    let _ = writeln!(
        out,
        "        RuntimeLocale::Custom(_) => {en_locale_index},"
    );
    out.push_str("    }\n");
    out.push_str("}\n\n");

    for (locale_idx, (_code, runtime_variant)) in active_locales.iter().enumerate() {
        let table_name = format!("LOCALE_{runtime_variant}");
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
        for (_, runtime_variant) in chunk {
            let table_name = format!("LOCALE_{runtime_variant}");
            let _ = write!(out, "{table_name}, ");
        }
        out.push('\n');
    }
    out.push_str("];\n\n");

    out.push_str(
        "pub const fn lookup_localized_string(\n    locale: RuntimeLocale,\n    key_hash: u64,\n) -> Option<&'static str> {\n",
    );
    out.push_str("    let Some(key_index) = key_hash_to_index(key_hash) else {\n");
    out.push_str("        return None;\n");
    out.push_str("    };\n");
    out.push_str("    let key_index = key_index as usize;\n");
    out.push_str("    let locale_index = runtime_locale_index(locale);\n");
    out.push_str("    let string_index = LOCALES[locale_index][key_index] as usize;\n");
    out.push_str("    Some(STRINGS[string_index])\n");
    out.push_str("}\n");

    fs::write(static_dir.join("localizations.rs"), out)?;
    Ok(())
}

fn write_empty_localizations(static_dir: &Path) -> Result<(), StaticPipelineError> {
    fs::write(
        static_dir.join("localizations.rs"),
        "// Auto-generated by Perro Static Pipeline. Do not edit.\n\
#![allow(unused_imports)]\n\n\
pub const fn lookup_localized_string(_locale: perro::resource_context::sub_apis::Locale, _key_hash: u64) -> Option<&'static str> {\n\
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
