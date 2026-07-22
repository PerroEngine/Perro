use std::{
    cell::RefCell,
    collections::HashMap,
    fs::{self, File},
    io::{self, Read, Seek, Write},
    path::{Component, Path, PathBuf},
    sync::{Arc, LazyLock, RwLock},
};

#[cfg(not(target_arch = "wasm32"))]
use crate::data_local_dir;
use perro_assets::archive::{PerroAssetsArchive, PerroAssetsFile};

pub type StaticBytesLookup = fn(u64) -> &'static [u8];
pub type StaticShaderLookup = fn(u64) -> &'static str;
pub type DlcStaticBinaryLookup = unsafe extern "C" fn(u64, *mut *const u8, *mut usize) -> bool;

#[derive(Clone, Copy, Debug, Default)]
pub struct StaticResourceLookups {
    pub font_lookup: Option<StaticBytesLookup>,
    pub texture_lookup: Option<StaticBytesLookup>,
    pub mesh_lookup: Option<StaticBytesLookup>,
    pub collision_trimesh_lookup: Option<StaticBytesLookup>,
    pub navmesh_lookup: Option<StaticBytesLookup>,
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

struct ProjectAssetState {
    root: Option<ProjectRoot>,
    archive: Option<Arc<PerroAssetsArchive>>,
}

static PROJECT_ASSET_STATE: RwLock<ProjectAssetState> = RwLock::new(ProjectAssetState {
    root: None,
    archive: None,
});
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

pub fn validate_dlc_name(name: &str) -> io::Result<()> {
    let mut components = Path::new(name).components();
    let is_single_normal =
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
    if !is_single_normal || name.contains(['/', '\\', '"']) || name.chars().any(char::is_control) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid dlc name",
        ));
    }
    if is_reserved_dlc_name(name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "dlc name `self` is reserved",
        ));
    }
    Ok(())
}

pub fn get_project_root() -> ProjectRoot {
    PROJECT_ASSET_STATE
        .read()
        .expect("required value must be present")
        .root
        .clone()
        .expect("Project root not set")
}

/// Parse and atomically install a project root and its archive backing.
pub fn try_set_project_root(root: ProjectRoot) -> io::Result<()> {
    let archive = match &root {
        ProjectRoot::PerroAssets { data, .. } => {
            Some(Arc::new(PerroAssetsArchive::open_from_bytes(data)?))
        }
        ProjectRoot::Disk { .. } => None,
    };

    let mut state = PROJECT_ASSET_STATE
        .write()
        .expect("required value must be present");
    state.root = Some(root);
    state.archive = archive;
    Ok(())
}

/// Install a project root, panicking when an embedded archive is invalid.
///
/// Use [`try_set_project_root`] for untrusted or fallible startup paths.
pub fn set_project_root(root: ProjectRoot) {
    try_set_project_root(root).expect("Failed to open PerroAssets archive");
}

pub fn clear_dlc_mounts() {
    let mut mounts = DLC_MOUNTS.write().expect("required value must be present");
    let mut archives = DLC_ARCHIVES
        .write()
        .expect("required value must be present");
    let mut lookups = DLC_STATIC_BINARY_LOOKUPS
        .write()
        .expect("required value must be present");
    mounts.clear();
    archives.clear();
    lookups.clear();
}

pub fn mounted_dlc_names() -> Vec<String> {
    let mut out = DLC_MOUNTS
        .read()
        .expect("required value must be present")
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    out.sort();
    out
}

pub fn read_mounted_dlc_file(name: &str, virtual_path: &str) -> io::Result<Vec<u8>> {
    validate_asset_relative_path(virtual_path)?;
    let key = name.to_ascii_lowercase();
    let archive = DLC_ARCHIVES
        .read()
        .expect("required value must be present")
        .get(&key)
        .cloned();
    if let Some(archive) = archive {
        archive.read_file(virtual_path)
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("dlc archive mount not found: {name}"),
        ))
    }
}

