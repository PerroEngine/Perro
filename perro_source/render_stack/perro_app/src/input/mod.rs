mod gamepad;
mod joycon;
pub(crate) mod kbm;

pub use gamepad::GamepadInput;
pub use joycon::JoyConInput;
pub use kbm::KbmInput;

pub(crate) fn preinit_gamepads() {
    gamepad::preinit();
}
