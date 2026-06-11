use super::core::RuntimeResourceApi;
use perro_resource_api::sub_apis::{GltfAPI, GltfInfo};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write;

impl GltfAPI for RuntimeResourceApi {
    fn inspect_gltf(&self, source: &str) -> Option<GltfInfo> {
        let source = normalize_source(source);
        let path = strip_gltf_fragment(source.as_ref());
        if !is_gltf_path(path) {
            return None;
        }
        let bytes = perro_io::load_asset(path).ok()?;
        let doc = gltf::Gltf::from_slice(&bytes).ok()?;
        Some(GltfInfo {
            mesh_count: doc.meshes().count(),
            material_count: doc.materials().count(),
            skeleton_count: doc.skins().count(),
            animation_count: doc.animations().count(),
            node_count: doc.nodes().count(),
            scene_count: doc.scenes().count(),
            texture_count: doc.textures().count(),
        })
    }

    fn convert_gltf_animation_to_panim(
        &self,
        source: &str,
        fps: f32,
        animation_index: usize,
        skeleton_object: &str,
    ) -> Result<String, String> {
        let source = normalize_source(source);
        let path = strip_gltf_fragment(source.as_ref());
        if !is_gltf_path(path) {
            return Err(format!("not gltf source: {source}"));
        }
        let bytes =
            perro_io::load_asset(path).map_err(|err| format!("load gltf fail {path}: {err}"))?;
        convert_gltf_animation_bytes_to_panim(&bytes, fps, animation_index, skeleton_object, path)
    }