pub fn mount_dlc_disk(name: &str, root: impl AsRef<Path>) -> io::Result<()> {
    validate_dlc_name(name)?;
    let root = root.as_ref().to_path_buf();
    if !root.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("dlc disk root not found: {}", root.display()),
        ));
    }
    replace_dlc_mount(
        name,
        DlcMount {
            name: name.to_string(),
            source: DlcMountSource::Disk(root),
        },
        None,
    );
    Ok(())
}

pub fn mount_dlc_archive(name: &str, archive_path: impl AsRef<Path>) -> io::Result<()> {
    validate_dlc_name(name)?;
    let archive_path = archive_path.as_ref().to_path_buf();
    let archive = Arc::new(PerroAssetsArchive::open_from_file(&archive_path)?);
    replace_dlc_mount(
        name,
        DlcMount {
            name: name.to_string(),
            source: DlcMountSource::Archive(archive_path),
        },
        Some(archive),
    );
    Ok(())
}

fn replace_dlc_mount(name: &str, mount: DlcMount, archive: Option<Arc<PerroAssetsArchive>>) {
    let key = name.to_ascii_lowercase();
    let mut mounts = DLC_MOUNTS.write().expect("required value must be present");
    let mut archives = DLC_ARCHIVES
        .write()
        .expect("required value must be present");
    let mut lookups = DLC_STATIC_BINARY_LOOKUPS
        .write()
        .expect("required value must be present");
    archives.remove(&key);
    lookups.remove(&key);
    if let Some(archive) = archive {
        archives.insert(key.clone(), archive);
    }
    mounts.insert(key, mount);
}

/// Register a DLC callback that returns borrowed binary asset bytes.
///
/// # Safety
/// When `lookup` returns `true`, it must initialize both output pointers. The
/// returned data pointer must be non-null, point to `len` initialized bytes in
/// one allocation, and remain valid until the bytes are copied immediately
/// after the callback returns. `len` must not exceed `isize::MAX`. The callback
/// must not unwind across the C ABI boundary.
pub unsafe fn register_dlc_static_binary_lookup(name: &str, lookup: DlcStaticBinaryLookup) {
    DLC_STATIC_BINARY_LOOKUPS
        .write()
        .expect("required value must be present")
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
    WebUserStorage(String),
    PerroAssets(String),
    StaticBinary(String),
    DlcStaticBinary { dlc: String, path: String },
    DlcPerroAssets { dlc: String, virtual_path: String },
}

fn normalize_user_app_name(name: &str) -> String {
    name.replace(' ', "_")
}

fn user_app_name(project_root_opt: &Option<ProjectRoot>) -> String {
    let app_name = project_root_opt
        .as_ref()
        .map(|root| match root {
            ProjectRoot::Disk { name, .. } => name.as_str(),
            ProjectRoot::PerroAssets { name, .. } => name.as_str(),
        })
        .expect("Project root not set");
    normalize_user_app_name(app_name)
}

#[cfg(any(test, target_arch = "wasm32"))]
fn user_storage_key(app_name: &str, relative_path: &str) -> String {
    format!("perro:user:{app_name}:data:{relative_path}")
}

pub fn validate_virtual_asset_path(path: &str) -> io::Result<()> {
    if let Some(stripped) = path.strip_prefix("user://") {
        return validate_asset_relative_path(stripped);
    }

    if let Some(stripped) = path.strip_prefix("res://") {
        return validate_asset_relative_path(stripped);
    }

    if let Some(rest) = path.strip_prefix("dlc://") {
        let (mount_raw, rel_raw) = rest.split_once('/').unwrap_or((rest, ""));
        validate_dlc_mount_name(mount_raw)?;
        return validate_asset_relative_path(rel_raw.trim_start_matches('/'));
    }

    let path_buf = Path::new(path);
    if path_buf.is_absolute() {
        return Ok(());
    }
    validate_asset_relative_path(path)
}

pub fn validate_asset_relative_path(path: &str) -> io::Result<()> {
    if path.contains('\\') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "asset paths must use `/` separators",
        ));
    }
    if Path::new(path).is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "asset path must be relative",
        ));
    }
    for component in Path::new(path).components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "asset path escapes root",
                ));
            }
        }
    }
    Ok(())
}

