use perro_ids::{NodeID, ScriptMemberID};
use perro_input_api::{InputAPI, InputWindow};
use perro_resource_api::{ResourceWindow, api::ResourceAPI};
use perro_runtime_api::{RuntimeWindow, api::RuntimeAPI};
use perro_variant::{SceneVariantResolver, Variant};
use std::any::Any;

/// Magic bytes at the start of every v2 dynamic-script ABI descriptor.
pub const SCRIPT_ABI_V2_MAGIC: [u8; 8] = *b"PERROSC\0";
/// Dynamic-script ABI version understood by this engine build.
pub const SCRIPT_ABI_V2_VERSION: u32 = 2;

/// Prefix read before the runtime trusts the full dynamic-script descriptor.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScriptAbiDescriptorHeader {
    /// Fixed marker used to reject unrelated or corrupt symbols.
    pub magic: [u8; 8],
    /// Layout and calling-convention contract version.
    pub abi_version: u32,
    /// Byte size of the full descriptor supplied by the script library.
    pub descriptor_size: u32,
}

/// Compatibility gate exported by every compiler-generated script library.
///
/// Rust trait objects and their vtables do not have a stable C ABI. The runtime
/// therefore validates this descriptor before it reads constructors or calls
/// any other library function.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScriptAbiDescriptor {
    /// Stable prefix that can be checked before reading the full descriptor.
    pub header: ScriptAbiDescriptorHeader,
}

impl ScriptAbiDescriptor {
    /// Build a v2 descriptor for compiler-generated library glue.
    pub const fn v2() -> Self {
        Self {
            header: ScriptAbiDescriptorHeader {
                magic: SCRIPT_ABI_V2_MAGIC,
                abi_version: SCRIPT_ABI_V2_VERSION,
                descriptor_size: std::mem::size_of::<Self>() as u32,
            },
        }
    }
}

/// Native Rust constructor used by statically linked script registries.
pub type ScriptConstructor<API> = fn() -> *mut dyn ScriptBehavior<API>;

/// Constructor pointer transferred through the dynamic script-library ABI.
#[allow(improper_ctypes_definitions)]
pub type DynamicScriptConstructor<API> = extern "C" fn() -> *mut dyn ScriptBehavior<API>;

/// API surface bundle used by script callbacks.
///
/// Runtime code supplies concrete types for the mutable runtime API, shared
/// resource API, and frame input API. [`ScriptContext`] then passes window
/// wrappers over those concrete APIs into lifecycle and method callbacks.
pub trait ScriptAPI {
    type RT: RuntimeAPI + ?Sized;
    type RS: ResourceAPI + ?Sized;
    type IP: InputAPI + ?Sized;
}

/// Callback-scoped script context.
///
/// The runtime builds this right before it calls a lifecycle method or exported
/// script method. `run` is the only mutable engine surface and borrows the
/// runtime for the callback duration. `res` and `ipt` are shared windows over
/// resource and input state. None of these windows are stored on the script
/// instance, so Rust borrow scopes end when the callback returns.
///
/// Script state is not borrowed through this struct. State access goes through
/// closure APIs such as `with_state!` and `with_state_mut!`; those APIs borrow
/// the boxed concrete state only for the closure body so references cannot
/// escape the runtime's borrow.
pub struct ScriptContext<'a, API: ScriptAPI + ?Sized> {
    /// Mutable runtime operations for the current callback.
    pub run: &'a mut RuntimeWindow<'a, API::RT>,
    /// Shared resource operations for the current callback.
    pub res: &'a ResourceWindow<'a, API::RS>,
    /// Shared input snapshot operations for the current callback.
    pub ipt: &'a InputWindow<'a, API::IP>,
    /// Node id this script instance is attached to.
    pub id: NodeID,
}

