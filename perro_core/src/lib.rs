//! "Public" API of your engine
pub mod brk;
pub mod ids;
pub mod input;
pub mod node_arena;
pub mod nodes;
pub mod physics;
pub mod project;
pub mod project_creator;
pub mod rendering;
pub mod runtime;
pub mod scene;
pub mod scripting;
pub mod structs;
pub mod thread_utils;
pub mod types;

pub use ids::{LightID, MaterialID, MeshID, NodeID, TextureID, UIElementID};
pub use nodes::*;
pub use project::*;
pub use rendering::*;
pub use scene::*;
pub use scripting::*;
pub use structs::*;

pub type RuntimeApp = crate::rendering::app::App<crate::scripting::DllScriptProvider>;

pub mod prelude {
    // Core engine node types
    pub use crate::node_registry::{NodeType, SceneNode};
    pub use crate::nodes::*;
    pub use crate::ui_node::UINode;

    pub use crate::structs::*;

    // Script API — only what script authors should use
    pub use crate::script::*;

    pub fn string_to_u64(s: &str) -> u64 {
        const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;
        let mut hash = FNV_OFFSET_BASIS;
        for byte in s.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash
    }

    pub use crate::api::ScriptApi;

    // Core primitive/shared types (Vec2, Color, etc.)
    pub use crate::types::*;

    // IDs (u64) and type-safe wrappers from ids.rs
    pub use crate::{LightID, MaterialID, MeshID, NodeID, TextureID, UIElementID};
}
