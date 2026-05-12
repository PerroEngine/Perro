use std::{
    cell::RefCell,
    collections::HashMap,
    fs::{self, File},
    io::{self, Read, Seek, Write},
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, RwLock},
};

use crate::data_local_dir;
use perro_assets::archive::{PerroAssetsArchive, PerroAssetsFile};

pub type StaticBytesLookup = fn(u64) -> &'static [u8];
pub type StaticShaderLookup = fn(u64) -> &'static str;
pub type DlcStaticBinaryLookup = unsafe extern "C" fn(u64, *mut *const u8, *mut usize) -> bool;

#[derive(Clone, Copy, Debug, Default)]
pub struct StaticResourceLookups {
    pub texture_lookup: Option<StaticBytesLookup>,
    pub mesh_lookup: Option<StaticBytesLookup>,
    pub collision_trimesh_lookup: Option<StaticBytesLookup>,
    pub skeleton_lookup: Option<StaticBytesLookup>,
    pub shader_lookup: Option<StaticShaderLookup>,
    pub audio_lookup: Option<StaticBytesLookup>,
}

/// Trait alias for Read + Seek
pub trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

#[derive(Clone)]
pub enum ProjectRoot {
    Disk {
        root: PathBuf,
        name: String,
    },
    PerroAssets {
        data: &'static [u8],
        name: String,
        static_resource_lookups: StaticResourceLookups,
    },
}

static PROJECT_ROOT: RwLock<Option<ProjectRoot>> = RwLock::new(None);
static PERRO_ASSETS_ARCHIVE: RwLock<Option<PerroAssetsArchive>> = RwLock::new(None);
static DLC_MOUNTS: LazyLock<RwLock<HashMap<String, DlcMount>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static DLC_ARCHIVES: LazyLock<RwLock<HashMap<String, Arc<PerroAssetsArchive>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static DLC_STATIC_BINARY_LOOKUPS: LazyLock<RwLock<HashMap<String, DlcStaticBinaryLookup>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

thread_local! {
    static DLC_SELF_CONTEXT: RefCell<Option<String>> = const { RefCell::new(None) };
}

#[derive(Debug)]
pub struct DlcSelfContextGuard {
    previous: Option<String>,
}

#[derive(Clone, Debug)]
pub enum DlcMountSource {
    Disk(PathBuf),
    Archive(PathBuf),
}

#[derive(Clone, Debug)]
pub struct DlcMount {
    pub name: String,
    pub source: DlcMountSource,
}

pub fn is_reserved_dlc_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("self")
}

pub fn get_project_root() -> ProjectRoot {
    PROJECT_ROOT
        .read()
        .unwrap()
        .clone()
        .expect("Project root not set")
}

pub fn set_project_root(root: ProjectRoot) {
    *PROJECT_ROOT.write().unwrap() = Some(root.clone());

    if let ProjectRoot::PerroAssets { data, .. } = root {
        let archive =
            PerroAssetsArchive::open_from_bytes(data).expect("Failed to open PerroAssets archive");
        *PERRO_ASSETS_ARCHIVE.write().unwrap() = Some(archive);
    }
}

pub fn clear_dlc_mounts() {
    DLC_MOUNTS.write().unwrap().clear();
    DLC_ARCHIVES.write().unwrap().clear();
    DLC_STATIC_BINARY_LOOKUPS.write().unwrap().clear();
}

pub fn mounted_dlc_names() -> Vec<String> {
    let mut out = DLC_MOUNTS
        .read()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    out.sort();
    out
}

pub fn read_mounted_dlc_file(name: &str, virtual_path: &str) -> io::Result<Vec<u8>> {
    let key = name.to_ascii_lowercase();
    if let Some(archive) = DLC_ARCHIVES.read().unwrap().get(&key) {
        archive.read_file(virtual_path)
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("dlc archive mount not found: {name}"),
        ))
    }
}

