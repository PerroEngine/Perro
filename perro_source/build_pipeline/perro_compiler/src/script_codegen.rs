/// which vars scenes/anims inject into 1 script; drives scene-arm emit
#[derive(Clone, Debug)]
enum SceneVarUsage {
    /// permissive: scene arms 4 every pub field (tests/tools)
    #[allow(dead_code)]
    AllPub,
    /// static scene analysis: only observed injections get arms
    Exact {
        /// root field names set via .scn script_vars / .panim set_var
        roots: HashSet<String>,
        /// dotted nested member paths observed in injections (wide union)
        paths: HashSet<String>,
    },
}

impl SceneVarUsage {
    fn scene_field(&self, name: &str) -> bool {
        match self {
            SceneVarUsage::AllPub => true,
            SceneVarUsage::Exact { roots, .. } => roots.contains(name),
        }
    }

    fn scene_path(&self, member: &str) -> bool {
        match self {
            SceneVarUsage::AllPub => true,
            SceneVarUsage::Exact { paths, .. } => paths.contains(member),
        }
    }
}

/// permissive wrapper: scene arms 4 all pub fields (test/tool entry)
#[allow(dead_code)]
fn transpile_frontend_script(source: &str, source_include: &str) -> String {
    transpile_frontend_script_with_scene_vars(source, source_include, &SceneVarUsage::AllPub)
}

/// Script type that receives generated dispatch glue, if this source uses the frontend.
pub fn frontend_script_type(source: &str) -> Option<String> {
    if source.contains("impl ScriptBehavior") {
        return None;
    }
    let has_state = parse_marked_struct_name(source, "@State")
        .or_else(|| parse_attributed_struct_name(source, "state"))
        .is_some();
    if !has_state
        && !has_script_macro_invocation(source, "lifecycle")
        && !has_script_macro_invocation(source, "methods")
    {
        return None;
    }
    Some(
        parse_marked_struct_name(source, "@Script")
            .or_else(|| parse_attributed_struct_name(source, "script"))
            .or_else(|| parse_script_macro_target(source, "lifecycle"))
            .or_else(|| parse_script_macro_target(source, "methods"))
            .unwrap_or_else(|| "Script".to_string()),
    )
}

