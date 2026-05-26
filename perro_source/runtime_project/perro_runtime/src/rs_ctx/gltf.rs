use super::core::RuntimeResourceApi;
use perro_resource_api::sub_apis::{GltfAPI, GltfInfo};

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
