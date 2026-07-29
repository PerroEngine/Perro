pub mod compose;

#[cfg(feature = "check")]
pub mod check;

/// Strip comments and normalize whitespace without changing WGSL tokens.
#[must_use]
pub fn optimize_source(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut pending_space = false;

    while let Some(ch) = chars.next() {
        if ch.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }

        if ch == '/' {
            match chars.peek().copied() {
                Some('/') => {
                    chars.next();
                    for comment_ch in chars.by_ref() {
                        if comment_ch == '\n' {
                            break;
                        }
                    }
                    pending_space = !out.is_empty();
                    continue;
                }
                Some('*')
                    if chars
                        .clone()
                        .take(10)
                        .collect::<String>()
                        .starts_with("*__PERRO_") =>
                {
                    if pending_space {
                        out.push(' ');
                        pending_space = false;
                    }
                    out.push(ch);
                    continue;
                }
                Some('*') => {
                    chars.next();
                    let mut depth = 1usize;
                    let mut prior = '\0';
                    for comment_ch in chars.by_ref() {
                        if prior == '/' && comment_ch == '*' {
                            depth += 1;
                            prior = '\0';
                            continue;
                        }
                        if prior == '*' && comment_ch == '/' {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                            prior = '\0';
                            continue;
                        }
                        prior = comment_ch;
                    }
                    pending_space = !out.is_empty();
                    continue;
                }
                _ => {}
            }
        }

        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.push(ch);

        if ch == '"' {
            let mut escaped = false;
            for string_ch in chars.by_ref() {
                out.push(string_ch);
                if escaped {
                    escaped = false;
                } else if string_ch == '\\' {
                    escaped = true;
                } else if string_ch == '"' {
                    break;
                }
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::optimize_source;

    const WGSL: &str = r#"
// file comment
struct Out {
    @builtin(position) pos: vec4<f32>, /* field comment */
};

/* outer
   /* nested */
*/
@vertex
fn main(@builtin(vertex_index) index: u32) -> Out {
    var out: Out;
    let x = f32(index); // row comment
    out.pos = vec4<f32>(x, 0.0, 0.0, 1.0);
    return out;
}
"#;

    #[test]
    fn optimizer_keeps_canonical_wgsl_source_same() {
        let optimized = optimize_source(WGSL);
        let original = canonical_wgsl(WGSL);
        let result = canonical_wgsl(&optimized);

        assert_eq!(result, original);
        assert!(optimized.len() < WGSL.len());
    }

    #[test]
    fn optimizer_is_idempotent() {
        let once = optimize_source(WGSL);
        assert_eq!(optimize_source(&once), once);
    }

    #[test]
    fn optimizer_keeps_comment_markers_in_strings() {
        let source = "let a = \"//\"; /* rm */ let b = \"/* keep */\";";
        assert_eq!(
            optimize_source(source),
            "let a = \"//\"; let b = \"/* keep */\";"
        );
    }

    #[test]
    fn optimizer_keeps_engine_placeholders() {
        let source = "fn a() {} /*__PERRO_TEST__*/ fn b() {}";
        assert_eq!(optimize_source(source), source);
    }

    fn canonical_wgsl(source: &str) -> String {
        let module = naga::front::wgsl::parse_str(source).expect("parse WGSL");
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("validate WGSL");
        naga::back::wgsl::write_string(&module, &info, naga::back::wgsl::WriterFlags::empty())
            .expect("write WGSL")
    }
}