fn validate_dlc_mount_name(name: &str) -> io::Result<()> {
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\']) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid dlc mount name",
        ));
    }
    Ok(())
}

/// Resolve virtual path (res://foo/bar.png or user://save.dat) to actual location
pub fn resolve_path(path: &str) -> ResolvedPath {
    if validate_virtual_asset_path(path).is_err() {
        return ResolvedPath::Disk(PathBuf::from(path));
    }

    let project_root_opt = PROJECT_ASSET_STATE
        .read()
        .expect("required value must be present")
        .root
        .clone();

    // Handle user:// paths (always disk)
    if let Some(stripped) = path.strip_prefix("user://") {
        let app_name = user_app_name(&project_root_opt);

        #[cfg(target_arch = "wasm32")]
        {
            return ResolvedPath::WebUserStorage(user_storage_key(&app_name, stripped));
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let base = data_local_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join(app_name)
                .join("data");
            return ResolvedPath::Disk(base.join(stripped));
        }
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
            && let Some(mount) = DLC_MOUNTS
                .read()
                .expect("required value must be present")
                .get(&name)
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
    validate_virtual_asset_path(path)?;
    match resolve_path(path) {
        ResolvedPath::Disk(pb) => fs::read(pb),
        ResolvedPath::WebUserStorage(key) => load_web_user_asset(&key),
        ResolvedPath::PerroAssets(virtual_path) => {
            let archive = PROJECT_ASSET_STATE
                .read()
                .expect("required value must be present")
                .archive
                .clone();
            if let Some(archive) = archive {
                archive.read_file(&virtual_path)
            } else {
                Err(io::Error::other("PerroAssets archive not loaded"))
            }
        }
        ResolvedPath::StaticBinary(path) => load_static_binary(&path),
        ResolvedPath::DlcStaticBinary { dlc, path } => load_dlc_static_binary(&dlc, &path),
        ResolvedPath::DlcPerroAssets { dlc, virtual_path } => {
            let archive = DLC_ARCHIVES
                .read()
                .expect("required value must be present")
                .get(&dlc)
                .cloned();
            if let Some(archive) = archive {
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
    validate_virtual_asset_path(path)?;
    match resolve_path(path) {
        ResolvedPath::Disk(pb) => {
            let file = File::open(pb)?;
            Ok(Box::new(file))
        }
        ResolvedPath::WebUserStorage(key) => {
            let bytes = load_web_user_asset(&key)?;
            Ok(Box::new(std::io::Cursor::new(bytes)))
        }
        ResolvedPath::PerroAssets(virtual_path) => {
            let archive = PROJECT_ASSET_STATE
                .read()
                .expect("required value must be present")
                .archive
                .clone();
            if let Some(archive) = archive {
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
            let archive = DLC_ARCHIVES
                .read()
                .expect("required value must be present")
                .get(&dlc)
                .cloned();
            if let Some(archive) = archive {
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
    validate_virtual_asset_path(path)?;
    match resolve_path(path) {
        ResolvedPath::Disk(pb) => {
            if let Some(parent) = pb.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut file = File::create(pb)?;
            file.write_all(data)
        }
        ResolvedPath::WebUserStorage(key) => save_web_user_asset(&key, data),
        ResolvedPath::PerroAssets(_)
        | ResolvedPath::StaticBinary(_)
        | ResolvedPath::DlcStaticBinary { .. }
        | ResolvedPath::DlcPerroAssets { .. } => {
            Err(io::Error::other("Cannot save to packed archive"))
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn load_web_user_asset(key: &str) -> io::Result<Vec<u8>> {
    perro_web::storage::load_local_bytes(key)?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("web user data !found: {key}"),
        )
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn load_web_user_asset(key: &str) -> io::Result<Vec<u8>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        format!("web user data unsupported: {key}"),
    ))
}

#[cfg(target_arch = "wasm32")]
fn save_web_user_asset(key: &str, data: &[u8]) -> io::Result<()> {
    perro_web::storage::save_local_bytes(key, data)
}

#[cfg(not(target_arch = "wasm32"))]
fn save_web_user_asset(key: &str, _: &[u8]) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        format!("web user data unsupported: {key}"),
    ))
}

fn load_dlc_static_binary(dlc: &str, path: &str) -> io::Result<Vec<u8>> {
    let lookup = DLC_STATIC_BINARY_LOOKUPS
        .read()
        .expect("required value must be present")
        .get(dlc)
        .copied()
        .ok_or_else(|| io::Error::other(format!("dlc static binary lookup not loaded: {dlc}")))?;
    let mut ptr = std::ptr::null();
    let mut len = 0usize;
    // SAFETY: Lookup fn comes from registered static DLC code. It writes a pointer/len
    // pair to immutable static bytes for the requested path hash.
    let found = unsafe { lookup(perro_ids::string_to_u64(path), &mut ptr, &mut len) };
    if !found || ptr.is_null() || len == 0 {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("dlc static binary not found: {path}"),
        ));
    }
    if len > isize::MAX as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("dlc static binary too large: {len} bytes"),
        ));
    }
    // SAFETY: Successful lookup guarantees ptr is non-null and len bytes remain valid
    // for the duration of this call; copy immediately into an owned Vec.
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    if Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| matches!(ext.to_ascii_lowercase().as_str(), "ttf" | "otf" | "ttc"))
    {
        return decode_static_font(bytes);
    }
    Ok(bytes.to_vec())
}

