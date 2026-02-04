use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct SceneRef {
    path: String,
}

impl SceneRef {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

impl Serialize for SceneRef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.path)
    }
}

impl<'de> Deserialize<'de> for SceneRef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(deserializer)?;
        if let Some(s) = v.as_str() {
            return Ok(SceneRef::new(s));
        }
        if let Some(obj) = v.as_object() {
            if let Some(path) = obj.get("path").and_then(|p| p.as_str()) {
                return Ok(SceneRef::new(path));
            }
        }
        Ok(SceneRef::default())
    }
}
