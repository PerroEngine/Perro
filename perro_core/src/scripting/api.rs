// scripting/lang/script_api.rs
//! Perro Script API (single-file version with Deref)
//! Provides all engine APIs (JSON, Time, OS, Process) directly under `api`

#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]

use chrono::{Datelike, Local, Timelike};
use serde::Serialize;
use serde_json::Value;
use smallvec::SmallVec;
use std::{
    cell::{RefCell, UnsafeCell},
    env, io,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    process,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use crate::ids::{NodeID, TextureID};

use crate::{
    app_command::AppCommand,
    asset_io::{self, ResolvedPath, resolve_path},
    compiler::{BuildProfile, CompileTarget, Compiler},
    manifest::Project,
    node_registry::BaseNode,
    prelude::string_to_u64,
    rendering::Graphics,
    script::{SceneAccess, ScriptObject},
    transpiler::{script_path_to_identifier, transpile},
    types::ScriptType,
    ui_element::BaseElement,
};

//-----------------------------------------------------
// 1️⃣ Sub‑APIs (Engine modules)
//-----------------------------------------------------

#[derive(Default)]
pub struct JsonApi;
impl JsonApi {
    #[cfg_attr(not(debug_assertions), inline)]
    pub fn stringify<T: Serialize>(&self, val: &T) -> String {
        serde_json::to_string(val).unwrap_or_else(|_| "{}".to_string())
    }
    #[cfg_attr(not(debug_assertions), inline)]
    pub fn parse(&self, text: &str) -> Option<Value> {
        serde_json::from_str(text).ok()
    }
}

#[derive(Default)]
pub struct TimeApi {
    pub delta: f32,
}
impl TimeApi {
    #[cfg_attr(not(debug_assertions), inline)]
    pub fn get_unix_time_msec(&self) -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_millis(0))
            .as_millis()
    }
    #[cfg_attr(not(debug_assertions), inline)]
    pub fn get_unix_time(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs()
    }
    pub fn get_datetime_string(&self) -> String {
        let now = Local::now();
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            now.year(),
            now.month(),
            now.day(),
            now.hour(),
            now.minute(),
            now.second()
        )
    }
    #[cfg_attr(not(debug_assertions), inline)]
    pub fn sleep_msec(&self, ms: u64) {
        thread::sleep(Duration::from_millis(ms));
    }
    #[cfg_attr(not(debug_assertions), inline)]
    pub fn get_ticks_msec(&self) -> u128 {
        self.get_unix_time_msec()
    }
    #[cfg_attr(not(debug_assertions), inline)]
    pub fn get_delta(&self) -> f32 {
        self.delta
    }
}

#[derive(Default)]
pub struct OsApi;
impl OsApi {
    pub fn get_platform_name(&self) -> String {
        env::consts::OS.to_string()
    }
    pub fn getenv(&self, key: &str) -> Option<String> {
        env::var(key).ok()
    }
    pub fn open_file_explorer(&self, path: &str) -> bool {
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("explorer")
                .arg(path)
                .status()
                .is_ok()
        }

        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("xdg-open")
                .arg(path)
                .status()
                .is_ok()
        }

        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(path)
                .status()
                .is_ok()
        }
    }
}

#[derive(Default)]
pub struct ProcessApi;
impl ProcessApi {
    pub fn quit(&self, code: i32) {
        process::exit(code);
    }
    pub fn get_launch_args(&self) -> Vec<String> {
        env::args().collect()
    }
    pub fn exec(&self, cmd: &str, args: &[&str]) -> bool {
        process::Command::new(cmd).args(args).status().is_ok()
    }
}

// Thread-local storage for current ScriptApi context
// This allows JoyConApi methods to access the ScriptApi without lifetime issues
thread_local! {
    static SCRIPT_API_CONTEXT: RefCell<Option<*mut ScriptApi<'static>>> = RefCell::new(None);
    // Track the current script ID being executed (for nested call detection)
    // Scripts are now stored as Rc<RefCell<Box<dyn ScriptObject>>>, so they're always in the HashMap
    // We just track the ID to detect when a script calls itself (nested calls)
    static CURRENT_SCRIPT_ID: RefCell<Option<NodeID>> = RefCell::new(None);
}

pub struct JoyConApi {
    // Store a pointer to the ScriptApi that owns this instance
    // This allows methods to access the ScriptApi without thread-local storage
    api_ptr: Option<*mut ScriptApi<'static>>,
}

impl Default for JoyConApi {
    fn default() -> Self {
        Self { api_ptr: None }
    }
}

impl JoyConApi {
    /// Set the ScriptApi pointer for this instance
    /// Called when accessed through DerefMut
    #[cfg_attr(not(debug_assertions), inline)]
    fn set_api_ptr(&mut self, api_ptr: *mut ScriptApi<'static>) {
        self.api_ptr = Some(api_ptr);
    }

    /// Get the ScriptApi pointer - tries stored pointer, then gets from parent InputApi
    #[cfg_attr(not(debug_assertions), inline)]
    fn get_api_ptr(&self) -> Option<*mut ScriptApi<'static>> {
        // First try stored pointer
        if let Some(ptr) = self.api_ptr {
            return Some(ptr);
        }

        // Try to get from parent InputApi
        // Since InputApi has JoyConApi as its first field, we can use pointer arithmetic
        // to get from JoyConApi* to InputApi*
        let joycon_ptr = self as *const JoyConApi;
        // InputApi layout: JoyConApi is first field, so InputApi* == JoyConApi* (same address)
        let input_api_ptr = joycon_ptr as *const InputApi;
        unsafe { (*input_api_ptr).get_parent_ptr() }
    }
}

impl JoyConApi {
    /// Scan for Joy-Con 1 devices (HID)
    ///
    /// NOTE: This method should be called through ScriptApi::scan_joycon1_direct()
    /// to avoid thread-local context issues. This public method is kept for API compatibility.
    pub fn scan_joycon1(&mut self) -> Vec<Value> {
        // Get ScriptApi pointer - try stored pointer first, then get from parent InputApi
        let api_ptr = if let Some(ptr) = self.get_api_ptr() {
            ptr
        } else {
            // Fallback to thread-local (shouldn't be needed if DerefMut works correctly)
            let tl_ptr = SCRIPT_API_CONTEXT.with(|ctx| *ctx.borrow());
            if let Some(ptr) = tl_ptr {
                // Store it for next time using unsafe
                let self_ptr = self as *mut JoyConApi;
                unsafe {
                    (*self_ptr).set_api_ptr(ptr);
                }
                ptr
            } else {
                return vec![];
            }
        };

        unsafe {
            let api = &mut *api_ptr;
            Self::scan_joycon1_impl(api)
        }
    }

    /// Internal method that actually does the scan
    pub(crate) fn scan_joycon1_impl(api: &mut ScriptApi) -> Vec<Value> {
        if let Some(mgr) = api.scene.get_controller_manager() {
            let mgr = mgr.lock().unwrap();

            match mgr.scan_joycon1() {
                Ok(devices) => devices
                    .into_iter()
                    .map(|(serial, vid, pid)| {
                        serde_json::json!({
                            "serial": serial,
                            "vendor_id": vid,
                            "product_id": pid
                        })
                    })
                    .collect(),
                Err(e) => {
                    api.print_error(&format!("Joy-Con scan failed: {:?}", e));
                    vec![]
                }
            }
        } else {
            api.print_error("No controller manager found in scene");
            vec![]
        }
    }

    /// Scan for Joy-Con 2 devices (BLE)
    /// Returns a vector of device addresses/identifiers as JSON
    pub fn scan_joycon2(&mut self) -> Vec<Value> {
        // Get ScriptApi pointer
        let api_ptr = if let Some(ptr) = self.get_api_ptr() {
            ptr
        } else {
            let tl_ptr = SCRIPT_API_CONTEXT.with(|ctx| *ctx.borrow());
            if let Some(ptr) = tl_ptr {
                let self_ptr = self as *mut JoyConApi;
                unsafe {
                    (*self_ptr).set_api_ptr(ptr);
                }
                ptr
            } else {
                return vec![];
            }
        };

        unsafe {
            let api = &mut *api_ptr;
            Self::scan_joycon2_impl(api)
        }
    }

    pub(crate) fn scan_joycon2_impl(api: &mut ScriptApi) -> Vec<Value> {
        if let Some(mgr) = api.scene.get_controller_manager() {
            let mgr = mgr.lock().unwrap();
            match mgr.scan_joycon2_sync() {
                Ok(devices) => devices
                    .into_iter()
                    .map(|address| {
                        serde_json::json!({
                            "address": address,
                        })
                    })
                    .collect(),
                Err(e) => {
                    api.print_error(&format!("Joy-Con 2 scan failed: {:?}", e));
                    vec![]
                }
            }
        } else {
            api.print_error("No controller manager found in scene");
            vec![]
        }
    }

    /// Connect to a Joy-Con 2 device (BLE)
    /// Returns true if connection was successful, false otherwise
    pub fn connect_joycon2(&mut self, address: &str) -> bool {
        // Get ScriptApi pointer
        let api_ptr = if let Some(ptr) = self.get_api_ptr() {
            ptr
        } else {
            let tl_ptr = SCRIPT_API_CONTEXT.with(|ctx| *ctx.borrow());
            if let Some(ptr) = tl_ptr {
                let self_ptr = self as *mut JoyConApi;
                unsafe {
                    (*self_ptr).set_api_ptr(ptr);
                }
                ptr
            } else {
                return false;
            }
        };

        unsafe {
            let api = &mut *api_ptr;
            Self::connect_joycon2_impl(api, address)
        }
    }

    pub(crate) fn connect_joycon2_impl(api: &mut ScriptApi, address: &str) -> bool {
        if let Some(mgr) = api.scene.get_controller_manager() {
            let mgr = mgr.lock().unwrap();
            match mgr.connect_joycon2_sync(address) {
                Ok(_) => {
                    api.print(&format!("Successfully connected to Joy-Con 2: {}", address));
                    true
                }
                Err(e) => {
                    api.print_error(&format!(
                        "Failed to connect to Joy-Con 2 {}: {:?}",
                        address, e
                    ));
                    false
                }
            }
        } else {
            api.print_error("No controller manager found in scene");
            false
        }
    }

    /// Connect to a Joy-Con 1 device
    /// Returns true if connection was successful, false otherwise
    pub fn connect_joycon1(&mut self, serial: &str, vid: u64, pid: u64) -> bool {
        // Get ScriptApi pointer - try stored pointer first, then get from parent InputApi
        let api_ptr = if let Some(ptr) = self.get_api_ptr() {
            ptr
        } else {
            // Fallback to thread-local (shouldn't be needed if DerefMut works correctly)
            let tl_ptr = SCRIPT_API_CONTEXT.with(|ctx| *ctx.borrow());
            if let Some(ptr) = tl_ptr {
                let self_ptr = self as *mut JoyConApi;
                unsafe {
                    (*self_ptr).set_api_ptr(ptr);
                }
                ptr
            } else {
                return false;
            }
        };

        unsafe {
            let api = &mut *api_ptr;
            Self::connect_joycon1_impl(api, serial, vid, pid)
        }
    }

    pub(crate) fn connect_joycon1_impl(
        api: &mut ScriptApi,
        serial: &str,
        vid: u64,
        pid: u64,
    ) -> bool {
        if let Some(mgr) = api.scene.get_controller_manager() {
            let mgr = mgr.lock().unwrap();
            match mgr.connect_joycon1(serial, vid as u16, pid as u16) {
                Ok(_) => {
                    api.print(&format!("Successfully connected to Joy-Con: {}", serial));
                    true
                }
                Err(e) => {
                    api.print_error(&format!("Failed to connect to Joy-Con {}: {:?}", serial, e));
                    false
                }
            }
        } else {
            api.print_error("No controller manager found in scene");
            false
        }
    }

    /// Get data from all connected controllers as structs
    /// Returns Vec<JoyconState> directly - allows direct field access like controller.gyro.x
    pub fn get_data(&mut self) -> Vec<crate::input::joycon::JoyconState> {
        // Get ScriptApi pointer - try stored pointer first, then get from parent InputApi
        let api_ptr = if let Some(ptr) = self.get_api_ptr() {
            ptr
        } else {
            // Fallback to thread-local (shouldn't be needed if DerefMut works correctly)
            let tl_ptr = SCRIPT_API_CONTEXT.with(|ctx| *ctx.borrow());
            if let Some(ptr) = tl_ptr {
                let self_ptr = self as *mut JoyConApi;
                unsafe {
                    (*self_ptr).set_api_ptr(ptr);
                }
                ptr
            } else {
                return vec![];
            }
        };

        unsafe {
            let api = &mut *api_ptr;
            Self::get_data_impl(api)
        }
    }

