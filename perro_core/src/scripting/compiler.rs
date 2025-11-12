use std::collections::{HashSet, VecDeque};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use rand::RngCore;
use rand::seq::SliceRandom;

use crate::apply_fur::parse_fur_file;
use crate::ast::{FurElement, FurNode};
use crate::{BaseNode, SceneData};
use crate::asset_io::{resolve_path, ResolvedPath};
use crate::brk::build_brk;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub enum BuildProfile {
    Dev,
    Release,
    Check, // just validate
}

pub enum CompileTarget {
    Scripts, //.perro/scripts
    Project, // .perro/project
    VerboseProject
}

#[derive(Debug, Clone)]
pub enum Platform {
    Windows,
    MacOS,
    Linux,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Platform::Windows
        } else if cfg!(target_os = "macos") {
            Platform::MacOS
        } else {
            Platform::Linux
        }
    }

    pub fn toolchain_name(&self, version: &str) -> String {
        match self {
            Platform::Windows => format!("rust-{}-x86_64-pc-windows-gnu", version),
            Platform::MacOS => format!("rust-{}-x86_64-apple-darwin", version),
            Platform::Linux => format!("rust-{}-x86_64-unknown-linux-gnu", version),
        }
    }

    pub fn cargo_exe(&self) -> &'static str {
        match self {
            Platform::Windows => "cargo.exe",
            Platform::MacOS | Platform::Linux => "cargo",
        }
    }
}

pub struct Compiler {
    pub crate_manifest_path: PathBuf,
    target: CompileTarget,
    toolchain_root: Option<PathBuf>,
    platform: Platform,
    toolchain_version: Option<String>,
    project_root: PathBuf,
    from_source: bool,
}

impl Compiler {
    pub fn new(project_root: &Path, target: CompileTarget, from_source: bool) -> Self {
        let manifest = match target {
            CompileTarget::Scripts => project_root.join(".perro/scripts/Cargo.toml"),
            CompileTarget::Project | CompileTarget::VerboseProject => project_root.join(".perro/project/Cargo.toml"),
        };

        let manifest = dunce::canonicalize(&manifest).unwrap_or(manifest);

        let mut compiler = Self {
            crate_manifest_path: manifest,
            target,
            toolchain_root: None,
            platform: Platform::current(),
            toolchain_version: None,
            project_root: project_root.to_path_buf(),
            from_source
        };

        compiler.load_toolchain_config();
        compiler
    }

    pub fn with_toolchain_root<P: AsRef<Path>>(mut self, toolchain_root: P) -> Self {
        self.toolchain_root = Some(toolchain_root.as_ref().to_path_buf());
        self
    }

    fn load_toolchain_config(&mut self) {
        if let Ok(project) = crate::manifest::Project::load(Some(&self.project_root)) {
            if let Some(toolchain_version) = project.get_meta("toolchain") {
                eprintln!("📋 Found toolchain version in project metadata: {}", toolchain_version);
                self.toolchain_version = Some(toolchain_version.to_string());
                
                if self.toolchain_root.is_none() {
                    match resolve_path("user://toolchains") {
                        ResolvedPath::Disk(path_buf) => {
                            self.toolchain_root = Some(path_buf);
                        }
                        ResolvedPath::Brk(_) => {
                            eprintln!("⚠️  user://toolchains resolved to BRK path, falling back to project-relative");
                            let toolchain_root = self.project_root.join(".perro").join("toolchains");
                            self.toolchain_root = Some(toolchain_root);
                        }
                    }
                }
            }
        }
    }

    fn get_toolchain_dir(&self) -> Option<PathBuf> {
        let version = self.toolchain_version.as_deref().unwrap_or("1.90.0");
        let toolchain_name = self.platform.toolchain_name(version);
        let toolchain_path_str = format!("user://toolchains/{}", toolchain_name);
        
        match resolve_path(&toolchain_path_str) {
            ResolvedPath::Disk(path_buf) => Some(path_buf),
            ResolvedPath::Brk(_) => None,
        }
    }

