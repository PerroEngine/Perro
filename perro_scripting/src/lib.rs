pub mod script_trait;
pub use script_trait::*;

pub mod prelude {
    pub use crate::script_trait::{
        ScriptBehavior, ScriptConstructor, ScriptFlags, ScriptLifecycle,
    };
    pub use perro_api::prelude::{API, RuntimeAPI};
    pub use perro_ids::prelude::{NodeID, ScriptMemberID};
    pub use perro_variant::Variant;
}