    pub(crate) fn get_data_impl(api: &mut ScriptApi) -> Vec<crate::input::joycon::JoyconState> {
        if let Some(mgr) = api.scene.get_controller_manager() {
            let mgr = mgr.lock().unwrap();
            let data = mgr.get_data();
            data.into_iter()
                .map(|controller| {
                    // Use the unified state if available, otherwise convert from report
                    if let Some(mut state) = controller.state {
                        // Ensure serial is set (in case it wasn't set when state was created)
                        if state.serial.is_empty() {
                            state.serial = controller.serial.clone();
                        }
                        state
                    } else if let Some(ref report) = controller.latest_report {
                        use crate::input::joycon::{JoyconSide, JoyconState, JoyconVersion};
                        let side = if controller.is_left {
                            JoyconSide::Left
                        } else {
                            JoyconSide::Right
                        };
                        let version = if controller.is_joycon2 {
                            JoyconVersion::V2
                        } else {
                            JoyconVersion::V1
                        };
                        JoyconState::from_input_report(
                            report,
                            controller.serial.clone(),
                            side,
                            version,
                            true,
                        )
                    } else {
                        // No data available
                        use crate::input::joycon::{
                            JoyconButtons, JoyconSide, JoyconState, JoyconVersion,
                        };
                        use crate::structs::{Vector2, Vector3};
                        JoyconState {
                            serial: controller.serial.clone(),
                            stick: Vector2::ZERO,
                            gyro: Vector3::ZERO,
                            accel: Vector3::ZERO,
                            buttons: JoyconButtons::default(),
                            side: if controller.is_left {
                                JoyconSide::Left
                            } else {
                                JoyconSide::Right
                            },
                            version: if controller.is_joycon2 {
                                JoyconVersion::V2
                            } else {
                                JoyconVersion::V1
                            },
                            connected: false,
                        }
                    }
                })
                .collect()
        } else {
            vec![]
        }
    }

    /// Enable polling
    pub fn enable_polling(&mut self) -> bool {
        // Get ScriptApi pointer - try stored pointer first, then get from parent InputApi
        let api_ptr = if let Some(ptr) = self.get_api_ptr() {
            ptr
        } else {
            // Fallback to thread-local (shouldn't be needed if DerefMut works correctly)
            let tl_ptr = SCRIPT_API_CONTEXT.with(|ctx| *ctx.borrow());
            if let Some(ptr) = tl_ptr {
                let self_ptr = self as *mut JoyConApi;
                unsafe {
                    (*self_ptr).set_api_ptr(ptr);
                }
                ptr
            } else {
                return false;
            }
        };

        unsafe {
            let api = &mut *api_ptr;
            Self::enable_polling_impl(api)
        }
    }

    pub(crate) fn enable_polling_impl(api: &mut ScriptApi) -> bool {
        if let Some(mgr) = api.scene.get_controller_manager() {
            let mut mgr = mgr.lock().unwrap();
            api.print("Enabling Joy-Con polling...");
            api.print("Scanning for Joy-Con 1 devices (HID)...");

            // Scan Joy-Con 1 first and show results
            match mgr.scan_joycon1() {
                Ok(devices) => {
                    api.print(&format!("Found {} Joy-Con 1 device(s)", devices.len()));
                    for (serial, vid, pid) in &devices {
                        api.print(&format!(
                            "  - {} (VID: 0x{:04X}, PID: 0x{:04X})",
                            serial, vid, pid
                        ));
                    }
                }
                Err(e) => {
                    api.print_error(&format!("Failed to scan Joy-Con 1: {:?}", e));
                }
            }

            match mgr.enable_polling() {
                Ok(_) => {
                    api.print("Joy-Con polling enabled");
                    api.print("Joy-Con 1: Connecting and starting polling...");
                    api.print("Joy-Con 2: Starting background scan (may take 5-10 seconds)...");
                    api.print("Check console for Joy-Con 2 connection status");
                    true
                }
                Err(e) => {
                    api.print_error(&format!("Failed to enable Joy-Con polling: {:?}", e));
                    false
                }
            }
        } else {
            api.print_error("No controller manager found in scene");
            false
        }
    }

    /// Disable polling
    pub fn disable_polling(&mut self) {
        SCRIPT_API_CONTEXT.with(|ctx| {
            if let Some(api_ptr) = *ctx.borrow() {
                unsafe {
                    let api = &mut *api_ptr;
                    Self::disable_polling_impl(api);
                }
            }
        })
    }

    pub(crate) fn disable_polling_impl(api: &mut ScriptApi) {
        if let Some(mgr) = api.scene.get_controller_manager() {
            let mut mgr = mgr.lock().unwrap();
            mgr.disable_polling();
        }
    }

    /// Poll Joy-Con 1 synchronously (call from main loop)
    pub fn poll_joycon1_sync(&mut self) {
        // Get ScriptApi pointer - try stored pointer first, then get from parent InputApi
        let api_ptr = if let Some(ptr) = self.get_api_ptr() {
            ptr
        } else {
            // Fallback to thread-local (shouldn't be needed if DerefMut works correctly)
            let tl_ptr = SCRIPT_API_CONTEXT.with(|ctx| *ctx.borrow());
            if let Some(ptr) = tl_ptr {
                let self_ptr = self as *mut JoyConApi;
                unsafe {
                    (*self_ptr).set_api_ptr(ptr);
                }
                ptr
            } else {
                eprintln!("[poll_joycon1_sync] ERROR: No context available!");
                return;
            }
        };

        unsafe {
            let api = &mut *api_ptr;
            Self::poll_joycon1_sync_impl(api);
        }
    }

    pub(crate) fn poll_joycon1_sync_impl(api: &mut ScriptApi) {
        if let Some(mgr) = api.scene.get_controller_manager() {
            let mgr = mgr.lock().unwrap();
            mgr.poll_joycon1_sync();
        }
    }

    /// Check if polling is enabled
    pub fn is_polling_enabled(&mut self) -> bool {
        SCRIPT_API_CONTEXT.with(|ctx| {
            if let Some(api_ptr) = *ctx.borrow() {
                unsafe {
                    let api = &mut *api_ptr;
                    Self::is_polling_enabled_impl(api)
                }
            } else {
                false
            }
        })
    }

    pub(crate) fn is_polling_enabled_impl(api: &mut ScriptApi) -> bool {
        if let Some(mgr) = api.scene.get_controller_manager() {
            let mgr = mgr.lock().unwrap();
            mgr.is_polling_enabled()
        } else {
            false
        }
    }
}

pub struct ControllerApi {
    // Pointer to the parent ScriptApi - set when accessed through DerefMut
    api_ptr: Option<*mut ScriptApi<'static>>,
}

impl Default for ControllerApi {
    fn default() -> Self {
        Self { api_ptr: None }
    }
}

impl ControllerApi {
    /// Set the ScriptApi pointer for this instance
    #[cfg_attr(not(debug_assertions), inline)]
    fn set_api_ptr(&mut self, api_ptr: *mut ScriptApi<'static>) {
        self.api_ptr = Some(api_ptr);
    }

    /// Get the ScriptApi pointer
    #[cfg_attr(not(debug_assertions), inline)]
    fn get_api_ptr(&self) -> Option<*mut ScriptApi<'static>> {
        if let Some(ptr) = self.api_ptr {
            return Some(ptr);
        }
        // Try to get from parent InputApi
        let controller_ptr = self as *const ControllerApi;
        let input_api_ptr = controller_ptr as *const InputApi;
        unsafe { (*input_api_ptr).get_parent_ptr() }
    }

    /// Enable the controller manager
    /// This must be called before using any controller functionality
    pub fn enable(&mut self) -> bool {
        let api_ptr = if let Some(ptr) = self.get_api_ptr() {
            ptr
        } else {
            let tl_ptr = SCRIPT_API_CONTEXT.with(|ctx| *ctx.borrow());
            if let Some(ptr) = tl_ptr {
                let self_ptr = self as *mut ControllerApi;
                unsafe {
                    (*self_ptr).set_api_ptr(ptr);
                }
                ptr
            } else {
                return false;
            }
        };

        unsafe {
            let api = &mut *api_ptr;
            Self::enable_impl(api)
        }
    }

    pub(crate) fn enable_impl(api: &mut ScriptApi) -> bool {
        api.scene.enable_controller_manager()
    }
}

pub struct KeyboardApi {
    // Pointer to the parent ScriptApi - set when accessed through DerefMut
    api_ptr: Option<*mut ScriptApi<'static>>,
}

impl Default for KeyboardApi {
    fn default() -> Self {
        Self { api_ptr: None }
    }
}

