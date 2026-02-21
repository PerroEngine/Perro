pub mod script_trait;
pub use script_trait::*;

pub mod prelude {
    pub use crate::script_trait::{
        ScriptBehavior, ScriptConstructor, ScriptFlags, ScriptLifecycle,
    };
    pub use perro_context::prelude::{RuntimeAPI, RuntimeContext};
    pub use perro_ids::prelude::{NodeID, ScriptMemberID};
    pub use perro_variant::Variant;
}
