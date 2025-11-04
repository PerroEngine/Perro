// scripting/lang/codegen/rust.rs
#![allow(unused)]
#![allow(dead_code)]
use std::{fmt::format, fs, path::{Path, PathBuf}};
use std::fmt::Write as _;
use std::collections::HashMap;
use std::cell::RefCell;

use regex::Regex;

use crate::{asset_io::{ProjectRoot, get_project_root}, lang::ast::*, prelude::string_to_u64, script::Var};

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

        let to_ty = to.to_rust_type();
        if expr.ends_with(&to_ty) {
            return expr.to_string();
        }

        Stmt::generate_implicit_cast(expr, from, to)
    }

    pub fn is_struct_field(&self, name: &str) -> bool {
        self.variables.iter().any(|v| v.name == name)
            || self.exposed.iter().any(|v| v.name == name)
    }

    pub fn get_variable_type(&self, name: &str) -> Option<&Type> {
        if let Some(v) = self.variables.iter().find(|v| v.name == name) {
            return v.typ.as_ref();
        }
        if let Some(v) = self.exposed.iter().find(|v| v.name == name) {
            return v.typ.as_ref();
        }
        None
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
        // -------------------------------------------------------------
        // LITERALS
        // -------------------------------------------------------------
        Expr::Literal(lit) => self.infer_literal_type(lit, None),

        // -------------------------------------------------------------
        // IDENTIFIERS (variables, parameters, fields)
        // -------------------------------------------------------------
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

        // -------------------------------------------------------------
        // BINARY OPERATOR (a + b, a * b, etc.)
        // -------------------------------------------------------------
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

        // -------------------------------------------------------------
        // MEMBER ACCESS (foo.bar)
        // -------------------------------------------------------------
        Expr::MemberAccess(base, field) => {
            let base_type = self.infer_expr_type(base, current_func)?;
            self.get_member_type(&base_type, field)
        }

        // -------------------------------------------------------------
        // FUNCTION CALL
        // -------------------------------------------------------------
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

        // -------------------------------------------------------------
        // CAST
        // -------------------------------------------------------------
        Expr::Cast(_, target_type) => Some(target_type.clone()),

        // -------------------------------------------------------------
        // API CALL (Time.get_delta() etc.)
        // -------------------------------------------------------------
        Expr::ApiCall(api, _) => api.return_type(),

        // -------------------------------------------------------------
        // STRUCT INITIALIZATION (new MyStruct(...) or literal form)
        // -------------------------------------------------------------
        Expr::StructNew(ty_name, _fields) => {
            // The result of `new Struct(...)` is `Type::Custom("Struct")`
            Some(Custom(ty_name.clone()))
        }

        // -------------------------------------------------------------
        // SELF ACCESS
        // -------------------------------------------------------------
        Expr::SelfAccess => Some(Custom(self.node_type.clone())),

        // -------------------------------------------------------------
        // ARRAY or OBJECT LITERAL (future extension)
        // -------------------------------------------------------------
       Expr::ContainerLiteral(kind, elems) => Some(Type::Container(kind.clone())),

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
        match base_type {
            Type::Custom(type_name) if type_name == &self.node_type => {
                if let Some(exposed) = self.exposed.iter().find(|v| v.name == member) {
                    exposed.typ.clone()
                } else if let Some(var) = self.variables.iter().find(|v| v.name == member) {
                    var.typ.clone()
                } else {
                    None
                }
            }

            Type::Custom(type_name) => {
                if let Some(struct_def) = self.structs.iter().find(|s| &s.name == type_name) {
                    struct_def
                        .fields
                        .iter()
                        .find(|f| f.name == member)
                        .map(|f| f.typ.clone())
                } else {
                    None
                }
            }

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

        let exposed_fields: Vec<(&str, String, String)> = script.exposed.iter()
            .map(|exposed| {
                let name = exposed.name.as_str();
                let rust_type = exposed.rust_type();
                let default_val = exposed.default_value();
                (name, rust_type, default_val)
            })
            .collect();

        let variable_fields: Vec<(&str, String, String)> = script.variables.iter()
            .map(|var| {
                let name = var.name.as_str();
                let rust_type = var.rust_type(); 
                let default_val = var.default_value();
                (name, rust_type, default_val)
            })
            .collect();

        let mut merged_fields: HashMap<&str, (&String, &String, bool)> = HashMap::new();
        let mut field_order: Vec<&str> = Vec::new(); // preserve output order

        // Insert exposed first
        for (name, rust_type, default_val) in &exposed_fields {
            merged_fields.insert(name, (rust_type, default_val, true));
            field_order.push(name);
        }

        // Then add variables only if not already present
        for (name, rust_type, default_val) in &variable_fields {
            if !merged_fields.contains_key(name) {
                merged_fields.insert(name, (rust_type, default_val, false));
                field_order.push(name);
            }
        }

        // ========================================================================
        // {} - Main Script Structure
        // ========================================================================

        out.push_str("// ========================================================================\n");
        write!(out, "// {} - Main Script Structure\n", pascal_struct_name).unwrap();
        out.push_str("// ========================================================================\n\n");

        write!(out, "pub struct {}Script {{\n", pascal_struct_name).unwrap();
        write!(out, "    node: {},\n", script.node_type).unwrap();

        for name in &field_order {
            let (rust_type, _, _) = merged_fields.get(name).unwrap();
            write!(out, "    {}: {},\n", name, rust_type).unwrap();
        }


        out.push_str("}\n\n");

        out.push_str("// ========================================================================\n");
        write!(out, "// {} - Creator Function (FFI Entry Point)\n", pascal_struct_name).unwrap();
        out.push_str("// ========================================================================\n\n");
        
        out.push_str("#[unsafe(no_mangle)]\n");
        write!(out, "pub extern \"C\" fn {}_create_script() -> *mut dyn ScriptObject {{\n", struct_name.to_lowercase()).unwrap();
        write!(out, "    Box::into_raw(Box::new({}Script {{\n", pascal_struct_name).unwrap();
        if self.node_type == "Node" {
            write!(out, "        node: {}::new(\"{}\", None),\n", script.node_type, pascal_struct_name).unwrap();
        } else {
            write!(out, "        node: {}::new(\"{}\"),\n", script.node_type, pascal_struct_name).unwrap();
        }

       for name in &field_order {
            let (_, _, is_exposed) = merged_fields.get(name).unwrap();

            let init_code = if *is_exposed {
                script.exposed.iter()
                    .find(|e| e.name == **name)
                    .unwrap()
                    .rust_initialization(&script, current_func)
            } else {
                script.variables.iter()
                    .find(|v| v.name == **name)
                    .unwrap()
                    .rust_initialization(&script, current_func)
            };

            write!(out, "        {}: {},\n", name, init_code).unwrap();
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
            &script.exposed,
            &script.variables,
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
        writeln!(out, "    fn {}(&mut self, api: &mut ScriptApi<'_>) {{", self.name).unwrap();

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
                    expr.to_rust(needs_self, script, current_func)
                } else {
                    if var.typ.is_some() {
                        var.default_value()
                    } else {
                        String::new()
                    }
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
            }
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
    pub fn to_rust(&self, needs_self: bool, script: &Script, expected_type: Option<&Type>, current_func: Option<&Function>) -> String {
        use ContainerKind::*;

        match self {
           Expr::Ident(name) => {
            let is_local = current_func
                .and_then(|f| {
                    Some(
                        f.locals.iter().any(|v| v.name == *name)
                        || f.params.iter().any(|p| p.name == *name),
                    )
                }).unwrap_or(false);

            let is_field = script.variables.iter().any(|v| v.name == *name)
                || script.exposed.iter().any(|v| v.name == *name);

            if !is_local && is_field && !name.starts_with("self.") {
                format!("self.{}", name)
            } else {
                name.clone()
            }
        }
            Expr::Literal(lit) => {
                if let Some(expected) = expected_type {
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
                    Some(Type::Container(ContainerKind::Object)) => {
                        // dynamic object (serde_json::Value, like json!({ ... }))
                        let base_code = base.to_rust(needs_self, script, None, current_func);
                        format!("{}[\"{}\"].clone()", base_code, field)
                    }
                    Some(Type::Container(ContainerKind::HashMap)) => {
                        // HashMap-style map access
                        let base_code = base.to_rust(needs_self, script, None, current_func);
                        format!("{}[\"{}\"].clone()", base_code, field)
                    }
                    Some(Type::Container(ContainerKind::Array)) |
                    Some(Type::Container(ContainerKind::FixedArray(_))) => {
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
           Expr::ContainerLiteral(kind, elems) => match kind {
                HashMap => {
                    let entries: Vec<_> = elems.iter()
                        .map(|(k, v)| format!(
                            "({}, json!({}))",  // <--- wrap value in json!(...)
                            k.as_ref().map(|s| format!("\"{}\".to_string()", s))
                                .unwrap_or_else(|| "\"\".to_string()".into()),
                            v.to_rust(needs_self, script, None, current_func)
                        ))
                        .collect();
                    format!("HashMap::from([{}])", entries.join(", "))
                }
                Array => {
                    let elements: Vec<_> = elems.iter()
                        .map(|(_, v)| format!("json!({})", v.to_rust(needs_self, script, None, current_func)))
                        .collect();
                    format!("vec![{}]", elements.join(", "))
                }
                Object => {
                    let pairs: Vec<_> = elems.iter()
                        .map(|(k, v)| format!(
                            "\"{}\": {}",
                            k.as_deref().unwrap_or(""),
                            v.to_rust(needs_self, script, None, current_func)
                        ))
                        .collect();
                    format!("json!({{ {} }})", pairs.join(", "))
                }
                FixedArray(size) => {
                    let elements: Vec<_> = elems.iter()
                        .map(|(_, v)| v.to_rust(needs_self, script, None, current_func))
                        .collect();

                    // Rust: fill up to fixed size; pad with Default::default() if needed
                    let mut body = elements.clone();
                    while body.len() < *size {
                        body.push("Default::default()".into());
                    }
                    if body.len() > *size {
                        body.truncate(*size);
                    }
                    format!("[{}]", body.join(", "))
                }
            },
            Expr::StructNew(ty, fields) => {
                if fields.is_empty() {
                    // Simple default constructor
                    format!("{}::default()", ty)
                } else {
                    // Field-aware initialization
                    let field_inits: Vec<String> = fields
                        .iter()
                        .map(|(fname, fexpr)| {
                            // Look up the declared field type
                            let field_type = script.get_struct_field_type(ty, fname);
                            // Generate expression using that type as hint
                            let mut expr_code = fexpr.to_rust(needs_self, script, field_type.as_ref(), current_func);
                            
                            // 🔹 Clone non-Copy types when they're identifiers or member accesses
                            let expr_type = script.infer_expr_type(fexpr, current_func);
                            let should_clone = matches!(fexpr, Expr::Ident(_) | Expr::MemberAccess(..))
                                && expr_type
                                    .as_ref()
                                    .map_or(false, |ty| ty.requires_clone());
                            
                            if should_clone {
                                expr_code = format!("{}.clone()", expr_code);
                            }
                            
                            format!("{}: {}", fname, expr_code)
                        })
                        .collect();

                    format!("{} {{ {}, ..Default::default() }}",
                        ty,
                        field_inits.join(", ")
                    )
                }
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

                    _ => {
                        eprintln!("Warning: Unhandled cast from {:?} to {:?}", inner_type, target_type);
                        format!("({} as {})", inner_code, target_type.to_rust_type())
                    }
                }
            }
            Expr::Index(base, key) => {
    let base_type = script.infer_expr_type(base, current_func);
    let base_code = base.to_rust(needs_self, script, None, current_func);
    let key_code = key.to_rust(needs_self, script, Some(&Type::String), current_func);

    match base_type {
        Some(Type::Container(ContainerKind::HashMap)) => {
            // HashMap: .get(key).cloned().unwrap_or_default()
            format!("{}.get({}).cloned().unwrap_or_default()", base_code, key_code)
        }
        Some(Type::Container(ContainerKind::Object)) => {
            // serde_json::Value or JSON-style dynamic object
            format!("{}[{}].clone()", base_code, key_code)
        }
        Some(Type::Container(ContainerKind::Array))
        | Some(Type::Container(ContainerKind::FixedArray(_))) => {
            // Array or FixedArray: index must be integer; try to cast
            let index_code = key.to_rust(needs_self, script, Some(&Type::Number(NumberKind::Unsigned(32))), current_func);
            format!("{}.get({} as usize).cloned().unwrap_or_default()", base_code, index_code)
        }
        Some(Type::Custom(_)) => {
            // Custom structs do not support indexing
            "/* invalid index on struct */".to_string()
        }
        _ => {
            "/* unsupported index expression */".to_string()
        }
    }
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
            Expr::ContainerLiteral(_, elems) => elems.iter().any(|(_, e)| e.contains_api_call(script)),
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
    exposed: &[Variable],
    variables: &[Variable],
    functions: &[Function],
) -> String {
    let mut out = String::with_capacity(8192);
    let mut get_entries = String::with_capacity(512);
    let mut set_entries = String::with_capacity(512);
    let mut apply_entries = String::with_capacity(512);
    let mut dispatch_entries = String::with_capacity(4096);

    //----------------------------------------------------
    // Generate VAR GET, SET, APPLY tables
    //----------------------------------------------------
    for var in variables {
        let name = &var.name;
        let var_id = string_to_u64(name);
        let (accessor, conv) = var.json_access();

        // ---- GET ----
        write!(
            get_entries,
            "        m.insert({var_id}u64, |script: &{struct_name}| -> Option<Value> {{
            Some(json!(script.{name}))
        }});\n"
        )
        .unwrap();

        // ---- SET ----
        if accessor == "__CUSTOM__" {
            let type_name = &conv;
            write!(
                set_entries,
                "        m.insert({var_id}u64, |script: &mut {struct_name}, val: Value| -> Option<()> {{
            if let Ok(v) = serde_json::from_value::<{type_name}>(val) {{
                script.{name} = v;
                return Some(());
            }}
            None
        }});\n"
            )
            .unwrap();
        } else {
            write!(
                set_entries,
                "        m.insert({var_id}u64, |script: &mut {struct_name}, val: Value| -> Option<()> {{
            if let Some(v) = val.{accessor}() {{
                script.{name} = v{conv};
                return Some(());
            }}
            None
        }});\n"
            )
            .unwrap();
        }
    }

       for var in exposed {
        let name = &var.name;
        let var_id = string_to_u64(name);
        let (accessor, conv) = var.json_access();

        if accessor == "__CUSTOM__" {
            let type_name = &conv;
            writeln!(
                apply_entries,
                "        m.insert({var_id}u64, |script: &mut {struct_name}, val: &Value| {{
            if let Ok(v) = serde_json::from_value::<{type_name}>(val.clone()) {{
                script.{name} = v;
            }}
        }});"
            ).unwrap();
        } else {
            writeln!(
                apply_entries,
                "        m.insert({var_id}u64, |script: &mut {struct_name}, val: &Value| {{
            if let Some(v) = val.{accessor}() {{
                script.{name} = v{conv};
            }}
        }});"
            ).unwrap();
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
                    Type::String => {
                        format!(
                            "let {param_name} = params.get({i})
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_default();\n"
                        )
                    }
                    Type::Number(NumberKind::Signed(w)) => {
                        format!(
                            "let {param_name} = params.get({i})
                .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
                .unwrap_or_default() as i{w};\n"
                        )
                    }
                    Type::Number(NumberKind::Unsigned(w)) => {
                        format!(
                            "let {param_name} = params.get({i})
                .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
                .unwrap_or_default() as u{w};\n"
                        )
                    }
                    Type::Number(NumberKind::Float(32)) => {
                        format!(
                            "let {param_name} = params.get({i})
                .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))
                .unwrap_or_default() as f32;\n"
                        )
                    }
                    Type::Number(NumberKind::Float(64)) => {
                        format!(
                            "let {param_name} = params.get({i})
                .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))
                .unwrap_or_default();\n"
                        )
                    }
                    Type::Bool => {
                        format!(
                            "let {param_name} = params.get({i})
                .and_then(|v| v.as_bool())
                .unwrap_or_default();\n"
                        )
                    }
                    Type::Custom(tn) if tn == "Signal" => {
                        format!(
                            "let {param_name} = params.get({i})
                .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
                .unwrap_or_default() as u64;\n"
                        )
                    }
                    Type::Custom(tn) => {
                        format!(
                            "let {param_name} = params.get({i})
                .and_then(|v| serde_json::from_value::<{tn}>(v.clone()).ok())
                .unwrap_or_default();\n"
                        )
                    }
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
            "        m.insert({func_id}u64,
            |this: &mut {struct_name}, params: &[Value], api: &mut ScriptApi<'_>| {{
{param_parsing}            this.{func_name}({param_list}api, true);
        }});\n"
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