pub fn decode_static_font(bytes: &[u8]) -> io::Result<Vec<u8>> {
    if !bytes.starts_with(b"PFONT") {
        return Ok(bytes.to_vec());
    }
    let raw_len = u32::from_le_bytes(
        bytes
            .get(5..9)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "short PFONT"))?
            .try_into()
            .expect("required value must be present"),
    ) as usize;
    crate::decompress_zlib_limited(&bytes[9..], raw_len)
}

fn load_static_binary(path: &str) -> io::Result<Vec<u8>> {
    match PROJECT_ASSET_STATE
        .read()
        .expect("required value must be present")
        .root
        .as_ref()
    {
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
        "ttf" | "otf" | "ttc" => lookups.font_lookup.map(|lookup| lookup(hash)),
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
        "pnav" => lookups.navmesh_lookup.map(|lookup| lookup(hash)),
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
    if matches!(ext.as_str(), "ttf" | "otf" | "ttc") {
        return decode_static_font(bytes);
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
            | "pnav"
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
            | "ttf"
            | "otf"
            | "ttc"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_font_decode_handles_raw_and_pfont() {
        let raw = b"font bytes font bytes font bytes";
        assert_eq!(
            decode_static_font(raw).expect("required value must be present"),
            raw
        );
        let compressed = crate::compress_zlib_best(raw).expect("required value must be present");
        let mut packed = b"PFONT".to_vec();
        packed.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        packed.extend_from_slice(&compressed);
        assert_eq!(
            decode_static_font(&packed).expect("required value must be present"),
            raw
        );
    }

    static EMPTY_ARCHIVE: &[u8] = &[
        b'P', b'R', b'A', b'1', 1, 0, 0, 0, 0, 0, 0, 0, 20, 0, 0, 0, 0, 0, 0, 0,
    ];
    static TEST_LOCK: LazyLock<std::sync::Mutex<()>> = LazyLock::new(|| std::sync::Mutex::new(()));

    fn test_archive(label: &str, contents: &[u8]) -> &'static [u8] {
        let root = std::env::temp_dir().join(format!(
            "perro_io_project_root_{label}_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("required value must be present");
        let source = root.join("config.txt");
        let archive = root.join("project.perro");
        fs::write(&source, contents).expect("required value must be present");
        perro_assets::packer::build_perro_archive_from_entries(
            &archive,
            &[("res/config.txt".to_string(), source)],
        )
        .expect("required value must be present");
        let bytes = fs::read(&archive)
            .expect("required value must be present")
            .into_boxed_slice();
        let _ = fs::remove_dir_all(root);
        Box::leak(bytes)
    }

    #[test]
    fn invalid_archive_keeps_prior_root_and_backing() {
        let _guard = TEST_LOCK.lock().expect("required value must be present");
        let valid = test_archive("atomic", b"old backing");
        try_set_project_root(ProjectRoot::PerroAssets {
            data: valid,
            name: "Old Root".to_string(),
            static_resource_lookups: StaticResourceLookups::default(),
        })
        .expect("required value must be present");
        assert_eq!(
            load_asset("res://config.txt").expect("required value must be present"),
            b"old backing"
        );

        let err = try_set_project_root(ProjectRoot::PerroAssets {
            data: b"invalid archive",
            name: "Broken Root".to_string(),
            static_resource_lookups: StaticResourceLookups::default(),
        })
        .expect_err("operation must fail in this test");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(matches!(
            get_project_root(),
            ProjectRoot::PerroAssets { name, .. } if name == "Old Root"
        ));
        assert_eq!(
            load_asset("res://config.txt").expect("required value must be present"),
            b"old backing"
        );
    }

    #[test]
    fn disk_root_switch_clears_archive_backing() {
        let _guard = TEST_LOCK.lock().expect("required value must be present");
        let packed = test_archive("disk-switch", b"packed backing");
        try_set_project_root(ProjectRoot::PerroAssets {
            data: packed,
            name: "Packed Root".to_string(),
            static_resource_lookups: StaticResourceLookups::default(),
        })
        .expect("required value must be present");
        assert_eq!(
            load_asset("res://config.txt").expect("required value must be present"),
            b"packed backing"
        );

        let disk =
            std::env::temp_dir().join(format!("perro_io_project_disk_{}", std::process::id()));
        let _ = fs::remove_dir_all(&disk);
        fs::create_dir_all(disk.join("res")).expect("required value must be present");
        fs::write(disk.join("res/config.txt"), b"disk backing")
            .expect("required value must be present");
        try_set_project_root(ProjectRoot::Disk {
            root: disk.clone(),
            name: "Disk Root".to_string(),
        })
        .expect("required value must be present");

        assert!(
            PROJECT_ASSET_STATE
                .read()
                .expect("required value must be present")
                .archive
                .is_none()
        );
        assert_eq!(
            load_asset("res://config.txt").expect("required value must be present"),
            b"disk backing"
        );
        let _ = fs::remove_dir_all(disk);
    }

    #[test]
    fn resolve_user_path_normalizes_game_name_spaces() {
        let _guard = TEST_LOCK.lock().expect("required value must be present");
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

    #[test]
    fn user_storage_key_uses_app_name_prefix() {
        assert_eq!(
            super::user_storage_key("My_Cool_Game", "save/slot1.dat"),
            "perro:user:My_Cool_Game:data:save/slot1.dat"
        );
    }

    #[test]
    fn virtual_asset_paths_reject_root_escape() {
        assert!(validate_virtual_asset_path("user://../save.dat").is_err());
        assert!(validate_virtual_asset_path("res://../secret.txt").is_err());
        assert!(validate_virtual_asset_path("dlc://Expansion/../secret.txt").is_err());
        assert!(validate_virtual_asset_path("user://save/slot1.dat").is_ok());
        assert!(validate_virtual_asset_path("dlc://Expansion/scenes/main.scn").is_ok());
    }

    #[test]
    fn dlc_names_reject_path_components_and_manifest_control_chars() {
        for name in [
            "",
            ".",
            "..",
            "../escape",
            "..\\escape",
            "self",
            "SELF",
            "bad\"name",
            "bad\nname",
        ] {
            assert!(validate_dlc_name(name).is_err(), "accepted `{name:?}`");
        }
        for name in ["Expansion", "expansion-pack", "my expansion", "v1.2"] {
            assert!(validate_dlc_name(name).is_ok(), "rejected `{name}`");
        }
    }

    fn static_lookup(path_hash: u64) -> &'static [u8] {
        if path_hash == perro_ids::string_to_u64("res://textures/player.png") {
            b"static-ptex"
        } else if path_hash == perro_ids::string_to_u64("res://nav/level.pnav") {
            b"pnav 1\nv 0 0 0\nv 1 0 0\nv 0 0 1\ntri 0 1 2 1\n"
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
        // SAFETY: Test lookup validates out pointers and returns static byte storage.
        unsafe {
            *data_out = b"dlc-static-ptex".as_ptr();
            *len_out = b"dlc-static-ptex".len();
        }
        true
    }

    unsafe extern "C" fn oversized_dlc_static_lookup(
        _path_hash: u64,
        data_out: *mut *const u8,
        len_out: *mut usize,
    ) -> bool {
        if data_out.is_null() || len_out.is_null() {
            return false;
        }
        // SAFETY: Test callback receives writable output pointers from the loader.
        unsafe {
            *data_out = std::ptr::NonNull::<u8>::dangling().as_ptr();
            *len_out = isize::MAX as usize + 1;
        }
        true
    }

    #[test]
    fn resolve_static_binary_path_in_perro_assets_mode() {
        let _guard = TEST_LOCK.lock().expect("required value must be present");
        set_project_root(ProjectRoot::PerroAssets {
            data: EMPTY_ARCHIVE,
            name: "Static Test".to_string(),
            static_resource_lookups: StaticResourceLookups {
                texture_lookup: Some(static_lookup),
                navmesh_lookup: Some(static_lookup),
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
        let _guard = TEST_LOCK.lock().expect("required value must be present");
        set_project_root(ProjectRoot::PerroAssets {
            data: EMPTY_ARCHIVE,
            name: "Static Test".to_string(),
            static_resource_lookups: StaticResourceLookups {
                texture_lookup: Some(static_lookup),
                navmesh_lookup: Some(static_lookup),
                audio_lookup: Some(static_lookup),
                ..StaticResourceLookups::default()
            },
        });

        assert_eq!(
            load_asset("res://textures/player.png").expect("required value must be present"),
            b"static-ptex"
        );
        assert_eq!(
            load_asset("res://music/theme.mid").expect("required value must be present"),
            b"MThd"
        );
        assert_eq!(
            load_asset("res://nav/level.pnav").expect("required value must be present"),
            b"pnav 1\nv 0 0 0\nv 1 0 0\nv 0 0 1\ntri 0 1 2 1\n"
        );
        assert_eq!(
            load_asset("res://soundfonts/game.sf2").expect("required value must be present"),
            b"RIFF"
        );
        assert!(load_asset("res://textures/missing.png").is_err());
    }

    #[test]
    fn resolve_dev_res_path_stays_disk_even_for_static_ext() {
        let _guard = TEST_LOCK.lock().expect("required value must be present");
        let root =
            std::env::temp_dir().join(format!("perro_io_dev_static_ext_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("res").join("textures"))
            .expect("required value must be present");
        fs::write(root.join("res").join("textures").join("player.png"), b"raw")
            .expect("required value must be present");
        set_project_root(ProjectRoot::Disk {
            root: root.clone(),
            name: "Dev Test".to_string(),
        });

        match resolve_path("res://textures/player.png") {
            ResolvedPath::Disk(path) => assert_eq!(path, root.join("res/textures/player.png")),
            other => panic!("expected disk path, got {other:?}"),
        }
        assert_eq!(
            load_asset("res://textures/player.png").expect("required value must be present"),
            b"raw"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn load_asset_reads_dlc_static_binary_lookup() {
        let _guard = TEST_LOCK.lock().expect("required value must be present");
        let root =
            std::env::temp_dir().join(format!("perro_io_dlc_static_ext_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("required value must be present");
        let archive = root.join("Expansion.dlc");
        fs::write(&archive, EMPTY_ARCHIVE).expect("required value must be present");

        clear_dlc_mounts();
        mount_dlc_archive("Expansion", &archive).expect("required value must be present");
        // SAFETY: Test callback returns static bytes and follows the registration contract.
        unsafe { register_dlc_static_binary_lookup("Expansion", dlc_static_lookup) };

        match resolve_path("dlc://Expansion/textures/player.png") {
            ResolvedPath::DlcStaticBinary { dlc, path } => {
                assert_eq!(dlc, "expansion");
                assert_eq!(path, "dlc://Expansion/textures/player.png");
            }
            other => panic!("expected dlc static binary path, got {other:?}"),
        }
        assert_eq!(
            load_asset("dlc://Expansion/textures/player.png")
                .expect("required value must be present"),
            b"dlc-static-ptex"
        );

        clear_dlc_mounts();
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn remount_replaces_archive_and_static_lookup_backing() {
        let _guard = TEST_LOCK.lock().expect("required value must be present");
        let root =
            std::env::temp_dir().join(format!("perro_io_dlc_remount_{}", std::process::id()));
        let disk = root.join("disk");
        let archive = root.join("Expansion.dlc");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&disk).expect("required value must be present");
        fs::write(&archive, EMPTY_ARCHIVE).expect("required value must be present");

        clear_dlc_mounts();
        mount_dlc_archive("Expansion", &archive).expect("required value must be present");
        // SAFETY: Test callback returns immutable static bytes.
        unsafe { register_dlc_static_binary_lookup("Expansion", dlc_static_lookup) };
        mount_dlc_disk("EXPANSION", &disk).expect("required value must be present");

        assert!(
            !DLC_ARCHIVES
                .read()
                .expect("required value must be present")
                .contains_key("expansion")
        );
        assert!(
            !DLC_STATIC_BINARY_LOOKUPS
                .read()
                .expect("required value must be present")
                .contains_key("expansion")
        );
        assert!(matches!(
            &DLC_MOUNTS
                .read()
                .expect("required value must be present")
                .get("expansion")
                .expect("required value must be present")
                .source,
            DlcMountSource::Disk(_)
        ));

        // SAFETY: Test callback returns immutable static bytes.
        unsafe { register_dlc_static_binary_lookup("Expansion", dlc_static_lookup) };
        mount_dlc_archive("expansion", &archive).expect("required value must be present");
        assert!(
            DLC_ARCHIVES
                .read()
                .expect("required value must be present")
                .contains_key("expansion")
        );
        assert!(
            !DLC_STATIC_BINARY_LOOKUPS
                .read()
                .expect("required value must be present")
                .contains_key("expansion")
        );
        assert!(matches!(
            &DLC_MOUNTS
                .read()
                .expect("required value must be present")
                .get("expansion")
                .expect("required value must be present")
                .source,
            DlcMountSource::Archive(_)
        ));

        clear_dlc_mounts();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn load_asset_rejects_oversized_dlc_static_binary() {
        let _guard = TEST_LOCK.lock().expect("required value must be present");
        clear_dlc_mounts();
        // SAFETY: Test callback initializes both outputs; the loader rejects its
        // oversized length before constructing a slice from the dangling pointer.
        unsafe {
            register_dlc_static_binary_lookup("Oversized", oversized_dlc_static_lookup);
        }

        let err = load_dlc_static_binary("oversized", "dlc://Oversized/huge.bin")
            .expect_err("oversized lookup must fail");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        clear_dlc_mounts();
    }

    #[test]
    fn dlc_self_context_restores_nested_and_after_panic() {
        let _guard = TEST_LOCK.lock().expect("required value must be present");
        let root =
            std::env::temp_dir().join(format!("perro_io_dlc_self_context_{}", std::process::id()));
        let base = root.join("base");
        let nested = root.join("nested");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&base).expect("required value must be present");
        fs::create_dir_all(&nested).expect("required value must be present");

        clear_dlc_mounts();
        mount_dlc_disk("Base", &base).expect("required value must be present");
        mount_dlc_disk("Nested", &nested).expect("required value must be present");

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
