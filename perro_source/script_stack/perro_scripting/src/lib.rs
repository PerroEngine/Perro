mod macros;
pub mod script_trait;
pub use perro_scripting_macros::State;
pub use script_trait::*;

pub mod prelude {
    pub use crate::State;
    pub use crate::lifecycle;
    pub use crate::methods;
    pub use crate::script_trait::{
        ScriptBehavior, ScriptConstructor, ScriptFlags, ScriptLifecycle,
    };
    pub use perro_ids::prelude::{NodeID, ScriptMemberID};
    pub use perro_resource_context::prelude::ResourceContext;
    pub use perro_runtime_context::prelude::{RuntimeAPI, RuntimeContext};
    pub use perro_variant::Variant;
}
