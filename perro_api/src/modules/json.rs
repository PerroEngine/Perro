pub struct JSONModule;

impl JSONModule {
    pub fn new() -> Self {
        Self
    }

    pub fn parse(&self, json_str: &str) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::from_str(json_str)
    }

    pub fn stringify(&self, value: &serde_json::Value) -> String {
        serde_json::to_string(value).unwrap_or_default()
    }
}
