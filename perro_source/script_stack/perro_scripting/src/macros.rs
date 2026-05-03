#[macro_export]
macro_rules! lifecycle {
    ({ $($methods:item)* }) => {
        $crate::lifecycle!(Script { $($methods)* });
    };
    ($script_name:ident { $($methods:item)* }) => {
        #[doc = "@Script"]
        #[derive(Default)]
        struct $script_name;

        impl<RT, RS, IP> ScriptLifecycle<RT, RS, IP> for $script_name
        where
            RT: RuntimeAPI + ?Sized,
            RS: ResourceAPI + ?Sized,
            IP: InputAPI + ?Sized,
        {
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
            $ctx:ident : &mut ScriptContext<'_, RT, RS, IP>
            $(, $arg:ident : $arg_ty:ty )* $(,)?
        ) $(-> $ret:ty)? $body:block
        $($rest:tt)*
    ) => {
        $(#[$meta])*
        $vis fn $name<RT, RS, IP>(
            &$self_ident,
            $ctx: &mut ScriptContext<'_, RT, RS, IP>
            $(, $arg : $arg_ty )*
        ) $(-> $ret)?
        where
            RT: RuntimeAPI + ?Sized,
            RS: ResourceAPI + ?Sized,
            IP: InputAPI + ?Sized,
        $body

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