/// Optional lifecycle hooks implemented by scripts.
///
/// Generated script glue sets [`ScriptFlags`] for non-empty lifecycle methods so
/// the runtime only schedules callbacks that exist.
pub trait ScriptLifecycle<API: ScriptAPI + ?Sized> {
    /// Called when this script instance is attached and state is created.
    fn on_init(&self, _ctx: &mut ScriptContext<'_, API>) {}
    /// Called after all startup scripts for a loaded scene are attached.
    fn on_all_init(&self, _ctx: &mut ScriptContext<'_, API>) {}
    /// Called during variable-rate update when scheduled.
    fn on_update(&self, _ctx: &mut ScriptContext<'_, API>) {}
    /// Called during fixed-step update when scheduled.
    fn on_fixed_update(&self, _ctx: &mut ScriptContext<'_, API>) {}
    /// Called before this script instance is detached or its node is removed.
    fn on_removal(&self, _ctx: &mut ScriptContext<'_, API>) {}
}

/// Behavior object shared by all instances of one script definition.
///
/// `ScriptBehavior` is intentionally separate from per-node state. The behavior
/// object contains vtable dispatch and generated field/method glue; the runtime
/// can put it in an `Arc` and cheaply clone handles for callback dispatch. Each
/// attached node receives its own [`Any`] state object from
/// [`ScriptBehavior::create_state`], so mutable game state stays per instance.
pub trait ScriptBehavior<API: ScriptAPI + ?Sized>: ScriptLifecycle<API> {
    /// Return lifecycle flags used to build update/fixed/removal schedules.
    fn script_flags(&self) -> ScriptFlags;

    /// Create per-instance script state.
    ///
    /// The default state is `()`. Generated scripts override this for `#[State]`
    /// or script-defined state structs.
    fn create_state(&self) -> Box<dyn Any> {
        Box::new(())
    }

    /// Read a script variable from concrete state.
    fn get_var(&self, state: &dyn Any, var: ScriptMemberID) -> Variant;

    /// Write a script variable into concrete state.
    fn set_var(&self, state: &mut dyn Any, var: ScriptMemberID, value: Variant);

    /// Apply values injected from scene data before init callbacks run.
    ///
    /// `resolver` is used only by this scene path. It lets generated state
    /// decoding turn authored resource paths into typed resource IDs without
    /// changing the strict runtime `set_var` contract.
    fn apply_scene_injected_vars(
        &self,
        state: &mut dyn Any,
        vars: Vec<(ScriptMemberID, Variant)>,
        resolver: &mut dyn SceneVariantResolver,
    ) {
        let _ = resolver;
        for (var, value) in vars {
            self.set_var(state, var, value);
        }
    }

    /// Call an exported script method through generated dispatch glue.
    fn call_method(
        &self,
        method: ScriptMemberID,
        ctx: &mut ScriptContext<'_, API>,
        params: &[Variant],
    ) -> Variant;
}

/// Cast script state to a concrete type without a runtime type check.
///
/// The runtime does not call this directly on untrusted state. It stores the
/// state's `TypeId` beside the boxed value and checks that id before using this
/// fast cast helper.
///
/// # Safety
/// Caller must guarantee `state` points to a value of type `T`.
#[inline(always)]
pub unsafe fn state_ref_unchecked<T: 'static>(state: &dyn Any) -> &T {
    // SAFETY: Caller verifies the erased state is exactly T before using this unchecked cast.
    unsafe { &*(state as *const dyn Any as *const T) }
}

/// Mutably cast script state to a concrete type without a runtime type check.
///
/// The runtime uses this only after a `TypeId` match and only while holding a
/// unique mutable borrow of the boxed state.
///
/// # Safety
/// Caller must guarantee `state` points to a value of type `T`, and no other
/// references alias the returned mutable reference.
#[inline(always)]
pub unsafe fn state_mut_unchecked<T: 'static>(state: &mut dyn Any) -> &mut T {
    // SAFETY: Caller verifies type identity and unique mutable access to the erased state.
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
    /// No lifecycle hooks.
    pub const NONE: u8 = 0;
    /// `on_init` exists.
    pub const HAS_INIT: u8 = 1 << 0;
    /// `on_update` exists.
    pub const HAS_UPDATE: u8 = 1 << 1;
    /// `on_fixed_update` exists.
    pub const HAS_FIXED_UPDATE: u8 = 1 << 2;
    /// `on_all_init` exists.
    pub const HAS_ALL_INIT: u8 = 1 << 3;
    /// `on_removal` exists.
    pub const HAS_REMOVAL: u8 = 1 << 4;

    /// Create flags from a bitmask built by generated script glue.
    #[inline(always)]
    pub const fn new(flags: u8) -> Self {
        ScriptFlags(flags)
    }

    /// Return whether `on_init` exists.
    #[inline(always)]
    pub const fn has_init(self) -> bool {
        self.0 & Self::HAS_INIT != 0
    }

    /// Return whether `on_update` exists.
    #[inline(always)]
    pub const fn has_update(self) -> bool {
        self.0 & Self::HAS_UPDATE != 0
    }

    /// Return whether `on_fixed_update` exists.
    #[inline(always)]
    pub const fn has_fixed_update(self) -> bool {
        self.0 & Self::HAS_FIXED_UPDATE != 0
    }

    /// Return whether `on_all_init` exists.
    #[inline(always)]
    pub const fn has_all_init(self) -> bool {
        self.0 & Self::HAS_ALL_INIT != 0
    }

    /// Return whether `on_removal` exists.
    #[inline(always)]
    pub const fn has_removal(self) -> bool {
        self.0 & Self::HAS_REMOVAL != 0
    }
}
