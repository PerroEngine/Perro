//! Safe directory walking shared with `perro_assets`.

pub use perro_assets::walkdir::{
    PathExclusionGuard, collect_file_paths, collect_files, is_relative_path_excluded,
    matches_path_pattern, push_path_exclusions, walk_dir,
};
