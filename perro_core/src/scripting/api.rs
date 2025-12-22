// scripting/lang/script_api.rs
//! Perro Script API (single-file version with Deref)
//! Provides all engine APIs (JSON, Time, OS, Process) directly under `api`

use chrono::{Datelike, Local, Timelike};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use smallvec::SmallVec;
use std::{
    cell::RefCell,
    env, io,
    ops::{Deref, DerefMut},
    path::Path,
    process,
    rc::Rc,
    sync::mpsc::Sender,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use uuid::Uuid; // For date/time formatting

use crate::{
    Node, Node2D, Sprite2D,
    app_command::AppCommand,
    asset_io::{self, ResolvedPath, load_asset, resolve_path},
    compiler::{BuildProfile, CompileTarget, Compiler},
    input::joycon::ControllerManager,
    manifest::Project,
    node_registry::{BaseNode, IntoInner, SceneNode},
    prelude::string_to_u64,
    script::{CreateFn, SceneAccess, Script, Var},
    transpiler::{script_path_to_identifier, transpile},
    types::ScriptType,
    ui_node::UINode,
};
use std::sync::{Arc, Mutex};

//-----------------------------------------------------
// 1️⃣ Sub‑APIs (Engine modules)
//-----------------------------------------------------

#[derive(Default)]
pub struct JsonApi;
impl JsonApi {
    pub fn stringify<T: Serialize>(&self, val: &T) -> String {
        serde_json::to_string(val).unwrap_or_else(|_| "{}".to_string())
    }
    pub fn parse(&self, text: &str) -> Option<Value> {
        serde_json::from_str(text).ok()
    }
}

#[derive(Default)]
pub struct TimeApi {
    pub delta: f32,
}
impl TimeApi {
    pub fn get_unix_time_msec(&self) -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_millis(0))
            .as_millis()
    }
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
    pub fn sleep_msec(&self, ms: u64) {
        thread::sleep(Duration::from_millis(ms));
    }
    pub fn get_ticks_msec(&self) -> u128 {
        self.get_unix_time_msec()
    }
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
    fn set_api_ptr(&mut self, api_ptr: *mut ScriptApi<'static>) {
        self.api_ptr = Some(api_ptr);
    }

    /// Get the ScriptApi pointer - tries stored pointer, then gets from parent InputApi
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
                            buttons: JoyconButtons::default(),
                            stick: Vector2::zero(),
                            gyro: Vector3::zero(),
                            accel: Vector3::zero(),
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
    fn get_api_ptr(&mut self) -> Option<*mut ScriptApi<'static>> {
        if let Some(ptr) = self.api_ptr {
            return Some(ptr);
        }
        let input_api_ptr = self as *const KeyboardApi as *const InputApi;
        unsafe { (*input_api_ptr).get_parent_ptr() }
    }

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
    fn get_api_ptr(&mut self) -> Option<*mut ScriptApi<'static>> {
        if let Some(ptr) = self.api_ptr {
            return Some(ptr);
        }
        let input_api_ptr = self as *const MouseApi as *const InputApi;
        unsafe { (*input_api_ptr).get_parent_ptr() }
    }

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
            crate::structs2d::vector2::Vector2::zero()
        }
    }

    pub(crate) fn get_position_impl(api: &mut ScriptApi) -> crate::structs2d::vector2::Vector2 {
        if let Some(mgr) = api.scene.get_input_manager() {
            let mgr = mgr.lock().unwrap();
            mgr.get_mouse_position()
        } else {
            crate::structs2d::vector2::Vector2::zero()
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
                    crate::structs2d::vector2::Vector2::zero()
                }
            }
        } else {
            crate::structs2d::vector2::Vector2::zero()
        }
    }
}

pub struct InputApi {
    pub JoyCon: JoyConApi,
    pub Keyboard: KeyboardApi,
    pub Mouse: MouseApi,
    // Pointer to the parent ScriptApi - set when accessed through DerefMut
    parent_api_ptr: Option<*mut ScriptApi<'static>>,
}

impl Default for InputApi {
    fn default() -> Self {
        Self {
            JoyCon: JoyConApi::default(),
            Keyboard: KeyboardApi::default(),
            Mouse: MouseApi::default(),
            parent_api_ptr: None,
        }
    }
}

impl InputApi {
    /// Set the parent ScriptApi pointer
    fn set_parent_ptr(&mut self, api_ptr: *mut ScriptApi<'static>) {
        self.parent_api_ptr = Some(api_ptr);
        // Also set it in sub-APIs
        self.JoyCon.set_api_ptr(api_ptr);
        self.Keyboard.set_api_ptr(api_ptr);
        self.Mouse.set_api_ptr(api_ptr);
    }

