pub mod variant;
pub use variant::*;

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, sync::Arc};

    use perro_core::structs::{Vector2, Vector3};
    use perro_ids::{NodeID, TextureID};

    use super::*;

    // -------------------- Number Tests --------------------

    #[test]
    fn test_number_type_checks() {
        assert!(Number::I32(42).is_int());
        assert!(Number::U64(100).is_int());
        assert!(!Number::F32(3.14).is_int());

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
        assert_eq!(Number::F32(3.14).as_i64_lossy(), None);
        assert_eq!(Number::F64(2.71).as_i64_lossy(), None);
    }

    #[test]
    fn test_number_as_f64_lossy() {
        assert_eq!(Number::I32(42).as_f64_lossy(), Some(42.0));
        assert_eq!(Number::U64(100).as_f64_lossy(), Some(100.0));
        assert_eq!(Number::F32(3.14).as_f64_lossy(), Some(3.14f32 as f64));
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
        let v = Variant::bytes(&[1, 2, 3, 4]);
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

    // -------------------- Variant Accessors --------------------

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

        let v2 = Variant::Bool(true);
        assert_eq!(v2.as_number(), None);
    }

    #[test]
    fn test_variant_as_node() {
        let node_id = NodeID::from_u32(123);
        let v = Variant::NodeID(node_id);
        assert_eq!(v.as_node(), Some(node_id));
    }

    #[test]
    fn test_variant_as_texture() {
        let tex_id = TextureID::from_u32(456);
        let v = Variant::TextureID(tex_id);
        assert_eq!(v.as_texture(), Some(tex_id));
    }

    #[test]
    fn test_variant_as_vec2() {
        let vec = Vector2 { x: 1.0, y: 2.0 };
        let v = Variant::Vector2(vec);
        assert_eq!(v.as_vec2(), Some(vec));
    }

    #[test]
    fn test_variant_as_vec3() {
        let vec = Vector3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        let v = Variant::Vector3(vec);
        assert_eq!(v.as_vec3(), Some(vec));
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
        let v1: Variant = 3.14f32.into();
        if let Variant::Number(Number::F32(f)) = v1 {
            assert!((f - 3.14).abs() < 0.001);
        } else {
            panic!("Expected F32");
        }

        let v2: Variant = 2.71828f64.into();
        assert_eq!(v2.as_number(), Some(Number::F64(2.71828)));
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
        assert_ne!(Number::F32(3.14), Number::F64(3.14));
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
}
