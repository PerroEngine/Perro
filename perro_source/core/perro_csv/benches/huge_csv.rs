use criterion::{Criterion, black_box, criterion_group, criterion_main};
use perro_csv::{CSVQuery, parse_csv_static};

fn huge_csv_bench(c: &mut Criterion) {
    const ROWS: usize = 250_000;
    const COLS: usize = 8;
    let mut src = String::from("id,name,kind,power,cost,rarity,zone,flag\n");
    for row in 0..ROWS {
        src.push_str(&format!(
            "item_{row},Name {row},kind_{}, {}, {}, rarity_{}, zone_{}, {}\n",
            row % 64,
            row % 1000,
            row % 500,
            row % 7,
            row % 32,
            row % 2
        ));
    }
    let csv = parse_csv_static(src.as_bytes()).expect("parse huge csv");
    assert_eq!(csv.row_count(), ROWS);
    assert_eq!(csv.col_count(), COLS);
    let primary_keys: Vec<String> = (0..ROWS)
        .step_by(997)
        .map(|row| format!("item_{row}"))
        .collect();
    let primary_hashes: Vec<u64> = primary_keys
        .iter()
        .map(|key| perro_ids::string_to_u64(key))
        .collect();

    c.bench_function("huge_csv_primary_find", |b| {
        b.iter(|| {
            let mut total = 0usize;
            for key in &primary_keys {
                if let Some(found) = csv.find_primary(black_box(key)) {
                    total = total.wrapping_add(found.get(3).unwrap_or("").len());
                }
            }
            black_box(total)
        })
    });

    c.bench_function("huge_csv_primary_hash_find", |b| {
        b.iter(|| {
            let mut total = 0usize;
            for hash in &primary_hashes {
                if let Some(found) = csv.find_primary_hash(black_box(*hash)) {
                    total = total.wrapping_add(found.get(3).unwrap_or("").len());
                }
            }
            black_box(total)
        })
    });

    c.bench_function("huge_csv_header_get", |b| {
        b.iter(|| {
            let mut total = 0usize;
            for row in (0..ROWS).step_by(997) {
                total =
                    total.wrapping_add(csv.get_by_header(row, black_box("rarity")).unwrap().len());
            }
            black_box(total)
        })
    });

    c.bench_function("huge_csv_query_filter_sort_limit", |b| {
        b.iter(|| {
            let result = CSVQuery::new(csv)
                .where_eq("kind", black_box("kind_7"))
                .where_ge("power", black_box(200.0))
                .where_in("rarity", black_box(&["rarity_1", "rarity_2", "rarity_3"]))
                .select(black_box(&["id", "name", "power"]))
                .order_by_num_desc("power")
                .limit(black_box(32))
                .run();
            black_box(result.len())
        })
    });
}

criterion_group!(benches, huge_csv_bench);
criterion_main!(benches);
