use std::path::Path;

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp", "tga"];
const AUDIO_EXTENSIONS: &[&str] = &[
    "wav", "ogg", "mp3", "flac", "aac", "m4a", "mid", "midi", "sf2",
];
const RESOURCE_EXTENSIONS: &[&str] = &[
    "pmat",
    "uistyle",
    "ppart",
    "ptileset",
    "panim",
    "panimtree",
    "pskel2d",
    "pskel3d",
];
const MESH_EXTENSIONS: &[&str] = &["glb", "gltf", "obj", "fbx"];

pub fn sort_key(path: &str) -> (u8, String) {
    let folder = path.ends_with('/');
    let mut label = rel_label(path);
    label.make_ascii_lowercase();
    ((!folder) as u8, label)
}

pub fn res_browser_sort_key(path: &str) -> String {
    let mut label = rel_label(path);
    label.make_ascii_lowercase();
    let folder = path.ends_with('/');
    let last_part = label.trim_end_matches('/').matches('/').count();
    let mut key = String::with_capacity(label.len() + last_part + 1);
    // NUL component separators keep a folder's descendants before sibling names.
    for (idx, part) in label.split('/').filter(|part| !part.is_empty()).enumerate() {
        key.push_str(part);
        if idx < last_part || folder {
            key.push('\0');
        } else {
            key.push('\x01');
        }
    }
    key
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
    let Some(ext) = extension(path) else {
        return "other";
    };
    if ext.eq_ignore_ascii_case("scn") || ext.eq_ignore_ascii_case("fur") {
        "scene"
    } else if ext.eq_ignore_ascii_case("rs") {
        "script"
    } else if has_extension(ext, IMAGE_EXTENSIONS) {
        "image"
    } else if has_extension(ext, AUDIO_EXTENSIONS) {
        "audio"
    } else if has_extension(ext, RESOURCE_EXTENSIONS) {
        "resource"
    } else if has_extension(ext, MESH_EXTENSIONS) {
        "mesh"
    } else {
        "other"
    }
}

pub fn display_kind_label(path: &str) -> &'static str {
    if path.ends_with('/') {
        return "DIR";
    }
    let Some(ext) = extension(path) else {
        return "OTH";
    };
    if ext.eq_ignore_ascii_case("scn") || ext.eq_ignore_ascii_case("fur") {
        "SCN"
    } else if ext.eq_ignore_ascii_case("rs") {
        "RS"
    } else if has_extension(ext, IMAGE_EXTENSIONS) {
        "IMG"
    } else if has_extension(ext, AUDIO_EXTENSIONS) {
        "AUD"
    } else if ext.eq_ignore_ascii_case("panim") {
        "ANIM"
    } else if ext.eq_ignore_ascii_case("panimtree") {
        "TREE"
    } else if ext.eq_ignore_ascii_case("pmat") {
        "MAT"
    } else if ext.eq_ignore_ascii_case("uistyle") {
        "STYLE"
    } else if ext.eq_ignore_ascii_case("ppart") {
        "PART"
    } else if ext.eq_ignore_ascii_case("ptileset") {
        "TILE"
    } else if ext.eq_ignore_ascii_case("pskel2d") || ext.eq_ignore_ascii_case("pskel3d") {
        "SKEL"
    } else if ext.eq_ignore_ascii_case("glb") || ext.eq_ignore_ascii_case("gltf") {
        "GLB"
    } else if ext.eq_ignore_ascii_case("obj")
        || ext.eq_ignore_ascii_case("fbx")
        || ext.eq_ignore_ascii_case("pmesh")
    {
        "MESH"
    } else {
        "OTH"
    }
}

fn extension(path: &str) -> Option<&str> {
    Path::new(path).extension().and_then(|value| value.to_str())
}

#[inline]
fn has_extension(ext: &str, values: &[&str]) -> bool {
    values.iter().any(|value| ext.eq_ignore_ascii_case(value))
}

#[cfg(test)]
mod tests {
    use super::{display_kind_label, kind_label, res_browser_sort_key};

    #[test]
    fn browser_sort_keeps_folder_subtree_before_sibling_file() {
        let mut paths = vec![
            "res://a/z.scn".to_string(),
            "res://a.scn".to_string(),
            "res://a/".to_string(),
            "res://z/".to_string(),
            "res://z/a.scn".to_string(),
        ];
        paths.sort_by_cached_key(|path| res_browser_sort_key(path));
        assert_eq!(
            paths,
            vec![
                "res://a/",
                "res://a/z.scn",
                "res://a.scn",
                "res://z/",
                "res://z/a.scn",
            ]
        );
    }

    #[test]
    fn kind_labels_keep_case_insensitive_extension_rules() {
        assert_eq!(kind_label("res://Main.SCN"), "scene");
        assert_eq!(kind_label("res://hero.PNG"), "image");
        assert_eq!(display_kind_label("res://run.PANIM"), "ANIM");
        assert_eq!(display_kind_label("res://mesh.PMESH"), "MESH");
    }
}