fn transpile_frontend_script_with_scene_vars(
    source: &str,
    source_include: &str,
    scene_vars: &SceneVarUsage,
) -> String {
    let debug_methods = methods_debug_enabled();
    let source = ensure_script_allows(source);
    let source_include = escape_str(&normalize_generated_include_path(source_include));
    let Some(script_ty) = frontend_script_type(&source) else {
        return format!("include!(\"{source_include}\");\n");
    };
    let stripped_source = strip_transpiler_attributes(&source);

    let state_ty = parse_marked_struct_name(&source, "@State")
        .or_else(|| parse_attributed_struct_name(&source, "state"));
    let has_lifecycle_macro = has_script_macro_invocation(&source, "lifecycle");
    let needs_implicit_script_struct = parse_marked_struct_name(&source, "@Script").is_none()
        && parse_attributed_struct_name(&source, "script").is_none()
        && parse_named_struct(&stripped_source, &script_ty).is_none()
        && !has_lifecycle_macro;
    let script_ctor_expr =
        if needs_implicit_script_struct || is_unit_struct(&stripped_source, &script_ty) {
            script_ty.clone()
        } else {
            format!("<{script_ty} as Default>::default()")
        };
    let state_ty = state_ty.unwrap_or_else(|| "()".to_string());
    let state_ctor_expr = if state_ty == "()" {
        "()".to_string()
    } else {
        format!("<{state_ty} as Default>::default()")
    };

    let has_init = has_nonempty_lifecycle_method(&source, &script_ty, "on_init");
    let has_start = has_nonempty_lifecycle_method(&source, &script_ty, "on_all_init");
    let has_update = has_nonempty_lifecycle_method(&source, &script_ty, "on_update");
    let has_fixed = has_nonempty_lifecycle_method(&source, &script_ty, "on_fixed_update");
    let has_removal = has_nonempty_lifecycle_method(&source, &script_ty, "on_removal");
    let user_methods = parse_inherent_methods(&source, &script_ty);
    if debug_methods {
        let method_names = user_methods
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!(
            "[perro][methods] source={} script_ty={} methods_found={} [{}]",
            source_include,
            script_ty,
            user_methods.len(),
            method_names
        );
        if user_methods.is_empty() && source.contains("methods!(") {
            eprintln!(
                "[perro][methods][warn] methods! macro exists but zero methods were parsed for source={}",
                source_include
            );
        }
    }
    let state_fields = if state_ty == "()" {
        Vec::new()
    } else {
        parse_struct_fields(&source, &state_ty)
    };
    let exposed_fields = state_fields;
    let nested_fields = parse_local_nested_fields(&source, &exposed_fields);
    // pub-only sets drive ALL glue: get/set/call + scene inject.
    // non-pub members = internal only (with_state!/self.method), zero glue.
    let public_fields: Vec<ScriptField> = exposed_fields
        .iter()
        .filter(|field| field.is_pub)
        .cloned()
        .collect();
    let public_nested: Vec<NestedScriptField> = nested_fields
        .iter()
        .filter(|field| field.is_pub)
        .cloned()
        .collect();
    // scene arms: pub + observed injected (static scene analysis);
    // runtime spawns w/ vars ride the scene-match `_` -> set_var fallback
    let scene_fields: Vec<ScriptField> = public_fields
        .iter()
        .filter(|field| scene_vars.scene_field(&field.name))
        .cloned()
        .collect();
    let public_root_names: HashSet<&str> = public_fields
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    let scene_nested: Vec<NestedScriptField> = nested_fields
        .iter()
        .filter(|field| {
            let root = field.member.split('.').next().unwrap_or(&field.member);
            public_root_names.contains(root) && scene_vars.scene_path(&field.member)
        })
        .cloned()
        .collect();
    let public_methods: Vec<ScriptMethod> = user_methods
        .iter()
        .filter(|method| method.is_pub)
        .cloned()
        .collect();
    if debug_methods {
        for method in &user_methods {
            if !method.is_pub {
                eprintln!(
                    "[perro][methods][skip] not pub, no call_method arm | method=`{}`",
                    method.name
                );
            }
        }
    }

    let mut flags = String::from("ScriptFlags::NONE");
    if has_init {
        flags.push_str(" | ScriptFlags::HAS_INIT");
    }
    if has_start {
        flags.push_str(" | ScriptFlags::HAS_ALL_INIT");
    }
    if has_update {
        flags.push_str(" | ScriptFlags::HAS_UPDATE");
    }
    if has_fixed {
        flags.push_str(" | ScriptFlags::HAS_FIXED_UPDATE");
    }
    if has_removal {
        flags.push_str(" | ScriptFlags::HAS_REMOVAL");
    }

    // consts: pub members only; scene nested paths add their consts
    let mut const_nested = public_nested.clone();
    for field in &scene_nested {
        if !const_nested
            .iter()
            .any(|known| known.member == field.member)
        {
            const_nested.push(field.clone());
        }
    }
    let member_consts = generate_member_consts(&public_fields, &const_nested, &public_methods);
    let state_cast_helpers = generate_state_cast_helpers(&state_ty, &public_fields);
    let get_var_body = generate_get_var_body(&public_fields, &public_nested);
    let set_var_match_fn = generate_var_match_fns(
        &state_ty,
        &public_fields,
        &scene_fields,
        &public_nested,
        &scene_nested,
    );
    let set_var_body = generate_set_var_body(&public_fields);
    let has_scene_arms = !scene_fields.is_empty() || !scene_nested.is_empty();
    let apply_scene_injected_vars_body =
        generate_apply_scene_injected_vars_body(has_scene_arms, !public_fields.is_empty());
    let call_method_body = generate_call_method_body(&public_methods);

    let implicit_script_decl = if needs_implicit_script_struct {
        format!("#[derive(Default)]\nstruct {script_ty};\n\n")
    } else {
        String::new()
    };

    format!(
        r#"{implicit_script_decl}include!("{source_include}");

// ---- AUTO-GENERATED by Perro Compiler ----
{member_consts}
{state_cast_helpers}
{set_var_match_fn}

impl<API: ScriptAPI + ?Sized> ScriptBehavior<API> for {script_ty} {{
    fn script_flags(&self) -> ScriptFlags {{
        ScriptFlags::new({flags})
    }}

    fn create_state(&self) -> Box<dyn std::any::Any> {{
        Box::new({state_ctor_expr})
    }}

    fn get_var(&self, state: &dyn std::any::Any, var: ScriptMemberID) -> Variant {{
{get_var_body}
    }}

    fn set_var(&self, state: &mut dyn std::any::Any, var: ScriptMemberID, value: Variant) {{
{set_var_body}
    }}

    fn apply_scene_injected_vars(
        &self,
        state: &mut dyn std::any::Any,
        vars: Vec<(ScriptMemberID, Variant)>,
        resolver: &mut dyn perro_api::variant::SceneVariantResolver,
    ) {{
{apply_scene_injected_vars_body}
    }}

    fn call_method(
        &self,
        method: ScriptMemberID,
        ctx: &mut ScriptContext<'_, API>,
        params: &[Variant],
    ) -> Variant {{
{call_method_body}
    }}
}}

pub(crate) fn perro_create_script() -> *mut dyn ScriptBehavior<crate::RuntimeScriptApi> {{
    let script: Box<dyn ScriptBehavior<crate::RuntimeScriptApi>> =
        Box::new({script_ctor_expr});
    Box::into_raw(script)
}}

#[cfg(feature = "dynamic-scripts")]
#[allow(improper_ctypes_definitions)]
pub(crate) extern "C" fn perro_create_script_dynamic() -> *mut dyn ScriptBehavior<crate::RuntimeScriptApi> {{
    perro_create_script()
}}
"#
    )
}

