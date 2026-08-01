use crate::{StaticPipelineError, res_dir};
use perro_io::walkdir::collect_file_paths;
use std::path::Path;

/// The `res` tree listed once per build and shared by every generator.
///
/// Each generator used to walk the whole tree itself (~17 walks per build,
/// each canonicalizing every directory and stat'ing every entry) only to keep
/// the handful of paths matching its own extensions. Now the driver walks once
/// and each generator filters this listing.
///
/// Paths are relative to the res root, `/`-normalized and sorted, matching
/// what the per-generator walks produced.
#[derive(Clone, Debug, Default)]
pub struct ResFileTree {
    paths: Vec<String>,
}

impl ResFileTree {
    /// Walk the res root for the current thread's pipeline overrides. A
    /// missing root lists nothing, as an absent `res` dir did before.
    pub fn scan(project_root: &Path) -> Result<Self, StaticPipelineError> {
        let res_dir = res_dir(project_root);
        if !res_dir.exists() {
            return Ok(Self::default());
        }
        let mut paths = collect_file_paths(&res_dir, &res_dir)?
            .into_iter()
            .map(|rel| rel.replace('\\', "/"))
            .collect::<Vec<_>>();
        paths.sort();
        Ok(Self { paths })
    }

    /// Paths whose extension passes `keep`, which receives the raw extension
    /// (callers compare case-insensitively, as the per-generator walks did).
    pub(crate) fn filter_ext<F>(&self, keep: F) -> Vec<String>
    where
        F: Fn(&str) -> bool,
    {
        self.paths
            .iter()
            .filter(|rel| {
                Path::new(rel)
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(&keep)
            })
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(paths: &[&str]) -> ResFileTree {
        let mut paths = paths.iter().map(|p| (*p).to_string()).collect::<Vec<_>>();
        paths.sort();
        ResFileTree { paths }
    }

    #[test]
    fn filter_keeps_sorted_matching_paths_only() {
        let tree = tree(&[
            "sfx/hit.wav",
            "art/player.png",
            "scenes/main.scn",
            "no_extension",
            "art/enemy.PNG",
        ]);
        assert_eq!(
            tree.filter_ext(|ext| ext.eq_ignore_ascii_case("png")),
            vec!["art/enemy.PNG".to_string(), "art/player.png".to_string()]
        );
        assert!(tree.filter_ext(|ext| ext == "gltf").is_empty());
    }
}
