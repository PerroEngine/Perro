use pulldown_cmark::{html, CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd};
use std::collections::HashMap;

pub fn markdown_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(markdown, options);
    let events = add_heading_ids(parser);
    let events = highlight_code_blocks(events);
    let mut out = String::with_capacity(markdown.len());
    html::push_html(&mut out, events.into_iter());
    out
}

fn add_heading_ids<'a>(events: impl IntoIterator<Item = Event<'a>>) -> Vec<Event<'a>> {
    let mut out = Vec::new();
    let mut heading = None::<(pulldown_cmark::HeadingLevel, Vec<Event<'a>>, String)>;
    let mut seen = HashMap::<String, usize>::new();

    for event in events {
        match (&mut heading, event) {
            (None, Event::Start(Tag::Heading { level, .. })) => {
                heading = Some((level, Vec::new(), String::new()));
            }
            (Some((_, events, text)), event @ (Event::Text(_) | Event::Code(_))) => {
                match &event {
                    Event::Text(value) | Event::Code(value) => text.push_str(value),
                    _ => unreachable!(),
                }
                events.push(event);
            }
            (Some((level, events, text)), Event::End(TagEnd::Heading(_))) => {
                let id = unique_anchor_id(text, &mut seen);
                let level = heading_level_num(*level);
                out.push(Event::Html(CowStr::from(format!("<h{level} id=\"{id}\">"))));
                out.append(events);
                out.push(Event::Html(CowStr::from(format!("</h{level}>"))));
                heading = None;
            }
            (Some((_, events, _)), event) => events.push(event),
            (None, event) => out.push(event),
        }
    }

    if let Some((level, mut events, _)) = heading {
        out.push(Event::Start(Tag::Heading {
            level,
            id: None,
            classes: Vec::new(),
            attrs: Vec::new(),
        }));
        out.append(&mut events);
    }

    out
}

fn highlight_code_blocks<'a>(parser: impl IntoIterator<Item = Event<'a>>) -> Vec<Event<'a>> {
    let mut out = Vec::new();
    let mut code_block = None::<CodeBlock>;

    for event in parser {
        match (&mut code_block, event) {
            (None, Event::Start(Tag::CodeBlock(kind))) => {
                code_block = Some(CodeBlock::new(kind));
            }
            (Some(block), Event::Text(text)) => {
                block.code.push_str(&text);
            }
            (Some(block), Event::End(TagEnd::CodeBlock)) => {
                out.push(Event::Html(CowStr::from(block.render())));
                code_block = None;
            }
            (Some(block), event) => {
                block.code.push_str(&event_text(event));
            }
            (None, event) => out.push(event),
        }
    }

    out
}

pub fn anchor_id(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch.to_ascii_lowercase());
        } else if (ch.is_whitespace() || ch == '-') && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-');
    if out.is_empty() {
        "section".to_string()
    } else {
        out.to_string()
    }
}

pub fn unique_anchor_id(text: &str, seen: &mut HashMap<String, usize>) -> String {
    let base = anchor_id(text);
    let count = seen.entry(base.clone()).or_insert(0);
    let id = if *count == 0 {
        base
    } else {
        format!("{base}-{count}")
    };
    *count += 1;
    id
}

fn heading_level_num(level: pulldown_cmark::HeadingLevel) -> u8 {
    match level {
        pulldown_cmark::HeadingLevel::H1 => 1,
        pulldown_cmark::HeadingLevel::H2 => 2,
        pulldown_cmark::HeadingLevel::H3 => 3,
        pulldown_cmark::HeadingLevel::H4 => 4,
        pulldown_cmark::HeadingLevel::H5 => 5,
        pulldown_cmark::HeadingLevel::H6 => 6,
    }
}

struct CodeBlock {
    lang: String,
    code: String,
}

impl CodeBlock {
    fn new(kind: CodeBlockKind<'_>) -> Self {
        let lang = match kind {
            CodeBlockKind::Fenced(info) => info.split_whitespace().next().unwrap_or("").to_string(),
            CodeBlockKind::Indented => String::new(),
        };
        Self {
            lang,
            code: String::new(),
        }
    }

    fn render(&self) -> String {
        code_block_html(&self.lang, &self.code)
    }
}

