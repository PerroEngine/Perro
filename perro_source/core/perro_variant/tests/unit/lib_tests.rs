use std::{collections::BTreeMap, sync::Arc};

use perro_ids::{
    AnimationID, AudioBusID, LightID, MaterialID, MeshID, NodeID, PreloadedSceneID, SignalID,
    TagID, TextureID,
};
use perro_structs::{
    ColorBlindFilter, IVector2, IVector3, Matrix, Matrix3, PostProcessEffect, PostProcessSet,
    SqMatrix, UVector2, UVector3, UnitVector4, Vector2, Vector3, Vector4,
    VisualAccessibilitySettings,
};

use super::*;

// -------------------- Number Tests --------------------

#[test]
fn test_number_type_checks() {
    assert!(Number::I32(42).is_int());
    assert!(Number::U64(100).is_int());
    assert!(!Number::F32(3.5).is_int());

    assert!(Number::F64(2.71).is_float());
    assert!(!Number::I64(42).is_float());
}

#[test]
fn test_number_as_i64_lossy() {
    assert_eq!(Number::I8(-5).as_i64_lossy(), Some(-5i64));
    assert_eq!(Number::I32(42).as_i64_lossy(), Some(42i64));
    assert_eq!(Number::U8(200).as_i64_lossy(), Some(200i64));
    assert_eq!(Number::U32(1000).as_i64_lossy(), Some(1000i64));

    // Out of range values
    assert_eq!(Number::U128(u128::MAX).as_i64_lossy(), None);
    assert_eq!(Number::I128(i128::MAX).as_i64_lossy(), None);

    // Floats return None
    assert_eq!(Number::F32(3.5).as_i64_lossy(), None);
    assert_eq!(Number::F64(2.71).as_i64_lossy(), None);
}

#[test]
fn test_number_as_f64_lossy() {
    assert_eq!(Number::I32(42).as_f64_lossy(), Some(42.0));
    assert_eq!(Number::U64(100).as_f64_lossy(), Some(100.0));
    assert_eq!(Number::F32(3.5).as_f64_lossy(), Some(3.5f32 as f64));
    assert_eq!(Number::F64(2.71).as_f64_lossy(), Some(2.71));
}

// -------------------- Variant Constructors --------------------

#[test]
fn test_variant_null() {
    let v = Variant::null();
    assert!(v.is_null());
    assert_eq!(v, Variant::Null);
}

#[test]
fn test_variant_string() {
    let v = Variant::string("hello");
    assert_eq!(v.as_str(), Some("hello"));

    let v2 = Variant::string(String::from("world"));
    assert_eq!(v2.as_str(), Some("world"));
}

#[test]
fn test_variant_bytes() {
    let v = Variant::bytes([1, 2, 3, 4]);
    assert_eq!(v.as_bytes(), Some(&[1u8, 2, 3, 4][..]));

    let v2 = Variant::bytes(vec![5, 6, 7]);
    assert_eq!(v2.as_bytes(), Some(&[5u8, 6, 7][..]));
}

#[test]
fn test_variant_object() {
    let v = Variant::object();
    assert!(v.as_object().is_some());
    assert_eq!(v.as_object().unwrap().len(), 0);
}

#[test]
fn test_variant_array() {
    let v = Variant::array();
    assert!(v.as_array().is_some());
    assert_eq!(v.as_array().unwrap().len(), 0);
}

#[test]
fn test_variant_parse_helper() {
    let num = Variant::from(42_i32);
    assert_eq!(num.parse::<i32>(), Ok(42));
    assert!(num.parse::<u32>().is_err());

    let list = Variant::Array(vec![Variant::from(1_i32), Variant::from(2_i32)]);
    assert_eq!(list.parse::<Vec<i32>>(), Ok(vec![1, 2]));
}

// -------------------- Variant Accessors --------------------

