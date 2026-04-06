mod macros;
pub mod script_trait;
pub use perro_scripting_macros::{State, StateField, Variant};
pub use script_trait::*;

pub mod prelude {
    pub use crate::lifecycle;
    pub use crate::methods;
    pub use crate::script_trait::{
        ScriptBehavior, ScriptConstructor, ScriptFlags, ScriptLifecycle,
    };
    pub use crate::{State, StateField, Variant};
    pub use perro_ids::prelude::{NodeID, ScriptMemberID};
    pub use perro_input::prelude::*;
    pub use perro_resource_context::prelude::*;
    pub use perro_runtime_context::prelude::*;
    pub use perro_variant::{CustomVariant, StateField as StateFieldTrait, Variant};
}
