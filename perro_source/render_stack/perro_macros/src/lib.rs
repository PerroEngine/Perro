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
        .and_then(|paths| {
            let mut resolved_sources = Vec::with_capacity(paths.len());
            for path in paths {
                let resolved = resolve_path(&path)?;
                let source = fs::read_to_string(&resolved).map_err(|e| {
                    format!(
                        "include_str_stripped! failed read `{}`: {e}",
                        resolved.display()
                    )
                })?;
                resolved_sources.push((resolved, source));
            }
            Ok(resolved_sources)
        })
        .map(|resolved_sources| {
            // Expand to `{ const _: &[u8] = include_bytes!("abs path"); ...; "min" }`
            // so cargo tracks the source file and rebuilds on change; a bare
            // literal would go stale silently.
            let source = resolved_sources
                .iter()
                .map(|(_, source)| source.as_str())
                .collect::<String>();
            let lit = Literal::string(&perro_wgsl::optimize_source(&source));
            let mut inner = TokenStream::new();
            for (resolved, _) in resolved_sources {
                let abs = resolved.to_string_lossy().replace('\\', "/");
                inner.extend([
                    TokenTree::Ident(Ident::new("const", Span::call_site())),
                    TokenTree::Ident(Ident::new("_", Span::call_site())),
                    TokenTree::Punct(Punct::new(':', Spacing::Alone)),
                    TokenTree::Punct(Punct::new('&', Spacing::Alone)),
                    TokenTree::Group(Group::new(Delimiter::Bracket, {
                        let mut b = TokenStream::new();
                        b.extend([TokenTree::Ident(Ident::new("u8", Span::call_site()))]);
                        b
                    })),
                    TokenTree::Punct(Punct::new('=', Spacing::Alone)),
                    TokenTree::Punct(Punct::new(':', Spacing::Joint)),
                    TokenTree::Punct(Punct::new(':', Spacing::Alone)),
                    TokenTree::Ident(Ident::new("core", Span::call_site())),
                    TokenTree::Punct(Punct::new(':', Spacing::Joint)),
                    TokenTree::Punct(Punct::new(':', Spacing::Alone)),
                    TokenTree::Ident(Ident::new("include_bytes", Span::call_site())),
                    TokenTree::Punct(Punct::new('!', Spacing::Alone)),
                    TokenTree::Group(Group::new(Delimiter::Parenthesis, {
                        let mut p = TokenStream::new();
                        p.extend([TokenTree::Literal(Literal::string(&abs))]);
                        p
                    })),
                    TokenTree::Punct(Punct::new(';', Spacing::Alone)),
                ]);
            }
            inner.extend([TokenTree::Literal(lit)]);
            TokenStream::from(TokenTree::Group(Group::new(Delimiter::Brace, inner)))
        }) {
        Ok(stream) => stream,
        Err(err) => compile_error_tokens(&err),
    }
}

fn parse_input(input: TokenStream) -> Result<Vec<String>, String> {
    let tokens: Vec<TokenTree> = input.into_iter().collect();
    match tokens.as_slice() {
        [TokenTree::Literal(lit)] => parse_string_literal(&lit.to_string()).map(|path| vec![path]),
        [
            TokenTree::Ident(ident),
            TokenTree::Punct(p),
            TokenTree::Group(group),
        ] if ident.to_string() == "include_str" && p.as_char() == '!' => {
            parse_include_str(group).map(|path| vec![path])
        }
        [
            TokenTree::Ident(ident),
            TokenTree::Punct(p),
            TokenTree::Group(group),
        ] if ident.to_string() == "concat" && p.as_char() == '!' => parse_concat(group),
        _ => Err(
            "expected path, include_str!(\"path\"), or concat!(include_str!(...), ...)".to_string(),
        ),
    }
}

fn parse_concat(group: &Group) -> Result<Vec<String>, String> {
    if group.delimiter() != Delimiter::Parenthesis {
        return Err("concat! must use parentheses".to_string());
    }
    let mut paths = Vec::new();
    let mut item = TokenStream::new();
    for token in group.stream() {
        if matches!(&token, TokenTree::Punct(p) if p.as_char() == ',') {
            if item.is_empty() {
                return Err("concat! contains empty item".to_string());
            }
            paths.extend(parse_input(item)?);
            item = TokenStream::new();
        } else {
            item.extend([token]);
        }
    }
    if !item.is_empty() {
        paths.extend(parse_input(item)?);
    }
    if paths.is_empty() {
        return Err("concat! needs at least one source".to_string());
    }
    Ok(paths)
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

    Err(format!(
        "failed resolve `{path}` from crate root `{}`",
        base.display()
    ))
}

fn collect_suffix_matches(dir: &Path, suffix: &str, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        fs::read_dir(dir).map_err(|e| format!("failed read dir `{}`: {e}", dir.display()))?;
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

fn compile_error_tokens(msg: &str) -> TokenStream {
    let mut stream = TokenStream::new();
    stream.extend([TokenTree::Ident(Ident::new(
        "compile_error",
        Span::call_site(),
    ))]);
    stream.extend([TokenTree::Punct(Punct::new('!', Spacing::Alone))]);
    let mut inner = TokenStream::new();
    inner.extend([TokenTree::Literal(Literal::string(msg))]);
    stream.extend([TokenTree::Group(Group::new(Delimiter::Parenthesis, inner))]);
    stream
}
