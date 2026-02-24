use perro_variant::Variant;
use serde_json::Value;

pub fn parse(json_str: &str) -> Result<Variant, serde_json::Error> {
    let value: Value = serde_json::from_str(json_str)?;
    Ok(Variant::from_json_value(value))
}

pub fn stringify(value: &Variant) -> Result<String, serde_json::Error> {
    let json_value = value.to_json_value();
    serde_json::to_string(&json_value)
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, sync::Arc};

    use perro_variant::Variant;

    use super::{parse, stringify};

    #[test]
    fn parse_returns_object_variant_for_json_object() {
        let parsed = parse(r#"{"name":"perro","ok":true,"count":3}"#).expect("valid json");

        let obj = parsed.as_object().expect("object variant");
        assert_eq!(
            obj.get(Arc::<str>::from("name").as_ref())
                .and_then(Variant::as_str),
            Some("perro")
        );
        assert_eq!(
            obj.get(Arc::<str>::from("ok").as_ref())
                .and_then(Variant::as_bool),
            Some(true)
        );
        assert!(obj.contains_key(Arc::<str>::from("count").as_ref()));
    }

    #[test]
    fn parse_invalid_json_returns_error() {
        assert!(parse("{bad json").is_err());
    }

    #[test]
    fn stringify_and_parse_roundtrip_json_compatible_variant() {
        let mut map = BTreeMap::new();
        map.insert(Arc::<str>::from("title"), Variant::from("engine"));
        map.insert(Arc::<str>::from("enabled"), Variant::from(true));
        map.insert(
            Arc::<str>::from("items"),
            Variant::Array(vec![Variant::from(1u32), Variant::from(2u32)]),
        );
        let input = Variant::Object(map);

        let serialized = stringify(&input).expect("serialize");
        let reparsed = parse(&serialized).expect("parse");

        assert_eq!(input.to_json_value(), reparsed.to_json_value());
    }
}
