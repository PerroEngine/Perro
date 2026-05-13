use std::{
    collections::HashMap,
    hash::{BuildHasherDefault, Hasher},
    sync::{OnceLock, RwLock},
};

const INVALID_COL: usize = usize::MAX;
type U64HashMap<V> = HashMap<u64, V, BuildHasherDefault<U64IdentityHasher>>;

#[derive(Default)]
struct U64IdentityHasher(u64);

impl Hasher for U64IdentityHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        let mut out = 0u64;
        for (idx, byte) in bytes.iter().take(8).enumerate() {
            out |= (*byte as u64) << (idx * 8);
        }
        self.0 = out;
    }

    fn write_u64(&mut self, i: u64) {
        self.0 = i;
    }

    fn write_usize(&mut self, i: usize) {
        self.0 = i as u64;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CsvCell {
    pub text: &'static str,
    pub hash: u64,
}

impl CsvCell {
    pub const fn new(text: &'static str, hash: u64) -> Self {
        Self { text, hash }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CsvRow {
    pub cells: &'static [CsvCell],
}

impl CsvRow {
    pub const fn new(cells: &'static [CsvCell]) -> Self {
        Self { cells }
    }

    #[inline]
    pub fn get(&self, col: usize) -> Option<&'static str> {
        self.cells.get(col).map(|cell| cell.text)
    }

    #[inline]
    pub fn get_hash(&self, col: usize) -> Option<u64> {
        self.cells.get(col).map(|cell| cell.hash)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CsvRowIndex {
    pub key_hash: u64,
    pub row: usize,
}

impl CsvRowIndex {
    pub const fn new(key_hash: u64, row: usize) -> Self {
        Self { key_hash, row }
    }
}

#[derive(Debug)]
pub struct PerroCsv {
    pub headers: &'static [CsvCell],
    pub rows: &'static [CsvRow],
    pub primary_index: &'static [CsvRowIndex],
    primary_lookup: OnceLock<U64HashMap<usize>>,
    query_indexes: OnceLock<RwLock<HashMap<usize, CsvColumnHashIndex>>>,
}

impl PerroCsv {
    pub const fn new(
        headers: &'static [CsvCell],
        rows: &'static [CsvRow],
        primary_index: &'static [CsvRowIndex],
    ) -> Self {
        Self {
            headers,
            rows,
            primary_index,
            primary_lookup: OnceLock::new(),
            query_indexes: OnceLock::new(),
        }
    }

    pub const fn empty() -> Self {
        Self {
            headers: &[],
            rows: &[],
            primary_index: &[],
            primary_lookup: OnceLock::new(),
            query_indexes: OnceLock::new(),
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    #[inline]
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    #[inline]
    pub fn col_count(&self) -> usize {
        self.headers.len()
    }

    #[inline]
    pub fn headers(&self) -> &'static [CsvCell] {
        self.headers
    }

    #[inline]
    pub fn rows(&self) -> &'static [CsvRow] {
        self.rows
    }

    #[inline]
    pub fn row(&self, row: usize) -> Option<&'static CsvRow> {
        self.rows.get(row)
    }

    #[inline]
    pub fn header_index(&self, name: &str) -> Option<usize> {
        self.header_index_hash(perro_ids::string_to_u64(name))
    }

    #[inline]
    pub fn header_index_hash(&self, name_hash: u64) -> Option<usize> {
        self.headers.iter().position(|cell| cell.hash == name_hash)
    }

    #[inline]
    pub fn get(&self, row: usize, col: usize) -> Option<&'static str> {
        self.rows.get(row).and_then(|row| row.get(col))
    }

    #[inline]
    pub fn get_by_header(&self, row: usize, header: &str) -> Option<&'static str> {
        self.get_by_header_hash(row, perro_ids::string_to_u64(header))
    }

    #[inline]
    pub fn get_by_header_hash(&self, row: usize, header_hash: u64) -> Option<&'static str> {
        let col = self.header_index_hash(header_hash)?;
        self.get(row, col)
    }

    #[inline]
    pub fn find_primary_hash(&self, key_hash: u64) -> Option<&'static CsvRow> {
        let row = self.primary_lookup().get(&key_hash).copied()?;
        self.rows.get(row)
    }

    #[inline]
    pub fn find_primary(&self, key: &str) -> Option<&'static CsvRow> {
        self.find_primary_hash(perro_ids::string_to_u64(key))
    }

    pub fn find_hash(&self, col: usize, key_hash: u64) -> Option<&'static CsvRow> {
        if col == 0 && !self.primary_index.is_empty() {
            return self.find_primary_hash(key_hash);
        }
        self.rows
            .iter()
            .find(|row| row.cells.get(col).is_some_and(|cell| cell.hash == key_hash))
    }

    #[inline]
    pub fn find(&self, col: usize, key: &str) -> Option<&'static CsvRow> {
        self.find_hash(col, perro_ids::string_to_u64(key))
    }

    #[inline]
    pub fn query(&'static self) -> CSVQuery {
        CSVQuery::new(self)
    }

    pub fn to_buf(&self) -> PerroCsvBuf {
        PerroCsvBuf::from_static(self)
    }

    fn hash_index(&'static self, col: usize) -> CsvColumnHashIndex {
        let indexes = self
            .query_indexes
            .get_or_init(|| RwLock::new(HashMap::new()));
        if let Some(index) = indexes
            .read()
            .expect("csv query index rwlock poisoned")
            .get(&col)
            .cloned()
        {
            return index;
        }
        let mut write = indexes.write().expect("csv query index rwlock poisoned");
        if let Some(index) = write.get(&col).cloned() {
            return index;
        }
        let index = CsvColumnHashIndex::build(self, col);
        write.insert(col, index.clone());
        index
    }

    fn primary_lookup(&self) -> &U64HashMap<usize> {
        self.primary_lookup.get_or_init(|| {
            let mut lookup = U64HashMap::default();
            lookup.reserve(self.primary_index.len());
            for entry in self.primary_index {
                lookup.insert(entry.key_hash, entry.row);
            }
            lookup
        })
    }
}

