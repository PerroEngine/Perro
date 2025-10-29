//! “Public” API of your engine
pub mod nodes;
pub mod structs2d;
pub mod scene;     
pub mod rendering;
pub mod scripting;
pub mod project;
pub mod brk;
pub mod types;


pub use nodes::*;
pub use structs2d::*;
pub use rendering::*;
pub use scripting::*;
pub use scene::*;
pub use project::*;


use crate::app::App;
use crate::registry::DllScriptProvider;

pub type RuntimeApp = App<DllScriptProvider>;

pub mod prelude {
    // Core engine node types
    pub use crate::nodes::*;
    pub use crate::ui_node::Ui;

    // Transform + math types commonly needed for 2D
    pub use crate::structs2d::*;

    // Script API — only what script authors should use
    pub use crate::script::*;

    pub use crate::api::ScriptApi;
    // Correct source for UpdateOp
    pub use crate::script::{UpdateOp, Var};

    // Core primitive/shared types (Vec2, Color, etc.)
    pub use crate::types::*;
}
