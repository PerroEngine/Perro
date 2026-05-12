use perro_ids::{NodeID, ScriptMemberID};
use perro_input::{InputAPI, InputWindow};
use perro_resource_context::{ResourceWindow, api::ResourceAPI};
use perro_runtime_context::{RuntimeWindow, api::RuntimeAPI};
use perro_variant::Variant;
use std::any::Any;

#[allow(improper_ctypes_definitions)]
pub type ScriptConstructor<API> = extern "C" fn() -> *mut dyn ScriptBehavior<API>;

/// ScriptAPI groups the three API surfaces scripts depend on.
pub trait ScriptAPI {
    type RT: RuntimeAPI + ?Sized;
    type RS: ResourceAPI + ?Sized;
    type IP: InputAPI + ?Sized;
}

/// ScriptContext is the context passed to script lifecycle methods, providing access to the runtime, resource, and input APIs, as well as the ID of the node the script is attached to.
pub struct ScriptContext<'a, API: ScriptAPI + ?Sized> {
    pub run: &'a mut RuntimeWindow<'a, API::RT>,
    pub res: &'a ResourceWindow<'a, API::RS>,
    pub ipt: &'a InputWindow<'a, API::IP>,
    pub id: NodeID,
}

pub trait ScriptLifecycle<API: ScriptAPI + ?Sized> {
    fn on_init(&self, _ctx: &mut ScriptContext<'_, API>) {}
    fn on_all_init(&self, _ctx: &mut ScriptContext<'_, API>) {}
    fn on_update(&self, _ctx: &mut ScriptContext<'_, API>) {}
    fn on_fixed_update(&self, _ctx: &mut ScriptContext<'_, API>) {}
    fn on_removal(&self, _ctx: &mut ScriptContext<'_, API>) {}
}

pub trait ScriptBehavior<API: ScriptAPI + ?Sized>: ScriptLifecycle<API> {
    fn script_flags(&self) -> ScriptFlags;
    fn create_state(&self) -> Box<dyn Any> {
        Box::new(())
    }
    fn get_var(&self, state: &dyn Any, var: ScriptMemberID) -> Variant;
    fn set_var(&self, state: &mut dyn Any, var: ScriptMemberID, value: Variant);
    fn apply_scene_injected_vars(&self, state: &mut dyn Any, vars: Vec<(ScriptMemberID, Variant)>) {
        for (var, value) in vars {
            self.set_var(state, var, value);
        }
    }
    fn call_method(
        &self,
        method: ScriptMemberID,
        ctx: &mut ScriptContext<'_, API>,
        params: &[Variant],
    ) -> Variant;
}

/// Cast script state to a concrete type without a runtime type check.
///
/// # Safety
/// Caller must guarantee `state` points to a value of type `T`.
#[inline(always)]
pub unsafe fn state_ref_unchecked<T: 'static>(state: &dyn Any) -> &T {
    unsafe { &*(state as *const dyn Any as *const T) }
}

/// Mutably cast script state to a concrete type without a runtime type check.
///
/// # Safety
/// Caller must guarantee `state` points to a value of type `T`, and no other
/// references alias the returned mutable reference.
#[inline(always)]
pub unsafe fn state_mut_unchecked<T: 'static>(state: &mut dyn Any) -> &mut T {
    unsafe { &mut *(state as *mut dyn Any as *mut T) }
}

#[cfg(test)]
mod state_cast_tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct TestState {
        value: u64,
    }

    #[test]
    fn state_ref_unchecked_matches_safe_downcast_ref() {
        let state: Box<dyn Any> = Box::new(TestState { value: 42 });

        let safe = state.as_ref().downcast_ref::<TestState>();
        // SAFETY: state is constructed as TestState above.
        let fast = Some(unsafe { state_ref_unchecked::<TestState>(state.as_ref()) });

        assert_eq!(fast, safe);
        assert_eq!(
            fast.map(|state| state as *const TestState),
            safe.map(|state| state as *const TestState)
        );
    }

    #[test]
    fn state_mut_unchecked_matches_safe_downcast_mut() {
        let mut safe_state: Box<dyn Any> = Box::new(TestState { value: 42 });
        let mut fast_state: Box<dyn Any> = Box::new(TestState { value: 42 });

        let safe = safe_state.as_mut().downcast_mut::<TestState>().unwrap();
        // SAFETY: fast_state is constructed as TestState above.
        let fast = unsafe { state_mut_unchecked::<TestState>(fast_state.as_mut()) };

        assert_eq!(fast, safe);

        safe.value += 1;
        fast.value += 1;

        assert_eq!(
            fast_state.as_ref().downcast_ref::<TestState>(),
            safe_state.as_ref().downcast_ref::<TestState>()
        );
    }
}

/// Bitflags to track which lifecycle methods are implemented by a script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptFlags(u8);

impl ScriptFlags {
    pub const NONE: u8 = 0;
    pub const HAS_INIT: u8 = 1 << 0;
    pub const HAS_UPDATE: u8 = 1 << 1;
    pub const HAS_FIXED_UPDATE: u8 = 1 << 2;
    pub const HAS_ALL_INIT: u8 = 1 << 3;
    pub const HAS_REMOVAL: u8 = 1 << 4;

    #[inline(always)]
    pub const fn new(flags: u8) -> Self {
        ScriptFlags(flags)
    }

    #[inline(always)]
    pub const fn has_init(self) -> bool {
        self.0 & Self::HAS_INIT != 0
    }

    #[inline(always)]
    pub const fn has_update(self) -> bool {
        self.0 & Self::HAS_UPDATE != 0
    }

    #[inline(always)]
    pub const fn has_fixed_update(self) -> bool {
        self.0 & Self::HAS_FIXED_UPDATE != 0
    }

    #[inline(always)]
    pub const fn has_all_init(self) -> bool {
        self.0 & Self::HAS_ALL_INIT != 0
    }

    #[inline(always)]
    pub const fn has_removal(self) -> bool {
        self.0 & Self::HAS_REMOVAL != 0
    }
}
