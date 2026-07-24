use crate::{app, error::SteamError};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InputType {
    Unknown,
    SteamController,
    XBox360Controller,
    XBoxOneController,
    GenericGamepad,
    PS4Controller,
    AppleMFiController,
    AndroidController,
    SwitchJoyConPair,
    SwitchJoyConSingle,
    SwitchProController,
    MobileTouch,
    PS3Controller,
    PS5Controller,
    SteamDeckController,
}

impl From<steamworks::InputType> for InputType {
    fn from(input_type: steamworks::InputType) -> Self {
        match input_type {
            steamworks::InputType::Unknown => Self::Unknown,
            steamworks::InputType::SteamController => Self::SteamController,
            steamworks::InputType::XBox360Controller => Self::XBox360Controller,
            steamworks::InputType::XBoxOneController => Self::XBoxOneController,
            steamworks::InputType::GenericGamepad => Self::GenericGamepad,
            steamworks::InputType::PS4Controller => Self::PS4Controller,
            steamworks::InputType::AppleMFiController => Self::AppleMFiController,
            steamworks::InputType::AndroidController => Self::AndroidController,
            steamworks::InputType::SwitchJoyConPair => Self::SwitchJoyConPair,
            steamworks::InputType::SwitchJoyConSingle => Self::SwitchJoyConSingle,
            steamworks::InputType::SwitchProController => Self::SwitchProController,
            steamworks::InputType::MobileTouch => Self::MobileTouch,
            steamworks::InputType::PS3Controller => Self::PS3Controller,
            steamworks::InputType::PS5Controller => Self::PS5Controller,
            steamworks::InputType::SteamDeckController => Self::SteamDeckController,
        }
    }
}

pub type InputActionOrigin = steamworks_sys::EInputActionOrigin;
pub type InputSourceMode = steamworks_sys::EInputSourceMode;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SteamInputMode {
    #[default]
    Off,
    Metadata,
    Fallback,
    Actions,
}

