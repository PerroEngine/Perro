//! Models generated imported-type fallback: encode root, walk, decode root.
//! Public APIs only, compatible with the audit baseline.
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_ids::ScriptMemberID;
use perro_scripting::nested_vars::{apply_nested_object, get_nested_by_hash, set_nested_by_hash};
use perro_variant::{DeriveVariant, Variant};
use std::{collections::BTreeMap, sync::Arc};

fn nested(c: &mut Criterion) {
    let mut group = c.benchmark_group("audit_nested_fallback");
    for count in [8usize, 128, 4096] {
        let mut typed: BTreeMap<String, i32> = (0..count)
            .map(|index| (format!("field_{index:04}"), index as i32))
            .collect();
        let last_key = format!("field_{:04}", count - 1);
        let member = ScriptMemberID::from_string(&format!("root.{last_key}"));
        group.bench_function(BenchmarkId::new("typed_get_last", count), |b| {
            b.iter(|| {
                black_box(get_nested_by_hash(
                    "root",
                    black_box(&typed).to_variant(),
                    black_box(member),
                    &[],
                ))
            })
        });
        group.bench_function(BenchmarkId::new("typed_set_last", count), |b| {
            b.iter(|| {
                let mut root = black_box(&typed).to_variant();
                let mut value = Some(Variant::from(black_box(7_i32)));
                assert!(set_nested_by_hash(
                    "root",
                    &mut root,
                    member,
                    &mut value,
                    &[]
                ));
                typed = BTreeMap::<String, i32>::from_variant(&root).expect("valid root");
                black_box(&typed);
            })
        });
        group.bench_function(BenchmarkId::new("direct_typed_set_control", count), |b| {
            b.iter(|| {
                *typed.get_mut(black_box(&last_key)).expect("known key") = black_box(7);
            })
        });
        for patches in [1usize, 8, 64] {
            if patches > count {
                continue;
            }
            let incoming = Variant::Object(
                (count - patches..count)
                    .map(|index| (Arc::from(format!("field_{index:04}")), Variant::from(9_i32)))
                    .collect(),
            );
            let mut root = typed.to_variant();
            group.bench_function(
                BenchmarkId::new(format!("variant_patch_{patches}"), count),
                |b| {
                    b.iter(|| {
                        black_box(apply_nested_object(
                            "root",
                            &mut root,
                            black_box(incoming.clone()),
                            &[],
                        ))
                    })
                },
            );
        }
    }
    group.finish();
}

criterion_group!(nested_fallback, nested);
criterion_main!(nested_fallback);
