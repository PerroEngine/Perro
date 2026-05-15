mod animation_trees;
mod animations;
mod audios;
mod collision_trimeshes;
mod csvs;
mod error;
mod localizations;
mod materials;
mod meshes;
mod particles;
mod scenes;
mod shaders;
mod skeletons;
mod textures;
mod tilesets;
mod uistyles;

pub use animation_trees::generate_static_animation_trees;
pub use animations::generate_static_animations;
pub use audios::generate_static_audios;
pub use collision_trimeshes::generate_static_collision_trimeshes;
pub use csvs::generate_static_csvs;
pub use error::StaticPipelineError;
pub use localizations::generate_empty_localizations;
pub use localizations::generate_static_localizations;
pub use materials::generate_static_materials;
pub use meshes::generate_static_meshes;
pub use particles::generate_static_particles;
pub use scenes::generate_static_scenes;
pub use shaders::generate_static_shaders;
pub use skeletons::generate_static_skeletons;
pub use textures::generate_static_textures;
pub use tilesets::generate_static_tilesets;
pub use uistyles::generate_static_ui_styles;

use std::{
    collections::HashMap,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    sync::{OnceLock, RwLock},
};

const PERRO_DIR: &str = ".perro";
const PROJECT_DIR: &str = "project";
const SRC_DIR: &str = "src";
const STATIC_DIR: &str = "static";
const EMBEDDED_DIR: &str = "embedded";
const RES_DIR: &str = "res";

#[derive(Clone, Debug)]
pub struct StaticPipelineOverrides {
    pub res_dir: PathBuf,
    pub static_dir: PathBuf,
    pub embedded_dir: PathBuf,
    pub asset_prefix: String,
}

fn overrides_cell() -> &'static RwLock<Option<StaticPipelineOverrides>> {
    static CELL: OnceLock<RwLock<Option<StaticPipelineOverrides>>> = OnceLock::new();
    CELL.get_or_init(|| RwLock::new(None))
}

fn current_overrides() -> Option<StaticPipelineOverrides> {
    overrides_cell().read().ok().and_then(|v| v.clone())
}

pub fn set_static_pipeline_overrides(overrides: Option<StaticPipelineOverrides>) {
    if let Ok(mut slot) = overrides_cell().write() {
        *slot = overrides;
    }
}

pub(crate) fn static_dir(project_root: &Path) -> PathBuf {
    if let Some(overrides) = current_overrides() {
        return overrides.static_dir;
    }
    project_root
        .join(PERRO_DIR)
        .join(PROJECT_DIR)
        .join(SRC_DIR)
        .join(STATIC_DIR)
}

pub(crate) fn embedded_dir(project_root: &Path) -> PathBuf {
    if let Some(overrides) = current_overrides() {
        return overrides.embedded_dir;
    }
    project_root
        .join(PERRO_DIR)
        .join(PROJECT_DIR)
        .join(EMBEDDED_DIR)
}

pub(crate) fn res_dir(project_root: &Path) -> PathBuf {
    if let Some(overrides) = current_overrides() {
        return overrides.res_dir;
    }
    project_root.join(RES_DIR)
}

pub(crate) fn asset_prefix() -> String {
    current_overrides()
        .map(|overrides| overrides.asset_prefix)
        .unwrap_or_else(|| "res://".to_string())
}

pub(crate) fn is_asset_uri(path: &str) -> bool {
    path.starts_with(&asset_prefix())
}

pub(crate) fn asset_uri(rel: &str) -> String {
    format!("{}{}", asset_prefix(), rel.replace('\\', "/"))
}

pub(crate) fn strip_asset_prefix(path: &str) -> Option<String> {
    path.strip_prefix(&asset_prefix()).map(str::to_string)
}

pub(crate) fn ensure_unique_hashes<'a, I>(kind: &str, paths: I) -> Result<(), StaticPipelineError>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut by_hash = HashMap::<u64, &'a str>::new();
    for path in paths {
        let hash = perro_ids::string_to_u64(path);
        if let Some(prev) = by_hash.insert(hash, path) {
            return Err(StaticPipelineError::SceneParse(format!(
                "{kind} hash collision: `{prev}` + `{path}` => {hash}"
            )));
        }
    }
    Ok(())
}

pub(crate) fn write_hash_const(out: &mut String, name: &str, value: &str) {
    let _ = writeln!(
        out,
        "const {name}: u64 = perro_ids::hash_str!(\"{}\");",
        escape_rust_str(value)
    );
}

