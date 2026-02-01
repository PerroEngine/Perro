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
pub mod time_util;
pub mod types;

pub use ids::{LightID, MaterialID, MeshID, NodeID, SignalID, TextureID, UIElementID};
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

    #[inline]
    pub fn string_to_u64(s: &str) -> u64 {
        let mut hash: u64 = 0xA0761D6478BD642F;

        for &b in s.as_bytes() {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0xE7037ED1A0B428DB);
            hash = mix64(hash);
        }

        mix64(hash ^ (s.len() as u64))
    }

    #[inline]
    fn mix64(mut x: u64) -> u64 {
        x ^= x >> 30;
        x = x.wrapping_mul(0xBF58476D1CE4E5B9);
        x ^= x >> 27;
        x = x.wrapping_mul(0x94D049BB133111EB);
        x ^= x >> 31;
        x
    }

    pub use crate::api::ScriptApi;

    // Core primitive/shared types (Vec2, Color, etc.)
    pub use crate::types::*;

    // IDs (u64) and type-safe wrappers from ids.rs
    pub use crate::{LightID, MaterialID, MeshID, NodeID, SignalID, TextureID, UIElementID};
}
