# Csv Module

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Context | [Context](#context) |
| Reading Rows | [Reading Rows](#reading-rows) |
| Runtime Bytes | [Runtime Bytes](#runtime-bytes) |
| Practical Example | [Practical Example](#practical-example) |
| API Reference | [API Reference](#api-reference) |
| `load` | [`load`](#load) |
| `load_hashed` | [`load_hashed`](#load_hashed) |
| `load_hashed_with_source` | [`load_hashed_with_source`](#load_hashed_with_source) |
| `save` | [`save`](#save) |
| `save_hashed` | [`save_hashed`](#save_hashed) |
| `csv_load` | [`csv_load`](#csv_load) |
| `csv_save` | [`csv_save`](#csv_save) |

## Purpose

`ctx.res.Csv()` loads spreadsheet tables so designers can author game data in a `.csv` file instead of hard-coding it in Rust. A load returns a `&'static Csv` you can read by row, by header name, or by primary key, and query with filters and sorting. Use it whenever numbers and text should live in a spreadsheet a non-programmer can edit and reload.

## Use Cases

- Loot tables authored in a spreadsheet: load with `csv_load!` then pull a drop's weight and rarity with `Csv::get_by_header` or `Csv::query()`.
- Dialogue lines keyed by id: look up a line by its key column with `Csv::find_primary`, which uses the first column as a fast primary index.
- Enemy or weapon stat sheets: read a row's damage and health columns per spawn with `Csv::row` and `CsvRow::get`.
- Filtered drop rolls: `Csv::query().where_ge("level", 5).order_by_num_desc("weight").run()` to pick from rows matching the player's level.
- Exporting runtime data back to disk (highscores, telemetry) by building a `CsvBuf` and calling `save` / `csv_save!`.
- Localization sheets, which the [Localization](localization.md) module reads from the same `.csv` format.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res.Csv()`
- Loaded tables are `&'static Csv`; construct editable tables with `CsvBuf` before saving.
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Reading Rows

Loading returns a `&'static Csv`. Read it with these methods (all defined on `perro_csv::Csv`):

| Call | Return | Notes |
| --- | --- | --- |
| `csv.row_count()` / `csv.col_count()` | `usize` | Table shape. |
| `csv.get_by_header(row, "damage")` | `Option<&'static str>` | Cell text by header name. |
| `csv.find_primary("goblin")` | `Option<&'static CsvRow>` | Row whose first column equals the key, via the primary index. |
| `csv.find(col, "value")` | `Option<&'static CsvRow>` | First row where column `col` matches. |
| `csv.query()` | `CSVQuery` | Builder for `where_*`, `order_by_*`, `limit`, then `.run()`. |
| `row.get(col)` | `Option<&'static str>` | Cell text on a `CsvRow` by column index. |

A query is built fluently and executed with `.run()`, which returns a `CSVQueryResult` you can `.iter()` over; each `CSVQueryRow` exposes `get_header("name")`.

## Runtime Bytes

Use runtime bytes when CSV data is already in memory, for example a table downloaded at runtime or embedded in a save file.

| Call | Return | Notes |
| --- | --- | --- |
| `ctx.res.Csv().load_bytes(bytes)` | `&'static Csv` | Parses CSV bytes immediately. |
| `csv_load_bytes!(ctx.res, bytes)` | `&'static Csv` | Macro form. |

See [Runtime Bytes Resources](../../../resources/runtime_bytes.md).

## Practical Example

Load a loot table once, then roll a drop for the player's level each time an enemy dies.

```rust
lifecycle!({
    fn on_all_init(&self, ctx: &mut ScriptContext<'_, API>) {
        signal_connect!(ctx.run, ctx.id, signal!("enemy_died"), func!("on_enemy_died"));
    }
});

methods!({
    fn on_enemy_died(&self, ctx: &mut ScriptContext<'_, API>) {
        let loot = csv_load!(ctx.res, "res://data/loot.csv");
        if let Some(row) = loot.find_primary("forest_goblin") {
            // Columns: key, item, weight
            let item = row.get(1).unwrap_or("gold");
            let weight = row.get(2).and_then(|w| w.parse::<f32>().ok()).unwrap_or(1.0);
            let _ = (item, weight);
        }
    }
});
```

## API Reference

### `load`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Csv()` |
| Signature | `pub fn load<S: ResPathSource>(&self, source: S) -> &'static Csv` |
| Params | `source: S` (a `res://...csv` path or compatible source) |
| Returns | `&'static Csv` |
| Use when | Loading a table by path to read rows during gameplay. |
| Fails when / edge behavior | Returns an empty `Csv` (`row_count() == 0`) when the file is missing or cannot be parsed. |

### `load_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Csv()` |
| Signature | `pub fn load_hashed(&self, source_hash: u64) -> &'static Csv` |
| Params | `source_hash: u64` |
| Returns | `&'static Csv` |
| Use when | A precomputed path hash is already available and the source string is not needed. |
| Fails when / edge behavior | Returns an empty `Csv` when no table is registered for the hash. |

### `load_hashed_with_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Csv()` |
| Signature | `pub fn load_hashed_with_source<S: ResPathSource>(&self, source_hash: u64, source: S) -> &'static Csv` |
| Params | `source_hash: u64, source: S` |
| Returns | `&'static Csv` |
| Use when | The `csv_load!` literal path builds a compile-time hash and passes the source for first-load resolution. |
| Fails when / edge behavior | Returns an empty `Csv` when the file is missing or cannot be parsed. |

### `save`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Csv()` |
| Signature | `pub fn save<S: ResPathSource>(&self, source: S, csv: &CsvBuf) -> Result<(), String>` |
| Params | `source: S, csv: &CsvBuf` |
| Returns | `Result<(), String>` |
| Use when | Writing an editable `CsvBuf` back to disk, for example a highscore or export table. |
| Fails when / edge behavior | Returns `Err(String)` when serialization or the write fails. |

### `save_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Csv()` |
| Signature | `pub fn save_hashed<S: ResPathSource>(&self, source_hash: u64, source: S, csv: &CsvBuf) -> Result<(), String>` |
| Params | `source_hash: u64, source: S, csv: &CsvBuf` |
| Returns | `Result<(), String>` |
| Use when | The `csv_save!` literal path builds a compile-time hash. The default implementation ignores the hash and saves by source. |
| Fails when / edge behavior | Same as `save`. |

### `csv_load`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Csv()` |
| Signature | `csv_load!(ctx.res, source)` |
| Params | `ctx.res, source` |
| Returns | `&'static Csv` |
| Use when | Macro form of `load`. A literal path hashes at compile time and calls `load_hashed_with_source`; an expression path calls `load`. |
| Fails when / edge behavior | Returns an empty `Csv` when the file is missing or cannot be parsed. |

### `csv_save`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Csv()` |
| Signature | `csv_save!(ctx.res, source, csv)` |
| Params | `ctx.res, source, csv` |
| Returns | `Result<(), String>` |
| Use when | Macro form of `save`. A literal path hashes at compile time and calls `save_hashed`; an expression path calls `save`. |
| Fails when / edge behavior | Returns `Err(String)` when serialization or the write fails. |
