// scripting/lang/codegen/rust.rs
#![allow(unused)]
#![allow(dead_code)]
use std::{fmt::format, fs, path::{Path, PathBuf}};
use std::fmt::Write as _;
use std::collections::HashMap;
use std::cell::RefCell;

use regex::Regex;

use crate::{asset_io::{ProjectRoot, get_project_root}, lang::{api_modules::*, ast::*}, prelude::string_to_u64, script::Var};

// ============================================================================
// Type Inference Cache - Dramatically speeds up repeated type lookups
// ============================================================================

thread_local! {
    static TYPE_CACHE: RefCell<HashMap<usize, Option<Type>>> = RefCell::new(HashMap::new());
}

fn expr_cache_key(expr: &Expr) -> usize {
    expr as *const Expr as usize
}

fn clear_type_cache() {
    TYPE_CACHE.with(|cache| cache.borrow_mut().clear());
}

fn to_pascal_case(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    s.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect()
}

impl Script {

    pub fn infer_map_key_type(
        &self,
        map_expr: &Expr,
        current_func: Option<&Function>,
    ) -> Option<Type> {
        self.infer_expr_type(map_expr, current_func)
            .and_then(|t| match t {
                Type::Container(ContainerKind::Map, ref types) => types.get(0).cloned(),
                _ => None,
            })
    }
    pub fn infer_map_value_type(
        &self,
        map_expr: &Expr,
        current_func: Option<&Function>,
    ) -> Option<Type> {
        self.infer_expr_type(map_expr, current_func)
            .and_then(|t| match t {
                Type::Container(ContainerKind::Map, ref types) => types.get(1).cloned(),
                _ => None,
            })
    }

    pub fn get_struct_field_type(&self, struct_name: &str, field_name: &str) -> Option<Type> {
        self.structs
            .iter()
            .find(|s| s.name == struct_name)
            .and_then(|s| {
                s.fields
                    .iter()
                    .find(|f| f.name == field_name)
                    .map(|f| f.typ.clone())
            })
    }

    pub fn generate_implicit_cast_for_expr(&self, expr: &str, from: &Type, to: &Type) -> String {
        use Type::*;
        if from == to {
            return expr.to_string();
        }

        // Create a temporary Cast expression and use its to_rust method
        // to leverage the comprehensive casting logic in Expr::Cast.
        let temp_expr = Expr::Cast(
            Box::new(Expr::Ident(expr.to_string())), // Wrap original expr as an Ident for now
            to.clone(),
        );
        temp_expr.to_rust(false, self, Some(to), None) // Assume no self/func context for these implicit casts
    }

    pub fn is_struct_field(&self, name: &str) -> bool {
        self.variables.iter().any(|v| v.name == name)
    }

