#![allow(improper_ctypes_definitions)]
#![allow(unused)]

use std::any::Any;
use std::collections::HashMap;
use serde_json::{Value, json};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use perro_core::prelude::*;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::process::Command;
use rust_decimal::{Decimal, prelude::FromPrimitive};
use smallvec::{SmallVec, smallvec};
use phf::{phf_map, Map};

/// @PerroScript
pub static MEMBER_TO_ATTRIBUTES_MAP: Map<&'static str, &'static [&'static str]> = phf_map! {
};

static ATTRIBUTE_TO_MEMBERS_MAP: Map<&'static str, &'static [&'static str]> = phf_map! {
};

struct UpdaterScript {
    base: Node,
    check_timer: f32,
    state: UpdateState,
    my_version: String,
}

#[unsafe(no_mangle)]
pub extern "C" fn scripts_updater_rs_create_script() -> *mut dyn ScriptObject {
    Box::into_raw(Box::new(UpdaterScript {
        base: Node::new("Updater", None),
        check_timer: 0.0,
        state: UpdateState::Initial,
        my_version: String::new(),
    })) as *mut dyn ScriptObject
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
enum UpdateState {
    Initial,
    CheckingCache,
    CheckingOnline,
    DownloadingUpdate { version: String },
    ReadyToRelaunch { version: String },
    UpToDate,
    Error(String),
}


const MANIFEST_CACHE_HOURS: u64 = 6;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct Manifest {
    latest: String,
    versions: Vec<String>,
}

impl UpdaterScript {
    // ------------------- Cache Management -------------------

    fn is_manifest_cache_valid(&self, api: &ScriptApi) -> bool {
        if let Some(path) = api.resolve_path("user://manifest.json") {
            if let Ok(md) = std::fs::metadata(&path) {
                if let Ok(modified) = md.modified() {
                    if let Ok(elapsed) = modified.elapsed() {
                        return elapsed < Duration::from_secs(MANIFEST_CACHE_HOURS * 3600);
                    }
                }
            }
        }
        false
    }

    fn load_cached_manifest(&self, api: &ScriptApi) -> Option<Manifest> {
        api.resolve_path("user://manifest.json")
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|text| serde_json::from_str(&text).ok())
    }

    fn save_manifest_cache(&self, api: &ScriptApi, manifest: &Manifest) {
        if let Some(path) = api.resolve_path("user://manifest.json") {
            if let Ok(json) = serde_json::to_string_pretty(manifest) {
                std::fs::write(path, json).ok();
            }
        }
    }

    // ------------------- Network Operations -------------------

    fn download_file(&self, url: &str, dest_path: &Path) -> Result<(), String> {
        eprintln!("📥 Downloading: {}", url);
        
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create dir: {}", e))?;
        }

        // Try curl first
        let curl_result = Command::new("curl")
            .args(&[
                "-L",
                "-f",
                "--retry",
                "3",
                "--connect-timeout",
                "30",
                "-o",
                dest_path.to_str().unwrap(),
                url,
            ])
            .output();

        match curl_result {
            Ok(output) if output.status.success() => {
                eprintln!("✅ Downloaded with curl");
                return Ok(());
            }
            Ok(output) => {
                eprintln!("⚠️ curl failed: {}", String::from_utf8_lossy(&output.stderr));
            }
            Err(_) => eprintln!("⚠️ curl not available, trying wget..."),
        }

        // Fallback to wget
        let wget_result = Command::new("wget")
            .args(&[
                "--tries=3",
                "--timeout=30",
                "-O",
                dest_path.to_str().unwrap(),
                url,
            ])
            .output();

        match wget_result {
            Ok(output) if output.status.success() => {
                eprintln!("✅ Downloaded with wget");
                Ok(())
            }
            Ok(output) => Err(format!(
                "wget failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )),
            Err(_) => Err("Neither curl nor wget available".to_string()),
        }
    }

    fn fetch_online_manifest(&self, api: &ScriptApi) -> Result<Manifest, String> {
        eprintln!("📡 Fetching manifest from CDN...");

        let url = "https://cdn.perroengine.com/manifest.json";
        let tmp_path = std::env::temp_dir().join("perro_manifest.json");

        self.download_file(url, &tmp_path)?;

        let manifest_text = std::fs::read_to_string(&tmp_path)
            .map_err(|e| format!("Read manifest: {}", e))?;

        let manifest: Manifest = serde_json::from_str(&manifest_text)
            .map_err(|e| format!("Parse manifest: {}", e))?;

        self.save_manifest_cache(api, &manifest);
        eprintln!("✅ Manifest fetched: latest = {}", manifest.latest);
        
        Ok(manifest)
    }

    // ------------------- Version Management -------------------

    fn find_exe_in_dir(&self, dir: &Path) -> Option<PathBuf> {
        std::fs::read_dir(dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().map_or(false, |e| e == "exe"))
    }

    fn version_exists(&self, api: &ScriptApi, version: &str) -> bool {
        let path_str = format!("user://versions/{}/editor/", version);
        if let Some(resolved) = api.resolve_path(&path_str) {
            self.find_exe_in_dir(Path::new(&resolved)).is_some()
        } else {
            false
        }
    }

    fn download_and_install_version(
        &self,
        api: &ScriptApi,
        version: &str,
    ) -> Result<(), String> {
        eprintln!("🚀 Installing engine version {}...", version);

        let editor_dir_str = format!("user://versions/{}/editor/", version);
        let editor_dir_resolved = api
            .resolve_path(&editor_dir_str)
            .ok_or("Failed to resolve editor dir")?;

        let editor_path = Path::new(&editor_dir_resolved);
        std::fs::create_dir_all(editor_path)
            .map_err(|e| format!("Failed to create editor dir: {}", e))?;

        let editor_exe = editor_path.join("Perro_Engine.exe");

        if !editor_exe.exists() {
            let url = format!(
                "https://cdn.perroengine.com/versions/{}/Perro_Engine.exe",
                version
            );

            eprintln!("📥 Downloading engine: {}", url);
            self.download_file(&url, &editor_exe)?;
        }

        eprintln!("✅ Engine version {} installed", version);
        Ok(())
    }

    // ------------------- Launch -------------------

    fn launch_version_and_close(
        &self,
        api: &ScriptApi,
        version: &str,
    ) -> Result<(), String> {
        let path_str = format!("user://versions/{}/editor/", version);
        
        if let Some(resolved) = api.resolve_path(&path_str) {
            if let Some(exe) = self.find_exe_in_dir(Path::new(&resolved)) {
                eprintln!("🚀 Launching {} and closing current", exe.display());

                let parent_dir = exe
                    .parent()
                    .ok_or("Could not determine parent directory")?;

                let args: Vec<String> = std::env::args().skip(1).collect();

                std::process::Command::new(&exe)
                    .current_dir(parent_dir)
                    .args(&args)
                    .spawn()
                    .map_err(|e| format!("Failed to launch: {}", e))?;

                // Close current process
                std::process::exit(0);
            }
        }
        
        Err(format!("No exe found for version {}", version))
    }

    // ------------------- Update Logic -------------------

    fn process_update_check(&mut self, api: &ScriptApi) {
        match &self.state {
            UpdateState::Initial => {
                eprintln!("🔍 Starting update check...");
                self.state = UpdateState::CheckingCache;
            }

            UpdateState::CheckingCache => {
                // Check cached manifest first (instant)
                if self.is_manifest_cache_valid(api) {
                    if let Some(manifest) = self.load_cached_manifest(api) {
                        eprintln!("📋 Using cached manifest (valid for {} hours)", MANIFEST_CACHE_HOURS);
                        self.handle_manifest(api, manifest);
                        return;
                    }
                }
                
                eprintln!("📋 Cache invalid or missing, checking online...");
                self.state = UpdateState::CheckingOnline;
            }

            UpdateState::CheckingOnline => {
                match self.fetch_online_manifest(api) {
                    Ok(manifest) => {
                        self.handle_manifest(api, manifest);
                    }
                    Err(e) => {
                        eprintln!("⚠️ Failed to fetch online manifest: {}", e);
                        
                        // Try cached manifest as fallback
                        if let Some(manifest) = self.load_cached_manifest(api) {
                            eprintln!("📋 Using stale cached manifest as fallback");
                            self.handle_manifest(api, manifest);
                        } else {
                            eprintln!("❌ No network and no cache available");
                            self.state = UpdateState::Error(
                                "No internet connection and no cached manifest".to_string()
                            );
                        }
                    }
                }
            }

            UpdateState::DownloadingUpdate { version } => {
                let version = version.clone();
                eprintln!("📦 Downloading version {}...", version);
                
                match self.download_and_install_version(api, &version) {
                    Ok(_) => {
                        eprintln!("✅ Download complete!");
                        self.state = UpdateState::ReadyToRelaunch { version };
                    }
                    Err(e) => {
                        eprintln!("❌ Download failed: {}", e);
                        self.state = UpdateState::Error(e);
                    }
                }
            }

            UpdateState::ReadyToRelaunch { version } => {
                let version = version.clone();
                eprintln!("🚀 Relaunching with version {}...", version);
                
                if let Err(e) = self.launch_version_and_close(api, &version) {
                    eprintln!("❌ Failed to relaunch: {}", e);
                    self.state = UpdateState::Error(e);
                }
            }

            UpdateState::UpToDate => {
                // Do nothing, already up to date
            }

            UpdateState::Error(_) => {
                // Do nothing, error already logged
            }
        }
    }

    fn handle_manifest(&mut self, api: &ScriptApi, manifest: Manifest) {
        let latest = &manifest.latest;
        
        if natord::compare(latest, &self.my_version).is_gt() {
            eprintln!("🎉 Update available: {} -> {}", self.my_version, latest);
            
            if self.version_exists(api, latest) {
                eprintln!("✅ Version {} already downloaded", latest);
                self.state = UpdateState::ReadyToRelaunch { version: latest.clone() };
            } else {
                eprintln!("📦 Need to download version {}", latest);
                self.state = UpdateState::DownloadingUpdate { version: latest.clone() };
            }
        } else {
            eprintln!("✅ Already running latest version: {}", self.my_version);
            self.state = UpdateState::UpToDate;
        }
    }
}

impl Script for UpdaterScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {

      

        self.my_version = api.project().version().to_string();
        
        eprintln!("🔄 Updater initialized for version {}", self.my_version);
        
        // Skip in debug builds
        if cfg!(debug_assertions) {
            eprintln!("🐛 Debug build: updater disabled");
            return;
        }

        // Start update check after 1.0 seconds
        self.check_timer = -1.0;
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
        if cfg!(debug_assertions) {
            return;
        }

        self.check_timer += api.Time.get_delta();

        // Start checking after 1.0 seconds
        if self.check_timer >= 0.0 && self.state == UpdateState::Initial {
            self.process_update_check(api);
        } else if self.check_timer >= 0.0 && 
                  (self.state == UpdateState::CheckingCache || 
                   self.state == UpdateState::CheckingOnline) {
            self.process_update_check(api);
        } else if matches!(self.state, UpdateState::DownloadingUpdate { .. }) {
            self.process_update_check(api);
        } else if matches!(self.state, UpdateState::ReadyToRelaunch { .. }) {
            self.process_update_check(api);
        }
    }
}