    /// Set the parent ScriptApi pointer (immutable version for Deref)
    fn set_parent_ptr_immut(&self, api_ptr: *mut ScriptApi<'static>) {
        unsafe {
            let self_mut = self as *const InputApi as *mut InputApi;
            (*self_mut).parent_api_ptr = Some(api_ptr);
            (*self_mut).JoyCon.set_api_ptr(api_ptr);
            (*self_mut).Keyboard.set_api_ptr(api_ptr);
            (*self_mut).Mouse.set_api_ptr(api_ptr);
        }
    }

    /// Get the parent ScriptApi pointer
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

#[allow(non_snake_case)]
#[derive(Default)]
pub struct EngineApi {
    pub JSON: JsonApi,
    pub Time: TimeApi,
    pub OS: OsApi,
    pub Process: ProcessApi,
    pub Input: InputApi,
}

//-----------------------------------------------------
// 4️⃣  Deref Implementation
//-----------------------------------------------------

impl<'a> Deref for ScriptApi<'a> {
    type Target = EngineApi;
    fn deref(&self) -> &Self::Target {
        // Also set the parent pointer when accessed immutably
        // This ensures the pointer is set even if DerefMut isn't called
        let api_ptr: *mut ScriptApi<'static> =
            unsafe { std::mem::transmute(self as *const ScriptApi<'a> as *mut ScriptApi<'static>) };

        unsafe {
            let input_api = &(*api_ptr).engine.Input;
            input_api.set_parent_ptr_immut(api_ptr);
        }
        &self.engine
    }
}

impl<'a> DerefMut for ScriptApi<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Set the parent pointer in InputApi so JoyConApi can access ScriptApi
        // Cast to 'static lifetime (safe because we control the lifetime)
        let api_ptr: *mut ScriptApi<'static> = unsafe { std::mem::transmute(self) };

        unsafe {
            // Set the pointer in InputApi and all sub-APIs
            let input_api = &mut (*api_ptr).engine.Input;
            input_api.set_parent_ptr(api_ptr);
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
}

impl<'a> ScriptApi<'a> {
    pub fn new(delta: f32, scene: &'a mut dyn SceneAccess, project: &'a mut Project) -> Self {
        let mut engine = EngineApi::default();
        engine.Time.delta = delta;
        Self {
            scene,
            project,
            engine,
        }
    }

    /// Set the thread-local context for this ScriptApi
    /// This allows JoyConApi methods to access the ScriptApi
    pub(crate) fn set_context(&mut self) {
        let api_ptr: *mut ScriptApi<'static> = unsafe { std::mem::transmute(self) };

        SCRIPT_API_CONTEXT.with(|ctx| {
            *ctx.borrow_mut() = Some(api_ptr);
        });
    }

    pub(crate) fn clear_context() {
        SCRIPT_API_CONTEXT.with(|ctx| {
            *ctx.borrow_mut() = None;
        });
    }

