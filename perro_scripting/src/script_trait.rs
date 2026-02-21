use perro_context::{RuntimeContext, api::RuntimeAPI};
use perro_ids::{NodeID, ScriptMemberID};
use perro_variant::Variant;
use std::any::Any;

#[allow(improper_ctypes_definitions)]
pub type ScriptConstructor<R> = extern "C" fn() -> *mut dyn ScriptBehavior<R>;

pub trait ScriptLifecycle<R: RuntimeAPI + ?Sized> {
    fn on_init(&self, _ctx: &mut RuntimeContext<'_, R>, _self_id: NodeID) {}
    fn on_start(&self, _ctx: &mut RuntimeContext<'_, R>, _self_id: NodeID) {}
    fn on_update(&self, _ctx: &mut RuntimeContext<'_, R>, _self_id: NodeID) {}
    fn on_fixed_update(&self, _ctx: &mut RuntimeContext<'_, R>, _self_id: NodeID) {}
    fn on_removed(&self, _ctx: &mut RuntimeContext<'_, R>, _self_id: NodeID) {}
}

pub trait ScriptBehavior<R: RuntimeAPI + ?Sized>: ScriptLifecycle<R> {
    fn script_flags(&self) -> ScriptFlags;
    fn create_state(&self) -> Box<dyn Any> {
        Box::new(())
    }
    fn get_var(&self, state: &dyn Any, var_id: ScriptMemberID) -> Variant;
    fn set_var(&self, state: &mut dyn Any, var_id: ScriptMemberID, value: &Variant);
    fn apply_exposed_vars(&self, state: &mut dyn Any, vars: &[(ScriptMemberID, Variant)]) {
        for (var_id, value) in vars {
            self.set_var(state, *var_id, value);
        }
    }
    fn call_method(
        &self,
        method_id: ScriptMemberID,
        ctx: &mut RuntimeContext<'_, R>,
        self_id: NodeID,
        params: &[Variant],
    ) -> Variant;
    fn attributes_of(&self, member: &str) -> &'static [&'static str];
    fn members_with(&self, attribute: &str) -> &'static [&'static str];
    fn has_attribute(&self, member: &str, attribute: &str) -> bool;
}

/// Bitflags to track which lifecycle methods are implemented by a script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptFlags(u8);

impl ScriptFlags {
    pub const NONE: u8 = 0;
    pub const HAS_INIT: u8 = 1 << 0;
    pub const HAS_UPDATE: u8 = 1 << 1;
    pub const HAS_FIXED_UPDATE: u8 = 1 << 2;
    pub const HAS_START: u8 = 1 << 3;
    pub const HAS_REMOVED: u8 = 1 << 4;

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
    pub const fn has_start(self) -> bool {
        self.0 & Self::HAS_START != 0
    }

    #[inline(always)]
    pub const fn has_removed(self) -> bool {
        self.0 & Self::HAS_REMOVED != 0
    }
}