// Natural ordering for version comparison
mod natord {
    pub fn compare(a: &str, b: &str) -> std::cmp::Ordering {
        let a: Vec<&str> = a.split('.').collect();
        let b: Vec<&str> = b.split('.').collect();
        let len = a.len().max(b.len());
        
        for i in 0..len {
            let ai = a.get(i).unwrap_or(&"0");
            let bi = b.get(i).unwrap_or(&"0");
            
            if let (Ok(na), Ok(nb)) = (ai.parse::<u32>(), bi.parse::<u32>()) {
                match na.cmp(&nb) {
                    std::cmp::Ordering::Equal => continue,
                    other => return other,
                }
            } else {
                match ai.cmp(bi) {
                    std::cmp::Ordering::Equal => continue,
                    other => return other,
                }
            }
        }
        
        std::cmp::Ordering::Equal
    }
}


impl ScriptObject for UpdaterScript {
    fn set_node_id(&mut self, id: Uuid) {
        self.base.id = id;
    }

    fn get_node_id(&self) -> Uuid {
        self.base.id
    }

    fn get_var(&self, var_id: u64) -> Option<Value> {
        VAR_GET_TABLE.get(&var_id).and_then(|f| f(self))
    }

    fn set_var(&mut self, var_id: u64, val: Value) -> Option<()> {
        VAR_SET_TABLE.get(&var_id).and_then(|f| f(self, val))
    }