impl KeyboardApi {
    #[cfg_attr(not(debug_assertions), inline)]
    fn get_api_ptr(&mut self) -> Option<*mut ScriptApi<'static>> {
        if let Some(ptr) = self.api_ptr {
            return Some(ptr);
        }
        let input_api_ptr = self as *const KeyboardApi as *const InputApi;
        unsafe { (*input_api_ptr).get_parent_ptr() }
    }

    #[cfg_attr(not(debug_assertions), inline)]
    fn set_api_ptr(&mut self, api_ptr: *mut ScriptApi<'static>) {
        self.api_ptr = Some(api_ptr);
    }

    /// Check if a key is pressed
    pub fn is_key_pressed<S: AsRef<str>>(&mut self, key: S) -> bool {
        if let Some(api_ptr) = self.get_api_ptr() {
            unsafe {
                let api = &mut *api_ptr;
                Self::is_key_pressed_impl(api, key.as_ref())
            }
        } else {
            false
        }
    }

    pub(crate) fn is_key_pressed_impl(api: &mut ScriptApi, key: &str) -> bool {
        use crate::input::manager::{InputSource, parse_input_source};

        if let Some(InputSource::Key(keycode)) = parse_input_source(key) {
            if let Some(mgr) = api.scene.get_input_manager() {
                let mgr = mgr.lock().unwrap();
                mgr.is_key_pressed(keycode)
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Get text input buffer (accumulated text input events)
    pub fn get_text_input(&mut self) -> String {
        if let Some(api_ptr) = self.get_api_ptr() {
            unsafe {
                let api = &mut *api_ptr;
                Self::get_text_input_impl(api)
            }
        } else {
            String::new()
        }
    }

    pub(crate) fn get_text_input_impl(api: &mut ScriptApi) -> String {
        if let Some(mgr) = api.scene.get_input_manager() {
            let mgr = mgr.lock().unwrap();
            mgr.get_text_input().to_string()
        } else {
            String::new()
        }
    }

    /// Clear text input buffer (call after processing text input)
    pub fn clear_text_input(&mut self) {
        if let Some(api_ptr) = self.get_api_ptr() {
            unsafe {
                let api = &mut *api_ptr;
                Self::clear_text_input_impl(api);
            }
        }
    }

    pub(crate) fn clear_text_input_impl(api: &mut ScriptApi) {
        if let Some(mgr) = api.scene.get_input_manager() {
            let mut mgr = mgr.lock().unwrap();
            mgr.clear_text_input();
        }
    }
}

pub struct MouseApi {
    api_ptr: Option<*mut ScriptApi<'static>>,
}

impl Default for MouseApi {
    fn default() -> Self {
        Self { api_ptr: None }
    }
}

impl MouseApi {
    #[cfg_attr(not(debug_assertions), inline)]
    fn get_api_ptr(&mut self) -> Option<*mut ScriptApi<'static>> {
        if let Some(ptr) = self.api_ptr {
            return Some(ptr);
        }
        let input_api_ptr = self as *const MouseApi as *const InputApi;
        unsafe { (*input_api_ptr).get_parent_ptr() }
    }

    #[cfg_attr(not(debug_assertions), inline)]
    fn set_api_ptr(&mut self, api_ptr: *mut ScriptApi<'static>) {
        self.api_ptr = Some(api_ptr);
    }

    /// Get mouse position in screen space
    pub fn get_position(&mut self) -> crate::structs2d::vector2::Vector2 {
        if let Some(api_ptr) = self.get_api_ptr() {
            unsafe {
                let api = &mut *api_ptr;
                Self::get_position_impl(api)
            }
        } else {
            crate::structs2d::vector2::Vector2::ZERO
        }
    }

    pub(crate) fn get_position_impl(api: &mut ScriptApi) -> crate::structs2d::vector2::Vector2 {
        if let Some(mgr) = api.scene.get_input_manager() {
            let mgr = mgr.lock().unwrap();
            mgr.get_mouse_position()
        } else {
            crate::structs2d::vector2::Vector2::ZERO
        }
    }

    /// Check if a mouse button is pressed
    pub fn is_button_pressed<S: AsRef<str>>(&mut self, button: S) -> bool {
        if let Some(api_ptr) = self.get_api_ptr() {
            unsafe {
                let api = &mut *api_ptr;
                Self::is_button_pressed_impl(api, button.as_ref())
            }
        } else {
            false
        }
    }

    pub(crate) fn is_button_pressed_impl(api: &mut ScriptApi, button: &str) -> bool {
        use crate::input::manager::{InputSource, parse_input_source};

        if let Some(InputSource::MouseButton(btn)) = parse_input_source(button) {
            if let Some(mgr) = api.scene.get_input_manager() {
                let mgr = mgr.lock().unwrap();
                mgr.is_mouse_button_pressed(btn)
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Get scroll wheel delta
    pub fn get_scroll_delta(&mut self) -> f32 {
        if let Some(api_ptr) = self.get_api_ptr() {
            unsafe {
                let api = &mut *api_ptr;
                Self::get_scroll_delta_impl(api)
            }
        } else {
            0.0
        }
    }

    pub(crate) fn get_scroll_delta_impl(api: &mut ScriptApi) -> f32 {
        if let Some(mgr) = api.scene.get_input_manager() {
            let mgr = mgr.lock().unwrap();
            mgr.get_scroll_delta()
        } else {
            0.0
        }
    }

    /// Check if mouse wheel scrolled up this frame
    pub fn is_wheel_up(&mut self) -> bool {
        if let Some(api_ptr) = self.get_api_ptr() {
            unsafe {
                let api = &mut *api_ptr;
                Self::is_wheel_up_impl(api)
            }
        } else {
            false
        }
    }

    pub(crate) fn is_wheel_up_impl(api: &mut ScriptApi) -> bool {
        if let Some(mgr) = api.scene.get_input_manager() {
            let mgr = mgr.lock().unwrap();
            mgr.is_mouse_wheel_up()
        } else {
            false
        }
    }

    /// Check if mouse wheel scrolled down this frame
    pub fn is_wheel_down(&mut self) -> bool {
        if let Some(api_ptr) = self.get_api_ptr() {
            unsafe {
                let api = &mut *api_ptr;
                Self::is_wheel_down_impl(api)
            }
        } else {
            false
        }
    }

    pub(crate) fn is_wheel_down_impl(api: &mut ScriptApi) -> bool {
        if let Some(mgr) = api.scene.get_input_manager() {
            let mgr = mgr.lock().unwrap();
            mgr.is_mouse_wheel_down()
        } else {
            false
        }
    }

    /// Get mouse position in world space (requires active camera)
    /// Returns None if no active camera or world position not available
    pub fn get_position_world(&mut self) -> Option<crate::structs2d::vector2::Vector2> {
        if let Some(api_ptr) = self.get_api_ptr() {
            unsafe {
                let api = &mut *api_ptr;
                Self::get_position_world_impl(api)
            }
        } else {
            None
        }
    }

    pub(crate) fn get_position_world_impl(
        api: &mut ScriptApi,
    ) -> Option<crate::structs2d::vector2::Vector2> {
        if let Some(mgr) = api.scene.get_input_manager() {
            let mgr = mgr.lock().unwrap();
            mgr.get_mouse_position_world()
        } else {
            None
        }
    }

    /// Convert screen coordinates to world coordinates using camera transform
    /// For 2D cameras: takes camera position, rotation, zoom, and virtual screen size
    pub fn screen_to_world(
        &mut self,
        camera_pos: crate::structs2d::vector2::Vector2,
        camera_rotation: f32,
        camera_zoom: f32,
        virtual_width: f32,
        virtual_height: f32,
        window_width: f32,
        window_height: f32,
    ) -> crate::structs2d::vector2::Vector2 {
        if let Some(api_ptr) = self.get_api_ptr() {
            unsafe {
                let api = &mut *api_ptr;
                let screen_pos = Self::get_position_impl(api);
                if let Some(mgr) = api.scene.get_input_manager() {
                    let mgr = mgr.lock().unwrap();
                    mgr.screen_to_world_2d(
                        screen_pos,
                        camera_pos,
                        camera_rotation,
                        camera_zoom,
                        virtual_width,
                        virtual_height,
                        window_width,
                        window_height,
                    )
                } else {
                    crate::structs2d::vector2::Vector2::ZERO
                }
            }
        } else {
            crate::structs2d::vector2::Vector2::ZERO
        }
    }
}

pub struct InputApi {
    pub Controller: ControllerApi,
    pub JoyCon: JoyConApi,
    pub Keyboard: KeyboardApi,
    pub Mouse: MouseApi,
    // Pointer to the parent ScriptApi - set when accessed through DerefMut
    parent_api_ptr: Option<*mut ScriptApi<'static>>,
}

impl Default for InputApi {
    fn default() -> Self {
        Self {
            Controller: ControllerApi::default(),
            JoyCon: JoyConApi::default(),
            Keyboard: KeyboardApi::default(),
            Mouse: MouseApi::default(),
            parent_api_ptr: None,
        }
    }
}

impl InputApi {
    /// Set the parent ScriptApi pointer
    #[cfg_attr(not(debug_assertions), inline)]
    fn set_parent_ptr(&mut self, api_ptr: *mut ScriptApi<'static>) {
        self.parent_api_ptr = Some(api_ptr);
        // Also set it in sub-APIs
        self.Controller.set_api_ptr(api_ptr);
        self.JoyCon.set_api_ptr(api_ptr);
        self.Keyboard.set_api_ptr(api_ptr);
        self.Mouse.set_api_ptr(api_ptr);
    }

    /// Set the parent ScriptApi pointer (immutable version for Deref)
    #[cfg_attr(not(debug_assertions), inline)]
    fn set_parent_ptr_immut(&self, api_ptr: *mut ScriptApi<'static>) {
        unsafe {
            let self_mut = self as *const InputApi as *mut InputApi;
            (*self_mut).parent_api_ptr = Some(api_ptr);
            (*self_mut).Controller.set_api_ptr(api_ptr);
            (*self_mut).JoyCon.set_api_ptr(api_ptr);
            (*self_mut).Keyboard.set_api_ptr(api_ptr);
            (*self_mut).Mouse.set_api_ptr(api_ptr);
        }
    }

    /// Get the parent ScriptApi pointer
    #[cfg_attr(not(debug_assertions), inline)]
    fn get_parent_ptr(&self) -> Option<*mut ScriptApi<'static>> {
        self.parent_api_ptr
    }

    /// Check if an action is currently pressed
    /// Actions are defined in project.toml [input] section
    pub fn get_action<S: AsRef<str>>(&mut self, action: S) -> bool {
        if let Some(api_ptr) = self.get_parent_ptr() {
            unsafe {
                let api = &mut *api_ptr;
                Self::get_action_impl(api, action.as_ref())
            }
        } else {
            false
        }
    }

    pub(crate) fn get_action_impl(api: &mut ScriptApi, action: &str) -> bool {
        if let Some(mgr) = api.scene.get_input_manager() {
            let mgr = mgr.lock().unwrap();
            mgr.is_action_pressed(action)
        } else {
            false
        }
    }
}

//-----------------------------------------------------
// 2️⃣ Engine API Aggregator
//-----------------------------------------------------

pub struct TextureApi {
    // Pointer to the parent ScriptApi - set when accessed through DerefMut
    api_ptr: Option<*mut ScriptApi<'static>>,
}

impl Default for TextureApi {
    fn default() -> Self {
        Self { api_ptr: None }
    }
}

impl TextureApi {
    /// Set the ScriptApi pointer for this instance
    /// Called when accessed through DerefMut
    #[cfg_attr(not(debug_assertions), inline)]
    fn set_api_ptr(&mut self, api_ptr: *mut ScriptApi<'static>) {
        self.api_ptr = Some(api_ptr);
    }

    /// Set the ScriptApi pointer (immutable version for Deref)
    #[cfg_attr(not(debug_assertions), inline)]
    fn set_api_ptr_immut(&self, api_ptr: *mut ScriptApi<'static>) {
        unsafe {
            let self_mut = self as *const TextureApi as *mut TextureApi;
            (*self_mut).api_ptr = Some(api_ptr);
        }
    }

    /// Get the ScriptApi pointer
    #[cfg_attr(not(debug_assertions), inline)]
    fn get_api_ptr(&self) -> Option<*mut ScriptApi<'static>> {
        self.api_ptr
    }

    /// Load a texture from a path
    /// Panics if Graphics is not available or texture cannot be loaded
    pub fn load(&mut self, path: &str) -> Option<crate::ids::TextureID> {
        // Get ScriptApi pointer - try stored pointer first
        let api_ptr = if let Some(ptr) = self.get_api_ptr() {
            ptr
        } else {
            // Fallback to thread-local (shouldn't be needed if DerefMut works correctly)
            let tl_ptr = SCRIPT_API_CONTEXT.with(|ctx| *ctx.borrow());
            if let Some(ptr) = tl_ptr {
                // Store it for next time
                self.set_api_ptr(ptr);
                ptr
            } else {
                eprintln!("[Texture.load] ERROR: No ScriptApi context available!");
                return None;
            }
        };

        unsafe {
            let api = &mut *api_ptr;
            Some(Self::load_impl(api, path))
        }
    }

    /// Internal implementation that actually does the load
    /// Panics if Graphics is not available or texture cannot be loaded
    pub(crate) fn load_impl(api: &mut ScriptApi, path: &str) -> crate::ids::TextureID {
        if let Some(gfx) = api.gfx.as_mut() {
            match gfx.texture_manager.get_or_load_texture_id(path, &gfx.device, &gfx.queue) {
                Ok(id) => id,
                Err(e) => {
                    panic!("{}", e);
                }
            }
        } else {
            panic!("Graphics not available");
        }
    }

    /// Preload a texture from path and pin it so it is never evicted; only Texture.remove(id) frees it.
    pub fn preload(&mut self, path: &str) -> Option<TextureID> {
        let api_ptr = if let Some(ptr) = self.get_api_ptr() {
            ptr
        } else {
            let tl_ptr = SCRIPT_API_CONTEXT.with(|ctx| *ctx.borrow());
            if let Some(ptr) = tl_ptr {
                self.set_api_ptr(ptr);
                ptr
            } else {
                eprintln!("[Texture.preload] ERROR: No ScriptApi context available!");
                return None;
            }
        };
        unsafe {
            let api = &mut *api_ptr;
            Some(Self::preload_impl(api, path))
        }
    }

    pub(crate) fn preload_impl(api: &mut ScriptApi, path: &str) -> TextureID {
        if let Some(gfx) = api.gfx.as_mut() {
            let id = match gfx.texture_manager.get_or_load_texture_id(path, &gfx.device, &gfx.queue) {
                Ok(id) => id,
                Err(e) => panic!("{}", e),
            };
            gfx.texture_manager.pin_texture(id);
            id
        } else {
            panic!("Graphics not available");
        }
    }

    /// Remove (unpin and free) a texture by id. Safe to call with nil or already-removed id.
    pub fn remove(&mut self, id: Option<TextureID>) {
        let api_ptr = if let Some(ptr) = self.get_api_ptr() {
            Some(ptr)
        } else {
            let tl_ptr = SCRIPT_API_CONTEXT.with(|ctx| *ctx.borrow());
            tl_ptr.map(|ptr| {
                self.set_api_ptr(ptr);
                ptr
            })
        };
        if let Some(api_ptr) = api_ptr {
            unsafe {
                let api = &mut *api_ptr;
                Self::remove_impl(api, id);
            }
        }
    }

    pub(crate) fn remove_impl(api: &mut ScriptApi, id: Option<TextureID>) {
        if let Some(id) = id {
            if let Some(gfx) = api.gfx.as_mut() {
                gfx.texture_manager.unpin_texture(id);
                gfx.texture_manager.remove_texture(id);
            }
        }
    }

    /// Create a texture from raw RGBA8 bytes and return its UUID
    pub fn create_from_bytes(&mut self, bytes: &[u8], width: u32, height: u32) -> Option<TextureID> {
        // Get ScriptApi pointer - try stored pointer first
        let api_ptr = if let Some(ptr) = self.get_api_ptr() {
            ptr
        } else {
            // Fallback to thread-local (shouldn't be needed if DerefMut works correctly)
            let tl_ptr = SCRIPT_API_CONTEXT.with(|ctx| *ctx.borrow());
            if let Some(ptr) = tl_ptr {
                // Store it for next time
                self.set_api_ptr(ptr);
                ptr
            } else {
                eprintln!("[Texture.create_from_bytes] ERROR: No ScriptApi context available!");
                return None;
            }
        };

        unsafe {
            let api = &mut *api_ptr;
            Self::create_from_bytes_impl(api, bytes, width, height)
        }
    }

    /// Internal implementation that actually creates the texture
    pub(crate) fn create_from_bytes_impl(api: &mut ScriptApi, bytes: &[u8], width: u32, height: u32) -> Option<TextureID> {
        if let Some(gfx) = api.gfx.as_mut() {
            Some(gfx.texture_manager.create_texture_from_bytes(bytes, width, height, &gfx.device, &gfx.queue))
        } else {
            api.print_error("Graphics not available - Texture.create_from_bytes() requires Graphics access");
            None
        }
    }

    /// Get the width of a texture by its UUID
    pub fn get_width(&self, id: TextureID) -> u32 {
        // Get ScriptApi pointer - try stored pointer first
        let api_ptr = if let Some(ptr) = self.get_api_ptr() {
            ptr
        } else {
            // Fallback to thread-local (shouldn't be needed if Deref works correctly)
            let tl_ptr = SCRIPT_API_CONTEXT.with(|ctx| *ctx.borrow());
            if let Some(ptr) = tl_ptr {
                // Store it for next time (need mutable access)
                unsafe {
                    let self_mut = self as *const TextureApi as *mut TextureApi;
                    (*self_mut).set_api_ptr(ptr);
                }
                ptr
            } else {
                return 0;
            }
        };

        unsafe {
            let api = &mut *api_ptr;
            Self::get_width_impl(api, id)
        }
    }

    /// Internal implementation
    pub(crate) fn get_width_impl(api: &mut ScriptApi, id: TextureID) -> u32 {
        if let Some(gfx) = api.gfx.as_mut() {
            gfx.texture_manager
                .get_texture_by_id(&id)
                .map(|tex| tex.width)
                .unwrap_or(0)
        } else {
            // Graphics not available, but we can still try to get from scene if texture was already loaded
            // For now, just return 0
            0
        }
    }

    /// Get the height of a texture by its UUID
    pub fn get_height(&self, id: TextureID) -> u32 {
        // Get ScriptApi pointer - try stored pointer first
        let api_ptr = if let Some(ptr) = self.get_api_ptr() {
            ptr
        } else {
            // Fallback to thread-local (shouldn't be needed if Deref works correctly)
            let tl_ptr = SCRIPT_API_CONTEXT.with(|ctx| *ctx.borrow());
            if let Some(ptr) = tl_ptr {
                // Store it for next time (need mutable access)
                unsafe {
                    let self_mut = self as *const TextureApi as *mut TextureApi;
                    (*self_mut).set_api_ptr(ptr);
                }
                ptr
            } else {
                return 0;
            }
        };

        unsafe {
            let api = &mut *api_ptr;
            Self::get_height_impl(api, id)
        }
    }

    /// Internal implementation
    pub(crate) fn get_height_impl(api: &mut ScriptApi, id: TextureID) -> u32 {
        if let Some(gfx) = api.gfx.as_mut() {
            gfx.texture_manager
                .get_texture_by_id(&id)
                .map(|tex| tex.height)
                .unwrap_or(0)
        } else {
            0
        }
    }

    /// Get the size of a texture by its UUID (returns Vector2)
    pub fn get_size(&self, id: TextureID) -> crate::Vector2 {
        // Get ScriptApi pointer - try stored pointer first
        let api_ptr = if let Some(ptr) = self.get_api_ptr() {
            ptr
        } else {
            // Fallback to thread-local (shouldn't be needed if Deref works correctly)
            let tl_ptr = SCRIPT_API_CONTEXT.with(|ctx| *ctx.borrow());
            if let Some(ptr) = tl_ptr {
                // Store it for next time (need mutable access)
                unsafe {
                    let self_mut = self as *const TextureApi as *mut TextureApi;
                    (*self_mut).set_api_ptr(ptr);
                }
                ptr
            } else {
                return crate::Vector2::new(0.0, 0.0);
            }
        };

        unsafe {
            let api = &mut *api_ptr;
            Self::get_size_impl(api, id)
        }
    }

    /// Internal implementation
    pub(crate) fn get_size_impl(api: &mut ScriptApi, id: TextureID) -> crate::Vector2 {
        if let Some(gfx) = api.gfx.as_mut() {
            gfx.texture_manager
                .get_texture_size_by_id(&id)
                .unwrap_or_else(|| crate::Vector2::new(0.0, 0.0))
        } else {
            crate::Vector2::new(0.0, 0.0)
        }
    }
}

#[derive(Default)]
pub struct DirectoryApi;
impl DirectoryApi {
    /// Scan a directory recursively and return all file paths
    /// Paths are returned relative to the base directory
    /// Example: `let files = api.Directory.scan("res://");`
    pub fn scan(&self, path: &str) -> Vec<String> {
        use walkdir::WalkDir;
        use crate::asset_io::resolve_path;
        
        let mut files = Vec::new();
        
        eprintln!("🔍 [DirectoryApi] Scanning directory: {}", path);
        
        // Resolve the path
        let resolved = match resolve_path(path) {
            crate::asset_io::ResolvedPath::Disk(pb) => {
                eprintln!("📂 [DirectoryApi] Resolved to disk path: {}", pb.display());
                pb
            },
            crate::asset_io::ResolvedPath::Brk(_) => {
                eprintln!("⚠️ [DirectoryApi] Cannot scan BRK archives recursively");
                return files;
            }
        };
        
        if !resolved.exists() {
            eprintln!("❌ [DirectoryApi] Path does not exist: {}", resolved.display());
            return files;
        }
        
        if !resolved.is_dir() {
            eprintln!("❌ [DirectoryApi] Path is not a directory: {}", resolved.display());
            return files;
        }
        
        eprintln!("✅ [DirectoryApi] Starting recursive walk of: {}", resolved.display());
        
        // Walk the directory recursively
        for entry in WalkDir::new(&resolved).into_iter().filter_map(|e| e.ok()) {
            let entry_path = entry.path();
            
            if entry_path.is_file() {
                // Get relative path from base directory
                if let Ok(relative) = entry_path.strip_prefix(&resolved) {
                    // Convert to forward slashes for consistency
                    let relative_str = relative.to_string_lossy().replace('\\', "/");
                    eprintln!("📄 [DirectoryApi] Found file: {}", relative_str);
                    files.push(relative_str);
                }
            } else if entry_path.is_dir() {
                if let Ok(relative) = entry_path.strip_prefix(&resolved) {
                    let relative_str = relative.to_string_lossy().replace('\\', "/");
                    eprintln!("📁 [DirectoryApi] Entering directory: {}", relative_str);
                }
            }
        }
        
        // Sort for consistent ordering
        files.sort();
        eprintln!("✅ [DirectoryApi] Scan complete. Found {} files", files.len());
        files
    }
    
    /// Scan a directory and return files with their full res:// paths
    /// The base_path should be something like "res://" and files will be returned as "res://path/to/file.ext"
    pub fn scan_with_prefix(&self, base_path: &str) -> Vec<String> {
        eprintln!("🔍 [DirectoryApi] Scanning with prefix: {}", base_path);
        let files = self.scan(base_path);
        let prefix = if base_path.ends_with('/') {
            base_path.to_string()
        } else {
            format!("{}/", base_path)
        };
        
        let prefixed_files: Vec<String> = files.iter()
            .map(|f| format!("{}{}", prefix, f))
            .collect();
        
        eprintln!("📋 [DirectoryApi] Returning {} files with prefix '{}'", prefixed_files.len(), prefix);
        for (i, file) in prefixed_files.iter().enumerate() {
            eprintln!("  [{}] {}", i + 1, file);
        }
        
        prefixed_files
    }
}

#[derive(Default)]
pub struct MathApi;

impl MathApi {
    /// Generate a random f32 between 0.0 and 1.0
    #[cfg_attr(not(debug_assertions), inline)]
    pub fn random(&self) -> f32 {
        let mut rng = rand::thread_rng();
        // Use fully qualified path to avoid reserved keyword issue
        <rand::rngs::ThreadRng as rand::RngCore>::next_u32(&mut rng) as f32 / u32::MAX as f32
    }
    
    /// Generate a random f32 between min (inclusive) and max (exclusive)
    #[cfg_attr(not(debug_assertions), inline)]
    pub fn random_range(&self, min: f32, max: f32) -> f32 {
        let mut rng = rand::thread_rng();
        let random_val = <rand::rngs::ThreadRng as rand::RngCore>::next_u32(&mut rng) as f32 / u32::MAX as f32;
        min + random_val * (max - min)
    }
    
    /// Generate a random i32 between min (inclusive) and max (exclusive)
    #[cfg_attr(not(debug_assertions), inline)]
    pub fn random_int(&self, min: i32, max: i32) -> i32 {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        Rng::gen_range(&mut rng, min..max)
    }
}

#[derive(Default)]
pub struct EditorApi {
    // Store a pointer to the ScriptApi that owns this instance
    api_ptr: Option<*mut ScriptApi<'static>>,
}

impl EditorApi {
    /// Set the ScriptApi pointer for this instance
    #[cfg_attr(not(debug_assertions), inline)]
    fn set_api_ptr(&mut self, api_ptr: *mut ScriptApi<'static>) {
        self.api_ptr = Some(api_ptr);
    }

    /// Get the ScriptApi pointer
    #[cfg_attr(not(debug_assertions), inline)]
    fn get_api_ptr(&self) -> Option<*mut ScriptApi<'static>> {
        self.api_ptr
    }

    /// Create a new Perro project
    /// Returns true on success, false on failure
    pub fn create_project(&mut self, project_name: &str, project_path: &str) -> bool {
        let api_ptr = if let Some(ptr) = self.get_api_ptr() {
            ptr
        } else {
            let tl_ptr = SCRIPT_API_CONTEXT.with(|ctx| *ctx.borrow());
            if let Some(ptr) = tl_ptr {
                let self_ptr = self as *mut EditorApi;
                unsafe {
                    (*self_ptr).set_api_ptr(ptr);
                }
                ptr
            } else {
                eprintln!("❌ EditorApi: No ScriptApi context available");
                return false;
            }
        };

        unsafe {
            let api = &mut *api_ptr;
            Self::create_project_impl(api, project_name, project_path)
        }
    }

    pub(crate) fn create_project_impl(
        api: &mut ScriptApi,
        project_name: &str,
        project_path: &str,
    ) -> bool {
        // Resolve the project path (supports user:// paths)
        let resolved_path = match resolve_path(project_path) {
            ResolvedPath::Disk(path) => path,
            ResolvedPath::Brk(_) => {
                api.print_error("Cannot create project in BRK path");
                return false;
            }
        };

        // Create the project using perro_core::project_creator
        match crate::project_creator::create_new_project(
            project_name,
            &resolved_path,
            false, // from_source = false, use crates.io dependency
            true,  // quiet = true, suppress verbose output when called from API
        ) {
            Ok(_) => {
                api.print(&format!("✅ Project '{}' created successfully at {}", project_name, resolved_path.display()));
                
                // Set the project_path runtime param so compile_scripts can use it
                let path_str = resolved_path.to_string_lossy().to_string();
                api.project().set_runtime_param("project_path", &path_str);
                
                true
            }
            Err(e) => {
                api.print_error(&format!("❌ Failed to create project: {}", e));
                false
            }
        }
    }

}

#[derive(Default)]
pub struct FileSystemApi;

impl FileSystemApi {
    /// Walk a directory recursively and return all file paths matching a filter
    /// Returns paths relative to the base directory
    /// Example: `let files = api.FileSystem.walk_files("res://", |p| p.ends_with(".scn"));`
    pub fn walk_files<F>(&self, base_path: &str, filter: F) -> Vec<String>
    where
        F: Fn(&Path) -> bool,
    {
        use crate::asset_io::resolve_path;
        use std::fs;

        let mut files = Vec::new();

        // Resolve the path - handle both res:// paths and direct disk paths
        let resolved = if base_path.starts_with("res://") || base_path.starts_with("user://") {
            match resolve_path(base_path) {
                crate::asset_io::ResolvedPath::Disk(pb) => pb,
                crate::asset_io::ResolvedPath::Brk(_) => {
                    return files;
                }
            }
        } else {
            // Already a disk path
            PathBuf::from(base_path)
        };

        if !resolved.exists() || !resolved.is_dir() {
            return files;
        }

        // Recursive walk using a stack
        let base_dir = resolved.clone();
        let mut stack = vec![resolved];
        while let Some(current_dir) = stack.pop() {
            if let Ok(entries) = fs::read_dir(&current_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        stack.push(path);
                    } else if path.is_file() && filter(&path) {
                        if let Ok(relative) = path.strip_prefix(&base_dir) {
                            let relative_str = relative.to_string_lossy().replace('\\', "/");
                            files.push(relative_str);
                        }
                    }
                }
            }
        }

        files.sort();
        files
    }

    /// Walk a directory recursively and return all files with a specific extension
    /// Returns paths relative to the base directory
    /// Example: `let scn_files = api.FileSystem.walk_files_with_ext("res://", "scn");`
    pub fn walk_files_with_ext(&self, base_path: &str, extension: &str) -> Vec<String> {
        self.walk_files(base_path, |path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case(extension))
                .unwrap_or(false)
        })
    }
}

