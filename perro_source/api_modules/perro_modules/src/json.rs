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
#[path = "../tests/unit/json_tests.rs"]
mod tests;
