use proc_macro::{Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree};
use std::fs;
use std::path::{Path, PathBuf};

#[proc_macro]
pub fn include_str_stripped(input: TokenStream) -> TokenStream {
    emit_minified(input)
}

#[proc_macro]
pub fn include_min_str(input: TokenStream) -> TokenStream {
    emit_minified(input)
}

#[proc_macro]
pub fn minified_wgsl(input: TokenStream) -> TokenStream {
    emit_minified(input)
}

fn emit_minified(input: TokenStream) -> TokenStream {
    match parse_input(input)
        .and_then(|path| load_source(&path))
        .map(|source| {
            let lit = Literal::string(&minify_text(&source));
            TokenStream::from(TokenTree::Literal(lit))
        }) {
        Ok(stream) => stream,
        Err(err) => compile_error_tokens(&err),
    }
}

fn parse_input(input: TokenStream) -> Result<String, String> {
    let tokens: Vec<TokenTree> = input.into_iter().collect();
    match tokens.as_slice() {
        [TokenTree::Literal(lit)] => parse_string_literal(&lit.to_string()),
        [TokenTree::Ident(ident), TokenTree::Punct(p), TokenTree::Group(group)]
            if ident.to_string() == "include_str" && p.as_char() == '!' =>
        {
            parse_include_str(group)
        }
        _ => Err(
            "expected string literal path or include_str!(\"path\") in include_str_stripped!"
                .to_string(),
        ),
    }
}

fn parse_include_str(group: &Group) -> Result<String, String> {
    if group.delimiter() != Delimiter::Parenthesis {
        return Err("include_str! must use parentheses".to_string());
    }
    let inner: Vec<TokenTree> = group.stream().into_iter().collect();
    match inner.as_slice() {
        [TokenTree::Literal(lit)] => parse_string_literal(&lit.to_string()),
        _ => Err("include_str! must contain exactly one string literal".to_string()),
    }
}

fn parse_string_literal(raw: &str) -> Result<String, String> {
    if raw.len() < 2 || !raw.starts_with('"') || !raw.ends_with('"') {
        return Err("expected string literal".to_string());
    }
    let s = &raw[1..raw.len() - 1];
    Ok(s.replace("\\\\", "\\").replace("\\\"", "\""))
}

fn load_source(path: &str) -> Result<String, String> {
    let resolved = resolve_path(path)?;
    fs::read_to_string(&resolved)
        .map_err(|e| format!("include_str_stripped! failed read `{}`: {e}", resolved.display()))
}

fn resolve_path(path: &str) -> Result<PathBuf, String> {
    let path_obj = Path::new(path);
    if path_obj.is_absolute() {
        return Ok(path_obj.to_path_buf());
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| "missing CARGO_MANIFEST_DIR for include_str_stripped!".to_string())?;
    let base = Path::new(&manifest_dir);
    let candidate = base.join(path);
    if candidate.exists() {
        return Ok(candidate);
    }
    let src_candidate = base.join("src").join(path);
    if src_candidate.exists() {
        return Ok(src_candidate);
    }

    let src_root = base.join("src");
    let mut matches = Vec::<PathBuf>::new();
    collect_suffix_matches(&src_root, path, &mut matches)?;
    if matches.len() == 1 {
        return Ok(matches.remove(0));
    }
    if matches.len() > 1 {
        return Err(format!(
            "path `{path}` ambiguous ({} matches); use longer path",
            matches.len()
        ));
    }

    Err(format!("failed resolve `{path}` from crate root `{}`", base.display()))
}

fn collect_suffix_matches(dir: &Path, suffix: &str, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("failed read dir `{}`: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("failed read dir entry: {e}"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_suffix_matches(&path, suffix, out)?;
            continue;
        }
        let rel = path.to_string_lossy().replace('\\', "/");
        if rel.ends_with(suffix) {
            out.push(path);
        }
    }
    Ok(())
}

fn minify_text(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut last_blank = false;
    for raw in src.lines() {
        let mut line = raw.trim();
        if line.starts_with("//") {
            continue;
        }
        if let Some(i) = line.find("//") {
            line = line[..i].trim_end();
        }
        if line.is_empty() {
            if !last_blank {
                out.push('\n');
                last_blank = true;
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
        last_blank = false;
    }
    out
}

fn compile_error_tokens(msg: &str) -> TokenStream {
    let mut stream = TokenStream::new();
    stream.extend([TokenTree::Ident(Ident::new("compile_error", Span::call_site()))]);
    stream.extend([TokenTree::Punct(Punct::new('!', Spacing::Alone))]);
    let mut inner = TokenStream::new();
    inner.extend([TokenTree::Literal(Literal::string(msg))]);
    stream.extend([TokenTree::Group(Group::new(Delimiter::Parenthesis, inner))]);
    stream
}