fn transpiled_exports_script_ctor(transpiled: &str) -> bool {
    transpiled.contains("fn perro_create_script(")
}

fn ensure_script_allows(source: &str) -> String {
    if source.contains("#![allow(unused_imports")
        || source.contains("#![allow(unused_variables")
        || source.contains("#![allow(dead_code")
    {
        return source.to_string();
    }
    format!("#![allow(unused_imports, unused_variables, dead_code)]\n{source}")
}

fn strip_transpiler_attributes(source: &str) -> String {
    let mut out = String::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        if is_transpiler_attr_line(trimmed) {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn parse_marked_struct_name(source: &str, marker: &str) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    for i in 0..lines.len() {
        let l = lines[i].trim();
        if !(l == format!("///{marker}") || l == format!("//{marker}")) {
            continue;
        }
        for next in lines.iter().skip(i + 1) {
            let n = next.trim();
            if n.is_empty() {
                continue;
            }
            if let Some(name) = parse_struct_name(n) {
                return Some(name);
            }
        }
    }
    None
}

fn has_nonempty_lifecycle_method(source: &str, script_ty: &str, method_name: &str) -> bool {
    let Ok(file) = syn::parse_file(source) else {
        return false;
    };
    file.items.iter().any(|item| match item {
        syn::Item::Impl(item_impl)
            if item_impl.trait_.is_none() && impl_targets_type(item_impl, script_ty) =>
        {
            item_impl
                .items
                .iter()
                .any(|item| impl_item_has_lifecycle_body(item, method_name))
        }
        syn::Item::Macro(item_macro) if item_macro.mac.path.is_ident("lifecycle") => {
            syn::parse2::<LifecycleBody>(item_macro.mac.tokens.clone()).is_ok_and(|body| {
                body.0
                    .iter()
                    .any(|item| impl_item_has_lifecycle_body(item, method_name))
            })
        }
        _ => false,
    })
}

struct LifecycleBody(Vec<syn::ImplItem>);

impl syn::parse::Parse for LifecycleBody {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        let content;
        syn::braced!(content in input);
        let mut items = Vec::new();
        while !content.is_empty() {
            items.push(content.parse()?);
        }
        Ok(Self(items))
    }
}

fn impl_targets_type(item_impl: &syn::ItemImpl, script_ty: &str) -> bool {
    let syn::Type::Path(path) = item_impl.self_ty.as_ref() else {
        return false;
    };
    path.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == script_ty)
}

fn impl_item_has_lifecycle_body(item: &syn::ImplItem, method_name: &str) -> bool {
    let syn::ImplItem::Fn(method) = item else {
        return false;
    };
    method.sig.ident == method_name
        && method
            .sig
            .receiver()
            .is_some_and(|receiver| receiver.reference.is_some())
        && method.sig.inputs.iter().any(|arg| match arg {
            syn::FnArg::Typed(arg) => type_mentions_ident(&arg.ty, "ScriptContext"),
            syn::FnArg::Receiver(_) => false,
        })
        && !method.block.stmts.is_empty()
}

