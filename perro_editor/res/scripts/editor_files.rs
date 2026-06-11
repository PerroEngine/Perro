use std::path::Path;

pub fn sort_key(path: &str) -> (u8, String) {
    let folder = path.ends_with('/');
    ((!folder) as u8, rel_label(path).to_ascii_lowercase())
}

pub fn res_browser_sort_key(path: &str) -> (usize, u8, String) {
    let label = rel_label(path);
    let depth = label.matches('/').count();
    let folder = path.ends_with('/');
    (depth, (!folder) as u8, label.to_ascii_lowercase())
}

pub fn rel_label(path: &str) -> String {
    path.trim_start_matches("res://")
        .trim_end_matches('/')
        .to_string()
}

pub fn kind_label(path: &str) -> &'static str {
    if path.ends_with('/') {
        return "folder";
    }
    let Some(ext) = Path::new(path).extension().and_then(|v| v.to_str()) else {
        return "other";
    };
    match ext.to_ascii_lowercase().as_str() {
        "scn" | "fur" => "scene",
        "rs" => "script",
        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "tga" => "image",
        "wav" | "ogg" | "mp3" | "flac" | "aac" | "m4a" | "mid" | "midi" | "sf2" => "audio",
        "pmat" | "uistyle" | "ppart" | "ptileset" | "panim" | "panimtree" | "pskel2d"
        | "pskel3d" => "resource",
        "glb" | "gltf" | "obj" | "fbx" => "mesh",
        _ => "other",
    }
}

pub fn display_kind_label(path: &str) -> &'static str {
    if path.ends_with('/') {
        return "DIR";
    }
    let Some(ext) = Path::new(path).extension().and_then(|v| v.to_str()) else {
        return "OTH";
    };
    match ext.to_ascii_lowercase().as_str() {
        "scn" | "fur" => "SCN",
        "rs" => "RS",
        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "tga" => "IMG",
        "wav" | "ogg" | "mp3" | "flac" | "aac" | "m4a" | "mid" | "midi" | "sf2" => "AUD",
        "panim" => "ANIM",
        "panimtree" => "TREE",
        "pmat" => "MAT",
        "uistyle" => "STYLE",
        "ppart" => "PART",
        "ptileset" => "TILE",
        "pskel2d" | "pskel3d" => "SKEL",
        "glb" | "gltf" => "GLB",
        "obj" | "fbx" | "pmesh" => "MESH",
        _ => "OTH",
    }
}