pub static EMPTY_CSV: PerroCsv = PerroCsv::empty();

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PerroCsvBuf {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl PerroCsvBuf {
    pub fn new(headers: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            headers: headers.into_iter().map(Into::into).collect(),
            rows: Vec::new(),
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_reader(bytes);
        let headers = reader
            .headers()
            .map_err(|err| format!("failed to parse csv headers: {err}"))?
            .iter()
            .map(|value| value.trim().to_string())
            .collect();
        let mut out = Self {
            headers,
            rows: Vec::new(),
        };
        for record in reader.records() {
            let record = record.map_err(|err| format!("failed to parse csv row: {err}"))?;
            out.rows.push(
                record
                    .iter()
                    .map(|value| value.trim().to_string())
                    .collect(),
            );
        }
        Ok(out)
    }

    pub fn from_static(table: &PerroCsv) -> Self {
        let headers = table
            .headers
            .iter()
            .map(|cell| cell.text.to_string())
            .collect();
        let rows = table
            .rows
            .iter()
            .map(|row| row.cells.iter().map(|cell| cell.text.to_string()).collect())
            .collect();
        Self { headers, rows }
    }

    #[inline]
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    #[inline]
    pub fn col_count(&self) -> usize {
        self.headers.len()
    }

    #[inline]
    pub fn headers(&self) -> &[String] {
        &self.headers
    }

    #[inline]
    pub fn rows(&self) -> &[Vec<String>] {
        &self.rows
    }

    #[inline]
    pub fn row(&self, row: usize) -> Option<&[String]> {
        self.rows.get(row).map(Vec::as_slice)
    }

    #[inline]
    pub fn get(&self, row: usize, col: usize) -> Option<&str> {
        self.rows
            .get(row)
            .and_then(|row| row.get(col))
            .map(String::as_str)
    }

    pub fn header_index(&self, name: &str) -> Option<usize> {
        self.headers.iter().position(|header| header == name)
    }

    pub fn get_by_header(&self, row: usize, header: &str) -> Option<&str> {
        let col = self.header_index(header)?;
        self.get(row, col)
    }

    pub fn push_row(
        &mut self,
        row: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<(), String> {
        let row = row.into_iter().map(Into::into).collect::<Vec<_>>();
        if row.len() != self.headers.len() {
            return Err(format!(
                "csv row has {} columns, expected {}",
                row.len(),
                self.headers.len()
            ));
        }
        self.rows.push(row);
        Ok(())
    }

    pub fn set(&mut self, row: usize, col: usize, value: impl Into<String>) -> Result<(), String> {
        let Some(row) = self.rows.get_mut(row) else {
            return Err(format!("csv row index out of range: {row}"));
        };
        let Some(cell) = row.get_mut(col) else {
            return Err(format!("csv column index out of range: {col}"));
        };
        *cell = value.into();
        Ok(())
    }

    pub fn set_by_header(
        &mut self,
        row: usize,
        header: &str,
        value: impl Into<String>,
    ) -> Result<(), String> {
        let col = self
            .header_index(header)
            .ok_or_else(|| format!("csv header not found: {header}"))?;
        self.set(row, col, value)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let mut writer = csv::WriterBuilder::new()
            .has_headers(false)
            .from_writer(Vec::new());
        writer
            .write_record(&self.headers)
            .map_err(|err| format!("failed to write csv headers: {err}"))?;
        for row in &self.rows {
            writer
                .write_record(row)
                .map_err(|err| format!("failed to write csv row: {err}"))?;
        }
        writer
            .into_inner()
            .map_err(|err| format!("failed to finish csv write: {err}"))
    }

    pub fn to_text(&self) -> Result<String, String> {
        String::from_utf8(self.to_bytes()?)
            .map_err(|err| format!("failed to encode csv as utf8: {err}"))
    }
}

impl From<&PerroCsv> for PerroCsvBuf {
    fn from(value: &PerroCsv) -> Self {
        Self::from_static(value)
    }
}

#[derive(Clone, Debug)]
struct CsvColumnHashIndex {
    entries: Vec<(u64, usize)>,
}

impl CsvColumnHashIndex {
    fn build(table: &PerroCsv, col: usize) -> Self {
        let mut entries = Vec::with_capacity(table.rows.len());
        for (row_idx, row) in table.rows.iter().enumerate() {
            if let Some(cell) = row.cells.get(col) {
                entries.push((cell.hash, row_idx));
            }
        }
        entries.sort_unstable_by_key(|entry| entry.0);
        Self { entries }
    }

    fn rows_for_hash(&self, hash: u64, out: &mut Vec<usize>) {
        let start = self.entries.partition_point(|entry| entry.0 < hash);
        let end = self.entries.partition_point(|entry| entry.0 <= hash);
        out.extend(self.entries[start..end].iter().map(|(_, row)| *row));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CsvLogic {
    And,
    Or,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CsvOrder {
    Asc,
    Desc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CsvCompare {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Clone, Debug)]
enum CsvPredicate {
    Text {
        col: usize,
        op: CsvCompare,
        value: String,
        hash: u64,
    },
    Number {
        col: usize,
        op: CsvCompare,
        value: f64,
    },
    Contains {
        col: usize,
        needle: String,
    },
    StartsWith {
        col: usize,
        prefix: String,
    },
    In {
        col: usize,
        values: Vec<String>,
        hashes: Vec<u64>,
    },
}

#[derive(Clone, Debug)]
struct CsvFilter {
    logic: CsvLogic,
    predicate: CsvPredicate,
}

#[derive(Clone, Copy, Debug)]
struct CsvSort {
    col: usize,
    order: CsvOrder,
    numeric: bool,
}

#[derive(Clone, Debug)]
pub struct CSVQuery {
    table: &'static PerroCsv,
    filters: Vec<CsvFilter>,
    select_cols: Option<Vec<usize>>,
    sort: Option<CsvSort>,
    limit: Option<usize>,
}

impl CSVQuery {
    pub fn new(table: &'static PerroCsv) -> Self {
        Self {
            table,
            filters: Vec::new(),
            select_cols: None,
            sort: None,
            limit: None,
        }
    }

    pub fn select(mut self, cols: &[&str]) -> Self {
        self.select_cols = Some(
            cols.iter()
                .filter_map(|col| self.table.header_index(col))
                .collect(),
        );
        self
    }

    pub fn select_hashes(mut self, cols: &[u64]) -> Self {
        self.select_cols = Some(
            cols.iter()
                .filter_map(|col| self.table.header_index_hash(*col))
                .collect(),
        );
        self
    }

    pub fn where_eq(self, col: &str, value: &str) -> Self {
        self.add_text(CsvLogic::And, col, CsvCompare::Eq, value)
    }

    pub fn where_ne(self, col: &str, value: &str) -> Self {
        self.add_text(CsvLogic::And, col, CsvCompare::Ne, value)
    }

    pub fn where_lt(self, col: &str, value: impl Into<f64>) -> Self {
        self.add_number(CsvLogic::And, col, CsvCompare::Lt, value.into())
    }

    pub fn where_le(self, col: &str, value: impl Into<f64>) -> Self {
        self.add_number(CsvLogic::And, col, CsvCompare::Le, value.into())
    }

    pub fn where_gt(self, col: &str, value: impl Into<f64>) -> Self {
        self.add_number(CsvLogic::And, col, CsvCompare::Gt, value.into())
    }

    pub fn where_ge(self, col: &str, value: impl Into<f64>) -> Self {
        self.add_number(CsvLogic::And, col, CsvCompare::Ge, value.into())
    }

    pub fn where_contains(self, col: &str, needle: &str) -> Self {
        self.add_contains(CsvLogic::And, col, needle)
    }

    pub fn where_starts_with(self, col: &str, prefix: &str) -> Self {
        self.add_starts_with(CsvLogic::And, col, prefix)
    }

    pub fn where_in(self, col: &str, values: &[&str]) -> Self {
        self.add_in(CsvLogic::And, col, values)
    }

    pub fn or_where_eq(self, col: &str, value: &str) -> Self {
        self.add_text(CsvLogic::Or, col, CsvCompare::Eq, value)
    }

    pub fn or_where_ne(self, col: &str, value: &str) -> Self {
        self.add_text(CsvLogic::Or, col, CsvCompare::Ne, value)
    }

    pub fn or_where_lt(self, col: &str, value: impl Into<f64>) -> Self {
        self.add_number(CsvLogic::Or, col, CsvCompare::Lt, value.into())
    }

    pub fn or_where_le(self, col: &str, value: impl Into<f64>) -> Self {
        self.add_number(CsvLogic::Or, col, CsvCompare::Le, value.into())
    }

    pub fn or_where_gt(self, col: &str, value: impl Into<f64>) -> Self {
        self.add_number(CsvLogic::Or, col, CsvCompare::Gt, value.into())
    }

    pub fn or_where_ge(self, col: &str, value: impl Into<f64>) -> Self {
        self.add_number(CsvLogic::Or, col, CsvCompare::Ge, value.into())
    }

    pub fn or_where_contains(self, col: &str, needle: &str) -> Self {
        self.add_contains(CsvLogic::Or, col, needle)
    }

    pub fn or_where_starts_with(self, col: &str, prefix: &str) -> Self {
        self.add_starts_with(CsvLogic::Or, col, prefix)
    }

    pub fn or_where_in(self, col: &str, values: &[&str]) -> Self {
        self.add_in(CsvLogic::Or, col, values)
    }

    pub fn order_by(mut self, col: &str, order: CsvOrder) -> Self {
        self.sort = Some(CsvSort {
            col: self.col(col),
            order,
            numeric: false,
        });
        self
    }

    pub fn order_by_asc(self, col: &str) -> Self {
        self.order_by(col, CsvOrder::Asc)
    }

    pub fn order_by_desc(self, col: &str) -> Self {
        self.order_by(col, CsvOrder::Desc)
    }

    pub fn order_by_num(mut self, col: &str, order: CsvOrder) -> Self {
        self.sort = Some(CsvSort {
            col: self.col(col),
            order,
            numeric: true,
        });
        self
    }

    pub fn order_by_num_asc(self, col: &str) -> Self {
        self.order_by_num(col, CsvOrder::Asc)
    }

    pub fn order_by_num_desc(self, col: &str) -> Self {
        self.order_by_num(col, CsvOrder::Desc)
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn run(&self) -> CSVQueryResult {
        let mut rows = Vec::<usize>::new();
        let candidates = self.candidate_rows();
        let iter: Box<dyn Iterator<Item = usize> + '_> =
            if let Some(candidates) = candidates.as_ref() {
                Box::new(candidates.iter().copied())
            } else {
                Box::new(0..self.table.rows.len())
            };
        for row_idx in iter {
            if self.matches(row_idx) {
                rows.push(row_idx);
            }
        }
        if let Some(sort) = self.sort
            && sort.col != INVALID_COL
        {
            rows.sort_unstable_by(|a, b| self.compare_rows(*a, *b, sort));
        }
        if let Some(limit) = self.limit {
            rows.truncate(limit);
        }
        let select_cols = self
            .select_cols
            .clone()
            .unwrap_or_else(|| (0..self.table.col_count()).collect());
        CSVQueryResult {
            table: self.table,
            rows,
            select_cols,
        }
    }

    fn add_text(mut self, logic: CsvLogic, col: &str, op: CsvCompare, value: &str) -> Self {
        self.filters.push(CsvFilter {
            logic,
            predicate: CsvPredicate::Text {
                col: self.col(col),
                op,
                value: value.to_string(),
                hash: perro_ids::string_to_u64(value),
            },
        });
        self
    }

    fn add_number(mut self, logic: CsvLogic, col: &str, op: CsvCompare, value: f64) -> Self {
        self.filters.push(CsvFilter {
            logic,
            predicate: CsvPredicate::Number {
                col: self.col(col),
                op,
                value,
            },
        });
        self
    }

    fn add_contains(mut self, logic: CsvLogic, col: &str, needle: &str) -> Self {
        self.filters.push(CsvFilter {
            logic,
            predicate: CsvPredicate::Contains {
                col: self.col(col),
                needle: needle.to_string(),
            },
        });
        self
    }

    fn add_starts_with(mut self, logic: CsvLogic, col: &str, prefix: &str) -> Self {
        self.filters.push(CsvFilter {
            logic,
            predicate: CsvPredicate::StartsWith {
                col: self.col(col),
                prefix: prefix.to_string(),
            },
        });
        self
    }

    fn add_in(mut self, logic: CsvLogic, col: &str, values: &[&str]) -> Self {
        self.filters.push(CsvFilter {
            logic,
            predicate: CsvPredicate::In {
                col: self.col(col),
                values: values.iter().map(|value| (*value).to_string()).collect(),
                hashes: values
                    .iter()
                    .map(|value| perro_ids::string_to_u64(value))
                    .collect(),
            },
        });
        self
    }

    fn col(&self, name: &str) -> usize {
        self.table.header_index(name).unwrap_or(INVALID_COL)
    }

    fn matches(&self, row_idx: usize) -> bool {
        let Some(first) = self.filters.first() else {
            return true;
        };
        let mut matched = self.predicate_matches(row_idx, &first.predicate);
        for filter in self.filters.iter().skip(1) {
            if filter.logic == CsvLogic::And && !matched {
                continue;
            }
            if filter.logic == CsvLogic::Or && matched {
                continue;
            }
            let next = self.predicate_matches(row_idx, &filter.predicate);
            matched = match filter.logic {
                CsvLogic::And => matched && next,
                CsvLogic::Or => matched || next,
            };
        }
        matched
    }

    fn compare_rows(&self, a: usize, b: usize, sort: CsvSort) -> std::cmp::Ordering {
        let a = self.table.get(a, sort.col).unwrap_or("");
        let b = self.table.get(b, sort.col).unwrap_or("");
        let ord = if sort.numeric {
            let a = a.parse::<f64>().unwrap_or(f64::NAN);
            let b = b.parse::<f64>().unwrap_or(f64::NAN);
            a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            a.cmp(b)
        };
        match sort.order {
            CsvOrder::Asc => ord,
            CsvOrder::Desc => ord.reverse(),
        }
    }

    fn predicate_matches(&self, row_idx: usize, predicate: &CsvPredicate) -> bool {
        match predicate {
            CsvPredicate::Text {
                col,
                op,
                value,
                hash,
            } => {
                let Some(cell) = self.table.row(row_idx).and_then(|row| row.cells.get(*col)) else {
                    return false;
                };
                if matches!(op, CsvCompare::Eq | CsvCompare::Ne) {
                    return compare_bool(cell.hash == *hash, *op);
                }
                compare_ord(cell.text.cmp(value.as_str()), *op)
            }
            CsvPredicate::Number { col, op, value } => {
                let Some(actual) = self
                    .table
                    .get(row_idx, *col)
                    .and_then(|v| v.parse::<f64>().ok())
                else {
                    return false;
                };
                let Some(ord) = actual.partial_cmp(value) else {
                    return false;
                };
                compare_ord(ord, *op)
            }
            CsvPredicate::Contains { col, needle } => self
                .table
                .get(row_idx, *col)
                .is_some_and(|value| value.contains(needle)),
            CsvPredicate::StartsWith { col, prefix } => self
                .table
                .get(row_idx, *col)
                .is_some_and(|value| value.starts_with(prefix)),
            CsvPredicate::In {
                col,
                values,
                hashes,
            } => {
                let Some(cell) = self.table.row(row_idx).and_then(|row| row.cells.get(*col)) else {
                    return false;
                };
                hashes.contains(&cell.hash)
                    || values.iter().any(|value| value.as_str() == cell.text)
            }
        }
    }

    fn candidate_rows(&self) -> Option<Vec<usize>> {
        let seed = self.filters.iter().find_map(|filter| {
            if filter.logic != CsvLogic::And {
                return None;
            }
            match &filter.predicate {
                CsvPredicate::Text {
                    col,
                    op: CsvCompare::Eq,
                    hash,
                    ..
                } if *col != INVALID_COL => Some((*col, CandidateSeed::One(*hash))),
                CsvPredicate::In { col, hashes, .. } if *col != INVALID_COL => {
                    Some((*col, CandidateSeed::Many(hashes.as_slice())))
                }
                _ => None,
            }
        })?;

        let (col, seed) = seed;
        let index = self.table.hash_index(col);
        let mut rows = Vec::new();
        match seed {
            CandidateSeed::One(hash) => index.rows_for_hash(hash, &mut rows),
            CandidateSeed::Many(hashes) => {
                for hash in hashes {
                    index.rows_for_hash(*hash, &mut rows);
                }
                rows.sort_unstable();
                rows.dedup();
            }
        }
        Some(rows)
    }
}

enum CandidateSeed<'a> {
    One(u64),
    Many(&'a [u64]),
}

fn compare_bool(equal: bool, op: CsvCompare) -> bool {
    match op {
        CsvCompare::Eq => equal,
        CsvCompare::Ne => !equal,
        _ => false,
    }
}

fn compare_ord(ord: std::cmp::Ordering, op: CsvCompare) -> bool {
    match op {
        CsvCompare::Eq => ord == std::cmp::Ordering::Equal,
        CsvCompare::Ne => ord != std::cmp::Ordering::Equal,
        CsvCompare::Lt => ord == std::cmp::Ordering::Less,
        CsvCompare::Le => ord != std::cmp::Ordering::Greater,
        CsvCompare::Gt => ord == std::cmp::Ordering::Greater,
        CsvCompare::Ge => ord != std::cmp::Ordering::Less,
    }
}

#[derive(Clone, Debug)]
pub struct CSVQueryResult {
    table: &'static PerroCsv,
    rows: Vec<usize>,
    select_cols: Vec<usize>,
}

impl CSVQueryResult {
    #[inline]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    #[inline]
    pub fn row(&self, index: usize) -> Option<CSVQueryRow<'_>> {
        let row = *self.rows.get(index)?;
        Some(CSVQueryRow {
            table: self.table,
            row,
            select_cols: &self.select_cols,
        })
    }

    #[inline]
    pub fn iter(&self) -> CSVQueryRows<'_> {
        CSVQueryRows {
            result: self,
            index: 0,
        }
    }
}

pub struct CSVQueryRows<'a> {
    result: &'a CSVQueryResult,
    index: usize,
}

impl<'a> Iterator for CSVQueryRows<'a> {
    type Item = CSVQueryRow<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let row = self.result.row(self.index)?;
        self.index += 1;
        Some(row)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CSVQueryRow<'a> {
    table: &'static PerroCsv,
    row: usize,
    select_cols: &'a [usize],
}

impl CSVQueryRow<'_> {
    #[inline]
    pub fn source_row(&self) -> usize {
        self.row
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.select_cols.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.select_cols.is_empty()
    }

    #[inline]
    pub fn get(&self, selected_col: usize) -> Option<&'static str> {
        let col = *self.select_cols.get(selected_col)?;
        self.table.get(self.row, col)
    }

    #[inline]
    pub fn get_header(&self, header: &str) -> Option<&'static str> {
        self.table.get_by_header(self.row, header)
    }
}