fn type_mentions_ident(ty: &syn::Type, expected: &str) -> bool {
    match ty {
        syn::Type::Path(path) => path.path.segments.iter().any(|part| part.ident == expected),
        syn::Type::Reference(reference) => type_mentions_ident(&reference.elem, expected),
        syn::Type::Group(group) => type_mentions_ident(&group.elem, expected),
        syn::Type::Paren(paren) => type_mentions_ident(&paren.elem, expected),
        _ => false,
    }
}

fn parse_named_struct(source: &str, expected: &str) -> Option<String> {
    for line in source.lines() {
        if let Some(name) = parse_struct_name(line.trim())
            && name == expected
        {
            return Some(name);
        }
    }
    None
}

fn parse_struct_name(line: &str) -> Option<String> {
    let line = line.trim_start_matches("pub ").trim_start();
    if !line.starts_with("struct ") {
        return None;
    }
    let rest = line.trim_start_matches("struct ").trim_start();
    let mut name = String::new();
    for c in rest.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            name.push(c);
        } else {
            break;
        }
    }
    if name.is_empty() { None } else { Some(name) }
}

fn is_unit_struct(source: &str, struct_name: &str) -> bool {
    source.lines().any(|line| {
        let line = line.trim();
        let line = line.trim_start_matches("pub ").trim_start();
        line == format!("struct {struct_name};")
    })
}

#[derive(Clone, Debug)]
struct ScriptField {
    name: String,
    ty: String,
    /// any `pub` form (`pub`, `pub(crate)`, ...) -> exposed 2 other scripts
    is_pub: bool,
}

#[derive(Clone, Debug)]
struct NestedScriptField {
    member: String,
    access: String,
    ty: String,
    /// root field + every path segment pub -> exposed 2 other scripts
    is_pub: bool,
}

fn parse_local_nested_fields(source: &str, state_fields: &[ScriptField]) -> Vec<NestedScriptField> {
    let mut local_structs = HashMap::<String, Vec<ScriptField>>::new();
    for line in source.lines() {
        let Some(name) = parse_struct_name(line.trim()) else {
            continue;
        };
        local_structs
            .entry(name.clone())
            .or_insert_with(|| parse_struct_fields(source, &name));
    }

    let mut out = Vec::new();
    for field in state_fields {
        let mut stack = HashSet::new();
        collect_local_nested_fields(
            &local_structs,
            local_type_name(&field.ty),
            &field.name,
            &field.name,
            field.is_pub,
            &mut stack,
            &mut out,
        );
    }
    out
}

fn collect_local_nested_fields(
    local_structs: &HashMap<String, Vec<ScriptField>>,
    ty: Option<&str>,
    member_prefix: &str,
    access_prefix: &str,
    prefix_is_pub: bool,
    stack: &mut HashSet<String>,
    out: &mut Vec<NestedScriptField>,
) {
    let Some(ty) = ty else {
        return;
    };
    let Some(fields) = local_structs.get(ty) else {
        return;
    };
    if fields.is_empty() || !stack.insert(ty.to_string()) {
        return;
    }

    for field in fields {
        let member = format!("{member_prefix}.{}", field.name);
        let access = format!("{access_prefix}.{}", field.name);
        let is_pub = prefix_is_pub && field.is_pub;
        let child_ty = local_type_name(&field.ty);
        let child_is_local_struct = child_ty
            .and_then(|name| local_structs.get(name))
            .is_some_and(|fields| !fields.is_empty());
        if child_is_local_struct {
            collect_local_nested_fields(
                local_structs,
                child_ty,
                &member,
                &access,
                is_pub,
                stack,
                out,
            );
        } else {
            out.push(NestedScriptField {
                member,
                access,
                ty: normalize_type(&field.ty),
                is_pub,
            });
        }
    }

    stack.remove(ty);
}

fn local_type_name(ty: &str) -> Option<&str> {
    let ty = ty.trim();
    if ty.is_empty() || ty.contains(['<', '[', '(', '&']) {
        return None;
    }
    ty.rsplit("::").next()
}
