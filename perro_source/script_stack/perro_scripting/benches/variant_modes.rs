extern crate self as perro_api;

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use perro_scripting::Variant;
use perro_variant::DeriveVariant;

pub mod variant {
    pub use perro_variant::{DeriveVariant, Variant, VariantSchema};
}

#[derive(Clone, PartialEq, Variant)]
#[variant(mode = "object")]
struct ObjectVec3 {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Clone, PartialEq, Variant)]
#[variant(mode = "array")]
struct ArrayVec3 {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Clone, PartialEq, Variant)]
#[variant(tag = "string")]
enum StringTagState {
    Idle,
    Move(ObjectVec3),
}

#[derive(Clone, PartialEq, Variant)]
#[variant(tag = "u16")]
enum U16TagState {
    Idle,
    Move(ArrayVec3),
}

fn bench_variant_modes(c: &mut Criterion) {
    let object = ObjectVec3 {
        x: 1.0,
        y: 2.0,
        z: 3.0,
    };
    let array = ArrayVec3 {
        x: 1.0,
        y: 2.0,
        z: 3.0,
    };
    let st_string = StringTagState::Move(object.clone());
    let st_u16 = U16TagState::Move(array.clone());

    c.bench_function("struct_object_encode", |b| {
        b.iter(|| black_box(<ObjectVec3 as DeriveVariant>::to_variant(black_box(&object))))
    });
    c.bench_function("struct_array_encode", |b| {
        b.iter(|| black_box(<ArrayVec3 as DeriveVariant>::to_variant(black_box(&array))))
    });

    let object_encoded = <ObjectVec3 as DeriveVariant>::to_variant(&object);
    let array_encoded = <ArrayVec3 as DeriveVariant>::to_variant(&array);
    c.bench_function("struct_object_decode", |b| {
        b.iter(|| black_box(<ObjectVec3 as DeriveVariant>::from_variant(black_box(&object_encoded))))
    });
    c.bench_function("struct_array_decode", |b| {
        b.iter(|| black_box(<ArrayVec3 as DeriveVariant>::from_variant(black_box(&array_encoded))))
    });

    c.bench_function("enum_string_tag_encode", |b| {
        b.iter(|| black_box(<StringTagState as DeriveVariant>::to_variant(black_box(&st_string))))
    });
    c.bench_function("enum_u16_tag_encode", |b| {
        b.iter(|| black_box(<U16TagState as DeriveVariant>::to_variant(black_box(&st_u16))))
    });
}

criterion_group!(benches, bench_variant_modes);
criterion_main!(benches);