#[test]
fn test_variant_get_kind() {
    assert_eq!(Variant::Null.get_kind(), VariantKind::Null);
    assert_eq!(Variant::from(true).get_kind(), VariantKind::Bool);
    assert_eq!(Variant::from(7_i32).get_kind(), VariantKind::Number);
    assert_eq!(Variant::from("text").get_kind(), VariantKind::String);
    assert_eq!(Variant::bytes([1_u8]).get_kind(), VariantKind::Bytes);
    assert_eq!(
        Variant::from(NodeID::from_u64(1)).get_kind(),
        VariantKind::ID
    );
    assert_eq!(
        Variant::from(Vector2::new(1.0, 2.0)).get_kind(),
        VariantKind::EngineStruct
    );
    assert_eq!(Variant::Array(Vec::new()).get_kind(), VariantKind::Array);
    assert_eq!(Variant::object().get_kind(), VariantKind::Object);
    assert_eq!(Variant::Null.kind_name(), "Null");
    assert_eq!(VariantKind::Bool.as_str(), "Bool");
}

#[test]
fn test_variant_as_bool() {
    let v = Variant::Bool(true);
    assert_eq!(v.as_bool(), Some(true));

    let v2 = Variant::Null;
    assert_eq!(v2.as_bool(), None);
}

#[test]
fn test_variant_as_number() {
    let v = Variant::Number(Number::I32(42));
    assert_eq!(v.as_number(), Some(Number::I32(42)));
    assert_eq!(v.as_i32(), Some(42));
    assert_eq!(v.as_i64(), None);

    let v2 = Variant::Bool(true);
    assert_eq!(v2.as_number(), None);
    assert_eq!(v2.as_i32(), None);
}

#[test]
fn test_variant_exact_numeric_accessors() {
    assert_eq!(Variant::from(1i8).as_i8(), Some(1));
    assert_eq!(Variant::from(2i16).as_i16(), Some(2));
    assert_eq!(Variant::from(3i32).as_i32(), Some(3));
    assert_eq!(Variant::from(4i64).as_i64(), Some(4));
    assert_eq!(Variant::from(5i128).as_i128(), Some(5));

    assert_eq!(Variant::from(6u8).as_u8(), Some(6));
    assert_eq!(Variant::from(7u16).as_u16(), Some(7));
    assert_eq!(Variant::from(8u32).as_u32(), Some(8));
    assert_eq!(Variant::from(9u64).as_u64(), Some(9));
    assert_eq!(Variant::from(10u128).as_u128(), Some(10));

    assert_eq!(Variant::from(3.5f32).as_f32(), Some(3.5));
    assert_eq!(Variant::from(7.5f64).as_f64(), Some(7.5));

    // Exact typed accessors intentionally do not coerce across numeric variants.
    let n = Variant::from(42i32);
    assert_eq!(n.as_i64(), None);
    assert_eq!(n.as_f32(), None);
    assert_eq!(n.as_u32(), None);
}

#[test]
fn test_variant_as_node() {
    let node = NodeID::from_u32(123);
    let v = Variant::from(node);
    assert_eq!(v.as_node(), Some(node));
}

#[test]
fn test_variant_as_texture() {
    let tex = TextureID::from_u32(456);
    let v = Variant::from(tex);
    assert_eq!(v.as_texture(), Some(tex));
}

#[test]
fn test_variant_as_preloaded_scene() {
    let id = PreloadedSceneID::from_u64(1234);
    let v = Variant::from(id);
    assert_eq!(v.as_preloaded_scene(), Some(id));
}

#[test]
fn test_variant_as_engine_ids() {
    let material = MaterialID::from_u64(10);
    let mesh = MeshID::from_u64(11);
    let animation = AnimationID::from_u64(12);
    let light = LightID::from_u64(13);
    let signal = SignalID::from_u64(15);
    let audio_bus = AudioBusID::from_u64(16);
    let tag = TagID::from_u64(17);

    assert_eq!(Variant::from(material).as_material(), Some(material));
    assert_eq!(Variant::from(mesh).as_mesh(), Some(mesh));
    assert_eq!(Variant::from(animation).as_animation(), Some(animation));
    assert_eq!(Variant::from(light).as_light(), Some(light));
    assert_eq!(Variant::from(signal).as_signal(), Some(signal));
    assert_eq!(Variant::from(audio_bus).as_audio_bus(), Some(audio_bus));
    assert_eq!(Variant::from(tag).as_tag(), Some(tag));
    assert_eq!(Variant::from(material).as_mesh(), None);
}

