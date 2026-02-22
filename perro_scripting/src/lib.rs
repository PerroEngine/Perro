pub mod script_trait;
pub use script_trait::*;
pub use perro_scripting_macros::State;

#[macro_export]
macro_rules! lifecycle {
    ({ $($methods:item)* }) => {
        $crate::lifecycle!(Script { $($methods)* });
    };
    ($script_name:ident { $($methods:item)* }) => {
        #[doc = "@Script"]
        #[derive(Default)]
        struct $script_name;

        impl<R: RuntimeAPI + ?Sized> ScriptLifecycle<R> for $script_name {
            $($methods)*
        }
    };
}

#[macro_export]
macro_rules! methods {
    ({ $($methods:tt)* }) => {
        $crate::methods!(Script { $($methods)* });
    };
    ($script_name:ident { $($methods:tt)* }) => {
        impl $script_name {
            $crate::__methods_internal! { $($methods)* }
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __methods_internal {
    () => {};
    (
        $(#[$meta:meta])*
        $vis:vis fn $name:ident(
            &$self_ident:ident,
            $ctx:ident : &mut RuntimeContext<'_, R>,
            $self_id:ident : NodeID
            $(, $arg:ident : $arg_ty:ty )* $(,)?
        ) $(-> $ret:ty)? $body:block
        $($rest:tt)*
    ) => {
        $(#[$meta])*
        $vis fn $name<R: RuntimeAPI + ?Sized>(
            &$self_ident,
            $ctx: &mut RuntimeContext<'_, R>,
            $self_id: NodeID
            $(, $arg : $arg_ty )*
        ) $(-> $ret)? $body

        $crate::__methods_internal! { $($rest)* }
    };
    (
        $method:item
        $($rest:tt)*
    ) => {
        $method
        $crate::__methods_internal! { $($rest)* }
    };
}

pub mod prelude {
    pub use crate::lifecycle;
    pub use crate::methods;
    pub use crate::State;
    pub use crate::script_trait::{
        ScriptBehavior, ScriptConstructor, ScriptFlags, ScriptLifecycle,
    };
    pub use perro_context::prelude::{RuntimeAPI, RuntimeContext};
    pub use perro_ids::prelude::{NodeID, ScriptMemberID};
    pub use perro_variant::Variant;
}
