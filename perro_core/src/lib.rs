//! “Public” API of your engine
pub mod nodes;
pub mod structs2d;
pub mod scene;     
pub mod rendering;
pub mod scripting;
pub mod project;


pub use nodes::*;
pub use structs2d::*;
pub use rendering::*;
pub use scripting::*;
pub use scene::*;
pub use project::*;


use crate::app::App;
use crate::registry::DllScriptProvider;

pub type RuntimeApp = App<DllScriptProvider>;

