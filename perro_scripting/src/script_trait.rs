use perro_ids::{NodeID, ScriptMemberID};
use perro_variant::Variant;

#[allow(improper_ctypes_definitions)]
pub type ScriptConstructor = extern "C" fn() -> *mut dyn ScriptObject;

pub trait ScriptLifecycle {
    fn init(&mut self);
    fn update(&mut self);
    fn fixed_update(&mut self);
}

pub trait ScriptObject: ScriptLifecycle {
    fn internal_init(&mut self) {
        self.init();
    }

    fn internal_update(&mut self) {
        self.update();
    }

    fn internal_fixed_update(&mut self) {
        self.fixed_update();
    }

    fn script_flags(&self) -> ScriptFlags;

    fn get_id(&self) -> NodeID;
    fn set_id(&mut self, id: NodeID);

    fn get_var(&self, var_id: ScriptMemberID) -> Variant;
    fn set_var(&mut self, var_id: ScriptMemberID, value: Variant);

    fn apply_exposed_vars(&mut self, vars: &[(ScriptMemberID, Variant)]) {
        for (var_id, value) in vars {
            self.set_var(*var_id, value.clone());
        }
    }

    fn call_method(&mut self, method_id: ScriptMemberID, params: &[Variant]) -> Variant;

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