pub fn parse_csv_static(bytes: &[u8]) -> Result<&'static PerroCsv, String> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(bytes);
    let headers = reader
        .headers()
        .map_err(|err| format!("failed to parse csv headers: {err}"))?
        .clone();

    let row_capacity = bytes.iter().filter(|&&byte| byte == b'\n').count();
    let mut interner = LocalCsvInterner::new(row_capacity);

    let headers = leak_cells(headers.iter().map(|value| value.trim()), &mut interner);
    let mut rows = Vec::<CsvRow>::with_capacity(row_capacity.saturating_sub(1));
    let mut index_rows = Vec::<CsvRowIndex>::with_capacity(row_capacity.saturating_sub(1));
    let mut primary_seen = HashMap::<u64, usize>::new();

    for record in reader.records() {
        let record = record.map_err(|err| format!("failed to parse csv row: {err}"))?;
        let cells = leak_cells(record.iter().map(|value| value.trim()), &mut interner);
        let row_idx = rows.len();
        if let Some(first) = cells.first()
            && !primary_seen.contains_key(&first.hash)
        {
            primary_seen.insert(first.hash, row_idx);
            index_rows.push(CsvRowIndex::new(first.hash, row_idx));
        }
        rows.push(CsvRow::new(cells));
    }
    index_rows.sort_by_key(|entry| entry.key_hash);

    Ok(Box::leak(Box::new(PerroCsv::new(
        headers,
        Box::leak(rows.into_boxed_slice()),
        Box::leak(index_rows.into_boxed_slice()),
    ))))
}

