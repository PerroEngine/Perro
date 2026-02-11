use perro_api::{API, api::RuntimeAPI};
use perro_ids::{NodeID, ScriptMemberID};
use perro_variant::Variant;
use std::any::Any;

#[allow(improper_ctypes_definitions)]
pub type ScriptConstructor<R> = extern "C" fn() -> *mut dyn ScriptBehavior<R>;

pub trait ScriptState {
    fn id(&self) -> NodeID;
    fn set_id(&mut self, id: NodeID);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub trait ScriptLifecycle<R: RuntimeAPI + ?Sized> {
    fn init(&self, api: &mut API<'_, R>, self_id: NodeID);
    fn update(&self, api: &mut API<'_, R>, self_id: NodeID);
    fn fixed_update(&self, api: &mut API<'_, R>, self_id: NodeID);
}

pub trait ScriptBehavior<R: RuntimeAPI + ?Sized>: ScriptLifecycle<R> {
    fn script_flags(&self) -> ScriptFlags;

    fn get_var(&self, state: &dyn ScriptState, var_id: ScriptMemberID) -> Variant;
    fn set_var(&self, state: &mut dyn ScriptState, var_id: ScriptMemberID, value: Variant);

    fn apply_exposed_vars(&self, state: &mut dyn ScriptState, vars: &[(ScriptMemberID, Variant)]) {
        for (var_id, value) in vars {
            self.set_var(state, *var_id, value.clone());
        }
    }

    fn call_method(
        &self,
        method_id: ScriptMemberID,
        api: &mut API<'_, R>,
        self_id: NodeID,
        params: &[Variant],
    ) -> Variant;

    fn attributes_of(&self, member: &str) -> Vec<String>;
    fn members_with(&self, attribute: &str) -> Vec<String>;
    fn has_attribute(&self, member: &str, attribute: &str) -> bool;
}

/// Bitflags to track which lifecycle methods are implemented by a script
/// This allows the engine to skip calling methods that are not implemented,
/// reducing overhead significantly when scripts only implement a subset of methods
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptFlags(u8);

impl ScriptFlags {
    pub const NONE: u8 = 0;
    pub const HAS_INIT: u8 = 1 << 0;
    pub const HAS_UPDATE: u8 = 1 << 1;
    pub const HAS_FIXED_UPDATE: u8 = 1 << 2;

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
}
