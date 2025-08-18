use std::path::PathBuf;

pub fn resolve_res_path(res_path: &str) -> PathBuf {
    const PREFIX: &str = "res://";
    if let Some(stripped) = res_path.strip_prefix(PREFIX) {
        let mut pb = PathBuf::from("res");
        pb.push(stripped);
        pb
    } else {
        PathBuf::from(res_path)
    }
}