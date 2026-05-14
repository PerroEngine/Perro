use super::core::RuntimeResourceApi;
use perro_csv::{Csv, CsvBuf, EMPTY_CSV};
use perro_io::{ProjectRoot, get_project_root, save_asset};
use perro_resource_api::sub_apis::CsvAPI;

impl CsvAPI for RuntimeResourceApi {
    fn load_csv_source_hashed(&self, source_hash: u64, source: Option<&str>) -> &'static Csv {
        if let Some(lookup) = self.static_csv_lookup {
            return lookup(source_hash);
        }

        if let Some(existing) = self
            .csv_cache
            .lock()
            .expect("resource api csv cache mutex poisoned")
            .get(&source_hash)
            .copied()
        {
            return existing;
        }

        let Some(source) = source else {
            return &EMPTY_CSV;
        };
        let Ok(bytes) = perro_io::load_asset(source) else {
            return &EMPTY_CSV;
        };
        let Ok(table) = perro_csv::parse_csv_static(&bytes) else {
            return &EMPTY_CSV;
        };
        self.csv_cache
            .lock()
            .expect("resource api csv cache mutex poisoned")
            .insert(source_hash, table);
        table
    }

    fn save_csv_source(&self, source: &str, csv: &CsvBuf) -> Result<(), String> {
        let bytes = csv.to_bytes()?;
        if let Some(stripped) = source.strip_prefix("res://")
            && let ProjectRoot::Disk { root, .. } = get_project_root()
        {
            let path = root.join("res").join(stripped);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|err| format!("failed to create csv dir `{source}`: {err}"))?;
            }
            std::fs::write(&path, &bytes)
                .map_err(|err| format!("failed to save csv `{source}`: {err}"))?;
        } else {
            save_asset(source, &bytes)
                .map_err(|err| format!("failed to save csv `{source}`: {err}"))?;
        }
        self.csv_cache
            .lock()
            .expect("resource api csv cache mutex poisoned")
            .remove(&perro_ids::string_to_u64(source));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use perro_io::{ProjectRoot, set_project_root};
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn unique_temp_dir(label: &str) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("perro_runtime_{label}_{ts}"))
    }

    fn setup_csv_project() -> PathBuf {
        let root = unique_temp_dir("csv");
        std::fs::create_dir_all(root.join("res/data")).expect("failed to create test res dir");
        std::fs::write(
            root.join("res/data/items.csv"),
            "id,name,power\nsword,Sword,10\npotion,Potion,0\n",
        )
        .expect("failed to write csv");
        set_project_root(ProjectRoot::Disk {
            root: root.clone(),
            name: "CsvTest".to_string(),
        });
        root
    }

    #[test]
    fn csv_loads_dev_table_and_caches() {
        let _project_root_guard = crate::rs_ctx::PROJECT_ROOT_TEST_LOCK.lock().unwrap();
        let _root = setup_csv_project();
        let api = RuntimeResourceApi::new(None, None, None, None, None, None, None, None);

        let table = api.load_csv_source("res://data/items.csv");
        assert_eq!(table.row_count(), 2);
        assert_eq!(
            table.find_primary("sword").and_then(|row| row.get(1)),
            Some("Sword")
        );

        let cached = api.load_csv_source("res://data/items.csv");
        assert!(std::ptr::eq(table, cached));
    }

    #[test]
    fn csv_loads_static_table_by_hash() {
        static HEADERS: [perro_csv::CsvCell; 2] = [
            perro_csv::CsvCell::new("id", perro_ids::hash_str!("id")),
            perro_csv::CsvCell::new("name", perro_ids::hash_str!("name")),
        ];
        static ROW_CELLS: [perro_csv::CsvCell; 2] = [
            perro_csv::CsvCell::new("sword", perro_ids::hash_str!("sword")),
            perro_csv::CsvCell::new("Sword", perro_ids::hash_str!("Sword")),
        ];
        static ROWS: [perro_csv::CsvRow; 1] = [perro_csv::CsvRow::new(&ROW_CELLS)];
        static INDEX: [perro_csv::CsvRowIndex; 1] = [perro_csv::CsvRowIndex::new(
            perro_ids::hash_str!("sword"),
            0,
        )];
        static TABLE: perro_csv::Csv = perro_csv::Csv::new(&HEADERS, &ROWS, &INDEX);

        fn lookup(hash: u64) -> &'static perro_csv::Csv {
            if hash == perro_ids::hash_str!("res://data/items.csv") {
                &TABLE
            } else {
                &perro_csv::EMPTY_CSV
            }
        }

        let api = RuntimeResourceApi::new(None, None, None, None, None, None, Some(lookup), None);
        let table = api.load_csv_source("res://data/items.csv");
        assert_eq!(
            table.find_primary("sword").and_then(|row| row.get(1)),
            Some("Sword")
        );
    }

    #[test]
    fn csv_saves_buf_to_disk() {
        let _project_root_guard = crate::rs_ctx::PROJECT_ROOT_TEST_LOCK.lock().unwrap();
        let root = setup_csv_project();
        let api = RuntimeResourceApi::new(None, None, None, None, None, None, None, None);
        let mut csv = perro_csv::CsvBuf::new(["id", "name", "power"]);
        csv.push_row(["axe", "Axe", "14"]).unwrap();
        csv.push_row(["bow", "Bow", "8"]).unwrap();

        api.save_csv_source("res://data/generated.csv", &csv)
            .unwrap();

        let saved = std::fs::read_to_string(root.join("res/data/generated.csv")).unwrap();
        assert!(saved.contains("id,name,power"));
        assert!(saved.contains("axe,Axe,14"));
    }
}
