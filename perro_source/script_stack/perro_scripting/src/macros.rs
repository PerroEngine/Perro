#[macro_export]
macro_rules! lifecycle {
    ({ $($methods:item)* }) => {
        $crate::lifecycle!(Script { $($methods)* });
    };
    ($script_name:ident { $($methods:item)* }) => {
        #[doc = "@Script"]
        #[derive(Default)]
        struct $script_name;

        impl<RT: RuntimeAPI + ?Sized, RS: perro_resource_context::api::ResourceAPI + ?Sized, IP: perro_input::InputAPI + ?Sized> ScriptLifecycle<RT, RS, IP> for $script_name {
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
            $ctx:ident : &mut RuntimeContext<'_, RT>,
            $res:ident : &ResourceContext<'_, RS>,
            $ipt:ident : &InputContext<'_, IP>,
            $self:ident : NodeID
            $(, $arg:ident : $arg_ty:ty )* $(,)?
        ) $(-> $ret:ty)? $body:block
        $($rest:tt)*
    ) => {
        $(#[$meta])*
        $vis fn $name<RT: RuntimeAPI + ?Sized, RS: perro_resource_context::api::ResourceAPI + ?Sized, IP: perro_input::InputAPI + ?Sized>(
            &$self_ident,
            $ctx: &mut RuntimeContext<'_, RT>,
            $res: &perro_resource_context::ResourceContext<'_, RS>,
            $ipt: &perro_input::InputContext<'_, IP>,
            $self: NodeID
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
