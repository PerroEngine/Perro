pub fn short_path(path: &str, max: usize) -> String {
    if path.len() <= max {
        path.to_string()
    } else if path.is_ascii() {
        if max <= 3 {
            ".".repeat(max)
        } else {
            format!("...{}", &path[path.len() - (max - 3)..])
        }
    } else if path.chars().count() <= max {
        path.to_string()
    } else if max <= 3 {
        ".".repeat(max)
    } else {
        let keep = max - 3;
        let start = path
            .char_indices()
            .rev()
            .nth(keep - 1)
            .map(|(idx, _)| idx)
            .unwrap_or(0);
        format!("...{}", &path[start..])
    }
}

#[cfg(test)]
mod tests {
    use super::short_path;

    #[test]
    fn short_path_keeps_utf8_boundaries() {
        assert_eq!(short_path("res://éclair/😀.scn", 10), "...r/😀.scn");
    }

    #[test]
    fn short_path_handles_small_limits() {
        assert_eq!(short_path("abcdef", 0), "");
        assert_eq!(short_path("abcdef", 2), "..");
        assert_eq!(short_path("abcdef", 3), "...");
    }
}