#[allow(non_snake_case)]
#[derive(Default)]
pub struct EngineApi {
    pub JSON: JsonApi,
    pub Time: TimeApi,
    pub OS: OsApi,
    pub Process: ProcessApi,
    pub Input: InputApi,
    pub Texture: TextureApi,
    pub Math: MathApi,
    pub Editor: EditorApi,
    pub Directory: DirectoryApi,
    pub FileSystem: FileSystemApi,
}

//-----------------------------------------------------
// 4️⃣  Deref Implementation
//-----------------------------------------------------

impl<'a> Deref for ScriptApi<'a> {
    type Target = EngineApi;
    fn deref(&self) -> &Self::Target {
        // Set the parent pointer in InputApi and TextureApi when accessed immutably
        // This ensures the pointer is set even if DerefMut isn't called
        let api_ptr: *mut ScriptApi<'static> =
            unsafe { std::mem::transmute(self as *const ScriptApi<'a> as *mut ScriptApi<'static>) };

        unsafe {
            let input_api = &(*api_ptr).engine.Input;
            input_api.set_parent_ptr_immut(api_ptr);
            
            let texture_api = &(*api_ptr).engine.Texture;
            texture_api.set_api_ptr_immut(api_ptr);
        }
        &self.engine
    }
}

impl<'a> DerefMut for ScriptApi<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Set the parent pointer in InputApi and TextureApi so they can access ScriptApi
        // Cast to 'static lifetime (safe because we control the lifetime)
        let api_ptr: *mut ScriptApi<'static> = unsafe { std::mem::transmute(self) };

        unsafe {
            // Set the pointer in InputApi
            let input_api = &mut (*api_ptr).engine.Input;
            input_api.set_parent_ptr(api_ptr);
            
            // Set the pointer in TextureApi
            let texture_api = &mut (*api_ptr).engine.Texture;
            texture_api.set_api_ptr(api_ptr);
            
            // Set the pointer in EditorApi
            let editor_api = &mut (*api_ptr).engine.Editor;
            editor_api.set_api_ptr(api_ptr);
        }

        // Return the reference - safe because we're returning a reference to the same memory
        unsafe { &mut (*api_ptr).engine }
    }
}

//-----------------------------------------------------
// 3️⃣ Script API Context (main entry point for scripts)
//-----------------------------------------------------
pub struct ScriptApi<'a> {
    pub(crate) scene: &'a mut dyn SceneAccess,
    project: &'a mut Project,
    engine: EngineApi,
    pub(crate) gfx: Option<&'a mut Graphics>, // Always Some when created via new(), but Option for compatibility
}