pub fn mount_dlc_disk(name: &str, root: impl AsRef<Path>) -> io::Result<()> {
    if is_reserved_dlc_name(name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "dlc name `self` is reserved",
        ));
    }
    let root = root.as_ref().to_path_buf();
    if !root.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("dlc disk root not found: {}", root.display()),
        ));
    }
    DLC_MOUNTS.write().unwrap().insert(
        name.to_ascii_lowercase(),
        DlcMount {
            name: name.to_string(),
            source: DlcMountSource::Disk(root),
        },
    );
    Ok(())
}

pub fn mount_dlc_archive(name: &str, archive_path: impl AsRef<Path>) -> io::Result<()> {
    if is_reserved_dlc_name(name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "dlc name `self` is reserved",
        ));
    }
    let archive_path = archive_path.as_ref().to_path_buf();
    let archive = Arc::new(PerroAssetsArchive::open_from_file(&archive_path)?);
    let key = name.to_ascii_lowercase();
    DLC_ARCHIVES.write().unwrap().insert(key.clone(), archive);
    DLC_MOUNTS.write().unwrap().insert(
        key,
        DlcMount {
            name: name.to_string(),
            source: DlcMountSource::Archive(archive_path),
        },
    );
    Ok(())
}

pub fn register_dlc_static_binary_lookup(name: &str, lookup: DlcStaticBinaryLookup) {
    DLC_STATIC_BINARY_LOOKUPS
        .write()
        .unwrap()
        .insert(name.to_ascii_lowercase(), lookup);
}

pub fn set_dlc_self_context(name: Option<&str>) {
    DLC_SELF_CONTEXT.with(|ctx| {
        *ctx.borrow_mut() = name.map(|v| v.to_ascii_lowercase());
    });
}

pub fn push_dlc_self_context(name: Option<&str>) -> DlcSelfContextGuard {
    let next = name.map(|v| v.to_ascii_lowercase());
    let previous = DLC_SELF_CONTEXT.with(|ctx| {
        let mut ctx = ctx.borrow_mut();
        std::mem::replace(&mut *ctx, next)
    });
    DlcSelfContextGuard { previous }
}

impl Drop for DlcSelfContextGuard {
    fn drop(&mut self) {
        DLC_SELF_CONTEXT.with(|ctx| {
            *ctx.borrow_mut() = self.previous.take();
        });
    }
}

#[derive(Debug, Clone)]
pub enum ResolvedPath {
    Disk(PathBuf),
    PerroAssets(String),
    StaticBinary(String),
    DlcStaticBinary { dlc: String, path: String },
    DlcPerroAssets { dlc: String, virtual_path: String },
}

fn normalize_user_app_name(name: &str) -> String {
    name.replace(' ', "_")
}

