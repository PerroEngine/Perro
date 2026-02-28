pub mod lexer;
pub mod parser;
pub mod runtime_scene;
pub mod static_scene;

pub use lexer::*;
pub use parser::*;
pub use runtime_scene::*;

// Re-export static scene types with different names to avoid confusion
pub use static_scene::{
    Scene as StaticScene, SceneKey as StaticSceneKey, SceneNodeDataEntry as StaticNodeData,
    SceneNodeEntry as StaticNodeEntry, SceneNodeType as StaticNodeType,
    SceneValue as StaticSceneValue,
};

#[cfg(test)]
#[path = "../tests/unit/lib_tests.rs"]
mod tests;