    fn get_cargo_path(&self) -> Option<PathBuf> {
        self.get_toolchain_dir().map(|toolchain_dir| {
            toolchain_dir
                .join("cargo")
                .join("bin")
                .join(self.platform.cargo_exe())
        })
    }

    fn build_command(&self, profile: BuildProfile) -> Result<Command, String> {
        let mut cmd = if self.from_source {
            eprintln!("🔧 Using system cargo (debug mode)");
            Command::new("cargo")
        } else {
            // Try to use toolchain cargo, fallback to system
            if let Some(cargo_path) = self.get_cargo_path() {
                if cargo_path.exists() {
                    eprintln!("✅ Using toolchain cargo: {}", cargo_path.display());
                    Command::new(cargo_path)
                } else {
                    eprintln!("⚠️  Toolchain cargo not found, using system cargo");
                    Command::new("cargo")
                }
            } else {
                eprintln!("🔧 Using system cargo (no custom toolchain)");
                Command::new("cargo")
            }
        };

        match profile {
            BuildProfile::Check => cmd.arg("check"),
            _ => cmd.arg("build"),
        };

        let num_cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        cmd.arg("--manifest-path")
            .arg(&self.crate_manifest_path)
            .arg("-j")
            .arg(num_cpus.to_string())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        match self.target {
            CompileTarget::Scripts => {
                cmd.arg("--profile").arg("hotreload");
            }
            CompileTarget::Project | CompileTarget::VerboseProject => {
                match profile {
                    BuildProfile::Dev => cmd.arg("--profile").arg("dev"),
                    BuildProfile::Release => cmd.arg("--release"),
                    BuildProfile::Check => &mut cmd,
                };
                
                cmd.env("PERRO_BUILD_TIMESTAMP", std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    .to_string());
            }
        }

        Ok(cmd)
    }

    pub fn compile(&self, profile: BuildProfile) -> Result<(), String> {
       
        
        if matches!(self.target, CompileTarget::Project) {
            let mut key = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut key);

            println!("🔑 Compile-time AES key: {:02X?}", key);

            // Time key file writing (usually very fast, but good for completeness)
            let key_write_start = Instant::now();
            self.write_key_file(&key).map_err(|e| e.to_string())?;
            let key_write_elapsed = key_write_start.elapsed();
            println!("✔ Key file written (total {:.2?})", key_write_elapsed);

              // Generate scenes in the project crate instead of scripts crate
            let project_manifest = self.project_root.join(".perro/project/Cargo.toml");
            if project_manifest.exists() {
                let project_crate_root = project_manifest
                    .parent()
                    .expect("Project crate manifest has no parent");
                println!("⚙️ Generating static scene code for project crate...");
                let codegen_start = Instant::now();
                
                self.codegen_assets(project_crate_root)
                    .map_err(|e| format!("Asset codegen failed: {}", e))?;
                let codegen_elapsed = codegen_start.elapsed();
                println!("✅ Asset codegen complete (total {:.2?})", codegen_elapsed);
            } else {
                eprintln!(
                    "⚠️  Could not find project manifest at {}; skipping scene codegen.",
                    project_manifest.display()
                );
            }


            let res_dir = self.project_root.join("res");
            let output = self.project_root.join("assets.brk");


            // --- TIME THE BRK BUILD HERE ---
            println!("📦 Building BRK archive from {}...", res_dir.display());
            let brk_build_start = Instant::now();
            build_brk(&output, &res_dir, &self.project_root, &key)
                .map_err(|e| e.to_string())?;
            let brk_build_elapsed = brk_build_start.elapsed();
            println!("✅ BRK archive built (total {:.2?})", brk_build_elapsed);
            // --- END BRK TIMING ---
        }



        let toolchain_info = if self.from_source {
            "system (local development)".to_string()
        } else {
            let version = self.toolchain_version.as_deref().unwrap_or("1.83.0");
            let toolchain_name = self.platform.toolchain_name(version);
            
            self.get_toolchain_dir()
                .map(|p| format!("{} ({})", toolchain_name, p.display()))
                .unwrap_or_else(|| "system (fallback)".to_string())
        };