/// Resolve virtual path (res://foo/bar.png or user://save.dat) to actual location
pub fn resolve_path(path: &str) -> ResolvedPath {
    let project_root_opt = PROJECT_ROOT.read().unwrap().clone();

    // Handle user:// paths (always disk)
    if let Some(stripped) = path.strip_prefix("user://") {
        let app_name = project_root_opt
            .as_ref()
            .map(|root| match root {
                ProjectRoot::Disk { name, .. } => name.as_str(),
                ProjectRoot::PerroAssets { name, .. } => name.as_str(),
            })
            .expect("Project root not set");
        let app_name = normalize_user_app_name(app_name);

        let base = data_local_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join(app_name)
            .join("data");
        return ResolvedPath::Disk(base.join(stripped));
    }

    if let Some(rest) = path.strip_prefix("dlc://") {
        let (mount_raw, rel_raw) = rest.split_once('/').unwrap_or((rest, ""));
        let rel = rel_raw.trim_start_matches('/');
        let mount_name = if mount_raw.eq_ignore_ascii_case("self") {
            DLC_SELF_CONTEXT.with(|ctx| ctx.borrow().clone())
        } else {
            Some(mount_raw.to_ascii_lowercase())
        };
        if let Some(name) = mount_name
            && let Some(mount) = DLC_MOUNTS.read().unwrap().get(&name)
        {
            return match &mount.source {
                DlcMountSource::Disk(root) => ResolvedPath::Disk(root.join(rel)),
                DlcMountSource::Archive(_) => {
                    if is_static_binary_path(rel) {
                        ResolvedPath::DlcStaticBinary {
                            dlc: name,
                            path: format!("dlc://{}/{rel}", mount.name),
                        }
                    } else {
                        ResolvedPath::DlcPerroAssets {
                            dlc: name,
                            virtual_path: format!("res/{rel}"),
                        }
                    }
                }
            };
        }
        return ResolvedPath::Disk(PathBuf::from(path));
    }

    // Handle absolute filesystem paths
    let path_buf = PathBuf::from(path);
    if path_buf.is_absolute() {
        return ResolvedPath::Disk(path_buf);
    }

    match project_root_opt {
        Some(ProjectRoot::Disk { root, .. }) => {
            if let Some(stripped) = path.strip_prefix("res://") {
                let primary = root.join("res").join(stripped);
                if primary.exists() {
                    ResolvedPath::Disk(primary)
                } else {
                    // Fallback: if root already points at a res directory, avoid res/res.
                    ResolvedPath::Disk(root.join(stripped))
                }
            } else {
                ResolvedPath::Disk(root.join(path))
            }
        }
        Some(ProjectRoot::PerroAssets { .. }) => {
            if let Some(stripped) = path.strip_prefix("res://") {
                if is_static_binary_path(stripped) {
                    ResolvedPath::StaticBinary(format!("res://{}", stripped))
                } else {
                    ResolvedPath::PerroAssets(format!("res/{}", stripped))
                }
            } else {
                ResolvedPath::PerroAssets(path.to_string())
            }
        }
        None => ResolvedPath::Disk(PathBuf::from(path)),
    }
}

/// Load an asset fully into memory
pub fn load_asset(path: &str) -> io::Result<Vec<u8>> {
    match resolve_path(path) {
        ResolvedPath::Disk(pb) => fs::read(pb),
        ResolvedPath::PerroAssets(virtual_path) => {
            if let Some(archive) = PERRO_ASSETS_ARCHIVE.read().unwrap().as_ref() {
                archive.read_file(&virtual_path)
            } else {
                Err(io::Error::other("PerroAssets archive not loaded"))
            }
        }
        ResolvedPath::StaticBinary(path) => load_static_binary(&path),
        ResolvedPath::DlcStaticBinary { dlc, path } => load_dlc_static_binary(&dlc, &path),
        ResolvedPath::DlcPerroAssets { dlc, virtual_path } => {
            if let Some(archive) = DLC_ARCHIVES.read().unwrap().get(&dlc) {
                archive.read_file(&virtual_path)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("dlc archive mount not found: {dlc}"),
                ))
            }
        }
    }
}

/// Stream an asset (for large files)
pub fn stream_asset(path: &str) -> io::Result<Box<dyn ReadSeek>> {
    match resolve_path(path) {
        ResolvedPath::Disk(pb) => {
            let file = File::open(pb)?;
            Ok(Box::new(file))
        }
        ResolvedPath::PerroAssets(virtual_path) => {
            if let Some(archive) = PERRO_ASSETS_ARCHIVE.read().unwrap().as_ref() {
                let file: PerroAssetsFile = archive.stream_file(&virtual_path)?;
                Ok(Box::new(file))
            } else {
                Err(io::Error::other("PerroAssets archive not loaded"))
            }
        }
        ResolvedPath::StaticBinary(_) => Err(io::Error::other("Cannot stream static binary")),
        ResolvedPath::DlcStaticBinary { .. } => {
            Err(io::Error::other("Cannot stream static binary"))
        }
        ResolvedPath::DlcPerroAssets { dlc, virtual_path } => {
            if let Some(archive) = DLC_ARCHIVES.read().unwrap().get(&dlc) {
                let file: PerroAssetsFile = archive.stream_file(&virtual_path)?;
                Ok(Box::new(file))
            } else {
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("dlc archive mount not found: {dlc}"),
                ))
            }
        }
    }
}