// =========================== Static Dispatch Tables ===========================

static VAR_GET_TABLE: once_cell::sync::Lazy<
    std::collections::HashMap<u64, fn(&{struct_name}) -> Option<Value>>
> = once_cell::sync::Lazy::new(|| {{
    use std::collections::HashMap;
    let mut m: HashMap<u64, fn(&{struct_name}) -> Option<Value>> =
        HashMap::with_capacity({var_count});
{get_entries}    m
}});

static VAR_SET_TABLE: once_cell::sync::Lazy<
    std::collections::HashMap<u64, fn(&mut {struct_name}, Value) -> Option<()>>
> = once_cell::sync::Lazy::new(|| {{
    use std::collections::HashMap;
    let mut m: HashMap<u64, fn(&mut {struct_name}, Value) -> Option<()>> =
        HashMap::with_capacity({var_count});
{set_entries}    m
}});

static VAR_APPLY_TABLE: once_cell::sync::Lazy<
    std::collections::HashMap<u64, fn(&mut {struct_name}, &Value)>
> = once_cell::sync::Lazy::new(|| {{
    use std::collections::HashMap;
    let mut m: HashMap<u64, fn(&mut {struct_name}, &Value)> =
        HashMap::with_capacity({var_count});
{apply_entries}    m
}});