impl SteamInputMode {
    pub const fn allows_action_reads(self) -> bool {
        matches!(self, Self::Fallback | Self::Actions)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct InputHandle(u64);

impl InputHandle {
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ActionSetHandle(u64);

impl ActionSetHandle {
    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DigitalActionHandle(u64);

impl DigitalActionHandle {
    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AnalogActionHandle(u64);

impl AnalogActionHandle {
    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputController {
    pub handle: InputHandle,
    pub input_type: InputType,
    pub is_joycon: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DigitalActionData {
    pub state: bool,
    pub active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnalogActionData {
    pub mode: InputSourceMode,
    pub x: f32,
    pub y: f32,
    pub active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionData {
    pub rot_quat: [f32; 4],
    pub pos_accel: [f32; 3],
    pub rot_vel: [f32; 3],
}

pub const FALLBACK_ACTION_SET: &str = "perro_gamepad";
pub const FALLBACK_DIGITAL_ACTIONS: [&str; 18] = [
    "perro_bottom",
    "perro_right",
    "perro_left",
    "perro_top",
    "perro_dpad_up",
    "perro_dpad_down",
    "perro_dpad_left",
    "perro_dpad_right",
    "perro_start",
    "perro_select",
    "perro_home",
    "perro_capture",
    "perro_l1",
    "perro_r1",
    "perro_l2",
    "perro_r2",
    "perro_l3",
    "perro_r3",
];
pub const FALLBACK_ANALOG_ACTIONS: [&str; 4] = [
    "perro_left_stick",
    "perro_right_stick",
    "perro_left_trigger",
    "perro_right_trigger",
];

#[derive(Clone, Debug, PartialEq)]
pub struct FallbackGamepad {
    pub handle: InputHandle,
    pub input_type: InputType,
    pub buttons: [bool; 18],
    pub axes: [f32; 6],
    pub motion: MotionData,
}

#[derive(Clone, Copy)]
struct FallbackActions {
    set: u64,
    digital: [u64; 18],
    analog: [u64; 4],
}

fn fallback_actions() -> &'static Mutex<Option<FallbackActions>> {
    static ACTIONS: OnceLock<Mutex<Option<FallbackActions>>> = OnceLock::new();
    ACTIONS.get_or_init(|| Mutex::new(None))
}

pub const fn fallback_eligible(input_type: InputType, native_gamepad_present: bool) -> bool {
    match input_type {
        InputType::MobileTouch => true,
        InputType::SteamController | InputType::Unknown | InputType::GenericGamepad => {
            !native_gamepad_present
        }
        InputType::XBox360Controller
        | InputType::XBoxOneController
        | InputType::PS3Controller
        | InputType::PS4Controller
        | InputType::PS5Controller
        | InputType::SwitchProController
        | InputType::SwitchJoyConPair
        | InputType::SwitchJoyConSingle
        | InputType::SteamDeckController
        | InputType::AppleMFiController
        | InputType::AndroidController => false,
    }
}

pub fn fallback_gamepads(native_gamepad_present: bool) -> Result<Vec<FallbackGamepad>, SteamError> {
    if mode()? != SteamInputMode::Fallback {
        return Err(SteamError::Disabled);
    }
    app::with_client(|client| {
        let input = client.input();
        input.run_frame();
        let actions = {
            let mut cached = fallback_actions()
                .lock()
                .map_err(|_| SteamError::NotReady)?;
            *cached.get_or_insert_with(|| FallbackActions {
                set: input.get_action_set_handle(FALLBACK_ACTION_SET),
                digital: FALLBACK_DIGITAL_ACTIONS.map(|name| input.get_digital_action_handle(name)),
                analog: FALLBACK_ANALOG_ACTIONS.map(|name| input.get_analog_action_handle(name)),
            })
        };
        if actions.set == 0 {
            return Err(SteamError::CallFailed("input.fallback_action_set"));
        }

        let mut gamepads = Vec::new();
        for handle in input.get_connected_controllers() {
            let input_type: InputType = input.get_input_type_for_handle(handle).into();
            if !fallback_eligible(input_type, native_gamepad_present) {
                continue;
            }
            input.activate_action_set_handle(handle, actions.set);

            let mut buttons = [false; 18];
            for (index, action) in actions.digital.into_iter().enumerate() {
                if action == 0 {
                    continue;
                }
                let data = input.get_digital_action_data(handle, action);
                buttons[index] = data.bActive && data.bState;
            }

            let mut axes = [0.0; 6];
            for (index, action) in actions.analog.into_iter().enumerate() {
                if action == 0 {
                    continue;
                }
                let data = input.get_analog_action_data(handle, action);
                if !data.bActive {
                    continue;
                }
                match index {
                    0 => {
                        axes[0] = data.x;
                        axes[1] = data.y;
                    }
                    1 => {
                        axes[2] = data.x;
                        axes[3] = data.y;
                    }
                    2 => axes[4] = data.x.clamp(0.0, 1.0),
                    3 => axes[5] = data.x.clamp(0.0, 1.0),
                    _ => unreachable!(),
                }
            }
            buttons[14] |= axes[4] > 0.5;
            buttons[15] |= axes[5] > 0.5;

            let motion = input.get_motion_data(handle);
            gamepads.push(FallbackGamepad {
                handle: InputHandle(handle),
                input_type,
                buttons,
                axes,
                motion: MotionData {
                    rot_quat: [
                        motion.rotQuatX,
                        motion.rotQuatY,
                        motion.rotQuatZ,
                        motion.rotQuatW,
                    ],
                    pos_accel: [motion.posAccelX, motion.posAccelY, motion.posAccelZ],
                    rot_vel: [motion.rotVelX, motion.rotVelY, motion.rotVelZ],
                },
            });
        }
        Ok(gamepads)
    })
}

fn mode_state() -> &'static Mutex<SteamInputMode> {
    static MODE: OnceLock<Mutex<SteamInputMode>> = OnceLock::new();
    MODE.get_or_init(|| Mutex::new(SteamInputMode::Off))
}

pub(crate) fn init_for_mode(mode: SteamInputMode) -> Result<(), SteamError> {
    set_mode(mode)?;
    if mode == SteamInputMode::Off {
        return Ok(());
    }
    if !is_init(true)? {
        return Err(SteamError::CallFailed("input.init"));
    }
    Ok(())
}

pub(crate) fn set_mode(mode: SteamInputMode) -> Result<(), SteamError> {
    let mut state = mode_state().lock().map_err(|_| SteamError::NotReady)?;
    *state = mode;
    Ok(())
}

pub fn mode() -> Result<SteamInputMode, SteamError> {
    mode_state()
        .lock()
        .map(|state| *state)
        .map_err(|_| SteamError::NotReady)
}

pub fn is_init(explicitly_call_run_frame: bool) -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.input().init(explicitly_call_run_frame)))
}

pub fn run_frame() -> Result<(), SteamError> {
    app::with_client(|client| {
        client.input().run_frame();
        Ok(())
    })
}

pub fn get_connected_controllers() -> Result<Vec<InputHandle>, SteamError> {
    app::with_client(|client| {
        Ok(client
            .input()
            .get_connected_controllers()
            .into_iter()
            .map(InputHandle)
            .collect())
    })
}

pub fn get_controller_info() -> Result<Vec<InputController>, SteamError> {
    app::with_client(|client| {
        let input = client.input();
        Ok(input
            .get_connected_controllers()
            .into_iter()
            .map(|handle| {
                let input_type = InputType::from(input.get_input_type_for_handle(handle));
                InputController {
                    handle: InputHandle(handle),
                    input_type,
                    is_joycon: input_type_is_joycon(input_type),
                }
            })
            .collect())
    })
}

pub const fn input_type_is_joycon(input_type: InputType) -> bool {
    matches!(
        input_type,
        InputType::SwitchJoyConPair | InputType::SwitchJoyConSingle
    )
}

pub fn input_type(handle: InputHandle) -> Result<InputType, SteamError> {
    app::with_client(|client| {
        Ok(client
            .input()
            .get_input_type_for_handle(handle.raw())
            .into())
    })
}

pub fn is_action_manifest_set(path: &str) -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.input().set_input_action_manifest_file_path(path)))
}

pub fn is_binding_panel_shown(input_handle: InputHandle) -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.input().show_binding_panel(input_handle.raw())))
}