#[test]
fn test_variant_as_vec2() {
    let vec = Vector2 { x: 1.0, y: 2.0 };
    let v = Variant::from(vec);
    assert_eq!(v.as_vec2(), Some(vec));
}

#[test]
fn test_variant_as_vec3() {
    let vec = Vector3 {
        x: 1.0,
        y: 2.0,
        z: 3.0,
    };
    let v = Variant::from(vec);
    assert_eq!(v.as_vec3(), Some(vec));
}

#[test]
fn test_variant_as_ivec2() {
    let vec = IVector2::new(-1, 2);
    let v = Variant::from(vec);
    assert_eq!(v.as_ivec2(), Some(vec));
}

#[test]
fn test_variant_as_ivec3() {
    let vec = IVector3::new(-1, 2, -3);
    let v = Variant::from(vec);
    assert_eq!(v.as_ivec3(), Some(vec));
}

#[test]
fn test_variant_as_uvec2() {
    let vec = UVector2::new(1, 2);
    let v = Variant::from(vec);
    assert_eq!(v.as_uvec2(), Some(vec));
}

#[test]
fn test_variant_as_uvec3() {
    let vec = UVector3::new(1, 2, 3);
    let v = Variant::from(vec);
    assert_eq!(v.as_uvec3(), Some(vec));
}

#[test]
fn test_variant_as_unit_vec4() {
    let vec = UnitVector4::new([1.0, 0.5, -1.0, 2.0]);
    let v = Variant::from(vec);
    assert_eq!(v.as_unit_vec4(), Some(vec));
    assert_eq!(v.as_unit_vec4().unwrap().to_u8(), [255, 128, 0, 255]);
}

#[test]
fn test_unit_vec4_parse_and_json() {
    let from_vec = Variant::from(Vector4::new(1.0, 0.5, -1.0, 2.0))
        .parse::<UnitVector4>()
        .unwrap();
    assert_eq!(from_vec.to_u8(), [255, 128, 0, 255]);

    let from_array = Variant::Array(vec![
        Variant::from(1.0_f32),
        Variant::from(0.5_f32),
        Variant::from(-1.0_f32),
        Variant::from(2.0_f32),
    ])
    .parse::<UnitVector4>()
    .unwrap();
    assert_eq!(from_array.to_u8(), [255, 128, 0, 255]);

    let json = Variant::from(from_array).to_json_value();
    assert_eq!(json["x"].as_f64(), Some(1.0));
    assert!(json["y"].as_f64().unwrap() > 0.5);
    assert_eq!(json["z"].as_f64(), Some(0.0));
    assert_eq!(json["w"].as_f64(), Some(1.0));
}

#[test]
fn test_matrix_variant_accessors_parse_and_json() {
    let rows = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]];
    let matrix = Matrix::<3, 3>::new(rows);
    let variant = Variant::from(matrix);

    assert_eq!(variant.as_matrix3x3(), Some(matrix));
    assert_eq!(variant.as_matrix3().unwrap().to_rows(), rows);
    assert_eq!(variant.parse::<Matrix<3, 3>>(), Ok(matrix));
    assert_eq!(variant.parse::<Matrix3>().unwrap().to_rows(), rows);

    let json = variant.to_json_value();
    assert_eq!(json[0][1].as_f64(), Some(2.0));
    assert_eq!(json[2][2].as_f64(), Some(9.0));
}