pub fn code_block_html(lang: &str, code: &str) -> String {
    let lang_class = class_lang(lang);
    let code = if lang.eq_ignore_ascii_case("rust") || lang.eq_ignore_ascii_case("rs") {
        highlight_rust(code)
    } else {
        html_escape(code)
    };
    let label = if lang.is_empty() {
        String::new()
    } else {
        format!(r#"<span class="code-lang">{}</span>"#, html_escape(lang))
    };

    format!(
        r#"<figure class="code-script {lang_class}">{label}<pre><code>{code}</code></pre></figure>"#
    )
}

fn event_text(event: Event<'_>) -> String {
    match event {
        Event::Text(text) | Event::Code(text) | Event::Html(text) | Event::InlineHtml(text) => {
            text.to_string()
        }
        Event::SoftBreak | Event::HardBreak => "\n".to_string(),
        Event::Rule => "---".to_string(),
        _ => String::new(),
    }
}

fn class_lang(lang: &str) -> String {
    let clean = lang
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .collect::<String>();
    if clean.is_empty() {
        "language-text".to_string()
    } else {
        format!("language-{clean}")
    }
}

fn highlight_rust(code: &str) -> String {
    let chars = code.chars().collect::<Vec<_>>();
    let mut out = String::with_capacity(code.len() + code.len() / 3);
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];
        if ch == '/' && chars.get(i + 1) == Some(&'/') {
            let start = i;
            i += 2;
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            push_span(&mut out, "tok-comment", &chars[start..i]);
        } else if ch == '/' && chars.get(i + 1) == Some(&'*') {
            let start = i;
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(chars.len());
            push_span(&mut out, "tok-comment", &chars[start..i]);
        } else if ch == '#' && chars.get(i + 1) == Some(&'[') {
            let start = i;
            i += 2;
            while i < chars.len() && chars[i] != ']' {
                i += 1;
            }
            i = (i + 1).min(chars.len());
            push_span(&mut out, "tok-attr", &chars[start..i]);
        } else if starts_raw_string(&chars, i) {
            let (end, _) = raw_string_end(&chars, i);
            push_span(&mut out, "tok-str", &chars[i..end]);
            i = end;
        } else if ch == '"' {
            let start = i;
            i += 1;
            while i < chars.len() {
                if chars[i] == '\\' {
                    i = (i + 2).min(chars.len());
                } else {
                    let done = chars[i] == '"';
                    i += 1;
                    if done {
                        break;
                    }
                }
            }
            push_span(&mut out, "tok-str", &chars[start..i]);
        } else if ch == '\'' && is_lifetime(&chars, i) {
            let start = i;
            i += 1;
            while i < chars.len() && is_ident_continue(chars[i]) {
                i += 1;
            }
            push_span(&mut out, "tok-life", &chars[start..i]);
        } else if ch == '\'' {
            let start = i;
            i += 1;
            while i < chars.len() {
                if chars[i] == '\\' {
                    i = (i + 2).min(chars.len());
                } else {
                    let done = chars[i] == '\'';
                    i += 1;
                    if done {
                        break;
                    }
                }
            }
            push_span(&mut out, "tok-str", &chars[start..i]);
        } else if ch.is_ascii_digit() {
            let start = i;
            i += 1;
            while i < chars.len()
                && (chars[i].is_ascii_alphanumeric()
                    || chars[i] == '_'
                    || chars[i] == '.'
                    || chars[i] == ':')
            {
                i += 1;
            }
            push_span(&mut out, "tok-num", &chars[start..i]);
        } else if is_ident_start(ch) {
            let start = i;
            i += 1;
            while i < chars.len() && is_ident_continue(chars[i]) {
                i += 1;
            }
            let ident = chars[start..i].iter().collect::<String>();
            if chars.get(i) == Some(&'!') {
                i += 1;
                push_span(&mut out, "tok-macro", &chars[start..i]);
            } else if is_rust_keyword(&ident) {
                push_span(&mut out, "tok-kw", &chars[start..i]);
            } else if is_builtin_type(&ident)
                || ident.chars().next().is_some_and(char::is_uppercase)
            {
                push_span(&mut out, "tok-type", &chars[start..i]);
            } else {
                push_escaped_chars(&mut out, &chars[start..i]);
            }
        } else {
            push_escaped_char(&mut out, ch);
            i += 1;
        }
    }

    out
}

fn starts_raw_string(chars: &[char], i: usize) -> bool {
    if chars.get(i) != Some(&'r') {
        return false;
    }
    let mut j = i + 1;
    while chars.get(j) == Some(&'#') {
        j += 1;
    }
    chars.get(j) == Some(&'"')
}

fn raw_string_end(chars: &[char], i: usize) -> (usize, usize) {
    let mut hash_count = 0;
    let mut j = i + 1;
    while chars.get(j) == Some(&'#') {
        hash_count += 1;
        j += 1;
    }
    j += 1;
    while j < chars.len() {
        if chars[j] == '"' {
            let mut k = j + 1;
            let mut seen = 0;
            while seen < hash_count && chars.get(k) == Some(&'#') {
                seen += 1;
                k += 1;
            }
            if seen == hash_count {
                return (k, hash_count);
            }
        }
        j += 1;
    }
    (chars.len(), hash_count)
}

fn is_lifetime(chars: &[char], i: usize) -> bool {
    chars.get(i + 1).is_some_and(|ch| is_ident_start(*ch)) && chars.get(i + 2) != Some(&'\'')
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn is_rust_keyword(ident: &str) -> bool {
    matches!(
        ident,
        "as" | "async"
            | "await"
            | "break"
            | "const"
            | "continue"
            | "crate"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
    )
}

fn is_builtin_type(ident: &str) -> bool {
    matches!(
        ident,
        "bool"
            | "char"
            | "f32"
            | "f64"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "str"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
    )
}

fn push_span(out: &mut String, class_name: &str, chars: &[char]) {
    out.push_str(r#"<span class=""#);
    out.push_str(class_name);
    out.push_str(r#"">"#);
    push_escaped_chars(out, chars);
    out.push_str("</span>");
}

fn push_escaped_chars(out: &mut String, chars: &[char]) {
    for &ch in chars {
        push_escaped_char(out, ch);
    }
}

fn push_escaped_char(out: &mut String, ch: char) {
    match ch {
        '&' => out.push_str("&amp;"),
        '<' => out.push_str("&lt;"),
        '>' => out.push_str("&gt;"),
        '"' => out.push_str("&quot;"),
        '\'' => out.push_str("&#39;"),
        _ => out.push(ch),
    }
}

fn html_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        push_escaped_char(&mut out, ch);
    }
    out
}