    fn convert_gltf_material_to_pmat(
        &self,
        source: &str,
        material_index: usize,
    ) -> Result<String, String> {
        let source = normalize_source(source);
        let path = strip_gltf_fragment(source.as_ref());
        if !is_gltf_path(path) {
            return Err(format!("not gltf source: {source}"));
        }
        let bytes =
            perro_io::load_asset(path).map_err(|err| format!("load gltf fail {path}: {err}"))?;
        convert_gltf_material_bytes_to_pmat(&bytes, material_index, path)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct PanimTrackTarget {
    object: String,
    prop: String,
}

#[derive(Default)]
struct PanimFrameBlock {
    tracks: BTreeMap<PanimTrackTarget, String>,
}

fn convert_gltf_animation_bytes_to_panim(
    bytes: &[u8],
    fps: f32,
    animation_index: usize,
    skeleton_object: &str,
    label: &str,
) -> Result<String, String> {
    let (doc, buffers, _images) =
        gltf::import_slice(bytes).map_err(|err| format!("import gltf fail {label}: {err}"))?;
    let animation = doc
        .animations()
        .nth(animation_index)
        .ok_or_else(|| format!("animation index {animation_index} missing"))?;
    let animation_name = animation
        .name()
        .map(sanitize_display_text)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| format!("Animation{}", animation.index()));
    let joint_nodes = collect_gltf_joint_nodes(&doc);
    let node_names = doc
        .nodes()
        .map(|node| {
            (
                node.index(),
                node.name()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| format!("Node{}", node.index())),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut frames = BTreeMap::<u32, PanimFrameBlock>::new();
    let mut objects = BTreeMap::<String, String>::new();
    let mut used_object_names = BTreeSet::<String>::from([skeleton_object.to_string()]);
    let mut object_name_by_node = HashMap::<usize, String>::new();

    for channel in animation.channels() {
        let target = channel.target();
        if target.property() == gltf::animation::Property::MorphTargetWeights {
            continue;
        }
        let node = target.node();
        let node_index = node.index();
        let node_name = node_names
            .get(&node_index)
            .cloned()
            .unwrap_or_else(|| format!("Node{node_index}"));
        let (object, prop) = if joint_nodes.contains(&node_index) {
            objects.insert(skeleton_object.to_string(), "Skeleton3D".to_string());
            (
                skeleton_object.to_string(),
                format!(
                    "bone[\"{}\"].{}",
                    escape_panim_str(&node_name),
                    gltf_target_property_name(&target)
                ),
            )
        } else {
            let object = object_name_by_node
                .entry(node_index)
                .or_insert_with(|| unique_panim_ident(&node_name, &mut used_object_names))
                .clone();
            objects.insert(object.clone(), "Node3D".to_string());
            (object, gltf_target_property_name(&target).to_string())
        };
        let reader = channel.reader(|buffer| buffers.get(buffer.index()).map(|b| b.0.as_slice()));
        let sampler = channel.sampler();
        let value_step = match sampler.interpolation() {
            gltf::animation::Interpolation::CubicSpline => 3,
            gltf::animation::Interpolation::Linear | gltf::animation::Interpolation::Step => 1,
        };
        let value_offset = if value_step == 3 { 1 } else { 0 };
        let inputs = reader
            .read_inputs()
            .ok_or_else(|| format!("channel on {node_name} lacks times"))?
            .collect::<Vec<f32>>();
        if inputs.is_empty() {
            continue;
        }
        match reader
            .read_outputs()
            .ok_or_else(|| format!("channel on {node_name} lacks values"))?
        {
            gltf::animation::util::ReadOutputs::Translations(values) => {
                let values = values.collect::<Vec<_>>();
                for (index, time) in inputs.iter().copied().enumerate() {
                    if let Some(value) = values.get(index * value_step + value_offset).copied() {
                        insert_panim_track(
                            &mut frames,
                            time,
                            fps,
                            &object,
                            &prop,
                            panim_vec3_value(value),
                        );
                    }
                }
            }
            gltf::animation::util::ReadOutputs::Rotations(values) => {
                let values = values.into_f32().collect::<Vec<_>>();
                for (index, time) in inputs.iter().copied().enumerate() {
                    if let Some(value) = values.get(index * value_step + value_offset).copied() {
                        insert_panim_track(
                            &mut frames,
                            time,
                            fps,
                            &object,
                            &prop,
                            panim_quat_value(value),
                        );
                    }
                }
            }
            gltf::animation::util::ReadOutputs::Scales(values) => {
                let values = values.collect::<Vec<_>>();
                for (index, time) in inputs.iter().copied().enumerate() {
                    if let Some(value) = values.get(index * value_step + value_offset).copied() {
                        insert_panim_track(
                            &mut frames,
                            time,
                            fps,
                            &object,
                            &prop,
                            panim_vec3_value(value),
                        );
                    }
                }
            }
            gltf::animation::util::ReadOutputs::MorphTargetWeights(_) => {}
        }
    }

    if frames.is_empty() {
        return Err(format!("animation {animation_name} no trs tracks"));
    }
    render_gltf_panim(&animation_name, fps, &objects, &frames)
}

fn convert_gltf_material_bytes_to_pmat(
    bytes: &[u8],
    material_index: usize,
    label: &str,
) -> Result<String, String> {
    let doc =
        gltf::Gltf::from_slice(bytes).map_err(|err| format!("open gltf fail {label}: {err}"))?;
    let material = doc
        .materials()
        .nth(material_index)
        .ok_or_else(|| format!("material index {material_index} missing"))?;
    let pbr = material.pbr_metallic_roughness();
    let color = pbr.base_color_factor();
    let emissive = material.emissive_factor();
    let alpha_mode = match material.alpha_mode() {
        gltf::material::AlphaMode::Opaque => "OPAQUE",
        gltf::material::AlphaMode::Mask => "MASK",
        gltf::material::AlphaMode::Blend => "BLEND",
    };
    let mut out = String::new();
    let _ = writeln!(out, "type = \"standard\"\n");
    let _ = writeln!(
        out,
        "base_color_factor = ({}, {}, {}, {})",
        panim_fmt_f32(color[0]),
        panim_fmt_f32(color[1]),
        panim_fmt_f32(color[2]),
        panim_fmt_f32(color[3])
    );
    let _ = writeln!(
        out,
        "metallic_factor = {}",
        panim_fmt_f32(pbr.metallic_factor())
    );
    let _ = writeln!(
        out,
        "roughness_factor = {}",
        panim_fmt_f32(pbr.roughness_factor())
    );
    let _ = writeln!(
        out,
        "emissive_factor = ({}, {}, {})",
        panim_fmt_f32(emissive[0]),
        panim_fmt_f32(emissive[1]),
        panim_fmt_f32(emissive[2])
    );
    let _ = writeln!(out, "alpha_mode = \"{alpha_mode}\"");
    let _ = writeln!(
        out,
        "alpha_cutoff = {}",
        panim_fmt_f32(material.alpha_cutoff().unwrap_or(0.5))
    );
    let _ = writeln!(out, "double_sided = {}", material.double_sided());
    if let Some(info) = pbr.base_color_texture() {
        let _ = writeln!(out, "base_color_texture = {}", info.texture().index());
    }
    if let Some(info) = pbr.metallic_roughness_texture() {
        let _ = writeln!(
            out,
            "metallic_roughness_texture = {}",
            info.texture().index()
        );
    }
    if let Some(info) = material.normal_texture() {
        let _ = writeln!(out, "normal_texture = {}", info.texture().index());
        let _ = writeln!(out, "normal_scale = {}", panim_fmt_f32(info.scale()));
    }
    if let Some(info) = material.occlusion_texture() {
        let _ = writeln!(out, "occlusion_texture = {}", info.texture().index());
        let _ = writeln!(
            out,
            "occlusion_strength = {}",
            panim_fmt_f32(info.strength())
        );
    }
    if let Some(info) = material.emissive_texture() {
        let _ = writeln!(out, "emissive_texture = {}", info.texture().index());
    }
    Ok(out)
}

fn collect_gltf_joint_nodes(doc: &gltf::Document) -> HashSet<usize> {
    let mut joints = HashSet::new();
    for skin in doc.skins() {
        for joint in skin.joints() {
            joints.insert(joint.index());
        }
    }
    joints
}

fn gltf_target_property_name(target: &gltf::animation::Target) -> &'static str {
    match target.property() {
        gltf::animation::Property::Translation => "position",
        gltf::animation::Property::Rotation => "rotation",
        gltf::animation::Property::Scale => "scale",
        gltf::animation::Property::MorphTargetWeights => "weights",
    }
}

fn insert_panim_track(
    frames: &mut BTreeMap<u32, PanimFrameBlock>,
    time: f32,
    fps: f32,
    object: &str,
    prop: &str,
    value: String,
) {
    if !time.is_finite() {
        return;
    }
    let frame = (time * fps).round().max(0.0) as u32;
    frames.entry(frame).or_default().tracks.insert(
        PanimTrackTarget {
            object: object.to_string(),
            prop: prop.to_string(),
        },
        value,
    );
}

fn render_gltf_panim(
    animation_name: &str,
    fps: f32,
    objects: &BTreeMap<String, String>,
    frames: &BTreeMap<u32, PanimFrameBlock>,
) -> Result<String, String> {
    let mut out = String::new();
    let _ = writeln!(out, "[Animation]");
    let _ = writeln!(out, "name = \"{}\"", escape_panim_str(animation_name));
    let _ = writeln!(out, "fps = {}", panim_fmt_f32(fps));
    let _ = writeln!(out, "default_interp = \"interpolate\"");
    let _ = writeln!(out, "default_ease = \"linear\"");
    let _ = writeln!(out, "[/Animation]\n");
    let _ = writeln!(out, "[Objects]");
    for (object, node_type) in objects {
        let _ = writeln!(out, "{object} = {node_type}");
    }
    let _ = writeln!(out, "[/Objects]\n");
    for (frame, block) in frames {
        let _ = writeln!(out, "[Frame{frame}]");
        let mut props_by_object = BTreeMap::<&str, Vec<(&str, &str)>>::new();
        for (target, value) in &block.tracks {
            props_by_object
                .entry(&target.object)
                .or_default()
                .push((&target.prop, value));
        }
        for (object, props) in props_by_object {
            let _ = writeln!(out, "@{object} {{");
            for (prop, value) in props {
                let _ = writeln!(out, "    {prop} = {value}");
            }
            let _ = writeln!(out, "}}");
        }
        let _ = writeln!(out, "[/Frame{frame}]\n");
    }
    Ok(out)
}

fn sanitize_display_text(raw: &str) -> String {
    raw.chars()
        .filter(|ch| !ch.is_control())
        .collect::<String>()
        .trim()
        .to_string()
}

fn unique_panim_ident(raw: &str, used: &mut BTreeSet<String>) -> String {
    let base = sanitize_panim_ident(raw);
    if used.insert(base.clone()) {
        return base;
    }
    for index in 1..1000 {
        let candidate = format!("{base}_{index}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
    }
    format!("{base}_x")
}

fn sanitize_panim_ident(raw: &str) -> String {
    let ident = sanitize_display_text(raw)
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    if ident.is_empty() {
        "Node".to_string()
    } else if ident
        .chars()
        .next()
        .map(|ch| ch.is_ascii_digit())
        .unwrap_or(false)
    {
        format!("N_{ident}")
    } else {
        ident
    }
}

fn panim_vec3_value(value: [f32; 3]) -> String {
    format!(
        "({}, {}, {})",
        panim_fmt_f32(value[0]),
        panim_fmt_f32(value[1]),
        panim_fmt_f32(value[2])
    )
}

fn panim_quat_value(value: [f32; 4]) -> String {
    format!(
        "({}, {}, {}, {})",
        panim_fmt_f32(value[0]),
        panim_fmt_f32(value[1]),
        panim_fmt_f32(value[2]),
        panim_fmt_f32(value[3])
    )
}

fn panim_fmt_f32(value: f32) -> String {
    if value == 0.0 {
        return "0.0".to_string();
    }
    let mut out = format!("{value:.6}");
    while out.contains('.') && out.ends_with('0') {
        out.pop();
    }
    if out.ends_with('.') {
        out.push('0');
    }
    out
}

fn escape_panim_str(raw: &str) -> String {
    raw.replace('\\', "\\\\").replace('"', "\\\"")
}

fn normalize_source(source: &str) -> std::borrow::Cow<'_, str> {
    if source.contains('\\') {
        std::borrow::Cow::Owned(source.replace('\\', "/"))
    } else {
        std::borrow::Cow::Borrowed(source)
    }
}

fn strip_gltf_fragment(source: &str) -> &str {
    let Some((path, selector)) = source.rsplit_once(':') else {
        return source;
    };
    if path.is_empty() || selector.contains('/') || selector.contains('\\') {
        return source;
    }
    if selector.contains('[') && selector.ends_with(']') {
        return path;
    }
    source
}

fn is_gltf_path(path: &str) -> bool {
    let Some(ext) = std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
    else {
        return false;
    };
    matches!(ext.to_ascii_lowercase().as_str(), "glb" | "gltf")
}

#[cfg(test)]
mod tests {
    use super::RuntimeResourceApi;
    use perro_resource_api::{
        ResourceWindow, animation_count, glb_inspect, material_count, mesh_count, node_count,
        scene_count, skeleton_count, texture_count,
    };
    use std::{fs, path::PathBuf, sync::Arc};

    fn new_api() -> Arc<RuntimeResourceApi> {
        RuntimeResourceApi::new(None, None, None, None, None, None, None, None)
    }

    fn write_test_gltf(name: &str, text: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("perro_gltf_info_{}_{}", std::process::id(), name));
        fs::create_dir_all(&dir).expect("create gltf test dir");
        let path = dir.join("asset.gltf");
        fs::write(&path, text).expect("write gltf test asset");
        path
    }

    #[test]
    fn glb_info_counts_gltf_entries() {
        let path = write_test_gltf(
            "counts",
            r#"{
                "asset": { "version": "2.0" },
                "scenes": [{ "nodes": [0] }],
                "nodes": [{}],
                "meshes": [{ "primitives": [] }, { "primitives": [] }],
                "materials": [{}, {}, {}],
                "skins": [{ "joints": [0] }]
            }"#,
        );
        let api = new_api();
        let res = ResourceWindow::new(api.as_ref());
        let source = path.to_string_lossy();
        let info = res.Glbs().inspect(source.as_ref()).expect("inspect gltf");

        assert_eq!(info.mesh_count, 2);
        assert_eq!(info.material_count, 3);
        assert_eq!(info.skeleton_count, 1);
        assert_eq!(info.animation_count, 0);
        assert_eq!(info.node_count, 1);
        assert_eq!(info.scene_count, 1);
        assert_eq!(info.texture_count, 0);
        assert_eq!(glb_inspect!(res, source.as_ref()), Some(info));
        assert_eq!(mesh_count!(res, source.as_ref()), Some(2));
        assert_eq!(material_count!(res, source.as_ref()), Some(3));
        assert_eq!(skeleton_count!(res, source.as_ref()), Some(1));
        assert_eq!(animation_count!(res, source.as_ref()), Some(0));
        assert_eq!(node_count!(res, source.as_ref()), Some(1));
        assert_eq!(scene_count!(res, source.as_ref()), Some(1));
        assert_eq!(texture_count!(res, source.as_ref()), Some(0));
    }

    #[test]
    fn glb_info_accepts_sub_asset_source() {
        let path = write_test_gltf(
            "fragment",
            r#"{
                "asset": { "version": "2.0" },
                "meshes": [{ "primitives": [] }]
            }"#,
        );
        let api = new_api();
        let res = ResourceWindow::new(api.as_ref());
        let source = format!("{}:mesh[0]", path.to_string_lossy());

        assert_eq!(res.Glbs().mesh_count(source.as_str()), Some(1));
    }
}