impl<'a> ScriptApi<'a> {
    pub fn new(
        delta: f32,
        scene: &'a mut dyn SceneAccess,
        project: &'a mut Project,
        gfx: &'a mut Graphics,
    ) -> Self {
        let mut engine = EngineApi::default();
        engine.Time.delta = delta;
        Self {
            scene,
            project,
            engine,
            gfx: Some(gfx),
        }
    }

    /// Set the thread-local context for this ScriptApi
    /// This allows JoyConApi methods to access the ScriptApi
    #[cfg_attr(not(debug_assertions), inline)]
    pub(crate) fn set_context(&mut self) {
        let api_ptr: *mut ScriptApi<'static> = unsafe { std::mem::transmute(self) };

        SCRIPT_API_CONTEXT.with(|ctx| {
            *ctx.borrow_mut() = Some(api_ptr);
        });
    }

    #[cfg_attr(not(debug_assertions), inline)]
    pub(crate) fn clear_context() {
        SCRIPT_API_CONTEXT.with(|ctx| {
            *ctx.borrow_mut() = None;
        });
    }


    //-------------------------------------------------
    // Core access
    //-------------------------------------------------
    #[cfg_attr(not(debug_assertions), inline)]
    pub fn project(&mut self) -> &mut Project {
        self.project
    }

    //-------------------------------------------------
    // Input / Joy-Con API helpers
    //-------------------------------------------------
    /// Scan for Joy-Con 1 devices (HID)
    pub fn scan_joycon1(&mut self) -> Vec<Value> {
        JoyConApi::scan_joycon1_impl(self)
    }

    /// Get data from all connected controllers as structs
    pub fn get_joycon_data(&mut self) -> Vec<crate::input::joycon::JoyconState> {
        JoyConApi::get_data_impl(self)
    }

    /// Enable polling
    pub fn enable_joycon_polling(&mut self) -> bool {
        JoyConApi::enable_polling_impl(self)
    }

    /// Disable polling
    pub fn disable_joycon_polling(&mut self) {
        JoyConApi::disable_polling_impl(self)
    }

    /// Poll Joy-Con 1 synchronously (call from main loop)
    pub fn poll_joycon1_sync(&mut self) {
        JoyConApi::poll_joycon1_sync_impl(self)
    }

    /// Check if polling is enabled
    pub fn is_joycon_polling_enabled(&mut self) -> bool {
        JoyConApi::is_polling_enabled_impl(self)
    }

    //-------------------------------------------------
    // Compilation helpers
    //-------------------------------------------------
    pub fn compile_scripts(&mut self) -> Result<(), String> {
        self.run_compile(BuildProfile::Dev, CompileTarget::Scripts)
    }
    pub fn compile_project(&mut self) -> Result<(), String> {
        self.run_compile(BuildProfile::Release, CompileTarget::Project)
    }
    /// Compile scripts and run the project in dev mode using run_dev_with_path()
    /// This spawns a separate process that calls run_dev(), which handles its own window and scene
    /// If the spawned process crashes, it won't affect the editor
    /// Output from the spawned process is captured and forwarded to the editor's console
    pub fn compile_and_run(&mut self) -> Result<(), String> {
        // First compile the scripts
        self.compile_scripts()?;
        
        // Get the project path
        let project_path_str = self
            .project
            .get_runtime_param("project_path")
            .ok_or("Missing runtime param: project_path")?;
        
        // Ensure the path is absolute - resolve it if needed
        let absolute_path = if Path::new(project_path_str).is_absolute() {
            PathBuf::from(project_path_str)
        } else {
            // Try to canonicalize the path to make it absolute
            match std::fs::canonicalize(project_path_str) {
                Ok(abs_path) => abs_path,
                Err(_) => {
                    // If canonicalization fails, try to resolve relative to current dir
                    env::current_dir()
                        .map_err(|e| format!("Failed to get current directory: {}", e))?
                        .join(project_path_str)
                }
            }
        };
        
        // Validate that the path exists and contains project.toml
        if !absolute_path.exists() {
            return Err(format!("Project path does not exist: {}", absolute_path.display()));
        }
        
        let project_toml = absolute_path.join("project.toml");
        if !project_toml.exists() {
            return Err(format!(
                "project.toml not found at: {}\n\
                The path exists but does not contain a project.toml file.",
                absolute_path.display()
            ));
        }
        
        eprintln!("[compile_and_run] Project path from runtime param: {}", project_path_str);
        eprintln!("[compile_and_run] Absolute path to use: {}", absolute_path.display());
        eprintln!("[compile_and_run] Verified project.toml exists at: {}", project_toml.display());
        
        // Spawn a separate process - event loops can't be recreated in the same process
        // In dev mode, use cargo run -p perro_core
        // In release mode, use the current executable
        let absolute_path_str = absolute_path.to_string_lossy().to_string();
        
        let mut cmd = if cfg!(debug_assertions) {
            // Dev mode: use cargo run -p perro_core -- --path PATH --run
            eprintln!("[compile_and_run] Dev mode: Using cargo run -p perro_core -- --path {} --run", absolute_path_str);
            let mut cargo_cmd = process::Command::new("cargo");
            cargo_cmd.args(&["run", "-p", "perro_core", "--", "--path", &absolute_path_str, "--run"]);
            cargo_cmd.current_dir(env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?);
            cargo_cmd
        } else {
            // Release mode: use current executable
            let exe_path = env::current_exe()
                .map_err(|e| format!("Failed to get current executable path: {}", e))?;
            eprintln!("[compile_and_run] Release mode: Spawning separate process: {} --path {} --run", exe_path.display(), absolute_path_str);
            let mut exe_cmd = process::Command::new(exe_path);
            exe_cmd.args(&["--path", &absolute_path_str, "--run"]);
            exe_cmd.current_dir(&absolute_path);
            exe_cmd
        };
        
        // Platform-specific: Make the process run completely independently
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            // DETACHED_PROCESS = 0x00000008 (detached from parent)
            // CREATE_NEW_PROCESS_GROUP = 0x00000200 (new process group)
            // Combine flags: 0x00000208 (detached process, allows GUI windows)
            // We don't use CREATE_NO_WINDOW because the game needs to create its GUI window via winit
            // CREATE_NO_WINDOW only affects console windows, but we want to be safe
            cmd.creation_flags(0x00000208);
            // In release mode, redirect stdout to null but allow stderr to be captured
            // This way we can see errors if the process crashes
            // In dev mode, we want to see all output
            if cfg!(debug_assertions) {
                cmd.stdout(process::Stdio::inherit());
                cmd.stderr(process::Stdio::inherit());
            } else {
                // Release mode: hide stdout but capture stderr for debugging
                cmd.stdout(process::Stdio::null());
                // For now, also hide stderr to avoid console windows
                // TODO: Could pipe stderr to a log file or editor console
                cmd.stderr(process::Stdio::null());
            }
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            // On Unix-like systems, redirect to /dev/null to hide output
            // In dev mode, show output for debugging
            if cfg!(debug_assertions) {
                cmd.stdout(process::Stdio::inherit());
                cmd.stderr(process::Stdio::inherit());
            } else {
                cmd.stdout(process::Stdio::null());
                cmd.stderr(process::Stdio::null());
            }
        }
        
        // Spawn the process (it will run completely independently)
        let child = cmd.spawn()
            .map_err(|e| format!("Failed to spawn dev runtime process: {}", e))?;
        
        // Get the process ID and store it as a runtime parameter
        let process_id = child.id();
        self.project.set_runtime_param("runtime_process_id", &process_id.to_string());
        self.project.set_runtime_param("runtime_process_running", "true");
        
        // Don't keep the child handle - fully detach the process
        // The OS will handle cleanup when the process exits
        // We don't need to wait for it or track it - the manager script will check process status periodically
        // This ensures the process is completely independent and the OS scheduler can balance them
        drop(child);
        
        // The manager script will check process status periodically using the process ID
        
        #[cfg(target_os = "windows")]
        eprintln!("[compile_and_run] Spawned runtime process (PID: {}) as detached process", process_id);
        
        #[cfg(not(target_os = "windows"))]
        eprintln!("[compile_and_run] Spawned runtime process (PID: {}) as independent process", process_id);
        
