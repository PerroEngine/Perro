//! Unified Joy-Con state structures
//!
//! Provides a unified API for both Joy-Con 1 (HID) and Joy-Con 2 (BLE) controllers
//! using Rust structs instead of JSON.

use crate::structs::{Vector2, Vector3};
use serde::{Serialize, Deserialize};

// ==========================================
// Joy-Con Identifiers
// ==========================================

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JoyconSide {
    Left,
    Right,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JoyconVersion {
    V1,
    V2,
}

// ==========================================
// Buttons (final: includes SL / SR)
// ==========================================

#[derive(Copy, Clone, Debug, Default, Serialize, Deserialize)]
pub struct JoyconButtons {
    // Right Joy-Con face buttons (ignored on Left)
    pub a: bool,
    pub b: bool,
    pub x: bool,
    pub y: bool,
    
    // Left Joy-Con D-pad (ignored on Right)
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    
    // Shoulders and rails
    // L/R are the regular shoulder buttons.
    // ZL/ZR are the triggers.
    // SL/SR are the small side buttons found on the rail (useful when Joy-Cons are used horizontally).
    pub l: bool,
    pub zl: bool,
    pub r: bool,
    pub zr: bool,
    pub sl: bool,   // side button left (or left-rail when attached)
    pub sr: bool,   // side button right (or right-rail when attached)
    
    // System buttons
    pub plus: bool,
    pub minus: bool,
    pub home: bool,
    pub capture: bool,
    
    // Joystick click
    pub stick_press: bool,
}

// ==========================================
// Joy-Con State
// ==========================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JoyconState {
    pub serial: String,               // Serial number or address identifier
    pub side: JoyconSide,             // Left or Right
    pub version: JoyconVersion,       // V1 or V2
    pub connected: bool,              // Controller still active
    pub buttons: JoyconButtons,       // All button states (includes sl/sr)
    pub stick: Vector2,               // normalized stick (-1.0..1.0)
    pub gyro: Vector3,                // gyro in radians/second (converted from degrees/second)
    pub accel: Vector3,               // accel in g-forces
}

impl Default for JoyconState {
    fn default() -> Self {
        Self {
            serial: String::new(),
            side: JoyconSide::Left,
            version: JoyconVersion::V1,
            connected: false,
            buttons: JoyconButtons::default(),
            stick: Vector2::zero(),
            gyro: Vector3::zero(),
            accel: Vector3::zero(),
        }
    }
}

impl JoyconState {
    /// Create a new JoyconState from an InputReport
    pub fn from_input_report(
        report: &crate::input::joycon::input_report::InputReport,
        serial: String,
        side: JoyconSide,
        version: JoyconVersion,
        connected: bool,
    ) -> Self {
        let buttons = JoyconButtons {
            a: report.buttons.a,
            b: report.buttons.b,
            x: report.buttons.x,
            y: report.buttons.y,
            up: report.buttons.up,
            down: report.buttons.down,
            left: report.buttons.left,
            right: report.buttons.right,
            l: report.buttons.l,
            zl: report.buttons.zl,
            r: report.buttons.r,
            zr: report.buttons.zr,
            sl: report.buttons.sl,
            sr: report.buttons.sr,
            plus: report.buttons.plus,
            minus: report.buttons.minus,
            home: report.buttons.home,
            capture: report.buttons.capture,
            stick_press: report.buttons.stick_press,
        };
        
        // Stick is already Vector2 with normalized values (-1.0 to 1.0)
        let stick = report.stick;
        
        // Convert gyro from degrees/second to radians/second for direct use in rotations
        // Remap Joy-Con axes to match engine coordinate system:
        // Joy-Con X → Engine Z (roll)
        // Joy-Con Y → Engine X (pitch)
        // Joy-Con Z → Engine Y (yaw, up/down)
        // Negate X and Y so all axes are positive (makes rotations more intuitive)
        const DEG_TO_RAD: f32 = std::f32::consts::PI / 180.0;
        
        // Convert gyro from degrees/second to radians/second for direct use in rotations
        // Remap Joy-Con axes to match engine coordinate system:
        // Joy-Con X → Engine Z (roll)
        // Joy-Con Y → Engine X (pitch)
        // Joy-Con Z → Engine Y (yaw, up/down)
        // Negate X and Y so all axes are positive (makes rotations more intuitive)
        // Note: Axis transformations are done in the individual decode functions (decode vs decode_joycon2)
        let gyro = Vector3::new(
            -report.gyro.y * DEG_TO_RAD,  // Joy-Con Y → Engine X (pitch, negated)
            -report.gyro.z * DEG_TO_RAD,  // Joy-Con Z → Engine Y (yaw, up/down, negated)
            report.gyro.x * DEG_TO_RAD,   // Joy-Con X → Engine Z (roll, positive)
        );
        
        // Remap accelerometer axes to match engine coordinate system:
        // When Joy-Con is flat forward: Y is up/down, gravity should be positive Y
        // Joy-Con X → Engine Z
        // Joy-Con Y → Engine X
        // Joy-Con Z → Engine Y (up/down, gravity is positive Y when flat)
        // Since gravity on Joy-Con Z is negative when flat, negate it to get positive Y
        let accel = Vector3::new(
            report.accel.y,   // Joy-Con Y → Engine X
            -report.accel.z,  // Joy-Con Z → Engine Y (negated so gravity is positive when flat)
            report.accel.x,   // Joy-Con X → Engine Z
        );
        
        Self {
            serial,
            side,
            version,
            connected,
            buttons,
            stick,
            gyro,
            accel,
        }
    }
}

