//! Core scripting traits and context types.
//!
//! Generated and hand-written scripts implement [`ScriptBehavior`]. The runtime
//! stores each behavior behind an `Arc<dyn ScriptBehavior<_>>` and creates one
//! boxed state object per attached script instance. Lifecycle callbacks receive
//! [`ScriptContext`], which exposes runtime, resource, and input windows for the
//! duration of that callback.

mod macros;
pub mod script_trait;
pub use perro_scripting_macros::{State, Variant};
pub use script_trait::*;

/// Common imports for generated and hand-written scripts.
pub mod prelude {
    pub use crate::lifecycle;
    pub use crate::methods;
    pub use crate::script_trait::{
        SCRIPT_ABI_V2_MAGIC, SCRIPT_ABI_V2_VERSION, ScriptAPI, ScriptAbiDescriptor,
        ScriptAbiDescriptorHeader, ScriptBehavior, ScriptConstructor, ScriptContext, ScriptFlags,
        ScriptLifecycle, state_mut_unchecked, state_ref_unchecked,
    };
    pub use crate::{State, Variant};
    pub use perro_ids::prelude::*;
    pub use perro_input_api::prelude::*;
    pub use perro_resource_api::prelude::*;
    pub use perro_runtime_api::prelude::*;
    pub use perro_variant::{DeriveVariant, Variant, VariantKind, VariantSchema};
}