    fn apply_exposed(&mut self, hashmap: &HashMap<u64, Value>) {
        for (var_id, val) in hashmap.iter() {
            if let Some(f) = VAR_APPLY_TABLE.get(var_id) {
                f(self, val);
            }
        }
    }

    fn call_function(
        &mut self,
        id: u64,
        api: &mut ScriptApi<'_>,
        params: &[Value],
    ) {
        if let Some(f) = DISPATCH_TABLE.get(&id) {
            f(self, params, api);
        }
    }

    // Attributes

    fn attributes_of(&self, member: &str) -> Vec<String> {
        MEMBER_TO_ATTRIBUTES_MAP
            .get(member)
            .map(|attrs| attrs.iter().map(|s| s.to_string()).collect())
            .unwrap_or_default()
    }

    fn members_with(&self, attribute: &str) -> Vec<String> {
        ATTRIBUTE_TO_MEMBERS_MAP
            .get(attribute)
            .map(|members| members.iter().map(|s| s.to_string()).collect())
            .unwrap_or_default()
    }

    fn has_attribute(&self, member: &str, attribute: &str) -> bool {
        MEMBER_TO_ATTRIBUTES_MAP
            .get(member)
            .map(|attrs| attrs.iter().any(|a| *a == attribute))
            .unwrap_or(false)
    }
}

// =========================== Static PHF Dispatch Tables ===========================

static VAR_GET_TABLE: phf::Map<u64, fn(&UpdaterScript) -> Option<Value>> =
    phf::phf_map! {

    };

static VAR_SET_TABLE: phf::Map<u64, fn(&mut UpdaterScript, Value) -> Option<()>> =
    phf::phf_map! {

    };

static VAR_APPLY_TABLE: phf::Map<u64, fn(&mut UpdaterScript, &Value)> =
    phf::phf_map! {

    };

static DISPATCH_TABLE: phf::Map<
    u64,
    fn(&mut UpdaterScript, &[Value], &mut ScriptApi<'_>),
> = phf::phf_map! {

    };