    //-------------------------------------------------
    // Core access
    //-------------------------------------------------
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
    fn run_compile(&mut self, profile: BuildProfile, target: CompileTarget) -> Result<(), String> {
        let project_path_str = self
            .project
            .get_runtime_param("project_path")
            .ok_or("Missing runtime param: project_path")?;
        let project_path = Path::new(project_path_str);
        transpile(project_path, false).map_err(|e| format!("Transpile failed: {}", e))?;
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
    pub fn set_target_fps(&mut self, fps: f32) {
        self.project.set_target_fps(fps);
        if let Some(tx) = self.scene.get_command_sender() {
            let _ = tx.send(AppCommand::SetTargetFPS(fps));
        }
    }

    //-------------------------------------------------
    // Lifecycle / Updates
    //-------------------------------------------------
    pub fn call_init(&mut self, script_id: Uuid) {
        self.set_context();
        // Take/insert pattern needed to avoid borrow checker issues
        // (take_script/insert_script don't modify filtered vectors, just HashMap)
        if let Some(mut script) = self.scene.take_script(script_id) {
            // Check if script has init implemented before calling
            if script.script_flags().has_init() {
                script.engine_init(self);
            }
            self.scene.insert_script(script_id, script);
        }
        Self::clear_context();
    }

    pub fn call_update(&mut self, id: Uuid) {
        self.set_context();
        // Take/insert pattern needed to avoid borrow checker issues
        // (take_script/insert_script don't modify filtered vectors, just HashMap)
        if let Some(mut script) = self.scene.take_script(id) {
            // Check if script has update implemented before calling
            if script.script_flags().has_update() {
                script.engine_update(self);
            }
            self.scene.insert_script(id, script);
        }
        Self::clear_context();
    }

    pub fn call_fixed_update(&mut self, id: Uuid) {
        self.set_context();
        // Take/insert pattern needed to avoid borrow checker issues
        // (take_script/insert_script don't modify filtered vectors, just HashMap)
        if let Some(mut script) = self.scene.take_script(id) {
            // Check if script has fixed_update implemented before calling
            if script.script_flags().has_fixed_update() {
                script.engine_fixed_update(self);
            }
            self.scene.insert_script(id, script);
        }
        Self::clear_context();
    }

    pub fn call_node_internal_fixed_update(&mut self, node_id: Uuid) {
        // We need to get the node and call the method, but we can't hold a RefMut
        // while also borrowing self mutably. So we check if update is needed first,
        // then drop that borrow before calling the method.
        let needs_update = {
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

    pub fn call_function(&mut self, id: Uuid, func: &str, params: &[Value]) {
        let func_id = self.string_to_u64(func);
        self.call_function_id(id, func_id, params);
    }

    pub fn call_function_id(&mut self, id: Uuid, func: u64, params: &[Value]) {
        self.set_context();
        // Safely take script out, call method, put it back
        if let Some(mut script) = self.scene.take_script(id) {
            script.call_function(func, self, params);
            self.scene.insert_script(id, script);
        }
        Self::clear_context();
    }
    
    /// Internal fast-path call that skips context setup (used when context already set)
    #[inline]
    pub(crate) fn call_function_id_fast(&mut self, id: Uuid, func: u64, params: &[Value]) {
        // Safely take script out, call method, put it back (no context overhead)
        if let Some(mut script) = self.scene.take_script(id) {
            script.call_function(func, self, params);
            self.scene.insert_script(id, script);
        }
    }

    pub fn string_to_u64(&mut self, string: &str) -> u64 {
        string_to_u64(string)
    }

    // ========== INSTANT SIGNALS (zero-allocation, immediate) ==========
    
    /// Emit signal instantly - handlers called immediately
    /// Params passed as compile-time slice, zero allocation
    pub fn emit_signal(&mut self, name: &str, params: &[Value]) {
        let id = self.string_to_u64(name);
        self.emit_signal_id(id, params);
    }

    /// Emit signal instantly by ID - handlers called immediately
    /// Params passed as compile-time slice, zero allocation
    /// OPTIMIZED: Handles emission in existing API context to avoid double-borrow
    /// OPTIMIZED: Set context once and batch all calls to minimize overhead
    pub fn emit_signal_id(&mut self, id: u64, params: &[Value]) {
        let start = std::time::Instant::now();
        
        // Copy out listeners before calling functions
        let script_map_opt = self.scene.get_signal_connections(id);
        if script_map_opt.is_none() {
            return;
        }

        // OPTIMIZED: Use SmallVec with inline capacity of 4 listeners
        // Most signals have 1-3 listeners, so this avoids heap allocation in common case
        let script_map = script_map_opt.unwrap();
        let mut call_list = SmallVec::<[(Uuid, u64); 4]>::new();
        for (uuid, fns) in script_map.iter() {
            for &fn_id in fns.iter() {
                call_list.push((*uuid, fn_id));
            }
        }

        let setup_time = start.elapsed();
        let call_start = std::time::Instant::now();

        // OPTIMIZED: Set context once for all calls instead of per-call
        self.set_context();
        
        // OPTIMIZED: Use fast-path calls that skip redundant context operations
        for (target_id, fn_id) in call_list.iter() {
            self.call_function_id_fast(*target_id, *fn_id, params);
        }
        
        // Clear context once after all calls
        Self::clear_context();
        
        let call_time = call_start.elapsed();
        let total_time = start.elapsed();
        
        eprintln!("[SIGNAL TIMING] Signal ID: {} | Listeners: {} | Setup: {:?} | Calls: {:?} | Total: {:?}", 
            id, call_list.len(), setup_time, call_time, total_time);
    }

    // ========== DEFERRED SIGNALS (queued, processed at frame end) ==========
    
    /// Emit signal deferred - queued and processed at end of frame
    /// Use when emitting during iteration or need frame-end processing
    pub fn emit_signal_deferred(&mut self, name: &str, params: &[Value]) {
        let id = self.string_to_u64(name);
        self.scene.emit_signal_id_deferred(id, params);
    }

    /// Emit signal deferred by ID - queued and processed at end of frame
    /// Use when emitting during iteration or need frame-end processing
    pub fn emit_signal_id_deferred(&mut self, id: u64, params: &[Value]) {
        self.scene.emit_signal_id_deferred(id, params);
    }

    pub fn connect_signal(&mut self, name: &str, target: Uuid, function: &'static str) {
        let id = string_to_u64(name);
        let fn_id = string_to_u64(function);
        self.scene.connect_signal_id(id, target, fn_id);
    }

    pub fn connect_signal_id(&mut self, id: u64, target: Uuid, function_id: u64) {
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
        boxed.set_id(Uuid::nil()); // explicitly detached

        // run init() safely using a temporary sub‑APIs
        // note: doesn’t touch scene.scripts, only passes mut ref
        {
            let project_ref = self.project as *mut _;
            let project_mut = unsafe { &mut *project_ref };
            let mut sub_api = ScriptApi::new(0.0, self.scene, project_mut);
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
    pub fn get_node_clone<T: Clone>(&mut self, id: Uuid) -> T
    where
        SceneNode: IntoInner<T>,
    {
        let node_enum = self
            .scene
            .get_scene_node_ref(id)
            .unwrap_or_else(|| panic!("Node {} not found", id))
            .clone();
        node_enum.into_inner()
    }
    
    /// Read a value from a node using a closure
    /// The closure receives &T where T is the node type and returns any Clone value (including Copy types)
    /// For Copy types like primitives, no actual cloning happens at runtime
    /// For non-Copy types like String/Cow, the value is cloned out of the node
    /// Example: `let parent = api.read_node::<CollisionShape2D, _>(c_id, |c| c.parent_id);`
    /// Example: `let name = api.read_node::<Sprite2D, _>(self.id, |s| s.name.clone());`
    pub fn read_node<T: 'static, R: Clone>(&self, node_id: Uuid, f: impl FnOnce(&T) -> R) -> R {
        let node = self.scene.get_scene_node_ref(node_id)
            .unwrap_or_else(|| panic!("Node {} not found", node_id));
        
        let typed_node = node.as_any().downcast_ref::<T>()
            .unwrap_or_else(|| panic!("Node {} is not of type {}", node_id, std::any::type_name::<T>()));
        
        f(typed_node)
    }
    
    /// Mutate a node directly with a closure - no clones needed!
    /// The closure receives &mut T where T is the node type
    /// Returns true if the node was found and successfully mutated, false otherwise
    /// Example: `api.mutate_node::<Node2D>(node_id, |n| n.transform.position.x = 5.0);`
    pub fn mutate_node<T: 'static, F>(&mut self, id: Uuid, f: F)
    where
        F: FnOnce(&mut T),
    {
        // Get mutable access to the node
        let node = self.scene.get_scene_node_mut(id)
            .unwrap_or_else(|| panic!("Node {} not found", id));
        
        // Try to downcast to the requested type and mutate in place
        let typed_node = node.as_any_mut().downcast_mut::<T>()
            .unwrap_or_else(|| panic!("Node {} is not of type {}", id, std::any::type_name::<T>()));
        
        f(typed_node); // mutate in place
        
        // Mark dirty after modification
        node.mark_dirty();
        node.mark_transform_dirty_if_node2d();
        
        // If this is a Node2D, mark transform dirty recursively (including children)
        // This ensures children's global transforms are recalculated when parent moves
        if node.as_node2d().is_some() {
            self.scene.mark_transform_dirty_recursive(id);
        }
    }
    
    /// Create a new node of the specified type and add it to the scene
    /// Returns the ID of the newly created node
    /// Example: `let node_id = api.create_node::<Node2D>();`
    pub fn create_node<T>(&mut self) -> Uuid
    where
        T: Default,
        T: crate::nodes::node_registry::ToSceneNode,
    {
        let node: T = Default::default();
        let scene_node = node.to_scene_node();
        let id = scene_node.get_id();
        
        // Add to scene
        self.scene.add_node_to_scene(scene_node)
            .unwrap_or_else(|e| panic!("Failed to add node to scene: {}", e));
        
        id
    }
    
    /// Reparent a child node to a new parent
    /// Handles removing from old parent if it exists
    /// Example: `api.reparent(parent_id, child_id);`
    pub fn reparent(&mut self, new_parent_id: Uuid, child_id: Uuid) {
        // Don't reparent to nil parent
        if new_parent_id.is_nil() {
            return;
        }
        
        // Get the child's current parent_id (returns Uuid, nil if no parent)
        let old_parent_id = self.scene.get_scene_node_ref(child_id)
            .unwrap_or_else(|| panic!("Child node {} not found", child_id))
            .get_parent();
        
        // Remove from old parent if it has one (this also sets child's parent_id to nil)
        if !old_parent_id.is_nil() {
            self.remove_child(old_parent_id, child_id);
        }
        
        // Set the child's parent_id to the new parent
        if let Some(child_node) = self.scene.get_scene_node_mut(child_id) {
            child_node.set_parent(Some(new_parent_id));
        }
        
        // Add child to the new parent's children list
        if let Some(parent_node) = self.scene.get_scene_node_mut(new_parent_id) {
            parent_node.add_child(child_id);
        }
    }

    /// Get a child node by name, searching through the parent's children
    /// Returns the child node's ID if found, None otherwise
    pub fn get_child_by_name(&mut self, parent_id: Uuid, child_name: &str) -> Option<Uuid> {
        let children: Vec<Uuid> = {
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
    /// Returns the parent node's ID (Uuid::nil() if node not found or has no parent)
    pub fn get_parent(&mut self, node_id: Uuid) -> Uuid {
        if let Some(node) = self.scene.get_scene_node_ref(node_id) {
            node.get_parent()
        } else {
            Uuid::nil()
        }
    }

    pub fn merge_nodes(&mut self, nodes: &[SceneNode]) {
        self.scene.merge_nodes(nodes);
    }

    /// Remove a child from a parent node by directly mutating the scene
    /// This works with any node type through the BaseNode trait
    /// Remove a child from its parent
    /// Sets the child's parent_id to nil and removes it from parent's children list
    pub fn remove_child(&mut self, parent_id: Uuid, child_id: Uuid) {
        // Remove from parent's children list
        if let Some(parent) = self.scene.get_scene_node_mut(parent_id) {
            parent.remove_child(&child_id);
        }
        
        // Set child's parent_id to nil
        if let Some(child) = self.scene.get_scene_node_mut(child_id) {
            child.set_parent(None);
        }
    }

    /// Get the global transform for a node (calculates lazily if dirty)
    pub fn get_global_transform(&mut self, node_id: Uuid) -> Option<crate::structs2d::Transform2D> {
        self.scene.get_global_transform(node_id)
    }

    /// Set the global transform for a node (marks it as dirty)
    pub fn set_global_transform(&mut self, node_id: Uuid, transform: crate::structs2d::Transform2D) -> Option<()> {
        self.scene.set_global_transform(node_id, transform)
    }

    /// Set a script variable by name
    /// 
    /// **Warning**: Do NOT call this with `self.id` from within the same script's update/init methods!
    /// Scripts are temporarily removed during update, so accessing your own vars will return None.
    /// Instead, access your own fields directly (e.g., `self.my_field = value;`)
    /// 
    /// This is intended for cross-script communication only.
    pub fn set_script_var(&mut self, node_id: Uuid, name: &str, val: Value) -> Option<()> {
        let var_id = string_to_u64(name);

        self.set_script_var_id(node_id, var_id, val)
    }

    pub fn set_script_var_id(&mut self, node_id: Uuid, var_id: u64, val: Value) -> Option<()> {
        // Safely take script out, call method, put it back
        if let Some(mut script) = self.scene.take_script(node_id) {
            let result = script.set_var(var_id, val);
            self.scene.insert_script(node_id, script);
            result?;
            Some(())
        } else {
            // Script not found - may be currently taken out during its own update
            // This is expected if you try to access self.id from within a script
            None
        }
    }

    /// Get a script variable by name
    /// 
    /// **Warning**: Do NOT call this with `self.id` from within the same script's update/init methods!
    /// Scripts are temporarily removed during update, so accessing your own vars will return Value::Null.
    /// Instead, access your own fields directly (e.g., `let x = self.my_field;`)
    /// 
    /// This is intended for cross-script communication only.
    pub fn get_script_var(&mut self, id: Uuid, name: &str) -> Value {
        let var_id = string_to_u64(name);

        self.get_script_var_id(id, var_id)
    }

    pub fn get_script_var_id(&mut self, id: Uuid, var_id: u64) -> Value {
        // Safely take script out, call method, put it back
        if let Some(script) = self.scene.take_script(id) {
            let result = script.get_var(var_id).unwrap_or_default();
            self.scene.insert_script(id, script);
            result
        } else {
            // Script not found - may be currently taken out during its own update
            // This is expected if you try to access self.id from within a script
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
