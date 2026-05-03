mod macros;
pub mod script_trait;
pub use perro_scripting_macros::{State, Variant};
pub use script_trait::*;

pub mod prelude {
    pub use crate::lifecycle;
    pub use crate::methods;
    pub use crate::script_trait::{
        ScriptBehavior, ScriptConstructor, ScriptContext, ScriptFlags, ScriptLifecycle,
    };
    pub use crate::{State, Variant};
    pub use perro_ids::prelude::*;
    pub use perro_input::prelude::*;
    pub use perro_resource_context::prelude::*;
    pub use perro_runtime_context::prelude::*;
    pub use perro_variant::{Variant, VariantCodec, VariantSchema};
}