    pub fn get_variable_type(&self, name: &str) -> Option<&Type> {
        self.variables.iter().find(|v| v.name == name).and_then(|v| v.typ.as_ref())
    }

pub fn infer_expr_type(
    &self,
    expr: &Expr,
    current_func: Option<&Function>,
) -> Option<Type> {
    use Type::*;

    // 🔹 check cache first for performance
    let cache_key = expr as *const Expr as usize;
    if let Some(cached) = TYPE_CACHE.with(|cache| cache.borrow().get(&cache_key).cloned()) {
        return cached;
    }

    let result = match expr {
        Expr::Literal(lit) => self.infer_literal_type(lit, None),
        Expr::Ident(name) => {
                if let Some(func) = current_func {
                    // 1. Local variable
                    if let Some(local) = func.locals.iter().find(|v| v.name == *name) {
                        if let Some(t) = &local.typ {
                            Some(t.clone())
                        } else if let Some(val) = &local.value {
                            self.infer_expr_type(&val.expr, current_func)
                        } else {
                            None
                        }
                    }
                    // 2. Function parameter
                    else if let Some(param) = func.params.iter().find(|p| p.name == *name) {
                        Some(param.typ.clone())
                    }
                    // 3. Script-level variable or exposed field
                    else {
                        self.get_variable_type(name).cloned()
                    }
                } else {
                    self.get_variable_type(name).cloned()
                }
            }
        Expr::BinaryOp(left, _op, right) => {
                let left_type = self.infer_expr_type(left, current_func);
                let right_type = self.infer_expr_type(right, current_func);

                match (&left_type, &right_type) {
                    (Some(l), Some(r)) if l == r => Some(l.clone()),
                    (Some(l), Some(r)) => self.promote_types(l, r),
                    (Some(l), None) => Some(l.clone()),
                    (None, Some(r)) => Some(r.clone()),
                    _ => Some(Number(NumberKind::Float(32))), // fallback type
                }
            }
        Expr::MemberAccess(base, field) => {
                let base_type = self.infer_expr_type(base, current_func)?;
                self.get_member_type(&base_type, field)
            }
        Expr::Call(target, _args) => match &**target {
                Expr::Ident(fname) => self.get_function_return_type(fname),
                Expr::MemberAccess(base, method) => {
                    let base_type = self.infer_expr_type(base, current_func)?;
                    if let Type::Custom(type_name) = base_type {
                        if type_name == self.node_type {
                            self.get_function_return_type(method)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            },
        Expr::Cast(_, target_type) => Some(target_type.clone()),
        Expr::ApiCall(api, args) => match api {
            ApiModule::MapOp(MapApi::Get) => {
                if let Some(Type::Container(ContainerKind::Map, ref params)) = self.infer_expr_type(&args[0], current_func) {
                    return params.get(1).cloned(); // value type
                }
                Some(Type::Object)
            }
            ApiModule::ArrayOp(ArrayApi::Pop) => {
                if let Some(Type::Container(ContainerKind::Array, ref params)) = self.infer_expr_type(&args[0], current_func) {
                    return params.get(0).cloned(); // element type
                }
                Some(Type::Object)
            }
            // ... other API cases ...
            _ => api.return_type(),
        }
        Expr::StructNew(ty_name, _fields) => {
                // The result of `new Struct(...)` is `Type::Custom("Struct")`
                Some(Custom(ty_name.clone()))
            }
        Expr::SelfAccess => Some(Custom(self.node_type.clone())),
        Expr::ObjectLiteral(_) => Some(Type::Object),
        Expr::ContainerLiteral(kind, _) => match kind {
                ContainerKind::Array    => Some(Type::Container(ContainerKind::Array, vec![Type::Object])),
                ContainerKind::Map  => Some(Type::Container(ContainerKind::Map, vec![Type::String, Type::Object])),
                ContainerKind::FixedArray(_) => Some(Type::Container(kind.clone(), vec![Type::Object])),
            },
        Expr::Index(base, _key) => {
                let base_type = self.infer_expr_type(base, current_func)?;

                match base_type {
                    // Case 1: Base is a Container (Array, Map, FixedArray)
                    Type::Container(container_kind, inner_types) => {
                        match container_kind {
                            ContainerKind::Array => {
                                if inner_types.first() == Some(&Type::Object) {
                                    Some(Type::Object) // Dynamic array, elements are Value
                                } else {
                                    inner_types.first().cloned() // Typed array, elements are the inner type
                                }
                            }
                            ContainerKind::Map => {
                                if inner_types.last() == Some(&Type::Object) {
                                    Some(Type::Object) // Dynamic map, values are Value
                                } else {
                                    inner_types.last().cloned() // Typed map, values are the inner type
                                }
                            }
                            ContainerKind::FixedArray(_) => { // Fixed size does not affect element type
                                inner_types.first().cloned() // Fixed array, elements are the inner type
                            }
                        }
                    }
                    // Case 2: Base is a dynamic Object (serde_json::Value)
                    Type::Object => Some(Type::Object),

                    // Case 3: Any other type (e.g., custom struct that might deref, but no direct indexing support at this AST level)
                    _ => None, // Or use self.infer_map_value_type(base, current_func) if you want to be very lenient
                }
            }, // <-- This comma is important.

        Expr::BaseAccess => Some(Custom(self.node_type.clone())),
        _ => None,
    };

    // ✅ Cache the result
    TYPE_CACHE.with(|cache| {
        cache.borrow_mut().insert(cache_key, result.clone());
    });

    result
}

    fn infer_literal_type(&self, lit: &Literal, expected_type: Option<&Type>) -> Option<Type> {
        match lit {
            Literal::Number(_) => {
                if let Some(expected) = expected_type {
                    Some(expected.clone())
                } else {
                    Some(Type::Number(NumberKind::Float(32)))
                }
            }
            Literal::Bool(_) => Some(Type::Bool),
            Literal::String(_) | Literal::Interpolated(_) => Some(Type::String),
        }
    }

    fn promote_types(&self, left: &Type, right: &Type) -> Option<Type> {
        // Fast path for identical types
        if left == right {
            return Some(left.clone());
        }

        match (left, right) {
            (Type::Number(NumberKind::BigInt), Type::Number(_))
            | (Type::Number(_), Type::Number(NumberKind::BigInt)) => {
                Some(Type::Number(NumberKind::BigInt))
            }

            (Type::Number(NumberKind::Decimal), Type::Number(_))
            | (Type::Number(_), Type::Number(NumberKind::Decimal)) => {
                Some(Type::Number(NumberKind::Decimal))
            }

            (Type::Number(NumberKind::Float(w1)), Type::Number(NumberKind::Float(w2))) => {
                Some(Type::Number(NumberKind::Float(*w1.max(w2))))
            }

            (Type::Number(NumberKind::Float(w)), Type::Number(_))
            | (Type::Number(_), Type::Number(NumberKind::Float(w))) => {
                Some(Type::Number(NumberKind::Float(*w)))
            }

            (Type::Number(NumberKind::Signed(w1)), Type::Number(NumberKind::Unsigned(w2)))
            | (Type::Number(NumberKind::Unsigned(w2)), Type::Number(NumberKind::Signed(w1))) => {
                Some(Type::Number(NumberKind::Signed(u8::max(*w1, *w2))))
            }

            (Type::Number(NumberKind::Signed(w1)), Type::Number(NumberKind::Signed(w2))) => {
                Some(Type::Number(NumberKind::Signed(*w1.max(w2))))
            }

            (Type::Number(NumberKind::Unsigned(w1)), Type::Number(NumberKind::Unsigned(w2))) => {
                Some(Type::Number(NumberKind::Unsigned(*w1.max(w2))))
            }

            _ => Some(left.clone()),
        }
    }
    
    fn get_member_type(&self, base_type: &Type, member: &str) -> Option<Type> {
    fn get_struct_field_type_recursive<'a>(
        structs: &'a [StructDef],
        struct_name: &str,
        field_name: &str,
    ) -> Option<Type> {
        let struct_def = structs.iter().find(|s| s.name == struct_name)?;
        
        // (1) Check direct fields
        if let Some(f) = struct_def.fields.iter().find(|f| f.name == field_name) {
            return Some(f.typ.clone());
        }

        // (2) If base exists, recurse upward
        if let Some(ref base_name) = struct_def.base {
            if let Some(basedef) = structs.iter().find(|b| &b.name == base_name) {
                return get_struct_field_type_recursive(structs, base_name, field_name);
            }
        }
        None
    }

    match base_type {
        // --- For custom structs ---
        Type::Custom(type_name) => {
            if type_name == &self.node_type {
                // script-level node fields (like `self.energy` if exposed)
                if let Some(var) = self.variables.iter().find(|v| v.name == member) {
                    return var.typ.clone();
                }
            }

            // Now: recursive base traversal for any struct
            get_struct_field_type_recursive(&self.structs, type_name, member)
        }

        // Container/Primitive types don’t support `.member`
        _ => None,
    }
}

    fn get_function_return_type(&self, func_name: &str) -> Option<Type> {
        self.functions
            .iter()
            .find(|f| f.name == func_name)
            .map(|f| f.return_type.clone())
    }


  pub fn to_rust(&mut self, struct_name: &str, project_path: &Path, current_func: Option<&Function>, verbose: bool) -> String {
        self.verbose = verbose;
        // Clear cache at the start of codegen
        clear_type_cache();

         let mut script = self.clone();
        // 🔹 Analyze self usage and call propagation before codegen
        analyze_self_usage(&mut script);
        
        let mut out = String::with_capacity(8192); // Pre-allocate larger buffer
        let pascal_struct_name = to_pascal_case(struct_name);

        // Headers
        out.push_str("#![allow(improper_ctypes_definitions)]\n");
        out.push_str("#![allow(unused)]\n\n");
        out.push_str("use std::any::Any;\n");
        out.push_str("use std::collections::HashMap;\n");
        out.push_str("use smallvec::{SmallVec, smallvec};\n");
        out.push_str("use serde_json::{Value, json};\n");
        out.push_str("use serde::{Serialize, Deserialize};\n");
        out.push_str("use uuid::Uuid;\n");
        out.push_str("use std::ops::{Deref, DerefMut};\n");
        out.push_str("use rust_decimal::{Decimal, prelude::*};\n");
        out.push_str("use num_bigint::BigInt;\n");
        out.push_str("use std::str::FromStr;\n");
        out.push_str("use std::{rc::Rc, cell::RefCell};\n\n");
        out.push_str("use perro_core::prelude::*;\n\n");

        out.push_str("//=======================================;\n");
        out.push_str("// Auto Generated by Perro Transpiler [Any further edits to this file will be overwritten on next transile];\n");
        out.push_str("//=======================================;\n\n");

        // The `script.script_vars` is now the single, authoritative, and ordered list
        // of all script-level variables as they appeared in the Pup source.
        let all_script_vars = &script.variables; 

        // ========================================================================
        // {} - Main Script Structure
        // ========================================================================

        out.push_str("// ========================================================================\n");
        write!(out, "// {} - Main Script Structure\n", pascal_struct_name).unwrap();
        out.push_str("// ========================================================================\n\n");

        write!(out, "pub struct {}Script {{\n", pascal_struct_name).unwrap();
        write!(out, "    node: {},\n", script.node_type).unwrap();

        // Use `all_script_vars` for defining struct fields to ensure the correct order
        for var in all_script_vars {
            write!(out, "    {}: {},\n", var.name, var.rust_type()).unwrap();
        }

        out.push_str("}\n\n");

       out.push_str("// ========================================================================\n");
        write!(
            out,
            "// {} - Creator Function (FFI Entry Point)\n",
            pascal_struct_name
        )
        .unwrap();
        out.push_str("// ========================================================================\n\n");

        // Emit FFI header
        out.push_str("#[unsafe(no_mangle)]\n");
        write!(
            out,
            "pub extern \"C\" fn {}_create_script() -> *mut dyn ScriptObject {{\n",
            struct_name.to_lowercase()
        )
        .unwrap();

        // Optional: handle node init
        if self.node_type == "Node" {
            write!(
                out,
                "    let node = {}::new(\"{}\", None);\n",
                script.node_type, pascal_struct_name
            )
            .unwrap();
        } else {
            write!(
                out,
                "    let node = {}::new(\"{}\");\n",
                script.node_type, pascal_struct_name
            )
            .unwrap();
        }

        // -----------------------------------------------------
        // 1. Emit local variable predefinitions for all fields
        //    (Crucially, iterate in dependency order using `all_script_vars`)
        // -----------------------------------------------------
        for var in all_script_vars { // Direct use of `all_script_vars`
            let name = &var.name;
            let mut init_code = var
                .rust_initialization(&script, current_func);

            if init_code.contains("self.") {
                init_code = init_code.replace("self.", "");
            }

            let re_ident = Regex::new(r"\b([A-Za-z_][A-Za-z0-9_]*)\b").unwrap();

            // track which other variables this initializer mentions
            let mut referenced_vars = Vec::new();
            for cap in re_ident.captures_iter(&init_code) {
                let ref_name = cap[1].to_string();

                // skip self-reference and Rust keywords and explicit types/constructors
                // Ensure we only process variables that are *actual* variable references,
                // not keywords or type names that happen to match part of the regex.
                if ref_name == *name
                   || !all_script_vars.iter().any(|v| v.name == ref_name) // Check against all_script_vars for proper dependency
                    || !ref_name.chars().next().map_or(false, |c| c.is_lowercase()) // Simple heuristic: referenced variables are lowercase, types are PascalCase
                    || ["let", "mut", "new", "HashMap", "vec", "json"].contains(&ref_name.as_str()) // Rust keywords/macros
                {
                    continue;
                }

                referenced_vars.push(ref_name);
            }

            // ------------------------------
            // 3. If any referenced variable is non-Copy, ensure ".clone()"
            // ------------------------------
            for ref_name in referenced_vars {
                if let Some(ref_type) = script.get_variable_type(&ref_name) {
                    if ref_type.requires_clone() {
                        // Replace *bare identifier* occurrences with `.clone()`
                        let re_replace =
                            Regex::new(&format!(r"\b{}\b", regex::escape(&ref_name))).unwrap();
                        // Prevent double-cloning if `init_code` already has it (e.g., from `json!(var.clone())`)
                        // This check is a heuristic; more robust would be to track expression types.
                        if !init_code.contains(&format!("{}.clone()", ref_name)) {
                             init_code = re_replace
                                .replace_all(&init_code, format!("{}.clone()", ref_name))
                                .to_string();
                        }
                    }
                }
            }
                
            // Predeclare variable instead of inline it
            write!(out, "    let {} = {};\n", name, init_code).unwrap();
        }

        // -----------------------------------------------------
        // 2. Emit actual struct construction
        // -----------------------------------------------------
        write!(
            out,
            "\n    Box::into_raw(Box::new({}Script {{\n",
            pascal_struct_name
        )
        .unwrap();

        // Fill in struct fields using locals (safe to reference one another now)
        write!(out, "        node,\n").unwrap();
        // Use `all_script_vars` here again for consistent ordering
        for var in all_script_vars {
            write!(out, "        {},\n", var.name).unwrap();
        }

        out.push_str("    })) as *mut dyn ScriptObject\n");
        out.push_str("}\n\n");

        if !script.structs.is_empty() {
            out.push_str("// ========================================================================\n");
            out.push_str("// Supporting Struct Definitions\n");
            out.push_str("// ========================================================================\n\n");
            
            for s in &script.structs {
                out.push_str(&s.to_rust_definition(&script));
                out.push_str("\n\n");
            }
        }

        out.push_str("// ========================================================================\n");
        write!(out, "// {} - Script Init & Update Implementation\n", pascal_struct_name).unwrap();
        out.push_str("// ========================================================================\n\n");

        write!(out, "impl Script for {}Script {{\n", pascal_struct_name).unwrap();

        for func in &script.functions {
            if func.is_trait_method {
                out.push_str(&func.to_rust_trait_method(&script.node_type, &script));
            }
        }
        out.push_str("}\n\n");

        let helpers: Vec<_> = script.functions.iter().filter(|f| !f.is_trait_method).collect();
        if !helpers.is_empty() {
            out.push_str("// ========================================================================\n");
            write!(out, "// {} - Script-Defined Methods\n", pascal_struct_name).unwrap();
            out.push_str("// ========================================================================\n\n");

            write!(out, "impl {}Script {{\n", pascal_struct_name).unwrap();
            for func in helpers {
                out.push_str(&func.to_rust_method(&script.node_type, &script));
            }
            out.push_str("}\n\n");
        }

        out.push_str(&implement_script_boilerplate(
            &format!("{}Script", pascal_struct_name),
            &script.variables, // Pass the unified list for exposed vars
            &script.functions
        ));

        if let Err(e) = write_to_crate(&project_path, &out, struct_name) {
            eprintln!("Warning: Failed to write to crate: {}", e);
        }

        out
    }
}

fn analyze_self_usage(script: &mut Script) {
    // Step 1: mark direct `self` usage
    for func in &mut script.functions {
        func.uses_self = func.body.iter().any(|stmt| stmt.contains_self());
    }

    // Step 2: track which functions call which others
    let mut edges: HashMap<String, Vec<String>> = HashMap::new();
    for func in &script.functions {
        let callees = extract_called_functions(&func.body);
        edges.insert(func.name.clone(), callees);
    }

    // Step 3: recursively propagate self usage through the call graph
   let mut changed = true;
while changed {
    changed = false;

    // Take a snapshot of current function states (immutable copy)
    let snapshot: Vec<(String, bool)> = script
        .functions
        .iter()
        .map(|f| (f.name.clone(), f.uses_self))
        .collect();

    for func in &mut script.functions {
        if !func.uses_self {
            if let Some(callees) = edges.get(&func.name) {
                if callees.iter().any(|callee_name| {
                    snapshot
                        .iter()
                        .any(|(name, uses_self)| name == callee_name && *uses_self)
                }) {
                    func.uses_self = true;
                    changed = true;
                }
            }
        }
    }
}
}

fn extract_called_functions(stmts: &[Stmt]) -> Vec<String> {
    fn recurse_expr(expr: &Expr) -> Vec<String> {
        match expr {
            Expr::Call(target, _) => {
                let mut v = Vec::new();
                if let Some(name) = Expr::get_target_name(target) {
                    v.push(name.to_string());
                }
                v.extend(recurse_expr(target));
                v
            }
            Expr::BinaryOp(l, _, r) => {
                let mut v = recurse_expr(l);
                v.extend(recurse_expr(r));
                v
            }
            Expr::MemberAccess(b, _) => recurse_expr(b),
            _ => vec![],
        }
    }

    let mut out = Vec::new();
    for s in stmts {
        match s {
            Stmt::Expr(e) => out.extend(recurse_expr(&e.expr)),
            Stmt::Assign(_, e) | Stmt::AssignOp(_, _, e) => out.extend(recurse_expr(&e.expr)),
            Stmt::MemberAssign(l, r) | Stmt::MemberAssignOp(l, _, r) => {
                out.extend(recurse_expr(&l.expr));
                out.extend(recurse_expr(&r.expr));
            }
            Stmt::VariableDecl(v) => {
                if let Some(init) = &v.value {
                    out.extend(recurse_expr(&init.expr));
                }
            }
            _ => {}
        }
    }
    out
}

impl StructDef {
    pub fn to_rust_definition(&self, script: &Script) -> String {
        let mut out = String::with_capacity(1024);

        // === Struct Definition ===
        writeln!(
            out,
            "#[derive(Default, Debug, Clone, Serialize, Deserialize)]"
        )
        .unwrap();
        writeln!(out, "pub struct {} {{", self.name).unwrap();

        if let Some(base) = &self.base {
            writeln!(out, "    pub base: {},", base).unwrap();
        }

        for field in &self.fields {
            writeln!(
                out,
                "    pub {}: {},",
                field.name,
                field.typ.to_rust_type()
            )
            .unwrap();
        }

        writeln!(out, "}}\n").unwrap();

        // === Display Implementation ===
        writeln!(out, "impl std::fmt::Display for {} {{", self.name).unwrap();
        writeln!(
            out,
            "    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{"
        )
        .unwrap();
        writeln!(out, "        write!(f, \"{{{{ \")?;").unwrap();

        // --- flatten base display if present ---
        if let Some(_base) = &self.base {
            writeln!(out, "        // Flatten base Display").unwrap();
            writeln!(out, "        let base_str = format!(\"{{}}\", self.base);").unwrap();
            writeln!(
                out,
                "        let base_inner = base_str.trim_matches(|c| c == '{{' || c == '}}').trim();"
            )
            .unwrap();
            writeln!(out, "        if !base_inner.is_empty() {{").unwrap();
            writeln!(
                out,
                "            write!(f, \"{{}}\", base_inner)?;"
            )
            .unwrap();
            if !self.fields.is_empty() {
                writeln!(out, "            write!(f, \", \")?;").unwrap();
            }
            writeln!(out, "        }}").unwrap();
        }

        // --- print own fields ---
        for (i, field) in self.fields.iter().enumerate() {
            let sep = if i + 1 < self.fields.len() { ", " } else { " " };
            writeln!(
                out,
                "        write!(f, \"{name}: {{:?}}{sep}\", self.{name})?;",
                name = field.name,
                sep = sep
            )
            .unwrap();
        }

        writeln!(out, "        write!(f, \"}}}}\")").unwrap();
        writeln!(out, "    }}").unwrap();
        writeln!(out, "}}\n").unwrap();

        // === Method Implementations ===
        if !self.methods.is_empty() {
            writeln!(out, "impl {} {{", self.name).unwrap();
            for m in &self.methods {
                out.push_str(&m.to_rust_method(&self.name, script));
            }
            writeln!(out, "}}\n").unwrap();
        }

        // === Deref Implementations (for base inheritance-style field access) ===
        if let Some(base) = &self.base {
            writeln!(
                out,
                "impl std::ops::Deref for {} {{",
                self.name
            )
            .unwrap();
            writeln!(out, "    type Target = {};", base).unwrap();
            writeln!(
                out,
                "    fn deref(&self) -> &Self::Target {{ &self.base }}",
            )
            .unwrap();
            writeln!(out, "}}\n").unwrap();

            writeln!(out, "impl std::ops::DerefMut for {} {{", self.name).unwrap();
            writeln!(
                out,
                "    fn deref_mut(&mut self) -> &mut Self::Target {{ &mut self.base }}",
            )
            .unwrap();
            writeln!(out, "}}\n").unwrap();
        }

        out
    }
}

impl Function {
    pub fn to_rust_method(&self, node_type: &str, script: &Script) -> String {

        let mut out = String::with_capacity(512);

        // ---------------------------------------------------
        // Generate method signature using owned parameters
        // ---------------------------------------------------
        let mut param_list = String::from("&mut self");

        if !self.params.is_empty() {
            let joined = self
                .params
                .iter()
                .map(|p| match &p.typ {
                    // Strings: passed as owned String
                    Type::String => format!("mut {}: String", p.name),

                    // Custom structs and script types: passed as owned and mutable
                    Type::Custom(name)  => {
                        format!("mut {}: {}", p.name, name)
                    }

                    // Plain primitives: passed by value
                    _ => format!("mut {}: {}", p.name, p.typ.to_rust_type()),
                })
                .collect::<Vec<_>>()
                .join(", ");

            write!(param_list, ", {}", joined).unwrap();
        }

            param_list.push_str(", api: &mut ScriptApi<'_>, external_call: bool");

        writeln!(out, "    fn {}({}) {{", self.name, param_list).unwrap();


        // ---------------------------------------------------
        // (1) Insert additional preamble if the method uses self/api
        // ---------------------------------------------------
        let needs_self = self.uses_self;
        

        if needs_self {
            writeln!(
                out,
                "        if external_call {{"
            )
            .unwrap();
            writeln!(
                out,
                "            self.node = api.get_node_clone::<{}>(self.node.id);",
                node_type
            )
            .unwrap();
            writeln!(out, "        }}").unwrap();
        }

        // ---------------------------------------------------
        // (2) Emit body
        // ---------------------------------------------------
        for stmt in &self.body {
            out.push_str(&stmt.to_rust(needs_self, script, Some(self)));
        }

        if needs_self {
            out.push_str("\n        if external_call {\n");
            out.push_str("            api.merge_nodes(vec![self.node.clone().to_scene_node()]);\n");
            out.push_str("        }\n");
        }
        

        out.push_str("    }\n\n");
        out
    }


    // ============================================================
    // for trait-style API methods (unchanged, still fine)
    // ============================================================
    pub fn to_rust_trait_method(&self, node_type: &str, script: &Script) -> String {
        let mut out = String::with_capacity(512);
        writeln!(out, "    fn {}(&mut self, api: &mut ScriptApi<'_>) {{", self.name.to_lowercase()).unwrap();

        let needs_self = self.uses_self;

        if needs_self {
            writeln!(
                out,
                "        self.node = api.get_node_clone::<{}>(self.node.id);",
                node_type
            )
            .unwrap();
        }

        for stmt in &self.body {
            out.push_str(&stmt.to_rust(needs_self, script, Some(self)));
        }

        if needs_self {
            out.push_str("\n        api.merge_nodes(vec![self.node.clone().to_scene_node()]);\n");
        }

        out.push_str("    }\n\n");
        out
    }
}

impl Stmt {
    fn to_rust(&self, needs_self: bool, script: &Script, current_func: Option<&Function>) -> String {
        match self {
            Stmt::Expr(expr) => {
                let expr_str = expr.to_rust(needs_self, script, current_func);
                if expr_str.trim().is_empty() {
                    String::new()
                } else if expr_str.trim_end().ends_with(';') {
                    format!("        {}\n", expr_str)
                } else {
                    format!("        {};\n", expr_str)
                }
            }

       Stmt::VariableDecl(var) => {
    let expr_str = if let Some(expr) = &var.value {
        let raw_expr = expr.to_rust(needs_self, script, current_func);

        match &expr.expr {
            Expr::Ident(_) | Expr::MemberAccess(..) => {
                if let Some(ty) = script.infer_expr_type(&expr.expr, current_func) {
                    if ty.requires_clone() {
                        format!("{}.clone()", raw_expr)
                    } else {
                        raw_expr
                    }
                } else {
                    raw_expr
                }
            }
            _ => raw_expr
        }
    } else if var.typ.is_some() {
        var.default_value()
    } else {
        String::new()
    };

    if expr_str.is_empty() {
        format!("        let mut {};\n", var.name)
    } else {
        format!("        let mut {} = {};\n", var.name, expr_str)
    }
}
            Stmt::Assign(name, expr) => {
            let target = if script.is_struct_field(name) && !name.starts_with("self.") {
                format!("self.{}", name)
            } else {
                name.clone()
            };

                let target_type = self.get_target_type(name, script, current_func);
                let expr_type = script.infer_expr_type(&expr.expr, current_func);

                let mut expr_str = expr.expr.to_rust(needs_self, script, target_type.as_ref(), current_func);

               let should_clone = matches!(expr.expr, Expr::Ident(_) | Expr::MemberAccess(..))
                && expr_type
                    .as_ref()
                    .map_or(false, |ty| ty.requires_clone());

                if should_clone {
                    expr_str = format!("{}.clone()", expr_str);
                }

                let final_expr = if let Some(target_type) = &target_type {
                    if let Some(expr_type) = &expr_type {
                        if expr_type.can_implicitly_convert_to(target_type) && expr_type != target_type {
                            script.generate_implicit_cast_for_expr(&expr_str, expr_type, target_type)
                        } else {
                            expr_str
                        }
                    } else {
                        expr_str
                    }
                } else {
                    expr_str
                };

                format!("        {} = {};\n", target, final_expr)
            }

            Stmt::AssignOp(name, op, expr) => {
            let target = if script.is_struct_field(name) && !name.starts_with("self.") {
                format!("self.{}", name)
            } else {
                name.clone()
            };

                let target_type = self.get_target_type(name, script, current_func);
                let expr_str = expr.expr.to_rust(needs_self, script, target_type.as_ref(), current_func);

                if matches!(op, Op::Add) && target_type == Some(Type::String) {
                    return format!("        {target}.push_str({expr_str}.as_str());\n");
                }
                
                if let Some(target_type) = &target_type {
                    let expr_type = script.infer_expr_type(&expr.expr, current_func);
                    if let Some(expr_type) = expr_type {
                        let cast_expr = if expr_type.can_implicitly_convert_to(target_type) && &expr_type != target_type {
                            Self::generate_implicit_cast(&expr_str, &expr_type, target_type)
                        } else {
                            expr_str
                        };
                        format!("        {} {}= {};\n", target, op.to_rust_assign(), cast_expr)
                    } else {
                        format!("        {} {}= {};\n", target, op.to_rust_assign(), expr_str)
                    }
                } else {
                    format!("        {} {}= {};\n", target, op.to_rust_assign(), expr_str)
                }
            }

            Stmt::MemberAssign(lhs_expr, rhs_expr) => {
                let lhs_code = lhs_expr.to_rust(needs_self, script, current_func);
                let lhs_type = script.infer_expr_type(&lhs_expr.expr, current_func);
                let rhs_type = script.infer_expr_type(&rhs_expr.expr, current_func);

                let mut rhs_code = rhs_expr.expr.to_rust(needs_self, script, lhs_type.as_ref(), current_func);

                let final_rhs = if let Some(lhs_ty) = &lhs_type {
                    if let Some(rhs_ty) = &rhs_type {
                        if rhs_ty.can_implicitly_convert_to(lhs_ty) && rhs_ty != lhs_ty {
                            script.generate_implicit_cast_for_expr(&rhs_code, rhs_ty, lhs_ty)
                        } else {
                            rhs_code
                        }
                    } else {
                        rhs_code
                    }
                } else {
                    rhs_code
                };

                let should_clone = matches!(rhs_expr.expr, Expr::Ident(_) | Expr::MemberAccess(..))
                && rhs_type
                    .as_ref()
                    .map_or(false, |ty| ty.requires_clone());

                if should_clone {
                    format!("        {lhs_code} = {}.clone();\n", final_rhs)
                } else {
                    format!("        {lhs_code} = {final_rhs};\n")
                }
            }

            Stmt::MemberAssignOp(lhs_expr, op, rhs_expr) => {
                let lhs_code = lhs_expr.to_rust(needs_self, script, current_func);
                let lhs_type = script.infer_expr_type(&lhs_expr.expr, current_func);

                let mut rhs_code = rhs_expr.expr.to_rust(needs_self, script, lhs_type.as_ref(), current_func);

                if matches!(op, Op::Add) && lhs_type == Some(Type::String) {
                    return format!("        {lhs_code}.push_str({rhs_code}.as_str());\n");
                }

                let final_rhs = if let Some(lhs_ty) = &lhs_type {
                    let rhs_ty = script.infer_expr_type(&rhs_expr.expr, current_func);
                    if let Some(rhs_ty) = rhs_ty {
                        if rhs_ty.can_implicitly_convert_to(lhs_ty) && rhs_ty != *lhs_ty {
                            script.generate_implicit_cast_for_expr(&rhs_code, &rhs_ty, lhs_ty)
                        } else {
                            rhs_code
                        }
                    } else {
                        rhs_code
                    }
                } else {
                    rhs_code
                };

                format!("        {lhs_code} {}= {};\n", op.to_rust_assign(), final_rhs)
            }

            Stmt::Pass => String::new(),

            Stmt::ScriptAssign(var, field, rhs) => {
                let rhs_str = rhs.to_rust(needs_self, script, current_func);
                
                let ctor = match script.infer_expr_type(&rhs.expr, current_func) {
                    Some(Type::Number(NumberKind::Signed(_))) => "I32",
                    Some(Type::Number(NumberKind::Unsigned(_))) => "U32", 
                    Some(Type::Number(NumberKind::Float(_))) => "F32",
                    Some(Type::Number(NumberKind::Decimal)) => "Decimal",
                    Some(Type::Number(NumberKind::BigInt)) => "BigInt",
                    Some(Type::Bool) => "Bool",
                    Some(Type::String) => "String",
                    _ => "F32",
                };
                
                format!(
                    "        api.update_script_var(&{}_id, \"{}\", UpdateOp::Set, Var::{}({}));\n",
                    var, field, ctor, rhs_str
                )
            }

            Stmt::ScriptAssignOp(var, field, op, rhs) => {
                let rhs_str = rhs.to_rust(needs_self, script, current_func);
                let op_str = match op {
                    Op::Add => "Add",
                    Op::Sub => "Sub",
                    Op::Mul => "Mul",
                    Op::Div => "Div",
                };

                let ctor = match script.infer_expr_type(&rhs.expr, current_func) {
                    Some(Type::Number(NumberKind::Signed(_))) => "I32",
                    Some(Type::Number(NumberKind::Unsigned(_))) => "U32",
                    Some(Type::Number(NumberKind::Float(_))) => "F32",
                    Some(Type::Number(NumberKind::Decimal)) => "Decimal",
                    Some(Type::Number(NumberKind::BigInt)) => "BigInt",
                    Some(Type::Bool) => "Bool",
                    Some(Type::String) => "String",
                    _ => "F32",
                };

                format!(
                    "        api.update_script_var(&{}_id, \"{}\", UpdateOp::{}, Var::{}({}));\n",
                    var, field, op_str, ctor, rhs_str
                )
            }

Stmt::IndexAssign(array_expr, index_expr, rhs_expr) => {
    let array_code = array_expr.to_rust(needs_self, script, None, current_func);
    let index_code = index_expr.to_rust(needs_self, script, None, current_func);

    let lhs_type = script.infer_expr_type(&array_expr, current_func);
    let rhs_type = script.infer_expr_type(&rhs_expr.expr, current_func);

    let mut rhs_code = rhs_expr.expr.to_rust(needs_self, script, lhs_type.as_ref(), current_func);

    // Insert implicit conversion if needed, matching your member assign arm
    let final_rhs = if let Some(lhs_ty) = &lhs_type {
        if let Some(rhs_ty) = &rhs_type {
            if rhs_ty.can_implicitly_convert_to(lhs_ty) && rhs_ty != lhs_ty {
                script.generate_implicit_cast_for_expr(&rhs_code, rhs_ty, lhs_ty)
            } else {
                rhs_code
            }
        } else {
            rhs_code
        }
    } else {
        rhs_code
    };

    // Insert `.clone()` if needed, matching your member assign arm
    let should_clone = matches!(rhs_expr.expr, Expr::Ident(_) | Expr::MemberAccess(..))
        && rhs_type
            .as_ref()
            .map_or(false, |ty| ty.requires_clone());

    if should_clone {
        format!("        {}[{}] = {}.clone();\n", array_code, index_code, final_rhs)
    } else {
        format!("        {}[{}] = {};\n", array_code, index_code, final_rhs)
    }
},

Stmt::IndexAssignOp(array_expr, index_expr, op, rhs_expr) => {
    let array_code = array_expr.to_rust(needs_self, script, None, current_func);
    let index_code = index_expr.to_rust(needs_self, script, None, current_func);

    let lhs_type = script.infer_expr_type(&array_expr, current_func);
    let rhs_type = script.infer_expr_type(&rhs_expr.expr, current_func);

    let mut rhs_code = rhs_expr.expr.to_rust(needs_self, script, lhs_type.as_ref(), current_func);

    // Special case: string += something becomes push_str.
    if matches!(op, Op::Add) && lhs_type == Some(Type::String) {
        return format!("        {}[{}].push_str({}.as_str());\n", array_code, index_code, rhs_code);
    }

    // Insert implicit cast if needed
    let final_rhs = if let Some(lhs_ty) = &lhs_type {
        if let Some(rhs_ty) = &rhs_type {
            if rhs_ty.can_implicitly_convert_to(lhs_ty) && rhs_ty != lhs_ty {
                script.generate_implicit_cast_for_expr(&rhs_code, rhs_ty, lhs_ty)
            } else {
                rhs_code
            }
        } else {
            rhs_code
        }
    } else {
        rhs_code
    };

    format!("        {}[{}] {}= {};\n", array_code, index_code, op.to_rust_assign(), final_rhs)
}
        }
    }

    fn generate_implicit_cast(expr: &str, from_type: &Type, to_type: &Type) -> String {
        use Type::*;
        use NumberKind::*;

        if from_type == to_type {
            return expr.to_string();
        }

        match (from_type, to_type) {
            (Number(Float(32)), Number(Float(64))) => format!("({} as f64)", expr),
            (Number(Float(64)), Number(Float(32))) => format!("({} as f32)", expr),
            (Number(Signed(_) | Unsigned(_)), Number(Float(64))) => format!("({} as f64)", expr),
            (Number(Signed(_) | Unsigned(_)), Number(Float(32))) => format!("({} as f32)", expr),
            (Number(Signed(_)), Number(Signed(to_w))) => format!("({} as i{})", expr, to_w),
            (Number(Signed(_)), Number(Unsigned(to_w))) => format!("({} as u{})", expr, to_w),
            (Number(Unsigned(_)), Number(Unsigned(to_w))) => format!("({} as u{})", expr, to_w),
            (Number(Unsigned(_)), Number(NumberKind::BigInt)) => format!("BigInt::from({})", expr), // Added: Unsigned to BigInt
            (Number(Unsigned(_)), Number(Signed(to_w))) => format!("({} as i{})", expr, to_w),
            (Number(BigInt), Number(Signed(w))) => match w {
                32 => format!("{}.to_i32().unwrap_or_default()", expr),
                64 => format!("{}.to_i64().unwrap_or_default()", expr),
                _ => format!("({}.to_i64().unwrap_or_default() as i{})", expr, w),
            },
            (Number(Signed(_) | Unsigned(_)), Number(BigInt)) => format!("BigInt::from({})", expr),
            (Number(Decimal), Number(Signed(w))) => match w {
                32 => format!("{}.to_i32().unwrap_or_default()", expr),
                64 => format!("{}.to_i64().unwrap_or_default()", expr),
                _ => format!("({}.to_i64().unwrap_or_default() as i{})", expr, w),
            },
            (Number(Signed(_) | Unsigned(_)), Number(Decimal)) => format!("Decimal::from({})", expr),


            _ => {
                eprintln!("Warning: Unhandled cast from {:?} to {:?}", from_type, to_type);
                expr.to_string()
            },

        }
    }

    fn get_target_type(&self, name: &str, script: &Script, current_func: Option<&Function>) -> Option<Type> {
        if let Some(func) = current_func {
            if let Some(local) = func.locals.iter().find(|v| v.name == name) {
                return local.typ.clone();
            }
            if let Some(param) = func.params.iter().find(|p| p.name == name) {
                return Some(param.typ.clone());
            }
        }

        if let Some((base, field)) = name.split_once('.') {
            if let Some(base_ty) = script.get_variable_type(base) {
                if let Some(field_ty) = script.get_member_type(base_ty, field) {
                    return Some(field_ty);
                }
            }
        }

        script.get_variable_type(name).cloned()
    }

    fn contains_self(&self) -> bool {
        match self {
            Stmt::Expr(e) => e.contains_self(),
            Stmt::VariableDecl(var) => var.value.as_ref().map_or(false, |e| e.contains_self()),
            Stmt::Assign(_, e) | Stmt::AssignOp(_, _, e) => e.contains_self(),
            Stmt::MemberAssign(lhs, rhs) | Stmt::MemberAssignOp(lhs, _, rhs) => {
                lhs.contains_self() || rhs.contains_self()
            }
            Stmt::ScriptAssign(_, _, expr) | Stmt::ScriptAssignOp(_, _, _, expr) => expr.contains_self(),
            Stmt::IndexAssign(array, index, value)
            | Stmt::IndexAssignOp(array, index, _, value) => {
                array.contains_self() || index.contains_self() || value.contains_self()
            }
            Stmt::Pass => false,
        }
    }

    pub fn contains_api_call(&self, script: &Script) -> bool {
        match self {
            Stmt::Expr(e) => e.contains_api_call(script),
            Stmt::VariableDecl(v) => v.value.as_ref().map_or(false, |e| e.contains_api_call(script)),
            Stmt::Assign(_, e) | Stmt::AssignOp(_, _, e) => e.contains_api_call(script),
            Stmt::MemberAssign(a, b) | Stmt::MemberAssignOp(a, _, b) => {
                a.contains_api_call(script) || b.contains_api_call(script)
            }
            Stmt::IndexAssign(array, index, value)
            | Stmt::IndexAssignOp(array, index, _, value) => {
                array.contains_api_call(script)
                    || index.contains_api_call(script)
                    || value.contains_api_call(script)
            }
            Stmt::ScriptAssign(_, _, e) | Stmt::ScriptAssignOp(_, _, _, e) => e.contains_api_call(script),
            Stmt::Pass => false,
        }
    }
}

impl TypedExpr {
    pub fn to_rust(&self, needs_self: bool, script: &Script, current_func: Option<&Function>) -> String {
        let type_hint = self.inferred_type.as_ref();
        self.expr.to_rust(needs_self, script, type_hint, current_func)
    }

    pub fn contains_self(&self) -> bool {
        self.expr.contains_self()
    }

    pub fn contains_api_call(&self, script: &Script) -> bool {
        self.expr.contains_api_call(script)
    }
}

impl Expr {
fn clone_if_needed(
    expr_code: String,
    expr: &Expr,
    script: &Script,
    current_func: Option<&Function>,
) -> String {
    if Expr::should_clone_expr(&expr_code, expr, script, current_func) {
        format!("{}.clone()", expr_code)
    } else {
        expr_code
    }
}

fn should_clone_expr(expr_code: &str, expr: &Expr, script: &Script, current_func: Option<&Function>) -> bool {
    if expr_code.starts_with("json!(")
        || expr_code.starts_with("HashMap::from(")
        || expr_code.starts_with("vec![")
        || expr_code.contains("serde_json::from_value::<")
        || expr_code.contains(".parse::<")
        || expr_code.contains('{')  // struct literal produces an owned value
    {
        return false;
    }

    match expr {
            Expr::Ident(_) | Expr::MemberAccess(..) => {
                if let Some(ty) = script.infer_expr_type(expr, current_func) {
                    ty.requires_clone()
                } else {
                    false
                }
            }
            _ => false,
        }
}



    pub fn to_rust(&self, needs_self: bool, script: &Script, expected_type: Option<&Type>, current_func: Option<&Function>) -> String {
        use ContainerKind::*;

        match self {
            Expr::Ident(name) => {
                let is_local = current_func
                    .map(|f| {
                        f.locals.iter().any(|v| v.name == *name)
                            || f.params.iter().any(|p| p.name == *name)
                    })
                    .unwrap_or(false);

                // Check against `script_vars` to see if it's a field
                let is_field = script.variables.iter().any(|v| v.name == *name);

                let ident_code = if !is_local && is_field && !name.starts_with("self.") {
                    format!("self.{}", name)
                } else {
                    name.clone()
                };

                // ✨ Add this: wrap in json! if going to Value/Object
                if let Some(Type::Object) = expected_type {
                    format!("json!({})", ident_code)
                } else {
                    ident_code
                }
            }
            Expr::Literal(lit) => {
                // New: check if the expected_type is Type::Object
                if let Some(Type::Object) = expected_type {
                    format!("json!({})", lit.to_rust(None))
                } else if let Some(expected) = expected_type {
                    lit.to_rust(Some(expected))
                } else {
                    let inferred_type = script.infer_literal_type(lit, None);
                    lit.to_rust(inferred_type.as_ref())
                }
            }
            Expr::BinaryOp(left, op, right) => {
                let left_type = script.infer_expr_type(left, current_func);
                let right_type = script.infer_expr_type(right, current_func);

                let dominant_type = if let Some(expected) = expected_type.cloned() {
                    Some(expected)
                } else {
                    match (&left_type, &right_type) {
                        (Some(l), Some(r)) => script.promote_types(l, r).or(Some(l.clone())),
                        (Some(l), None) => Some(l.clone()),
                        (None, Some(r)) => Some(r.clone()),
                        _ => None,
                    }
                };

                let left_raw = left.to_rust(needs_self, script, dominant_type.as_ref(), current_func);
                let right_raw = right.to_rust(needs_self, script, dominant_type.as_ref(), current_func);

                let (left_str, right_str) = match (&left_type, &right_type, &dominant_type) {
                    (Some(l), Some(r), Some(dom)) => {
                        let l_cast = if l.can_implicitly_convert_to(dom) && l != dom {
                            script.generate_implicit_cast_for_expr(&left_raw, l, dom)
                        } else {
                            left_raw
                        };
                        let r_cast = if r.can_implicitly_convert_to(dom) && r != dom {
                            script.generate_implicit_cast_for_expr(&right_raw, r, dom)
                        } else {
                            right_raw
                        };
                        (l_cast, r_cast)
                    }
                    _ => (left_raw, right_raw),
                };

                if matches!(op, Op::Add)
                    && (left_type == Some(Type::String) || right_type == Some(Type::String))
                {
                    return format!("format!(\"{{}}{{}}\", {}, {})", left_str, right_str);
                }

                format!("({} {} {})", left_str, op.to_rust(), right_str)
            }
            Expr::MemberAccess(base, field) => {
                let base_type = script.infer_expr_type(base, current_func);

                match base_type {
                    Some(Type::Object) => {
                        // dynamic object (serde_json::Value)
                        let base_code = base.to_rust(needs_self, script, None, current_func);
                        format!("{}[\"{}\"].clone()", base_code, field)
                    }
                    Some(Type::Container(ContainerKind::Map, _)) => {
                        let base_code = base.to_rust(needs_self, script, None, current_func);
                        format!("{}[\"{}\"].clone()", base_code, field)
                    }
                    Some(Type::Container(ContainerKind::Array, _)) |
                    Some(Type::Container(ContainerKind::FixedArray(_), _)) => {
                        // Vec or FixedArray (support access via integer index, not field name)
                        // We'll still return an error if used as .field, but you can choose logic:
                        let base_code = base.to_rust(needs_self, script, None, current_func);
                        format!("/* Cannot perform field access '{}' on array or fixed array */ {}", field, base_code)
                    }
                    Some(Type::Custom(_)) => {
                        // typed struct: regular .field access
                        let base_code = base.to_rust(needs_self, script, None, current_func);
                        format!("{}.{}", base_code, field)
                    }
                    _ => {
                        // fallback, assume normal member access
                        let base_code = base.to_rust(needs_self, script, None, current_func);
                        format!("{}.{}", base_code, field)
                    }
                }
            }
            Expr::SelfAccess => {
                if needs_self {
                    "self.node".to_string()
                } else {
                    "self".to_string()
                }
            }
            Expr::BaseAccess => "self.base".to_string(),
            Expr::Call(target, args) => {
    
                // ==============================================================
                // Extract the target function name, if possible
                // ==============================================================
                let func_name = Self::get_target_name(target);

                // Determine whether this is a local method on the current script
                let is_local_function = func_name
                    .as_ref()
                    .map(|name| script.functions.iter().any(|f| f.name == *name))
                    .unwrap_or(false);

                // ==============================================================
                // Convert each argument expression into Rust source code
                // with proper ownership semantics and type-aware cloning
                // ==============================================================
                let args_rust: Vec<String> = args
                    .iter()
                    .map(|arg| {
                        // Generate code for argument
                        let code = arg.to_rust(needs_self, script, None, current_func);
                        // Ask the script context to infer the argument type
                        let arg_type = script.infer_expr_type(arg, current_func);

                        match (arg, &arg_type) {
                            // ----------------------------------------------------------
                            // 1️⃣ Literal values — simple by-value semantics
                            // ----------------------------------------------------------
                            (Expr::Literal(Literal::String(_)), _)
                            | (Expr::Literal(Literal::Interpolated(_)), _) => {
                                // Strings use owned String, so clone
                                format!("{}.clone()", code)
                            }
                            (Expr::Literal(_), _) => {
                                // Numeric or bool literals — pass directly
                                code
                            }

                            // ----------------------------------------------------------
                            // 2️⃣ Identifiers & member accesses
                            // ----------------------------------------------------------
                            (Expr::Ident(_) | Expr::MemberAccess(..), Some(Type::String))
                            | (Expr::Ident(_) | Expr::MemberAccess(..), Some(Type::Custom(_)))
                            | (Expr::Ident(_) | Expr::MemberAccess(..), Some(Type::Script)) => {
                                // Owned strings and structs cloned
                                format!("{}.clone()", code)
                            }
                            (Expr::Ident(_) | Expr::MemberAccess(..), _) => {
                                // Primitives & known copies — pass directly
                                code
                            }

                            // ----------------------------------------------------------
                            // 3️⃣ Computed expressions — ops, casts, nested calls, etc.
                            // ----------------------------------------------------------
                            (Expr::BinaryOp(..) | Expr::Call(..) | Expr::Cast(..), Some(Type::String))
                            | (Expr::BinaryOp(..) | Expr::Call(..) | Expr::Cast(..), Some(Type::Custom(_)))
                            | (Expr::BinaryOp(..) | Expr::Call(..) | Expr::Cast(..), Some(Type::Script)) => {
                                // Complex expressions producing owned objects → clone
                                format!("({}).clone()", code)
                            }
                            (Expr::BinaryOp(..) | Expr::Call(..) | Expr::Cast(..), _) => {
                                // Pure primitives / temporaries
                                format!("({})", code)
                            }

                            // ----------------------------------------------------------
                            // 4️⃣ Fallback / unknown type (inference unresolved)
                            // ----------------------------------------------------------
                            (_) => {
                                // Safe fallback — assume Clone is implemented
                                format!("{}.clone()", code)
                            }
                        }
                    })
                    .collect();


                // Convert the target expression (e.g., func or self.method)
                let mut target_str = target.to_rust(needs_self, script, None, current_func);

                // If this is a local user-defined function, prefix with `self.`
                // After generating `let mut target_str = target.to_rust(...)`:
                if is_local_function && !target_str.starts_with("self.") {
                    target_str = format!("self.{}", func_name.unwrap());
                }

                // ==============================================================
                // Finally, build the Rust call string
                // Handles API injection and empty arg lists
                // ==============================================================
                    if args_rust.is_empty() {
                        format!("{}(api, false);", target_str)
                    } else {
                        format!("{}({}, api, false);", target_str, args_rust.join(", "))
                    }
            }
  Expr::ContainerLiteral(_, data) => match data {
    // ===============================================================
    // MAP LITERAL: { "key": value, other_key: expr }
    // ===============================================================
    ContainerLiteralData::Map(pairs) => {
        let code = if pairs.is_empty() {
            "HashMap::new()".to_string()
        } else {
            // Expected key/value types (from context if known)
            let (expected_key_type, expected_val_type) = match expected_type {
                Some(Type::Container(ContainerKind::Map, types)) if types.len() == 2 => {
                    (&types[0], &types[1])
                }
                _ => (&Type::String, &Type::Object),
            };

           let entries: Vec<_> = pairs
    .iter()
    .map(|(k_expr, v_expr)| {
        let raw_k = k_expr.to_rust(needs_self, script, Some(expected_key_type), current_func);
        let raw_v = v_expr.to_rust(needs_self, script, Some(expected_val_type), current_func);

        let k_final = if Expr::should_clone_expr(&raw_k, k_expr, script, current_func)
        {
            format!("{}.clone()", raw_k)
        } else {
            raw_k
        };

        let v_final = if Expr::should_clone_expr(&raw_v, v_expr, script, current_func)
        {
            format!("{}.clone()", raw_v)
        } else {
            raw_v
        };

        format!("({}, {})", k_final, v_final)
    })
    .collect();

            format!("HashMap::from([{}])", entries.join(", "))
        };

        if matches!(expected_type, Some(Type::Object)) {
            format!("json!({})", code)
        } else {
            code
        }
    }

    // ===============================================================
    // ARRAY LITERAL: [expr1, expr2, expr3]
    // ===============================================================
    ContainerLiteralData::Array(elems) => {
        let code = if elems.is_empty() {
            "Vec::new()".to_string()
        } else {
            let elem_ty = match expected_type {
                Some(Type::Container(ContainerKind::Array, types))
                    if !types.is_empty() =>
                {
                    &types[0]
                }
                _ => &Type::Object,
            };

           let elements: Vec<_> = elems
    .iter()
    .map(|e| {
        let rendered = e.to_rust(needs_self, script, Some(elem_ty), current_func);
        if Expr::should_clone_expr(&rendered, e, script, current_func) {
            format!("{}.clone()", rendered)
        } else {
            rendered
        }
    })
    .collect();

            format!("vec![{}]", elements.join(", "))
        };

        if matches!(expected_type, Some(Type::Object)) {
            format!("json!({})", code)
        } else {
            code
        }
    }

    // ===============================================================
    // FIXED ARRAY LITERAL: [a, b, c] with explicit constant size
    // ===============================================================
    ContainerLiteralData::FixedArray(size, elems) => {
        let mut body: Vec<_> = elems
            .iter()
            .map(|e| {
                let rendered = e.to_rust(needs_self, script, None, current_func);
                match e {
                    Expr::Ident(_) | Expr::MemberAccess(..) => {
                        let ty = script.infer_expr_type(e, current_func);
                        if ty.as_ref().map_or(false, |t| t.requires_clone()) {
                            format!("{}.clone()", rendered)
                        } else {
                            rendered
                        }
                    }
                    _ => rendered,
                }
            })
            .collect();

        while body.len() < *size {
            body.push("Default::default()".into());
        }
        if body.len() > *size {
            body.truncate(*size);
        }

        let code = format!("[{}]", body.join(", "));

        if matches!(expected_type, Some(Type::Object)) {
            format!("json!({})", code)
        } else {
            code
        }
    }
}
Expr::StructNew(ty, args) => {
    use std::collections::HashMap;

    // --- Flatten structure hierarchy correctly ---
    fn gather_flat_fields<'a>(
        s: &'a StructDef,
        script: &'a Script,
        out: &mut Vec<(&'a str, &'a Type, Option<&'a str>)>,
    ) {
        if let Some(ref base) = s.base {
            if let Some(basedef) = script.structs.iter().find(|b| &b.name == base) {
                gather_flat_fields_with_parent(basedef, script, out, Some(base.as_str()));
            }
        }

        // Derived-level fields: no parent
        for f in &s.fields {
            out.push((f.name.as_str(), &f.typ, None));
        }
    }

    fn gather_flat_fields_with_parent<'a>(
        s: &'a StructDef,
        script: &'a Script,
        out: &mut Vec<(&'a str, &'a Type, Option<&'a str>)>,
        parent_name: Option<&'a str>,
    ) {
        // Include base of the base, recursively
        if let Some(ref base) = s.base {
            if let Some(basedef) = script.structs.iter().find(|b| &b.name == base) {
                gather_flat_fields_with_parent(basedef, script, out, Some(base.as_str()));
            }
        }

        // Tag each field in this struct with its owning base
        for f in &s.fields {
            out.push((f.name.as_str(), &f.typ, parent_name));
        }
    }

    // --- Get struct info ---
    let struct_def = script
        .structs
        .iter()
        .find(|s| s.name == *ty)
        .expect("Struct not found");

    let mut flat_fields = Vec::new();
    gather_flat_fields(struct_def, script, &mut flat_fields);

    // Map arguments in order to flattened field list
    // ----------------------------------------------------------
// Map each parsed (field_name, expr) to its real definition
// ----------------------------------------------------------
let mut field_exprs: Vec<(&str, &Type, Option<&str>, &Expr)> = Vec::new();

for (field_name, expr) in args {
    // look for a matching field by name anywhere in the flattened struct hierarchy
    if let Some((fname, fty, parent)) =
        flat_fields.iter().find(|(fname, _, _)| *fname == field_name.as_str())
    {
        // found: record exact type & base
        field_exprs.push((*fname, *fty, *parent, expr));
    } else {
        // unknown field; keep it but use Type::Object as a fallback
        field_exprs.push((field_name.as_str(), &Type::Object, None, expr));
    }
}

    // --- Group by base name (if parent) ---
    let mut base_fields: HashMap<&str, Vec<(&str, &Type, &Expr)>> = HashMap::new();
    let mut derived_fields: Vec<(&str, &Type, &Expr)> = Vec::new();

    for (fname, fty, parent, expr) in &field_exprs {
        if let Some(base_name) = parent {
            base_fields
                .entry(base_name)
                .or_default()
                .push((*fname, *fty, *expr));
        } else {
            derived_fields.push((*fname, *fty, *expr));
        }
    }

    // --- Recursive builder for nested base init ---
    fn build_base_init(
        base_name: &str,
        base_fields: &HashMap<&str, Vec<(&str, &Type, &Expr)>>,
        script: &Script,
        needs_self: bool,
        current_func: Option<&Function>,
    ) -> String {
        let base_struct = script
            .structs
            .iter()
            .find(|s| s.name == base_name)
            .expect("Base struct not found");

        let mut parts = String::new();

        // Handle deeper bases first
        if let Some(ref inner) = base_struct.base {
            let inner_code =
                build_base_init(inner, base_fields, script, needs_self, current_func);
            parts.push_str(&format!("base: {}, ", inner_code));
        }

        // Write base’s own fields
        if let Some(local_fields) = base_fields.get(base_name) {
            for (fname, fty, expr) in local_fields {
                let mut expr_code =
                    expr.to_rust(needs_self, script, Some(fty), current_func);
                let expr_type = script.infer_expr_type(expr, current_func);
                let should_clone = matches!(expr, Expr::Ident(_) | Expr::MemberAccess(..))
                    && expr_type
                        .as_ref()
                        .map_or(false, |ty| ty.requires_clone());
                if should_clone {
                    expr_code = format!("{}.clone()", expr_code);
                }
                parts.push_str(&format!("{}: {}, ", fname, expr_code));
            }
        }

        format!("{} {{ {}..Default::default() }}", base_name, parts)
    }

    // --- Build final top-level struct ---
    let mut code = String::new();

    // 1️⃣ Base (if exists)
    if let Some(ref base_name) = struct_def.base {
        let base_code = build_base_init(base_name, &base_fields, script, needs_self, current_func);
        code.push_str(&format!("base: {}, ", base_code));
    }

    // 2️⃣ Derived-only fields
    for (fname, fty, expr) in &derived_fields {
        let mut expr_code = expr.to_rust(needs_self, script, Some(fty), current_func);
        let expr_type = script.infer_expr_type(expr, current_func);
        let should_clone = matches!(expr, Expr::Ident(_) | Expr::MemberAccess(..))
            && expr_type
                .as_ref()
                .map_or(false, |ty| ty.requires_clone());
        if should_clone {
            expr_code = format!("{}.clone()", expr_code);
        }
        code.push_str(&format!("{}: {}, ", fname, expr_code));
    }

    format!("{} {{ {}..Default::default() }}", ty, code)
}
            Expr::ApiCall(module, args) => {
                // Get expected param types (if defined for this API)
                let expected_param_types = module.param_types();

                // Generate argument code with expected type hints applied **now**
                let mut arg_strs: Vec<String> = args
                    .iter()
                    .enumerate()
                    .map(|(i, arg)| {
            // Determine expected type for this argument
            let expected_ty_hint = expected_param_types
                .as_ref()
                .and_then(|v| v.get(i));

            // Ask expression to render itself, with the hint
            arg.to_rust(needs_self, script, expected_ty_hint, current_func)
                    })
                    .collect();

                // Re‑enforce if API declares argument types and conversion is still needed
                if let Some(expected) = &expected_param_types {
                    for (i, expected_ty) in expected.iter().enumerate() {
            if let Some(arg_expr) = args.get(i) {
                // 1. Infer arg type (contextually refined now)
                let actual_ty = script.infer_expr_type(arg_expr, current_func);

                // 2. If convertible and different ⇒ implicit cast
                if let Some(actual_ty) = &actual_ty {
                    if actual_ty.can_implicitly_convert_to(expected_ty)
                        && actual_ty != expected_ty
                    {
                        arg_strs[i] = script.generate_implicit_cast_for_expr(
                            &arg_strs[i],
                            actual_ty,
                            expected_ty,
                        );
                    }
                }
            }
                    }
                }

                // Delegate to actual API handler for final construction
                module.to_rust(&args, script, needs_self, current_func)
            }
            Expr::Cast(inner, target_type) => {
                let inner_type = script.infer_expr_type(inner, current_func);
                let inner_code = inner.to_rust(needs_self, script, Some(target_type), current_func);

                match (&inner_type, target_type) {
                    // String → Numeric Type Conversions
                    (Some(Type::String), Type::Number(NumberKind::Signed(w))) => match w {
                        8   => format!("{}.parse::<i8>().unwrap_or_default()", inner_code),
                        16  => format!("{}.parse::<i16>().unwrap_or_default()", inner_code),
                        32  => format!("{}.parse::<i32>().unwrap_or_default()", inner_code),
                        64  => format!("{}.parse::<i64>().unwrap_or_default()", inner_code),
                        128 => format!("{}.parse::<i128>().unwrap_or_default()", inner_code),
                        _   => format!("{}.parse::<i32>().unwrap_or_default()", inner_code),
                    },

                    (Some(Type::String), Type::Number(NumberKind::Unsigned(w))) => match w {
                        8   => format!("{}.parse::<u8>().unwrap_or_default()", inner_code),
                        16  => format!("{}.parse::<u16>().unwrap_or_default()", inner_code),
                        32  => format!("{}.parse::<u32>().unwrap_or_default()", inner_code),
                        64  => format!("{}.parse::<u64>().unwrap_or_default()", inner_code),
                        128 => format!("{}.parse::<u128>().unwrap_or_default()", inner_code),
                        _   => format!("{}.parse::<u32>().unwrap_or_default()", inner_code),
                    },

                    (Some(Type::String), Type::Number(NumberKind::Float(w))) => match w {
                        32 => format!("{}.parse::<f32>().unwrap_or_default()", inner_code),
                        64 => format!("{}.parse::<f64>().unwrap_or_default()", inner_code),
                        _  => format!("{}.parse::<f32>().unwrap_or_default()", inner_code),
                    },

                    (Some(Type::String), Type::Number(NumberKind::Decimal)) =>
                        format!("Decimal::from_str({}.as_ref()).unwrap_or_default()", inner_code),

                    (Some(Type::String), Type::Number(NumberKind::BigInt)) =>
                        format!("BigInt::from_str({}.as_ref()).unwrap_or_default()", inner_code),

                    (Some(Type::String), Type::Bool) =>
                        format!("{}.parse::<bool>().unwrap_or_default()", inner_code),

                    // Numeric/Bool → String Conversions
                    (Some(Type::Number(_)), Type::String) | (Some(Type::Bool), Type::String) =>
                        format!("{}.to_string()", inner_code),

                    // BigInt → Signed Integer
                    (Some(Type::Number(NumberKind::BigInt)), Type::Number(NumberKind::Signed(w))) => match w {
                        8   => format!("{}.to_i8().unwrap_or_default()", inner_code),
                        16  => format!("{}.to_i16().unwrap_or_default()", inner_code),
                        32  => format!("{}.to_i32().unwrap_or_default()", inner_code),
                        64  => format!("{}.to_i64().unwrap_or_default()", inner_code),
                        128 => format!("{}.to_i128().unwrap_or_default()", inner_code),
                        _   => format!("({}.to_i64().unwrap_or_default() as i{})", inner_code, w),
                    },

                    // BigInt → Unsigned Integer
                    (Some(Type::Number(NumberKind::BigInt)), Type::Number(NumberKind::Unsigned(w))) => match w {
                        8   => format!("{}.to_u8().unwrap_or_default()", inner_code),
                        16  => format!("{}.to_u16().unwrap_or_default()", inner_code),
                        32  => format!("{}.to_u32().unwrap_or_default()", inner_code),
                        64  => format!("{}.to_u64().unwrap_or_default()", inner_code),
                        128 => format!("{}.to_u128().unwrap_or_default()", inner_code),
                        _   => format!("({}.to_u64().unwrap_or_default() as u{})", inner_code, w),
                    },

                    // BigInt ↔ Float
                    (Some(Type::Number(NumberKind::BigInt)), Type::Number(NumberKind::Float(32))) =>
                        format!("{}.to_f32().unwrap_or_default()", inner_code),
                    (Some(Type::Number(NumberKind::BigInt)), Type::Number(NumberKind::Float(64))) =>
                        format!("{}.to_f64().unwrap_or_default()", inner_code),
                    (Some(Type::Number(NumberKind::Float(w))), Type::Number(NumberKind::BigInt)) => match w {
                        32 => format!("BigInt::from({} as i32)", inner_code),
                        64 => format!("BigInt::from({} as i64)", inner_code),
                        _  => format!("BigInt::from({} as i64)", inner_code),
                    },

                    // BigInt → String
                    (Some(Type::Number(NumberKind::BigInt)), Type::String) =>
                        format!("{}.to_string()", inner_code),

                    // Decimal → Integer
                    (Some(Type::Number(NumberKind::Decimal)), Type::Number(NumberKind::Signed(w))) => match w {
                        8   => format!("{}.to_i8().unwrap_or_default()", inner_code),
                        16  => format!("{}.to_i16().unwrap_or_default()", inner_code),
                        32  => format!("{}.to_i32().unwrap_or_default()", inner_code),
                        64  => format!("{}.to_i64().unwrap_or_default()", inner_code),
                        128 => format!("({}.to_i64().unwrap_or_default() as i{})", inner_code, w),
                        _   => format!("({}.to_i64().unwrap_or_default() as i{})", inner_code, w),
                    },
                    (Some(Type::Number(NumberKind::Decimal)), Type::Number(NumberKind::Unsigned(w))) => match w {
                        8   => format!("{}.to_u8().unwrap_or_default()", inner_code),
                        16  => format!("{}.to_u16().unwrap_or_default()", inner_code),
                        32  => format!("{}.to_u32().unwrap_or_default()", inner_code),
                        64  => format!("{}.to_u64().unwrap_or_default()", inner_code),
                        128 => format!("({}.to_u64().unwrap_or_default() as u{})", inner_code, w),
                        _   => format!("({}.to_u64().unwrap_or_default() as u{})", inner_code, w),
                    },

                    // Decimal → Float
                    (Some(Type::Number(NumberKind::Decimal)), Type::Number(NumberKind::Float(32))) =>
                        format!("{}.to_f32().unwrap_or_default()", inner_code),
                    (Some(Type::Number(NumberKind::Decimal)), Type::Number(NumberKind::Float(64))) =>
                        format!("{}.to_f64().unwrap_or_default()", inner_code),

                    // Integer/Float → Decimal
                    (Some(Type::Number(NumberKind::Signed(_) | NumberKind::Unsigned(_))), Type::Number(NumberKind::Decimal)) =>
                        format!("Decimal::from({})", inner_code),

                    (Some(Type::Number(NumberKind::Float(32))), Type::Number(NumberKind::Decimal)) =>
                        format!("Decimal::from_f32({}).unwrap_or_default()", inner_code),
                    (Some(Type::Number(NumberKind::Float(64))), Type::Number(NumberKind::Decimal)) =>
                        format!("Decimal::from_f64({}).unwrap_or_default()", inner_code),

                    // Decimal ↔ BigInt
                    (Some(Type::Number(NumberKind::Decimal)), Type::Number(NumberKind::BigInt)) =>
                        format!("BigInt::from({}.to_i64().unwrap_or_default())", inner_code),
                    (Some(Type::Number(NumberKind::BigInt)), Type::Number(NumberKind::Decimal)) =>
                        format!("Decimal::from({}.to_i64().unwrap_or_default())", inner_code),

                    // Decimal → String
                    (Some(Type::Number(NumberKind::Decimal)), Type::String) =>
                        format!("{}.to_string()", inner_code),

                    // Standard Numeric Casts
                    (Some(Type::Number(NumberKind::Signed(_))), Type::Number(NumberKind::Signed(to_w))) =>
                        format!("({} as i{})", inner_code, to_w),
                    (Some(Type::Number(NumberKind::Signed(_))), Type::Number(NumberKind::Unsigned(to_w))) =>
                        format!("({} as u{})", inner_code, to_w),
                    (Some(Type::Number(NumberKind::Unsigned(_))), Type::Number(NumberKind::Unsigned(to_w))) =>
                        format!("({} as u{})", inner_code, to_w),
                    (Some(Type::Number(NumberKind::Unsigned(_))), Type::Number(NumberKind::Signed(to_w))) =>
                        format!("({} as i{})", inner_code, to_w),
        
                    (Some(Type::Number(NumberKind::Signed(_) | NumberKind::Unsigned(_))), Type::Number(NumberKind::Float(32))) =>
                        format!("({} as f32)", inner_code),
                    (Some(Type::Number(NumberKind::Signed(_) | NumberKind::Unsigned(_))), Type::Number(NumberKind::Float(64))) =>
                        format!("({} as f64)", inner_code),
        
                    (Some(Type::Number(NumberKind::Float(_))), Type::Number(NumberKind::Signed(w))) =>
                        format!("({}.round() as i{})", inner_code, w),
                    (Some(Type::Number(NumberKind::Float(_))), Type::Number(NumberKind::Unsigned(w))) =>
                        format!("({}.round() as u{})", inner_code, w),
        
                    (Some(Type::Number(NumberKind::Float(32))), Type::Number(NumberKind::Float(64))) =>
                        format!("({} as f64)", inner_code),
                    (Some(Type::Number(NumberKind::Float(64))), Type::Number(NumberKind::Float(32))) =>
                        format!("({} as f32)", inner_code),

                    (Some(Type::Number(NumberKind::Signed(_) | NumberKind::Unsigned(_))), Type::Number(NumberKind::BigInt)) =>
                        format!("BigInt::from({})", inner_code),

                    // ==========================================================
                    // JSON Value (ContainerKind::Object) → Anything
                    // ==========================================================
                (Some(Type::Object), target) => {
    use NumberKind::*;
    match target {
        Type::Number(Signed(w)) =>
            format!("{}.as_i64().unwrap_or_default() as i{}", inner_code, w),

        Type::Number(Unsigned(w)) =>
            format!("{}.as_u64().unwrap_or_default() as u{}", inner_code, w),

        Type::Number(Float(w)) => match w {
            32 => format!("{}.as_f64().unwrap_or_default() as f32", inner_code),
            64 => format!("{}.as_f64().unwrap_or_default()", inner_code),
            _ => format!("{}.as_f64().unwrap_or_default() as f64", inner_code),
        },

        Type::String =>
            format!("{}.as_str().unwrap_or_default().to_string()", inner_code),

        Type::Bool =>
            format!("{}.as_bool().unwrap_or_default()", inner_code),

        Type::Custom(name) =>
            format!("serde_json::from_value::<{}>({}.clone()).unwrap_or_default()", name, inner_code),

        Type::Container(ContainerKind::Array, inner) => format!(
            "serde_json::from_value::<Vec<{}>>({}).unwrap_or_default()",
            inner.get(0).map_or("Value".to_string(), |t| t.to_rust_type()),
            inner_code
        ),

        Type::Container(ContainerKind::Map, inner) => format!(
            "serde_json::from_value::<HashMap<{}, {}>>({}).unwrap_or_default()",
            inner.get(0).map_or("String".to_string(), |k| k.to_rust_type()),
            inner.get(1).map_or("Value".to_string(), |v| v.to_rust_type()),
            inner_code
        ),

        _ => format!("{}.clone()", inner_code),
    }
}

                    _ => {
                        eprintln!("Warning: Unhandled cast from {:?} to {:?}", inner_type, target_type);
                        format!("({} as {})", inner_code, target_type.to_rust_type())
                    }
                }
            }
          Expr::Index(base, key) => {
    let base_type = script.infer_expr_type(base, current_func);
    let base_code = base.to_rust(needs_self, script, None, current_func);
    // Key type inference for Map access should be specific, otherwise it defaults to String
    let key_code = if let Some(Type::Container(ContainerKind::Map, inner_types)) = &base_type {
        let key_ty = inner_types.get(0).unwrap_or(&Type::String);
        key.to_rust(needs_self, script, Some(key_ty), current_func)
    } else {
        // For arrays or objects, assume string key for now (or other default)
        key.to_rust(needs_self, script, Some(&Type::String), current_func)
    };

    match base_type {
        // ----------------------------------------------------------
        // ✅ Typed HashMap<K,V>
        // ----------------------------------------------------------
        Some(Type::Container(ContainerKind::Map, ref inner_types)) => {
            let key_ty = inner_types.get(0).unwrap_or(&Type::String);
            // No need to re-infer key_code, already done above with correct type
            let final_key_code = if *key_ty == Type::String {
                format!("{}.as_str()", key_code)
            } else {
                format!("&{}", key_code)
            };
            format!("{}.get({}).cloned().unwrap_or_default()", base_code, final_key_code)
        }

        // ----------------------------------------------------------
        // ✅ Dynamic JSON object (serde_json::Value)
        // ----------------------------------------------------------
        Some(Type::Object) => {
            // Produces a `Value`, good for later .as_* casts
            format!("{}[{}].clone()", base_code, key_code)
        }

        // ----------------------------------------------------------
        // ✅ Arrays: differentiate typed Vec<T> vs. Vec<Value>
        // ----------------------------------------------------------
        Some(Type::Container(ContainerKind::Array, _)) => { // inner_types not needed for codegen here, inference does the heavy lifting
            let index_code = key.to_rust(
                needs_self,
                script,
                Some(&Type::Number(NumberKind::Unsigned(32))),
                current_func,
            );
            // Result from .get() is cloned, so it's a T or Value, handled by infer_expr_type
            format!("{}.get({} as usize).cloned().unwrap_or_default()", base_code, index_code)
        }

        // ----------------------------------------------------------
        // ✅ Fixed-size array: [T; N]
        // ----------------------------------------------------------
        Some(Type::Container(ContainerKind::FixedArray(_), _)) => { // inner_types not needed for codegen here
            let index_code = key.to_rust(
                needs_self,
                script,
                Some(&Type::Number(NumberKind::Unsigned(32))),
                current_func,
            );
            // Result from .get() is cloned, so it's a T or Value, handled by infer_expr_type
            format!("{}.get({} as usize).cloned().unwrap_or_default()", base_code, index_code)
        }

        // ----------------------------------------------------------
        // Invalid or unsupported index base
        // ----------------------------------------------------------
        Some(Type::Custom(_)) => "/* invalid index on struct */".to_string(),
        _ => "/* unsupported index expression */".to_string(),
    }
}
            Expr::ObjectLiteral(items) => {
                let pairs: Vec<_> = items.iter()
                    .map(|(k, v)| format!(
                        "\"{}\": {}",
                        k.as_deref().unwrap_or(""),
                        v.to_rust(needs_self, script, None, current_func)
                    ))
                    .collect();
                format!("json!({{ {} }})", pairs.join(", "))
            }
        }
    }

    fn contains_self(&self) -> bool {
        match self {
            Expr::SelfAccess => true,
            Expr::MemberAccess(base, _) => base.contains_self(),
            Expr::BinaryOp(left, _, right) => left.contains_self() || right.contains_self(),
            Expr::Call(target, args) => {
                target.contains_self() || args.iter().any(|arg| arg.contains_self())
            }
            _ => false,
        }
    }

    pub fn contains_api_call(&self, script: &Script) -> bool {
        match self {
            Expr::ApiCall(..) => true,
            Expr::MemberAccess(base, _) => base.contains_api_call(script),
            Expr::BinaryOp(l, _, r) => l.contains_api_call(script) || r.contains_api_call(script),
            Expr::Call(target, args) => {
                target.contains_api_call(script)
                    || args.iter().any(|a| a.contains_api_call(script))
            }
            Expr::ContainerLiteral(_, data) => match data {
                ContainerLiteralData::Array(elements) => {
                    elements.iter().any(|e| e.contains_api_call(script))
                }
                ContainerLiteralData::Map(pairs) => {
                    pairs.iter().any(|(k, v)| k.contains_api_call(script) || v.contains_api_call(script))
                }
                ContainerLiteralData::FixedArray(_, elements) => {
                    elements.iter().any(|e| e.contains_api_call(script))
                }
            }
            _ => false,
        }
    }

    fn get_target_name(expr: &Expr) -> Option<&str> {
        match expr {
            Expr::Ident(n) => Some(n.as_str()),
            Expr::MemberAccess(_, n) => Some(n.as_str()),
            _ => None,
        }
    }
}

impl Literal {
    fn to_rust(&self, expected_type: Option<&Type>) -> String {
        match self {
            Literal::Number(raw) => {
                match expected_type {
                    Some(Type::Number(NumberKind::Signed(w))) => format!("{}i{}", raw, w),
                    Some(Type::Number(NumberKind::Unsigned(w))) => format!("{}u{}", raw, w),
                    Some(Type::Number(NumberKind::Float(w))) => match w {
                        32 => format!("{}f32", raw),
                        64 => format!("{}f64", raw),
                        _ => format!("{}f32", raw),
                    },
                    Some(Type::Number(NumberKind::Decimal)) => {
                        format!("Decimal::from_str(\"{}\").unwrap()", raw)
                    },
                    Some(Type::Number(NumberKind::BigInt)) => {
                        format!("BigInt::from_str(\"{}\").unwrap()", raw)
                    },
                    _ => format!("{}f32", raw)
                }
            }

            Literal::String(s) => format!("String::from(\"{}\")", s),
            
            Literal::Bool(b) => b.to_string(),
            
            Literal::Interpolated(s) => {
                let re = Regex::new(r"\{([A-Za-z_][A-Za-z0-9_]*)\}").unwrap();
                let mut fmt = String::new();
                let mut args = Vec::new();
                let mut last = 0;

                for cap in re.captures_iter(s) {
                    let m = cap.get(0).unwrap();
                    fmt.push_str(&s[last..m.start()]);
                    fmt.push_str("{}");
                    last = m.end();
                    args.push(cap[1].to_string());
                }
                fmt.push_str(&s[last..]);

                if args.is_empty() {
                    format!("\"{}\"", fmt)
                } else {
                    format!("format!(\"{}\", {})", fmt, args.join(", "))
                }
            }
        }
    }
}

impl Op {
    fn to_rust(&self) -> &'static str {
        match self {
            Op::Add => "+",
            Op::Sub => "-",
            Op::Mul => "*",
            Op::Div => "/",
        }
    }
    
    fn to_rust_assign(&self) -> &'static str {
        match self {
            Op::Add => "+",
            Op::Sub => "-",
            Op::Mul => "*",
            Op::Div => "/",
        }
    }
}

