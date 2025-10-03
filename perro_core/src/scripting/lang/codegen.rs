// scripting/lang/codegen/rust.rs
#![allow(unused)]#![allow(dead_code)]
use std::{fs, path::{Path, PathBuf}};

use crate::{asset_io::{get_project_root, ProjectRoot}, lang::ast::*};


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
                Literal::Number(_) => Some(Type::Number),
                Literal::Bool(_) => Some(Type::Bool),
                Literal::String(_) => Some(Type::String),
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
            Expr::Call(func_name, _) => self.get_function_return_type(func_name),
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

    pub fn to_rust(&self, struct_name: &str) -> String {
        let mut out = String::new();

        // Headers
    out.push_str("#![allow(improper_ctypes_definitions)]\n\n");
    out.push_str("#![allow(unused)]\n\n");
    out.push_str("use std::any::Any;\n\n");
    out.push_str("use std::collections::HashMap;\n");
    out.push_str("use serde_json::Value;\n");
    out.push_str("use uuid::Uuid;\n");
    out.push_str("use perro_core::{script::{UpdateOp, Var}, scripting::api::ScriptApi, scripting::script::Script, ");
    out.push_str(&format!("{} }};\n\n", self.node_type));


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

    // Creator function
    out.push_str("#[unsafe(no_mangle)]\n");
    out.push_str(&format!(
        "pub extern \"C\" fn {}_create_script() -> *mut dyn Script {{\n",
        struct_name.to_lowercase()
    ));
    out.push_str(&format!("    Box::into_raw(Box::new({}_script {{\n", struct_name));
    out.push_str("        node_id: Uuid::nil(),\n");

    // Use collected data for creator
    for export in &self.exports {
        let init_code = export.rust_initialization(self);
        out.push_str(&format!("    {}: {},\n", export.name, init_code));
    }

    for var in &self.variables {
        let init_code = var.rust_initialization(self);
        out.push_str(&format!("    {}: {},\n", var.name, init_code));
    }


    out.push_str("    })) as *mut dyn Script\n");
    out.push_str("}\n\n");

    // Struct definition
    out.push_str(&format!("pub struct {}_script {{\n", struct_name));
    out.push_str("    node_id: Uuid,\n");

    // Use collected data for struct fields
    for (name, rust_type, _default_val) in &export_fields {
        out.push_str(&format!("    pub {}: {},\n", name, rust_type));
    }

    for (name, rust_type, _default_val) in &variable_fields {
    out.push_str(&format!("    pub {}: {},\n", name, rust_type));
    }

        out.push_str("}\n\n");

        // Script impl
        out.push_str(&format!("impl Script for {}_script {{\n", struct_name));

        // Trait methods (init, update)
        for func in &self.functions {
            if func.is_trait_method {
                out.push_str(&func.to_rust_trait_method(&self.node_type, &self));
            }
        }

        // Required methods
        out.push_str("    fn set_node_id(&mut self, id: Uuid) {\n");
        out.push_str("        self.node_id = id;\n");
        out.push_str("    }\n\n");

        out.push_str("    fn get_node_id(&self) -> Uuid {\n");
        out.push_str("        self.node_id\n");
        out.push_str("    }\n\n");

        // Within your `impl Script for {}` block:
        out.push_str("    fn as_any(&self) -> &dyn Any {\n");
        out.push_str("        self as &dyn Any\n");
        out.push_str("    }\n\n");
        out.push_str("    fn as_any_mut(&mut self) -> &mut dyn Any {\n");
        out.push_str("        self as &mut dyn Any\n");
        out.push_str("    }\n");

        // Add set_exports_from_map method

    out.push_str("    fn apply_exports(&mut self, hashmap: &std::collections::HashMap<String, serde_json::Value>) {\n");

    for export in &self.exports {
        let name = &export.name;
        let typ = export.rust_type();

        let assignment = match typ.as_str() {
            "String" => format!(
                "        self.{0} = hashmap.get(\"{0}\").and_then(|v| v.as_str()).unwrap_or(\"\").to_string();\n",
                name
            ),
            "f32" => format!(
                "        self.{0} = hashmap.get(\"{0}\").and_then(|v| v.as_f64()).map(|v| v as f32).unwrap_or(0.0);\n",
                name
            ),

            "i32" => format!(
                "        self.{0} = hashmap.get(\"{0}\").and_then(|v| v.as_i64()).map(|v| v as i32).unwrap_or(0);\n",
                name
            ),
            "bool" => format!(
                "        self.{0} = hashmap.get(\"{0}\").and_then(|v| v.as_bool()).unwrap_or(false);\n",
                name
            ),
            _ => format!("        // TODO: implement assignment for type {}\n", typ),
        };

        out.push_str(&assignment);
    }

    out.push_str("    }\n\n");
    
    

        // 1) get_var
out.push_str("    fn get_var(&self, name: &str) -> Option<Var> {\n");
out.push_str("        match name {\n");
for field in self.exports.iter().chain(self.variables.iter()) {
    let name = &field.name;
    let typ  = field.rust_type();
    let arm = match typ.as_str() {
        "String" => format!(
            "            \"{0}\" => Some(Var::String(self.{0}.clone())),\n",
            name
        ),
        "f32" => format!(
            "            \"{0}\" => Some(Var::F32(self.{0})),\n",
            name
        ),
        "i32" => format!(
            "            \"{0}\" => Some(Var::I32(self.{0})),\n",
            name
        ),
        "bool" => format!(
            "            \"{0}\" => Some(Var::Bool(self.{0})),\n",
            name
        ),
        other => format!(
            "            // TODO: get_var for unsupported type `{}`\n",
            other
        ),
    };
    out.push_str(&arm);
}
out.push_str("            _ => None,\n");
out.push_str("        }\n");
out.push_str("    }\n\n");

// 2) set_var
out.push_str(
    "    fn set_var(&mut self, name: &str, val: Var) -> Option<()> {\n",
);
out.push_str("        match (name, val) {\n");
for field in self.exports.iter().chain(self.variables.iter()) {
    let name = &field.name;
    let typ  = field.rust_type();
    let arm = match typ.as_str() {
        "String" => format!(
            "            (\"{0}\", Var::String(v)) => {{ self.{0} = v; Some(()) }},\n",
            name
        ),
        "f32" => format!(
            "            (\"{0}\", Var::F32(v)) => {{ self.{0} = v; Some(()) }},\n",
            name
        ),
        "i32" => format!(
            "            (\"{0}\", Var::I32(v)) => {{ self.{0} = v; Some(()) }},\n",
            name
        ),
        "bool" => format!(
            "            (\"{0}\", Var::Bool(v)) => {{ self.{0} = v; Some(()) }},\n",
            name
        ),
        other => format!(
            "            // TODO: set_var for unsupported type `{}`\n",
            other
        ),
    };
    out.push_str(&arm);
}
out.push_str("            _ => None,\n");
out.push_str("        }\n");
out.push_str("    }\n");

out.push_str("}\n");

        // Helper methods
        let helpers: Vec<_> = self.functions.iter().filter(|f| !f.is_trait_method).collect();
        if !helpers.is_empty() {
            out.push_str(&format!("impl {}Script {{\n", struct_name));
            for func in helpers {
                out.push_str(&func.to_rust_helper(&self.node_type, &self));
            }
            out.push_str("}\n");
        }

        if let Err(e) = write_to_crate( &out, struct_name) {
            eprintln!("Warning: Failed to write to crate: {}", e);
        }

        out
    }
}

