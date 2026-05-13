use crate::ResPathSource;
use perro_csv::{PerroCsv, PerroCsvBuf};

pub trait CsvAPI {
    fn load_csv_source_hashed(&self, source_hash: u64, source: Option<&str>) -> &'static PerroCsv;
    fn save_csv_source(&self, source: &str, csv: &PerroCsvBuf) -> Result<(), String>;
    fn save_csv_source_hashed(
        &self,
        source_hash: u64,
        source: &str,
        csv: &PerroCsvBuf,
    ) -> Result<(), String> {
        let _ = source_hash;
        self.save_csv_source(source, csv)
    }

    fn load_csv_source(&self, source: &str) -> &'static PerroCsv {
        self.load_csv_source_hashed(perro_ids::string_to_u64(source), Some(source))
    }
}

pub struct CsvModule<'res, R: CsvAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: CsvAPI + ?Sized> CsvModule<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    #[inline]
    pub fn load<S: ResPathSource>(&self, source: S) -> &'static PerroCsv {
        self.api.load_csv_source(source.as_res_path_str())
    }

    #[inline]
    pub fn load_hashed(&self, source_hash: u64) -> &'static PerroCsv {
        self.api.load_csv_source_hashed(source_hash, None)
    }

    #[inline]
    pub fn load_hashed_with_source<S: ResPathSource>(
        &self,
        source_hash: u64,
        source: S,
    ) -> &'static PerroCsv {
        self.api
            .load_csv_source_hashed(source_hash, Some(source.as_res_path_str()))
    }

    #[inline]
    pub fn save<S: ResPathSource>(&self, source: S, csv: &PerroCsvBuf) -> Result<(), String> {
        self.api.save_csv_source(source.as_res_path_str(), csv)
    }

    #[inline]
    pub fn save_hashed<S: ResPathSource>(
        &self,
        source_hash: u64,
        source: S,
        csv: &PerroCsvBuf,
    ) -> Result<(), String> {
        self.api
            .save_csv_source_hashed(source_hash, source.as_res_path_str(), csv)
    }
}

#[macro_export]
macro_rules! csv_load {
    ($res:expr, $source:literal) => {{
        const __HASH: u64 = $crate::__perro_string_to_u64($source);
        $res.Csv().load_hashed_with_source(__HASH, $source)
    }};
    ($res:expr, $source:expr) => {
        $res.Csv().load($source)
    };
}

#[macro_export]
macro_rules! csv_save {
    ($res:expr, $source:literal, $csv:expr) => {{
        const __HASH: u64 = $crate::__perro_string_to_u64($source);
        $res.Csv().save_hashed(__HASH, $source, $csv)
    }};
    ($res:expr, $source:expr, $csv:expr) => {
        $res.Csv().save($source, $csv)
    };
}