#[test]
fn test_matrix_variant_parse_from_rows_flat_and_object() {
    let row_array = Variant::Array(vec![
        Variant::Array(vec![
            Variant::from(1.0_f32),
            Variant::from(2.0_f32),
            Variant::from(3.0_f32),
            Variant::from(4.0_f32),
        ]),
        Variant::Array(vec![
            Variant::from(5.0_f32),
            Variant::from(6.0_f32),
            Variant::from(7.0_f32),
            Variant::from(8.0_f32),
        ]),
        Variant::Array(vec![
            Variant::from(9.0_f32),
            Variant::from(10.0_f32),
            Variant::from(11.0_f32),
            Variant::from(12.0_f32),
        ]),
        Variant::Array(vec![
            Variant::from(13.0_f32),
            Variant::from(14.0_f32),
            Variant::from(15.0_f32),
            Variant::from(16.0_f32),
        ]),
    ]);
    let expected = Matrix::<4, 4>::new([
        [1.0, 2.0, 3.0, 4.0],
        [5.0, 6.0, 7.0, 8.0],
        [9.0, 10.0, 11.0, 12.0],
        [13.0, 14.0, 15.0, 16.0],
    ]);

    assert_eq!(row_array.parse::<Matrix<4, 4>>(), Ok(expected));

    let flat = Variant::Array(
        (1..=16)
            .map(|v| Variant::from(v as f32))
            .collect::<Vec<_>>(),
    );
    assert_eq!(flat.parse::<Matrix<4, 4>>(), Ok(expected));

    let mut object = BTreeMap::new();
    object.insert(Arc::from("rows"), row_array);
    assert_eq!(
        Variant::Object(object).parse::<Matrix<4, 4>>(),
        Ok(expected)
    );
}

#[test]
fn test_sq_matrix_u8_parse_and_variant() {
    let value = Variant::Array(vec![
        Variant::Array(vec![Variant::from(1_u8), Variant::from(2_u8)]),
        Variant::Array(vec![Variant::from(3_u8), Variant::from(4_u8)]),
    ]);
    let expected = SqMatrix::<2, u8>::new([[1, 2], [3, 4]]);

    assert_eq!(value.parse::<SqMatrix<2, u8>>(), Ok(expected));
    assert_eq!(expected.to_variant(), value);
}

#[test]
fn test_sq_matrix_f32_maps_to_fast_matrix_variants() {
    let m2 = SqMatrix::<2>::new([[1.0, 2.0], [3.0, 4.0]]);
    let m3 = SqMatrix::<3>::identity();
    let m4 = SqMatrix::<4>::identity();

    assert!(m2.to_variant().as_matrix2().is_some());
    assert!(m3.to_variant().as_matrix3().is_some());
    assert!(m4.to_variant().as_matrix4().is_some());

    assert_eq!(m2.to_variant().parse::<SqMatrix<2>>(), Ok(m2));
    assert_eq!(m3.to_variant().parse::<SqMatrix<3>>(), Ok(m3));
    assert_eq!(m4.to_variant().parse::<SqMatrix<4>>(), Ok(m4));
}

#[test]
fn test_matrix_any_size_f32_parse_and_shape() {
    let matrix = Matrix::<2, 3>::new([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]);
    let variant = matrix.to_variant();

    assert_eq!(
        variant.matrix_shape(),
        Some(MatrixShape::new(2, 3, MatrixCellType::F32))
    );
    assert_eq!(variant.parse::<Matrix<2, 3>>(), Ok(matrix));

    let square = SqMatrix::<5>::default();
    let variant = square.to_variant();
    assert_eq!(
        variant.matrix_shape(),
        Some(MatrixShape::new(5, 5, MatrixCellType::F32))
    );
    assert_eq!(variant.parse::<SqMatrix<5>>(), Ok(square));
}