pub(crate) fn escape_rust_str(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

pub fn write_static_mod_rs(project_root: &Path) -> Result<(), StaticPipelineError> {
    let static_dir = static_dir(project_root);
    fs::create_dir_all(&static_dir)?;
    fs::write(
        static_dir.join("mod.rs"),
        "#![allow(unused_imports)]\n\npub mod scenes;\npub mod materials;\npub mod ui_styles;\npub mod tilesets;\npub mod particles;\npub mod animations;\npub mod animation_trees;\npub mod meshes;\npub mod collision_trimeshes;\npub mod skeletons;\npub mod textures;\npub mod shaders;\npub mod audios;\npub mod csvs;\npub mod localizations;\n",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use perro_animation::{AnimationClip, AnimationTreeAsset};
    use perro_render_bridge::{
        Material3D, Mesh3D, MeshSurfaceRange, ParticleProfile3D, RuntimeMeshVertex,
        StandardMaterial3D, TileSet2D, decode_tileset_2d_binary,
    };
    use perro_scene::{Scene, SceneKey, SceneNodeData, SceneNodeEntry};
    use perro_structs::Color;
    use perro_ui::UiStyle;
    use std::borrow::Cow;
    use std::hint::black_box;
    use std::{
        sync::OnceLock,
        time::{Duration, Instant},
    };

    const ITEM_COUNT: usize = 16;
    const ITERATIONS: usize = 20_000;

    #[test]
    #[ignore = "bench probe; run with --release --ignored --nocapture"]
    fn static_asset_lookup_and_parse_bench() {
        if cfg!(debug_assertions) {
            eprintln!("warn run this with --release for useful numbers");
        }

        let hashes = lookup_hashes();
        let samples = [
            sample_ref("scene", hashes, lookup_scene, parse_scene),
            sample_ref("material", hashes, lookup_material, parse_material),
            sample_ref("ui_style", hashes, lookup_ui_style, parse_ui_style),
            sample_ref("particle", hashes, lookup_particle, parse_particle),
            sample_ref("animation", hashes, lookup_animation, parse_animation),
            sample_ref(
                "animation_tree",
                hashes,
                lookup_animation_tree,
                parse_animation_tree,
            ),
            sample_ref("localization", hashes, lookup_localized_string, parse_str),
            sample_bytes("tileset_ptset", hashes, lookup_tileset, parse_tileset),
            sample_bytes("mesh_pmesh", hashes, lookup_mesh, parse_mesh),
            sample_bytes(
                "collision_pmesh",
                hashes,
                lookup_collision_trimesh,
                parse_collision_trimesh,
            ),
            sample_bytes("skeleton_pskel", hashes, lookup_skeleton, parse_skeleton),
            sample_bytes("texture_ptex", hashes, lookup_texture, parse_texture),
            sample_ref("shader_wgsl", hashes, lookup_shader, parse_str),
            sample_bytes("audio_pawdio", hashes, lookup_audio, parse_audio),
        ];

        eprintln!("static_asset_lookup_and_parse_bench items={ITEM_COUNT} iters={ITERATIONS}");
        for sample in samples {
            let n = (ITEM_COUNT * ITERATIONS) as f64;
            eprintln!(
                "{:<18} lookup {:>8.3} ns/item parse {:>8.3} ns/item",
                sample.name,
                sample.lookup.as_secs_f64() * 1_000_000_000.0 / n,
                sample.parse.as_secs_f64() * 1_000_000_000.0 / n
            );
        }
    }

    struct BenchSample {
        name: &'static str,
        lookup: Duration,
        parse: Duration,
    }

    fn sample_ref<T: 'static + ?Sized>(
        name: &'static str,
        hashes: &[u64; ITEM_COUNT],
        lookup: fn(u64) -> &'static T,
        parse: fn(&'static T) -> usize,
    ) -> BenchSample {
        let start = Instant::now();
        let mut refs = Vec::with_capacity(ITEM_COUNT * ITERATIONS);
        for _ in 0..ITERATIONS {
            for hash in hashes {
                refs.push(black_box(lookup(black_box(*hash))));
            }
        }
        let lookup_elapsed = start.elapsed();

        let start = Instant::now();
        let mut total = 0usize;
        for item in refs {
            total = total.wrapping_add(black_box(parse(black_box(item))));
        }
        black_box(total);
        BenchSample {
            name,
            lookup: lookup_elapsed,
            parse: start.elapsed(),
        }
    }

    fn sample_bytes(
        name: &'static str,
        hashes: &[u64; ITEM_COUNT],
        lookup: fn(u64) -> &'static [u8],
        parse: fn(&[u8]) -> usize,
    ) -> BenchSample {
        let start = Instant::now();
        let mut refs = Vec::with_capacity(ITEM_COUNT * ITERATIONS);
        for _ in 0..ITERATIONS {
            for hash in hashes {
                refs.push(black_box(lookup(black_box(*hash))));
            }
        }
        let lookup_elapsed = start.elapsed();

        let start = Instant::now();
        let mut total = 0usize;
        for item in refs {
            total = total.wrapping_add(black_box(parse(black_box(item))));
        }
        black_box(total);
        BenchSample {
            name,
            lookup: lookup_elapsed,
            parse: start.elapsed(),
        }
    }

    fn lookup_hashes() -> &'static [u64; ITEM_COUNT] {
        static HASHES: OnceLock<[u64; ITEM_COUNT]> = OnceLock::new();
        HASHES.get_or_init(|| {
            let mut out = [0u64; ITEM_COUNT];
            for (i, slot) in out.iter_mut().enumerate() {
                *slot = perro_ids::string_to_u64(&format!("res://bench/{i}"));
            }
            out
        })
    }

    static SCENE_NODES: &[SceneNodeEntry] = &[SceneNodeEntry {
        data: SceneNodeData {
            ty: Cow::Borrowed("Sprite2D"),
            fields: Cow::Borrowed(&[]),
            base: None,
        },
        has_data_override: false,
        key: SceneKey::new(0),
        name: Some(Cow::Borrowed("Sprite")),
        tags: Cow::Borrowed(&[]),
        children: Cow::Borrowed(&[]),
        parent: None,
        script: None,
        clear_script: false,
        root_of: None,
        script_vars: Cow::Borrowed(&[]),
    }];
    static SCENE_NAMES: &[Cow<'static, str>] = &[Cow::Borrowed("Sprite")];
    static SCENE_ITEM: Scene = Scene {
        nodes: Cow::Borrowed(SCENE_NODES),
        root: Some(SceneKey::new(0)),
        key_names: Cow::Borrowed(SCENE_NAMES),
    };
    static MATERIAL_ITEM: Material3D = Material3D::Standard(StandardMaterial3D::const_default());
    static UI_STYLE_ITEM: UiStyle = UiStyle::panel();
    static PARTICLE_ITEM: ParticleProfile3D = ParticleProfile3D {
        path: perro_render_bridge::ParticlePath3D::None,
        expr_x_ops: None,
        expr_y_ops: None,
        expr_z_ops: None,
        lifetime_min: 0.6,
        lifetime_max: 1.4,
        speed_min: 1.0,
        speed_max: 3.0,
        spread_radians: core::f32::consts::FRAC_PI_3,
        size: 6.0,
        size_min: 0.65,
        size_max: 1.35,
        force: [0.0, 0.0, 0.0],
        color_start: Color::WHITE,
        color_end: Color::new(1.0, 0.4, 0.1, 0.0),
        emissive: [0.0, 0.0, 0.0],
        spin_angular_velocity: 0.0,
    };
    static ANIMATION_ITEM: AnimationClip = AnimationClip {
        name: Cow::Borrowed("Bench"),
        fps: 24.0,
        total_frames: 12,
        objects: Cow::Borrowed(&[]),
        object_tracks: Cow::Borrowed(&[]),
        frame_events: Cow::Borrowed(&[]),
    };
    static ANIMATION_TREE_ITEM: AnimationTreeAsset = AnimationTreeAsset {
        name: Cow::Borrowed("BenchTree"),
        slots: Cow::Borrowed(&[]),
        nodes: Cow::Borrowed(&[]),
        output: Cow::Borrowed(""),
    };
    static LOCALIZED_ITEM: &str = "Bench text";
    static SHADER_ITEM: &str =
        "@fragment\nfn fs_main() -> @location(0) vec4<f32> { return vec4<f32>(1.0); }\n";
    static PTEX_ITEM: &[u8] = &[
        b'P', b'T', b'E', b'X', 2, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 128, 4, 0, 0, 0, 255,
        255, 255, 255,
    ];
    static PAWDIO_ITEM: &[u8] = &[
        b'P', b'A', b'W', b'D', b'I', b'O', 2, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, b't', b'e', b's',
        b't',
    ];
    static PSKEL_ITEM: &[u8] = &[
        b'P', b'S', b'K', b'E', b'L', 4, 0, 0, 0, 0, 0, 0, 128, 0, 0, 0, 0,
    ];

    fn lookup_scene(hash: u64) -> &'static Scene {
        match_bench_hash(hash, &SCENE_ITEM, &SCENE_ITEM)
    }

    fn lookup_material(hash: u64) -> &'static Material3D {
        match_bench_hash(hash, &MATERIAL_ITEM, &MATERIAL_ITEM)
    }

    fn lookup_ui_style(hash: u64) -> &'static UiStyle {
        match_bench_hash(hash, &UI_STYLE_ITEM, &UI_STYLE_ITEM)
    }

    fn lookup_particle(hash: u64) -> &'static ParticleProfile3D {
        match_bench_hash(hash, &PARTICLE_ITEM, &PARTICLE_ITEM)
    }

    fn lookup_animation(hash: u64) -> &'static AnimationClip {
        match_bench_hash(hash, &ANIMATION_ITEM, &ANIMATION_ITEM)
    }

    fn lookup_animation_tree(hash: u64) -> &'static AnimationTreeAsset {
        match_bench_hash(hash, &ANIMATION_TREE_ITEM, &ANIMATION_TREE_ITEM)
    }

    fn lookup_localized_string(hash: u64) -> &'static str {
        match_bench_hash(hash, LOCALIZED_ITEM, "")
    }

    fn lookup_shader(hash: u64) -> &'static str {
        match_bench_hash(hash, SHADER_ITEM, "")
    }

    fn lookup_tileset(hash: u64) -> &'static [u8] {
        match_bench_hash(hash, tileset_bytes().as_slice(), &[])
    }

    fn lookup_mesh(hash: u64) -> &'static [u8] {
        match_bench_hash(hash, pmesh_bytes().as_slice(), &[])
    }

    fn lookup_collision_trimesh(hash: u64) -> &'static [u8] {
        match_bench_hash(hash, pmesh_bytes().as_slice(), &[])
    }

    fn lookup_skeleton(hash: u64) -> &'static [u8] {
        match_bench_hash(hash, PSKEL_ITEM, &[])
    }

    fn lookup_texture(hash: u64) -> &'static [u8] {
        match_bench_hash(hash, PTEX_ITEM, &[])
    }

    fn lookup_audio(hash: u64) -> &'static [u8] {
        match_bench_hash(hash, PAWDIO_ITEM, &[])
    }

    fn match_bench_hash<T: ?Sized>(hash: u64, item: &'static T, empty: &'static T) -> &'static T {
        let hashes = lookup_hashes();
        match hash {
            h if h == hashes[0] => item,
            h if h == hashes[1] => item,
            h if h == hashes[2] => item,
            h if h == hashes[3] => item,
            h if h == hashes[4] => item,
            h if h == hashes[5] => item,
            h if h == hashes[6] => item,
            h if h == hashes[7] => item,
            h if h == hashes[8] => item,
            h if h == hashes[9] => item,
            h if h == hashes[10] => item,
            h if h == hashes[11] => item,
            h if h == hashes[12] => item,
            h if h == hashes[13] => item,
            h if h == hashes[14] => item,
            h if h == hashes[15] => item,
            _ => empty,
        }
    }

    fn parse_scene(scene: &'static Scene) -> usize {
        let owned = scene.clone();
        owned.nodes.len() + owned.key_names.len() + usize::from(owned.root.is_some())
    }

    fn parse_material(material: &'static Material3D) -> usize {
        let params = material.standard_params();
        (params.base_color_factor[0].to_bits() as usize) ^ params.base_color_texture as usize
    }

    fn parse_ui_style(style: &'static UiStyle) -> usize {
        style.fill.to_unorm8x4().to_le_u32() as usize
    }

    fn parse_particle(particle: &'static ParticleProfile3D) -> usize {
        particle.lifetime_min.to_bits() as usize
            ^ particle.lifetime_max.to_bits() as usize
            ^ particle.size.to_bits() as usize
    }

    fn parse_animation(animation: &'static AnimationClip) -> usize {
        animation.frame_count() as usize + animation.duration_seconds().to_bits() as usize
    }

    fn parse_animation_tree(tree: &'static AnimationTreeAsset) -> usize {
        tree.name.len() + tree.slots.len() + tree.nodes.len() + tree.output.len()
    }

    fn parse_str(text: &'static str) -> usize {
        text.as_bytes()
            .iter()
            .fold(0usize, |acc, b| acc ^ *b as usize)
    }

    fn parse_tileset(bytes: &[u8]) -> usize {
        let tileset = decode_tileset_2d_binary(bytes).expect("decode PTSET");
        tileset.texture.len() + tileset.tiles.len()
    }

    fn parse_mesh(bytes: &[u8]) -> usize {
        let mesh = decode_raw_pmesh_mesh(bytes).expect("decode PMESH");
        mesh.vertices.len() + mesh.indices.len() + mesh.surface_ranges.len()
    }

    fn parse_collision_trimesh(bytes: &[u8]) -> usize {
        parse_mesh(bytes)
    }

    fn parse_skeleton(bytes: &[u8]) -> usize {
        if bytes.len() >= 17 && &bytes[0..5] == b"PSKEL" {
            u32::from_le_bytes(bytes[13..17].try_into().unwrap()) as usize
        } else {
            0
        }
    }

    fn parse_texture(bytes: &[u8]) -> usize {
        let (rgba, width, height) = decode_raw_ptex(bytes).expect("decode PTEX");
        rgba.len() + width as usize + height as usize
    }

    fn parse_audio(bytes: &[u8]) -> usize {
        if bytes.len() >= 18 && &bytes[0..6] == b"PAWDIO" {
            let raw_len = u32::from_le_bytes(bytes[14..18].try_into().unwrap()) as usize;
            raw_len + bytes[18..].len()
        } else {
            0
        }
    }

    fn tileset_bytes() -> &'static Vec<u8> {
        static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
        BYTES.get_or_init(|| {
            let tileset = TileSet2D {
                texture: Cow::Borrowed("res://bench/texture.png"),
                tile_size: [16.0, 16.0],
                columns: 1,
                rows: 1,
                tiles: Cow::Borrowed(&[]),
            };
            perro_render_bridge::encode_tileset_2d_binary(&tileset)
        })
    }

    fn pmesh_bytes() -> &'static Vec<u8> {
        static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
        BYTES.get_or_init(|| {
            let mut raw = Vec::new();
            for (pos, normal, uv) in [
                ([0.0f32, 0.0, 0.0], [0.0f32, 1.0, 0.0], [0.0f32, 0.0]),
                ([1.0f32, 0.0, 0.0], [0.0f32, 1.0, 0.0], [1.0f32, 0.0]),
                ([0.0f32, 1.0, 0.0], [0.0f32, 1.0, 0.0], [0.0f32, 1.0]),
            ] {
                for value in pos {
                    raw.extend_from_slice(&value.to_le_bytes());
                }
                for value in normal {
                    raw.extend_from_slice(&value.to_le_bytes());
                }
                for value in uv {
                    raw.extend_from_slice(&value.to_le_bytes());
                }
            }
            for index in [0u32, 1, 2] {
                raw.extend_from_slice(&index.to_le_bytes());
            }
            raw.extend_from_slice(&0u32.to_le_bytes());
            raw.extend_from_slice(&3u32.to_le_bytes());

            let mut bytes = Vec::new();
            bytes.extend_from_slice(perro_asset_formats::pmesh::MAGIC);
            bytes.extend_from_slice(&perro_asset_formats::pmesh::VERSION.to_le_bytes());
            bytes.extend_from_slice(
                &(perro_asset_formats::pmesh::FLAG_HAS_NORMAL
                    | perro_asset_formats::pmesh::FLAG_HAS_UV0
                    | perro_asset_formats::pmesh::FLAG_PAYLOAD_RAW)
                    .to_le_bytes(),
            );
            bytes.extend_from_slice(&3u32.to_le_bytes());
            bytes.extend_from_slice(&3u32.to_le_bytes());
            bytes.extend_from_slice(&1u32.to_le_bytes());
            bytes.extend_from_slice(&0u32.to_le_bytes());
            bytes.extend_from_slice(&0u32.to_le_bytes());
            bytes.extend_from_slice(&(raw.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&raw);
            bytes
        })
    }

    fn decode_raw_ptex(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
        if bytes.len() < 24 || &bytes[0..4] != perro_asset_formats::ptex::MAGIC {
            return None;
        }
        let version = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
        if version != perro_asset_formats::ptex::VERSION {
            return None;
        }
        let width = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
        let height = u32::from_le_bytes(bytes[12..16].try_into().ok()?);
        let flags = u32::from_le_bytes(bytes[16..20].try_into().ok()?);
        if flags & perro_asset_formats::ptex::FLAG_PAYLOAD_RAW == 0 {
            return None;
        }
        let raw_len = u32::from_le_bytes(bytes[20..24].try_into().ok()?) as usize;
        let raw = bytes.get(24..24 + raw_len)?.to_vec();
        Some((raw, width, height))
    }

    fn decode_raw_pmesh_mesh(bytes: &[u8]) -> Option<Mesh3D> {
        if bytes.len() < 37 || &bytes[0..5] != perro_asset_formats::pmesh::MAGIC {
            return None;
        }
        let version = u32::from_le_bytes(bytes[5..9].try_into().ok()?);
        if version != perro_asset_formats::pmesh::VERSION {
            return None;
        }
        let flags = u32::from_le_bytes(bytes[9..13].try_into().ok()?);
        if flags & perro_asset_formats::pmesh::FLAG_PAYLOAD_RAW == 0 {
            return None;
        }
        let vertex_count = u32::from_le_bytes(bytes[13..17].try_into().ok()?) as usize;
        let index_count = u32::from_le_bytes(bytes[17..21].try_into().ok()?) as usize;
        let surface_count = u32::from_le_bytes(bytes[21..25].try_into().ok()?) as usize;
        let raw_len = u32::from_le_bytes(bytes[33..37].try_into().ok()?) as usize;
        let raw = bytes.get(37..37 + raw_len)?;
        let has_normal = flags & 1 != 0;
        let has_uv0 = flags & 2 != 0;
        let stride = 12 + if has_normal { 12 } else { 0 } + if has_uv0 { 8 } else { 0 };
        let vertex_bytes = vertex_count.checked_mul(stride)?;
        let index_bytes = index_count.checked_mul(4)?;
        let surface_bytes = surface_count.checked_mul(8)?;
        if raw.len() < vertex_bytes + index_bytes + surface_bytes {
            return None;
        }
        let mut vertices = Vec::with_capacity(vertex_count);
        for i in 0..vertex_count {
            let mut cursor = i * stride;
            let position = read_f32x3(raw, cursor)?;
            cursor += 12;
            let normal = if has_normal {
                let v = read_f32x3(raw, cursor)?;
                cursor += 12;
                v
            } else {
                [0.0, 1.0, 0.0]
            };
            let uv = if has_uv0 {
                read_f32x2(raw, cursor)?
            } else {
                [0.0, 0.0]
            };
            vertices.push(RuntimeMeshVertex {
                position,
                normal,
                uv,
                joints: [0, 0, 0, 0],
                weights: perro_structs::Unorm8x4::new([1.0, 0.0, 0.0, 0.0]),
            });
        }
        let mut indices = Vec::with_capacity(index_count);
        for i in 0..index_count {
            let off = vertex_bytes + i * 4;
            indices.push(u32::from_le_bytes(raw[off..off + 4].try_into().ok()?));
        }
        let mut surface_ranges = Vec::with_capacity(surface_count);
        for i in 0..surface_count {
            let off = vertex_bytes + index_bytes + i * 8;
            surface_ranges.push(MeshSurfaceRange {
                index_start: u32::from_le_bytes(raw[off..off + 4].try_into().ok()?),
                index_count: u32::from_le_bytes(raw[off + 4..off + 8].try_into().ok()?),
            });
        }
        Some(Mesh3D {
            vertices,
            indices,
            surface_ranges,
        })
    }

    fn read_f32x3(bytes: &[u8], off: usize) -> Option<[f32; 3]> {
        Some([
            f32::from_le_bytes(bytes[off..off + 4].try_into().ok()?),
            f32::from_le_bytes(bytes[off + 4..off + 8].try_into().ok()?),
            f32::from_le_bytes(bytes[off + 8..off + 12].try_into().ok()?),
        ])
    }

    fn read_f32x2(bytes: &[u8], off: usize) -> Option<[f32; 2]> {
        Some([
            f32::from_le_bytes(bytes[off..off + 4].try_into().ok()?),
            f32::from_le_bytes(bytes[off + 4..off + 8].try_into().ok()?),
        ])
    }
}
