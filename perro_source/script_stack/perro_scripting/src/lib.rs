mod macros;
pub mod script_trait;
pub use perro_scripting_macros::{State, Variant};
pub use script_trait::*;

pub mod prelude {
    pub use crate::lifecycle;
    pub use crate::methods;
    pub use crate::script_trait::{
        ScriptAPI, ScriptBehavior, ScriptConstructor, ScriptContext, ScriptFlags, ScriptLifecycle,
        state_mut_unchecked, state_ref_unchecked,
    };
    pub use crate::{State, Variant};
    pub use perro_ids::prelude::*;
    pub use perro_input_api::prelude::*;
    pub use perro_resource_api::prelude::*;
    pub use perro_runtime_api::prelude::*;
    pub use perro_variant::{DeriveVariant, Variant, VariantKind, VariantSchema};
}