        Ok(())
    }
    fn run_compile(&mut self, profile: BuildProfile, target: CompileTarget) -> Result<(), String> {
        let project_path_str = self
            .project
            .get_runtime_param("project_path")
            .ok_or("Missing runtime param: project_path")?;
        let project_path = Path::new(project_path_str);
        transpile(project_path, false, false, false).map_err(|e| format!("Transpile failed: {}", e))?;
        let compiler = Compiler::new(project_path, target, false);
        compiler
            .compile(profile)
            .map_err(|e| format!("Compile failed: {}", e))
    }

    //-------------------------------------------------
    // Window / App Commands
    //-------------------------------------------------
    pub fn set_window_title(&mut self, title: String) {
        self.project.set_name(title.clone());
        if let Some(tx) = self.scene.get_command_sender() {
            let _ = tx.send(AppCommand::SetWindowTitle(title));
        }
    }
    pub fn set_fps_cap(&mut self, fps: f32) {
        self.project.set_fps_cap(fps);
        if let Some(tx) = self.scene.get_command_sender() {
            let _ = tx.send(AppCommand::SetFpsCap(fps));
        }
    }

    //-------------------------------------------------
    // Lifecycle / Updates
    //-------------------------------------------------
    /// Call the init() method on a script
    /// 
    /// SAFETY: This is safe because:
    /// - Called synchronously by the engine during scene initialization
    /// - Only one script's init() is called at a time
    /// - Scripts are never accessed concurrently
    /// - All script code goes through the API (transpiler guarantee)
    pub fn call_init(&mut self, script_id: NodeID) {
        self.set_context();
        // Set current script ID in thread-local context
        CURRENT_SCRIPT_ID.with(|ctx| *ctx.borrow_mut() = Some(script_id));
        // Scripts are now always in memory as Rc<UnsafeCell<>>, so we can access them directly
        if let Some(script_rc) = self.scene.get_script(script_id) {
            // Check if script has init implemented before calling
            unsafe {
                let script_ptr = script_rc.get();
                let has_init = (*script_ptr).script_flags().has_init();
                if has_init {
                    let script_mut = &mut *script_ptr;
                    let script_mut = Box::as_mut(script_mut);
                    script_mut.engine_init(self);
                }
            }
        }
        CURRENT_SCRIPT_ID.with(|ctx| *ctx.borrow_mut() = None);
        Self::clear_context();
    }

    /// Call the update() method on a script
    /// 
    /// SAFETY: This is safe because:
    /// - Called synchronously by the engine during the update loop
    /// - Scripts are called one at a time in a controlled sequence
    /// - Nested calls (through call_function_id) are safe (same execution context)
    /// - All script code goes through the API (transpiler guarantee)
    pub fn call_update(&mut self, id: NodeID) {
        self.set_context();
        // Set current script ID in thread-local context
        CURRENT_SCRIPT_ID.with(|ctx| *ctx.borrow_mut() = Some(id));
        // Scripts are now always in memory as Rc<UnsafeCell<>>, so we can access them directly
        if let Some(script_rc) = self.scene.get_script(id) {
            // Check if script has update implemented before calling
            unsafe {
                let script_ptr = script_rc.get();
                let has_update = (*script_ptr).script_flags().has_update();
                if has_update {
                    let script_mut = &mut *script_ptr;
                    let script_mut = Box::as_mut(script_mut);
                    script_mut.engine_update(self);
                }
            }
        }
        CURRENT_SCRIPT_ID.with(|ctx| *ctx.borrow_mut() = None);
        Self::clear_context();
    }

    pub fn call_fixed_update(&mut self, id: NodeID) {
        self.set_context();
        // Set current script ID in thread-local context
        CURRENT_SCRIPT_ID.with(|ctx| *ctx.borrow_mut() = Some(id));
        // Scripts are now always in memory as Rc<UnsafeCell<>>, so we can access them directly
        if let Some(script_rc) = self.scene.get_script(id) {
            // Check if script has fixed_update implemented before calling
            unsafe {
                let script_ptr = script_rc.get();
                let has_fixed_update = (*script_ptr).script_flags().has_fixed_update();
                if has_fixed_update {
                    let script_mut = &mut *script_ptr;
                    let script_mut = Box::as_mut(script_mut);
                    script_mut.engine_fixed_update(self);
                }
            }
        }
        CURRENT_SCRIPT_ID.with(|ctx| *ctx.borrow_mut() = None);
        Self::clear_context();
    }


    pub fn call_node_internal_fixed_update(&mut self, node_id: NodeID) {
        // We need to get the node and call the method, but we can't hold a RefMut
        // while also borrowing self mutably. So we check if update is needed first,
        // then drop that borrow before calling the method.
        let needs_update = {
            if self.scene.get_scene_node_ref(node_id).is_none() {
                return;
            }
            self.scene.get_scene_node_ref(node_id)
                .map(|node_ref| {
                    node_ref.needs_internal_fixed_update()
                })
                .unwrap_or(false)
        };
        
        if needs_update {
            // We need to split the borrows: get a RefMut from the scene while also
            // having &mut self. Since RefMut borrows from the RefCell inside the scene's
            // data (not from the &mut dyn SceneAccess itself), we can use unsafe code
            // to split the borrows safely. The RefMut will be dropped at the end of
            // the block, ensuring the borrow is released.
            unsafe {
                // Get a raw pointer to the scene to split the borrows
                let scene_ptr: *mut dyn SceneAccess = &mut *self.scene;
                if let Some(node) = (*scene_ptr).get_scene_node_mut(node_id) {
                    // The RefMut borrows from the RefCell, not from the SceneAccess trait object,
                    // so it's safe to use &mut self here as long as we don't access self.scene
                    // through the mutable reference while the RefMut is alive.
                    node.internal_fixed_update(self);
                }
            }
        }
    }

    pub fn call_node_internal_render_update(&mut self, node_id: NodeID) {
        // We need to get the node and call the method, but we can't hold a RefMut
        // while also borrowing self mutably. So we check if update is needed first,
        // then drop that borrow before calling the method.
        let needs_update = {
            self.scene.get_scene_node_ref(node_id)
                .map(|node_ref| {
                    node_ref.needs_internal_render_update()
                })
                .unwrap_or(false)
        };
        
        if needs_update {
            // We need to split the borrows: get a RefMut from the scene while also
            // having &mut self. Since RefMut borrows from the RefCell inside the scene's
            // data (not from the &mut dyn SceneAccess itself), we can use unsafe code
            // to split the borrows safely. The RefMut will be dropped at the end of
            // the block, ensuring the borrow is released.
            unsafe {
                // Get a raw pointer to the scene to split the borrows
                let scene_ptr: *mut dyn SceneAccess = &mut *self.scene;
                if let Some(node) = (*scene_ptr).get_scene_node_mut(node_id) {
                    // The RefMut borrows from the RefCell, not from the SceneAccess trait object,
                    // so it's safe to use &mut self here as long as we don't access self.scene
                    // through the mutable reference while the RefMut is alive.
                    node.internal_render_update(self);
                }
            }
        }
    }

    pub fn call_function(&mut self, id: NodeID, func: &str, params: &[Value]) {
        let func_id = self.string_to_u64(func);
        self.call_function_id(id, func_id, params);
    }

    /// Call a function on a script by ID
    /// 
    /// Scripts are stored as Rc<UnsafeCell<Box<dyn ScriptObject>>> in the HashMap, so they're
    /// always accessible. We use UnsafeCell::get() to get a mutable reference when calling functions.
    /// 
    /// This works for:
    /// - Calling functions on the same script (nested calls)
    /// - Calling functions on different scripts (e.g., child nodes, parent nodes, any script)
    /// - All calls are synchronous and safe (same execution context)
    /// 
    /// SAFETY: Calling functions on different scripts is just as safe as calling on the same script
    /// because all access goes through the API, execution is synchronous, and the API controls
    /// all state mutations. There's no memory safety concern - each script is independently
    /// stored in the HashMap and accessed through the API.
    pub fn call_function_id(&mut self, id: NodeID, func: u64, params: &[Value]) {
        // Set ScriptApi context (for JoyCon API thread-local access)
        // This is idempotent - safe to call multiple times
        self.set_context();
        
        // Check if this is a nested call to the same script
        let current_id_opt = CURRENT_SCRIPT_ID.with(|ctx| *ctx.borrow());
        
        // Store previous script ID before setting new one
        let previous_script_id = current_id_opt;
        
        // Set current script ID for nested call detection
        CURRENT_SCRIPT_ID.with(|ctx| *ctx.borrow_mut() = Some(id));
        
        // Scripts are now always in memory as Rc<UnsafeCell<Box<dyn ScriptObject>>>, so we can access them directly
        if let Some(script_rc) = self.scene.get_script(id) {
            
            // With UnsafeCell, we can always get a mutable reference, even for nested calls
            // 
            // SAFETY: This is safe because of our design invariants:
            // 
            // 1. **Controlled Access**: All script access is controlled by the API. Scripts are
            //    never accessed directly by user code - the transpiler ensures all script code
            //    goes through the API methods (call_function_id, get_script_var_id, set_script_var_id).
            //
            // 2. **Synchronous Execution**: All script execution is synchronous and controlled:
            //    - init(), update(), fixed_update() are called by the engine in a controlled sequence
            //    - Function calls through call_function_id are synchronous (as if inlined)
            //    - Variable access (get/set) is synchronous and controlled
            //
            // 3. **No Concurrent Access**: Scripts are single-threaded. The API ensures that:
            //    - Only one script function executes at a time (synchronous call stack)
            //    - Nested calls are safe because they're part of the same call chain
            //    - Variable reads are safe (clone/copy semantics)
            //    - Variable writes are safe (same as calling a mutable function)
            //
            // 4. **API-Controlled State**: The API controls all state mutations:
            //    - Scripts can only mutate their own state through the API
            //    - Scripts can call other scripts through the API (same script OR different scripts)
            //    - Calling different scripts is just as safe as calling the same script
            //    - Each script is independently stored and accessed (no shared mutable state)
            //    - The API ensures proper ordering and synchronization
            //
            // 5. **Cross-Script Calls**: Calling functions on different scripts (e.g., child nodes,
            //    parent nodes, any script) is safe because:
            //    - Each script is stored independently in the HashMap
            //    - Each script has its own UnsafeCell (no shared mutable state)
            //    - All access goes through the API (controlled, synchronous)
            //    - No memory leaks or safety issues - each script is properly managed
            //
            // 6. **Transpiler Guarantees**: The transpiler ensures:
            //    - All script code goes through the API
            //    - No direct access to script internals
            //    - All function calls are through call_function_id
            //    - All variable access is through get_script_var_id/set_script_var_id
            //
            // Therefore, creating mutable references through UnsafeCell is safe because:
            // - We're accessing script state in a controlled, synchronous manner
            // - There's no possibility of data races (single-threaded)
            // - The API ensures proper access patterns
            // - Calls are equivalent to inlining (same execution context)
            // - Each script is independently managed (no shared mutable state between scripts)
            // - Cross-script calls are safe (each script has its own UnsafeCell)
            unsafe {
                let script_ptr = script_rc.get();
                let script_mut = &mut *script_ptr;
                let script_mut = Box::as_mut(script_mut);
                script_mut.call_function(func, self, params);
            }
        }
        
        // Restore previous script ID (if any)
        CURRENT_SCRIPT_ID.with(|ctx| *ctx.borrow_mut() = previous_script_id);
        Self::clear_context();
    }
    
    /// Alias for call_function_id (for backwards compatibility)
    /// The "fast" version was meant to skip context setup, but we always do it now
    #[cfg_attr(not(debug_assertions), inline)]
    pub(crate) fn call_function_id_fast(&mut self, id: NodeID, func: u64, params: &[Value]) {
        self.call_function_id(id, func, params);
    }

    #[cfg_attr(not(debug_assertions), inline)]
    pub fn string_to_u64(&mut self, string: &str) -> u64 {
        string_to_u64(string)
    }

    // ========== INSTANT SIGNALS (zero-allocation, immediate) ==========
    
    /// Emit signal instantly - handlers called immediately
    /// Params passed as compile-time slice, zero allocation
    #[cfg_attr(not(debug_assertions), inline)]
    pub fn emit_signal(&mut self, name: &str, params: &[Value]) {
        let id = self.string_to_u64(name);
        self.emit_signal_id(id, params);
    }

    /// Emit signal instantly by ID - handlers called immediately
    /// Params passed as compile-time slice, zero allocation
    /// OPTIMIZED: Handles emission in existing API context to avoid double-borrow
    /// OPTIMIZED: Set context once and batch all calls to minimize overhead
    pub fn emit_signal_id(&mut self, id: u64, params: &[Value]) {
        // Copy out listeners before calling functions
        let script_map_opt = self.scene.get_signal_connections(id);
        if script_map_opt.is_none() {
            return;
        }

        // OPTIMIZED: Use SmallVec with inline capacity of 4 listeners
        // Most signals have 1-3 listeners, so this avoids heap allocation in common case
        let script_map = script_map_opt.unwrap();
        let mut call_list = SmallVec::<[(NodeID, u64); 4]>::new();
        for (uuid, fns) in script_map.iter() {
            for &fn_id in fns.iter() {
                call_list.push((*uuid, fn_id));
            }
        }

        // OPTIMIZED: Set context once for all calls instead of per-call
        self.set_context();
        
        // OPTIMIZED: Use fast-path calls that skip redundant context operations
        // IMPORTANT: Check if node still exists before EACH handler call
        // (previous handler might have deleted the node)
        for (target_id, fn_id) in call_list.iter() {
            // First, check if the target node still exists (it might have been deleted)
            if self.scene.get_scene_node_ref(*target_id).is_none() {
                continue; // Target node was deleted, skip this handler
            }
            
            // If params contain a node ID (first param is usually the collision node), check if it still exists
            // NOTE: We only skip if the UUID exists as a scene node AND was deleted. If it doesn't exist
            // as a scene node at all, it might be a different type of UUID (like file tree item_id), so proceed.
            let should_call = if let Some(first_param) = params.get(0) {
                if let Some(node_id_str) = first_param.as_str() {
                    if let Ok(node_id) = NodeID::parse_str(node_id_str) {
                        // Check if this UUID exists as a scene node
                        if let Some(_) = self.scene.get_scene_node_ref(node_id) {
                            // It's a scene node and it exists, proceed
                            true
                        } else {
                            // UUID doesn't exist as a scene node - might be a different type of UUID (item_id, etc.)
                            // Proceed with handler call
                            true
                        }
                    } else {
                        true // Not a UUID param, proceed
                    }
                } else {
                    true // Not a string param, proceed
                }
            } else {
                true // No params, proceed
            };
            
            if should_call {
                self.call_function_id_fast(*target_id, *fn_id, params);
            }
        }
        
        // Clear context once after all calls
        Self::clear_context();
    }

    // ========== DEFERRED SIGNALS (queued, processed at frame end) ==========
    
    /// Emit signal deferred - queued and processed at end of frame
    /// Use when emitting during iteration or need frame-end processing
    #[cfg_attr(not(debug_assertions), inline)]
    pub fn emit_signal_deferred(&mut self, name: &str, params: &[Value]) {
        let id = self.string_to_u64(name);
        self.scene.emit_signal_id_deferred(id, params);
    }

    /// Emit signal deferred by ID - queued and processed at end of frame
    /// Use when emitting during iteration or need frame-end processing
    #[cfg_attr(not(debug_assertions), inline)]
    pub fn emit_signal_id_deferred(&mut self, id: u64, params: &[Value]) {
        self.scene.emit_signal_id_deferred(id, params);
    }

    #[cfg_attr(not(debug_assertions), inline)]
    pub fn connect_signal(&mut self, name: &str, target: NodeID, function: &'static str) {
        let id = string_to_u64(name);
        let fn_id = string_to_u64(function);
        self.scene.connect_signal_id(id, target, fn_id);
    }

    #[cfg_attr(not(debug_assertions), inline)]
    pub fn connect_signal_id(&mut self, id: u64, target: NodeID, function_id: u64) {
        self.scene.connect_signal_id(id, target, function_id);
    }

    pub fn instantiate_script(&mut self, path: &str) -> Option<ScriptType> {
        // Convert to registry key
        let identifier = match script_path_to_identifier(path) {
            Ok(id) => id,
            Err(_) => {
                return None;
            }
        };

        // Get ctor safely from provider (no scene.borrow_mut recursion)
        let ctor = match self.scene.load_ctor(&identifier) {
            Ok(c) => c,
            Err(_) => {
                return None;
            }
        };

        // Construct script without registering it
        let raw = ctor();
        let mut boxed: ScriptType = unsafe { Box::from_raw(raw) };
        boxed.set_id(NodeID::nil()); // explicitly detached

        // run init() safely using a temporary sub‑APIs
        // note: doesn't touch scene.scripts, only passes mut ref
        {
            let project_ref = self.project as *mut _;
            let project_mut = unsafe { &mut *project_ref };
            let gfx_ref = self.gfx.as_mut().expect("Graphics required for instantiate_script");
            let mut sub_api = ScriptApi::new(0.0, self.scene, project_mut, gfx_ref);
            boxed.init(&mut sub_api);
        }

        Some(boxed)
    }

    //-------------------------------------------------
    // Asset IO
    //-------------------------------------------------
    pub fn load_asset(&mut self, path: &str) -> Option<Vec<u8>> {
        asset_io::load_asset(path).ok()
    }
    pub fn save_asset<D>(&mut self, path: &str, data: D) -> io::Result<()>
    where
        D: AsRef<[u8]>,
    {
        asset_io::save_asset(path, data.as_ref())
    }
    pub fn resolve_path(&self, path: &str) -> Option<String> {
        match resolve_path(path) {
            ResolvedPath::Disk(pathbuf) => pathbuf.to_str().map(String::from),
            ResolvedPath::Brk(vpath) => vpath.into(),
        }
    }

    //-------------------------------------------------
    // Scene / Node Access
    //-------------------------------------------------
    
    /// Read a value from a node using a closure
    /// The closure receives &T where T is the node type and returns any Clone value (including Copy types)
    /// For Copy types like primitives, no actual cloning happens at runtime
    /// For non-Copy types like String/Cow, the value is cloned out of the node
    /// Returns a default value if the node doesn't exist (prevents panics when nodes are removed)
    /// Example: `let parent = api.read_node::<CollisionShape2D, _>(c_id, |c| c.parent);`
    /// Example: `let name = api.read_node::<Sprite2D, _>(self.id, |s| s.name.clone());`
    /// 
    /// Uses optimized compile-time match dispatch instead of Any downcast for better performance
    #[cfg_attr(not(debug_assertions), inline(always))]
    pub fn read_node<T: crate::nodes::node_registry::NodeTypeDispatch, R: Clone + Default>(&self, node_id: NodeID, f: impl FnOnce(&T) -> R) -> R {
        // Check if node_id is nil (from get_parent() when node was removed)
        if node_id.is_nil() {
            return R::default();
        }
        
        // Check if node exists (might have been removed during signal handling)
        if let Some(node) = self.scene.get_scene_node_ref(node_id) {
            if let Some(result) = node.with_typed_ref(f) {
                return result;
            }
        }
        
        // Node doesn't exist or wrong type - return default value
        // This prevents panics when nodes are removed during signal handling
        R::default()
    }
    
    /// Read transform-related properties from any Node2D-based node
    /// This works with Node2D, Sprite2D, Area2D, CollisionShape2D, etc.
    /// Safer than read_node::<Node2D> when you don't know the exact type
    /// Returns a default value if the node doesn't exist (prevents panics when nodes are removed)
    /// Example: `let pos = api.read_node2d_transform(parent_id, |n2d| n2d.transform.position);`
    #[cfg_attr(not(debug_assertions), inline(always))]
    pub fn read_node2d_transform<R: Clone + Default>(&self, node_id: NodeID, f: impl FnOnce(&crate::nodes::_2d::node_2d::Node2D) -> R) -> R {
        // Check if node_id is nil (from get_parent() when node was removed)
        if node_id.is_nil() {
            return R::default();
        }
        
        // Check if node exists (might have been removed during signal handling)
        if let Some(node) = self.scene.get_scene_node_ref(node_id) {
            if let Some(node2d) = node.as_node2d() {
                return f(node2d);
            }
        }
        
        // Node doesn't exist or is not Node2D-based - return default value
        // This prevents panics when nodes are removed during signal handling
        R::default()
    }
    
    /// Mutate a node directly with a closure - no clones needed!
    /// The closure receives &mut T where T is the node type
    /// Returns true if the node was found and successfully mutated, false otherwise
    /// Example: `api.mutate_node::<Node2D>(node_id, |n| n.transform.position.x = 5.0);`
    /// 
    /// Uses optimized compile-time match dispatch instead of Any downcast for better performance
    #[cfg_attr(not(debug_assertions), inline(always))]
    pub fn mutate_node<T: crate::nodes::node_registry::NodeTypeDispatch, F>(&mut self, id: NodeID, f: F)
    where
        F: FnOnce(&mut T),
    {
        // Get mutable access to the node
        let is_node2d = {
            let node = self.scene.get_scene_node_mut(id)
                .unwrap_or_else(|| panic!("Node {} not found", id));
            
            // Use optimized match dispatch instead of Any downcast
            // The closure is called directly within with_typed_mut, which handles the type extraction
            let result = node.with_typed_mut(f);
            if result.is_none() {
                // Provide better error message with actual node type
                let actual_type = node.get_type();
                let node_name = node.get_name();
                let expected_type_name = std::any::type_name::<T>();
                eprintln!("[ERROR] mutate_node: Node {} ({}, {:?}) is not of type {} (expected: {})", 
                    id, node_name, actual_type, expected_type_name, expected_type_name);
                panic!("Node {} is not of type {} (actual type: {:?})", id, expected_type_name, actual_type);
            }
            
            // Check if Node2D before releasing borrow (avoid redundant lookup)
            let is_node2d = node.as_node2d().is_some();
            
            // Mark transform dirty if Node2D
            node.mark_transform_dirty_if_node2d();
            
            is_node2d
        }; // node borrow released here
        
        // Always call mark_needs_rerender - it will check HashSet.contains() (O(1)) 
        // and only add if not already in the set
        self.scene.mark_needs_rerender(id);
        
        // If this is a Node2D, mark transform dirty recursively (including children)
        // This ensures children's global transforms are recalculated when parent moves
        // Note: mark_transform_dirty_recursive will also add to dirty_nodes if needed
        if is_node2d {
            self.scene.mark_transform_dirty_recursive(id);
        }
    }
    
    /// Mutate a SceneNode directly using BaseNode trait methods
    /// This works with any node type without needing to know the specific type
    /// Only allows access to base Node fields (name, id, parent, children, etc.)
    /// Example: `api.mutate_scene_node(node_id, |n| n.set_name("test".into()));`
    #[cfg_attr(not(debug_assertions), inline(always))]
    pub fn mutate_scene_node<F>(&mut self, id: NodeID, f: F)
    where
        F: FnOnce(&mut crate::nodes::node_registry::SceneNode),
    {
        // Get mutable access to the node
        let is_node2d = {
            let node = self.scene.get_scene_node_mut(id)
                .unwrap_or_else(|| panic!("Node {} not found", id));
            
            f(node); // mutate in place using BaseNode methods
            
            // Check if Node2D before releasing borrow (avoid redundant lookup)
            let is_node2d = node.as_node2d().is_some();
            
            // Mark transform dirty if Node2D
            node.mark_transform_dirty_if_node2d();
            
            is_node2d
        }; // node borrow released here
        
        // Always call mark_needs_rerender - it will check HashSet.contains() (O(1))
        // and only add if not already in the set
        self.scene.mark_needs_rerender(id);
        
        // If this is a Node2D, mark transform dirty recursively (including children)
        // Note: mark_transform_dirty_recursive will also add to dirty_nodes if needed
        if is_node2d {
            self.scene.mark_transform_dirty_recursive(id);
        }
    }
    
    /// Read a value from a SceneNode using BaseNode trait methods
    /// This works with any node type without needing to know the specific type
    /// Only allows access to base Node fields (name, id, parent, children, etc.)
    /// Example: `let name = api.read_scene_node(node_id, |n| n.get_name().to_string());`
    #[cfg_attr(not(debug_assertions), inline)]
    pub fn read_scene_node<R: Clone>(&self, node_id: NodeID, f: impl FnOnce(&crate::nodes::node_registry::SceneNode) -> R) -> R {
        let node = self.scene.get_scene_node_ref(node_id)
            .unwrap_or_else(|| panic!("Node {} not found", node_id));
        
        f(node)
    }
    
    /// Create a new node of the specified type and add it to the scene.
    /// The arena assigns the next open slot+generation as the node ID (no monotonic counter).
    /// Returns the ID of the newly created node.
    /// Example: `let node_id = api.create_node::<Node2D>();`
    pub fn create_node<T>(&mut self) -> NodeID
    where
        T: Default,
        T: crate::nodes::node_registry::ToSceneNode,
    {
        let node: T = Default::default();
        let scene_node = node.to_scene_node();
        let gfx_ref = self.gfx.as_mut().expect("Graphics required for add_node_to_scene");
        self.scene.add_node_to_scene(scene_node, gfx_ref)
            .unwrap_or_else(|e| panic!("Failed to add node to scene: {}", e))
    }
    
    /// Get a UINode by node ID and execute a closure with mutable access
    /// This allows you to modify UI elements dynamically
    /// Example: `api.with_ui_node(ui_node_id, |ui| { ui.add_element(...); });`
    /// 
    /// Uses optimized compile-time match dispatch instead of Any downcast for better performance
    #[cfg_attr(not(debug_assertions), inline(always))]
    pub fn with_ui_node<F, R>(&mut self, node_id: NodeID, f: F) -> R
    where
        F: FnOnce(&mut crate::nodes::ui::ui_node::UINode) -> R,
    {
        let node = self.scene.get_scene_node_mut(node_id)
            .unwrap_or_else(|| panic!("Node {} not found", node_id));
        
        let result = node.with_typed_mut(f)
            .unwrap_or_else(|| panic!("Node {} is not a UINode", node_id));
        
        // Mark the UI node as needing rerender after modification
        self.scene.mark_needs_rerender(node_id);
        
        result
    }
    
    /// Add a UI element to a UINode
    /// The element will be added to the elements map and marked for rerender
    /// If parent_element_id is provided and not nil, the element will be set as a child of that parent element
    /// Returns the element's UUID if successful, None otherwise
    /// Example: `api.add_ui_element(ui_node_id, "my_button", UIElement::Button(...), Some(parent_element_id));`
    pub fn add_ui_element(
        &mut self,
        ui_node_id: NodeID,
        element_name: &str,
        mut element: crate::ui_element::UIElement,
        parent_element_id: Option<crate::ids::UIElementID>,
    ) -> Option<crate::ids::UIElementID> {
        self.with_ui_node(ui_node_id, |ui| {
            use indexmap::IndexMap;
            
            // Get or create elements map
            let elements = ui.elements.get_or_insert_with(|| IndexMap::new());
            
            // Get or create root_ids
            let root_ids = ui.root_ids.get_or_insert_with(|| Vec::new());
            
            let element_id = element.get_id();
            
            // Set the element's name
            element.set_name(element_name);
            
            // Set parent if provided
            if let Some(parent_id) = parent_element_id {
                if !parent_id.is_nil() {
                    // Set the element's parent
                    element.set_parent(Some(parent_id));
                    
                    // Add to parent's children list
                    if let Some(parent_element) = elements.get_mut(&parent_id) {
                        let mut children = parent_element.get_children().to_vec();
                        children.push(element_id);
                        parent_element.set_children(children);
                    }
                    
                    elements.insert(element_id, element);
                } else {
                    // No parent, add as root
                    root_ids.push(element_id);
                    elements.insert(element_id, element);
                }
            } else {
                // No parent specified, add as root
                root_ids.push(element_id);
                elements.insert(element_id, element);
            }
            
            // Mark element as needing rerender and layout
            ui.mark_element_needs_layout(element_id);
            eprintln!("🔧 [add_ui_element] Marked element {} ({}) for layout/rerender", element_name, element_id);
            eprintln!("🔧 [add_ui_element] needs_rerender now has {} elements", ui.needs_rerender.len());
            
            Some(element_id)
        })
    }
    
    /// Append FUR elements to a UINode dynamically
    /// Parses the FUR string and adds the elements as children of the specified parent
    /// Returns true if successful, false otherwise
    /// Example: `api.append_fur_to_ui(ui_node_id, "[Button]Click Me[/Button]", Some(parent_id));`
    pub fn append_fur_to_ui(
        &mut self,
        ui_node_id: NodeID,
        fur_string: &str,
        parent_element_id: Option<crate::ids::UIElementID>,
    ) -> bool {
        use crate::nodes::ui::parser::FurParser;
        use crate::nodes::ui::apply_fur::append_fur_elements_to_ui;
        
        // Parse the FUR string
        let mut parser = match FurParser::new(fur_string) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("❌ Failed to create FUR parser: {}", e);
                return false;
            }
        };
        
        let fur_ast = match parser.parse() {
            Ok(ast) => ast,
            Err(e) => {
                eprintln!("❌ Failed to parse FUR string: {}", e);
                return false;
            }
        };
        
        // Extract elements from AST
        let fur_elements: Vec<crate::fur_ast::FurElement> = fur_ast
            .into_iter()
            .filter_map(|node| match node {
                crate::fur_ast::FurNode::Element(el) => Some(el),
                _ => None,
            })
            .collect();
        
        if fur_elements.is_empty() {
            eprintln!("⚠️ No FUR elements found in string");
            return false;
        }
        
        // Append to UI node using mutate_node for proper rerender marking
        self.mutate_node::<crate::nodes::ui::ui_node::UINode, _>(ui_node_id, |ui| {
            append_fur_elements_to_ui(ui, &fur_elements, parent_element_id);
        });
        
        true
    }
    
    /// Reparent a child node to a new parent
    /// Handles removing from old parent if it exists
    /// Example: `api.reparent(parent_id, child_id);`
    pub fn reparent(&mut self, new_parent_id: NodeID, child_id: NodeID) {

        if self.scene.get_scene_node_ref(child_id).is_none() {
            eprintln!("⚠️ reparent: Child node {} does not exist, cannot reparent to {}", child_id, new_parent_id);
            return;
        }
        
        // Check if new parent exists
        if self.scene.get_scene_node_ref(new_parent_id).is_none() {
            eprintln!("⚠️ reparent: Parent node {} does not exist, cannot reparent child {}", new_parent_id, child_id);
            return;
        }
        
        let old_parent_id_opt = {
            let child_node = self.scene.get_scene_node_ref(child_id)
                .expect("Child node should exist (checked above)");
            child_node.get_parent().map(|p| p.id)
        };
        
        // Remove from old parent if it has one (this also sets child's parent to None)
        if let Some(old_parent_id) = old_parent_id_opt {
            self.remove_child(old_parent_id, child_id);
            // Update Node2D children cache for the old parent
            self.scene.update_node2d_children_cache_on_remove(old_parent_id, child_id);
        }
        
        // Create ParentType for new parent (need to do this before mutable borrow)
        let parent_type_opt = self.create_parent_type(new_parent_id);
        
        // Set the child's parent to the new parent (with type info)
        if let Some(child_node) = self.scene.get_scene_node_mut(child_id) {
            if let Some(parent_type) = parent_type_opt {
                child_node.set_parent(Some(parent_type));
            }
        }
        
        // Add child to the new parent's children list
        if let Some(parent_node) = self.scene.get_scene_node_mut(new_parent_id) {
            parent_node.add_child(child_id);
        }
        
        // Update Node2D children cache for the parent (if it's Node2D-based)
        self.scene.update_node2d_children_cache_on_add(new_parent_id, child_id);
    }

    /// Get a child node by name, searching through the parent's children
    /// Returns the child node's ID if found, None otherwise
    pub fn get_child_by_name(&mut self, parent_id: NodeID, child_name: &str) -> Option<NodeID> {
        let children: Vec<NodeID> = {
            if let Some(parent_node) = self.scene.get_scene_node_ref(parent_id) {
                parent_node.get_children().iter().copied().collect()
            } else {
                return None;
            }
        }; // parent_node borrow ends here

        // Now iterate over the collected children (no borrows held)
        for child_id in children {
            if let Some(child_node) = self.scene.get_scene_node_ref(child_id) {
                if child_node.get_name() == child_name {
                    return Some(child_id);
                }
            }
        }
        None
    }

    /// Get the parent node ID of a given node
    /// Returns the parent's UUID if the node has a parent
    /// Panics if the node is not found or has no parent
    /// NOTE: Callers should check if the node exists before calling this
    #[cfg_attr(not(debug_assertions), inline)]
    pub fn get_parent(&mut self, node_id: NodeID) -> NodeID {
        let node = self.scene.get_scene_node_ref(node_id)
            .unwrap_or_else(|| panic!("Node {} not found", node_id));
        
        let parent_id = node.get_parent()
            .map(|p| p.id)
            .unwrap_or_else(|| panic!("Node {} has no parent", node_id));
        
        parent_id
    }
    
    /// Returns the parent's NodeType if the node has a parent
    /// Panics if the node is not found or has no parent
    /// NOTE: Callers should check if the node exists before calling this
    #[cfg_attr(not(debug_assertions), inline)]
    pub fn get_parent_type(&mut self, node_id: NodeID) -> crate::node_registry::NodeType {
        let node = self.scene.get_scene_node_ref(node_id)
            .unwrap_or_else(|| panic!("Node {} not found", node_id));
        node.get_parent()
            .map(|p| p.node_type)
            .unwrap_or_else(|| panic!("Node {} has no parent", node_id))
    }
    
    /// Returns the NodeType of the given node
    /// Panics if the node is not found
    /// NOTE: Callers should check if the node exists before calling this
    /// Useful for runtime type checking before casting
    /// Example: `match api.get_type(node_id) { NodeType::Sprite2D => ..., _ => ... }`
    #[cfg_attr(not(debug_assertions), inline)]
    pub fn get_type(&mut self, node_id: NodeID) -> crate::node_registry::NodeType {
        let node = self.scene.get_scene_node_ref(node_id)
            .unwrap_or_else(|| panic!("Node {} not found", node_id));
        // Use the node's get_type() method which now returns NodeType directly
        node.get_type()
    }
    
    /// Helper to create a ParentType from a node ID by looking up its type in the scene
    fn create_parent_type(&self, parent_id: NodeID) -> Option<crate::nodes::node::ParentType> {
        if let Some(parent_node) = self.scene.get_scene_node_ref(parent_id) {
            // Get the node type - we need to match on the SceneNode enum to get the actual type
            let node_type = match parent_node {
                crate::nodes::node_registry::SceneNode::Node(_) => crate::node_registry::NodeType::Node,
                crate::nodes::node_registry::SceneNode::Node2D(_) => crate::node_registry::NodeType::Node2D,
                crate::nodes::node_registry::SceneNode::Sprite2D(_) => crate::node_registry::NodeType::Sprite2D,
                crate::nodes::node_registry::SceneNode::Area2D(_) => crate::node_registry::NodeType::Area2D,
                crate::nodes::node_registry::SceneNode::CollisionShape2D(_) => crate::node_registry::NodeType::CollisionShape2D,
                crate::nodes::node_registry::SceneNode::ShapeInstance2D(_) => crate::node_registry::NodeType::ShapeInstance2D,
                crate::nodes::node_registry::SceneNode::Camera2D(_) => crate::node_registry::NodeType::Camera2D,
                crate::nodes::node_registry::SceneNode::UINode(_) => crate::node_registry::NodeType::UINode,
                crate::nodes::node_registry::SceneNode::Node3D(_) => crate::node_registry::NodeType::Node3D,
                crate::nodes::node_registry::SceneNode::MeshInstance3D(_) => crate::node_registry::NodeType::MeshInstance3D,
                crate::nodes::node_registry::SceneNode::Camera3D(_) => crate::node_registry::NodeType::Camera3D,
                crate::nodes::node_registry::SceneNode::DirectionalLight3D(_) => crate::node_registry::NodeType::DirectionalLight3D,
                crate::nodes::node_registry::SceneNode::OmniLight3D(_) => crate::node_registry::NodeType::OmniLight3D,
                crate::nodes::node_registry::SceneNode::SpotLight3D(_) => crate::node_registry::NodeType::SpotLight3D,
            };
            Some(crate::nodes::node::ParentType::new(parent_id, node_type))
        } else {
            None
        }
    }

    /// Remove a child from a parent node by directly mutating the scene
    /// This works with any node type through the BaseNode trait
    /// Remove a child from its parent
    /// Sets the child's parent to None and removes it from parent's children list
    pub fn remove_child(&mut self, parent_id: NodeID, child_id: NodeID) {
        // Remove from parent's children list
        if let Some(parent) = self.scene.get_scene_node_mut(parent_id) {
            parent.remove_child(&child_id);
        }
        
        // Set child's parent to None
        if let Some(child) = self.scene.get_scene_node_mut(child_id) {
            child.set_parent(None);
        }
    }
    
    /// Clear all children of a node, recursively deleting them and all their descendants
    /// This removes nodes from the hashmap, removes scripts, clears child lists, and updates caches
    pub fn clear_children(&mut self, parent_id: NodeID) {
        // Check if parent exists
        if self.scene.get_scene_node_ref(parent_id).is_none() {
            return;
        }
        
        // Recursively collect all descendant IDs first (before any mutations)
        // We need to collect them in a bottom-up order (all descendants before their parents)
        // to ensure safe deletion order - this prevents stop_rendering_recursive from
        // trying to access children that have already been deleted
        
        // Use a post-order traversal to collect nodes: children first, then parents
        // This naturally gives us the correct deletion order
        let mut all_descendant_ids = Vec::new();
        let mut visited = std::collections::HashSet::new();
        
        // Stack-based post-order traversal
        // Each element is (node_id, children_processed)
        // Only collect children that actually exist in the scene
        let mut stack: Vec<(NodeID, bool)> = {
            let children = self.scene.get_scene_node_ref(parent_id)
                .map(|node| {
                    node.get_children().iter().copied().collect::<Vec<NodeID>>()
                })
                .unwrap_or_default();
            
            children.iter()
                .copied()
                .filter(|&id| self.scene.get_scene_node_ref(id).is_some())
                .map(|id| (id, false))
                .collect()
        };
        
        while let Some((node_id, children_processed)) = stack.pop() {
            if visited.contains(&node_id) {
                continue;
            }
            
            // Check if node still exists (might have been deleted)
            if self.scene.get_scene_node_ref(node_id).is_none() {
                continue;
            }
            
            if children_processed {
                // All children have been processed, add this node
                all_descendant_ids.push(node_id);
                visited.insert(node_id);
            } else {
                // Mark that we're processing this node's children
                stack.push((node_id, true));
                
                // Add children to stack (they'll be processed first)
                // Only process children if node still exists
                if let Some(node) = self.scene.get_scene_node_ref(node_id) {
                    for child_id in node.get_children() {
                        // Only add if child exists and hasn't been visited
                        let child_exists = self.scene.get_scene_node_ref(*child_id).is_some();
                        let not_visited = !visited.contains(child_id);
                        
                        if not_visited && child_exists {
                            stack.push((*child_id, false));
                        }
                    }
                }
            }
        }
        
        // Delete all descendants - they're already in bottom-up order (children before parents)
        // remove_node already checks if node exists, so this is safe
        let gfx_ref = self.gfx.as_mut().expect("Graphics required for clear_children");
        
        for descendant_id in all_descendant_ids.iter() {
            // Double-check node still exists (might have been deleted as a child of another node)
            // This can happen if a node appears in multiple branches of the tree
            if self.scene.get_scene_node_ref(*descendant_id).is_some() {
                self.scene.remove_node(*descendant_id, gfx_ref);
            }
        }
        
        // Clear the parent's children list (should already be empty, but ensure it's clean)
        if let Some(parent) = self.scene.get_scene_node_mut(parent_id) {
            parent.clear_children();
        }
        
        // Update Node2D children cache for the parent (now empty)
        self.scene.update_node2d_children_cache_on_clear(parent_id);
    }

    /// Remove a node from the scene
    /// This recursively removes all children first, then removes the node from its parent's children list (if it has a parent), and finally deletes the node
    /// Example: `api.remove_node(node_id);`
    pub fn remove_node(&mut self, node_id: NodeID) {
        // Check if node exists
        if self.scene.get_scene_node_ref(node_id).is_none() {
            return; // Node doesn't exist, nothing to do
        }
        
        // First, recursively remove all children (this deletes all descendants)
        self.clear_children(node_id);
        
        // Get the parent ID before we delete the node
        let parent_id_opt = {
            let node = self.scene.get_scene_node_ref(node_id);
            node.and_then(|n| n.get_parent().map(|p| p.id))
        };
        
        // Remove from parent's children list if it has a parent
        if let Some(parent_id) = parent_id_opt {
            // Remove from parent's children list
            if let Some(parent) = self.scene.get_scene_node_mut(parent_id) {
                parent.remove_child(&node_id);
            }
            
            // Update Node2D children cache for the parent
            self.scene.update_node2d_children_cache_on_remove(parent_id, node_id);
        }
        
        // Now actually remove the node from the scene
        let gfx_ref = self.gfx.as_mut().expect("Graphics required for remove_node");
        self.scene.remove_node(node_id, gfx_ref);
    }

    /// Get the global transform for a node (calculates lazily if dirty)
    pub fn get_global_transform(&mut self, node_id: NodeID) -> Option<crate::structs2d::Transform2D> {
        self.scene.get_global_transform(node_id)
    }

    /// Set the global transform for a node (marks it as dirty)
    pub fn set_global_transform(&mut self, node_id: NodeID, transform: crate::structs2d::Transform2D) -> Option<()> {
        self.scene.set_global_transform(node_id, transform)
    }

    /// Set a script variable by name
    /// 
    /// Self-calls are now supported - you can call this with `self.id` from within the same script.
    /// The script will use the stored script pointer to access variables directly.
    pub fn set_script_var(&mut self, node_id: NodeID, name: &str, val: Value) -> Option<()> {
        let var_id = string_to_u64(name);

        self.set_script_var_id(node_id, var_id, val)
    }

    pub fn set_script_var_id(&mut self, node_id: NodeID, var_id: u64, val: Value) -> Option<()> {
        // Scripts are now always in memory as Rc<UnsafeCell<>>, so we can access them directly
        // SAFETY: Setting variables is safe because:
        // - It's equivalent to calling a mutable function on the script
        // - All access is controlled by the API (synchronous, single-threaded)
        // - The API ensures proper ordering and synchronization
        // - Nested calls are safe (same execution context, as if inlined)
        // - Setting variables on different scripts is safe (each script is independently managed)
        // - No memory leaks - each script's state is properly contained
        if let Some(script_rc) = self.scene.get_script(node_id) {
            unsafe {
                let script_ptr = script_rc.get();
                let script_mut = &mut *script_ptr;
                let script_mut = Box::as_mut(script_mut);
                script_mut.set_var(var_id, val)?;
            }
            Some(())
        } else {
            // Script not found
            None
        }
    }

    /// Get a script variable by name
    /// 
    /// Self-calls are now supported - you can call this with `self.id` from within the same script.
    /// The script will use the stored script pointer to access variables directly.
    pub fn get_script_var(&mut self, id: NodeID, name: &str) -> Value {
        let var_id = string_to_u64(name);

        self.get_script_var_id(id, var_id)
    }

    pub fn get_script_var_id(&mut self, id: NodeID, var_id: u64) -> Value {
        // Scripts are now always in memory as Rc<UnsafeCell<>>, so we can access them directly
        // SAFETY: Reading variables is safe because:
        // - We're only reading (immutable access)
        // - Values are cloned/copied (no shared mutable state)
        // - All access is controlled by the API
        // - Execution is synchronous (no concurrent reads)
        if let Some(script_rc) = self.scene.get_script(id) {
            unsafe {
                let script_ptr = script_rc.get();
                let script_ref = &*script_ptr;
                let script_ref = Box::as_ref(script_ref);
                script_ref.get_var(var_id).unwrap_or_default()
            }
        } else {
            // Script not found
            Value::Null
        }
    }

    //prints

    pub fn print<T: std::fmt::Display>(&self, msg: T) {
        println!("{}", msg);
    }

    /// Print a warning in yellow
    pub fn print_warn<T: std::fmt::Display>(&self, msg: T) {
        // [WARN] in bright yellow, message in dim golden yellow
        println!("\x1b[93m[WARN]\x1b[0m \x1b[33m{}\x1b[0m", msg);
    }

    /// Print an error with `[ERROR]` in ruby red and message in red
    pub fn print_error<T: std::fmt::Display>(&self, msg: T) {
        // `[ERROR]` in ruby-like red (38;5;160), message in standard red (31)
        println!("\x1b[38;5;160m[ERROR]\x1b[0m \x1b[31m{}\x1b[0m", msg);
    }

    /// Print info in blue/cyan tones (bright blue tag, cyan/yellowish message)
    pub fn print_info<T: std::fmt::Display>(&self, msg: T) {
        // [INFO] in bright blue, message in cyan (or pale yellow if you want warmth)
        println!("\x1b[94m[INFO]\x1b[0m \x1b[96m{}\x1b[0m", msg);
    }
}