pub fn implement_script_boilerplate(
    struct_name: &str,
    script_vars: &[Variable],
    functions: &[Function],
) -> String {
    let mut out = String::with_capacity(8192);
    let mut get_entries = String::with_capacity(512);
    let mut set_entries = String::with_capacity(512);
    let mut apply_entries = String::with_capacity(512);
    let mut dispatch_entries = String::with_capacity(4096);

    let mut public_var_count = 0;
    let mut exposed_var_count = 0;

    //----------------------------------------------------
    // Generate VAR GET, SET, APPLY tables
    //----------------------------------------------------
    for var in script_vars {
        let name = &var.name;
        let var_id = string_to_u64(name);
        let (accessor, conv) = var.json_access();

        // If public, generate GET and SET entries
        if var.is_public {
            public_var_count += 1;

            // ------------------------------
            // Special casing for Containers (GET)
            // ------------------------------
            if let Some(Type::Container(kind, _elem_types)) = &var.typ {
                match kind {
                    ContainerKind::Array | ContainerKind::FixedArray(_) | ContainerKind::Map => {
                        writeln!(
                            get_entries,
                            "        {var_id}u64 => |script: &{struct_name}| -> Option<Value> {{
                                Some(serde_json::to_value(&script.{name}).unwrap_or_default())
                            }},"
                        ).unwrap();
                    }
                }
            } else {
                writeln!(
                    get_entries,
                    "        {var_id}u64 => |script: &{struct_name}| -> Option<Value> {{
                        Some(json!(script.{name}))
                    }},"
                ).unwrap();
            }

            // ------------------------------
            // Special casing for Containers (SET)
            // ------------------------------
            if let Some(Type::Container(kind, elem_types)) = &var.typ {
                match kind {
                    ContainerKind::Array => {
                        let elem_ty = elem_types.get(0).unwrap_or(&Type::Object);
                        let elem_rs = elem_ty.to_rust_type();
                        if *elem_ty != Type::Object {
                            writeln!(
                                set_entries,
                                "        {var_id}u64 => |script: &mut {struct_name}, val: Value| -> Option<()> {{
                                    if let Ok(vec_typed) = serde_json::from_value::<Vec<{elem_rs}>>(val) {{
                                        script.{name} = vec_typed;
                                        return Some(());
                                    }}
                                    None
                                }},"
                            ).unwrap();
                        } else {
                            writeln!(
                                set_entries,
                                "        {var_id}u64 => |script: &mut {struct_name}, val: Value| -> Option<()> {{
                                    if let Some(v) = val.as_array() {{
                                        script.{name} = v.clone();
                                        return Some(());
                                    }}
                                    None
                                }},"
                            ).unwrap();
                        }
                    }
                    ContainerKind::FixedArray(size) => {
                        let elem_ty = elem_types.get(0).unwrap_or(&Type::Object);
                        let elem_rs = elem_ty.to_rust_type();
                        if *elem_ty != Type::Object {
                            writeln!(
                                set_entries,
                                "        {var_id}u64 => |script: &mut {struct_name}, val: Value| -> Option<()> {{
                                    if let Ok(arr_typed) = serde_json::from_value::<[{elem_rs}; {size}]>(val) {{
                                        script.{name} = arr_typed;
                                        return Some(());
                                    }}
                                    None
                                }},"
                            ).unwrap();
                        } else {
                            writeln!(
                                set_entries,
                                "        {var_id}u64 => |script: &mut {struct_name}, val: Value| -> Option<()> {{
                                    if let Some(v) = val.as_array() {{
                                        let mut out: [{elem_rs}; {size}] = [Default::default(); {size}];
                                        for (i, el) in v.iter().enumerate().take({size}) {{
                                            out[i] = serde_json::from_value::<{elem_rs}>(el.clone()).unwrap_or_default();
                                        }}
                                        script.{name} = out;
                                        return Some(());
                                    }}
                                    None
                                }},"
                            ).unwrap();
                        }
                    }
                    ContainerKind::Map => {
                        let key_ty = elem_types.get(0).unwrap_or(&Type::String);
                        let val_ty = elem_types.get(1).unwrap_or(&Type::Object);
                        let key_rs = key_ty.to_rust_type();
                        let val_rs = val_ty.to_rust_type();

                        if *val_ty != Type::Object {
                            writeln!(
                                set_entries,
                                "        {var_id}u64 => |script: &mut {struct_name}, val: Value| -> Option<()> {{
                                    if let Ok(map_typed) = serde_json::from_value::<HashMap<{key_rs}, {val_rs}>>(val) {{
                                        script.{name} = map_typed;
                                        return Some(());
                                    }}
                                    None
                                }},"
                            ).unwrap();
                        } else {
                            writeln!(
                                set_entries,
                                "        {var_id}u64 => |script: &mut {struct_name}, val: Value| -> Option<()> {{
                                    if let Some(v) = val.as_object() {{
                                        script.{name} = v.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                                        return Some(());
                                    }}
                                    None
                                }},"
                            ).unwrap();
                        }
                    }
                }
            } else {
                if accessor == "__CUSTOM__" {
                    let type_name = &conv;
                    writeln!(
                        set_entries,
                        "        {var_id}u64 => |script: &mut {struct_name}, val: Value| -> Option<()> {{
                            if let Ok(v) = serde_json::from_value::<{type_name}>(val) {{
                                script.{name} = v;
                                return Some(());
                            }}
                            None
                        }},"
                    ).unwrap();
                } else {
                    writeln!(
                        set_entries,
                        "        {var_id}u64 => |script: &mut {struct_name}, val: Value| -> Option<()> {{
                            if let Some(v) = val.{accessor}() {{
                                script.{name} = v{conv};
                                return Some(());
                            }}
                            None
                        }},"
                    ).unwrap();
                }
            }
        }

        // If exposed, generate APPLY entries
        if var.is_exposed {
            exposed_var_count += 1;

            // ------------------------------
            // Special casing for Containers (APPLY)
            // ------------------------------
            if let Some(Type::Container(kind, elem_types)) = &var.typ {
                match kind {
                    ContainerKind::Array => {
                        let elem_ty = elem_types.get(0).unwrap_or(&Type::Object);
                        let elem_rs = elem_ty.to_rust_type();
                        if *elem_ty != Type::Object {
                            writeln!(
                                apply_entries,
                                "        {var_id}u64 => |script: &mut {struct_name}, val: &Value| {{
                                    if let Ok(vec_typed) = serde_json::from_value::<Vec<{elem_rs}>>(val.clone()) {{
                                        script.{name} = vec_typed;
                                    }}
                                }},"
                            ).unwrap();
                        } else {
                            writeln!(
                                apply_entries,
                                "        {var_id}u64 => |script: &mut {struct_name}, val: &Value| {{
                                    if let Some(v) = val.as_array() {{
                                        script.{name} = v.clone();
                                    }}
                                }},"
                            ).unwrap();
                        }
                    }
                    ContainerKind::FixedArray(size) => {
                        let elem_ty = elem_types.get(0).unwrap_or(&Type::Object);
                        let elem_rs = elem_ty.to_rust_type();
                        if *elem_ty != Type::Object {
                            writeln!(
                                apply_entries,
                                "        {var_id}u64 => |script: &mut {struct_name}, val: &Value| {{
                                    if let Ok(arr_typed) = serde_json::from_value::<[{elem_rs}; {size}]>(val.clone()) {{
                                        script.{name} = arr_typed;
                                    }}
                                }},"
                            ).unwrap();
                        } else {
                            writeln!(
                                apply_entries,
                                "        {var_id}u64 => |script: &mut {struct_name}, val: &Value| {{
                                    if let Some(v) = val.as_array() {{
                                        let mut out: [{elem_rs}; {size}] = [Default::default(); {size}];
                                        for (i, el) in v.iter().enumerate().take({size}) {{
                                            out[i] = serde_json::from_value::<{elem_rs}>(el.clone()).unwrap_or_default();
                                        }}
                                        script.{name} = out;
                                    }}
                                }},"
                            ).unwrap();
                        }
                    }
                    ContainerKind::Map => {
                        let key_ty = elem_types.get(0).unwrap_or(&Type::String);
                        let val_ty = elem_types.get(1).unwrap_or(&Type::Object);
                        let key_rs = key_ty.to_rust_type();
                        let val_rs = val_ty.to_rust_type();

                        if *val_ty != Type::Object {
                            writeln!(
                                apply_entries,
                                "        {var_id}u64 => |script: &mut {struct_name}, val: &Value| {{
                                    if let Ok(map_typed) = serde_json::from_value::<HashMap<{key_rs}, {val_rs}>>(val.clone()) {{
                                        script.{name} = map_typed;
                                    }}
                                }},"
                            ).unwrap();
                        } else {
                            writeln!(
                                apply_entries,
                                "        {var_id}u64 => |script: &mut {struct_name}, val: &Value| {{
                                    if let Some(v) = val.as_object() {{
                                        script.{name} = v.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                                    }}
                                }},"
                            ).unwrap();
                        }
                    }
                }
            } else {
                if accessor == "__CUSTOM__" {
                    let type_name = &conv;
                    writeln!(
                        apply_entries,
                        "        {var_id}u64 => |script: &mut {struct_name}, val: &Value| {{
                            if let Ok(v) = serde_json::from_value::<{type_name}>(val.clone()) {{
                                script.{name} = v;
                            }}
                        }},"
                    ).unwrap();
                } else {
                    writeln!(
                        apply_entries,
                        "        {var_id}u64 => |script: &mut {struct_name}, val: &Value| {{
                            if let Some(v) = val.{accessor}() {{
                                script.{name} = v{conv};
                            }}
                        }},"
                    ).unwrap();
                }
            }
        }
    }

    //----------------------------------------------------
    // FUNCTION DISPATCH TABLE GENERATION
    //----------------------------------------------------
    for func in functions {
        if func.is_trait_method {
            continue;
        }

        let func_name = &func.name;
        let func_id = string_to_u64(func_name);

        let mut param_parsing = String::new();
        let mut param_list = String::new();

        if !func.params.is_empty() {
            for (i, param) in func.params.iter().enumerate() {
                let param_name = &param.name;
                let parse_code = match &param.typ {
                    Type::String => format!(
                        "let {param_name} = params.get({i})
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();\n"
                    ),
                    Type::Number(NumberKind::Signed(w)) =>
                        format!("let {param_name} = params.get({i})
                            .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
                            .unwrap_or_default() as i{w};\n"),
                    Type::Number(NumberKind::Unsigned(w)) =>
                        format!("let {param_name} = params.get({i})
                            .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
                            .unwrap_or_default() as u{w};\n"),
                    Type::Number(NumberKind::Float(32)) =>
                        format!("let {param_name} = params.get({i})
                            .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))
                            .unwrap_or_default() as f32;\n"),
                    Type::Number(NumberKind::Float(64)) =>
                        format!("let {param_name} = params.get({i})
                            .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))
                            .unwrap_or_default();\n"),
                    Type::Bool => format!(
                        "let {param_name} = params.get({i})
                            .and_then(|v| v.as_bool())
                            .unwrap_or_default();\n"
                    ),
                    Type::Custom(tn) if tn == "Signal" => format!(
                        "let {param_name} = params.get({i})
                            .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
                            .unwrap_or_default() as u64;\n"
                    ),
                    Type::Custom(tn) => format!(
                        "let {param_name} = params.get({i})
                            .and_then(|v| serde_json::from_value::<{tn}>(v.clone()).ok())
                            .unwrap_or_default();\n"
                    ),
                    _ => format!("let {param_name} = Default::default();\n"),
                };
                param_parsing.push_str(&parse_code);
            }

            param_list = func
                .params
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            param_list.push_str(", ");
        }

        write!(
            dispatch_entries,
            "        {func_id}u64 => | script: &mut {struct_name}, params: &[Value], api: &mut ScriptApi<'_>| {{
{param_parsing}            script.{func_name}({param_list}api, true);
        }},\n"
        )
        .unwrap();
    }

    //----------------------------------------------------
    // FINAL OUTPUT
    //----------------------------------------------------
    write!(
        out,
        r#"
impl ScriptObject for {struct_name} {{
    fn set_node_id(&mut self, id: Uuid) {{
        self.node.id = id;
    }}

    fn get_node_id(&self) -> Uuid {{
        self.node.id
    }}

    fn get_var(&self, var_id: u64) -> Option<Value> {{
            VAR_GET_TABLE.get(&var_id).and_then(|f| f(self))
    }}

    fn set_var(&mut self, var_id: u64, val: Value) -> Option<()> {{
        VAR_SET_TABLE.get(&var_id).and_then(|f| f(self, val))
    }}

    fn apply_exposed(&mut self, hashmap: &HashMap<u64, Value>) {{
        for (var_id, val) in hashmap.iter() {{
            if let Some(f) = VAR_APPLY_TABLE.get(var_id) {{
                f(self, val);
            }}
        }}
    }}

    fn call_function(&mut self, id: u64, api: &mut ScriptApi<'_>, params: &SmallVec<[Value; 3]>) {{
        if let Some(f) = DISPATCH_TABLE.get(&id) {{
            f(self, params, api);
        }}
    }}
}}

