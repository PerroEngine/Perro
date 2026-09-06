use criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
use perro_ids::ScriptMemberID;
use perro_scripting::nested_vars::set_nested_by_hash;
use perro_variant::{DeriveVariant, Variant, params};
use std::{collections::BTreeMap, rc::Rc, sync::Arc};

fn array_decode<const N: usize>(c: &mut Criterion) {
    let value = [7_i32; N].to_variant();
    c.bench_function(&format!("variant_hotpaths/array_decode_{N}"), |b| {
        b.iter(|| black_box(<[i32; N]>::from_variant(black_box(&value))))
    });
}

// Pre-audit implementation, for a same-run check when host load changes.
#[inline]
fn heap_array_decode<T: DeriveVariant, const N: usize>(value: &Variant) -> Option<[T; N]> {
    let items = value.as_array()?;
    if items.len() != N {
        return None;
    }
    let mut out = Vec::with_capacity(N);
    for item in items {
        out.push(T::from_variant(item)?);
    }
    let boxed: Box<[T]> = out.into_boxed_slice();
    let boxed: Box<[T; N]> = boxed.try_into().ok()?;
    Some(*boxed)
}

fn bench_hotpaths(c: &mut Criterion) {
    array_decode::<3>(c);
    array_decode::<16>(c);
    array_decode::<128>(c);
    let large_array = [7_i32; 128].to_variant();
    c.bench_function("variant_hotpaths/array_decode_128_reference", |b| {
        b.iter(|| black_box(heap_array_decode::<i32, 128>(black_box(&large_array))))
    });

    let payload = Variant::Array((0..128_i32).map(Variant::from).collect());
    c.bench_function("variant_hotpaths/unique_arc_encode", |b| {
        b.iter_batched(
            || Arc::new(payload.clone()),
            |value| black_box(Variant::from(black_box(value))),
            BatchSize::SmallInput,
        )
    });
    c.bench_function("variant_hotpaths/unique_rc_encode", |b| {
        b.iter_batched(
            || Rc::new(payload.clone()),
            |value| black_box(Variant::from(black_box(value))),
            BatchSize::SmallInput,
        )
    });
    let shared = Arc::new(payload);
    c.bench_function("variant_hotpaths/shared_arc_encode", |b| {
        b.iter(|| black_box(Variant::from(black_box(Arc::clone(&shared)))))
    });

    let mut root = Variant::Object(
        (0..64)
            .map(|i| (Arc::from(format!("field_{i:02}")), Variant::from(i)))
            .collect::<BTreeMap<_, _>>(),
    );
    let last = ScriptMemberID::from_string("root.field_63");
    c.bench_function("variant_hotpaths/nested_set_64", |b| {
        b.iter(|| {
            let mut value = Some(Variant::from(7_i32));
            black_box(set_nested_by_hash(
                "root",
                &mut root,
                black_box(last),
                &mut value,
                &[],
            ))
        })
    });
    c.bench_function("variant_hotpaths/params_scalars", |b| {
        b.iter(|| {
            black_box(params![black_box(1_i32), black_box(2_f32), black_box(true)]);
        })
    });
}

criterion_group!(benches, bench_hotpaths);
criterion_main!(benches);