#[test]
fn test_nested_sq_matrix_parse_and_shape() {
    let inner = SqMatrix::<2>::new([[1.0, 2.0], [3.0, 4.0]]);
    let matrix = SqMatrix::<2, SqMatrix<2>>::new([[inner, inner], [inner, inner]]);
    let variant = matrix.to_variant();

    assert_eq!(
        variant.matrix_shape(),
        Some(MatrixShape::new(
            2,
            2,
            MatrixCellType::Matrix(Box::new(MatrixShape::new(2, 2, MatrixCellType::F32)))
        ))
    );
    assert_eq!(variant.parse::<SqMatrix<2, SqMatrix<2>>>(), Ok(matrix));
}

#[test]
fn test_uvec_parse_from_object() {
    let mut vec2 = BTreeMap::new();
    vec2.insert(Arc::from("x"), Variant::from(8_u32));
    vec2.insert(Arc::from("y"), Variant::from(13_u32));
    assert_eq!(
        Variant::Object(vec2).parse::<UVector2>(),
        Ok(UVector2::new(8, 13))
    );

    let mut vec3 = BTreeMap::new();
    vec3.insert(Arc::from("x"), Variant::from(1_i64));
    vec3.insert(Arc::from("y"), Variant::from(2_i64));
    vec3.insert(Arc::from("z"), Variant::from(3_i64));
    assert_eq!(
        Variant::Object(vec3).parse::<UVector3>(),
        Ok(UVector3::new(1, 2, 3))
    );
}

#[test]
fn test_ivec_parse_from_object() {
    let mut vec2 = BTreeMap::new();
    vec2.insert(Arc::from("x"), Variant::from(-8_i32));
    vec2.insert(Arc::from("y"), Variant::from(13_i32));
    assert_eq!(
        Variant::Object(vec2).parse::<IVector2>(),
        Ok(IVector2::new(-8, 13))
    );

    let mut vec3 = BTreeMap::new();
    vec3.insert(Arc::from("x"), Variant::from(-1_i64));
    vec3.insert(Arc::from("y"), Variant::from(2_i64));
    vec3.insert(Arc::from("z"), Variant::from(-3_i64));
    assert_eq!(
        Variant::Object(vec3).parse::<IVector3>(),
        Ok(IVector3::new(-1, 2, -3))
    );
}

#[test]
fn test_variant_as_post_process_set() {
    let mut set = PostProcessSet::new();
    set.add_unnamed(PostProcessEffect::Blur { strength: 1.5 });
    let v = Variant::from(set.clone());
    assert_eq!(v.as_post_process_set(), Some(&set));
}

#[test]
fn test_variant_as_visual_accessibility() {
    let settings =
        VisualAccessibilitySettings::new().with_color_blind(ColorBlindFilter::Protan, 0.7);
    let v = Variant::from(settings);
    assert_eq!(v.as_visual_accessibility_settings(), Some(settings));
}

#[test]
fn test_variant_as_array_mut() {
    let mut v = Variant::Array(vec![Variant::Bool(true)]);

    if let Some(arr) = v.as_array_mut() {
        arr.push(Variant::Bool(false));
    }

    assert_eq!(v.as_array().unwrap().len(), 2);
}

#[test]
fn test_variant_as_object_mut() {
    let mut v = Variant::Object(BTreeMap::new());

    if let Some(obj) = v.as_object_mut() {
        obj.insert(Arc::from("key"), Variant::Bool(true));
    }

    assert_eq!(v.as_object().unwrap().len(), 1);
}

// -------------------- From Implementations --------------------

#[test]
fn test_from_bool() {
    let v: Variant = true.into();
    assert_eq!(v, Variant::Bool(true));
}

#[test]
fn test_from_signed_ints() {
    let v1: Variant = 42i8.into();
    assert_eq!(v1.as_number(), Some(Number::I8(42)));

    let v2: Variant = 1000i16.into();
    assert_eq!(v2.as_number(), Some(Number::I16(1000)));

    let v3: Variant = 100000i32.into();
    assert_eq!(v3.as_number(), Some(Number::I32(100000)));

    let v4: Variant = 10000000000i64.into();
    assert_eq!(v4.as_number(), Some(Number::I64(10000000000)));
}

