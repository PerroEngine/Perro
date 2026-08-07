#[macro_export]
macro_rules! demo_exclude {
    ({ $($body:tt)* }) => {{
        #[cfg(not(feature = "perro-demo"))]
        {
            $($body)*
        }
    }};
}

#[macro_export]
macro_rules! lifecycle {
    ({ $($methods:item)* }) => {
        $crate::lifecycle!(Script { $($methods)* });
    };
    ($script_name:ident { $($methods:item)* }) => {
        #[doc = "@Script"]
        #[derive(Default)]
        struct $script_name;

        impl<API> ScriptLifecycle<API> for $script_name
        where
            API: ScriptAPI + ?Sized,
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
    // Fast path: every item is a `ScriptContext` method, so the whole list is
    // rewritten in one repetition.
    //
    // The arms below are a tt-muncher: each step re-emits the untouched tail
    // into a fresh invocation, so a block of N methods pushes O(N^2) tokens
    // through expansion. One 48-method script was generating ~900 KB on its own.
    // This arm handles the common all-methods case in a single pass; mixed lists
    // (a plain helper `fn` among them) still fall through to the muncher.
    (
        $(
            $(#[$fast_meta:meta])*
            $fast_vis:vis fn $fast_name:ident(
                &$fast_self:ident,
                $fast_ctx:ident : &mut ScriptContext<'_, API>
                $(, $fast_arg:ident : $fast_arg_ty:ty )* $(,)?
            ) $(-> $fast_ret:ty)? $fast_body:block
        )+
    ) => {
        $(
            #[allow(clippy::too_many_arguments)]
            $(#[$fast_meta])*
            $fast_vis fn $fast_name<API>(
                &$fast_self,
                $fast_ctx: &mut ScriptContext<'_, API>
                $(, $fast_arg : $fast_arg_ty )*
            ) $(-> $fast_ret)?
            where
                API: ScriptAPI + ?Sized,
            $fast_body
        )+
    };
    (
        $(#[$meta:meta])*
        $vis:vis fn $name:ident(
            &$self_ident:ident,
            $ctx:ident : &mut ScriptContext<'_, API>
            $(, $arg:ident : $arg_ty:ty )* $(,)?
        ) $(-> $ret:ty)? $body:block
        $($rest:tt)*
    ) => {
        #[allow(clippy::too_many_arguments)]
        $(#[$meta])*
        $vis fn $name<API>(
            &$self_ident,
            $ctx: &mut ScriptContext<'_, API>
            $(, $arg : $arg_ty )*
        ) $(-> $ret)?
        where
            API: ScriptAPI + ?Sized,
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

#[cfg(test)]
mod tests {
    #[test]
    #[allow(unused_assignments, unused_mut)]
    fn demo_exclude_matches_feature() {
        let mut value = 0;
        crate::demo_exclude!({
            value = 1;
        });
        #[cfg(feature = "perro-demo")]
        assert_eq!(value, 0);
        #[cfg(not(feature = "perro-demo"))]
        assert_eq!(value, 1);
    }
}