static DISPATCH_TABLE: once_cell::sync::Lazy<
    std::collections::HashMap<u64,
        fn(&mut {struct_name}, &[Value], &mut ScriptApi<'_>)
    >
> = once_cell::sync::Lazy::new(|| {{
    use std::collections::HashMap;
    let mut m:
        HashMap<u64, fn(&mut {struct_name}, &[Value], &mut ScriptApi<'_>)> =
        HashMap::with_capacity({funcs});
{dispatch_entries}    m
}});
"#,
        struct_name = struct_name,
        var_count = variables.len(),
        get_entries = get_entries,
        set_entries = set_entries,
        apply_entries = apply_entries,
        dispatch_entries = dispatch_entries,
        funcs = functions.len()
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

    let mut exposed = Vec::new();
    let mut variables = Vec::new();

    let expose_re = Regex::new(r"///\s*@expose[^\n]*\n\s*(?:pub\s+)?(\w+)\s*:\s*([^,]+),?").unwrap();
    for cap in expose_re.captures_iter(&struct_body) {
        let name = cap[1].to_string();
        let typ = cap[2].trim().to_string();
        exposed.push(Variable {
            name: name.clone(),
            typ: Some(Variable::parse_type(&typ)),
            value: None,
        });
        if cap[0].contains("pub") {
            variables.push(Variable {
                name,
                typ: Some(Variable::parse_type(&typ)),
                value: None,
            });
    }
    }

    let pub_re = Regex::new(r"pub\s+(\w+)\s*:\s*([^,\n}]+)").unwrap();
    for cap in pub_re.captures_iter(&struct_body) {
        let name = cap[1].to_string();
        if name == "node" || variables.iter().any(|v| v.name == name) {
            continue;
        }
        let typ = cap[2].trim().to_string();
        variables.push(Variable {
            name,
            typ: Some(Variable::parse_type(&typ)),
            value: None,
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
            if let Some((name, typ)) = param.split_once(':') {
                let name = name.trim().to_string();
                let typ_str = typ.trim();
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

    let boilerplate = implement_script_boilerplate(&actual_struct_name, &exposed, &variables, &functions);
    let combined = format!("{}\n\n{}", final_contents, boilerplate);

    write_to_crate(project_path, &combined, struct_name)
}