fn parse_struct_fields(source: &str, struct_name: &str) -> Vec<ScriptField> {
    let lines: Vec<&str> = source.lines().collect();
    let mut struct_line = None;
    for (i, line) in lines.iter().enumerate() {
        if parse_struct_name(line.trim()) == Some(struct_name.to_string()) {
            struct_line = Some(i);
            break;
        }
    }
    let Some(start) = struct_line else {
        return Vec::new();
    };

    let mut fields = Vec::new();
    let mut depth = 0_i32;
    let mut opened = false;
    let mut i = start;

    while i < lines.len() {
        let raw_line = lines[i];
        let line = strip_line_comment(raw_line);
        if !opened {
            if let Some(pos) = line.find('{') {
                opened = true;
                depth = 1;
                let rest = &line[pos + 1..];
                if depth == 1
                    && let Some(field) = parse_field_line(rest)
                {
                    fields.push(field);
                }
                depth += brace_delta(rest);
                if depth <= 0 {
                    break;
                }
            }
            i += 1;
            continue;
        }

        if depth == 1
            && let Some(field) = parse_field_line(line)
        {
            fields.push(field);
        }
        depth += brace_delta(line);
        if depth <= 0 {
            break;
        }
        i += 1;
    }

    fields
}

fn strip_line_comment(line: &str) -> &str {
    line.split("//").next().unwrap_or(line)
}

fn brace_delta(line: &str) -> i32 {
    let opens = line.chars().filter(|c| *c == '{').count() as i32;
    let closes = line.chars().filter(|c| *c == '}').count() as i32;
    opens - closes
}

fn parse_field_line(line: &str) -> Option<ScriptField> {
    let trimmed = line.trim().trim_end_matches(',').trim();
    if trimmed.is_empty()
        || trimmed.starts_with("#[")
        || trimmed.starts_with("///")
        || trimmed.starts_with("//")
    {
        return None;
    }

    let without_vis = if let Some(rest) = trimmed.strip_prefix("pub(") {
        let after = rest.split_once(')')?.1;
        after.trim()
    } else {
        trimmed.trim_start_matches("pub ").trim_start()
    };

    let (name, ty) = without_vis.split_once(':')?;
    let name = name.trim();
    let ty = ty.trim();
    if name.is_empty() || ty.is_empty() || !is_ident(name) {
        return None;
    }

    Some(ScriptField {
        name: name.to_string(),
        ty: ty.to_string(),
    })
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn normalize_type(ty: &str) -> String {
    ty.chars().filter(|c| !c.is_whitespace()).collect()
}

fn supported_fields(fields: &[ScriptField]) -> Vec<ScriptField> {
    fields.to_vec()
}

fn member_const_name(field_name: &str) -> String {
    let mut out = String::from("__PERRO_VAR_");
    for c in field_name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push('_');
        }
    }
    out
}

fn method_const_name(method_name: &str) -> String {
    let mut out = String::from("__PERRO_METHOD_");
    for c in method_name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push('_');
        }
    }
    out
}

#[derive(Clone, Debug)]
struct ScriptMethod {
    name: String,
    takes_raw_params: bool,
    params: Vec<ScriptMethodParam>,
    returns_variant: bool,
}

#[derive(Clone, Debug)]
struct ScriptMethodParam {
    name: String,
    ty: String,
}

fn generate_member_consts(fields: &[ScriptField], methods: &[ScriptMethod]) -> String {
    if fields.is_empty() && methods.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    for field in fields {
        let const_name = member_const_name(&field.name);
        out.push_str(&format!(
            "const {const_name}: ScriptMemberID = var!(\"{}\");\n",
            field.name
        ));
    }
    for method in methods {
        let const_name = method_const_name(&method.name);
        out.push_str(&format!(
            "const {const_name}: ScriptMemberID = func!(\"{}\");\n",
            method.name
        ));
    }
    out
}

fn generate_call_method_body(methods: &[ScriptMethod]) -> String {
    if methods.is_empty() {
        return "        let _ = (method, ctx, params);\n        Variant::Null".to_string();
    }

    let mut out = String::new();
    out.push_str("        match method {\n");
    for method in methods {
        let const_name = method_const_name(&method.name);
        let call = if method.takes_raw_params {
            format!("self.{}(ctx, params)", method.name)
        } else if method.params.is_empty() {
            format!("self.{}(ctx)", method.name)
        } else {
            let args = method
                .params
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!("self.{}(ctx, {args})", method.name)
        };

        let mut prelude = String::new();
        let mut supported = true;
        if !method.takes_raw_params && !method.params.is_empty() {
            for (i, param) in method.params.iter().enumerate() {
                if let Some(binding) = generate_call_param_binding(i, param) {
                    prelude.push_str("                ");
                    prelude.push_str(&binding);
                    prelude.push('\n');
                } else {
                    supported = false;
                    break;
                }
            }
        }

        if !supported {
            out.push_str(&format!(
                "            {const_name} => {{\n                let _ = (ctx, params);\n                Variant::Null\n            }}\n"
            ));
            continue;
        }

        if method.returns_variant {
            out.push_str(&format!(
                "            {const_name} => {{\n{prelude}                {call}\n            }}\n"
            ));
        } else {
            out.push_str(&format!(
                "            {const_name} => {{\n{prelude}                {call};\n                Variant::Null\n            }}\n"
            ));
        }
    }
    out.push_str("            _ => Variant::Null,\n");
    out.push_str("        }");
    out
}