pub fn action_set_handle(name: &str) -> Result<ActionSetHandle, SteamError> {
    app::with_client(|client| Ok(ActionSetHandle(client.input().get_action_set_handle(name))))
}

pub fn activate_action_set(
    input_handle: InputHandle,
    action_set: ActionSetHandle,
) -> Result<(), SteamError> {
    app::with_client(|client| {
        client
            .input()
            .activate_action_set_handle(input_handle.raw(), action_set.raw());
        Ok(())
    })
}

pub fn digital_action_handle(name: &str) -> Result<DigitalActionHandle, SteamError> {
    app::with_client(|client| {
        Ok(DigitalActionHandle(
            client.input().get_digital_action_handle(name),
        ))
    })
}

pub fn analog_action_handle(name: &str) -> Result<AnalogActionHandle, SteamError> {
    app::with_client(|client| {
        Ok(AnalogActionHandle(
            client.input().get_analog_action_handle(name),
        ))
    })
}

pub fn digital_action_data(
    input_handle: InputHandle,
    action: DigitalActionHandle,
) -> Result<DigitalActionData, SteamError> {
    if !mode()?.allows_action_reads() {
        return Err(SteamError::Disabled);
    }
    app::with_client(|client| {
        let data = client
            .input()
            .get_digital_action_data(input_handle.raw(), action.raw());
        Ok(DigitalActionData {
            state: data.bState,
            active: data.bActive,
        })
    })
}