// =========================== Static PHF Dispatch Tables ===========================

static VAR_GET_TABLE: phf::Map<u64, fn(&{struct_name}) -> Option<Value>> = phf::phf_map! {{
{get_entries}}};

static VAR_SET_TABLE: phf::Map<u64, fn(&mut {struct_name}, Value) -> Option<()>> = phf::phf_map! {{
{set_entries}}};

static VAR_APPLY_TABLE: phf::Map<u64, fn(&mut {struct_name}, &Value)> = phf::phf_map! {{
{apply_entries}}};

static DISPATCH_TABLE: phf::Map<u64, fn(&mut {struct_name}, &[Value], &mut ScriptApi<'_>)> = phf::phf_map! {{
{dispatch_entries}}};"#,
        struct_name = struct_name,
        get_entries = get_entries,
        set_entries = set_entries,
        apply_entries = apply_entries,
        dispatch_entries = dispatch_entries,
    )
    .unwrap();

    out
}

pub fn write_to_crate(
    project_path: &Path,
    contents: &str,
    struct_name: &str,
) -> Result<(), String> {
    let base_path = project_path.join(".perro/scripts/src");
    let lower_name = struct_name.to_lowercase();
    let file_path = base_path.join(format!("{}.rs", lower_name));

    fs::create_dir_all(&base_path).map_err(|e| format!("Failed to create dir: {}", e))?;

    fs::write(&file_path, contents)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    let lib_rs_path = base_path.join("lib.rs");
    let mut current_content = fs::read_to_string(&lib_rs_path).unwrap_or_default();

    let mod_line = format!("pub mod {};", lower_name);
    if !current_content.contains(&mod_line) {
        current_content = current_content.replace(
            "// __PERRO_MODULES__",
            &format!("{}\n// __PERRO_MODULES__", mod_line),
        );
    }

    let import_line = format!("use {}::{}_create_script;", lower_name, lower_name);
    if !current_content.contains(&import_line) {
        current_content = current_content.replace(
            "// __PERRO_IMPORTS__",
            &format!("{}\n// __PERRO_IMPORTS__", import_line),
        );
    }

    let registry_line = format!(
        "    map.insert(\"{}\".to_string(), {}_create_script as CreateFn);\n",
        lower_name, lower_name
    );
    if !current_content.contains(&registry_line) {
        current_content = current_content.replace(
            "// __PERRO_REGISTRY__",
            &format!("{}    // __PERRO_REGISTRY__", registry_line),
        );
    }

    fs::write(&lib_rs_path, current_content)
        .map_err(|e| format!("Failed to update lib.rs: {}", e))?;

    let should_compile_path = project_path.join(".perro/scripts/should_compile");
    fs::write(should_compile_path, "true")
        .map_err(|e| format!("Failed to write should_compile: {}", e))?;

    Ok(())
}