/// Save an asset (disk only)
pub fn save_asset(path: &str, data: &[u8]) -> io::Result<()> {
    match resolve_path(path) {
        ResolvedPath::Disk(pb) => {
            if let Some(parent) = pb.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut file = File::create(pb)?;
            file.write_all(data)
        }
        ResolvedPath::PerroAssets(_)
        | ResolvedPath::StaticBinary(_)
        | ResolvedPath::DlcStaticBinary { .. }
        | ResolvedPath::DlcPerroAssets { .. } => {
            Err(io::Error::other("Cannot save to packed archive"))
        }
    }
}

fn load_dlc_static_binary(dlc: &str, path: &str) -> io::Result<Vec<u8>> {
    let lookup = DLC_STATIC_BINARY_LOOKUPS
        .read()
        .unwrap()
        .get(dlc)
        .copied()
        .ok_or_else(|| io::Error::other(format!("dlc static binary lookup not loaded: {dlc}")))?;
    let mut ptr = std::ptr::null();
    let mut len = 0usize;
    let found = unsafe { lookup(perro_ids::string_to_u64(path), &mut ptr, &mut len) };
    if !found || ptr.is_null() || len == 0 {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("dlc static binary not found: {path}"),
        ));
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    Ok(bytes.to_vec())
}

fn load_static_binary(path: &str) -> io::Result<Vec<u8>> {
    match PROJECT_ROOT.read().unwrap().as_ref() {
        Some(ProjectRoot::PerroAssets {
            static_resource_lookups,
            ..
        }) => load_static_resource_binary(*static_resource_lookups, path),
        _ => Err(io::Error::other("Static resource lookups not loaded")),
    }
}

fn load_static_resource_binary(lookups: StaticResourceLookups, path: &str) -> io::Result<Vec<u8>> {
    let hash = perro_ids::string_to_u64(path);
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();

    let bytes = match ext.as_str() {
        "png" | "jpg" | "jpeg" | "bmp" | "gif" | "ico" | "tga" | "webp" | "rgba" => {
            lookups.texture_lookup.map(|lookup| lookup(hash))
        }
        "glb" | "gltf" => lookups.mesh_lookup.map(|lookup| lookup(hash)),
        "pmesh" => {
            let mesh = lookups
                .mesh_lookup
                .map(|lookup| lookup(hash))
                .unwrap_or(b"");
            if mesh.is_empty() {
                lookups.collision_trimesh_lookup.map(|lookup| lookup(hash))
            } else {
                Some(mesh)
            }
        }
        "pskel" => lookups.skeleton_lookup.map(|lookup| lookup(hash)),
        "wgsl" => lookups.shader_lookup.map(|lookup| lookup(hash).as_bytes()),
        "mp3" | "wav" | "ogg" | "flac" | "mid" | "midi" | "sf2" => {
            lookups.audio_lookup.map(|lookup| lookup(hash))
        }
        _ => None,
    }
    .unwrap_or(b"");

    if bytes.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("static resource not found: {path}"),
        ));
    }
    Ok(bytes.to_vec())
}

