// scripting/lang/codegen/rust.rs
#![allow(unused)]#![allow(dead_code)]
use std::{fs, path::{Path, PathBuf}};
use std::fmt::Write as _;

use regex::Regex;

use crate::{asset_io::{get_project_root, ProjectRoot}, lang::ast::*, script::Var};

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

fn is_engine_api(name: &str) -> bool {
    matches!(name, "JSON" | "OS" | "Time")
}

impl Script {

    pub fn is_struct_field(&self, name: &str) -> bool {
        self.variables.iter().any(|v| v.name == name)
            || self.exports.iter().any(|v| v.name == name)
    }

    pub fn get_variable_type(&self, name: &str) -> Option<&Type> {
    self.variables
        .iter()
        .find(|v| v.name == name)
        .and_then(|v| v.typ.as_ref())
}


   fn infer_expr_type(&self, expr: &Expr) -> Option<Type> {
    match expr {
        Expr::Literal(lit) => match lit {
            Literal::Int(_) => Some(Type::Int),
            Literal::Float(_) => Some(Type::Float),
            Literal::Bool(_) => Some(Type::Bool),
            Literal::String(_) => Some(Type::String),
            Literal::Interpolated(_) => Some(Type::String),
        },

        Expr::Ident(name) => self.get_variable_type(name).cloned(),

        Expr::BinaryOp(left, _, right) => {
            let left_type = self.infer_expr_type(left)?;
            let right_type = self.infer_expr_type(right)?;
            if left_type == right_type {
                Some(left_type)
            } else if (left_type == Type::Float && right_type == Type::Int)
                || (left_type == Type::Int && right_type == Type::Float)
            {
                Some(Type::Float)
            } else {
                None
            }
        }

        Expr::MemberAccess(base, member) => {
            let base_type = self.infer_expr_type(base)?;
            self.get_member_type(&base_type, member)
        }

        // ✅ NEW: handle expression-based calls
        Expr::Call(target, _) => {
            match &**target {
                // Plain identifier: check if it's a known function in this script
                Expr::Ident(func_name) => self.get_function_return_type(func_name),

                // Method on `self`: infer type from method name if it exists
                Expr::MemberAccess(base, method) => {
                    let base_type = self.infer_expr_type(base)?;
                    // If it’s 'self' type, see if that method exists in the same script node
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

                // Calls on custom API objects like JSON / Time / OS
                Expr::Ident(name) => {
                if is_engine_api(name) {
                    Some(Type::Custom(name.clone()))
                } else {
                    self.get_variable_type(name).cloned()
                }
            }

                _ => None,
            }
        }

        Expr::SelfAccess => Some(Type::Custom(self.node_type.clone())),

        _ => None,
    }
}

    
    fn get_member_type(&self, base_type: &Type, member: &str) -> Option<Type> {
    match base_type {
        Type::Custom(type_name) if type_name == &self.node_type => {
            // Check exports first
            if let Some(export) = self.exports.iter().find(|v| v.name == member) {
                export.typ.clone()
            }
            // Then check variables
            else if let Some(var) = self.variables.iter().find(|v| v.name == member) {
                var.typ.clone()
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

pub fn to_rust(&self, struct_name: &str, project_path: &Path) -> String {
    let mut out = String::new();
    let pascal_struct_name = to_pascal_case(struct_name);

    // Headers
    out.push_str("#![allow(improper_ctypes_definitions)]\n");
    out.push_str("#![allow(unused)]\n\n");
    out.push_str("use std::any::Any;\n");
    out.push_str("use std::collections::HashMap;\n");
    out.push_str("use serde_json::{Value, json};\n");
    out.push_str("use uuid::Uuid;\n");
    out.push_str("use std::ops::{Deref, DerefMut};\n");
    out.push_str("use std::{rc::Rc, cell::RefCell};\n\n");
    out.push_str("use perro_core::prelude::*;\n\n");

    // Collect field data
    let export_fields: Vec<(&str, String, String)> = self.exports.iter()
        .map(|export| {
            let name = export.name.as_str();
            let rust_type = export.rust_type();
            let default_val = export.default_value();
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

    // ========================================================================
    // MAIN SCRIPT STRUCT
    // ========================================================================
    out.push_str("// ========================================================================\n");
    out.push_str(&format!("// {} - Main Script Structure\n", pascal_struct_name));
    out.push_str("// ========================================================================\n\n");
    
    out.push_str(&format!("pub struct {}Script {{\n", pascal_struct_name));
    out.push_str("    node_id: Uuid,\n");

    // Add export fields
    for (name, rust_type, _default_val) in &export_fields {
        out.push_str(&format!("    {}: {},\n", name, rust_type));
    }

    // Add variable fields
    for (name, rust_type, _default_val) in &variable_fields {
        out.push_str(&format!("    {}: {},\n", name, rust_type));
    }

    out.push_str("}\n\n");

    // ========================================================================
    // SCRIPT CREATOR FUNCTION
    // ========================================================================
    out.push_str("// ========================================================================\n");
    out.push_str(&format!("// {} - Creator Function (FFI Entry Point)\n", pascal_struct_name));
    out.push_str("// ========================================================================\n\n");
    
    out.push_str("#[unsafe(no_mangle)]\n");
    out.push_str(&format!(
        "pub extern \"C\" fn {}_create_script() -> *mut dyn ScriptObject {{\n",
        struct_name.to_lowercase()
    ));
    out.push_str(&format!("    Box::into_raw(Box::new({}Script {{\n", pascal_struct_name));
    out.push_str("        node_id: Uuid::nil(),\n");

    // Initialize exports
    for export in &self.exports {
        let init_code = export.rust_initialization(self);
        out.push_str(&format!("        {}: {},\n", export.name, init_code));
    }

    // Initialize variables
    for var in &self.variables {
        let init_code = var.rust_initialization(self);
        out.push_str(&format!("        {}: {},\n", var.name, init_code));
    }

    out.push_str("    })) as *mut dyn ScriptObject\n");
    out.push_str("}\n\n");

    // ========================================================================
    // SUPPORTING STRUCTURES
    // ========================================================================
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
    out.push_str(&format!("// {} - Script Init & Update Implementation\n", pascal_struct_name));
    out.push_str("// ========================================================================\n\n");

 // Script impl
     out.push_str(&format!("impl Script for {}Script {{\n", pascal_struct_name));

         // Trait methods (init, update)
        for func in &self.functions {
            if func.is_trait_method {
                out.push_str(&func.to_rust_trait_method(&self.node_type, &self));
            }
        }
         out.push_str("}\n\n");

           // Helper methods
        let helpers: Vec<_> = self.functions.iter().filter(|f| !f.is_trait_method).collect();
        if !helpers.is_empty() {

              out.push_str("// ========================================================================\n");
    out.push_str(&format!("// {} - Script-Defined Methods\n", pascal_struct_name));
    out.push_str("// ========================================================================\n\n");

            out.push_str(&format!("impl {}Script {{\n", pascal_struct_name));
            for func in helpers {
                out.push_str(&func.to_rust_method(&self.node_type, &self));
            }
            out.push_str("}\n\n");
        }


        
        out.push_str(&implement_script_boilerplate(
            &format!("{}Script", pascal_struct_name),
            &self.exports,
            &self.variables,
        ));


        if let Err(e) = write_to_crate(&project_path, &out, struct_name) {
            eprintln!("Warning: Failed to write to crate: {}", e);
        }

        out
    }

 

    pub fn function_uses_api(&self, name: &str) -> bool {
    if is_engine_api(name) { return true; }
    self.functions
        .iter()
        .find(|f| f.name == name)
        .map(|f| f.requires_api(self))
        .unwrap_or(false)
    }
}


fn implement_script_boilerplate(
    struct_name: &str,
    exports: &[Variable],
    variables: &[Variable],
) -> String {
    let mut get_matches = String::new();
    let mut set_matches = String::new();
    let mut apply_exports_matches = String::new();

    for var in variables {
        let name = &var.name;
        let rust_type = var.rust_type();

        get_matches.push_str(&format!(
            "            \"{name}\" => Some(&self.{name} as &dyn Any),\n"
        ));

        set_matches.push_str(&format!(
            "            \"{name}\" => {{\n                \
if let Ok(v) = val.downcast::<{rust_type}>() {{\n                    \
self.{name} = *v;\n                    \
return Some(());\n                \
}}\n                \
return None;\n            \
}},\n"
        ));

    }

   for var in exports {
    let name = &var.name;
    let rust_type = var.rust_type();

    let assignment = match rust_type.as_str() {
        "i32" | "f32" | "bool" => format!("self.{name} = *v;"),
        _ => format!("self.{name} = v.clone();"), // clone non-primitives
    };

    apply_exports_matches.push_str(&format!(
        "                \"{name}\" => {{
                    if let Some(value) = hashmap.get(\"{name}\") {{
                        if let Some(v) = value.downcast_ref::<{rust_type}>() {{
                            {assignment}
                        }}
                    }}
                }},\n"
    ));
}



    format!(
        r#"
impl ScriptObject for {struct_name} {{
    fn set_node_id(&mut self, id: Uuid) {{
        self.node_id = id;
    }}

    fn get_node_id(&self) -> Uuid {{
        self.node_id
    }}

    fn as_any(&self) -> &dyn Any {{
        self as &dyn Any
    }}

    fn as_any_mut(&mut self) -> &mut dyn Any {{
        self as &mut dyn Any
    }}

    fn get_var(&self, name: &str) -> Option<&dyn Any> {{
        match name {{
{get_matches}            _ => None,
        }}
    }}

    fn set_var(&mut self, name: &str, val: Box<dyn Any>) -> Option<()> {{
        match name {{
{set_matches}            _ => None,
        }}
    }}

    fn apply_exports(&mut self, hashmap: &HashMap<String, Box<dyn Any>>) {{
        for (key, _) in hashmap.iter() {{
            match key.as_str() {{
{apply_exports_matches}                _ => {{}},
            }}
        }}
    }}
}}
"#
    )
}

impl StructDef {
    pub fn to_rust_definition(&self, script: &Script) -> String {
        let mut out = String::new();

        // Always derive Default, Debug, Clone
        writeln!(out, "#[derive(Default, Debug, Clone)]").unwrap();
        writeln!(out, "pub struct {} {{", self.name).unwrap();

        // ✅ If this struct extends another, emit a base field
        if let Some(base) = &self.base {
            writeln!(out, "    pub base: {},", base).unwrap();
        }

        // Emit regular fields
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

        // --- Method impl block ---
        writeln!(out, "impl {} {{", self.name).unwrap();
        writeln!(out, "    pub fn new() -> Self {{ Self::default() }}").unwrap();
       for m in &self.methods {
    out.push_str(&m.to_rust_method(&self.name, script));
}
        writeln!(out, "}}\n").unwrap();

        // ✅ Implement Deref/DerefMut if base exists
        if let Some(base) = &self.base {
            writeln!(
                out,
                "impl Deref for {} {{\n    type Target = {};\n    fn deref(&self) -> &Self::Target {{ &self.base }}\n}}\n",
                self.name, base
            )
            .unwrap();

            writeln!(
                out,
                "impl DerefMut for {} {{\n    fn deref_mut(&mut self) -> &mut Self::Target {{ &mut self.base }}\n}}\n",
                self.name
            )
            .unwrap();
        }

        out
    }
}

impl Type {
    pub fn to_rust_type(&self) -> &str {
        match self {
            Type::Float => "f32",
            Type::Int => "i32",
            Type::Bool => "bool",
            Type::String => "String",
            Type::StrRef => "&str",
            Type::Script => "Option<ScriptType>",
            Type::Custom(name) => name.as_str(),
            Type::Void => "()",
        }
    }
}

impl Function {
    pub fn to_rust_method(&self, node_type: &str, script: &Script) -> String {
        let needs_api = self.requires_api(script);
        let mut out = String::new();

        // --- build parameter list ---
        let mut param_list = String::new();
        param_list.push_str("&mut self");

        // user-defined parameters
        if !self.params.is_empty() {
            let joined = self.params
                .iter()
                .map(|p| format!("{}: {}", p.name, p.typ.to_rust_type()))
                .collect::<Vec<_>>()
                .join(", ");
            write!(param_list, ", {}", joined).unwrap();
        }

        // append engine API only if needed
        if needs_api {
            write!(param_list, ", api: &mut ScriptApi<'_>").unwrap();
        }
            // final header line
        writeln!(out, "    fn {}({}) {{", self.name, param_list).unwrap();

        // Always shadow all parameters so they behave like mutable Pup locals.
        for param in &self.params {
            writeln!(out, "        let mut {0} = {0};", param.name).unwrap();
        }

        // --- prelude (delta, self_node etc.) ---
        let needs_delta = self.body.iter().any(|stmt| stmt.contains_delta());
        let needs_self = self.body.iter().any(|stmt| stmt.contains_self());

        if needs_api && needs_delta {
            out.push_str("        let delta = api.delta();\n");
        }

        if needs_api && needs_self {
            out.push_str(&format!(
                "        let mut self_node = api.get_node_clone::<{}>(&self.node_id);\n",
                node_type
            ));
        }

        // --- body ---
        for stmt in &self.body {
            out.push_str(&stmt.to_rust(needs_self, script));
        }

        // --- footers (merge node, etc.) ---
        if needs_api && needs_self {
            out.push_str("\n        api.merge_nodes(vec![self_node.to_scene_node()]);\n");
        }

        out.push_str("    }\n\n");
        out
    }

    /// Does this function or any child call require ScriptApi?
    fn requires_api(&self, script: &Script) -> bool {
        self.body.iter().any(|stmt| stmt.contains_api_call(script))
    }

    /// For trait / engine lifecycle methods (init, update, etc.)
    pub fn to_rust_trait_method(&self, node_type: &str, script: &Script) -> String {
        let mut out = format!("    fn {}(", self.name);
        out.push_str("&mut self, api: &mut ScriptApi<'_>) {\n");

        let needs_delta = self.body.iter().any(|stmt| stmt.contains_delta());
        let needs_self = self.body.iter().any(|stmt| stmt.contains_self());

        if needs_delta {
            out.push_str("        let delta = api.delta();\n");
        }

        let mut cloned_nodes = Vec::new();

        if needs_self {
            out.push_str(&format!(
                "        let mut self_node = api.get_node_clone::<{}>(&self.node_id);\n",
                node_type
            ));
            cloned_nodes.push("self_node".to_string());
        }

        // --- body ---
        for stmt in &self.body {
            out.push_str(&stmt.to_rust(needs_self, script));
        }

        // --- merge nodes ---
        if !cloned_nodes.is_empty() {
            let merge_args = cloned_nodes.iter()
                .map(|n| format!("{}.to_scene_node()", n))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("\n        api.merge_nodes(vec![{}]);\n", merge_args));
        }

        out.push_str("    }\n\n");
        out
    }
}

impl Stmt {
     fn to_rust(&self, needs_self: bool, script: &Script) -> String {
        match self {
            Stmt::Expr(expr) => {
                        let expr_str = expr.to_rust(needs_self, script, None);
                        if expr_str.trim().is_empty() {
                            "".to_string()
                        } else if expr_str.trim_end().ends_with(';') {
                            format!("        {}\n", expr_str)
                        } else {
                            format!("        {};\n", expr_str)
                        }
                    }
           Stmt::VariableDecl(var) => {
                    // var: Variable { name, typ: Option<Type>, value: Option<Expr> }
                    
                    let expected_type = var.typ.as_ref();
                    let expr_str = if let Some(expr) = &var.value {
                        expr.to_rust(needs_self, script, expected_type)
                    } else {
                        // If no explicit initializer, you might want a default value:
                        if let Some(t) = expected_type {
                            var.default_value()
                        } else {
                            // No type and no value - possibly error or just empty initialization
                            String::new()
                        }
                    };
                    
                    if expr_str.is_empty() {
                        format!("        let {};\n", var.name)
                    } else {
                        format!("        let {} = {};\n", var.name, expr_str)
                    }
                }

            Stmt::Assign(name, expr) => {
                        let target = if script.is_struct_field(name) {
                            format!("self.{}", name)
                        } else {
                            name.clone()
                        };
                        let expected_type = script.get_variable_type(name); // you implement this
                        let expr_str = expr.to_rust(needs_self, script, expected_type);
                        format!("        {} = {};\n", target, expr_str)
                    }
            Stmt::AssignOp(name, op, expr) => {
                        let target = if script.is_struct_field(name) {
                            format!("self.{}", name)
                        } else {
                            name.clone()
                        };
                        let expected_type = script.get_variable_type(name);
                        format!(
                            "        {} = {} {} {};\n",
                            target,
                            target,
                            op.to_rust(),
                            expr.to_rust(needs_self, script, expected_type)
                        )
                    }
            Stmt::MemberAssign(lhs_expr, rhs_expr) => {
                        let expected_type = script.infer_expr_type(lhs_expr); // implement this
                        format!(
                            "        {} = {};\n",
                            lhs_expr.to_rust(needs_self, script, None),
                            rhs_expr.to_rust(needs_self, script, expected_type.as_ref())
                        )
                    }
            Stmt::MemberAssignOp(lhs_expr, op, rhs_expr) => {
                        let expected_type = script.infer_expr_type(lhs_expr); // implement this
                        format!(
                            "        {} {}= {};\n",
                            lhs_expr.to_rust(needs_self, script, None),
                            op.to_rust_assign(),
                            rhs_expr.to_rust(needs_self, script, expected_type.as_ref())
                        )
                    }
            Stmt::Pass => "".to_string(),
            Stmt::ScriptAssign(var, field, rhs) => {
                // 1) generate the RHS expression as a Var constructor
                let rhs_str = rhs.to_rust(needs_self, script, None);
                // pick the Var:: variant based on the AST Literal or expected type:
                let ctor = match rhs {
                    Expr::Literal(Literal::Int(_))    => "I32",
                    Expr::Literal(Literal::Float(_))  => "F32",
                    Expr::Literal(Literal::Bool(_))   => "Bool",
                    Expr::Literal(Literal::String(_)) => "String",
                    _ => { 
                        // fallback, or query script.infer_expr_type(rhs) 
                        "I32" 
                    }
                };
                format!(
                    "        api.update_script_var(&{var}_id, \"{field}\", \
            UpdateOp::Set, Var::{ctor}({rhs}));\n",
                    var   = var,
                    field = field,
                    ctor  = ctor,
                    rhs   = rhs_str
                )
            }

            Stmt::ScriptAssignOp(var, field, op, rhs) => {
                let rhs_str = rhs.to_rust(needs_self, script, None);
                let op_str = match op {
                    Op::Add => "Add",
                    Op::Sub => "Sub",
                    Op::Mul => "Mul",
                    Op::Div => "Div",
                };
                let ctor = match rhs {
                    Expr::Literal(Literal::Int(_))    => "I32",
                    Expr::Literal(Literal::Float(_))  => "F32",
                    Expr::Literal(Literal::Bool(_))   => "Bool",
                    Expr::Literal(Literal::String(_)) => "String",
                    _ => "I32",
                };
                format!(
                    "        api.update_script_var(&{var}_id, \"{field}\", \
            UpdateOp::{op_str}, Var::{ctor}({rhs}));\n",
                    var    = var,
                    field  = field,
                    op_str = op_str,
                    ctor   = ctor,
                    rhs    = rhs_str
                )
            }
        }
    }



    fn contains_self(&self) -> bool {
        match self {
            Stmt::Expr(e) => e.contains_self(),
            Stmt::VariableDecl(var) => {
                if let Some(expr) = &var.value {
                    expr.contains_self()
                } else {
                    false
                }
            }

            Stmt::Assign(_, e) => e.contains_self(),
            Stmt::Pass => false,
            Stmt::AssignOp(_, _, e) => e.contains_self(),
            Stmt::MemberAssign(lhs_expr, rhs_expr) => {
                        lhs_expr.contains_self() || rhs_expr.contains_self()
                    }
            Stmt::MemberAssignOp(lhs_expr, _, rhs_expr) => {
                        lhs_expr.contains_self() || rhs_expr.contains_self()
                    }
Stmt::ScriptAssign(_, _, expr) => expr.contains_self(),
            Stmt::ScriptAssignOp(_, _, _, expr) => expr.contains_self(),
        }
    }

    fn contains_delta(&self) -> bool {
        match self {
            Stmt::Expr(e) => e.contains_delta(),
           Stmt::VariableDecl(var) => {
                if let Some(expr) = &var.value {
                    expr.contains_delta()
                } else {
                    false
                }
            }
            Stmt::Assign(_, e) => e.contains_delta(),
            Stmt::AssignOp(_, _, e) => e.contains_delta(),
            Stmt::MemberAssign(lhs, rhs) => lhs.contains_delta() || rhs.contains_delta(),
            Stmt::MemberAssignOp(lhs, _, rhs) => lhs.contains_delta() || rhs.contains_delta(),
            Stmt::Pass => false,
            Stmt::ScriptAssign(_, _, expr) => expr.contains_delta(),
            Stmt::ScriptAssignOp(_, _, _, expr) => expr.contains_delta(),

        }
    }

    pub fn contains_api_call(&self, script: &Script) -> bool {
    match self {
        Stmt::Expr(e)                   => e.contains_api_call(script),
        Stmt::VariableDecl(v)           => v.value.as_ref().map_or(false, |e| e.contains_api_call(script)),
        Stmt::Assign(_, e)              => e.contains_api_call(script),
        Stmt::AssignOp(_, _, e)         => e.contains_api_call(script),
        Stmt::MemberAssign(a, b)        => a.contains_api_call(script) || b.contains_api_call(script),
        Stmt::MemberAssignOp(a, _, b)   => a.contains_api_call(script) || b.contains_api_call(script),
        Stmt::ScriptAssign(_, _, e)     => e.contains_api_call(script),
        Stmt::ScriptAssignOp(_, _, _, e)=> e.contains_api_call(script),
        Stmt::Pass                      => false,
    }
}
}

impl Expr {
    pub fn to_rust(
        &self,
        needs_self: bool,
        script: &Script,
        expected_type: Option<&Type>,
    ) -> String {
        match self {
            Expr::Ident(name) => { name.clone() }
            Expr::Literal(lit) => lit.to_rust(expected_type),
            Expr::BinaryOp(left, op, right) => {
                        let left_type = script.infer_expr_type(left);
                        let right_type = script.infer_expr_type(right);

                        // Determine common type (same logic you already had)
                        let common_type = match (left_type.clone(), right_type.clone()) {
                            (Some(l), Some(r)) if l == r => Some(l),
                            (Some(Type::Float), Some(Type::Int))
                            | (Some(Type::Int), Some(Type::Float)) => Some(Type::Float),
                            (Some(l), _) => Some(l),
                            (_, Some(r)) => Some(r),
                            _ => expected_type.cloned(),
                        };

                        // Helper: cast expression string to target type if needed
                        fn cast_expr(
                            expr_str: String,
                            expr_type: Option<Type>,
                            target_type: Option<&Type>,
                        ) -> String {
                            match (expr_type, target_type) {
                                (Some(from), Some(to)) if from != *to => match to {
                                    Type::Float => format!("({} as f32)", expr_str),
                                    Type::Int => format!("({} as i32)", expr_str),
                                    _ => expr_str,
                                },
                                _ => expr_str,
                            }
                        }

                        // Generate left and right expr strings with casts if needed
                        let left_str = cast_expr(
                            left.to_rust(needs_self, script, common_type.as_ref()),
                            left_type,
                            common_type.as_ref(),
                        );

                        let right_str = cast_expr(
                            right.to_rust(needs_self, script, common_type.as_ref()),
                            right_type,
                            common_type.as_ref(),
                        );

                        format!("{} {} {}", left_str, op.to_rust(), right_str)
                    }
            Expr::MemberAccess(base, field) => {
                        format!("{}.{}", base.to_rust(needs_self, script, None), field)
                    }
            Expr::SelfAccess => {
                        if needs_self {
                            "self_node".to_string()
                        } else {
                            "self".to_string()
                        }
                    }
            Expr::BaseAccess => "self.base".to_string(),
           Expr::Call(target, args) => {
    let args_rust: Vec<String> = args
        .iter()
        .map(|a| a.to_rust(needs_self, script, None))
        .collect();
    let needs_api = Self::get_target_name(target)
        .map(|n| script.function_uses_api(n))
        .unwrap_or(false);
    let target_str = target.to_rust(needs_self, script, None);
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
                            .map(|(k, v)| format!("\"{}\": {}", k, v.to_rust(needs_self, script, None)))
                            .collect();
                        format!("&json!({{ {} }})", fields.join(", "))
                    }
            Expr::ApiCall(module, args) => module.to_rust(args, script, needs_self),
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

    fn contains_delta(&self) -> bool {
        match self {
            Expr::Ident(name) => name == "delta",
            Expr::BinaryOp(left, _, right) => left.contains_delta() || right.contains_delta(),
            Expr::MemberAccess(base, _) => base.contains_delta(),
            Expr::Call(target, args) => {
                target.contains_delta() || args.iter().any(|arg| arg.contains_delta())
            }
            Expr::Literal(_) | Expr::SelfAccess => false,
            _ => false
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

    // small helper so contains_api_call compiles
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
            Literal::Int(i) => match expected_type {
                Some(Type::Float) => format!("{}f32", *i as f32),
                Some(Type::Bool) => format!("{}", if *i != 0 { "true" } else { "false" }),
                _ => i.to_string(),
            },
            Literal::Float(f) => match expected_type {
                Some(Type::Int) => format!("{}", *f as i32),
                Some(Type::Bool) => format!("{}", if *f != 0.0 { "true" } else { "false" }),
                Some(Type::Float) => format!("{}f32", f),
                _ => format!("{}f32", f),
            },
            Literal::String(s) => match expected_type {
                Some(Type::String) | None => format!("\"{}\"", s),
                Some(Type::StrRef) => format!("\"{}\"", s), // &str
                Some(Type::Bool) => format!("{}", if !s.is_empty() { "true" } else { "false" }),
                _ => format!("\"{}\"", s),
            },
            Literal::Interpolated(s) => {
            use regex::Regex;
            let re = Regex::new(r"\{([A-Za-z_][A-Za-z0-9_]*)\}").unwrap();

            let mut fmt = String::new();
            let mut args = Vec::new();
            let mut last_end = 0;

            for cap in re.captures_iter(s) {
                let m = cap.get(0).unwrap();
                fmt.push_str(&s[last_end..m.start()]);
                fmt.push_str("{}");
                last_end = m.end();
                args.push(cap[1].to_string());
            }

            fmt.push_str(&s[last_end..]);

            if args.is_empty() {
                format!("\"{}\"", fmt)
            } else {
                format!("format!(\"{}\", {})", fmt, args.join(", "))
            }
        }
            Literal::Bool(b) => match expected_type {
                Some(Type::Bool) | None => b.to_string(),
                Some(Type::Int) => format!("{}", if *b { 1 } else { 0 }),
                Some(Type::Float) => format!("{}", if *b { 1.0 } else { 0.0 }),
                Some(Type::String) => format!("\"{}\"", b),
                _ => b.to_string(),
            },
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

pub fn write_to_crate(
    project_path: &Path,
    contents: &str,
    struct_name: &str,
) -> Result<(), String> {
    let base_path = project_path.join(".perro/scripts/src");
    let lower_name = struct_name.to_lowercase();
    let file_path = base_path.join(format!("{}.rs", lower_name));

    fs::create_dir_all(&base_path).map_err(|e| format!("Failed to create dir: {}", e))?;

    // Rename create function only for raw Rust scripts
   

    fs::write(&file_path, contents)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    // --- Update lib.rs dynamic content only ---
    let lib_rs_path = base_path.join("lib.rs");
    let mut current_content = fs::read_to_string(&lib_rs_path).unwrap_or_default();

    // Add module
    let mod_line = format!("pub mod {};", lower_name);
    if !current_content.contains(&mod_line) {
        current_content = current_content.replace(
            "// __PERRO_MODULES__",
            &format!("{}\n// __PERRO_MODULES__", mod_line),
        );
    }

    // Add import
    let import_line = format!("use {}::{}_create_script;", lower_name, lower_name);
    if !current_content.contains(&import_line) {
        current_content = current_content.replace(
            "// __PERRO_IMPORTS__",
            &format!("{}\n// __PERRO_IMPORTS__", import_line),
        );
    }

    // Add registry entry
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

    // Mark that we should recompile
    let should_compile_path = project_path.join(".perro/scripts/should_compile");
    fs::write(should_compile_path, "true")
        .map_err(|e| format!("Failed to write should_compile: {}", e))?;

    Ok(())
}



fn extract_create_script_fn_name(contents: &str) -> Option<String> {
    // Look for pattern: pub extern "C" fn SOMETHING_create_script()
    for line in contents.lines() {
        if line.contains("pub extern \"C\" fn") && line.contains("_create_script") {
            // Extract function name between "fn " and "("
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
    // Only generate boilerplate if @PerroScript is present
    let marker_re = Regex::new(r"///\s*@PerroScript").unwrap();
    let marker_pos = match marker_re.find(code) {
        Some(m) => m.end(),
        None => return write_to_crate(project_path, code, struct_name),
    };

    // Starting from the marker, find the NEXT struct definition only
    let struct_after_marker_re = Regex::new(r"struct\s+(\w+)\s*\{([^}]*)\}").unwrap();
    let captures = struct_after_marker_re
        .captures(&code[marker_pos..])
        .ok_or_else(|| "Could not find struct after @PerroScript".to_string())?;

    let actual_struct_name_from_struct = captures[1].to_string();
    let struct_body = captures[2].to_string();

    let mut exports = Vec::new();
    let mut variables = Vec::new();

    // Match @expose fields first (next line is always the field)
    let expose_re = Regex::new(r"///\s*@expose[^\n]*\n\s*(?:pub\s+)?(\w+)\s*:\s*([^,]+),").unwrap();
    for cap in expose_re.captures_iter(&struct_body) {
        let name = cap[1].to_string();
        let typ = cap[2].trim().to_string();
        exports.push(Variable {
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

    // Match remaining public fields (but not node_id, and not ones already added)
    let pub_re = Regex::new(r"pub\s+(\w+)\s*:\s*([^,]+),").unwrap();
    for cap in pub_re.captures_iter(&struct_body) {
        let name = cap[1].to_string();
        if name == "node_id" || variables.iter().any(|v| v.name == name) {
            continue;
        }
        let typ = cap[2].trim().to_string();
        variables.push(Variable {
            name,
            typ: Some(Variable::parse_type(&typ)),
            value: None,
        });
    }

    eprintln!("EXPORTS: {:?}", exports.iter().map(|v| &v.name).collect::<Vec<_>>());
    eprintln!("VARIABLES: {:?}", variables.iter().map(|v| &v.name).collect::<Vec<_>>());

    // --- Keep original logic for impl Script / create function renaming ---
    let lower_name = struct_name.to_lowercase();

    // Extract the actual struct name from the code using regex
    let impl_script_re = Regex::new(r"impl\s+Script\s+for\s+(\w+)\s*\{").unwrap();
    let actual_struct_name = if let Some(cap) = impl_script_re.captures(&code) {
        cap[1].to_string()
    } else {
        to_pascal_case(struct_name)
    };

    // Rename the create function to match expected convention
    let final_contents = if let Some(actual_fn_name) = extract_create_script_fn_name(&code) {
        let expected_fn_name = format!("{}_create_script", lower_name);
        code.replace(&actual_fn_name, &expected_fn_name)
    } else {
        code.to_string()
    };

    // Generate boilerplate using the fields we collected
    let boilerplate = implement_script_boilerplate(&actual_struct_name, &exports, &variables);
    let combined = format!("{}\n\n{}", final_contents, boilerplate);

    write_to_crate(project_path, &combined, struct_name)
}