        println!("🚀 Compiling {:?} [{:?}] with toolchain: {}", 
            self.target_name(), 
            profile,
            toolchain_info
        );
        
        let start = Instant::now();
        let mut cmd = self.build_command(profile)?;
        let status = cmd
            .status()
            .map_err(|e| format!("Failed to run cargo: {e}"))?;
        let elapsed = start.elapsed();

        if status.success() {
            println!("✅ Compilation successful! (total {:.2?})", elapsed);
            Ok(())
        } else {
            Err(format!("❌ Compilation failed after {:.2?}", elapsed))
        }
    }

    fn target_name(&self) -> &'static str {
        match self.target {
            CompileTarget::Scripts => "scripts",
            CompileTarget::Project | CompileTarget::VerboseProject => "project",
        }
    }

    fn write_key_file(&self, key: &[u8; 32]) -> std::io::Result<()> {
        // Split into 4 parts of 8 bytes (fixed-size array instead of Vec)
        let mut parts: [[u8; 8]; 4] = [[0; 8]; 4];
        for (i, chunk) in key.chunks(8).enumerate() {
            parts[i].copy_from_slice(chunk);
        }

        // Generate 8 random constants (fixed-size array instead of Vec)
        let mut consts: [u32; 8] = [0; 8];
        for i in 0..8 {
            consts[i] = rand::random::<u32>();
        }

        // Random operations (unchanged, as ops is a static array)
        let ops = ["^", "+", "-", ">>", "<<"];

        // Build mask expressions (runtime code) and mask values (fixed-size arrays)
        // Note: mask_exprs still needs heap allocation for String content,
        // but the container itself is now fixed size.
        let mut mask_exprs: [String; 4] = [
            String::new(), String::new(), String::new(), String::new()
        ];
        let mut mask_values: [u8; 4] = [0; 4];

        for i in 0..4 { // Loop 4 times as there are 4 parts
            let c1 = rand::random::<usize>() % 8;
            let c2 = rand::random::<usize>() % 8;
            let op = ops.choose(&mut rand::thread_rng()).unwrap();

            let expr = match *op {
                "^" => {
                    mask_values[i] = (consts[c1] as u8) ^ (consts[c2] as u8);
                    format!("((CONST{} as u8) ^ (CONST{} as u8))", c1 + 1, c2 + 1)
                }
                "+" => {
                    mask_values[i] = (consts[c1] as u8).wrapping_add(consts[c2] as u8);
                    format!("((CONST{} as u8).wrapping_add(CONST{} as u8))", c1 + 1, c2 + 1)
                }
                "-" => {
                    mask_values[i] = (consts[c1] as u8).wrapping_sub(consts[c2] as u8);
                    format!("((CONST{} as u8).wrapping_sub(CONST{} as u8))", c1 + 1, c2 + 1)
                }
                ">>" => {
                    mask_values[i] = ((consts[c1] >> 8) as u8) ^ (consts[c2] as u8);
                    format!("((CONST{} >> 8) as u8) ^ (CONST{} as u8)", c1 + 1, c2 + 1)
                }
                "<<" => {
                    mask_values[i] = ((consts[c1] << 3) as u8) ^ (consts[c2] as u8);
                    format!("(((CONST{} << 3) as u8) ^ (CONST{} as u8))", c1 + 1, c2 + 1)
                }
                _ => unreachable!(),
            };

            mask_exprs[i] = expr; // Assign to the fixed-size array
        }

        // Path to key.rs
        let key_path = self
            .crate_manifest_path
            .parent()
            .unwrap()
            .join("src")
            .join("key.rs");

        let mut f = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&key_path)?;

        writeln!(f, "// Auto-generated by Perro compiler")?;

        // Write masked parts
        for (i, part) in parts.iter().enumerate() {
            write!(f, "const PART{}: [u8; 8] = [", i + 1)?;
            for (j, b) in part.iter().enumerate() {
                if j > 0 {
                    write!(f, ", ")?;
                }
                // Apply mask at compile time
                let masked = b ^ mask_values[i]; // mask_values access now matches part index
                write!(f, "0x{:02X}", masked)?;
            }
            writeln!(f, "];")?;
        }

        // Write constants
        for (i, c) in consts.iter().enumerate() {
            writeln!(f, "const CONST{}: u32 = 0x{:08X};", i + 1, c)?;
        }

        // Write get_aes key with inlined mask reconstruction
        writeln!(f, "pub fn get_aes_key() -> [u8; 32] {{")?;
        writeln!(f, "    let mut key = [0u8; 32];")?;
        writeln!(f, "    for i in 0..8 {{")?;
        writeln!(f, "        key[i]      = PART1[i] ^ ({});", mask_exprs[0])?;
        writeln!(f, "        key[i + 8]  = PART2[i] ^ ({});", mask_exprs[1])?;
        writeln!(f, "        key[i + 16] = PART3[i] ^ ({});", mask_exprs[2])?;
        writeln!(f, "        key[i + 24] = PART4[i] ^ ({});", mask_exprs[3])?;
        writeln!(f, "    }}")?;
        writeln!(f, "    key")?;
        writeln!(f, "}}")?;
        Ok(())
    }

    fn codegen_assets(&self, project_crate_root: &Path) -> anyhow::Result<()> {
        // Ensure src directory exists within the project crate
        let project_src_dir = project_crate_root.join("src");
        fs::create_dir_all(&project_src_dir)?;

        println!("🎬 Generating static scene definitions...");
        self.codegen_scenes_file(&project_src_dir)?;

        println!("📋 Generating static FUR UI definitions...");
        self.codegen_fur_file(&project_src_dir)?;

        println!("📝 Generating static Project manifest...");
        self.codegen_manifest_file(&project_src_dir)?;

        Ok(())
    }

    fn codegen_scenes_file(&self, project_src_dir: &Path) -> anyhow::Result<()> {
        use regex::Regex;
        use std::fmt::Write as _;

        let scenes_output_path = project_src_dir.join("scenes.rs");
        let mut scenes_file = File::create(&scenes_output_path)?;

        // --- File header ---
        writeln!(scenes_file, "// Auto-generated by Perro Engine compiler")?;
        writeln!(scenes_file, "#![allow(clippy::all)]")?;
        writeln!(scenes_file, "use once_cell::sync::Lazy;")?;
        writeln!(scenes_file, "use uuid::{{Uuid,uuid}};")?;
        writeln!(scenes_file, "use indexmap::IndexMap;")?;
        writeln!(scenes_file, "use perro_core::scene::SceneData;")?;
        writeln!(scenes_file, "use perro_core::structs::*;")?;
        writeln!(scenes_file, "use perro_core::node_registry::*;")?;
        writeln!(scenes_file, "use perro_core::nodes::*;")?;
        writeln!(scenes_file, "use perro_core::ui_node::UINode;")?;
        writeln!(scenes_file, "use std::{{borrow::Cow, collections::HashMap}};")?;
        writeln!(scenes_file, "\n// --- GENERATED SCENE DEFINITIONS ---")?;

        let res_dir = self.project_root.join("res");
        if !res_dir.exists() {
            eprintln!(
                "WARNING: `res` directory not found at {}. No scenes will be compiled.",
                res_dir.display()
            );
            // Still create an empty PERRO_SCENES map
            writeln!(scenes_file, "\n/// A map of scene paths to their statically compiled SceneData blueprints.")?;
            writeln!(scenes_file, "pub static PERRO_SCENES: Lazy<HashMap<&'static str, &'static SceneData>> = Lazy::new(|| {{")?;
            writeln!(scenes_file, "    HashMap::new()")?;
            writeln!(scenes_file, "}});")?;
            scenes_file.flush()?;
            return Ok(());
        }

        let mut processed_scene_paths: HashSet<String> = HashSet::new();
        let mut scene_queue: VecDeque<String> = VecDeque::new();
        let mut static_scene_definitions_code = String::new();
        let mut map_insertions_code = String::new();

        // --- Walk `res/` for *.scn files ---
        for entry in WalkDir::new(&res_dir) {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "scn") {
                println!("cargo:rerun-if-changed={}", path.display());
                let relative_path = path.strip_prefix(&res_dir)?.to_string_lossy().to_string();
                let res_path = format!("res://{}", relative_path.replace('\\', "/"));
                if processed_scene_paths.insert(res_path.clone()) {
                    scene_queue.push_back(res_path);
                }
            }
        }

        // --- Generate static definitions ---
        while let Some(current_res_path) = scene_queue.pop_front() {
            let local_path = current_res_path.strip_prefix("res://").unwrap();
            let full_fs_path = res_dir.join(local_path);
            if !full_fs_path.exists() {
                eprintln!("Skipping missing {}", full_fs_path.display());
                continue;
            }

            let mut scene_data: SceneData = SceneData::load(&current_res_path)?;
            SceneData::fix_relationships(&mut scene_data);

            let static_scene_name = Self::sanitize_res_path_to_ident(&current_res_path);
            let root_id_str = scene_data.root_id.to_string();

            let mut entries = String::new();
            for (uuid, node) in &scene_data.nodes {
                let mut node_str = format!("{:#?}", node);

                // --- UUID fixups ---
                let uuid_literal_regex = Regex::new(
                    r"\b([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\b"
                )?;
                node_str = uuid_literal_regex
                .replace_all(&node_str, "uuid!(\"$1\")")
                .to_string();


                // Normalize whitespace and string conversions
                node_str = node_str.replace("Some(\n", "Some(");
                // Handle Some("value")  ->  Some("value".to_string())
                let string_some_regex = Regex::new(r#"Some\(\s*"([^"]+)"\s*,?\s*\)"#)?;
                node_str = string_some_regex
                .replace_all(&node_str, "Some(Cow::Borrowed(\"$1\"))")
                .to_string();

                let string_field_regex = Regex::new(r#":\s*"([^"]+)","#)?;
                node_str = string_field_regex
                .replace_all(&node_str, ": Cow::Borrowed(\"$1\"),")
                .to_string();


                node_str = node_str.replace(": []", ": vec![]");
                node_str = node_str.replace(": {},", ": HashMap::new(),");

                // --- Option<Vec<Uuid>>: safe bracket correction ---
                let regex_children = Regex::new(r"children:\s*Some\s*\(\s*\[")?;
                let regex_root_ids = Regex::new(r"root_ids:\s*Some\s*\(\s*\[")?;
                node_str = regex_children
                    .replace_all(&node_str, "children: Some(vec![")
                    .to_string();
                node_str = regex_root_ids
                    .replace_all(&node_str, "root_ids: Some(vec![")
                    .to_string();

                let regex_children_empty = Regex::new(r"children:\s*Some\s*\(\s*\[\s*\]\s*\)")?;
                let regex_root_ids_empty = Regex::new(r"root_ids:\s*Some\s*\(\s*\[\s*\]\s*\)")?;
                node_str = regex_children_empty
                    .replace_all(&node_str, "children: Some(vec![])")
                    .to_string();
                node_str = regex_root_ids_empty
                    .replace_all(&node_str, "root_ids: Some(vec![])")
                    .to_string();

                // --- Extract SceneNode variant ---
                if let Some(open_paren) = node_str.find('(') {
                    if let Some(variant_pos) = node_str.find("SceneNode::") {
                        let variant_start = variant_pos + "SceneNode::".len();
                        let variant_end = open_paren;
                        let variant_name = node_str[variant_start..variant_end].trim();

                        let inner_start = open_paren + 1;
                        let inner = node_str[inner_start..]
                            .trim_end()
                            .trim_end_matches(')')
                            .trim();

                        writeln!(
                            &mut entries,
                            "        (uuid!(\"{}\"), SceneNode::{}({})),",
                            uuid, variant_name, inner
                        )?;

                    } else {
                      writeln!(
                        &mut entries,
                        "        (uuid!(\"{}\"), SceneNode::{}),",
                        uuid, node_str.trim()
                    )?;

                    }
                }
            }

            let indexmap_formatted = format!("IndexMap::from([\n{}\n    ])", entries);

            // Write static scene (without pub modifier)
            static_scene_definitions_code.push_str(&format!(
                "
/// Auto-generated static scene for {path}
static {name}: Lazy<SceneData> = Lazy::new(|| SceneData {{
    root_id: uuid!(\"{root_id}\"),
    nodes: {nodes},
}});
",
                path = current_res_path,
                name = static_scene_name,
                root_id = root_id_str,
                nodes = indexmap_formatted
            ));

            map_insertions_code.push_str(&format!(
                "    m.insert(\"{}\", &*{});\n",
                current_res_path, static_scene_name
            ));
        }

        // --- Write all scene definitions ---
        writeln!(scenes_file, "{}", static_scene_definitions_code)?;

        // --- Write PERRO_SCENES map directly in scenes.rs ---
        writeln!(scenes_file, "\n/// A map of scene paths to their statically compiled SceneData blueprints.")?;
        writeln!(scenes_file, "pub static PERRO_SCENES: Lazy<HashMap<&'static str, &'static SceneData>> = Lazy::new(|| {{")?;
        writeln!(scenes_file, "    let mut m = HashMap::new();")?;
        write!(scenes_file, "{}", map_insertions_code)?;
        writeln!(scenes_file, "    m")?;
        writeln!(scenes_file, "}});")?;

        scenes_file.flush()?;

        Ok(())
    }

    // Helper to convert a res:// path into a valid Rust static identifier.
    fn sanitize_res_path_to_ident(res_path: &str) -> String {
        res_path
            .trim_start_matches("res://")
            .replace(".scn", "")
            .replace(".fur", "")
            .replace("/", "_")
            .replace("-", "_")
            .to_uppercase()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect::<String>()
    }
fn codegen_fur_file(&self, project_src_dir: &Path) -> anyhow::Result<()> {
    use regex::Regex;
    use std::fmt::Write as _;

    let fur_output_path = project_src_dir.join("fur.rs");
    let mut fur_file = File::create(&fur_output_path)?;

    // --- File header ---
    writeln!(fur_file, "// Auto-generated by Perro Engine compiler")?;
    writeln!(fur_file, "#![allow(clippy::all)]")?;
    writeln!(fur_file, "use once_cell::sync::Lazy;")?;
    writeln!(fur_file, "use uuid::Uuid;")?;
    writeln!(fur_file, "use indexmap::IndexMap;")?;
    writeln!(fur_file, "use perro_core::ui::ast::{{FurElement, FurNode}};")?;
    writeln!(fur_file, "use std::collections::HashMap;")?;
    writeln!(fur_file, "use std::borrow::Cow;")?;
    writeln!(fur_file, "\n// --- GENERATED FUR DEFINITIONS ---")?;

    let res_dir = self.project_root.join("res");
    if !res_dir.exists() {
        eprintln!(
            "WARNING: `res` directory not found at {}. No FUR files will be compiled.",
            res_dir.display()
        );
        // Still create an empty PERRO_FUR map
        writeln!(fur_file, "\n/// A map of FUR file paths to their statically compiled UI element trees.")?;
        writeln!(fur_file, "pub static PERRO_FUR: &[(&str, &[FurElement])] = &[];")?;
        writeln!(fur_file, "")?;
        fur_file.flush()?;
        return Ok(());
    }

    let mut processed_fur_paths: HashSet<String> = HashSet::new();
    let mut fur_queue: VecDeque<String> = VecDeque::new();
    let mut static_fur_definitions_code = String::new();
    let mut map_insertions_code = String::new();

    // --- Walk `res/` for *.fur files ---
    for entry in WalkDir::new(&res_dir) {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "fur") {
            println!("cargo:rerun-if-changed={}", path.display());
            let relative_path = path.strip_prefix(&res_dir)?.to_string_lossy().to_string();
            let res_path = format!("res://{}", relative_path.replace('\\', "/"));
            if processed_fur_paths.insert(res_path.clone()) {
                fur_queue.push_back(res_path);
            }
        }
    }

    // --- Generate static definitions ---
    while let Some(current_res_path) = fur_queue.pop_front() {
        // Parse directly using the virtual path (res://...),
        // letting load_asset() handle resolution
        let ast = parse_fur_file(&current_res_path)
            .map_err(|e| anyhow::anyhow!("Failed to parse FUR file {}: {}", current_res_path, e))?;

        // Extract FurElements from the AST
        let fur_elements: Vec<FurElement> = ast
            .into_iter()
            .filter_map(|f| match f {
               FurNode::Element(el) => Some(el),
                _ => None,
            })
            .collect();

        if fur_elements.is_empty() {
            eprintln!("⚠️  No elements found in {}, skipping", current_res_path);
            continue;
        }

        let static_fur_name = Self::sanitize_res_path_to_ident(&current_res_path);

        // Generate Rust code for this FUR file's elements
        let mut elements_code = String::new();
        for element in &fur_elements {
            let element_code = self.codegen_fur_element(element, 1)?;
            writeln!(&mut elements_code, "        {},", element_code)?;
        }

        static_fur_definitions_code.push_str(&format!(
            r#"
/// Auto-generated static FUR elements for {path}
pub static {name}: Lazy<Vec<FurElement>> = Lazy::new(|| vec![
{elements}
]);
"#,
            path = current_res_path,
            name = static_fur_name,
            elements = elements_code
        ));

      map_insertions_code.push_str(&format!(
    "    m.insert(\"{}\", {}.as_slice());\n",
    current_res_path, static_fur_name
));
    }

    // --- Write all FUR definitions ---
    writeln!(fur_file, "{}", static_fur_definitions_code)?;

// --- Write PERRO_FUR map directly in fur.rs ---
writeln!(fur_file, "\n/// A map of FUR file paths to their statically compiled UI element trees.")?;
writeln!(fur_file, "pub static PERRO_FUR: Lazy<HashMap<&'static str, &'static [FurElement]>> = Lazy::new(|| {{")?;
writeln!(fur_file, "    let mut m = HashMap::new();")?;
write!(fur_file, "{}", map_insertions_code)?;
writeln!(fur_file, "    m")?;
writeln!(fur_file, "}});")?;
writeln!(fur_file, "")?;

    fur_file.flush()?;

    Ok(())
}

fn codegen_fur_element(&self, element: &FurElement, indent_level: usize) -> anyhow::Result<String> {
    use std::fmt::Write as _;
    
    let indent = "    ".repeat(indent_level);
    let mut code = String::new();

    writeln!(&mut code, "{}FurElement {{", indent)?;
    writeln!(&mut code, "{}    tag_name: Cow::Borrowed(\"{}\"),", indent, element.tag_name)?;
    writeln!(&mut code, "{}    id: Cow::Borrowed(\"{}\"),", indent, element.id)?;
    
    // Generate attributes HashMap
    if element.attributes.is_empty() {
        writeln!(&mut code, "{}    attributes: HashMap::new(),", indent)?;
    } else {
        writeln!(&mut code, "{}    attributes: HashMap::from([", indent)?;
        for (key, value) in &element.attributes {
            writeln!(
                &mut code, 
                "{}        (Cow::Borrowed(\"{}\"), Cow::Borrowed(\"{}\")),", 
                indent, 
                key, 
                value.replace("\"", "\\\"")
            )?;
        }
        writeln!(&mut code, "{}    ]),", indent)?;
    }

    // Generate children Vec<FurNode>
    if element.children.is_empty() {
        writeln!(&mut code, "{}    children: vec![],", indent)?;
    } else {
        writeln!(&mut code, "{}    children: vec![", indent)?;
        for child in &element.children {
            match child {
                FurNode::Element(child_el) => {
                    let child_code = self.codegen_fur_element(child_el, indent_level + 2)?;
                    writeln!(&mut code, "{}        FurNode::Element({}),", indent, child_code.trim())?;
                }
                FurNode::Text(text) => {
                    writeln!(
                        &mut code, 
                        "{}        FurNode::Text(Cow::Borrowed(\"{}\")),", 
                        indent, 
                        text.replace("\"", "\\\"")
                    )?;
                }
            }
        }
        writeln!(&mut code, "{}    ],", indent)?;
    }

    writeln!(&mut code, "{}    self_closing: {},", indent, element.self_closing)?;
    write!(&mut code, "{}}}", indent)?;

    Ok(code)
}

    // Add this method to your Compiler impl block in the compiler file
fn codegen_manifest_file(&self, project_src_dir: &Path) -> anyhow::Result<()> {
    use std::fmt::Write as _;

    let manifest_output_path = project_src_dir.join("manifest.rs");
    let mut manifest_file = File::create(&manifest_output_path)?;

    // Load the project manifest from project.toml
    let project_toml_path = self.project_root.join("project.toml");
    let project = crate::manifest::Project::load_from_file(&project_toml_path)
        .map_err(|e| anyhow::anyhow!("Failed to load project manifest: {}", e))?;

    // --- File header ---
    writeln!(manifest_file, "// Auto-generated by Perro Engine compiler")?;
    writeln!(manifest_file, "#![allow(clippy::all)]")?;
    writeln!(manifest_file, "use once_cell::sync::Lazy;")?;
    writeln!(manifest_file, "use perro_core::manifest::Project;")?;
    writeln!(manifest_file, "\n// --- GENERATED PROJECT MANIFEST ---")?;

    // Generate static metadata PHF map
    let metadata_map_name = "PERRO_METADATA";
    if !project.metadata().is_empty() {
        writeln!(manifest_file, "\nstatic {}: phf::Map<&'static str, &'static str> = phf::phf_map! {{", metadata_map_name)?;
        for (key, value) in project.metadata() {
            writeln!(
                manifest_file,
                "    \"{}\" => \"{}\",",
                key,
                value.replace("\"", "\\\"")
            )?;
        }
        writeln!(manifest_file, "}};")?;
    }

    // Generate the Lazy Project
    writeln!(manifest_file, "\n/// Statically compiled project manifest")?;
    writeln!(manifest_file, "pub static PERRO_PROJECT: Lazy<Project> = Lazy::new(|| {{")?;
    writeln!(manifest_file, "    Project::new_static(")?;
    writeln!(manifest_file, "        \"{}\".to_string(),", project.name())?;
    writeln!(manifest_file, "        \"{}\".to_string(),", project.version())?;
    writeln!(manifest_file, "        \"{}\".to_string(),", project.main_scene())?;
    
    // Handle optional icon
    if let Some(icon) = project.icon() {
        writeln!(manifest_file, "        Some(\"{}\".to_string()),", icon)?;
    } else {
        writeln!(manifest_file, "        None,")?;
    }
    
    writeln!(manifest_file, "        {}f32,", project.target_fps())?;
    writeln!(manifest_file, "        {}f32,", project.xps())?;
    
    // Handle optional root script
    if let Some(script) = project.root_script() {
        writeln!(manifest_file, "        Some(\"{}\".to_string()),", script)?;
    } else {
        writeln!(manifest_file, "        None,")?;
    }
    
    // Pass PHF map reference
    if !project.metadata().is_empty() {
        writeln!(manifest_file, "        &{},", metadata_map_name)?;
    } else {
        writeln!(manifest_file, "        &phf::phf_map! {{}},")?;
    }
    
    writeln!(manifest_file, "    )")?;
    writeln!(manifest_file, "}});")?;

    manifest_file.flush()?;

    Ok(())
}
}