#[test]
fn test_from_unsigned_ints() {
    let v1: Variant = 200u8.into();
    assert_eq!(v1.as_number(), Some(Number::U8(200)));

    let v2: Variant = 50000u16.into();
    assert_eq!(v2.as_number(), Some(Number::U16(50000)));

    let v3: Variant = 3000000000u32.into();
    assert_eq!(v3.as_number(), Some(Number::U32(3000000000)));
}

#[test]
fn test_from_floats() {
    let v1: Variant = 3.5f32.into();
    if let Variant::Number(Number::F32(f)) = v1 {
        assert!((f - 3.5).abs() < 0.001);
    } else {
        panic!("Expected F32");
    }

    let v2: Variant = 2.5f64.into();
    assert_eq!(v2.as_number(), Some(Number::F64(2.5)));
}

#[test]
fn test_from_string_types() {
    let v1: Variant = "hello".into();
    assert_eq!(v1.as_str(), Some("hello"));

    let v2: Variant = String::from("world").into();
    assert_eq!(v2.as_str(), Some("world"));

    let v3: Variant = Arc::<str>::from("arc").into();
    assert_eq!(v3.as_str(), Some("arc"));
}

#[test]
fn test_from_bytes_types() {
    let v1: Variant = (&[1u8, 2, 3][..]).into();
    assert_eq!(v1.as_bytes(), Some(&[1u8, 2, 3][..]));

    let v2: Variant = vec![4u8, 5, 6].into();
    assert_eq!(v2.as_bytes(), Some(&[4u8, 5, 6][..]));
}

#[test]
fn test_from_vec_variant() {
    let v: Variant = vec![Variant::Bool(true), Variant::Bool(false)].into();
    assert_eq!(v.as_array().unwrap().len(), 2);
}

#[test]
fn test_from_btreemap() {
    let mut map = BTreeMap::new();
    map.insert(Arc::from("key1"), Variant::Bool(true));
    map.insert(Arc::from("key2"), Variant::Bool(false));

    let v: Variant = map.into();
    assert_eq!(v.as_object().unwrap().len(), 2);
}

// -------------------- Integration Tests --------------------

#[test]
fn test_nested_variant_structure() {
    let mut inner_obj = BTreeMap::new();
    inner_obj.insert(Arc::from("x"), Variant::from(10i32));
    inner_obj.insert(Arc::from("y"), Variant::from(20i32));

    let mut outer_obj = BTreeMap::new();
    outer_obj.insert(Arc::from("position"), Variant::Object(inner_obj));
    outer_obj.insert(Arc::from("name"), Variant::string("player"));

    let v = Variant::Object(outer_obj);

    if let Some(obj) = v.as_object() {
        assert!(obj.contains_key(&Arc::from("position")));
        assert!(obj.contains_key(&Arc::from("name")));
    } else {
        panic!("Expected object");
    }
}

#[test]
fn test_variant_array_operations() {
    let mut arr = vec![
        Variant::from(1i32),
        Variant::from(2i32),
        Variant::from(3i32),
    ];
    arr.push(Variant::from(4i32));

    let v = Variant::Array(arr);
    assert_eq!(v.as_array().unwrap().len(), 4);
}

#[test]
fn test_variant_clone() {
    let v1 = Variant::string("test");
    let v2 = v1.clone();

    assert_eq!(v1, v2);
    assert_eq!(v1.as_str(), v2.as_str());
}

#[test]
fn test_number_equality() {
    assert_eq!(Number::I32(42), Number::I32(42));
    assert_ne!(Number::I32(42), Number::I64(42));
    assert_ne!(Number::F32(3.5), Number::F64(3.5));
}

#[test]
fn test_variant_equality() {
    assert_eq!(Variant::Null, Variant::Null);
    assert_eq!(Variant::Bool(true), Variant::Bool(true));
    assert_ne!(Variant::Bool(true), Variant::Bool(false));

    let s1 = Variant::string("test");
    let s2 = Variant::string("test");
    assert_eq!(s1, s2);
}
