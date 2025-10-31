// scripting/lang/codegen/rust.rs
#![allow(unused)]
#![allow(dead_code)]
use std::{fmt::format, fs, path::{Path, PathBuf}};
use std::fmt::Write as _;
use std::collections::HashMap;
use std::cell::RefCell;

use regex::Regex;

use crate::{asset_io::{get_project_root, ProjectRoot}, lang::ast::*, script::Var};

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
    fn generate_implicit_cast_for_expr(&self, expr: &str, from: &Type, to: &Type) -> String {
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
        // Check cache first
        let cache_key = expr_cache_key(expr);
        let cached = TYPE_CACHE.with(|cache| {
            cache.borrow().get(&cache_key).cloned()
        });
        
        if let Some(result) = cached {
            return result;
        }

        let result = match expr {
            Expr::Literal(lit) => self.infer_literal_type(lit, None),

            Expr::Ident(name) => {
                if let Some(func) = current_func {
                    if let Some(local) = func.locals.iter().find(|v| v.name == *name) {
                        if let Some(typ) = &local.typ {
                            Some(typ.clone())
                        } else if let Some(value) = &local.value {
                            self.infer_expr_type(&value.expr, current_func)
                        } else {
                            None
                        }
                    } else if let Some(param) = func.params.iter().find(|p| p.name == *name) {
                        Some(param.typ.clone())
                    } else {
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
                    (Some(l), None) => Some(l.clone()),
                    (None, Some(r)) => Some(r.clone()),
                    (Some(l), Some(r)) => self.promote_types(l, r),
                    _ => Some(Type::Number(NumberKind::Float(32))),
                }
            }

            Expr::MemberAccess(base, member) => {
                if matches!(**base, Expr::SelfAccess) {
                    if let Some(exposed) = self.exposed.iter().find(|v| v.name == member.as_str()) {
                        exposed.typ.clone()
                    } else if let Some(var) = self.variables.iter().find(|v| v.name == member.as_str()) {
                        var.typ.clone()
                    } else {
                        None
                    }
                } else {
                    let base_type = self.infer_expr_type(base, current_func)?;
                    self.get_member_type(&base_type, member)
                }
            }

            Expr::Call(target, _) => {
                match &**target {
                    Expr::Ident(func_name) => self.get_function_return_type(func_name),
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
                }
            }

            Expr::SelfAccess => Some(Type::Custom(self.node_type.clone())),
            
            Expr::Cast(_, target_type) => Some(target_type.clone()),

            Expr::ApiCall(api, _) => api.return_type(),
            
            _ => None,
        };

        // Cache the result
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

    pub fn function_uses_api(&self, name: &str) -> bool {
        self.functions
            .iter()
            .find(|f| f.name == name)
            .map(|f| f.requires_api(self))
            .unwrap_or(false)
    }

  pub fn to_rust(&self, struct_name: &str, project_path: &Path, current_func: Option<&Function>) -> String {
        // Clear cache at the start of codegen
        clear_type_cache();
        
        let mut out = String::with_capacity(8192); // Pre-allocate larger buffer
        let pascal_struct_name = to_pascal_case(struct_name);

        // Headers
        out.push_str("#![allow(improper_ctypes_definitions)]\n");
        out.push_str("#![allow(unused)]\n\n");
        out.push_str("use std::any::Any;\n");
        out.push_str("use std::collections::HashMap;\n");
        out.push_str("use serde_json::{Value, json};\n");
        out.push_str("use serde::{Serialize, Deserialize};\n");
        out.push_str("use uuid::Uuid;\n");
        out.push_str("use std::ops::{Deref, DerefMut};\n");
        out.push_str("use rust_decimal::{Decimal, prelude::*};\n");
        out.push_str("use num_bigint::BigInt;\n");
        out.push_str("use std::str::FromStr;\n");
        out.push_str("use std::{rc::Rc, cell::RefCell};\n\n");
        out.push_str("use perro_core::prelude::*;\n\n");

        let exposed_fields: Vec<(&str, String, String)> = self.exposed.iter()
            .map(|exposed| {
                let name = exposed.name.as_str();
                let rust_type = exposed.rust_type();
                let default_val = exposed.default_value();
                (name, rust_type, default_val)
            })
            .collect();

        let variable_fields: Vec<(&str, String, String)> = self.variables.iter()
            .map(|var| {
                let name = var.name.as_str();
                let rust_type = var.rust_type(); 
                let default_val = var.default_value();
                (name, rust_type, default_val)
            })
            .collect();

        out.push_str("// ========================================================================\n");
        write!(out, "// {} - Main Script Structure\n", pascal_struct_name).unwrap();
        out.push_str("// ========================================================================\n\n");
        
        write!(out, "pub struct {}Script {{\n", pascal_struct_name).unwrap();
        write!(out, "    node: {},\n", self.node_type).unwrap();

        for (name, rust_type, _) in &exposed_fields {
            write!(out, "    {}: {},\n", name, rust_type).unwrap();
        }

        for (name, rust_type, _) in &variable_fields {
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
            write!(out, "        node: {}::new(\"{}\", None),\n", self.node_type, pascal_struct_name).unwrap();
        } else {
            write!(out, "        node: {}::new(\"{}\"),\n", self.node_type, pascal_struct_name).unwrap();
        }

        for exposed in &self.exposed {
            let init_code = exposed.rust_initialization(self, current_func);
            write!(out, "        {}: {},\n", exposed.name, init_code).unwrap();
        }

        for var in &self.variables {
            let init_code = var.rust_initialization(self, current_func);
            write!(out, "        {}: {},\n", var.name, init_code).unwrap();
        }

        out.push_str("    })) as *mut dyn ScriptObject\n");
        out.push_str("}\n\n");

        if !self.structs.is_empty() {
            out.push_str("// ========================================================================\n");
            out.push_str("// Supporting Struct Definitions\n");
            out.push_str("// ========================================================================\n\n");
            
            for s in &self.structs {
                out.push_str(&s.to_rust_definition(self));
                out.push_str("\n\n");
            }
        }

        out.push_str("// ========================================================================\n");
        write!(out, "// {} - Script Init & Update Implementation\n", pascal_struct_name).unwrap();
        out.push_str("// ========================================================================\n\n");

        write!(out, "impl Script for {}Script {{\n", pascal_struct_name).unwrap();

        for func in &self.functions {
            if func.is_trait_method {
                out.push_str(&func.to_rust_trait_method(&self.node_type, &self));
            }
        }
        out.push_str("}\n\n");

        let helpers: Vec<_> = self.functions.iter().filter(|f| !f.is_trait_method).collect();
        if !helpers.is_empty() {
            out.push_str("// ========================================================================\n");
            write!(out, "// {} - Script-Defined Methods\n", pascal_struct_name).unwrap();
            out.push_str("// ========================================================================\n\n");

            write!(out, "impl {}Script {{\n", pascal_struct_name).unwrap();
            for func in helpers {
                out.push_str(&func.to_rust_method(&self.node_type, &self));
            }
            out.push_str("}\n\n");
        }

        out.push_str(&implement_script_boilerplate(
            &format!("{}Script", pascal_struct_name),
            &self.exposed,
            &self.variables,
            &self.functions
        ));

        if let Err(e) = write_to_crate(&project_path, &out, struct_name) {
            eprintln!("Warning: Failed to write to crate: {}", e);
        }

        out
    }
}


impl StructDef {
    pub fn to_rust_definition(&self, script: &Script) -> String {
        let mut out = String::with_capacity(1024);

        writeln!(out, "#[derive(Default, Debug, Clone, Serialize, Deserialize)]").unwrap();
        writeln!(out, "pub struct {} {{", self.name).unwrap();

        if let Some(base) = &self.base {
            writeln!(out, "    pub base: {},", base).unwrap();
        }

        for field in &self.fields {
            writeln!(out, "    pub {}: {},", field.name, field.typ.to_rust_type()).unwrap();
        }

        writeln!(out, "}}\n").unwrap();

        writeln!(out, "impl std::fmt::Display for {} {{", self.name).unwrap();
        writeln!(out, "    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{").unwrap();
        writeln!(out, "        write!(f, \"{{{{ \")?;").unwrap();
        for (i, field) in self.fields.iter().enumerate() {
            let sep = if i + 1 < self.fields.len() { ", " } else { " " };
            writeln!(out, "        write!(f, \"{name}: {{:?}}{sep}\", self.{name})?;", name = field.name, sep = sep).unwrap();
        }
        writeln!(out, "        write!(f, \"}}}}\")").unwrap();
        writeln!(out, "    }}").unwrap();
        writeln!(out, "}}\n").unwrap();

        writeln!(out, "impl {} {{", self.name).unwrap();
        writeln!(out, "    pub fn new() -> Self {{ Self::default() }}").unwrap();
        for m in &self.methods {
            out.push_str(&m.to_rust_method(&self.name, script));
        }
        writeln!(out, "}}\n").unwrap();

        if let Some(base) = &self.base {
            writeln!(
                out,
                "impl Deref for {} {{\n    type Target = {};\n    fn deref(&self) -> &Self::Target {{ &self.base }}\n}}\n",
                self.name, base
            ).unwrap();

            writeln!(
                out,
                "impl DerefMut for {} {{\n    fn deref_mut(&mut self) -> &mut Self::Target {{ &mut self.base }}\n}}\n",
                self.name
            ).unwrap();
        }

        out
    }
}

impl Function {
    pub fn to_rust_method(&self, node_type: &str, script: &Script) -> String {
        let needs_api = self.requires_api(script);
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

        if needs_api {
            param_list.push_str(", api: &mut ScriptApi<'_>");
        }

        writeln!(out, "    fn {}({}) {{", self.name, param_list).unwrap();


        // ---------------------------------------------------
        // (1) Insert additional preamble if the method uses self/api
        // ---------------------------------------------------
        let needs_self = self.body.iter().any(|stmt| stmt.contains_self());

        if needs_api && needs_self {
            writeln!(
                out,
                "        self.node = \
                 api.get_node_clone::<{}>(&self.node.id);",
                node_type
            )
            .unwrap();
        }

        // ---------------------------------------------------
        // (2) Emit body
        // ---------------------------------------------------
        for stmt in &self.body {
            out.push_str(&stmt.to_rust(needs_self, script, Some(self)));
        }

        if needs_api && needs_self {
            out.push_str("\n        api.merge_nodes(vec![self.node.clone().to_scene_node()]);\n");
        }

        out.push_str("    }\n\n");
        out
    }

    // ============================================================
    // helper for detecting API usage
    // ============================================================
    fn requires_api(&self, script: &Script) -> bool {
        self.body.iter().any(|stmt| stmt.contains_api_call(script))
    }

    // ============================================================
    // for trait-style API methods (unchanged, still fine)
    // ============================================================
    pub fn to_rust_trait_method(&self, node_type: &str, script: &Script) -> String {
        let mut out = String::with_capacity(512);
        writeln!(out, "    fn {}(&mut self, api: &mut ScriptApi<'_>) {{", self.name).unwrap();

        let needs_self = self.body.iter().any(|stmt| stmt.contains_self());

        if needs_self {
            writeln!(
                out,
                "        self.node = api.get_node_clone::<{}>(&self.node.id);",
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
                let target = if script.is_struct_field(name) {
                    format!("self.{}", name)
                } else {
                    name.clone()
                };

                let target_type = self.get_target_type(name, script, current_func);
                let expr_type = script.infer_expr_type(&expr.expr, current_func);

                let mut expr_str = expr.expr.to_rust(needs_self, script, target_type.as_ref(), current_func);

                let should_clone = matches!(expr.expr, Expr::Ident(_) | Expr::MemberAccess(..))
                    && matches!((&target_type, &expr_type), 
                        (Some(Type::String), Some(Type::String)) | (Some(Type::Custom(_)), Some(Type::Custom(_))));

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
                let target = if script.is_struct_field(name) {
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
                    && matches!((&lhs_type, &rhs_type), 
                        (Some(Type::String), Some(Type::String)) | (Some(Type::Custom(_)), Some(Type::Custom(_))));

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
        match self {
            Expr::Ident(name) => name.clone(),
            
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
                if let Expr::Ident(base_name) = &**base {
                    if script.structs.iter().any(|s| s.name == *base_name) {
                        return format!("{}::{}", base_name, field);
                    }
                }
                format!(
                    "{}.{}",
                    base.to_rust(needs_self, script, None, current_func),
                    field
                )
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

    // ==============================================================
    // Check if this function uses the Script API
    // ==============================================================
    let needs_api = func_name
        .as_ref()
        .map(|n| script.function_uses_api(n))
        .unwrap_or(false);

    // Convert the target expression (e.g., func or self.method)
    let mut target_str = target.to_rust(needs_self, script, None, current_func);

    // If this is a local user-defined function, prefix with `self.`
    if is_local_function {
        target_str = format!("self.{}", func_name.unwrap());
    }

    // ==============================================================
    // Finally, build the Rust call string
    // Handles API injection and empty arg lists
    // ==============================================================
    if needs_api {
        if args_rust.is_empty() {
            format!("{}(api);", target_str)
        } else {
            format!("{}({}, api);", target_str, args_rust.join(", "))
        }
    } else if args_rust.is_empty() {
        format!("{}();", target_str)
    } else {
        format!("{}({});", target_str, args_rust.join(", "))
    }
}
            
            Expr::ObjectLiteral(pairs) => {
                let fields: Vec<String> = pairs.iter()
                    .map(|(k, v)| format!("\"{}\": {}", k, v.to_rust(needs_self, script, None, current_func)))
                    .collect();
                format!("&json!({{ {} }})", fields.join(", "))
            }
            
            Expr::ApiCall(module, args) => {
                module.to_rust(args, script, needs_self, current_func)
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
                Self::get_target_name(target)
                    .map(|n| script.function_uses_api(n))
                    .unwrap_or(false)
                    || target.contains_api_call(script)
                    || args.iter().any(|a| a.contains_api_call(script))
            }
            Expr::ObjectLiteral(pairs) => pairs.iter().any(|(_, e)| e.contains_api_call(script)),
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

fn implement_script_boilerplate(
    struct_name: &str,
    exposed: &[Variable],
    variables: &[Variable],
    functions: &[Function], // Add this parameter
) -> String {
    let mut out = String::with_capacity(2048);
    let mut get_matches = String::with_capacity(512);
    let mut set_matches = String::with_capacity(512);
    let mut apply_exposed_matches = String::with_capacity(512);
    let mut call_function_matches = String::with_capacity(512);

    for var in variables {
        let name = &var.name;
        let (accessor, conv) = var.json_access();

        write!(get_matches, "            \"{name}\" => Some(json!(self.{name})),\n").unwrap();

        if accessor == "__CUSTOM__" {
            let type_name = &conv;
            write!(set_matches,
                "            \"{name}\" => {{
                if let Ok(v) = serde_json::from_value::<{type_name}>(val) {{
                    self.{name} = v;
                    return Some(());
                }}
                None
            }},\n"
            ).unwrap();
        } else {
            write!(set_matches,
                "            \"{name}\" => {{
                if let Some(v) = val.{accessor}() {{
                    self.{name} = v{conv};
                    return Some(());
                }}
                None
            }},\n"
            ).unwrap();
        }
    }

    for var in exposed {
        let name = &var.name;
        let (accessor, conv) = var.json_access();

        if accessor == "__CUSTOM__" {
            let type_name = &conv;
            write!(apply_exposed_matches,
                "                \"{name}\" => {{
                    if let Some(value) = hashmap.get(\"{name}\") {{
                        if let Ok(v) = serde_json::from_value::<{type_name}>(value.clone()) {{
                            self.{name} = v;
                        }}
                    }}
                }},\n"
            ).unwrap();
        } else {
            write!(apply_exposed_matches,
                "                \"{name}\" => {{
                    if let Some(value) = hashmap.get(\"{name}\") {{
                        if let Some(v) = value.{accessor}() {{
                            self.{name} = v{conv};
                        }}
                    }}
                }},\n"
            ).unwrap();
        }
    }

    // Generate function call matches for non-trait methods
   for func in functions {
        if !func.is_trait_method {
            let func_name = &func.name;
            
            if func.params.is_empty() {
                // No parameters
                write!(call_function_matches,
                    "            \"{func_name}\" => {{
                self.{func_name}(api);
            }},\n"
                ).unwrap();
            } else {
                // Has parameters - need to parse from Vec<Value>
                let mut param_parsing = String::new();
                
                for (i, param) in func.params.iter().enumerate() {
                    let param_name = &param.name;
                    let parse_code = match &param.typ {
                        Type::String => {
                            format!("let {param_name} = params.get({i})\n                    .and_then(|v| v.as_str())\n                    .map(|s| s.to_string())\n                    .unwrap_or_default();\n")
                        }
                        Type::Number(NumberKind::Signed(w)) => {
                            format!("let {param_name} = params.get({i})\n                    .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))\n                    .unwrap_or_default() as i{w};\n")
                        }
                        Type::Number(NumberKind::Unsigned(w)) => {
                            format!("let {param_name} = params.get({i})\n                    .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))\n                    .unwrap_or_default() as u{w};\n")
                        }
                        Type::Number(NumberKind::Float(32)) => {
                            format!("let {param_name} = params.get({i})\n                    .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))\n                    .unwrap_or_default() as f32;\n")
                        }
                        Type::Number(NumberKind::Float(64)) => {
                            format!("let {param_name} = params.get({i})\n                    .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))\n                    .unwrap_or_default();\n")
                        }
                        Type::Bool => {
                            format!("let {param_name} = params.get({i})\n                    .and_then(|v| v.as_bool())\n                    .unwrap_or_default();\n")
                        }
                        Type::Custom(type_name) if type_name == "Signal" => {
                            // Treat Signal as String
                            format!("let {param_name} = params.get({i})\n                    .and_then(|v| v.as_str())\n                    .map(|s| s.to_string())\n                    .unwrap_or_default();\n")
                        }
                        Type::Custom(type_name) => {
                            format!("let {param_name} = params.get({i})\n                    .and_then(|v| serde_json::from_value::<{type_name}>(v.clone()).ok())\n                    .unwrap_or_default();\n")
                        }
                        _ => {
                            format!("let {param_name} = Default::default(); // Unsupported type\n")
                        }
                    };
                    
                    param_parsing.push_str("                ");
                    param_parsing.push_str(&parse_code);
                }
                
                let param_names: Vec<_> = func.params.iter().map(|p| &p.name).collect();
               let param_list = func.params.iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
                
                write!(call_function_matches,
                    "            \"{func_name}\" => {{
{param_parsing}                self.{func_name}({param_list}, api);
            }},\n"
                ).unwrap();
            }
        }
    }

    write!(out,
        r#"
impl ScriptObject for {struct_name} {{
    fn set_node_id(&mut self, id: Uuid) {{
        self.node.id = id;
    }}

    fn get_node_id(&self) -> Uuid {{
        self.node.id
    }}

    fn get_var(&self, name: &str) -> Option<Value> {{
        match name {{
{get_matches}            _ => None,
        }}
    }}

    fn set_var(&mut self, name: &str, val: Value) -> Option<()> {{
        match name {{
{set_matches}            _ => None,
        }}
    }}

    fn apply_exposed(&mut self, hashmap: &HashMap<String, Value>) {{
        for (key, _) in hashmap.iter() {{
            match key.as_str() {{
{apply_exposed_matches}                _ => {{}}
            }}
        }}
    }}

    fn call_function(&mut self, name: &str, api: &mut ScriptApi<'_>, params: &Vec<Value>) {{
        match name {{
{call_function_matches}            _ => {{}}
        }}
    }}
}}
"#
    ).unwrap();

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

    let expose_re = Regex::new(r"///\s*@expose[^\n]*\n\s*(?:pub\s+)?(\w+)\s*:\s*([^,]+),").unwrap();
    for cap in expose_re.captures_iter(&struct_body) {
        let name = cap[1].to_string();
        let typ = cap[2].trim().to_string();
        exposed.push(Variable {
            name: name.clone(),
            typ: Some(Variable::parse_type(&typ)),
            value: None,
        });
        variables.push(Variable {
            name,
            typ: Some(Variable::parse_type(&typ)),
            value: None,
        });
    }

    let pub_re = Regex::new(r"pub\s+(\w+)\s*:\s*([^,]+),").unwrap();
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