impl Function {
    fn to_rust_trait_method(&self, node_type: &str, script: &Script) -> String {
        let mut out = format!("    fn {}(", self.name);
        // Add api parameter
        out.push_str("&mut self, api: &mut ScriptApi<'_>) {\n");

        let needs_delta = self.body.iter().any(|stmt| stmt.contains_delta());
        let needs_self = self.body.iter().any(|stmt| stmt.contains_self());

        if needs_delta {
            out.push_str("        let delta = api.get_delta();\n");
        }

        if needs_self {
            out.push_str(&format!(
                "        let self_node = api.get_node_mut::<{}>(&self.node_id).unwrap();\n",
                node_type
            ));
        }

        // Generate body
        for stmt in &self.body {
            out.push_str(&stmt.to_rust(needs_self, script));
        }

        out.push_str("    }\n\n");
        out
    }

    fn to_rust_helper(&self, node_type: &str, script: &Script) -> String {
        let mut out = format!("    fn {}(&mut self, api: &mut ScriptApi<'_>) {{\n", self.name);

        // Check if we need self_node
        let needs_delta = self.body.iter().any(|stmt| stmt.contains_delta());
        let needs_self = self.body.iter().any(|stmt| stmt.contains_self());

        if needs_delta {
            out.push_str("        let delta = api.get_delta();\n");
        }

        if needs_self {
            out.push_str(&format!(
                "        let self_node = api.get_node_mut::<{}>(&self.node_id).unwrap();\n",
                node_type
            ));
        }

        // Generate body
        for stmt in &self.body {
            out.push_str(&stmt.to_rust(needs_self, script));
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
            Stmt::Call(name, args) => {
                        let args_str: Vec<String> = args.iter().map(|arg| arg.to_rust(needs_self, script, None)).collect();
                        format!("        {}({});\n", name, args_str.join(", "))
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
            Stmt::Call(_, args) => args.iter().any(|e| e.contains_self()),
            Stmt::Pass => false,
            Stmt::AssignOp(_, _, e) => e.contains_self(),
            Stmt::MemberAssign(lhs_expr, rhs_expr) => {
                        lhs_expr.contains_self() || rhs_expr.contains_self()
                    }
            Stmt::MemberAssignOp(lhs_expr, _, rhs_expr) => {
                        lhs_expr.contains_self() || rhs_expr.contains_self()
                    }
Stmt::ScriptAssign(_, _, expr) => todo!(),
            Stmt::ScriptAssignOp(_, field, op, expr) => todo!(),
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
            Stmt::Call(_, args) => args.iter().any(|e| e.contains_delta()),
            Stmt::Pass => false,
            Stmt::ScriptAssign(_, _, expr) => todo!(),
            Stmt::ScriptAssignOp(_, field, op, expr) => todo!(),
        }
    }
}

impl Expr {
    pub fn to_rust(&self, needs_self: bool, script: &Script, expected_type: Option<&Type>) -> String {
    match self {
        Expr::Ident(name) => name.clone(),
        Expr::Literal(lit) => lit.to_rust(expected_type),
        Expr::BinaryOp(left, op, right) => {
            let left_type = script.infer_expr_type(left);
            let right_type = script.infer_expr_type(right);

            // Determine common type (use your existing logic)
            let common_type = match (left_type.clone(), right_type.clone()) {
                (Some(l), Some(r)) if l == r => Some(l),
                (Some(Type::Float), Some(Type::Int)) | (Some(Type::Int), Some(Type::Float)) => Some(Type::Float),
                (Some(l), _) => Some(l),
                (_, Some(r)) => Some(r),
                _ => expected_type.cloned(),
            };

            // Helper: cast expression string to the target type if needed
            fn cast_expr(expr_str: String, expr_type: Option<Type>, target_type: Option<&Type>) -> String {
                match (expr_type, target_type) {
                    (Some(from), Some(to)) if from != *to => {
                        // Cast float/double/int as needed
                        match to {
                            Type::Float => format!("({} as f32)", expr_str),
                            Type::Number => format!("({} as f32)", expr_str),
                            Type::Int => format!("({} as i32)", expr_str),
                            _ => expr_str,
                        }
                    }
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

        Expr::MemberAccess(base, field) => format!("{}.{}", base.to_rust(needs_self, script, None), field),
        Expr::ScriptAccess(base, field) => format!("{}.{}", base.to_rust(needs_self, script, None), field),
        Expr::SelfAccess => {
            if needs_self {
                "self_node".to_string()
            } else {
                "self".to_string()
            }
        }
        Expr::Call(name, args) => {
            let args_str: Vec<String> = args
                .iter()
                .map(|arg| arg.to_rust(needs_self, script, None))
                .collect();
            format!("{}({})", name, args_str.join(", "))
        }
    }
}

    fn contains_self(&self) -> bool {
        match self {
            Expr::SelfAccess => true,
            Expr::MemberAccess(base, _) => base.contains_self(),
            Expr::ScriptAccess(base, _) => base.contains_self(),
            Expr::BinaryOp(left, _, right) => left.contains_self() || right.contains_self(),
            Expr::Call(_, args) => args.iter().any(|arg| arg.contains_self()),
            _ => false,
        }
    }

    fn contains_delta(&self) -> bool {
        match self {
            Expr::Ident(name) => name == "delta",
            Expr::BinaryOp(left, _, right) => left.contains_delta() || right.contains_delta(),
            Expr::MemberAccess(base, _) => base.contains_delta(),
            Expr::ScriptAccess(base, _) => base.contains_delta(),
            Expr::Call(_, args) => args.iter().any(|arg| arg.contains_delta()),
            Expr::Literal(_) | Expr::SelfAccess => false,
        }
    }
}

impl Literal {
    fn to_rust(&self, expected_type: Option<&Type>) -> String {
        match self {
            Literal::Int(i) => match expected_type {
                Some(Type::Float) => format!("{}f32", *i as f32),
                Some(Type::Number) => format!("{}f32", *i as f32),
                Some(Type::Bool) => format!("{}", if *i != 0 { "true" } else { "false" }),
                _ => i.to_string(),
            },
            Literal::Float(f) => match expected_type {
                Some(Type::Int) => format!("{}", *f as i32),
                Some(Type::Number) => format!("{}f32", *f as f32),
                Some(Type::Bool) => format!("{}", if *f != 0.0 { "true" } else { "false" }),
                Some(Type::Float) => format!("{}f32", f),
                _ => format!("{}f32", f),
            },
            Literal::Number(f) => match expected_type {
                Some(Type::Int) => format!("{}", *f as i32),
                Some(Type::Float) => format!("{}f32", *f as f32),
                Some(Type::Bool) => format!("{}", if *f != 0.0 { "true" } else { "false" }),
                Some(Type::Number) | None => format!("{}f32", *f as f32),
                _ => format!("{}f32", *f as f32),
            },
            Literal::String(s) => match expected_type {
                Some(Type::String) | None => format!("\"{}\".to_string()", s),
                Some(Type::Bool) => format!("{}", if !s.is_empty() { "true" } else { "false" }),
                _ => format!("\"{}\"", s),
            },
            Literal::Bool(b) => match expected_type {
                Some(Type::Bool) | None => b.to_string(),
                Some(Type::Int) => format!("{}", if *b { 1 } else { 0 }),
                Some(Type::Float) => format!("{}", if *b { 1.0 } else { 0.0 }),
                Some(Type::Number) => format!("{}", if *b { 1.0 } else { 0.0 }),
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

pub fn write_to_crate(contents: &str, struct_name: &str) -> Result<(), String> {
    // ✅ Extract disk root
    let project_root = match get_project_root() {
        ProjectRoot::Disk { root, .. } => root,
        ProjectRoot::Brk { .. } => {
            return Err("write_to_crate is not supported to a .pak".into());
        }
    };

    let base_path = project_root.join(".perro/scripts/src");
    let lower_name = struct_name.to_lowercase(); // Create binding once
    let file_path = base_path.join(format!("{}.rs", lower_name));

    fs::create_dir_all(&base_path).map_err(|e| format!("Failed to create dir: {}", e))?;
    
    // ✅ If this is a raw Rust file (ends with _rs), rewrite the create_script function name
    let final_contents = if lower_name.ends_with("_rs") {
        // Extract base name without _rs suffix
        let base_name = lower_name.strip_suffix("_rs").unwrap();
        
        // Replace {base_name}_create_script with {base_name}_rs_create_script
        let old_fn = format!("{}_create_script", base_name);
        let new_fn = format!("{}_create_script", lower_name);
        
        contents.replace(&old_fn, &new_fn)
    } else {
        contents.to_string()
    };
    
    fs::write(&file_path, final_contents).map_err(|e| format!("Failed to write file: {}", e))?;

    let lib_rs_path = base_path.join("lib.rs");
    let mut current_content = fs::read_to_string(&lib_rs_path).unwrap_or_default();

    // Ensure header exists
    if !current_content.contains("get_script_registry") {
        current_content = String::from(
            "use perro_core::script::{CreateFn, Script};\n\
             use std::collections::HashMap;\n\n\
             // __PERRO_MODULES__\n\
             // __PERRO_IMPORTS__\n\n\
             pub fn get_script_registry() -> HashMap<String, CreateFn> {\n\
                 let mut map: HashMap<String, CreateFn> = HashMap::new();\n\
                 // __PERRO_REGISTRY__\n\
                 map\n\
             }\n",
        );
    }

    // Add module
    let mod_line = format!("pub mod {};", lower_name);
    if !current_content.contains(&mod_line) {
        current_content = current_content.replace(
            "// __PERRO_MODULES__",
            &format!("{}\n// __PERRO_MODULES__", mod_line),
        );
    }

    // Add import
    let import_line = format!(
        "use {}::{}_create_script;",
        lower_name,
        lower_name
    );
    if !current_content.contains(&import_line) {
        current_content = current_content.replace(
            "// __PERRO_IMPORTS__",
            &format!("{}\n// __PERRO_IMPORTS__", import_line),
        );
    }

    // Add registry entry
    let registry_line = format!(
        "    map.insert(\"{}\".to_string(), {}_create_script as CreateFn);\n",
        lower_name,
        lower_name
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
    let should_compile_path = project_root.join(".perro/scripts/should_compile");
    fs::write(should_compile_path, "true")
        .map_err(|e| format!("Failed to write should_compile: {}", e))?;

    Ok(())
}