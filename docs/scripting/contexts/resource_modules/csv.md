# CSV Module

CSV files under `res/` are runtime data tables.

Use them for item data, balance values, quest rows, spawn tables, dialog tables, and other small database-like files.

## Load

```rust
let items = csv_load!(ctx.res, "res://data/items.csv");
```

Equivalent method form:

```rust
let items = ctx.res.Csv().load("res://data/items.csv");
```

`csv_load!` hashes literal paths at compile time.

## Mutable Buffers

`PerroCsv` is immutable.

Use `PerroCsvBuf` when code needs to build, edit, or save CSV data.

```rust
let mut generated = PerroCsvBuf::new(["id", "name", "power"]);
generated.push_row(["axe", "Axe", "14"])?;
generated.set_by_header(0, "power", "16")?;

csv_save!(ctx.res, "res://data/generated.csv", &generated)?;
```

Promote loaded CSV to owned buffer:

```rust
let items = csv_load!(ctx.res, "res://data/items.csv");
let mut editable = items.to_buf();
editable.set_by_header(0, "name", "Iron Sword")?;

csv_save!(ctx.res, "res://data/items.csv", &editable)?;
```

Equivalent method form:

```rust
ctx.res.Csv().save("res://data/generated.csv", &generated)?;
```

Common buffer methods:

- `PerroCsvBuf::new(headers)`
- `PerroCsvBuf::from_bytes(bytes)`
- `PerroCsvBuf::from_static(table)`
- `PerroCsv::to_buf()`
- `push_row(row)`
- `set(row, col, value)`
- `set_by_header(row, header, value)`
- `to_bytes()`
- `to_text()`

## Access

```rust
let sword = items.find_primary("sword");
let name = sword.and_then(|row| row.get(1));
let power = sword.and_then(|row| row.get(3));

let rarity = items.get_by_header(0, "rarity");
```

## Query

```rust
let weapons = CSVQuery::new(items)
    .where_eq("kind", "weapon")
    .where_ge("power", 10.0)
    .where_in("rarity", &["common", "rare"])
    .select(&["id", "name", "power"])
    .order_by_num_desc("power")
    .limit(8)
    .run();

for row in weapons.iter() {
    let id = row.get(0);
    let name = row.get(1);
}
```

Query ops:

- `where_eq`, `where_ne`
- `where_lt`, `where_le`, `where_gt`, `where_ge`
- `where_contains`
- `where_starts_with`
- `where_in`
- `or_where_*` variants
- `select`
- `order_by_asc`, `order_by_desc`
- `order_by_num_asc`, `order_by_num_desc`
- `limit`

Filters combine left-to-right.

Use `where_*` for `AND`.

Use `or_where_*` for `OR`.

`where_lt/le/gt/ge` compare numbers.

`order_by_*` compares text.

`order_by_num_*` compares numbers.

Result rows keep source row access:

```rust
let row = weapons.row(0);
let source_row_index = row.map(|row| row.source_row());
let name_by_selected_col = row.and_then(|row| row.get(1));
let name_by_header = row.and_then(|row| row.get_header("name"));
```

Common methods:

- `row_count()`
- `col_count()`
- `headers()`
- `rows()`
- `row(index)`
- `get(row, col)`
- `get_by_header(row, header)`
- `find_primary(key)`
- `find_primary_hash(key_hash)`
- `find(col, key)`
- `find_hash(col, key_hash)`

## Format

First row is headers.

First column is treated as the primary key.

Example:

```csv
id,name,kind,power,cost
sword,Sword,weapon,10,25
potion,Potion,consumable,0,8
```

## Build Behavior

Dev mode reads and parses the CSV once, then caches the table.

Static builds emit CSV tables as Rust static data and return `&'static PerroCsv`.

Release lookup avoids file IO, CSV parse work, and per-load allocation.

Missing or invalid CSV returns an empty table.

## Bench Snapshot

Local `perro_csv` bench uses 250,000 rows and 8 columns.

```powershell
cargo bench -p perro_csv --bench huge_csv -- --sample-size 10 --warm-up-time 1 --measurement-time 2
```

Snapshot:

- primary string find batch: ~5.6 us
- primary hash find batch: ~3.4 us
- header get batch: ~2.8 us
- filter/sort/limit query: ~1.3 ms after lazy column index warmup
