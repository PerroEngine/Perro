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
    node_registry::{IntoInner, SceneNode},
    prelude::string_to_u64,
    script::{CreateFn, SceneAccess, Script, UpdateOp, Var},
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
        unsafe {
            (*input_api_ptr).get_parent_ptr()
        }
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
                Ok(devices) => {
                    devices.into_iter().map(|(serial, vid, pid)| {
                        serde_json::json!({
                            "serial": serial,
                            "vendor_id": vid,
                            "product_id": pid
                        })
                    }).collect()
                },
                Err(e) => {
                    api.print_error(&format!("Joy-Con scan failed: {:?}", e));
                    vec![]
                },
            }
        } else {
            api.print_error("No controller manager found in scene");
            vec![]
        }
    }
    
    /// Scan for Joy-Con 2 devices (BLE) - async, returns empty for now
    /// Note: This would need async support in the scripting API
    pub fn scan_joycon2(&mut self) -> Vec<Value> {
        // TODO: Implement async scan when scripting API supports async
        vec![]
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
    
    pub(crate) fn connect_joycon1_impl(api: &mut ScriptApi, serial: &str, vid: u64, pid: u64) -> bool {
        if let Some(mgr) = api.scene.get_controller_manager() {
            let mgr = mgr.lock().unwrap();
            match mgr.connect_joycon1(serial, vid as u16, pid as u16) {
                Ok(_) => {
                    api.print(&format!("Successfully connected to Joy-Con: {}", serial));
                    true
                },
                Err(e) => {
                    api.print_error(&format!("Failed to connect to Joy-Con {}: {:?}", serial, e));
                    false
                },
            }
        } else {
            api.print_error("No controller manager found in scene");
            false
        }
    }
    
    /// Get data from all connected controllers
    pub fn get_data(&mut self) -> Vec<Value> {
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
    
    pub(crate) fn get_data_impl(api: &mut ScriptApi) -> Vec<Value> {
        if let Some(mgr) = api.scene.get_controller_manager() {
            let mgr = mgr.lock().unwrap();
            let data = mgr.get_data();
            data.into_iter().map(|controller| {
                let report = controller.latest_report.as_ref().map(|r| {
                    // Apply calibration
                    let (calibrated_gyro, calibrated_accel) = controller.calibration.apply(r.gyro, r.accel);
                    
                    // Apply stick calibration: subtract center offset to get values centered at 0
                    // If center is 2000 and we read 2050, calibrated raw should be 50
                    let stick_h_raw_calibrated = r.stick.horizontal as i32 - controller.calibration.stick_center.horizontal as i32;
                    let stick_v_raw_calibrated = r.stick.vertical as i32 - controller.calibration.stick_center.vertical as i32;
                    
                    // Normalize to -1.0 to 1.0 range
                    const ESTIMATED_RANGE: f32 = 1500.0;
                    let stick_h_norm = (stick_h_raw_calibrated as f32 / ESTIMATED_RANGE).clamp(-1.0, 1.0);
                    let stick_v_norm = (stick_v_raw_calibrated as f32 / ESTIMATED_RANGE).clamp(-1.0, 1.0);
                    
                    // Apply deadzone: values between -50 and 50 raw units should be treated as 0
                    const STICK_DEADZONE_RAW: i32 = 50;
                    let stick_h = if stick_h_raw_calibrated.abs() < STICK_DEADZONE_RAW { 0.0 } else { stick_h_norm };
                    let stick_v = if stick_v_raw_calibrated.abs() < STICK_DEADZONE_RAW { 0.0 } else { stick_v_norm };
                    
                    // For h_raw and v_raw, use the calibrated (offset) values
                    let stick_h_raw = stick_h_raw_calibrated;
                    let stick_v_raw = stick_v_raw_calibrated;
                    
                    // Apply gyro deadzone: values between -50 and 50 deg/s should be treated as 0
                    const GYRO_DEADZONE: f32 = 50.0;
                    let gyro_x = if calibrated_gyro.x.abs() < GYRO_DEADZONE { 0.0 } else { calibrated_gyro.x };
                    let gyro_y = if calibrated_gyro.y.abs() < GYRO_DEADZONE { 0.0 } else { calibrated_gyro.y };
                    let gyro_z = if calibrated_gyro.z.abs() < GYRO_DEADZONE { 0.0 } else { calibrated_gyro.z };
                    
                    // Build JSON with side-specific buttons
                    let buttons_json = if controller.is_left {
                        serde_json::json!({
                            "up": r.buttons.up,
                            "down": r.buttons.down,
                            "left": r.buttons.left,
                            "right": r.buttons.right,
                            "l": r.buttons.l,
                            "zl": r.buttons.zl,
                            "minus": r.buttons.minus,
                            "capture": r.buttons.capture,
                            "sl": r.buttons.sl,
                            "sr": r.buttons.sr,
                            "stick_press": r.buttons.stick_press,
                        })
                    } else {
                        serde_json::json!({
                            "a": r.buttons.a,
                            "b": r.buttons.b,
                            "x": r.buttons.x,
                            "y": r.buttons.y,
                            "r": r.buttons.r,
                            "zr": r.buttons.zr,
                            "home": r.buttons.home,
                            "plus": r.buttons.plus,
                            "sl": r.buttons.sl,
                            "sr": r.buttons.sr,
                            "stick_press": r.buttons.stick_press,
                        })
                    };
                    
                    serde_json::json!({
                        "buttons": buttons_json,
                        "stick": {
                            "h_raw": stick_h_raw,
                            "v_raw": stick_v_raw,
                            "h": stick_h,
                            "v": stick_v,
                        },
                        "gyro": {
                            "x": gyro_x,
                            "y": gyro_y,
                            "z": gyro_z,
                        },
                        "accel": {
                            "x": calibrated_accel.x,
                            "y": calibrated_accel.y,
                            "z": calibrated_accel.z,
                        },
                        "battery_level": r.battery_level,
                        "charging": r.charging,
                    })
                });
                serde_json::json!({
                    "serial": controller.serial,
                    "is_left": controller.is_left,
                    "is_joycon2": controller.is_joycon2,
                    "report": report,
                })
            }).collect()
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
            match mgr.enable_polling() {
                Ok(_) => {
                    api.print("Joy-Con polling enabled");
                    true
                },
                Err(e) => {
                    api.print_error(&format!("Failed to enable Joy-Con polling: {:?}", e));
                    false
                },
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

pub struct InputApi {
    pub JoyCon: JoyConApi,
    // Pointer to the parent ScriptApi - set when accessed through DerefMut
    parent_api_ptr: Option<*mut ScriptApi<'static>>,
}

impl Default for InputApi {
    fn default() -> Self {
        Self {
            JoyCon: JoyConApi::default(),
            parent_api_ptr: None,
        }
    }
}

impl InputApi {
    /// Set the parent ScriptApi pointer
    fn set_parent_ptr(&mut self, api_ptr: *mut ScriptApi<'static>) {
        
        self.parent_api_ptr = Some(api_ptr);
        // Also set it in JoyConApi
        self.JoyCon.set_api_ptr(api_ptr);
 
    }
    
    /// Set the parent ScriptApi pointer (immutable version for Deref)
    fn set_parent_ptr_immut(&self, api_ptr: *mut ScriptApi<'static>) {
       
        unsafe {
            let self_mut = self as *const InputApi as *mut InputApi;
            (*self_mut).parent_api_ptr = Some(api_ptr);
            (*self_mut).JoyCon.set_api_ptr(api_ptr);
        }
      
    }
    
    /// Get the parent ScriptApi pointer
    fn get_parent_ptr(&self) -> Option<*mut ScriptApi<'static>> {
     
        self.parent_api_ptr
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
        let api_ptr: *mut ScriptApi<'static> = unsafe { std::mem::transmute(self as *const ScriptApi<'a> as *mut ScriptApi<'static>) };
      
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
            // Set the pointer in InputApi and JoyConApi
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
    scene: &'a mut dyn SceneAccess,
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
    
    /// Get data from all connected controllers
    pub fn get_joycon_data(&mut self) -> Vec<Value> {
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
        if let Some(script_rc) = self.scene.get_script(script_id) {
            script_rc.borrow_mut().engine_init(self);
        }
        Self::clear_context();
    }

    pub fn call_update(&mut self, id: Uuid) {
        self.set_context();
        if let Some(rc_script) = self.scene.get_script(id) {
            let mut script = rc_script.borrow_mut();
            script.engine_update(self);
        }
        Self::clear_context();
    }

    pub fn call_function(&mut self, id: Uuid, func: &str, params: &SmallVec<[Value; 3]>) {
        let func_id = self.string_to_u64(func);
        self.call_function_id(id, func_id, params);
    }

    pub fn call_function_id(&mut self, id: Uuid, func: u64, params: &SmallVec<[Value; 3]>) {
        self.set_context();
        if let Some(rc_script) = self.scene.get_script(id) {
            let mut script = rc_script.borrow_mut();
            script.call_function(func, self, params);
        }
        Self::clear_context();
    }

    pub fn string_to_u64(&mut self, string: &str) -> u64 {
        string_to_u64(string)
    }

    // The human-friendly one
    pub fn emit_signal(&mut self, name: &str, params: SmallVec<[Value; 3]>) {
        let id = self.string_to_u64(name);
        self.scene.queue_signal_id(id, params);
    }

    // The low-level one
    pub fn emit_signal_id(&mut self, id: u64, params: SmallVec<[Value; 3]>) {
        self.scene.queue_signal_id(id, params);
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
        boxed.set_node_id(Uuid::nil()); // explicitly detached

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
            .get_scene_node(id)
            .unwrap_or_else(|| panic!("Node {} not found", id))
            .clone();
        node_enum.into_inner()
    }
    pub fn merge_nodes(&mut self, nodes: Vec<SceneNode>) {
        self.scene.merge_nodes(nodes);
    }

    pub fn set_script_var(&mut self, node_id: Uuid, name: &str, val: Value) -> Option<()> {
        let var_id = string_to_u64(name);

        self.set_script_var_id(node_id, var_id, val)
    }

    pub fn set_script_var_id(&mut self, node_id: Uuid, var_id: u64, val: Value) -> Option<()> {
        let rc_script = self.scene.get_script(node_id)?;
        let mut script = rc_script.borrow_mut();

        script.set_var(var_id, val)?;
        Some(())
    }

    pub fn get_script_var(&mut self, id: Uuid, name: &str) -> Value {
        let var_id = string_to_u64(name);

        self.get_script_var_id(id, var_id)
    }

    pub fn get_script_var_id(&mut self, id: Uuid, var_id: u64) -> Value {
        if let Some(rc_script) = self.scene.get_script(id) {
            let script = rc_script.borrow_mut();
            return script.get_var(var_id).unwrap_or_default();
        }
        Value::Null
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