fn is_static_binary_path(path: &str) -> bool {
    let Some(ext) = Path::new(path).extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "png"
            | "jpg"
            | "jpeg"
            | "bmp"
            | "gif"
            | "ico"
            | "tga"
            | "webp"
            | "rgba"
            | "glb"
            | "gltf"
            | "pmesh"
            | "pskel"
            | "wgsl"
            | "mp3"
            | "wav"
            | "ogg"
            | "flac"
            | "aac"
            | "m4a"
            | "mid"
            | "midi"
            | "sf2"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    static EMPTY_ARCHIVE: &[u8] = &[
        b'P', b'R', b'A', b'1', 1, 0, 0, 0, 0, 0, 0, 0, 20, 0, 0, 0, 0, 0, 0, 0,
    ];
    static TEST_LOCK: LazyLock<std::sync::Mutex<()>> = LazyLock::new(|| std::sync::Mutex::new(()));

    #[test]
    fn resolve_user_path_normalizes_game_name_spaces() {
        let _guard = TEST_LOCK.lock().unwrap();
        set_project_root(ProjectRoot::Disk {
            root: PathBuf::from("C:/tmp/perro-test-root"),
            name: "My Cool Game".to_string(),
        });
        let resolved = resolve_path("user://save.dat");
        let disk_path = match resolved {
            ResolvedPath::Disk(path) => path,
            _ => panic!("expected disk path"),
        };
        let as_text = disk_path.to_string_lossy();
        assert!(as_text.contains("My_Cool_Game"));
        assert!(!as_text.contains("My Cool Game"));
    }

    fn static_lookup(path_hash: u64) -> &'static [u8] {
        if path_hash == perro_ids::string_to_u64("res://textures/player.png") {
            b"static-ptex"
        } else if path_hash == perro_ids::string_to_u64("res://music/theme.mid") {
            b"MThd"
        } else if path_hash == perro_ids::string_to_u64("res://soundfonts/game.sf2") {
            b"RIFF"
        } else {
            b""
        }
    }

    unsafe extern "C" fn dlc_static_lookup(
        path_hash: u64,
        data_out: *mut *const u8,
        len_out: *mut usize,
    ) -> bool {
        if path_hash != perro_ids::string_to_u64("dlc://Expansion/textures/player.png") {
            return false;
        }
        if data_out.is_null() || len_out.is_null() {
            return false;
        }
        unsafe {
            *data_out = b"dlc-static-ptex".as_ptr();
            *len_out = b"dlc-static-ptex".len();
        }
        true
    }

    #[test]
    fn resolve_static_binary_path_in_perro_assets_mode() {
        let _guard = TEST_LOCK.lock().unwrap();
        set_project_root(ProjectRoot::PerroAssets {
            data: EMPTY_ARCHIVE,
            name: "Static Test".to_string(),
            static_resource_lookups: StaticResourceLookups {
                texture_lookup: Some(static_lookup),
                audio_lookup: Some(static_lookup),
                ..StaticResourceLookups::default()
            },
        });

        match resolve_path("res://textures/player.png") {
            ResolvedPath::StaticBinary(path) => assert_eq!(path, "res://textures/player.png"),
            other => panic!("expected static binary path, got {other:?}"),
        }
    }

    #[test]
    fn load_asset_reads_static_resource_lookup() {
        let _guard = TEST_LOCK.lock().unwrap();
        set_project_root(ProjectRoot::PerroAssets {
            data: EMPTY_ARCHIVE,
            name: "Static Test".to_string(),
            static_resource_lookups: StaticResourceLookups {
                texture_lookup: Some(static_lookup),
                audio_lookup: Some(static_lookup),
                ..StaticResourceLookups::default()
            },
        });

        assert_eq!(
            load_asset("res://textures/player.png").unwrap(),
            b"static-ptex"
        );
        assert_eq!(load_asset("res://music/theme.mid").unwrap(), b"MThd");
        assert_eq!(load_asset("res://soundfonts/game.sf2").unwrap(), b"RIFF");
        assert!(load_asset("res://textures/missing.png").is_err());
    }

    #[test]
    fn resolve_dev_res_path_stays_disk_even_for_static_ext() {
        let _guard = TEST_LOCK.lock().unwrap();
        let root =
            std::env::temp_dir().join(format!("perro_io_dev_static_ext_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("res").join("textures")).unwrap();
        fs::write(root.join("res").join("textures").join("player.png"), b"raw").unwrap();
        set_project_root(ProjectRoot::Disk {
            root: root.clone(),
            name: "Dev Test".to_string(),
        });

        match resolve_path("res://textures/player.png") {
            ResolvedPath::Disk(path) => assert_eq!(path, root.join("res/textures/player.png")),
            other => panic!("expected disk path, got {other:?}"),
        }
        assert_eq!(load_asset("res://textures/player.png").unwrap(), b"raw");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn load_asset_reads_dlc_static_binary_lookup() {
        let _guard = TEST_LOCK.lock().unwrap();
        let root =
            std::env::temp_dir().join(format!("perro_io_dlc_static_ext_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let archive = root.join("Expansion.dlc");
        fs::write(&archive, EMPTY_ARCHIVE).unwrap();

        clear_dlc_mounts();
        mount_dlc_archive("Expansion", &archive).unwrap();
        register_dlc_static_binary_lookup("Expansion", dlc_static_lookup);

        match resolve_path("dlc://Expansion/textures/player.png") {
            ResolvedPath::DlcStaticBinary { dlc, path } => {
                assert_eq!(dlc, "expansion");
                assert_eq!(path, "dlc://Expansion/textures/player.png");
            }
            other => panic!("expected dlc static binary path, got {other:?}"),
        }
        assert_eq!(
            load_asset("dlc://Expansion/textures/player.png").unwrap(),
            b"dlc-static-ptex"
        );

        clear_dlc_mounts();
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn dlc_self_context_restores_nested_and_after_panic() {
        let _guard = TEST_LOCK.lock().unwrap();
        let root =
            std::env::temp_dir().join(format!("perro_io_dlc_self_context_{}", std::process::id()));
        let base = root.join("base");
        let nested = root.join("nested");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&nested).unwrap();

        clear_dlc_mounts();
        mount_dlc_disk("Base", &base).unwrap();
        mount_dlc_disk("Nested", &nested).unwrap();

        {
            let _base_ctx = push_dlc_self_context(Some("Base"));
            match resolve_path("dlc://self/file.txt") {
                ResolvedPath::Disk(path) => assert_eq!(path, base.join("file.txt")),
                other => panic!("expected base disk path, got {other:?}"),
            }

            {
                let _nested_ctx = push_dlc_self_context(Some("Nested"));
                match resolve_path("dlc://self/file.txt") {
                    ResolvedPath::Disk(path) => assert_eq!(path, nested.join("file.txt")),
                    other => panic!("expected nested disk path, got {other:?}"),
                }
            }

            match resolve_path("dlc://self/file.txt") {
                ResolvedPath::Disk(path) => assert_eq!(path, base.join("file.txt")),
                other => panic!("expected restored base disk path, got {other:?}"),
            }

            let panic_result = std::panic::catch_unwind(|| {
                let _panic_ctx = push_dlc_self_context(Some("Nested"));
                match resolve_path("dlc://self/file.txt") {
                    ResolvedPath::Disk(path) => assert_eq!(path, nested.join("file.txt")),
                    other => panic!("expected nested disk path, got {other:?}"),
                }
                panic!("force unwind");
            });
            assert!(panic_result.is_err());

            match resolve_path("dlc://self/file.txt") {
                ResolvedPath::Disk(path) => assert_eq!(path, base.join("file.txt")),
                other => panic!("expected restored base disk path, got {other:?}"),
            }
        }

        match resolve_path("dlc://self/file.txt") {
            ResolvedPath::Disk(path) => assert_eq!(path, PathBuf::from("dlc://self/file.txt")),
            other => panic!("expected unresolved self path, got {other:?}"),
        }

        clear_dlc_mounts();
        let _ = fs::remove_dir_all(&root);
    }
}