pub fn analog_action_data(
    input_handle: InputHandle,
    action: AnalogActionHandle,
) -> Result<AnalogActionData, SteamError> {
    if !mode()?.allows_action_reads() {
        return Err(SteamError::Disabled);
    }
    app::with_client(|client| {
        let data = client
            .input()
            .get_analog_action_data(input_handle.raw(), action.raw());
        let mode = data.eMode;
        let x = data.x;
        let y = data.y;
        let active = data.bActive;
        Ok(AnalogActionData { mode, x, y, active })
    })
}

pub fn digital_action_origins(
    input_handle: InputHandle,
    action_set: ActionSetHandle,
    action: DigitalActionHandle,
) -> Result<Vec<InputActionOrigin>, SteamError> {
    app::with_client(|client| {
        Ok(client.input().get_digital_action_origins(
            input_handle.raw(),
            action_set.raw(),
            action.raw(),
        ))
    })
}

pub fn analog_action_origins(
    input_handle: InputHandle,
    action_set: ActionSetHandle,
    action: AnalogActionHandle,
) -> Result<Vec<InputActionOrigin>, SteamError> {
    app::with_client(|client| {
        Ok(client.input().get_analog_action_origins(
            input_handle.raw(),
            action_set.raw(),
            action.raw(),
        ))
    })
}

pub fn glyph_for_action_origin(origin: InputActionOrigin) -> Result<String, SteamError> {
    app::with_client(|client| Ok(client.input().get_glyph_for_action_origin(origin)))
}

pub fn string_for_action_origin(origin: InputActionOrigin) -> Result<String, SteamError> {
    app::with_client(|client| Ok(client.input().get_string_for_action_origin(origin)))
}

pub fn motion_data(input_handle: InputHandle) -> Result<MotionData, SteamError> {
    app::with_client(|client| {
        let data = client.input().get_motion_data(input_handle.raw());
        let rot_quat_x = data.rotQuatX;
        let rot_quat_y = data.rotQuatY;
        let rot_quat_z = data.rotQuatZ;
        let rot_quat_w = data.rotQuatW;
        let pos_accel_x = data.posAccelX;
        let pos_accel_y = data.posAccelY;
        let pos_accel_z = data.posAccelZ;
        let rot_vel_x = data.rotVelX;
        let rot_vel_y = data.rotVelY;
        let rot_vel_z = data.rotVelZ;
        Ok(MotionData {
            rot_quat: [rot_quat_x, rot_quat_y, rot_quat_z, rot_quat_w],
            pos_accel: [pos_accel_x, pos_accel_y, pos_accel_z],
            rot_vel: [rot_vel_x, rot_vel_y, rot_vel_z],
        })
    })
}

pub fn shutdown() -> Result<(), SteamError> {
    app::with_client(|client| {
        client.input().shutdown();
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::{InputType, fallback_eligible};

    #[test]
    fn fallback_keeps_native_controller_types_native() {
        for input_type in [
            InputType::XBox360Controller,
            InputType::XBoxOneController,
            InputType::PS4Controller,
            InputType::PS5Controller,
            InputType::SwitchProController,
            InputType::SwitchJoyConPair,
            InputType::SwitchJoyConSingle,
            InputType::SteamDeckController,
        ] {
            assert!(!fallback_eligible(input_type, false));
        }
    }

    #[test]
    fn fallback_uses_steam_only_when_native_misses_pad() {
        assert!(fallback_eligible(InputType::SteamController, false));
        assert!(fallback_eligible(InputType::GenericGamepad, false));
        assert!(fallback_eligible(InputType::Unknown, false));
        assert!(!fallback_eligible(InputType::SteamController, true));
        assert!(!fallback_eligible(InputType::GenericGamepad, true));
        assert!(!fallback_eligible(InputType::Unknown, true));
        assert!(fallback_eligible(InputType::MobileTouch, true));
    }
}