fn extract_create_script_fn_name(contents: &str) -> Option<String> {
    for line in contents.lines() {
        if line.contains("pub extern \"C\" fn") && line.contains("_create_script") {
            if let Some(start) = line.find("fn ") {
                let after_fn = &line[start + 3..];
                if let Some(end) = after_fn.find('(') {
                    let fn_name = after_fn[..end].trim();
                    if fn_name.ends_with("_create_script") {
                        return Some(fn_name.to_string());
                    }
                }
            }
        }
    }
    None
}

pub fn derive_rust_perro_script(project_path: &Path, code: &str, struct_name: &str) -> Result<(), String> {
    let marker_re = Regex::new(r"///\s*@PerroScript").unwrap();
    let marker_pos = match marker_re.find(code) {
        Some(m) => m.end(),
        None => return write_to_crate(project_path, code, struct_name),
    };

    let struct_after_marker_re = Regex::new(r"struct\s+(\w+)\s*\{([^}]*)\}").unwrap();
    let captures = struct_after_marker_re
        .captures(&code[marker_pos..])
        .ok_or_else(|| "Could not find struct after @PerroScript".to_string())?;

    let actual_struct_name_from_struct = captures[1].to_string();
    let struct_body = captures[2].to_string();

    let mut variables = Vec::new(); // This variable is no longer needed but its usage is removed below

    let expose_re = Regex::new(r"///\s*@expose[^\n]*\n\s*(?:pub\s+)?(\w+)\s*:\s*([^,]+),?").unwrap();
    for cap in expose_re.captures_iter(&struct_body) {
        let name = cap[1].to_string();
        let typ = cap[2].trim().to_string();
        // This old way of populating 'exposed' and 'variables' is no longer used,
        // as the parser now populates 'script_vars' directly with flags.
        // Keeping it for now as a comment for context of removal in future refactors.
        let mut is_pub = false;
        if cap[0].contains("pub") {is_pub = true;}
        variables.push(Variable {
            name: name.clone(),
            typ: Some(Variable::parse_type(&typ)),
            value: None,
            is_exposed: true,
            is_public: is_pub, 
        });
    }

    let pub_re = Regex::new(r"pub\s+(\w+)\s*:\s*([^,\n}]+)").unwrap();
    for cap in pub_re.captures_iter(&struct_body) {
        let name = cap[1].to_string();
        // This old way of populating 'variables' is no longer used.
        // Its functionality is now absorbed by 'script_vars' in the parser.
        if name == "node" || variables.iter().any(|v| v.name == name) {
            continue;
        }
        let typ = cap[2].trim().to_string();
        variables.push(Variable {
            name,
            typ: Some(Variable::parse_type(&typ)),
            value: None,
            is_exposed: false, // Not explicitly exposed
            is_public: true, // Explicitly pub
        }); 
    }

    let lower_name = struct_name.to_lowercase();

    let impl_script_re = Regex::new(r"impl\s+Script\s+for\s+(\w+)\s*\{").unwrap();
    let actual_struct_name = if let Some(cap) = impl_script_re.captures(&code) {
        cap[1].to_string()
    } else {
        to_pascal_case(struct_name)
    };

    // Extract function names from impl blocks
    let mut functions = Vec::new();
    
    // Find impl StructNameScript { ... } blocks (not trait impl blocks)
    let impl_block_re = Regex::new(&format!(
        r"impl\s+{}\s*\{{([^}}]*(?:\{{[^}}]*\}}[^}}]*)*)\}}",
        regex::escape(&format!("{}Script", to_pascal_case(struct_name)))
    )).unwrap();
    
if let Some(impl_cap) = impl_block_re.captures(&code) {
    let impl_body = &impl_cap[1];
    
    // Find all function definitions with their full signatures
    // Matches: fn function_name(&mut self, param: Type, ...) -> ReturnType {
    let fn_re = Regex::new(r"fn\s+(\w+)\s*\(([^)]*)\)(?:\s*->\s*([^{]+))?").unwrap();
    
    for fn_cap in fn_re.captures_iter(impl_body) {
        let fn_name = fn_cap[1].to_string();
        let params_str = fn_cap.get(2).map_or("", |m| m.as_str());
        let return_str = fn_cap.get(3).map_or("", |m| m.as_str().trim());
        
        // Parse parameters
        let mut params = Vec::new();
        
        // Split by comma and parse each parameter
        for param in params_str.split(',') {
            let param = param.trim();
            if param.is_empty() || param == "&mut self" || param == "&self" {
                continue;
            }
            
            // Remove 'mut ' prefix if present
            let param = param.strip_prefix("mut ").unwrap_or(param).trim();
            
            // Split by ':' to get name and type
            if let Some((name, typ_str)) = param.split_once(':') {
                let name = name.trim().to_string();
                let typ = Variable::parse_type(typ_str);
                
                params.push(Param {
                    name,
                    typ,
                });
            }
        }
        
        // Parse return type
        let return_type = if return_str.is_empty() {
            Type::Void
        } else {
            Variable::parse_type(return_str)
        };
        
        functions.push(Function {
            name: fn_name,
            is_trait_method: false,
            params,
            return_type,
            uses_self: false,
            body: vec![],
            locals: vec![],
        });
    }
}

    let final_contents = if let Some(actual_fn_name) = extract_create_script_fn_name(&code) {
        let expected_fn_name = format!("{}_create_script", lower_name);
        code.replace(&actual_fn_name, &expected_fn_name)
    } else {
        code.to_string()
    };

    let boilerplate = implement_script_boilerplate(&actual_struct_name,&variables, &functions);
    let combined = format!("{}\n\n{}", final_contents, boilerplate);

    write_to_crate(project_path, &combined, struct_name)
}