struct LocalCsvInterner {
    values: HashMap<&'static str, &'static str>,
}

impl LocalCsvInterner {
    fn new(row_capacity: usize) -> Self {
        Self {
            values: HashMap::with_capacity(row_capacity.saturating_mul(2).max(64)),
        }
    }

    fn intern(&mut self, value: &str) -> &'static str {
        if let Some(existing) = self.values.get(value).copied() {
            return existing;
        }
        let leaked = Box::leak(value.to_string().into_boxed_str());
        self.values.insert(leaked, leaked);
        leaked
    }
}

fn leak_cells<'a>(
    values: impl Iterator<Item = &'a str>,
    interner: &mut LocalCsvInterner,
) -> &'static [CsvCell] {
    let cells: Vec<CsvCell> = values
        .map(|value| {
            let text = interner.intern(value);
            CsvCell::new(text, perro_ids::string_to_u64(text))
        })
        .collect();
    Box::leak(cells.into_boxed_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_finds_primary_row() {
        let csv = parse_csv_static(b"id,name\nsword,Sword\npotion,Potion\n").unwrap();
        assert_eq!(csv.row_count(), 2);
        assert_eq!(csv.get_by_header(0, "name"), Some("Sword"));
        assert_eq!(
            csv.find_primary("potion").and_then(|row| row.get(1)),
            Some("Potion")
        );
    }

    #[test]
    fn query_filters_sorts_selects_and_limits() {
        let csv = parse_csv_static(
            b"id,name,kind,power,rarity\n\
              sword,Sword,weapon,10,common\n\
              axe,Axe,weapon,14,rare\n\
              potion,Potion,consumable,0,common\n\
              bow,Bow,weapon,8,common\n",
        )
        .unwrap();

        let result = CSVQuery::new(csv)
            .where_eq("kind", "weapon")
            .where_ge("power", 9.0)
            .where_in("rarity", &["common", "rare"])
            .select(&["id", "power"])
            .order_by_num_desc("power")
            .limit(2)
            .run();

        assert_eq!(result.len(), 2);
        let rows: Vec<_> = result
            .iter()
            .map(|row| (row.get(0).unwrap(), row.get(1).unwrap()))
            .collect();
        assert_eq!(rows, vec![("axe", "14"), ("sword", "10")]);
    }

    #[test]
    fn query_supports_or_contains_and_starts_with() {
        let csv = parse_csv_static(
            b"id,name,kind,power\n\
              sword,Iron Sword,weapon,10\n\
              potion,Small Potion,consumable,0\n\
              scroll,Fire Scroll,magic,3\n",
        )
        .unwrap();

        let result = csv
            .query()
            .where_starts_with("name", "Iron")
            .or_where_contains("name", "Scroll")
            .order_by_asc("id")
            .run();

        let ids: Vec<_> = result
            .iter()
            .map(|row| row.get_header("id").unwrap())
            .collect();
        assert_eq!(ids, vec!["scroll", "sword"]);
    }

    #[test]
    fn csv_buf_builds_edits_and_writes() {
        let mut csv = PerroCsvBuf::new(["id", "name", "note"]);
        csv.push_row(["sword", "Sword", "plain"]).unwrap();
        csv.push_row(["potion", "Potion", "has,comma"]).unwrap();
        csv.set_by_header(0, "note", "sharp").unwrap();

        assert_eq!(csv.row_count(), 2);
        assert_eq!(csv.get_by_header(1, "note"), Some("has,comma"));

        let text = csv.to_text().unwrap();
        assert!(text.contains("id,name,note"));
        assert!(text.contains("potion,Potion,\"has,comma\""));

        let parsed = PerroCsvBuf::from_bytes(text.as_bytes()).unwrap();
        assert_eq!(parsed, csv);
    }

    #[test]
    fn csv_promotes_to_buf() {
        let csv = parse_csv_static(b"id,name\nsword,Sword\npotion,Potion\n").unwrap();
        let mut buf = csv.to_buf();

        assert_eq!(buf.get_by_header(0, "name"), Some("Sword"));
        buf.set_by_header(1, "name", "Big Potion").unwrap();
        assert_eq!(buf.get_by_header(1, "name"), Some("Big Potion"));

        let from_ref = PerroCsvBuf::from(csv);
        assert_eq!(from_ref.get_by_header(1, "name"), Some("Potion"));
    }
}
