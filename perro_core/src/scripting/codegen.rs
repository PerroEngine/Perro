// scripting/lang/codegen/rust.rs
#![allow(unused)]
#![allow(dead_code)]
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::{
    fmt::format,
    fs,
    path::{Path, PathBuf},
};

use regex::Regex;

use crate::api_modules::*;
use crate::ast::*;
use crate::scripting::ast::{BuiltInEnumVariant, ContainerKind, Expr, Literal, NumberKind, Type};
use crate::node_registry::NodeType;
use crate::structs::engine_structs::EngineStruct as EngineStructKind;
use crate::structs::engine_registry::ENGINE_REGISTRY;
use crate::{
    asset_io::{ProjectRoot, get_project_root},
    prelude::string_to_u64,
    script::Var,
};
use crate::scripting::source_map::SourceMapBuilder;

// ============================================================================
// Type Inference Cache - Dramatically speeds up repeated type lookups
// ============================================================================

thread_local! {
    static TYPE_CACHE: RefCell<HashMap<usize, Option<Type>>> = RefCell::new(HashMap::new());
    static SCRIPT_MEMBERS_CACHE: RefCell<Option<(usize, std::collections::HashSet<String>)>> = RefCell::new(None);
}

fn expr_cache_key(expr: &Expr) -> usize {
    expr as *const Expr as usize
}

fn clear_type_cache() {
    TYPE_CACHE.with(|cache| cache.borrow_mut().clear());
}

fn clear_script_members_cache() {
    SCRIPT_MEMBERS_CACHE.with(|cache| *cache.borrow_mut() = None);
}

// Helper function to check if a type name is a node type
pub fn is_node_type(type_name: &str) -> bool {
    // Check engine registry for node types
    ENGINE_REGISTRY.node_defs.keys().any(|node_type| {
        format!("{:?}", node_type) == type_name
    })
}

// Helper function to convert a type name string to NodeType
fn string_to_node_type(type_name: &str) -> Option<NodeType> {
    ENGINE_REGISTRY.node_defs.keys().find(|node_type| {
        format!("{:?}", node_type) == type_name
    }).cloned()
}

/// Check if a Type is a node type
pub fn type_is_node(typ: &Type) -> bool {
    matches!(typ, Type::Node(_) | Type::DynNode)
}


const TRANSPILED_IDENT: &str = "__t_";

/// Functions that should NOT be prefixed with __t_
const RESERVED_FUNCTIONS: &[&str] = &["init", "update", "fixed_update", "draw"];

/// Rename a function: add __t_ prefix except for reserved functions
pub fn rename_function(func_name: &str) -> String {
    // Check if it's a reserved function
    if RESERVED_FUNCTIONS.contains(&func_name) {
        return func_name.to_string();
    }
    
    // Check if already renamed (to prevent double prefixing)
    if func_name.starts_with(TRANSPILED_IDENT) {
        return func_name.to_string();
    }
    
    // Add transpiled identifier prefix
    format!("{}{}", TRANSPILED_IDENT, func_name)
}

/// Rename a struct: add __t_ prefix
pub fn rename_struct(struct_name: &str) -> String {
    // Check if already renamed (to prevent double prefixing)
    if struct_name.starts_with(TRANSPILED_IDENT) {
        return struct_name.to_string();
    }
    
    // Add transpiled identifier prefix
    format!("{}{}", TRANSPILED_IDENT, struct_name)
}

/// Check if a type becomes Uuid or Option<Uuid> (i.e., represents an ID)
fn type_becomes_id(typ: &Type) -> bool {
    match typ {
        Type::Node(_) | 
        Type::DynNode | 
        Type::EngineStruct(EngineStructKind::Texture) |
        Type::Uuid => true,
        Type::Option(boxed) => matches!(boxed.as_ref(), Type::Uuid),
        _ => false,
    }
}

/// Rename a variable: if it's a type that becomes Uuid or Option<Uuid>, add _id suffix; otherwise add prefix
pub fn rename_variable(var_name: &str, typ: Option<&Type>) -> String {

    // Special case: "self" should NEVER be renamed - it's always self.id
    if var_name == "self" {
        return "self.id".to_string();
    }
    
    // Check if already renamed (to prevent double prefixing)
    if var_name.starts_with(TRANSPILED_IDENT) {
        return var_name.to_string();
    }
    
    // If it's a type that becomes Uuid or Option<Uuid> (node types, Texture, Uuid, Option<Uuid>), add _id suffix
    if let Some(typ) = typ {
        if type_becomes_id(typ) {
            // Check if already has _id suffix
            if var_name.ends_with("_id") {
                return var_name.to_string();
            }
            return format!("{}_id", var_name);
        }
    }
    
    // Otherwise, transpiled identifier prefix
    format!("{}{}",TRANSPILED_IDENT, var_name)
}

/// Get the node type from a Type, if it's a Node type
pub fn get_node_type(typ: &Type) -> Option<&NodeType> {
    match typ {
        Type::Node(nt) => Some(nt),
        _ => None,
    }
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
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
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
        use NumberKind::*;
        if from == to {
            return expr.to_string();
        }
        
        // Special case: if expr is "self.id" or already ends with ".id", and target is Uuid, no cast needed
        if expr == "self.id" || (expr.ends_with(".id") && matches!(to, Type::Uuid)) {
            return expr.to_string();
        }
        
        // Special case: if expr is "self" and target type is Uuid, just return "self.id"
        // This prevents self from being treated as a variable and stored as t_id_self
        if expr == "self" && matches!(to, Type::Uuid) {
            return "self.id".to_string();
        }
        
        // Direct handling for common conversions to avoid type inference issues with temp variables
        match (from, to) {
            // T -> Option<T> conversions (wrapping in Some)
            (from_ty, Type::Option(inner)) if from_ty == inner.as_ref() => {
                return format!("Some({})", expr);
            }
            // Integer to float conversions (explicit cast needed in Rust)
            (Number(Signed(_) | Unsigned(_)), Number(Float(32))) => {
                return format!("({} as f32)", expr);
            }
            (Number(Signed(_) | Unsigned(_)), Number(Float(64))) => {
                return format!("({} as f64)", expr);
            }
            _ => {}
        }
        
        // Special case: if expr is "self", use SelfAccess instead of Ident
        // This ensures self is always treated as self.id, never stored as a variable
        let inner_expr = if expr == "self" {
            Box::new(Expr::SelfAccess)
        } else {
            Box::new(Expr::Ident(expr.to_string()))
        };
        
        // Create a temporary Cast expression and use its to_rust method
        // to leverage the comprehensive casting logic in Expr::Cast.
        let temp_expr = Expr::Cast(inner_expr, to.clone());
        temp_expr.to_rust(false, self, Some(to), None, None) // Assume no self/func context for these implicit casts
    }

    pub fn is_struct_field(&self, name: &str) -> bool {
        self.variables.iter().any(|v| v.name == name)
    }

    pub fn get_variable_type(&self, name: &str) -> Option<&Type> {
        self.variables
            .iter()
            .find(|v| v.name == name)
            .and_then(|v| v.typ.as_ref())
    }
    
    /// Check if an identifier is a loop variable by searching for for loops that use it
    fn is_loop_variable(&self, name: &str, body: &[crate::scripting::ast::Stmt]) -> bool {
        use crate::scripting::ast::Stmt;
        for stmt in body {
            match stmt {
                Stmt::For { var_name: loop_var, .. } if loop_var == name => {
                    return true;
                }
                Stmt::For { body: loop_body, .. } | Stmt::ForTraditional { body: loop_body, .. } => {
                    if self.is_loop_variable(name, loop_body) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    pub fn infer_expr_type(&self, expr: &Expr, current_func: Option<&Function>) -> Option<Type> {
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
                    // 3. Check if it's a loop variable (not in locals/params but used in for loop)
                    // Loop variables from ranges are typically i32
                    else if self.is_loop_variable(name, &func.body) {
                        Some(Type::Number(NumberKind::Signed(32)))
                    }
                    // 4. Script-level variable or exposed field
                    else {
                        self.get_variable_type(name).cloned()
                    }
                } else {
                    self.get_variable_type(name).cloned()
                }
            }
            Expr::Range(start, end) => {
                // Ranges are iterable, so they return a type that can be used in for loops
                // For now, we'll infer it as a range type that generates Rust's Range
                // The actual iteration type will be inferred from context
                Some(Type::Container(
                    ContainerKind::Array,
                    vec![Type::Number(NumberKind::Signed(32))],
                ))
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
                    if let Some(Type::Container(ContainerKind::Map, ref params)) =
                        self.infer_expr_type(&args[0], current_func)
                    {
                        return params.get(1).cloned(); // value type
                    }
                    Some(Type::Object)
                }
                ApiModule::ArrayOp(ArrayApi::Pop) => {
                    if let Some(Type::Container(ContainerKind::Array, ref params)) =
                        self.infer_expr_type(&args[0], current_func)
                    {
                        return params.get(0).cloned(); // element type
                    }
                    Some(Type::Object)
                }
                // ... other API cases ...
                _ => api.return_type(),
            },
            Expr::StructNew(ty_name, _fields) => {
                // Check if it's a node type - if so, return Type::Node(...)
                // This allows rename_variable to correctly add _id suffix
                if let Some(node_type) = string_to_node_type(ty_name) {
                    Some(Type::Node(node_type))
                } else if let Some(engine_struct) = EngineStructKind::from_string(ty_name) {
                    // Engine struct constructor returns the engine struct type
                    Some(Type::EngineStruct(engine_struct))
                } else {
                    // The result of `new Struct(...)` is `Type::Custom("Struct")`
                    Some(Custom(ty_name.clone()))
                }
            }
            Expr::SelfAccess => {
                // Convert node_type string to NodeType enum
                if let Some(node_type) = string_to_node_type(&self.node_type) {
                    Some(Type::Node(node_type))
                } else {
                    // Fallback to Custom for non-standard node types
                    Some(Custom(self.node_type.clone()))
                }
            }
            Expr::ObjectLiteral(_) => Some(Type::Object),
            Expr::ContainerLiteral(kind, _) => match kind {
                ContainerKind::Array => {
                    Some(Type::Container(ContainerKind::Array, vec![Type::Object]))
                }
                ContainerKind::Map => Some(Type::Container(
                    ContainerKind::Map,
                    vec![Type::String, Type::Object],
                )),
                ContainerKind::FixedArray(_) => {
                    Some(Type::Container(kind.clone(), vec![Type::Object]))
                }
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
                            ContainerKind::FixedArray(_) => {
                                // Fixed size does not affect element type
                                inner_types.first().cloned() // Fixed array, elements are the inner type
                            }
                        }
                    }
                    // Case 2: Base is a dynamic Object (serde_json::Value)
                    Type::Object => Some(Type::Object),

                    // Case 3: Any other type (e.g., custom struct that might deref, but no direct indexing support at this AST level)
                    _ => None, // Or use self.infer_map_value_type(base, current_func) if you want to be very lenient
                }
            } // <-- This comma is important.

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
            Literal::String(_) | Literal::Interpolated(_) => {
                // If expected type is CowStr or StrRef, infer as that
                // Otherwise default to String
                match expected_type {
                    Some(Type::CowStr) => Some(Type::CowStr),
                    Some(Type::StrRef) => Some(Type::StrRef),
                    _ => Some(Type::String),
                }
            }
        }
    }

    fn promote_types(&self, left: &Type, right: &Type) -> Option<Type> {
        // Fast path for identical types
        if left == right {
            return Some(left.clone());
        }

        match (left, right) {
            // Uuid vs Node: promote to Uuid (nodes are just UUIDs)
            (Type::Uuid, Type::Node(_)) | (Type::Node(_), Type::Uuid) => {
                Some(Type::Uuid)
            }
            (Type::Uuid, Type::Custom(tn)) | (Type::Custom(tn), Type::Uuid) if is_node_type(tn) => {
                Some(Type::Uuid)
            }

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
            // --- For node types ---
            Type::Node(node_type) => {
                // Resolve script field name to Rust field name (e.g., "texture" -> "texture_id")
                let rust_field = ENGINE_REGISTRY.resolve_field_name(node_type, member);
                // Look up the field type in the ENGINE_REGISTRY using the resolved field name
                ENGINE_REGISTRY.get_field_type_node(node_type, &rust_field)
            }

            // --- For Uuid (dynamic node) ---
            // When we have a Uuid, it could be any node type, so we need to check
            // if the field exists on any node type and return a common type if all match
            Type::Uuid => {
                // Find all node types that have this field
                // Note: find_nodes_with_field uses get_field_type_node which handles field name mapping
                let nodes_with_field = ENGINE_REGISTRY.find_nodes_with_field(member);
                if nodes_with_field.is_empty() {
                    return None;
                }
                
                // Get the field type from the first node type (they should all be the same)
                // get_field_type_node handles field name mapping (e.g., "texture" -> "texture_id")
                if let Some(first_node) = nodes_with_field.first() {
                    ENGINE_REGISTRY.get_field_type_node(first_node, member)
                } else {
                    None
                }
            }

            // --- For engine structs ---
            Type::EngineStruct(engine_struct) => {
                // Look up the field type in the ENGINE_REGISTRY
                ENGINE_REGISTRY.get_field_type_struct(engine_struct, member)
            }

            // --- For custom structs ---
            Type::Custom(type_name) => {
                // Special handling for ParentType
                if type_name == "ParentType" {
                    match member {
                        "id" => return Some(Type::Uuid),
                        "node_type" => return Some(Type::Custom("NodeType".into())),
                        _ => return None,
                    }
                }
                
                if type_name == &self.node_type {
                    // script-level node fields (like `self.energy` if exposed)
                    if let Some(var) = self.variables.iter().find(|v| v.name == member) {
                        return var.typ.clone();
                    }
                }

                // Now: recursive base traversal for any struct
                get_struct_field_type_recursive(&self.structs, type_name, member)
            }

            // Container/Primitive types don't support `.member`
            _ => None,
        }
    }

    fn get_function_return_type(&self, func_name: &str) -> Option<Type> {
        self.functions
            .iter()
            .find(|f| f.name == func_name)
            .map(|f| f.return_type.clone())
    }

    pub fn to_rust(
        &mut self,
        struct_name: &str,
        project_path: &Path,
        current_func: Option<&Function>,
        verbose: bool,
    ) -> String {
        self.verbose = verbose;
        // Clear caches at the start of codegen
        clear_type_cache();
        clear_script_members_cache();

        let mut script = self.clone();
        // 🔹 Analyze self usage and call propagation before codegen
        analyze_self_usage(&mut script);

        let mut out = String::with_capacity(8192); // Pre-allocate larger buffer
        let pascal_struct_name = to_pascal_case(struct_name);

        // Headers
        // Headers / Lints
        out.push_str("#![allow(improper_ctypes_definitions)]\n");
        out.push_str("#![allow(unused)]\n\n");

        // Standard library imports
        out.push_str("use std::{\n");
        out.push_str("    any::Any,\n");
        out.push_str("    borrow::Cow,\n");
        out.push_str("    cell::RefCell,\n");
        out.push_str("    collections::HashMap,\n");
        out.push_str("    ops::{Deref, DerefMut},\n");
        out.push_str("    rc::Rc,\n");
        out.push_str("    str::FromStr,\n");
        out.push_str("};\n\n");

        // External crates
        out.push_str("use num_bigint::BigInt;\n");
        out.push_str("use phf::{phf_map, Map};\n");
        out.push_str("use rust_decimal::Decimal;\n");
        out.push_str("use rust_decimal::prelude::{FromPrimitive, ToPrimitive};\n");
        out.push_str("use serde::{Deserialize, Serialize};\n");
        out.push_str("use serde_json::{json, Value};\n");
        out.push_str("use smallvec::{smallvec, SmallVec};\n");
        out.push_str("use uuid::Uuid;\n\n");

        // Internal modules
        out.push_str("use perro_core::prelude::*;\n\n");

        out.push_str("//=======================================;\n");
        out.push_str("// Auto Generated by Perro Transpiler [Any further edits to this file will be overwritten on next transile];\n");
        out.push_str("//=======================================;\n\n");

        // Generate constant for source location tracking
        // ScriptSource is available from perro_core::prelude::*
        let script_file = script.source_file.as_ref()
            .map(|f| {
                // Extract just the filename from the path (e.g., "res://player.pup" -> "player.pup")
                // Keep the extension, just get the actual filename instead of using identifier
                f.split('/').last().unwrap_or(f).to_string()
            })
            .unwrap_or_else(|| {
                // Fallback: derive from struct name but remove the _pup suffix, keep .pup extension
                let base_name = struct_name.strip_suffix("_pup").unwrap_or(struct_name);
                format!("{}.pup", base_name)
            });
        
        write!(out, "const __PERRO_SOURCE_FILE: &str = \"{}\";\n\n", script_file);

        // The `script.script_vars` is now the single, authoritative, and ordered list
        // of all script-level variables as they appeared in the Pup source.
        let all_script_vars = &script.variables;

        // ========================================================================
        // {} - Main Script Structure
        // ========================================================================

        out.push_str(
            "// ========================================================================\n",
        );
        write!(out, "// {} - Main Script Structure\n", pascal_struct_name).unwrap();
        out.push_str(
            "// ========================================================================\n\n",
        );

        // Generate MEMBER_TO_ATTRIBUTES_MAP and ATTRIBUTE_TO_MEMBERS_MAP once at the top
        // Also build reverse index for O(1) attribute lookups

        // Build reverse index: attribute -> members
        let mut attribute_to_members: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        out.push_str("static MEMBER_TO_ATTRIBUTES_MAP: Map<&'static str, &'static [&'static str]> = phf_map! {\n");
        for var in &script.variables {
            let attrs = script
                .attributes
                .get(&var.name)
                .cloned()
                .unwrap_or_else(|| var.attributes.clone());
            // Only store members that have attributes
            if !attrs.is_empty() {
                write!(out, "    \"{}\" => &[", var.name).unwrap();
                for (i, attr) in attrs.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    write!(out, "\"{}\"", attr).unwrap();
                    attribute_to_members
                        .entry(attr.clone())
                        .or_insert_with(Vec::new)
                        .push(var.name.clone());
                }
                out.push_str("],\n");
            }
        }
        for func in &script.functions {
            // Suffix function names with "()" to differentiate from variables
            let func_key = format!("{}()", func.name);
            let attrs = script
                .attributes
                .get(&func.name)
                .cloned()
                .unwrap_or_else(|| func.attributes.clone());
            // Only store members that have attributes
            if !attrs.is_empty() {
                write!(out, "    \"{}\" => &[", func_key).unwrap();
                for (i, attr) in attrs.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    write!(out, "\"{}\"", attr).unwrap();
                    attribute_to_members
                        .entry(attr.clone())
                        .or_insert_with(Vec::new)
                        .push(func_key.clone());
                }
                out.push_str("],\n");
            }
        }
        for struct_def in &script.structs {
            for field in &struct_def.fields {
                let qualified_name = format!("{}.{}", struct_def.name, field.name);
                let attrs = script
                    .attributes
                    .get(&qualified_name)
                    .cloned()
                    .unwrap_or_else(|| field.attributes.clone());
                // Only store members that have attributes
                if !attrs.is_empty() {
                    write!(out, "    \"{}\" => &[", qualified_name).unwrap();
                    for (i, attr) in attrs.iter().enumerate() {
                        if i > 0 {
                            out.push_str(", ");
                        }
                        write!(out, "\"{}\"", attr).unwrap();
                        attribute_to_members
                            .entry(attr.clone())
                            .or_insert_with(Vec::new)
                            .push(qualified_name.clone());
                    }
                    out.push_str("],\n");
                }
            }
        }
        out.push_str("};\n\n");

        // Generate reverse index for O(1) attribute lookups
        out.push_str("static ATTRIBUTE_TO_MEMBERS_MAP: Map<&'static str, &'static [&'static str]> = phf_map! {\n");
        for (attr, members) in &attribute_to_members {
            write!(out, "    \"{}\" => &[", attr).unwrap();
            for (i, member) in members.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write!(out, "\"{}\"", member).unwrap();
            }
            out.push_str("],\n");
        }
        out.push_str("};\n\n");

        write!(out, "pub struct {}Script {{\n", pascal_struct_name).unwrap();
        // Scripts now use id: Uuid instead of base: NodeType
        write!(out, "    id: Uuid,\n").unwrap();

        // Use `all_script_vars` for defining struct fields to ensure the correct order
        for var in all_script_vars {
            let renamed_name = rename_variable(&var.name, var.typ.as_ref());
            write!(out, "    {}: {},\n", renamed_name, var.rust_type()).unwrap();
        }

        out.push_str("}\n\n");

        out.push_str(
            "// ========================================================================\n",
        );
        write!(
            out,
            "// {} - Creator Function (FFI Entry Point)\n",
            pascal_struct_name
        )
        .unwrap();
        out.push_str(
            "// ========================================================================\n\n",
        );

        // Emit FFI header
        out.push_str("#[unsafe(no_mangle)]\n");
        write!(
            out,
            "pub extern \"C\" fn {}_create_script() -> *mut dyn ScriptObject {{\n",
            struct_name.to_lowercase()
        )
        .unwrap();

        // Initialize id - will be set when script is attached to a node
        write!(
            out,
            "    let id = Uuid::nil(); // Will be set when attached to node\n"
        )
        .unwrap();

        // MEMBER_TO_ATTRIBUTES_MAP is generated at the top, not here in the create function

        // -----------------------------------------------------
        // 1. Emit local variable predefinitions for all fields
        //    (Crucially, iterate in dependency order using `all_script_vars`)
        // -----------------------------------------------------
        for var in all_script_vars {
            // Direct use of `all_script_vars`
            let name = &var.name;
            let mut init_code = var.rust_initialization(&script, current_func);

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
                    || ["let", "mut", "new", "HashMap", "vec", "json"].contains(&ref_name.as_str())
                // Rust keywords/macros
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
        write!(out, "        id,\n").unwrap();
        // Use `all_script_vars` here again for consistent ordering
        for var in all_script_vars {
            let renamed_name = rename_variable(&var.name, var.typ.as_ref());
            let name = &var.name;
            write!(out, "        {}: {},\n", renamed_name, name).unwrap();
        }

        out.push_str("    })) as *mut dyn ScriptObject\n");
        out.push_str("}\n\n");

        if !script.structs.is_empty() {
            out.push_str(
                "// ========================================================================\n",
            );
            out.push_str("// Supporting Struct Definitions\n");
            out.push_str(
                "// ========================================================================\n\n",
            );

            for s in &script.structs {
                out.push_str(&s.to_rust_definition(&script));
                out.push_str("\n\n");
            }
        }

        out.push_str(
            "// ========================================================================\n",
        );
        write!(
            out,
            "// {} - Script Init & Update Implementation\n",
            pascal_struct_name
        )
        .unwrap();
        out.push_str(
            "// ========================================================================\n\n",
        );

        write!(out, "impl Script for {}Script {{\n", pascal_struct_name).unwrap();

        for func in &script.functions {
            if func.is_trait_method {
                out.push_str(&func.to_rust_trait_method(&script.node_type, &script));
            }
        }
        out.push_str("}\n\n");

        let helpers: Vec<_> = script
            .functions
            .iter()
            .filter(|f| !f.is_trait_method)
            .collect();
        if !helpers.is_empty() {
            out.push_str(
                "// ========================================================================\n",
            );
            write!(out, "// {} - Script-Defined Methods\n", pascal_struct_name).unwrap();
            out.push_str(
                "// ========================================================================\n\n",
            );

            write!(out, "impl {}Script {{\n", pascal_struct_name).unwrap();
            for func in helpers {
                out.push_str(&func.to_rust_method(&script.node_type, &script));
            }
            out.push_str("}\n\n");
        }

        out.push_str(&implement_script_boilerplate(
            &format!("{}Script", pascal_struct_name),
            &script.variables, // Pass the unified list for exposed vars
            &script.functions,
            &script.attributes,
        ));

        if let Err(e) = write_to_crate(&project_path, &out, struct_name) {
            eprintln!("Warning: Failed to write to crate: {}", e);
        }

        out
    }
}

/// Extract node information from a member access expression
/// Returns (node_id_expr, node_type_name, field_path, closure_var_name) if it's a node member access
/// field_path is the full path like "transform.position.x"
/// Helper function to extract mutable API calls to temporary variables
/// Returns (temp_var_decl, temp_var_name) if extraction is needed, otherwise (String::new(), node_id)
fn extract_mutable_api_call(node_id: &str) -> (String, String) {
    // If node_id is already a temp variable (starts with __), don't extract it again
    if node_id.starts_with("__") && !node_id.contains("(") {
        return (String::new(), node_id.to_string());
    }
    
    // Check if node_id is an API call that requires mutable borrow (like api.get_parent)
    if node_id.starts_with("api.get_parent(") {
        // Generate a unique temporary variable name
        // Extract a hint from the argument if possible (e.g., "api.get_parent(collision_id)" -> "parent_collision_id")
        let temp_var = if let Some(start) = node_id.find('(') {
            let end = node_id.rfind(')').unwrap_or(node_id.len());
            let args = &node_id[start + 1..end];
            // Use a simple name based on the function
            format!("__parent_id")
        } else {
            "__parent_id".to_string()
        };
        
        let decl = format!("let {}: Uuid = {};", temp_var, node_id);
        (decl, temp_var)
    } else {
        (String::new(), node_id.to_string())
    }
}

fn extract_node_member_info(
    expr: &Expr,
    script: &Script,
    current_func: Option<&Function>,
) -> Option<(String, String, String, String)> {
    fn extract_recursive(
        expr: &Expr,
        script: &Script,
        current_func: Option<&Function>,
        field_path: &mut Vec<String>,
    ) -> Option<(String, String, String)> {
        match expr {
            Expr::MemberAccess(base, field) => {
                field_path.push(field.clone());
                extract_recursive(base, script, current_func, field_path)
            }
            Expr::SelfAccess => {
                // self.transform.position.x
                let path: Vec<String> = field_path.iter().rev().cloned().collect();
                Some(("self.id".to_string(), script.node_type.clone(), path.join(".")))
            }
            Expr::Ident(var_name) => {
                // Helper to find variable in nested blocks (for loops, if statements, etc.)
                fn find_variable_in_body<'a>(name: &str, body: &'a [crate::scripting::ast::Stmt]) -> Option<&'a crate::scripting::ast::Variable> {
                    use crate::scripting::ast::Stmt;
                    for stmt in body {
                        match stmt {
                            Stmt::VariableDecl(var) if var.name == name => {
                                return Some(var);
                            }
                            Stmt::If { then_body, else_body, .. } => {
                                if let Some(v) = find_variable_in_body(name, then_body) {
                                    return Some(v);
                                }
                                if let Some(else_body) = else_body {
                                    if let Some(v) = find_variable_in_body(name, else_body) {
                                        return Some(v);
                                    }
                                }
                            }
                            Stmt::For { body: for_body, .. } | Stmt::ForTraditional { body: for_body, .. } => {
                                if let Some(v) = find_variable_in_body(name, for_body) {
                                    return Some(v);
                                }
                            }
                            _ => {}
                        }
                    }
                    None
                }
                
                // Check if it's a node variable
                // First, check if the variable name ends with _id (renamed node variable)
                // If so, look up the original variable name
                let lookup_name = if var_name.ends_with("_id") {
                    &var_name[..var_name.len() - 3]
                } else {
                    var_name
                };
                
                let (var_type_ref, inferred_type_owned) = if let Some(func) = current_func {
                    // Strategy 1: Check in function locals first
                    let var_type_ref = func.locals.iter()
                        .find(|v| v.name == *lookup_name)
                        .and_then(|v| v.typ.as_ref())
                        .or_else(|| {
                            func.params.iter()
                                .find(|p| p.name == *lookup_name)
                                .map(|p| &p.typ)
                        });
                    
                    // Strategy 2: Check in nested blocks (for loops, if statements, etc.)
                    let var_type_ref = var_type_ref.or_else(|| {
                        find_variable_in_body(lookup_name, &func.body)
                            .and_then(|v| v.typ.as_ref())
                    });
                    
                    // Always try to infer from value expression, even if we have a type
                    // This handles cases where var b = new Sprite2D() creates a node but type might not be set
                    let inferred = func.locals.iter()
                        .find(|v| v.name == *lookup_name)
                        .and_then(|v| v.value.as_ref())
                        .and_then(|val| {
                            // First try infer_expr_type which should return Type::Node for StructNew with node types
                            let inferred = script.infer_expr_type(&val.expr, current_func);
                            if inferred.as_ref().map_or(false, |t| type_is_node(t)) {
                                return inferred;
                            }
                            // Fallback: directly check if it's a StructNew that creates a node
                            if let Expr::StructNew(ty_name, _) = &val.expr {
                                if let Some(node_type) = string_to_node_type(ty_name) {
                                    return Some(Type::Node(node_type));
                                }
                            }
                            inferred
                        })
                        .or_else(|| {
                            // Check in nested blocks for the variable value
                            find_variable_in_body(lookup_name, &func.body)
                                .and_then(|v| v.value.as_ref())
                                .and_then(|val| {
                                    let inferred = script.infer_expr_type(&val.expr, current_func);
                                    if inferred.as_ref().map_or(false, |t| type_is_node(t)) {
                                        return inferred;
                                    }
                                    // Fallback: directly check if it's a StructNew that creates a node
                                    if let Expr::StructNew(ty_name, _) = &val.expr {
                                        if let Some(node_type) = string_to_node_type(ty_name) {
                                            return Some(Type::Node(node_type));
                                        }
                                    }
                                    inferred
                                })
                        })
                        .or_else(|| {
                            // Also check if the value expression is a StructNew that creates a node
                            // (duplicate check for robustness)
                            func.locals.iter()
                                .find(|v| v.name == *lookup_name)
                                .and_then(|v| v.value.as_ref())
                                .and_then(|val| {
                                    if let Expr::StructNew(ty_name, _) = &val.expr {
                                        if let Some(node_type) = string_to_node_type(ty_name) {
                                            Some(Type::Node(node_type))
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                })
                        })
                        .or_else(|| {
                            // Check in nested blocks (duplicate check for robustness)
                            find_variable_in_body(lookup_name, &func.body)
                                .and_then(|v| v.value.as_ref())
                                .and_then(|val| {
                                    if let Expr::StructNew(ty_name, _) = &val.expr {
                                        if let Some(node_type) = string_to_node_type(ty_name) {
                                            Some(Type::Node(node_type))
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                })
                        });
                    
                    (var_type_ref, inferred)
                } else {
                    (script.get_variable_type(lookup_name), None)
                };
                
                // Check if it's a node type
                let var_type = var_type_ref.or_else(|| inferred_type_owned.as_ref());
                if let Some(typ) = var_type {
                    if type_is_node(typ) || matches!(typ, Type::DynNode) {
                        // Use the original variable name for renaming (not the _id version)
                        let renamed = rename_variable(lookup_name, Some(typ));
                        let node_type_name = match typ {
                            Type::Node(nt) => format!("{:?}", nt),
                            Type::DynNode => "__DYN_NODE__".to_string(), // Special marker for DynNode
                            _ => return None,
                        };
                        let path: Vec<String> = field_path.iter().rev().cloned().collect();
                        let closure_var = format!("t_id_{}", lookup_name);
                        Some((renamed, node_type_name, path.join(".")))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Expr::Cast(inner, target_type) => {
                // Handle casts to node types - extract from the inner expression
                match target_type {
                    Type::Node(node_type_enum) => {
                        // Cast to a specific node type - extract node_id from inner expression
                        // The inner might be GetParent which returns None, so we need to handle it specially
                        let node_id = match inner.as_ref() {
                            Expr::ApiCall(ApiModule::NodeSugar(NodeSugarApi::GetParent), args) => {
                                // Extract the node ID argument
                                let arg_expr = if let Some(Expr::SelfAccess) = args.get(0) {
                                    "self.id".to_string()
                                } else if let Some(Expr::Ident(name)) = args.get(0) {
                                    let is_node_var = if let Some(func) = current_func {
                                        func.locals.iter()
                                            .find(|v| v.name == *name)
                                            .and_then(|v| v.typ.as_ref())
                                            .map(|t| type_becomes_id(t))
                                            .or_else(|| {
                                                func.params.iter()
                                                    .find(|p| p.name == *name)
                                                    .map(|p| type_becomes_id(&p.typ))
                                            })
                                            .unwrap_or(false)
                                    } else {
                                        script.get_variable_type(name)
                                            .map(|t| type_becomes_id(&t))
                                            .unwrap_or(false)
                                    };
                                    
                                    if is_node_var {
                                        format!("{}_id", name)
                                    } else {
                                        name.clone()
                                    }
                                } else {
                                    "self.id".to_string()
                                };
                                format!("api.get_parent({})", arg_expr)
                            }
                            _ => {
                                // Try to extract from inner recursively
                                if let Some((node_id, _, _)) = extract_recursive(inner, script, current_func, field_path) {
                                    node_id
                                } else {
                                    return None;
                                }
                            }
                        };
                        let node_type_name = format!("{:?}", node_type_enum);
                        let path: Vec<String> = field_path.iter().rev().cloned().collect();
                        Some((node_id, node_type_name, path.join(".")))
                    }
                    Type::Custom(type_name) if is_node_type(&type_name) => {
                        // Cast to a node type by name - extract node_id from inner expression
                        let node_id = match inner.as_ref() {
                            Expr::ApiCall(ApiModule::NodeSugar(NodeSugarApi::GetParent), args) => {
                                // Extract the node ID argument
                                let arg_expr = if let Some(Expr::SelfAccess) = args.get(0) {
                                    "self.id".to_string()
                                } else if let Some(Expr::Ident(name)) = args.get(0) {
                                    let is_node_var = if let Some(func) = current_func {
                                        func.locals.iter()
                                            .find(|v| v.name == *name)
                                            .and_then(|v| v.typ.as_ref())
                                            .map(|t| type_becomes_id(t))
                                            .or_else(|| {
                                                func.params.iter()
                                                    .find(|p| p.name == *name)
                                                    .map(|p| type_becomes_id(&p.typ))
                                            })
                                            .unwrap_or(false)
                                    } else {
                                        script.get_variable_type(name)
                                            .map(|t| type_becomes_id(&t))
                                            .unwrap_or(false)
                                    };
                                    
                                    if is_node_var {
                                        format!("{}_id", name)
                                    } else {
                                        name.clone()
                                    }
                                } else {
                                    "self.id".to_string()
                                };
                                format!("api.get_parent({})", arg_expr)
                            }
                            _ => {
                                // Try to extract from inner recursively
                                if let Some((node_id, _, _)) = extract_recursive(inner, script, current_func, field_path) {
                                    node_id
                                } else {
                                    return None;
                                }
                            }
                        };
                        let path: Vec<String> = field_path.iter().rev().cloned().collect();
                        Some((node_id, type_name.clone(), path.join(".")))
                    }
                    _ => {
                        // Not a node type cast - continue extracting from inner
                        extract_recursive(inner, script, current_func, field_path)
                    }
                }
            }
            Expr::ApiCall(ApiModule::NodeSugar(NodeSugarApi::GetParent), args) => {
                // api.get_parent(node_id) returns Uuid - treat as node ID
                // Generate the full api.get_parent(...) expression as the node_id_expr
                // Extract the node ID argument similar to api_bindings.rs
                let arg_expr = if let Some(Expr::SelfAccess) = args.get(0) {
                    "self.id".to_string()
                } else if let Some(Expr::Ident(name)) = args.get(0) {
                    // Check if it's a type that becomes Uuid/Option<Uuid> (should have _id suffix)
                    let is_node_var = if let Some(func) = current_func {
                        func.locals.iter()
                            .find(|v| v.name == *name)
                            .and_then(|v| v.typ.as_ref())
                            .map(|t| type_becomes_id(t))
                            .or_else(|| {
                                func.params.iter()
                                    .find(|p| p.name == *name)
                                    .map(|p| type_becomes_id(&p.typ))
                            })
                            .unwrap_or(false)
                    } else {
                        script.get_variable_type(name)
                            .map(|t| type_becomes_id(&t))
                            .unwrap_or(false)
                    };
                    
                    if is_node_var {
                        // Node variables are stored as {name}_id
                        format!("{}_id", name)
                    } else {
                        // Uuid variable (like collision_id parameter) - use as-is
                        name.clone()
                    }
                } else {
                    // For complex expressions, fallback to self.id
                    // In practice, get_parent() is usually called with simple identifiers
                    "self.id".to_string()
                };
                
                // Generate the full api.get_parent(...) expression
                let node_id_expr = format!("api.get_parent({})", arg_expr);
                
                // Cannot determine node type from get_parent() alone - return None to fail transpilation
                // The type must be specified via casting (e.g., get_parent(x) as Sprite2D) or variable type annotation
                None
            }
            _ => None,
        }
    }
    
    let mut field_path = Vec::new();
    if let Some((node_id, node_type, path)) = extract_recursive(expr, script, current_func, &mut field_path) {
        // Check if the first field is a script member (for self access)
        if let Expr::MemberAccess(base, field) = expr {
            if matches!(base.as_ref(), Expr::SelfAccess) {
                let is_script_member = script.variables.iter().any(|v| v.name == *field)
                    || script.functions.iter().any(|f| f.name == *field);
                if is_script_member {
                    return None;
                }
            }
        }
        
        let closure_var = if node_id == "self.id" {
            "self_node".to_string()
        } else if node_id.starts_with("api.get_parent(") {
            // For api.get_parent(...), use "parent_node" as the closure variable name
            "parent_node".to_string()
        } else {
            // For node variables, node_id is like "bob_id", closure var should be "bob"
            // Extract original variable name by removing "_id" suffix
            let var_name = node_id.strip_suffix("_id").unwrap_or(&node_id);
            var_name.to_string()
        };
        
        Some((node_id, node_type, path, closure_var))
    } else {
        None
    }
}

fn expr_accesses_node(expr: &Expr, script: &Script) -> bool {
    match expr {
        Expr::SelfAccess => true, // `this` alone means accessing the node
        Expr::MemberAccess(base, field) => {
            // Check if this is `this.node` or `this.node.something`
            if matches!(base.as_ref(), Expr::SelfAccess) {
                // If field is "id", then we're accessing self.id
                if field == "id" {
                    return true;
                }
                // If field is a script member, it's NOT accessing the node
                let is_script_member = script.variables.iter().any(|v| v.name == *field)
                    || script.functions.iter().any(|f| f.name == *field);
                if is_script_member {
                    return false;
                }
            }
            // Recursively check the base
            expr_accesses_node(base, script)
        }
        Expr::BinaryOp(left, _, right) => {
            expr_accesses_node(left, script) || expr_accesses_node(right, script)
        }
        Expr::Call(target, args) => {
            expr_accesses_node(target, script)
                || args.iter().any(|arg| expr_accesses_node(arg, script))
        }
        _ => false,
    }
}

fn stmt_accesses_node(stmt: &Stmt, script: &Script) -> bool {
    match stmt {
        Stmt::Expr(e) => expr_accesses_node(&e.expr, script),
        Stmt::VariableDecl(var) => var
            .value
            .as_ref()
            .map_or(false, |e| expr_accesses_node(&e.expr, script)),
        Stmt::Assign(_, e) | Stmt::AssignOp(_, _, e) => expr_accesses_node(&e.expr, script),
        Stmt::MemberAssign(lhs, rhs) | Stmt::MemberAssignOp(lhs, _, rhs) => {
            expr_accesses_node(&lhs.expr, script) || expr_accesses_node(&rhs.expr, script)
        }
        Stmt::ScriptAssign(_, _, expr) | Stmt::ScriptAssignOp(_, _, _, expr) => {
            expr_accesses_node(&expr.expr, script)
        }
        Stmt::IndexAssign(array, index, value) | Stmt::IndexAssignOp(array, index, _, value) => {
            expr_accesses_node(array, script)
                || expr_accesses_node(index, script)
                || expr_accesses_node(&value.expr, script)
        }
        Stmt::Pass => false,
        Stmt::If {
            condition,
            then_body,
            else_body,
        } => {
            expr_accesses_node(&condition.expr, script)
                || then_body.iter().any(|s| stmt_accesses_node(s, script))
                || else_body.as_ref().map_or(false, |body| {
                    body.iter().any(|s| stmt_accesses_node(s, script))
                })
        }
        Stmt::For { iterable, body, .. } => {
            expr_accesses_node(&iterable.expr, script)
                || body.iter().any(|s| stmt_accesses_node(s, script))
        }
        Stmt::ForTraditional {
            init,
            condition,
            increment,
            body,
        } => {
            (init
                .as_ref()
                .map_or(false, |s| stmt_accesses_node(s.as_ref(), script)))
                || (condition
                    .as_ref()
                    .map_or(false, |c| expr_accesses_node(&c.expr, script)))
                || (increment
                    .as_ref()
                    .map_or(false, |s| stmt_accesses_node(s.as_ref(), script)))
                || body.iter().any(|s| stmt_accesses_node(s, script))
        }
    }
}

fn analyze_self_usage(script: &mut Script) {
    // Step 1: mark direct `self.node` usage (not just any self. access)
    // First pass: collect which functions need uses_self set (without mutating)
    let mut uses_self_flags: Vec<bool> = script
        .functions
        .iter()
        .map(|func| {
            func.body
                .iter()
                .any(|stmt| stmt_accesses_node(stmt, script))
        })
        .collect();

    // Step 1.5: Collect cloned child nodes for each function (before mutating)
    // collect_cloned_node_vars takes &Script (immutable), so we can call it here
    // Note: This collects ALL nodes, including loop-scoped ones. Loop-scoped nodes
    // are handled separately in the loop codegen, so they won't be merged at function level.
    let cloned_nodes_per_func: Vec<Vec<String>> = script
        .functions
        .iter()
        .map(|func| {
            let mut cloned_nodes = Vec::new();
            let mut cloned_ui_elements = Vec::new();
            collect_cloned_node_vars(
                &func.body,
                &mut cloned_nodes,
                &mut cloned_ui_elements,
                script,
            );
            cloned_nodes
        })
        .collect();

    // Second pass: apply the flags and cloned child nodes
    for (func, (uses_self, cloned_nodes)) in script
        .functions
        .iter_mut()
        .zip(uses_self_flags.iter().zip(cloned_nodes_per_func.iter()))
    {
        func.uses_self = *uses_self;
        func.cloned_child_nodes = cloned_nodes.clone();
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

/// Collect variable names that hold cloned child nodes (from self.get_node("name") as Type)
/// and cloned UI elements (from ui_node.get_element("name") as UIText)
/// These need to be merged back at function end
fn collect_cloned_node_vars(
    stmts: &[Stmt],
    cloned_nodes: &mut Vec<String>,
    cloned_ui_elements: &mut Vec<(String, String, String)>, // (ui_node_var, element_name, element_var)
    script: &Script,
) {
    // Check if an expression results in a cloned node that needs to be merged
    // This includes: get_node(), get_parent() as NodeType, and any cast from UuidOption to node type
    fn expr_contains_get_node(expr: &Expr, _verbose: bool) -> bool {
        match expr {
            Expr::ApiCall(ApiModule::NodeSugar(NodeSugarApi::GetChildByName), _) => true,
            Expr::ApiCall(ApiModule::NodeSugar(NodeSugarApi::GetParent), _) => {
                // get_parent() returns Uuid, which when cast to a node type just returns the UUID
                // Property access will use read_node/mutate_node under the hood
                true
            },
            Expr::Cast(inner, target_type) => {
                // Check if casting to a node type (this just returns the UUID, property access uses read_node/mutate_node)
                let is_node_type_cast = match target_type {
                    Type::Custom(tn) => is_node_type(tn),
                    Type::Node(_) => true,
                    _ => false,
                };
                if is_node_type_cast {
                    // This cast just returns the UUID, property access will use read_node/mutate_node
                    true
                } else {
                    // Continue checking inner expression
                    expr_contains_get_node(inner, _verbose)
                }
            },
            Expr::Call(target, args) => {
                expr_contains_get_node(target, _verbose)
                    || args.iter().any(|arg| expr_contains_get_node(arg, _verbose))
            }
            Expr::MemberAccess(base, _) => expr_contains_get_node(base, _verbose),
            Expr::BinaryOp(l, _, r) => {
                expr_contains_get_node(l, _verbose) || expr_contains_get_node(r, _verbose)
            }
            Expr::Index(base, idx) => {
                expr_contains_get_node(base, _verbose) || expr_contains_get_node(idx, _verbose)
            }
            Expr::Range(start, end) => {
                expr_contains_get_node(start, _verbose) || expr_contains_get_node(end, _verbose)
            }
            Expr::ObjectLiteral(pairs) => pairs
                .iter()
                .any(|(_, expr)| expr_contains_get_node(expr, _verbose)),
            Expr::ContainerLiteral(_, data) => match data {
                crate::ast::ContainerLiteralData::Array(elems) => {
                    elems.iter().any(|e| expr_contains_get_node(e, _verbose))
                }
                crate::ast::ContainerLiteralData::Map(pairs) => pairs.iter().any(|(k, v)| {
                    expr_contains_get_node(k, _verbose) || expr_contains_get_node(v, _verbose)
                }),
                crate::ast::ContainerLiteralData::FixedArray(_, elems) => {
                    elems.iter().any(|e| expr_contains_get_node(e, _verbose))
                }
            },
            Expr::StructNew(_, fields) => fields
                .iter()
                .any(|(_, expr)| expr_contains_get_node(expr, _verbose)),
            _ => false,
        }
    }

    fn is_cloned_ui_element_expr(expr: &Expr) -> Option<(String, String)> {
        // Returns (ui_node_var, element_name) if this is ui_node.get_element("name") as UIText
        match expr {
            Expr::Cast(inner, target_type) => {
                if let Expr::Call(target, args) = inner.as_ref() {
                    if let Expr::MemberAccess(base, method) = target.as_ref() {
                        if method == "get_element" && args.len() == 1 {
                            // Extract ui_node variable name from base
                            if let Expr::Ident(ui_node_var) = base.as_ref() {
                                // Extract element name from args[0] (args is Vec<Expr>, not Vec<TypedExpr>)
                                if let Expr::Literal(crate::ast::Literal::String(element_name)) =
                                    &args[0]
                                {
                                    return Some((ui_node_var.clone(), element_name.clone()));
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        None
    }

    fn is_new_node_expr(expr: &Expr) -> bool {
        // Check if this expression creates a new node (StructNew with node type)
        match expr {
            Expr::StructNew(ty, _) => is_node_type(ty),
            _ => false,
        }
    }

    for stmt in stmts {
        match stmt {
            Stmt::VariableDecl(var) => {
                if let Some(value) = &var.value {
                    // Check if the expression contains GetChildByName - if so, track the variable
                    if expr_contains_get_node(&value.expr, script.verbose) {
                        cloned_nodes.push(var.name.clone());
                    } else if is_new_node_expr(&value.expr) {
                        // Track newly created nodes (like var n = new Node2D())
                        cloned_nodes.push(var.name.clone());
                    } else if let Some((ui_node_var, element_name)) =
                        is_cloned_ui_element_expr(&value.expr)
                    {
                        cloned_ui_elements.push((ui_node_var, element_name, var.name.clone()));
                    }
                }
            }
            Stmt::Assign(name, expr) => {
                // Check if the expression contains GetChildByName - if so, track the variable
                if expr_contains_get_node(&expr.expr, script.verbose) {
                    cloned_nodes.push(name.clone());
                } else if is_new_node_expr(&expr.expr) {
                    // Track newly created nodes (like n = new Node2D())
                    cloned_nodes.push(name.clone());
                } else if let Some((ui_node_var, element_name)) =
                    is_cloned_ui_element_expr(&expr.expr)
                {
                    cloned_ui_elements.push((ui_node_var, element_name, name.clone()));
                }
            }
            Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                collect_cloned_node_vars(then_body, cloned_nodes, cloned_ui_elements, script);
                if let Some(else_body) = else_body {
                    collect_cloned_node_vars(else_body, cloned_nodes, cloned_ui_elements, script);
                }
            }
            Stmt::For { body, .. } | Stmt::ForTraditional { body, .. } => {
                collect_cloned_node_vars(body, cloned_nodes, cloned_ui_elements, script);
            }
            _ => {}
        }
    }
}

/// Check if a parameter is mutated in the function body
fn param_is_mutated(param_name: &str, body: &[Stmt]) -> bool {
    use crate::scripting::ast::{Expr, Stmt};
    
    for stmt in body {
        match stmt {
            // Direct assignment to parameter
            Stmt::Assign(name, _) if name == param_name => return true,
            
            // Assignment operations on parameter
            Stmt::AssignOp(name, _, _) if name == param_name => return true,
            
            // Member assignment (e.g., param.field = value)
            Stmt::MemberAssign(lhs, _) => {
                if expr_refers_to_param(&lhs.expr, param_name) {
                    return true;
                }
            }
            
            // Member assignment with operator (e.g., param.field += value)
            Stmt::MemberAssignOp(lhs, _, _) => {
                if expr_refers_to_param(&lhs.expr, param_name) {
                    return true;
                }
            }
            
            // Recursively check nested blocks
            Stmt::If { then_body, else_body, .. } => {
                if param_is_mutated(param_name, then_body) {
                    return true;
                }
                if let Some(else_body) = else_body {
                    if param_is_mutated(param_name, else_body) {
                        return true;
                    }
                }
            }
            
            Stmt::For { body, .. } | Stmt::ForTraditional { body, .. } => {
                if param_is_mutated(param_name, body) {
                    return true;
                }
            }
            
            _ => {}
        }
    }
    
    false
}

/// Check if a variable is declared inside a loop (loop-scoped)
fn is_variable_loop_scoped(var_name: &str, body: &[Stmt]) -> bool {
    use crate::scripting::ast::{Expr, Stmt};
    
    for stmt in body {
        match stmt {
            Stmt::For { body: loop_body, .. } | Stmt::ForTraditional { body: loop_body, .. } => {
                // Check if variable is declared inside this loop
                for loop_stmt in loop_body {
                    match loop_stmt {
                        Stmt::VariableDecl(var) if var.name == var_name => {
                            // Variable is declared in loop - it's loop-scoped
                            return true;
                        }
                        _ => {}
                    }
                }
                // Recursively check nested loops
                if is_variable_loop_scoped(var_name, loop_body) {
                    return true;
                }
            }
            _ => {}
        }
    }
    
    false
}

/// Check if an expression refers to a parameter (directly or via field access)
fn expr_refers_to_param(expr: &Expr, param_name: &str) -> bool {
    use crate::scripting::ast::Expr;
    
    match expr {
        Expr::Ident(name) => name == param_name,
        Expr::MemberAccess(base, _) => expr_refers_to_param(base, param_name),
        Expr::Index(base, _) => expr_refers_to_param(base, param_name),
        _ => false,
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
        let renamed_struct_name = rename_struct(&self.name);
        writeln!(out, "pub struct {} {{", renamed_struct_name).unwrap();

        for field in &self.fields {
            writeln!(out, "    pub {}: {},", field.name, field.typ.to_rust_type()).unwrap();
        }

        writeln!(out, "}}\n").unwrap();

        // === Display Implementation ===
        let renamed_struct_name = rename_struct(&self.name);
        writeln!(out, "impl std::fmt::Display for {} {{", renamed_struct_name).unwrap();
        writeln!(
            out,
            "    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{"
        )
        .unwrap();
        writeln!(out, "        write!(f, \"{{{{ \")?;").unwrap();

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

        // === Constructor Method ===
        let renamed_struct_name = rename_struct(&self.name);
        writeln!(out, "impl {} {{", renamed_struct_name).unwrap();
        write!(out, "    pub fn new(").unwrap();
        let mut param_list = Vec::new();
        for field in &self.fields {
            param_list.push(format!("{}: {}", field.name, field.typ.to_rust_type()));
        }
        writeln!(out, "{}) -> Self {{", param_list.join(", ")).unwrap();
        write!(out, "        Self {{").unwrap();
        for field in &self.fields {
            write!(out, " {}: {},", field.name, field.name).unwrap();
        }
        writeln!(out, " }}").unwrap();
        writeln!(out, "    }}").unwrap();
        writeln!(out, "}}\n").unwrap();

        // === Method Implementations ===
        if !self.methods.is_empty() {
            writeln!(out, "impl {} {{", renamed_struct_name).unwrap();
            for m in &self.methods {
                out.push_str(&m.to_rust_method(&self.name, script));
            }
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
                .map(|p| {
                    // Check if it's a type that becomes Uuid or Option<Uuid> - if so, rename to {name}_id and use Uuid/Option<Uuid>
                    if type_becomes_id(&p.typ) {
                        let renamed = rename_variable(&p.name, Some(&p.typ));
                        // If it's Option<Uuid>, use Option<Uuid>; otherwise use Uuid
                        if matches!(&p.typ, Type::Option(boxed) if matches!(boxed.as_ref(), Type::Uuid)) {
                            format!("mut {}: Option<Uuid>", renamed)
                        } else {
                            format!("mut {}: Uuid", renamed)
                        }
                    } else {
                        match &p.typ {
                            // Strings: passed as owned String
                            Type::String => format!("mut {}: String", p.name),

                            // Custom structs and script types: passed as owned and mutable
                            Type::Custom(name) => {
                                format!("mut {}: {}", p.name, name)
                            }

                            // Plain primitives: passed by value
                            _ => format!("mut {}: {}", p.name, p.typ.to_rust_type()),
                        }
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");

            write!(param_list, ", {}", joined).unwrap();
        }

        param_list.push_str(", api: &mut ScriptApi<'_>");

        let renamed_func_name = rename_function(&self.name);
        writeln!(out, "    fn {}({}) {{", renamed_func_name, param_list).unwrap();

        // ---------------------------------------------------
        // (1) Insert additional preamble if the method uses self/api
        // ---------------------------------------------------
        let needs_self = self.uses_self;

        // ---------------------------------------------------
        // (2) Use cloned child nodes that were already collected during analysis
        // ---------------------------------------------------
        // cloned_child_nodes was already populated by collect_cloned_node_vars during analyze_self_usage
        // which recursively scans all statements including nested if/for blocks and Assign statements
        let cloned_node_vars = &self.cloned_child_nodes;

        // Collect cloned UI elements (we still need to scan for these)
        let mut cloned_ui_elements: Vec<(String, String, String)> = Vec::new();
        collect_cloned_node_vars(&self.body, &mut Vec::new(), &mut cloned_ui_elements, script);

        // ---------------------------------------------------
        // (2.5) No longer need to track loop nodes for merging
        // ---------------------------------------------------

        // ---------------------------------------------------
        // (3) Emit body
        // ---------------------------------------------------
        for stmt in &self.body {
            out.push_str(&stmt.to_rust(needs_self, script, Some(self)));
        }

        // ---------------------------------------------------
        // (4) No longer need to merge nodes - we use mutate_node for assignments
        // ---------------------------------------------------

        // Merge cloned UI elements back into their UINodes
        if !cloned_ui_elements.is_empty() {
            out.push_str("\n        // Merge cloned UI elements back\n");
            // Group by ui_node_var
            use std::collections::HashMap;
            let mut by_ui_node: HashMap<String, Vec<(String, String)>> = HashMap::new();
            for (ui_node_var, element_name, element_var) in &cloned_ui_elements {
                by_ui_node
                    .entry(ui_node_var.clone())
                    .or_insert_with(Vec::new)
                    .push((element_name.clone(), element_var.clone()));
            }
            for (ui_node_var, elements) in by_ui_node {
                let merge_pairs: Vec<String> = elements
                    .iter()
                    .map(|(name, var)| {
                        format!(
                            "(\"{}\".to_string(), crate::ui_element::UIElement::Text({}.clone()))",
                            name, var
                        )
                    })
                    .collect();
                out.push_str(&format!(
                    "        {}.merge_elements(vec![{}]);\n",
                    ui_node_var,
                    merge_pairs.join(", ")
                ));
            }
        }

        out.push_str("    }\n\n");
        
        // Post-process to batch consecutive mutations on the same node
        batch_consecutive_mutations(&out)
    }
}

/// Post-process generated Rust code to batch consecutive api.mutate_node calls on the same node
fn batch_consecutive_mutations(code: &str) -> String {
    use regex::Regex;
    
    // Pattern to match: api.mutate_node(node_id, |closure_var: &mut NodeType| { body });
    let re = Regex::new(r"(?m)^\s*api\.mutate_node\(([^,]+),\s*\|([^:]+):\s*&mut\s+([^|]+)\|\s*\{\s*([^}]+)\s*\}\);?\s*$").unwrap();
    
    let lines: Vec<&str> = code.lines().collect();
    let mut result = String::with_capacity(code.len());
    let mut i = 0;
    
    while i < lines.len() {
        let line = lines[i];
        
        // Try to match a mutate_node call
        if let Some(caps) = re.captures(line) {
            let node_id = caps.get(1).unwrap().as_str();
            let closure_var = caps.get(2).unwrap().as_str();
            let node_type = caps.get(3).unwrap().as_str();
            let first_body = caps.get(4).unwrap().as_str();
            
            // Collect all consecutive mutations on the same node
            let mut bodies = vec![first_body.trim()];
            let mut j = i + 1;
            
            while j < lines.len() {
                if let Some(next_caps) = re.captures(lines[j]) {
                    let next_node_id = next_caps.get(1).unwrap().as_str();
                    let next_closure_var = next_caps.get(2).unwrap().as_str();
                    let next_node_type = next_caps.get(3).unwrap().as_str();
                    
                    // Same node, same closure var, same type - batch it
                    if next_node_id == node_id && next_closure_var == closure_var && next_node_type == node_type {
                        let next_body = next_caps.get(4).unwrap().as_str();
                        bodies.push(next_body.trim());
                        j += 1;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            
            // Generate batched mutation
            if bodies.len() > 1 {
                // Multiple mutations - batch them
                let indent = line.chars().take_while(|c| c.is_whitespace()).collect::<String>();
                result.push_str(&format!("{}api.mutate_node({}, |{}: &mut {}| {{\n", indent, node_id, closure_var, node_type));
                for body in &bodies {
                    // Remove trailing semicolon from body if present (we'll add it back)
                    let body_trimmed = body.trim_end_matches(';').trim();
                    result.push_str(&format!("{}    {};\n", indent, body_trimmed));
                }
                result.push_str(&format!("{}}});\n", indent));
            } else {
                // Single mutation - keep as is
                result.push_str(line);
                result.push('\n');
            }
            
            i = j;
        } else {
            // Not a mutate_node call, keep as is
            result.push_str(line);
            result.push('\n');
            i += 1;
        }
    }
    
    result
}

impl Function {
    // ============================================================
    // for trait-style API methods (unchanged, still fine)
    // ============================================================
    pub fn to_rust_trait_method(&self, node_type: &str, script: &Script) -> String {
        let mut out = String::with_capacity(512);
        writeln!(
            out,
            "    fn {}(&mut self, api: &mut ScriptApi<'_>) {{",
            self.name.to_lowercase()
        )
        .unwrap();

        let needs_self = self.uses_self;

        // Emit body
        for stmt in &self.body {
            out.push_str(&stmt.to_rust(needs_self, script, Some(self)));
        }

        out.push_str("    }\n\n");
        
        // Post-process to batch consecutive mutations on the same node
        batch_consecutive_mutations(&out)
    }
}

impl Stmt {
    fn to_rust(
        &self,
        needs_self: bool,
        script: &Script,
        current_func: Option<&Function>,
    ) -> String {
        match self {
            Stmt::Expr(expr) => {
                let expr_str = expr.to_rust(needs_self, script, current_func);
                // expr is TypedExpr, which already passes span through
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
                    // Pass the variable's type as expected type so map/array literals know what to generate
                    let raw_expr =
                        expr.expr
                            .to_rust(needs_self, script, var.typ.as_ref(), current_func, expr.span.as_ref());

                    // Check if we need to clone based on both the expression type and variable type
                    let var_type_is_custom = var
                        .typ
                        .as_ref()
                        .map_or(false, |t| matches!(t, Type::Custom(_)));
                    let var_type_requires_clone =
                        var.typ.as_ref().map_or(false, |t| t.requires_clone());
                    let expr_type = script.infer_expr_type(&expr.expr, current_func);

                    // Check if the expression is an Ident that refers to a struct field (which will get self. prefix)
                    // OR if it's a Cast with an Ident inside that is a struct field
                    let is_struct_field_access = match &expr.expr {
                        Expr::Ident(name) => script.is_struct_field(name),
                        Expr::Cast(inner, _) => {
                            if let Expr::Ident(name) = inner.as_ref() {
                                script.is_struct_field(name)
                            } else {
                                false
                            }
                        }
                        Expr::MemberAccess(..) => true, // MemberAccess always needs checking
                        _ => false,
                    };

                    let needs_clone = if is_struct_field_access {
                        // Always clone struct field access when assigning to a custom type (to avoid move errors)
                        var_type_is_custom
                            || var_type_requires_clone
                            || expr_type.as_ref().map_or(false, |ty| ty.requires_clone())
                    } else if matches!(expr.expr, Expr::Ident(_)) {
                        // Clone if the expression type requires it, or if assigning to a custom type
                        expr_type.as_ref().map_or(var_type_requires_clone, |ty| {
                            ty.requires_clone() || var_type_requires_clone
                        })
                    } else {
                        false
                    };

                    // Also check if the generated code contains self. but doesn't have .clone() yet
                    // This handles cases where casts might be optimized away but we still need to clone
                    let needs_clone_fallback = if !needs_clone && var_type_is_custom {
                        raw_expr.contains("self.") && !raw_expr.contains(".clone()")
                    } else {
                        false
                    };

                    // Don't clone if the expression already produces an owned value (e.g., from unwrap_or_default, from_str, etc.)
                    // This is important for generic functions like FromPrimitive::from_f32 where cloning breaks type inference
                    // Also don't clone if the expression already produces an owned value
                    // Note: read_node now returns Clone types, and for non-Copy types the .clone() inside the closure
                    // already produces an owned value, so read_node calls don't need an extra .clone()
                    let already_owned = raw_expr.contains(".unwrap_or_default()")
                        || raw_expr.contains(".unwrap()")
                        || raw_expr.contains("::from_str")
                        || raw_expr.contains("::from(")
                        || raw_expr.contains("::new(")
                        || raw_expr.contains("get_element_clone")
                        || raw_expr.contains("read_node(");

                    if (needs_clone || needs_clone_fallback) && !already_owned {
                        format!("{}.clone()", raw_expr)
                    } else {
                        raw_expr
                    }
                } else if var.typ.is_some() {
                    var.default_value()
                } else {
                    String::new()
                };

                // Check if the expression contains a temporary variable extraction for mutable API calls
                // Pattern: "let __parent_id = api.get_parent(...); api.read_node(...)" or "api.get_type(...)" etc.
                let (temp_stmt, final_expr_str) = if expr_str.contains("let __") && (expr_str.contains("; api.") || expr_str.contains(";api.")) {
                    // Extract the temporary variable declaration and the actual expression
                    // Look for any API call after the temp declaration
                    let semi_pos = expr_str.find("; api.")
                        .or_else(|| expr_str.find(";api."));
                    if let Some(pos) = semi_pos {
                        let temp_decl = expr_str[..pos + 1].trim_start().to_string();
                        // Skip "; " or ";"
                        let actual_expr = if expr_str.as_bytes().get(pos + 1) == Some(&b' ') {
                            &expr_str[pos + 2..]
                        } else {
                            &expr_str[pos + 1..]
                        };
                        (Some(temp_decl), actual_expr.to_string())
                    } else {
                        (None, expr_str.clone())
                    }
                } else {
                    (None, expr_str.clone())
                };

                // Add type annotation if variable has explicit type OR if we can infer from the expression
                let inferred_type = if let Some(expr) = &var.value {
                    script.infer_expr_type(&expr.expr, current_func)
                } else {
                    None
                };
                
                // Helper to convert type to Rust type annotation
                // Special case: Texture (EngineStruct) becomes Option<Uuid> in Rust
                let type_to_rust_annotation = |typ: &Type| -> String {
                    match typ {
                        Type::EngineStruct(EngineStructKind::Texture) => "Option<Uuid>".to_string(),
                        _ => typ.to_rust_type(),
                    }
                };
                
                let type_annotation = if let Some(typ) = &var.typ {
                    format!(": {}", type_to_rust_annotation(typ))
                } else if let Some(ref inferred) = inferred_type {
                    format!(": {}", type_to_rust_annotation(inferred))
                } else {
                    String::new()
                };

                // Use inferred type for renaming if var.typ is None
                let type_for_renaming = var.typ.as_ref().or(inferred_type.as_ref());
                let renamed_name = rename_variable(&var.name, type_for_renaming);
                
                // If we extracted a temporary statement, prepend it on the same line
                if let Some(ref temp_stmt) = temp_stmt {
                    if expr_str.is_empty() {
                        format!("        {} let mut {}{};\n", temp_stmt.trim_end(), renamed_name, type_annotation)
                    } else {
                        format!(
                            "        {} let mut {}{} = {};\n",
                            temp_stmt.trim_end(), renamed_name, type_annotation, final_expr_str
                        )
                    }
                } else if expr_str.is_empty() {
                    format!("        let mut {}{};\n", renamed_name, type_annotation)
                } else {
                    format!(
                        "        let mut {}{} = {};\n",
                        renamed_name, type_annotation, final_expr_str
                    )
                }
            }
            Stmt::Assign(name, expr) => {
                let var_type = script.get_variable_type(name);
                let expr_type = script.infer_expr_type(&expr.expr, current_func);
                
                // Check if the expression returns a UUID that represents a node or texture
                // (e.g., get_parent(), get_child_by_name(), Texture.load(), casts to node types, etc.)
                // OR if it returns NodeType or DynNode (which are also node UUID types)
                let is_direct_node_call = matches!(&expr.expr, 
                    Expr::ApiCall(ApiModule::NodeSugar(NodeSugarApi::GetParent), _) |
                    Expr::ApiCall(ApiModule::NodeSugar(NodeSugarApi::GetChildByName), _)
                );
                
                let is_direct_texture_call = matches!(&expr.expr,
                    Expr::ApiCall(ApiModule::Texture(TextureApi::Load), _) |
                    Expr::ApiCall(ApiModule::Texture(TextureApi::CreateFromBytes), _)
                );
                
                let is_node_cast = matches!(expr_type, Some(Type::Uuid)) && 
                    if let Expr::Cast(_, ref target_type) = expr.expr {
                        match target_type {
                            Type::Node(_) => true,
                            Type::Custom(tn) => is_node_type(tn),
                            _ => false,
                        }
                    } else {
                        false
                    };
                
                let is_id_uuid = is_direct_node_call || is_direct_texture_call || is_node_cast;
                
                // Check if the return type is NodeType or DynNode (from get_type(), etc.)
                let is_node_type_return = matches!(expr_type, Some(Type::NodeType | Type::DynNode));
                
                // If it's a UUID/Option<Uuid> representing a node/texture, or returns NodeType/DynNode, use _id suffix naming
                // This follows the same pattern as nodes: check both var_type and expr_type for Uuid/Option<Uuid>
                let is_id_type = matches!(var_type, Some(Type::Uuid)) 
                    || matches!(expr_type.as_ref(), Some(Type::Uuid)) 
                    || matches!(expr_type.as_ref(), Some(Type::Option(boxed)) if matches!(boxed.as_ref(), Type::Uuid));
                
                let type_for_renaming = if is_id_uuid && is_id_type {
                    // For node calls returning Uuid, treat as node type for naming
                    // For texture calls returning Texture (EngineStruct), use the actual type
                    if is_direct_texture_call {
                        expr_type.as_ref().or(var_type)
                    } else {
                        Some(&Type::Node(NodeType::Node)) // Treat as node type for naming
                    }
                } else if is_node_type_return || matches!(var_type, Some(Type::NodeType | Type::DynNode)) {
                    // Use the actual type (NodeType or DynNode) for naming
                    expr_type.as_ref().or(var_type)
                } else {
                    var_type
                };
                
                let renamed_name = rename_variable(name, type_for_renaming);
                let target = if script.is_struct_field(name) && !name.starts_with("self.") {
                    format!("self.{}", renamed_name)
                } else {
                    renamed_name
                };

                let target_type = self.get_target_type(name, script, current_func);

                // FIRST: Check for nested API calls at AST level BEFORE generating the string
                // This ensures we extract temp variables correctly and api is never renamed
                let (temp_decl_opt, modified_expr) = match &expr.expr {
                    Expr::ApiCall(outer_api, outer_args) => {
                        // Check if any argument is itself an API call that returns Uuid (or wrapped in a Cast)
                        let mut temp_decls = Vec::new();
                        let mut new_args = Vec::new();
                        let mut has_nested = false;
                        
                        for arg in outer_args.iter() {
                            // Check if arg is a Cast containing an ApiCall
                            let inner_api_call = if let Expr::Cast(inner_expr, _) = arg {
                                if let Expr::ApiCall(inner_api, inner_args) = inner_expr.as_ref() {
                                    Some((inner_api, inner_args))
                                } else {
                                    None
                                }
                            } else if let Expr::ApiCall(inner_api, inner_args) = arg {
                                Some((inner_api, inner_args))
                            } else {
                                None
                            };
                            
                            if let Some((inner_api, inner_args)) = inner_api_call {
                                if let Some(return_type) = inner_api.return_type() {
                                    // Check if it returns Uuid, DynNode, or Option<Uuid> (all need extraction)
                                    let needs_extraction = matches!(return_type, Type::Uuid | Type::DynNode) || 
                                        matches!(return_type, Type::Option(boxed) if matches!(boxed.as_ref(), Type::Uuid));
                                    
                                    if needs_extraction {
                                        has_nested = true;
                                        
                                        // Generate the inner call string - this should generate "api.get_parent(...)"
                                        let mut inner_call_str = inner_api.to_rust(inner_args, script, needs_self, current_func);
                                        
                                        // Fix any incorrect renaming of "api" to "__t_api" or "t_id_api"
                                        // The "api" identifier should NEVER be renamed - it's always the API parameter
                                        inner_call_str = inner_call_str.replace("__t_api.", "api.").replace("t_id_api.", "api.");
                                        
                                        // Generate temp variable name based on inner API
                                        let temp_var = match inner_api {
                                            ApiModule::NodeSugar(NodeSugarApi::GetParent) => "__parent_id",
                                            ApiModule::NodeSugar(NodeSugarApi::GetChildByName) => "__child_id",
                                            _ => "__temp_id",
                                        };
                                        
                                        // Only add temp declaration if we haven't seen this temp var yet
                                        if !temp_decls.iter().any(|(var, _)| *var == temp_var) {
                                            let type_annotation = if temp_var == "__parent_id" || temp_var == "__child_id" {
                                                ": Uuid"
                                            } else {
                                                ""
                                            };
                                            temp_decls.push((temp_var, format!("let {}{} = {};", temp_var, type_annotation, inner_call_str)));
                                        }
                                        
                                        // Replace the nested call with a temp variable identifier
                                        // If the original was a Cast, we don't need the cast anymore since we're extracting to a temp var
                                        // The temp var is already a Uuid, so we can use it directly
                                        new_args.push(Expr::Ident(temp_var.to_string()));
                                    } else {
                                        new_args.push(arg.clone());
                                    }
                                } else {
                                    new_args.push(arg.clone());
                                }
                            } else {
                                new_args.push(arg.clone());
                            }
                        }
                        
                        if has_nested && !temp_decls.is_empty() {
                            // Create a new expression with temp variables replaced
                            let new_expr = Expr::ApiCall(outer_api.clone(), new_args);
                            // Join temp declarations with spaces to put them on the same line
                            let all_temp_decls = temp_decls.iter().map(|(_, decl)| decl.clone()).collect::<Vec<_>>().join(" ");
                            (Some(all_temp_decls), Some(new_expr))
                        } else {
                            (None, None)
                        }
                    }
                    Expr::Call(target, call_args) => {
                        // Handle calls like api.read_node(api.get_parent(...), ...)
                        // Check if any argument is an API call that returns Uuid
                        let mut temp_decls = Vec::new();
                        let mut new_call_args = Vec::new();
                        let mut has_nested = false;
                        
                        for arg in call_args.iter() {
                            if let Expr::ApiCall(inner_api, inner_args) = arg {
                                if let Some(return_type) = inner_api.return_type() {
                                    // Check if it returns Uuid, DynNode, or Option<Uuid> (all need extraction)
                                    let needs_extraction = matches!(return_type, Type::Uuid | Type::DynNode) || 
                                        matches!(return_type, Type::Option(boxed) if matches!(boxed.as_ref(), Type::Uuid));
                                    
                                    if needs_extraction {
                                        has_nested = true;
                                        
                                        // Generate the inner call string - this should generate "api.get_parent(...)"
                                        let mut inner_call_str = inner_api.to_rust(inner_args, script, needs_self, current_func);
                                        
                                        // Fix any incorrect renaming of "api" to "__t_api" or "t_id_api"
                                        // The "api" identifier should NEVER be renamed - it's always the API parameter
                                        inner_call_str = inner_call_str.replace("__t_api.", "api.").replace("t_id_api.", "api.");
                                        
                                        // Generate temp variable name based on inner API
                                        let temp_var = match inner_api {
                                            ApiModule::NodeSugar(NodeSugarApi::GetParent) => "__parent_id",
                                            ApiModule::NodeSugar(NodeSugarApi::GetChildByName) => "__child_id",
                                            _ => "__temp_id",
                                        };
                                        
                                        // Only add temp declaration if we haven't seen this temp var yet
                                        if !temp_decls.iter().any(|(var, _)| *var == temp_var) {
                                            let type_annotation = if temp_var == "__parent_id" || temp_var == "__child_id" {
                                                ": Uuid"
                                            } else {
                                                ""
                                            };
                                            temp_decls.push((temp_var, format!("let {}{} = {};", temp_var, type_annotation, inner_call_str)));
                                        }
                                        
                                        // Replace the nested call with a temp variable identifier
                                        new_call_args.push(Expr::Ident(temp_var.to_string()));
                                    } else {
                                        new_call_args.push(arg.clone());
                                    }
                                } else {
                                    new_call_args.push(arg.clone());
                                }
                            } else {
                                new_call_args.push(arg.clone());
                            }
                        }
                        
                        if has_nested && !temp_decls.is_empty() {
                            // Create a new expression with temp variables replaced
                            let new_expr = Expr::Call(target.clone(), new_call_args);
                            // Join temp declarations with spaces to put them on the same line
                            let all_temp_decls = temp_decls.iter().map(|(_, decl)| decl.clone()).collect::<Vec<_>>().join(" ");
                            (Some(all_temp_decls), Some(new_expr))
                        } else {
                            (None, None)
                        }
                    }
                    _ => (None, None)
                };
                
                // Generate the expression string - use modified expression if we have one, otherwise use original
                let mut expr_str = if let Some(ref modified) = modified_expr {
                    modified.to_rust(needs_self, script, target_type.as_ref(), current_func, None)
                } else {
                    expr.expr.to_rust(needs_self, script, target_type.as_ref(), current_func, expr.span.as_ref())
                };
                
                // If we didn't catch it at AST level, try string-based detection as fallback
                let (temp_decl_opt, mut final_expr_str) = if temp_decl_opt.is_none() {
                    // Check if the expression string already contains an embedded temp declaration
                    // Pattern: "let __parent_id = api.get_parent(...); api.read_node(...)"
                    // or "let __parent_id: Uuid = api.get_parent(...); api.read_node(...)"
                    if expr_str.starts_with("let __") && (expr_str.contains("; api.") || expr_str.contains(";api.")) {
                        // Extract the temp declaration and the actual expression
                        let semi_pos = expr_str.find("; api.")
                            .or_else(|| expr_str.find(";api."));
                        if let Some(pos) = semi_pos {
                            // Extract temp declaration without leading spaces
                            let temp_decl = expr_str[..pos + 1].trim_start().to_string();
                            // Skip "; " or ";"
                            let actual_expr = if expr_str.as_bytes().get(pos + 1) == Some(&b' ') {
                                &expr_str[pos + 2..]
                            } else {
                                &expr_str[pos + 1..]
                            };
                            (Some(temp_decl), actual_expr.to_string())
                        } else {
                            (None, expr_str)
                        }
                    }
                    // For non-API-call expressions, use string-based detection
                    // Check for both "api.get_parent(" and "t_id_api.get_parent(" (in case api was renamed)
                    else if (expr_str.contains("api.get_parent(") || expr_str.contains("api.get_child_by_name(") || 
                                     expr_str.contains("t_id_api.get_parent(") || expr_str.contains("t_id_api.get_child_by_name(")) &&
                                    (expr_str.matches("api.").count() > 1 || expr_str.matches("t_id_api.").count() > 0) {
                        // Find the inner API call - check for both "api.get_parent(" and "t_id_api.get_parent("
                        let inner_start = expr_str.find("api.get_parent(")
                            .or_else(|| expr_str.find("api.get_child_by_name("))
                            .or_else(|| expr_str.find("t_id_api.get_parent("))
                            .or_else(|| expr_str.find("t_id_api.get_child_by_name("));
                        
                        if let Some(start) = inner_start {
                            // Find the matching closing parenthesis for the inner call
                            let mut depth = 0;
                            let mut end = start;
                            for (i, ch) in expr_str[start..].char_indices() {
                                if ch == '(' {
                                    depth += 1;
                                } else if ch == ')' {
                                    depth -= 1;
                                    if depth == 0 {
                                        end = start + i + 1;
                                        break;
                                    }
                                }
                            }
                            
                            let inner_call = &expr_str[start..end];
                            // Check if this inner call is already a temp variable (avoid redeclaration)
                            if inner_call.starts_with("__") && !inner_call.contains("(") {
                                // It's already a temp variable, don't redeclare
                                (None, expr_str)
                            } else {
                                let temp_var = if inner_call.contains("get_parent") {
                                    "__parent_id"
                                } else {
                                    "__child_id"
                                };
                                
                                // Fix the inner call - replace any incorrect renaming of "api" back to "api"
                                // The "api" identifier should NEVER be renamed - it's always the API parameter
                                let fixed_inner_call = inner_call.replace("__t_api.", "api.").replace("t_id_api.", "api.");
                                
                                // Check if we're trying to assign temp_var to itself (avoid "__parent_id = __parent_id")
                                if fixed_inner_call == temp_var {
                                    (None, expr_str)
                                } else if expr_str.contains(&format!("let {} =", temp_var)) {
                                    // Already declared earlier, just replace the inner call
                                    let final_expr = expr_str.replace(inner_call, temp_var);
                                    (None, final_expr)
                                } else {
                                    let type_annotation = if temp_var == "__parent_id" || temp_var == "__child_id" {
                                        ": Uuid"
                                    } else {
                                        ""
                                    };
                                    let temp_decl = format!("let {}{} = {};", temp_var, type_annotation, fixed_inner_call);
                                    let final_expr = expr_str.replace(inner_call, temp_var);
                                    (Some(temp_decl), final_expr)
                                }
                            }
                        } else {
                            (None, expr_str)
                        }
                    } else {
                        (None, expr_str)
                    }
                } else {
                    (temp_decl_opt, expr_str)
                };

                // Clone if:
                // 1. Expression type requires clone (BigInt, Decimal, String, etc.)
                // 2. OR if it's a MemberAccess and target type is a custom type (to avoid move errors)
                let should_clone = if matches!(expr.expr, Expr::Ident(_) | Expr::MemberAccess(..)) {
                    let expr_requires_clone =
                        expr_type.as_ref().map_or(false, |ty| ty.requires_clone());
                    let target_is_custom = target_type
                        .as_ref()
                        .map_or(false, |t| matches!(t, Type::Custom(_)));
                    let is_member_access = matches!(expr.expr, Expr::MemberAccess(..));
                    expr_requires_clone || (is_member_access && target_is_custom)
                } else {
                    false
                };

                if should_clone {
                    final_expr_str = format!("{}.clone()", final_expr_str);
                }

                let final_expr = if let Some(target_type) = &target_type {
                    if let Some(expr_type) = &expr_type {
                        if expr_type.can_implicitly_convert_to(target_type)
                            && expr_type != target_type
                        {
                            script.generate_implicit_cast_for_expr(
                                &final_expr_str,
                                expr_type,
                                target_type,
                            )
                        } else {
                            final_expr_str
                        }
                    } else {
                        final_expr_str
                    }
                } else {
                    final_expr_str
                };

                // If we have a temp declaration, prepend it before the assignment on the same line
                if let Some(temp_decl) = temp_decl_opt {
                    format!("        {} {} = {};\n", temp_decl, target, final_expr)
                } else {
                    format!("        {} = {};\n", target, final_expr)
                }
            }

            Stmt::AssignOp(name, op, expr) => {
                let var_type = script.get_variable_type(name);
                let expr_type = script.infer_expr_type(&expr.expr, current_func);
                
                // Check if the expression returns a UUID that represents a node or texture
                let is_direct_node_call = matches!(&expr.expr, 
                    Expr::ApiCall(ApiModule::NodeSugar(NodeSugarApi::GetParent), _) |
                    Expr::ApiCall(ApiModule::NodeSugar(NodeSugarApi::GetChildByName), _)
                );
                
                let is_direct_texture_call = matches!(&expr.expr,
                    Expr::ApiCall(ApiModule::Texture(TextureApi::Load), _) |
                    Expr::ApiCall(ApiModule::Texture(TextureApi::CreateFromBytes), _)
                );
                
                let is_node_cast = matches!(expr_type, Some(Type::Uuid)) && 
                    if let Expr::Cast(_, ref target_type) = expr.expr {
                        match target_type {
                            Type::Node(_) => true,
                            Type::Custom(tn) => is_node_type(tn),
                            _ => false,
                        }
                    } else {
                        false
                    };
                
                let is_id_uuid = is_direct_node_call || is_direct_texture_call || is_node_cast;
                
                // Check if the return type is NodeType or DynNode
                let is_node_type_return = matches!(expr_type, Some(Type::NodeType | Type::DynNode));
                
                // Determine type for renaming (same logic as Assign)
                let is_id_type = matches!(var_type, Some(Type::Uuid)) 
                    || matches!(expr_type.as_ref(), Some(Type::Uuid)) 
                    || matches!(expr_type.as_ref(), Some(Type::Option(boxed)) if matches!(boxed.as_ref(), Type::Uuid));
                
                let type_for_renaming = if is_id_uuid && is_id_type {
                    if is_direct_texture_call {
                        expr_type.as_ref().or(var_type)
                    } else {
                        Some(&Type::Node(NodeType::Node))
                    }
                } else if is_node_type_return || matches!(var_type, Some(Type::NodeType | Type::DynNode)) {
                    expr_type.as_ref().or(var_type)
                } else {
                    var_type
                };
                
                let renamed_name = rename_variable(name, type_for_renaming);
                let target = if script.is_struct_field(name) && !name.starts_with("self.") {
                    format!("self.{}", renamed_name)
                } else {
                    renamed_name
                };

                let target_type = self.get_target_type(name, script, current_func);
                let expr_str =
                    expr.expr
                        .to_rust(needs_self, script, target_type.as_ref(), current_func, expr.span.as_ref());

                if matches!(op, Op::Add) && target_type == Some(Type::String) {
                    return format!("        {target}.push_str({expr_str}.as_str());\n");
                }

                if let Some(target_type) = &target_type {
                    let expr_type = script.infer_expr_type(&expr.expr, current_func);
                    if let Some(expr_type) = expr_type {
                        let cast_expr = if expr_type.can_implicitly_convert_to(target_type)
                            && &expr_type != target_type
                        {
                            Self::generate_implicit_cast(&expr_str, &expr_type, target_type)
                        } else {
                            expr_str
                        };
                        // For Decimal AddAssign, ensure the expression is clearly typed as owned Decimal
                        let final_expr = if *target_type == Type::Number(NumberKind::Decimal)
                            && matches!(op, Op::Add)
                        {
                            // Use a block with explicit type to help compiler choose AddAssign<Decimal> impl
                            format!("{{ let tmp: Decimal = {}; tmp }}", cast_expr)
                        } else {
                            cast_expr
                        };
                        format!(
                            "        {} {}= {};\n",
                            target,
                            op.to_rust_assign(),
                            final_expr
                        )
                    } else {
                        format!(
                            "        {} {}= {};\n",
                            target,
                            op.to_rust_assign(),
                            expr_str
                        )
                    }
                } else {
                    format!(
                        "        {} {}= {};\n",
                        target,
                        op.to_rust_assign(),
                        expr_str
                    )
                }
            }

            Stmt::MemberAssign(lhs_expr, rhs_expr) => {
                // Check if this is a node member assignment (like self.transform.position.x = value)
                if let Some((node_id, node_type, field_path, closure_var)) = 
                    extract_node_member_info(&lhs_expr.expr, script, current_func) 
                {
                    // Check if this is a DynNode (special marker)
                    if node_type == "__DYN_NODE__" {
                        // Build field path from the expression
                        let mut field_path_vec = vec![];
                        let mut current_expr = &lhs_expr.expr;
                        while let Expr::MemberAccess(inner_base, inner_field) = current_expr {
                            field_path_vec.push(inner_field.clone());
                            current_expr = inner_base.as_ref();
                        }
                        field_path_vec.reverse();
                        
                        // Find all node types that have this field path
                        let compatible_node_types = ENGINE_REGISTRY.narrow_nodes_by_fields(&field_path_vec);
                        
                        if compatible_node_types.is_empty() {
                            // No compatible node types found, fallback to error
                            format!("        // ERROR: No compatible node types found for field path: {}\n", field_path)
                        } else {
                            // Generate RHS code once
                            let lhs_type = script.infer_expr_type(&lhs_expr.expr, current_func);
                            let rhs_type = script.infer_expr_type(&rhs_expr.expr, current_func);
                            
                            // Extract ALL API calls from RHS expression to avoid borrow checker issues
                            // API calls inside mutate_node closures need to be extracted before the closure
                            let mut extracted_api_calls = Vec::new();
                            let mut temp_counter = 0;
                            let mut temp_var_types: std::collections::HashMap<String, Type> = std::collections::HashMap::new();
                            
                            // Helper function to extract API calls from expressions
                            fn extract_api_calls_from_expr_helper(expr: &Expr, script: &Script, current_func: Option<&Function>, 
                                                       extracted: &mut Vec<(String, String)>, counter: &mut usize,
                                                       temp_var_types: &mut std::collections::HashMap<String, Type>,
                                                       needs_self: bool, expected_type: Option<&Type>) -> Expr {
                                match expr {
                                    // Extract API calls (like Math.random_range, Texture.load, etc.)
                                    Expr::ApiCall(api_module, api_args) => {
                                        // Extract ALL API calls, not just ones returning Uuid
                                        // This prevents borrow checker issues when API calls are inside closures
                                        let temp_var = format!("__temp_api_{}", counter);
                                        *counter += 1;
                                        
                                        // Generate the API call code
                                        let mut api_call_str = api_module.to_rust(api_args, script, needs_self, current_func);
                                        
                                        // Fix any incorrect renaming of "api" to "__t_api" or "t_id_api"
                                        api_call_str = api_call_str.replace("__t_api.", "api.").replace("t_id_api.", "api.");
                                        
                                        // Infer the return type for the temp variable
                                        let inferred_type = api_module.return_type();
                                        let type_annotation = inferred_type
                                            .as_ref()
                                            .map(|t| format!(": {}", t.to_rust_type()))
                                            .unwrap_or_default();
                                        
                                        // Store the type for this temp variable
                                        if let Some(ty) = inferred_type {
                                            temp_var_types.insert(temp_var.clone(), ty);
                                        }
                                        
                                        extracted.push((format!("let {}{} = {};", temp_var, type_annotation, api_call_str), temp_var.clone()));
                                        
                                        // Return an identifier expression for the temp variable
                                        Expr::Ident(temp_var)
                                    }
                                    Expr::MemberAccess(base, field) => {
                                        // Check if this member access would generate a read_node call
                                        if let Some((node_id, _, _, _)) = extract_node_member_info(expr, script, current_func) {
                                            // This is a node member access - extract it to a temp variable
                                            let temp_var = format!("__temp_read_{}", counter);
                                            *counter += 1;
                                            
                                            // Generate the read_node call
                                            let read_code = expr.to_rust(needs_self, script, expected_type, current_func, None);
                                            
                                            // Infer the type for the temp variable
                                            let inferred_type = script.infer_expr_type(expr, current_func);
                                            let type_annotation = inferred_type
                                                .as_ref()
                                                .map(|t| format!(": {}", t.to_rust_type()))
                                                .unwrap_or_default();
                                            
                                            // Store the type for this temp variable so we can check if it needs cloning
                                            if let Some(ty) = inferred_type {
                                                temp_var_types.insert(temp_var.clone(), ty);
                                            }
                                            
                                            extracted.push((format!("let {}{} = {};", temp_var, type_annotation, read_code), temp_var.clone()));
                                            
                                            // Return an identifier expression for the temp variable
                                            Expr::Ident(temp_var)
                                        } else {
                                            // Not a node member access, recurse into base
                                            let new_base = extract_api_calls_from_expr_helper(base, script, current_func, extracted, counter, temp_var_types, needs_self, None);
                                            Expr::MemberAccess(Box::new(new_base), field.clone())
                                        }
                                    }
                                    Expr::BinaryOp(left, op, right) => {
                                        let new_left = extract_api_calls_from_expr_helper(left, script, current_func, extracted, counter, temp_var_types, needs_self, None);
                                        let new_right = extract_api_calls_from_expr_helper(right, script, current_func, extracted, counter, temp_var_types, needs_self, None);
                                        Expr::BinaryOp(Box::new(new_left), op.clone(), Box::new(new_right))
                                    }
                                    Expr::Call(target, args) => {
                                        let new_target = extract_api_calls_from_expr_helper(target, script, current_func, extracted, counter, temp_var_types, needs_self, None);
                                        let new_args: Vec<Expr> = args.iter()
                                            .map(|arg| extract_api_calls_from_expr_helper(arg, script, current_func, extracted, counter, temp_var_types, needs_self, None))
                                            .collect();
                                        Expr::Call(Box::new(new_target), new_args)
                                    }
                                    _ => expr.clone(),
                                }
                            }
                            
                            let modified_rhs_expr = extract_api_calls_from_expr_helper(
                                &rhs_expr.expr, 
                                script, 
                                current_func, 
                                &mut extracted_api_calls, 
                                &mut temp_counter,
                                &mut temp_var_types,
                                needs_self,
                                lhs_type.as_ref()
                            );
                            
                            // Combine all temp declarations
                            let combined_temp_decl = if !extracted_api_calls.is_empty() {
                                Some(extracted_api_calls.iter().map(|(decl, _): &(String, String)| decl.clone()).collect::<Vec<_>>().join(" "))
                            } else {
                                None
                            };
                            
                            // Generate code for the (possibly modified) RHS expression
                            let rhs_code = modified_rhs_expr.to_rust(needs_self, script, lhs_type.as_ref(), current_func, rhs_expr.span.as_ref());
                            
                            let is_literal = matches!(rhs_expr.expr, Expr::Literal(_));
                            
                            // Apply implicit conversion if needed (especially important for temp variables)
                            let final_rhs = if let Some(lhs_ty) = &lhs_type {
                                if let Some(rhs_ty) = &rhs_type {
                                    if !is_literal && rhs_ty.can_implicitly_convert_to(lhs_ty) && rhs_ty != lhs_ty {
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
                            
                            // Check if the field is on the base Node type - if so, use mutate_scene_node
                            let first_field = field_path_vec.first().map(|s| s.as_str()).unwrap_or("");
                            let is_base_node_field = ENGINE_REGISTRY.get_field_type_node(&NodeType::Node, first_field).is_some();
                            
                            // If it's a single field on the base Node type, use mutate_scene_node
                            if is_base_node_field && field_path_vec.len() == 1 {
                                // Map field names to their setter methods from BaseNode trait
                                let setter_method = match first_field {
                                    "name" => Some("set_name"),
                                    "id" => Some("set_id"),
                                    "local_id" => Some("set_local_id"),
                                    "parent" => Some("set_parent"),
                                    "script_path" => Some("set_script_path"),
                                    // is_root_of doesn't have a setter, fall through to match statement
                                    _ => None,
                                };
                                
                                if let Some(setter) = setter_method {
                                    let temp_decl = combined_temp_decl.as_ref().map(|d| format!("        {}\n", d)).unwrap_or_default();
                                    format!(
                                        "{}        api.mutate_scene_node({}, |n| {{ n.{}({}); }});\n",
                                        temp_decl, node_id, setter, final_rhs
                                    )
                                } else {
                                    // Field doesn't have a setter, fall back to match statement approach
                                    // If only one compatible node type, skip match and do direct mutation
                                    if compatible_node_types.len() == 1 {
                                        let node_type_name = format!("{:?}", compatible_node_types[0]);
                                        // Resolve field names in path (e.g., "texture" -> "texture_id")
                                        let resolved_path: Vec<String> = field_path_vec.iter()
                                            .map(|f| ENGINE_REGISTRY.resolve_field_name(&compatible_node_types[0], f))
                                            .collect();
                                        let resolved_field_path = resolved_path.join(".");
                                        let temp_decl = combined_temp_decl.as_ref().map(|d| format!("        {}\n", d)).unwrap_or_default();
                                        format!(
                                            "{}        api.mutate_node({}, |{}: &mut {}| {{ {}.{} = {}; }});\n",
                                            temp_decl, node_id, closure_var, node_type_name, closure_var, resolved_field_path, final_rhs
                                        )
                                    } else {
                                        let mut match_arms = Vec::new();
                                        for node_type_enum in &compatible_node_types {
                                            let node_type_name = format!("{:?}", node_type_enum);
                                            // Resolve field names in path for this node type
                                            let resolved_path: Vec<String> = field_path_vec.iter()
                                                .map(|f| ENGINE_REGISTRY.resolve_field_name(node_type_enum, f))
                                                .collect();
                                            let resolved_field_path = resolved_path.join(".");
                                            match_arms.push(format!(
                                                "            NodeType::{} => api.mutate_node({}, |{}: &mut {}| {{ {}.{} = {}; }}),",
                                                node_type_name, node_id, closure_var, node_type_name, closure_var, resolved_field_path, final_rhs
                                            ));
                                        }
                                        
                                        let temp_decl = combined_temp_decl.as_ref().map(|d| format!("        {}\n", d)).unwrap_or_default();
                                        format!(
                                            "{}        match api.get_type({}) {{\n{}\n            _ => {{\n                let node_name = api.read_scene_node({}, |n| n.get_name().to_string());\n                let node_type = format!(\"{{:?}}\", api.get_type({}));\n                panic!(\"{{}} of type {{}} doesn't have field {{}}\", node_name, node_type, \"{}\");\n            }}\n        }}\n",
                                            temp_decl, node_id,
                                            match_arms.join("\n"),
                                            node_id,
                                            node_id,
                                            field_path
                                        )
                                    }
                                }
                            } else {
                                // Generate match arms for all compatible node types
                                // If only one compatible node type, skip match and do direct mutation
                                if compatible_node_types.len() == 1 {
                                    let node_type_name = format!("{:?}", compatible_node_types[0]);
                                    // Resolve field names in path
                                    let resolved_path: Vec<String> = field_path_vec.iter()
                                        .map(|f| ENGINE_REGISTRY.resolve_field_name(&compatible_node_types[0], f))
                                        .collect();
                                    let resolved_field_path = resolved_path.join(".");
                                    let temp_decl = combined_temp_decl.as_ref().map(|d| format!("        {}\n", d)).unwrap_or_default();
                                    format!(
                                        "{}        api.mutate_node({}, |{}: &mut {}| {{ {}.{} = {}; }});\n",
                                        temp_decl, node_id, closure_var, node_type_name, closure_var, resolved_field_path, final_rhs
                                    )
                                } else {
                                    let mut match_arms = Vec::new();
                                    for node_type_enum in &compatible_node_types {
                                        let node_type_name = format!("{:?}", node_type_enum);
                                        // Resolve field names in path for this node type
                                        let resolved_path: Vec<String> = field_path_vec.iter()
                                            .map(|f| ENGINE_REGISTRY.resolve_field_name(node_type_enum, f))
                                            .collect();
                                        let resolved_field_path = resolved_path.join(".");
                                        match_arms.push(format!(
                                            "            NodeType::{} => api.mutate_node({}, |{}: &mut {}| {{ {}.{} = {}; }}),",
                                            node_type_name, node_id, closure_var, node_type_name, closure_var, resolved_field_path, final_rhs
                                        ));
                                    }
                                    
                                    let temp_decl = combined_temp_decl.as_ref().map(|d| format!("        {}\n", d)).unwrap_or_default();
                                    format!(
                                        "{}        match api.get_type({}) {{\n{}\n            _ => {{\n                let node_name = api.read_scene_node({}, |n| n.get_name().to_string());\n                let node_type = format!(\"{{:?}}\", api.get_type({}));\n                panic!(\"{{}} of type {{}} doesn't have field {{}}\", node_name, node_type, \"{}\");\n            }}\n        }}\n",
                                        temp_decl, node_id,
                                        match_arms.join("\n"),
                                        node_id,
                                        node_id,
                                        field_path
                                    )
                                }
                            }
                        }
                    } else {
                        // This is a node member assignment - use mutate_node
                        let lhs_type = script.infer_expr_type(&lhs_expr.expr, current_func);
                        let rhs_type = script.infer_expr_type(&rhs_expr.expr, current_func);
                        
                        // Extract ALL API calls and read_node calls from RHS expression to avoid borrow checker issues
                        // API calls inside mutate_node closures need to be extracted before the closure
                        let mut extracted_api_calls = Vec::new();
                        let mut temp_counter = 0;
                        let mut temp_var_types: std::collections::HashMap<String, Type> = std::collections::HashMap::new();
                        
                        fn extract_api_calls_from_expr(expr: &Expr, script: &Script, current_func: Option<&Function>, 
                                                       extracted: &mut Vec<(String, String)>, counter: &mut usize,
                                                       temp_var_types: &mut std::collections::HashMap<String, Type>,
                                                       needs_self: bool, expected_type: Option<&Type>) -> Expr {
                            match expr {
                                // Extract API calls (like Math.random_range, Texture.load, etc.)
                                Expr::ApiCall(api_module, api_args) => {
                                    // Extract ALL API calls, not just ones returning Uuid
                                    // This prevents borrow checker issues when API calls are inside closures
                                    let temp_var = format!("__temp_api_{}", counter);
                                    *counter += 1;
                                    
                                    // Generate the API call code
                                    let mut api_call_str = api_module.to_rust(api_args, script, needs_self, current_func);
                                    
                                    // Fix any incorrect renaming of "api" to "__t_api" or "t_id_api"
                                    api_call_str = api_call_str.replace("__t_api.", "api.").replace("t_id_api.", "api.");
                                    
                                    // Infer the return type for the temp variable
                                    let inferred_type = api_module.return_type();
                                    let type_annotation = inferred_type
                                        .as_ref()
                                        .map(|t| format!(": {}", t.to_rust_type()))
                                        .unwrap_or_default();
                                    
                                    // Store the type for this temp variable
                                    if let Some(ty) = inferred_type {
                                        temp_var_types.insert(temp_var.clone(), ty);
                                    }
                                    
                                    extracted.push((format!("let {}{} = {};", temp_var, type_annotation, api_call_str), temp_var.clone()));
                                    
                                    // Return an identifier expression for the temp variable
                                    Expr::Ident(temp_var)
                                }
                                Expr::MemberAccess(base, field) => {
                                    // Check if this member access would generate a read_node call
                                    if let Some((node_id, _, _, _)) = extract_node_member_info(expr, script, current_func) {
                                        // This is a node member access - extract it to a temp variable
                                        let temp_var = format!("__temp_read_{}", counter);
                                        *counter += 1;
                                        
                                        // Generate the read_node call
                                        let read_code = expr.to_rust(needs_self, script, expected_type, current_func, None);
                                        
                                        // Infer the type for the temp variable
                                        let inferred_type = script.infer_expr_type(expr, current_func);
                                        let type_annotation = inferred_type
                                            .as_ref()
                                            .map(|t| format!(": {}", t.to_rust_type()))
                                            .unwrap_or_default();
                                        
                                        // Store the type for this temp variable so we can check if it needs cloning
                                        if let Some(ty) = inferred_type {
                                            temp_var_types.insert(temp_var.clone(), ty);
                                        }
                                        
                                        extracted.push((format!("let {}{} = {};", temp_var, type_annotation, read_code), temp_var.clone()));
                                        
                                        // Return an identifier expression for the temp variable
                                        Expr::Ident(temp_var)
                                    } else {
                                        // Not a node member access, recurse into base
                                        let new_base = extract_api_calls_from_expr(base, script, current_func, extracted, counter, temp_var_types, needs_self, None);
                                        Expr::MemberAccess(Box::new(new_base), field.clone())
                                    }
                                }
                                Expr::BinaryOp(left, op, right) => {
                                    let new_left = extract_api_calls_from_expr(left, script, current_func, extracted, counter, temp_var_types, needs_self, None);
                                    let new_right = extract_api_calls_from_expr(right, script, current_func, extracted, counter, temp_var_types, needs_self, None);
                                    Expr::BinaryOp(Box::new(new_left), op.clone(), Box::new(new_right))
                                }
                                Expr::Call(target, args) => {
                                    let new_target = extract_api_calls_from_expr(target, script, current_func, extracted, counter, temp_var_types, needs_self, None);
                                    let new_args: Vec<Expr> = args.iter()
                                        .map(|arg| extract_api_calls_from_expr(arg, script, current_func, extracted, counter, temp_var_types, needs_self, None))
                                        .collect();
                                    Expr::Call(Box::new(new_target), new_args)
                                }
                                _ => expr.clone(),
                            }
                        }
                        
                        // Extract API calls and read_node calls from RHS expression
                        let modified_rhs_expr = extract_api_calls_from_expr(
                            &rhs_expr.expr, 
                            script, 
                            current_func, 
                            &mut extracted_api_calls, 
                            &mut temp_counter,
                            &mut temp_var_types,
                            needs_self,
                            lhs_type.as_ref()
                        );
                        
                        // Combine all temp declarations from extracted API calls
                        let combined_temp_decl = if !extracted_api_calls.is_empty() {
                            Some(extracted_api_calls.iter().map(|(decl, _): &(String, String)| decl.clone()).collect::<Vec<_>>().join(" "))
                        } else {
                            None
                        };
                        
                        // Generate code for the (possibly modified) RHS expression
                        // If API calls were extracted, the modified expression uses temp variables
                        let rhs_code = modified_rhs_expr.to_rust(needs_self, script, lhs_type.as_ref(), current_func, rhs_expr.span.as_ref());
                        
                        // For literals, we already generated the code with the expected type,
                        // so skip implicit cast to avoid double conversion
                        let is_literal = matches!(rhs_expr.expr, Expr::Literal(_));
                        
                        // Apply implicit conversion if needed (especially important for temp variables)
                        let final_rhs = if let Some(lhs_ty) = &lhs_type {
                            if let Some(rhs_ty) = &rhs_type {
                                // For literals, if they were generated with the correct expected type,
                                // they should already be correct. Only apply cast if types don't match
                                // and it's not a literal (literals handle their own type conversion)
                                if !is_literal && rhs_ty.can_implicitly_convert_to(lhs_ty) && rhs_ty != lhs_ty {
                                    script.generate_implicit_cast_for_expr(&rhs_code, rhs_ty, lhs_ty)
                                } else if is_literal {
                                    // For literals, check if the generated code needs conversion
                                    // If lhs is Option<CowStr> but we got String::from, convert it
                                    if matches!(lhs_ty, Type::Option(inner) if matches!(inner.as_ref(), Type::CowStr))
                                        && rhs_code.contains("String::from(") {
                                        // Extract the literal from String::from("...") and convert to Some(Cow::Borrowed(...))
                                        let trimmed = rhs_code.trim();
                                        if trimmed.starts_with("String::from(") && trimmed.ends_with(')') {
                                            let inner_section = &trimmed["String::from(".len()..trimmed.len() - 1].trim();
                                            if inner_section.starts_with('"') && inner_section.ends_with('"') {
                                                format!("Some(Cow::Borrowed({}))", inner_section)
                                            } else {
                                                script.generate_implicit_cast_for_expr(&rhs_code, rhs_ty, lhs_ty)
                                            }
                                        } else {
                                            script.generate_implicit_cast_for_expr(&rhs_code, rhs_ty, lhs_ty)
                                        }
                                    } else {
                                        rhs_code
                                    }
                                } else {
                                    rhs_code
                                }
                            } else {
                                rhs_code
                            }
                        } else {
                            rhs_code
                        };
                        
                        // Resolve field names in path (e.g., "texture" -> "texture_id")
                        let resolved_field_path = if let Some(node_type_enum) = string_to_node_type(&node_type) {
                            let field_path_vec: Vec<&str> = field_path.split('.').collect();
                            let resolved_path: Vec<String> = field_path_vec.iter()
                                .map(|f| ENGINE_REGISTRY.resolve_field_name(&node_type_enum, f))
                                .collect();
                            resolved_path.join(".")
                        } else {
                            field_path.clone()
                        };
                        
                        let temp_decl = combined_temp_decl.as_ref().map(|d| format!("        {}\n", d)).unwrap_or_default();
                        format!(
                            "{}        api.mutate_node({}, |{}: &mut {}| {{ {}.{} = {}; }});\n",
                            temp_decl, node_id, closure_var, node_type, closure_var, resolved_field_path, final_rhs
                        )
                    }
                } else {
                    // Regular member assignment (not a node)
                    let lhs_code = lhs_expr.to_rust(needs_self, script, current_func);
                    // lhs_expr is TypedExpr, which already passes span through
                    let lhs_type = script.infer_expr_type(&lhs_expr.expr, current_func);
                    let rhs_type = script.infer_expr_type(&rhs_expr.expr, current_func);

                    let mut rhs_code =
                        rhs_expr
                            .expr
                            .to_rust(needs_self, script, lhs_type.as_ref(), current_func, rhs_expr.span.as_ref());

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
                        && rhs_type.as_ref().map_or(false, |ty| ty.requires_clone());

                    if should_clone {
                        format!("        {lhs_code} = {}.clone();\n", final_rhs)
                    } else {
                        format!("        {lhs_code} = {final_rhs};\n")
                    }
                }
            }

            Stmt::MemberAssignOp(lhs_expr, op, rhs_expr) => {
                // Check if this is a node member assignment (like self.transform.position.x += value)
                if let Some((node_id, node_type, field_path, closure_var)) = 
                    extract_node_member_info(&lhs_expr.expr, script, current_func) 
                {
                    // Check if this is a DynNode (special marker)
                    if node_type == "__DYN_NODE__" {
                        // Build field path from the expression
                        let mut field_path_vec = vec![];
                        let mut current_expr = &lhs_expr.expr;
                        while let Expr::MemberAccess(inner_base, inner_field) = current_expr {
                            field_path_vec.push(inner_field.clone());
                            current_expr = inner_base.as_ref();
                        }
                        field_path_vec.reverse();
                        
                        // Find all node types that have this field path
                        let compatible_node_types = ENGINE_REGISTRY.narrow_nodes_by_fields(&field_path_vec);
                        
                        if compatible_node_types.is_empty() {
                            // No compatible node types found, fallback to error
                            format!("        // ERROR: No compatible node types found for field path: {}\n", field_path)
                        } else {
                            // Generate match arms for all compatible node types
                            let lhs_type = script.infer_expr_type(&lhs_expr.expr, current_func);
                            
                            let mut rhs_code =
                                rhs_expr
                                    .expr
                                    .to_rust(needs_self, script, lhs_type.as_ref(), current_func, rhs_expr.span.as_ref());
                            
                            if matches!(op, Op::Add) && lhs_type == Some(Type::String) {
                                // If only one compatible node type, skip match and do direct mutation
                                if compatible_node_types.len() == 1 {
                                    let node_type_name = format!("{:?}", compatible_node_types[0]);
                                    // Resolve field names in path
                                    let resolved_path: Vec<String> = field_path_vec.iter()
                                        .map(|f| ENGINE_REGISTRY.resolve_field_name(&compatible_node_types[0], f))
                                        .collect();
                                    let resolved_field_path = resolved_path.join(".");
                                    return format!(
                                        "        api.mutate_node({}, |{}: &mut {}| {{ {}.{}.push_str({}.as_str()); }});\n",
                                        node_id, closure_var, node_type_name, closure_var, resolved_field_path, rhs_code
                                    );
                                } else {
                                    let mut match_arms = Vec::new();
                                    for node_type_enum in &compatible_node_types {
                                        let node_type_name = format!("{:?}", node_type_enum);
                                        // Resolve field names in path for this node type
                                        let resolved_path: Vec<String> = field_path_vec.iter()
                                            .map(|f| ENGINE_REGISTRY.resolve_field_name(node_type_enum, f))
                                            .collect();
                                        let resolved_field_path = resolved_path.join(".");
                                        match_arms.push(format!(
                                            "            NodeType::{} => api.mutate_node({}, |{}: &mut {}| {{ {}.{}.push_str({}.as_str()); }}),",
                                            node_type_name, node_id, closure_var, node_type_name, closure_var, resolved_field_path, rhs_code
                                        ));
                                    }
                                    return format!(
                                        "        match api.get_node_type({}) {{\n{}\n            _ => {{\n                let node_name = api.read_scene_node({}, |n| n.get_name().to_string());\n                let node_type = format!(\"{{:?}}\", api.get_node_type({}));\n                panic!(\"{{}} of type {{}} doesn't have field {{}}\", node_name, node_type, \"{}\");\n            }}\n        }}\n",
                                        node_id,
                                        match_arms.join("\n"),
                                        node_id,
                                        node_id,
                                        field_path
                                    );
                                }
                            }
                            
                            let final_rhs = if let Some(lhs_ty) = &lhs_type {
                                let rhs_ty = script.infer_expr_type(&rhs_expr.expr, current_func);
                                if let Some(rhs_ty) = &rhs_ty {
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
                            
                            // If only one compatible node type, skip match and do direct mutation
                            if compatible_node_types.len() == 1 {
                                let node_type_name = format!("{:?}", compatible_node_types[0]);
                                // Resolve field names in path
                                let resolved_path: Vec<String> = field_path_vec.iter()
                                    .map(|f| ENGINE_REGISTRY.resolve_field_name(&compatible_node_types[0], f))
                                    .collect();
                                let resolved_field_path = resolved_path.join(".");
                                format!(
                                    "        api.mutate_node({}, |{}: &mut {}| {{ {}.{} {}= {}; }});\n",
                                    node_id, closure_var, node_type_name, closure_var, resolved_field_path, op.to_rust_assign(), final_rhs
                                )
                            } else {
                                let mut match_arms = Vec::new();
                                for node_type_enum in &compatible_node_types {
                                    let node_type_name = format!("{:?}", node_type_enum);
                                    // Resolve field names in path for this node type
                                    let resolved_path: Vec<String> = field_path_vec.iter()
                                        .map(|f| ENGINE_REGISTRY.resolve_field_name(node_type_enum, f))
                                        .collect();
                                    let resolved_field_path = resolved_path.join(".");
                                    match_arms.push(format!(
                                        "            NodeType::{} => api.mutate_node({}, |{}: &mut {}| {{ {}.{} {}= {}; }}),",
                                        node_type_name, node_id, closure_var, node_type_name, closure_var, resolved_field_path, op.to_rust_assign(), final_rhs
                                    ));
                                }
                                
                                format!(
                                    "        match api.get_type({}) {{\n{}\n            _ => {{\n                let node_name = api.read_scene_node({}, |n| n.get_name().to_string());\n                let node_type = format!(\"{{:?}}\", api.get_type({}));\n                panic!(\"{{}} of type {{}} doesn't have field {{}}\", node_name, node_type, \"{}\");\n            }}\n        }}\n",
                                    node_id,
                                    match_arms.join("\n"),
                                    node_id,
                                    node_id,
                                    field_path
                                )
                            }
                        }
                    } else {
                        // This is a node member assignment - use mutate_node
                        let lhs_type = script.infer_expr_type(&lhs_expr.expr, current_func);
                        
                        let mut rhs_code =
                            rhs_expr
                                .expr
                                .to_rust(needs_self, script, lhs_type.as_ref(), current_func, rhs_expr.span.as_ref());
                        
                        // Resolve field names in path (e.g., "texture" -> "texture_id")
                        let resolved_field_path = if let Some(node_type_enum) = string_to_node_type(&node_type) {
                            let field_path_vec: Vec<&str> = field_path.split('.').collect();
                            let resolved_path: Vec<String> = field_path_vec.iter()
                                .map(|f| ENGINE_REGISTRY.resolve_field_name(&node_type_enum, f))
                                .collect();
                            resolved_path.join(".")
                        } else {
                            field_path.clone()
                        };
                        
                        if matches!(op, Op::Add) && lhs_type == Some(Type::String) {
                            return format!(
                                "        api.mutate_node({}, |{}: &mut {}| {{ {}.{}.push_str({}.as_str()); }});\n",
                                node_id, closure_var, node_type, closure_var, resolved_field_path, rhs_code
                            );
                        }
                        
                        let final_rhs = if let Some(lhs_ty) = &lhs_type {
                            let rhs_ty = script.infer_expr_type(&rhs_expr.expr, current_func);
                            if let Some(rhs_ty) = &rhs_ty {
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
                        
                        format!(
                            "        api.mutate_node({}, |{}: &mut {}| {{ {}.{} {}= {}; }});\n",
                            node_id, closure_var, node_type, closure_var, resolved_field_path, op.to_rust_assign(), final_rhs
                        )
                    }
                } else {
                    // Regular member assignment (not a node)
                    let lhs_code = lhs_expr.to_rust(needs_self, script, current_func);
                    // lhs_expr is TypedExpr, which already passes span through
                    let lhs_type = script.infer_expr_type(&lhs_expr.expr, current_func);

                    let mut rhs_code =
                        rhs_expr
                            .expr
                            .to_rust(needs_self, script, lhs_type.as_ref(), current_func, rhs_expr.span.as_ref());

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

                    format!(
                        "        {lhs_code} {}= {};\n",
                        op.to_rust_assign(),
                        final_rhs
                    )
                }
            }

            Stmt::Pass => String::new(),

            Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                let cond_str = condition.to_rust(needs_self, script, current_func);
                // condition is TypedExpr, which already passes span through
                let mut result = format!("        if {} {{\n", cond_str);

                for stmt in then_body {
                    let stmt_str = stmt.to_rust(needs_self, script, current_func);
                    // stmt is Stmt, which handles spans internally
                    // Add extra indentation for statements inside the block
                    let indented = stmt_str
                        .lines()
                        .map(|line| {
                            if line.trim().is_empty() {
                                String::new()
                            } else {
                                format!("    {}", line)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    result.push_str(&indented);
                    if !indented.ends_with('\n') {
                        result.push('\n');
                    }
                }

                result.push_str("        }");

                if let Some(else_body) = else_body {
                    result.push_str(" else {\n");
                    for stmt in else_body {
                        let stmt_str = stmt.to_rust(needs_self, script, current_func);
                    // stmt is Stmt, which handles spans internally
                        // Add extra indentation for statements inside the block
                        let indented = stmt_str
                            .lines()
                            .map(|line| {
                                if line.trim().is_empty() {
                                    String::new()
                                } else {
                                    format!("    {}", line)
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        result.push_str(&indented);
                        if !indented.ends_with('\n') {
                            result.push('\n');
                        }
                    }
                    result.push_str("        }\n");
                } else {
                    result.push_str("\n");
                }

                result
            }

            Stmt::For {
                var_name,
                iterable,
                body,
            } => {
                // Check if loop body creates any nodes that need to be merged
                let mut loop_node_vars = Vec::new();
                let mut loop_ui_elements = Vec::new();
                collect_cloned_node_vars(body, &mut loop_node_vars, &mut loop_ui_elements, script);
                
                let iter_str = iterable.to_rust(needs_self, script, current_func);
                // iterable is TypedExpr, which already passes span through
                let mut result = String::new();
                
                // Use TRANSPILED_IDENT prefix for loop variable (e.g., i -> __t_i)
                let loop_var_name = format!("{}{}", TRANSPILED_IDENT, var_name);
                result.push_str(&format!("        for {} in {} {{\n", loop_var_name, iter_str));

                // Track which nodes are created/modified in this iteration
                let mut nodes_created_this_iter = Vec::new();
                
                for stmt in body {
                    let stmt_str = stmt.to_rust(needs_self, script, current_func);
                    // stmt is Stmt, which handles spans internally
                    // Add extra indentation for statements inside the block
                    let indented = stmt_str
                        .lines()
                        .map(|line| {
                            if line.trim().is_empty() {
                                String::new()
                            } else {
                                format!("    {}", line)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    result.push_str(&indented);
                    if !indented.ends_with('\n') {
                        result.push('\n');
                    }
                    
                    // Track nodes created in this iteration (but don't push yet - wait until after modifications)
                    match stmt {
                        Stmt::VariableDecl(var) => {
                            if loop_node_vars.contains(&var.name) {
                                nodes_created_this_iter.push(var.name.clone());
                            }
                        }
                        Stmt::Assign(name, _) => {
                            if loop_node_vars.contains(name) && !nodes_created_this_iter.contains(name) {
                                nodes_created_this_iter.push(name.clone());
                            }
                        }
                        _ => {}
                    }
                }
                
                // No longer need to track nodes for merging - we use mutate_node for assignments

                result.push_str("        }\n");
                result
            }

            Stmt::ForTraditional {
                init,
                condition,
                increment,
                body,
            } => {
                // Check if loop body creates any nodes that need to be merged
                let mut loop_node_vars = Vec::new();
                let mut loop_ui_elements = Vec::new();
                collect_cloned_node_vars(body, &mut loop_node_vars, &mut loop_ui_elements, script);
                
                let mut result = String::new();

                // Init - declare variable before the loop if it's a VariableDecl
                if let Some(init_stmt) = init {
                    match init_stmt.as_ref() {
                        Stmt::VariableDecl(var) => {
                            // Default to f32 if type is not inferred (common for loop counters)
                            let var_type = if var.typ.is_none() {
                                "f32".to_string()
                            } else {
                                var.rust_type()
                            };
                            let init_val = if var.value.is_none() {
                                "0.0".to_string()
                            } else {
                                var.rust_initialization(script, current_func)
                            };
                            result.push_str(&format!(
                                "        let mut {}: {} = {};\n",
                                var.name, var_type, init_val
                            ));
                        }
                        Stmt::Assign(name, expr) => {
                            let expr_str = expr.to_rust(needs_self, script, current_func);
                // expr is TypedExpr, which already passes span through
                            result.push_str(&format!("        let mut {} = {};\n", name, expr_str));
                        }
                        _ => {
                            // For other init statements, just generate the code
                            let init_code = init_stmt.to_rust(needs_self, script, current_func);
                            // init_stmt is Stmt, which handles spans internally
                            result.push_str(&format!(
                                "        {}\n",
                                init_code.trim().trim_end_matches(';')
                            ));
                        }
                    }
                }

                // Convert to while loop since Rust doesn't support C-style for loops
                result.push_str("        while ");

                // Condition
                if let Some(cond) = condition {
                    let cond_str = cond.to_rust(needs_self, script, current_func);
                    result.push_str(&cond_str);
                } else {
                    result.push_str("true");
                }
                result.push_str(" {\n");

                // Track which nodes are created/modified in this iteration
                let mut nodes_created_this_iter = Vec::new();

                // Body
                for stmt in body {
                    let stmt_str = stmt.to_rust(needs_self, script, current_func);
                    // stmt is Stmt, which handles spans internally
                    // Add extra indentation for statements inside the block
                    let indented = stmt_str
                        .lines()
                        .map(|line| {
                            if line.trim().is_empty() {
                                String::new()
                            } else {
                                format!("            {}", line)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    result.push_str(&indented);
                    if !indented.ends_with('\n') {
                        result.push('\n');
                    }
                    
                    // Track nodes created in this iteration (but don't push yet - wait until after modifications)
                    match stmt {
                        Stmt::VariableDecl(var) => {
                            if loop_node_vars.contains(&var.name) {
                                nodes_created_this_iter.push(var.name.clone());
                            }
                        }
                        Stmt::Assign(name, _) => {
                            if loop_node_vars.contains(name) && !nodes_created_this_iter.contains(name) {
                                nodes_created_this_iter.push(name.clone());
                            }
                        }
                        _ => {}
                    }
                }
                
                // Push all nodes created/modified in this iteration AFTER all statements (so modifications are captured)
                if !loop_node_vars.is_empty() && !nodes_created_this_iter.is_empty() {
                    for node_var in &nodes_created_this_iter {
                        // No longer need to track nodes for merging - we use mutate_node for assignments
                    }
                }

                // Increment at the end of the loop body
                if let Some(incr_stmt) = increment {
                    match incr_stmt.as_ref() {
                        Stmt::AssignOp(name, op, expr) => {
                            // Rust doesn't have ++ or --, so use += and -=
                            let op_str = match op {
                                Op::Add => "+=",
                                Op::Sub => "-=",
                                Op::Mul => "*=",
                                Op::Div => "/=",
                                Op::Lt | Op::Gt | Op::Le | Op::Ge | Op::Eq | Op::Ne => {
                                    unreachable!(
                                        "Comparison operators cannot be used in assignment operations"
                                    )
                                }
                            };
                            let expr_str = expr.to_rust(needs_self, script, current_func);
                // expr is TypedExpr, which already passes span through
                            result.push_str(&format!(
                                "            {} {} {};\n",
                                name, op_str, expr_str
                            ));
                        }
                        Stmt::Assign(name, expr) => {
                            let expr_str = expr.to_rust(needs_self, script, current_func);
                // expr is TypedExpr, which already passes span through
                            result.push_str(&format!("            {} = {};\n", name, expr_str));
                        }
                        _ => {
                            let incr_code = incr_stmt.to_rust(needs_self, script, current_func);
                            // incr_stmt is Stmt, which handles spans internally
                            result.push_str(&format!(
                                "            {}\n",
                                incr_code.trim().trim_end_matches(';')
                            ));
                        }
                    }
                }

                result.push_str("        }\n");
                result
            }

            Stmt::ScriptAssign(var, field, rhs) => {
                let rhs_str = rhs.to_rust(needs_self, script, current_func);
                // rhs is TypedExpr, which already passes span through

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
                // rhs is TypedExpr, which already passes span through
                let op_str = match op {
                    Op::Add => "Add",
                    Op::Sub => "Sub",
                    Op::Mul => "Mul",
                    Op::Div => "Div",
                    Op::Lt | Op::Gt | Op::Le | Op::Ge | Op::Eq | Op::Ne => {
                        unreachable!("Comparison operators cannot be used in assignment operations")
                    }
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
                let lhs_type = script.infer_expr_type(&array_expr, current_func);
                let rhs_type = script.infer_expr_type(&rhs_expr.expr, current_func);
                let base_code = array_expr.to_rust(needs_self, script, None, current_func, None);

                // Check if this is a map (HashMap) vs array (Vec)
                let is_map = matches!(lhs_type, Some(Type::Container(ContainerKind::Map, _)));

                let (index_code, is_dynamic_array) = if is_map {
                    // For maps, use string key handling
                    // For assignment, we need String (not &str) for .insert()
                    let key_ty =
                        if let Some(Type::Container(ContainerKind::Map, inner_types)) = &lhs_type {
                            inner_types.get(0).unwrap_or(&Type::String)
                        } else {
                            &Type::String
                        };
                    let key_code_raw =
                        index_expr.to_rust(needs_self, script, Some(key_ty), current_func, None);
                    let key_type = script.infer_expr_type(index_expr, current_func);
                    let final_key_code = if *key_ty == Type::String {
                        // For String keys, convert the key to string if it's not already
                        // For assignment, we need String (not &str), so don't add .as_str()
                        if matches!(key_type, Some(Type::Number(_)) | Some(Type::Bool)) {
                            format!("{}.to_string()", key_code_raw)
                        } else if key_code_raw.starts_with("String::from") {
                            key_code_raw
                        } else {
                            format!("{}.to_string()", key_code_raw)
                        }
                    } else {
                        // For non-string keys, use reference
                        format!("&{}", key_code_raw)
                    };
                    (final_key_code, false)
                } else {
                    // For arrays, ensure index is usize
                    let index_code_raw = index_expr.to_rust(
                        needs_self,
                        script,
                        Some(&Type::Number(NumberKind::Unsigned(32))),
                        current_func,
                        None, // index_expr is Expr, no span available
                    );
                    let index_code = format!("{} as usize", index_code_raw);

                    // Check if this is a dynamic array (Vec<Value>) that needs special handling
                    let is_dynamic_array =
                        if let Some(Type::Container(ContainerKind::Array, inner_types)) = &lhs_type
                        {
                            inner_types.get(0).map_or(true, |t| {
                                *t == Type::Object || matches!(t, Type::Custom(_))
                            })
                        } else {
                            false
                        };
                    (index_code, is_dynamic_array)
                };

                if is_map {
                    // Handle map assignment
                    let inner_types =
                        if let Some(Type::Container(ContainerKind::Map, inner_types)) = &lhs_type {
                            inner_types
                        } else {
                            &vec![]
                        };
                    let value_ty = inner_types.get(1).unwrap_or(&Type::Object);
                    let is_dynamic_map = value_ty == &Type::Object;

                    let mut rhs_code =
                        rhs_expr
                            .expr
                            .to_rust(needs_self, script, Some(value_ty), current_func, None);

                    // Insert implicit conversion if needed
                    let final_rhs = if let Some(rhs_ty) = &rhs_type {
                        if rhs_ty.can_implicitly_convert_to(value_ty) && rhs_ty != value_ty {
                            script.generate_implicit_cast_for_expr(&rhs_code, rhs_ty, value_ty)
                        } else {
                            rhs_code
                        }
                    } else {
                        rhs_code
                    };

                    // For dynamic maps, wrap the value in json!()
                    let final_rhs_wrapped = if is_dynamic_map {
                        // Check if it's already wrapped in json!() or is a Value
                        if final_rhs.starts_with("json!") || final_rhs.contains("Value") {
                            final_rhs
                        } else {
                            format!("json!({})", final_rhs)
                        }
                    } else {
                        final_rhs
                    };

                    // Maps use .insert() for assignment
                    // index_code is already a String for string keys, or a reference for other key types
                    format!(
                        "        {}.insert({}, {});\n",
                        base_code, index_code, final_rhs_wrapped
                    )
                } else {
                    // Handle array assignment
                    let mut rhs_code =
                        rhs_expr
                            .expr
                            .to_rust(needs_self, script, lhs_type.as_ref(), current_func, rhs_expr.span.as_ref());

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

                    // For dynamic arrays, wrap the value in json!()
                    let final_rhs_wrapped = if is_dynamic_array {
                        // Check if it's already wrapped in json!() or is a Value
                        if final_rhs.starts_with("json!") || final_rhs.contains("Value") {
                            final_rhs
                        } else {
                            format!("json!({})", final_rhs)
                        }
                    } else {
                        final_rhs
                    };

                    // Check if index expression references the same array being indexed
                    // This causes a borrow checker error: cannot borrow as immutable and mutable
                    let index_refs_array = index_code.contains(&base_code);

                    // If the index references the array, extract it to a temporary variable first
                    if index_refs_array {
                        // Generate a temporary variable name based on the array name
                        // Extract the variable name from base_code (e.g., "self.array" -> "array")
                        let temp_index_var = if base_code.starts_with("self.") {
                            let var_name = base_code.strip_prefix("self.").unwrap_or(&base_code);
                            format!("__{}_idx", var_name.replace(".", "_"))
                        } else {
                            format!("__{}_idx", base_code.replace(".", "_"))
                        };
                        let bounds_check = if is_dynamic_array {
                            format!(
                                "        if {}.len() <= {} {{\n            {}.resize({} + 1, json!(null));\n        }}\n",
                                base_code, temp_index_var, base_code, temp_index_var
                            )
                        } else {
                            String::new()
                        };
                        format!(
                            "        let {} = {};\n{}{}[{}] = {};\n",
                            temp_index_var,
                            index_code,
                            bounds_check,
                            base_code,
                            temp_index_var,
                            final_rhs_wrapped
                        )
                    } else {
                        // Insert `.clone()` if needed, matching your member assign arm
                        let should_clone = !is_dynamic_array
                            && matches!(rhs_expr.expr, Expr::Ident(_) | Expr::MemberAccess(..))
                            && rhs_type.as_ref().map_or(false, |ty| ty.requires_clone());

                        if is_dynamic_array {
                            // For dynamic arrays, extract index and check bounds
                            let temp_index_var = format!(
                                "__idx_{}",
                                base_code.replace(".", "_").replace("self", "")
                            );
                            format!(
                                "        let {} = {};\n        if {}.len() <= {} {{\n            {}.resize({} + 1, json!(null));\n        }}\n        {}[{}] = {};\n",
                                temp_index_var,
                                index_code,
                                base_code,
                                temp_index_var,
                                base_code,
                                temp_index_var,
                                base_code,
                                temp_index_var,
                                final_rhs_wrapped
                            )
                        } else if should_clone {
                            format!(
                                "        {}[{}] = {}.clone();\n",
                                base_code, index_code, final_rhs_wrapped
                            )
                        } else {
                            format!(
                                "        {}[{}] = {};\n",
                                base_code, index_code, final_rhs_wrapped
                            )
                        }
                    }
                }
            }

            Stmt::IndexAssignOp(array_expr, index_expr, op, rhs_expr) => {
                let array_code = array_expr.to_rust(needs_self, script, None, current_func, None);
                // Ensure index is usize for array indexing
                let index_code_raw = index_expr.to_rust(
                    needs_self,
                    script,
                    Some(&Type::Number(NumberKind::Unsigned(32))),
                    current_func,
                    None, // index_expr is Expr, no span available
                );
                let index_code = format!("{} as usize", index_code_raw);

                let lhs_type = script.infer_expr_type(&array_expr, current_func);
                let rhs_type = script.infer_expr_type(&rhs_expr.expr, current_func);

                // Check if this is a dynamic array (Vec<Value>) that needs special handling
                let is_dynamic_array =
                    if let Some(Type::Container(ContainerKind::Array, inner_types)) = &lhs_type {
                        inner_types
                            .get(0)
                            .map_or(true, |t| *t == Type::Object || matches!(t, Type::Custom(_)))
                    } else {
                        false
                    };

                if is_dynamic_array {
                    // For dynamic arrays stored as Vec<Value>, operations need explicit casting
                    // This is a limitation - the user should cast the element first
                    format!(
                        "        // TODO: Dynamic array compound assignment - cast element to type first, do operation, then assign back as json!()\n"
                    )
                } else {
                    let mut rhs_code =
                        rhs_expr
                            .expr
                            .to_rust(needs_self, script, lhs_type.as_ref(), current_func, rhs_expr.span.as_ref());

                    // Special case: string += something becomes push_str.
                    if matches!(op, Op::Add) && lhs_type == Some(Type::String) {
                        return format!(
                            "        {}[{}].push_str({}.as_str());\n",
                            array_code, index_code, rhs_code
                        );
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

                    format!(
                        "        {}[{}] {}= {};\n",
                        array_code,
                        index_code,
                        op.to_rust_assign(),
                        final_rhs
                    )
                }
            }
        }
    }

    fn generate_implicit_cast(expr: &str, from_type: &Type, to_type: &Type) -> String {
        use NumberKind::*;
        use Type::*;

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
            (Number(Signed(_) | Unsigned(_)), Number(Decimal)) => {
                format!("Decimal::from({})", expr)
            }

            // String type conversions
            (String, CowStr) => {
                format!("{}.into()", expr)
            }
            (StrRef, CowStr) => {
                format!("{}.into()", expr)
            }
            (CowStr, String) => {
                format!("{}.into_owned()", expr)
            }
            (CowStr, StrRef) => {
                format!("{}.as_ref()", expr)
            }
            // String/StrRef/CowStr -> Option<CowStr> conversions
            (String, Option(inner)) if matches!(inner.as_ref(), CowStr) => {
                // Check if expr is a string literal (direct or wrapped in String::from)
                let trimmed = expr.trim();
                if trimmed.starts_with('"') && trimmed.ends_with('"') {
                    // Direct string literal: "..." -> Some(Cow::Borrowed("..."))
                    format!("Some(Cow::Borrowed({}))", expr)
                } else if trimmed.starts_with("String::from(") && trimmed.ends_with(')') {
                    // String::from("...") -> extract literal and use Cow::Borrowed
                    let inner_section = &trimmed["String::from(".len()..trimmed.len() - 1].trim();
                    if inner_section.starts_with('"') && inner_section.ends_with('"') {
                        format!("Some(Cow::Borrowed({}))", inner_section)
                    } else {
                        format!("Some({}.into())", expr)
                    }
                } else {
                    // Variable or other expression: use .into()
                    format!("Some({}.into())", expr)
                }
            }
            (StrRef, Option(inner)) if matches!(inner.as_ref(), CowStr) => {
                // StrRef is already &'static str, so for literals use Cow::Borrowed
                let trimmed = expr.trim();
                if trimmed.starts_with('"') && trimmed.ends_with('"') {
                    format!("Some(Cow::Borrowed({}))", expr)
                } else {
                    format!("Some({}.into())", expr)
                }
            }
            (CowStr, Option(inner)) if matches!(inner.as_ref(), CowStr) => {
                format!("Some({})", expr)
            }
            // Option unwrapping: Option<T> -> T (when assigning to non-Option field)
            (Option(inner_from), to) if inner_from.as_ref() == to => {
                format!("{}.unwrap_or_default()", expr)
            }
            // Wrapping: T -> Option<T> (when assigning T to Option<T> field)
            (from, Option(inner_to)) if from == inner_to.as_ref() => {
                format!("Some({})", expr)
            }
            // Option conversion: Option<From> -> Option<To>
            (Option(inner_from), Option(inner_to)) => {
                // Convert the inner type first
                let inner_expr = format!("{}", expr);
                let inner_from_ty = inner_from.as_ref();
                let inner_to_ty = inner_to.as_ref();
                
                // Handle the inner conversion
                let converted_inner = match (inner_from_ty, inner_to_ty) {
                    (String, CowStr) => {
                        // Check if expr is a string literal
                        let trimmed = inner_expr.trim();
                        if trimmed.starts_with('"') && trimmed.ends_with('"') {
                            format!("Cow::Borrowed({})", inner_expr)
                        } else if trimmed.starts_with("String::from(") && trimmed.ends_with(')') {
                            let inner_section = &trimmed["String::from(".len()..trimmed.len() - 1].trim();
                            if inner_section.starts_with('"') && inner_section.ends_with('"') {
                                format!("Cow::Borrowed({})", inner_section)
                            } else {
                                format!("{}.into()", inner_expr)
                            }
                        } else {
                            format!("{}.into()", inner_expr)
                        }
                    }
                    (StrRef, CowStr) => {
                        let trimmed = inner_expr.trim();
                        if trimmed.starts_with('"') && trimmed.ends_with('"') {
                            format!("Cow::Borrowed({})", inner_expr)
                        } else {
                            format!("{}.into()", inner_expr)
                        }
                    }
                    (CowStr, CowStr) => inner_expr,
                    (CowStr, String) => format!("{}.into_owned()", inner_expr),
                    (CowStr, StrRef) => format!("{}.as_ref()", inner_expr),
                    _ if inner_from_ty == inner_to_ty => inner_expr,
                    _ => {
                        // For other conversions, recursively call generate_implicit_cast
                        // Note: This is a standalone function, so we call it directly
                        Self::generate_implicit_cast(&inner_expr, inner_from_ty, inner_to_ty)
                    }
                };
                
                // Wrap in Some() if not already wrapped
                if converted_inner.starts_with("Some(") {
                    converted_inner
                } else {
                    format!("Some({})", converted_inner)
                }
            }
            // Node types -> Uuid (nodes are Uuid IDs)
            (Node(_), Uuid) => {
                expr.to_string() // Already a Uuid, no conversion needed
            }
            // Uuid -> Node type (for type checking, just pass through)
            (Uuid, Node(_)) => {
                expr.to_string() // Already a Uuid, no conversion needed
            }

            _ => {
                eprintln!(
                    "Warning: Unhandled cast from {:?} to {:?}",
                    from_type, to_type
                );
                expr.to_string()
            }
        }
    }

    fn get_target_type(
        &self,
        name: &str,
        script: &Script,
        current_func: Option<&Function>,
    ) -> Option<Type> {
        if let Some(func) = current_func {
            if let Some(local) = func.locals.iter().find(|v| v.name == name) {
                // First try explicit type
                if let Some(typ) = &local.typ {
                    return Some(typ.clone());
                }
                // If no explicit type, try to infer from value
                if let Some(value) = &local.value {
                    return script.infer_expr_type(&value.expr, current_func);
                }
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
            Stmt::ScriptAssign(_, _, expr) | Stmt::ScriptAssignOp(_, _, _, expr) => {
                expr.contains_self()
            }
            Stmt::IndexAssign(array, index, value)
            | Stmt::IndexAssignOp(array, index, _, value) => {
                array.contains_self() || index.contains_self() || value.contains_self()
            }
            Stmt::Pass => false,
            Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                condition.contains_self()
                    || then_body.iter().any(|s| s.contains_self())
                    || else_body
                        .as_ref()
                        .map_or(false, |body| body.iter().any(|s| s.contains_self()))
            }
            Stmt::For { iterable, body, .. } => {
                iterable.contains_self() || body.iter().any(|s| s.contains_self())
            }
            Stmt::ForTraditional {
                init,
                condition,
                increment,
                body,
            } => {
                (init.as_ref().map_or(false, |s| s.as_ref().contains_self()))
                    || (condition.as_ref().map_or(false, |c| c.contains_self()))
                    || (increment
                        .as_ref()
                        .map_or(false, |s| s.as_ref().contains_self()))
                    || body.iter().any(|s| s.contains_self())
            }
        }
    }

    pub fn contains_api_call(&self, script: &Script) -> bool {
        match self {
            Stmt::Expr(e) => e.contains_api_call(script),
            Stmt::VariableDecl(v) => v
                .value
                .as_ref()
                .map_or(false, |e| e.contains_api_call(script)),
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
            Stmt::ScriptAssign(_, _, e) | Stmt::ScriptAssignOp(_, _, _, e) => {
                e.contains_api_call(script)
            }
            Stmt::Pass => false,
            Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                condition.contains_api_call(script)
                    || then_body.iter().any(|s| s.contains_api_call(script))
                    || else_body.as_ref().map_or(false, |body| {
                        body.iter().any(|s| s.contains_api_call(script))
                    })
            }
            Stmt::For { iterable, body, .. } => {
                iterable.contains_api_call(script)
                    || body.iter().any(|s| s.contains_api_call(script))
            }
            Stmt::ForTraditional {
                init,
                condition,
                increment,
                body,
            } => {
                (init
                    .as_ref()
                    .map_or(false, |s| s.as_ref().contains_api_call(script)))
                    || (condition
                        .as_ref()
                        .map_or(false, |c| c.contains_api_call(script)))
                    || (increment
                        .as_ref()
                        .map_or(false, |s| s.as_ref().contains_api_call(script)))
                    || body.iter().any(|s| s.contains_api_call(script))
            }
        }
    }
}

impl TypedExpr {
    pub fn to_rust(
        &self,
        needs_self: bool,
        script: &Script,
        current_func: Option<&Function>,
    ) -> String {
        let type_hint = self.inferred_type.as_ref();
        let source_span = self.span.as_ref();
        self.expr
            .to_rust(needs_self, script, type_hint, current_func, source_span)
    }

    pub fn contains_self(&self) -> bool {
        match &self.expr {
            Expr::Range(start, end) => start.contains_self() || end.contains_self(),
            _ => self.expr.contains_self(),
        }
    }

    pub fn contains_api_call(&self, script: &Script) -> bool {
        match &self.expr {
            Expr::Range(start, end) => {
                start.contains_api_call(script) || end.contains_api_call(script)
            }
            _ => self.expr.contains_api_call(script),
        }
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

    fn should_clone_expr(
        expr_code: &str,
        expr: &Expr,
        script: &Script,
        current_func: Option<&Function>,
    ) -> bool {
        if expr_code.starts_with("json!(")
            || expr_code.starts_with("HashMap::from(")
            || expr_code.starts_with("vec![")
            || expr_code.contains("serde_json::from_value::<")
            || expr_code.contains(".parse::<")
            || expr_code.contains(".unwrap()")  // unwrap() produces owned value
            || expr_code.contains('{')
        // struct literal produces an owned value
        {
            return false;
        }

        match expr {
            Expr::Ident(name) => {
                // For temp variables (__temp_*), we need to check their actual type
                // Since they're not in the script's variable list, infer_expr_type won't find them
                // But we know from context that most read_node results are Copy types (f32, i32, etc.)
                // So we'll try to infer, and if we can't, we'll check if it looks like a Copy type
                if name.starts_with("__temp_") {
                    // Try to infer type first
                    if let Some(ty) = script.infer_expr_type(expr, current_func) {
                        // We have the type - check if it requires cloning
                        ty.requires_clone()
                    } else {
                        // Can't infer type for temp variable
                        // Most temp variables from read_node are Copy types (f32, i32, Vector2, etc.)
                        // So we assume it doesn't need cloning unless we can prove otherwise
                        // This is safe because Copy types don't need cloning
                        false
                    }
                } else {
                    // Regular variable - use normal type inference
                    if let Some(ty) = script.infer_expr_type(expr, current_func) {
                        ty.requires_clone()
                    } else {
                        false
                    }
                }
            }
            Expr::MemberAccess(..) => {
                if let Some(ty) = script.infer_expr_type(expr, current_func) {
                    ty.requires_clone()
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub fn to_rust(
        &self,
        needs_self: bool,
        script: &Script,
        expected_type: Option<&Type>,
        current_func: Option<&Function>,
        source_span: Option<&crate::scripting::source_span::SourceSpan>, // Source location for error reporting
    ) -> String {
        use ContainerKind::*;

        match self {
            Expr::Ident(name) => {
                // Special case: "self" ALWAYS becomes self.id - never rename it
                if name == "self" {
                    return "self.id".to_string();
                }
                
                // Special case: "api" should NEVER be renamed - it's always the API parameter
                if name == "api" {
                    return "api".to_string();
                }
                
                // Helper function to find a variable in nested blocks (if, for, etc.)
                fn find_variable_in_body<'a>(name: &str, body: &'a [Stmt]) -> Option<&'a Variable> {
                    use crate::scripting::ast::Stmt;
                    for stmt in body {
                        match stmt {
                            Stmt::VariableDecl(var) if var.name == name => {
                                return Some(var);
                            }
                            Stmt::If { then_body, else_body, .. } => {
                                if let Some(v) = find_variable_in_body(name, then_body) {
                                    return Some(v);
                                }
                                if let Some(else_body) = else_body {
                                    if let Some(v) = find_variable_in_body(name, else_body) {
                                        return Some(v);
                                    }
                                }
                            }
                            Stmt::For { body: for_body, .. } | Stmt::ForTraditional { body: for_body, .. } => {
                                if let Some(v) = find_variable_in_body(name, for_body) {
                                    return Some(v);
                                }
                            }
                            _ => {}
                        }
                    }
                    None
                }
                
                let is_local = current_func
                    .map(|f| {
                        f.locals.iter().any(|v| v.name == *name)
                            || f.params.iter().any(|p| p.name == *name)
                            || find_variable_in_body(name, &f.body).is_some()
                    })
                    .unwrap_or(false);

                // Check against `script_vars` to see if it's a field
                let is_field = script.variables.iter().any(|v| v.name == *name);
                
                // Special case: temp variables (__temp_*) should NEVER be renamed if they're NOT user variables
                // If a user actually named a variable __temp_*, we need to rename it to avoid collisions
                if name.starts_with("__temp_") && !is_local && !is_field {
                    return name.to_string();
                }
                
                // Get variable type for renaming
                // If var.typ is None, infer from the variable's value expression
                // We need to handle inferred types separately since we can't return a ref to a temp
                let (var_type_ref, inferred_type_owned) = if is_local {
                    let var_type_ref = current_func.and_then(|f| {
                        f.locals.iter()
                            .find(|v| v.name == *name)
                            .and_then(|v| {
                                // First try explicit type
                                v.typ.as_ref()
                            })
                            .or_else(|| {
                                f.params.iter()
                                    .find(|p| p.name == *name)
                                    .map(|p| &p.typ)
                            })
                    });
                    
                    let inferred = if var_type_ref.is_none() {
                        // If no explicit type, infer from value expression
                        current_func.and_then(|f| {
                            f.locals.iter()
                                .find(|v| v.name == *name)
                                .and_then(|v| v.value.as_ref())
                                .and_then(|val| script.infer_expr_type(&val.expr, current_func))
                        })
                    } else {
                        None
                    };
                    
                    (var_type_ref, inferred)
                } else if is_field {
                    let var_type_ref = script.get_variable_type(name);
                    
                    let inferred = if var_type_ref.is_none() {
                        // If no explicit type, infer from value expression
                        script.variables.iter()
                            .find(|v| v.name == *name)
                            .and_then(|v| v.value.as_ref())
                            .and_then(|val| script.infer_expr_type(&val.expr, current_func))
                    } else {
                        None
                    };
                    
                    (var_type_ref, inferred)
                } else {
                    (None, None)
                };
                
                // Use the EXACT same type determination logic as variable declaration
                // This ensures that when a variable is referenced, it uses the same renamed name
                // as when it was declared (e.g., if declared as tex_id, use tex_id when referenced)
                
                // First, compute the inferred type and API return type if needed
                // Store them in variables that live long enough
                // IMPORTANT: For API calls, the API return type is the most reliable source
                let (inferred_type_storage, api_return_type_storage): (Option<Type>, Option<Type>) = if is_local {
                    if let Some(func) = current_func {
                        // Try to find variable in top-level locals first
                        let local_opt = func.locals.iter().find(|v| v.name == *name)
                            .or_else(|| {
                                // If not found, search nested blocks
                                find_variable_in_body(name, &func.body)
                            });
                        
                        if let Some(local) = local_opt {
                            // Get API return type FIRST if value is an API call (most reliable)
                            let api_type = if let Some(val) = &local.value {
                                if let Expr::ApiCall(api_module, _) = &val.expr {
                                    api_module.return_type()
                                } else {
                                    None
                                }
                            } else {
                                None
                            };
                            
                            // Infer from value expression if no explicit type and not an API call
                            let explicit_type = local.typ.as_ref();
                            let inferred = if explicit_type.is_none() && api_type.is_none() {
                                local.value.as_ref()
                                    .and_then(|val| script.infer_expr_type(&val.expr, current_func))
                            } else {
                                None
                            };
                            
                            (inferred, api_type)
                        } else {
                            (None, None)
                        }
                    } else {
                        (None, None)
                    }
                } else if is_field {
                    let explicit_type = script.get_variable_type(name);
                    let inferred = if explicit_type.is_none() {
                        script.variables.iter()
                            .find(|v| v.name == *name)
                            .and_then(|v| v.value.as_ref())
                            .and_then(|val| script.infer_expr_type(&val.expr, current_func))
                    } else {
                        None
                    };
                    (inferred, None)
                } else {
                    (None, None)
                };
                
                // Now determine the type using the same logic as Stmt::VariableDecl
                // IMPORTANT: For variables assigned from API calls, prefer API return type
                // This ensures consistency with how they were declared
                let type_for_renaming = if is_local {
                    // For local variables, use the same logic as Stmt::VariableDecl:
                    // 1. If value is an API call, use API return type (most reliable)
                    // 2. Try explicit type (var.typ)
                    // 3. If not available, infer from value expression
                    if let Some(func) = current_func {
                        // Try to find variable in top-level locals first, then nested blocks
                        let local_opt = func.locals.iter().find(|v| v.name == *name)
                            .or_else(|| {
                                // If not found, search nested blocks
                                find_variable_in_body(name, &func.body)
                            });
                        
                        if let Some(local) = local_opt {
                            // Prefer API return type if available (this is what was used during declaration)
                            let explicit_type = local.typ.as_ref();
                            
                            // Use API type first (if available), then explicit type, then inferred type
                            // This ensures we use the same type that was used during declaration
                            api_return_type_storage.as_ref()
                                .or_else(|| explicit_type)
                                .or_else(|| inferred_type_storage.as_ref())
                        } else {
                            // Not found in locals, try params
                            current_func.and_then(|f| {
                                f.params.iter()
                                    .find(|p| p.name == *name)
                                    .map(|p| &p.typ)
                            })
                        }
                    } else {
                        var_type_ref
                    }
                } else if is_field {
                    // For script-level variables, use the same logic
                    let explicit_type = script.get_variable_type(name);
                    explicit_type.or_else(|| inferred_type_storage.as_ref())
                } else {
                    var_type_ref.or_else(|| inferred_type_owned.as_ref())
                };

                // Rename variable with t_id_ prefix or _id suffix
                // Use the same type determination logic as declaration to ensure consistency
                let renamed_name = rename_variable(name, type_for_renaming);

                let ident_code = if !is_local && is_field && !name.starts_with("self.") {
                    format!("self.{}", renamed_name)
                } else {
                    renamed_name
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
                    // Pass expected type to literal generation
                    lit.to_rust(Some(expected))
                } else {
                    // No expected type, infer it
                    let inferred_type = script.infer_literal_type(lit, None);
                    lit.to_rust(inferred_type.as_ref())
                }
            }
            Expr::BinaryOp(left, op, right) => {
                let mut left_type = script.infer_expr_type(left, current_func);
                let right_type = script.infer_expr_type(right, current_func);
                
                // Special handling for loop variables: if left is an identifier that might be a loop variable,
                // and we can't find its type, try to infer it from context (ranges produce i32 by default)
                if left_type.is_none() {
                    if let Expr::Ident(var_name) = left.as_ref() {
                        // Check if this might be a loop variable (starts with __t_ or is a common loop var name)
                        // Loop variables from ranges are typically i32
                        if var_name.starts_with("__t_") || var_name == "i" || var_name == "j" || var_name == "k" {
                            // Default to i32 for loop variables (ranges produce i32)
                            left_type = Some(Type::Number(NumberKind::Signed(32)));
                        }
                    }
                }

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

                // Check if left/right are len() calls BEFORE generating code
                let left_is_len = matches!(
                    left.as_ref(),
                    Expr::ApiCall(ApiModule::ArrayOp(ArrayApi::Len), _)
                ) || matches!(left.as_ref(), Expr::MemberAccess(_, field) if field == "Length" || field == "length" || field == "len");
                let right_is_len = matches!(
                    right.as_ref(),
                    Expr::ApiCall(ApiModule::ArrayOp(ArrayApi::Len), _)
                ) || matches!(right.as_ref(), Expr::MemberAccess(_, field) if field == "Length" || field == "length" || field == "len");

                let left_raw =
                    left.to_rust(needs_self, script, dominant_type.as_ref(), current_func, None);
                let right_raw =
                    right.to_rust(needs_self, script, dominant_type.as_ref(), current_func, None);

                // Also check the generated code strings for .len() calls
                let left_is_len = left_is_len || left_raw.ends_with(".len()");
                let right_is_len = right_is_len || right_raw.ends_with(".len()");

                let (left_str, right_str) = {
                    let mut l_str = left_raw.clone();
                    let mut r_str = right_raw.clone();

                    // If left is len() and right is u32/u64 or a literal that looks like u32, convert right to usize
                    if left_is_len {
                        // Check the rendered string first (most reliable)
                        if right_raw.ends_with("u32") || right_raw.ends_with("u") {
                            r_str = format!("({} as usize)", r_str);
                        } else if matches!(right_type, Some(Type::Number(NumberKind::Unsigned(32))))
                        {
                            r_str = format!("({} as usize)", r_str);
                        } else if matches!(right_type, Some(Type::Number(NumberKind::Unsigned(64))))
                        {
                            r_str = format!("({} as usize)", r_str);
                        } else if let Expr::Literal(Literal::Number(n)) = right.as_ref() {
                            // Check if it's a u32 literal (ends with u32 or is just a number that should be usize)
                            if n.ends_with("u32") || n.ends_with("u") {
                                r_str = format!("({} as usize)", r_str);
                            }
                        }
                    }
                    // If right is len() and left is u32/u64 or a literal, convert left to usize
                    if right_is_len {
                        // Check the rendered string first (most reliable)
                        if left_raw.ends_with("u32") || left_raw.ends_with("u") {
                            l_str = format!("({} as usize)", l_str);
                        } else if matches!(left_type, Some(Type::Number(NumberKind::Unsigned(32))))
                        {
                            l_str = format!("({} as usize)", l_str);
                        } else if matches!(left_type, Some(Type::Number(NumberKind::Unsigned(64))))
                        {
                            l_str = format!("({} as usize)", l_str);
                        } else if let Expr::Literal(Literal::Number(n)) = left.as_ref() {
                            if n.ends_with("u32") || n.ends_with("u") {
                                l_str = format!("({} as usize)", l_str);
                            }
                        }
                    }

                    // Apply normal type conversions
                    match (&left_type, &right_type, &dominant_type) {
                        (Some(l), Some(r), Some(dom)) => {
                            let l_cast = if l.can_implicitly_convert_to(dom) && l != dom {
                                script.generate_implicit_cast_for_expr(&l_str, l, dom)
                            } else {
                                l_str
                            };
                            let r_cast = if r.can_implicitly_convert_to(dom) && r != dom {
                                script.generate_implicit_cast_for_expr(&r_str, r, dom)
                            } else {
                                r_str
                            };
                            (l_cast, r_cast)
                        }
                        // Special case: if left is float and right is integer (even if types aren't fully inferred)
                        (Some(Type::Number(NumberKind::Float(32))), Some(Type::Number(NumberKind::Signed(_) | NumberKind::Unsigned(_))), _) => {
                            (l_str, format!("({} as f32)", r_str))
                        }
                        (Some(Type::Number(NumberKind::Float(64))), Some(Type::Number(NumberKind::Signed(_) | NumberKind::Unsigned(_))), _) => {
                            (l_str, format!("({} as f64)", r_str))
                        }
                        // Fallback: if left type is unknown but right is a float, cast left to float
                        (None, Some(Type::Number(NumberKind::Float(32))), _) => {
                            (format!("({} as f32)", l_str), r_str)
                        }
                        (None, Some(Type::Number(NumberKind::Float(64))), _) => {
                            (format!("({} as f64)", l_str), r_str)
                        }
                        // Fallback: if right type is unknown but left is a float, cast right to float
                        (Some(Type::Number(NumberKind::Float(32))), None, _) => {
                            (l_str, format!("({} as f32)", r_str))
                        }
                        (Some(Type::Number(NumberKind::Float(64))), None, _) => {
                            (l_str, format!("({} as f64)", r_str))
                        }
                        _ => (l_str, r_str),
                    }
                };

                // Apply cloning if needed for non-Copy types (BigInt, Decimal, String, etc.)
                let left_final = Expr::clone_if_needed(left_str, left, script, current_func);
                let right_final = Expr::clone_if_needed(right_str, right, script, current_func);

                if matches!(op, Op::Add)
                    && (left_type == Some(Type::String) || right_type == Some(Type::String))
                {
                    return format!("format!(\"{{}}{{}}\", {}, {})", left_final, right_final);
                }

                // Handle null checks: body != null -> body.is_some(), body == null -> body.is_none()
                // Check if one side is the identifier "null" and the other is an Option type
                if matches!(op, Op::Ne | Op::Eq) {
                    let left_is_null = matches!(left.as_ref(), Expr::Ident(name) if name == "null");
                    let right_is_null =
                        matches!(right.as_ref(), Expr::Ident(name) if name == "null");

                    if left_is_null && !right_is_null {
                        // null != body -> body.is_none(), null == body -> body.is_none()
                        if matches!(op, Op::Ne) {
                            return format!("{}.is_some()", right_final);
                        } else {
                            return format!("{}.is_none()", right_final);
                        }
                    } else if right_is_null && !left_is_null {
                        // body != null -> body.is_some(), body == null -> body.is_none()
                        if matches!(op, Op::Ne) {
                            return format!("{}.is_some()", left_final);
                        } else {
                            return format!("{}.is_none()", left_final);
                        }
                    }
                }

                format!("({} {} {})", left_final, op.to_rust(), right_final)
            }
            Expr::MemberAccess(base, field) => {
                // Special case: accessing .id or .node_type on parent field
                // self.parent.id -> api.read_node(self.id, |n| n.parent.as_ref().map(|p| p.id).unwrap_or(Uuid::nil()))
                // self.parent.node_type -> api.read_node(self.id, |n| n.parent.as_ref().map(|p| p.node_type.clone()).unwrap())
                if let Expr::MemberAccess(parent_base, parent_field) = base.as_ref() {
                    if matches!(parent_base.as_ref(), Expr::SelfAccess) && parent_field == "parent" {
                        if field == "id" {
                            return format!("api.read_node(self.id, |self_node: &{}| self_node.parent.as_ref().map(|p| p.id).unwrap_or(Uuid::nil()))", script.node_type);
                        } else if field == "node_type" {
                            return format!("api.read_node(self.id, |self_node: &{}| self_node.parent.as_ref().map(|p| p.node_type.clone()).unwrap())", script.node_type);
                        }
                    }
                }
                
                // Special case: accessing .id on a node just returns the ID directly
                // self.id -> self.id (already a Uuid on the script)
                // nodeVar.id -> nodeVar_id (node variables are stored as UUIDs)
                if field == "id" {
                    if matches!(base.as_ref(), Expr::SelfAccess) {
                        return "self.id".to_string();
                    } else if let Expr::Ident(var_name) = base.as_ref() {
                        // Check if this is a node variable
                        let var_type = script.infer_expr_type(base, current_func);
                        let is_node = match &var_type {
                            Some(Type::Node(_)) => true,
                            Some(Type::Custom(type_name)) => is_node_type(type_name),
                            _ => false,
                        };
                        
                        if is_node {
                            // Node variable - the variable itself is already the ID
                            return rename_variable(var_name, var_type.as_ref());
                        }
                    }
                }
                
                // Check if this is a node member access chain (like self.transform.position)
                // If so, wrap the entire chain in api.read_node
                if let Some((node_id, node_type, field_path, closure_var)) = 
                    extract_node_member_info(&Expr::MemberAccess(base.clone(), field.clone()), script, current_func) 
                {
                    // This is accessing node fields - use api.read_node
                    // Determine if we need to clone the result44
                    
                    if let Some(node_type_enum) = string_to_node_type(&node_type) {
                        let node_type_obj = Type::Node(node_type_enum);
                        
                        // Split the field path to check the final result type
                        let fields: Vec<&str> = field_path.split('.').collect();
                        
                        // Resolve field names in path (e.g., "texture" -> "texture_id")
                        let resolved_fields: Vec<String> = fields.iter()
                            .enumerate()
                            .map(|(i, f)| {
                                // For the first field, resolve against the node type
                                // For subsequent fields, we'd need to resolve against the intermediate type
                                // For now, just resolve the first field against the node type
                                if i == 0 {
                                    ENGINE_REGISTRY.resolve_field_name(&node_type_enum, f)
                                } else {
                                    f.to_string() // TODO: Resolve nested fields properly
                                }
                            })
                            .collect();
                        let resolved_field_path = resolved_fields.join(".");
                        
                        // Walk through the field chain to get the final type (using original field names for type checking)
                        let mut current_type = node_type_obj.clone();
                        for field_name in &fields {
                            if let Some(next_type) = script.get_member_type(&current_type, field_name) {
                                current_type = next_type;
                            }
                        }
                        
                        let needs_clone = current_type.requires_clone();
                        let is_option = matches!(current_type, Type::Option(_));
                        
                        // Only unwrap if the expected type is explicitly NOT an Option
                        // If both the field and expected type are Option, keep it as Option
                        // If expected type is None, keep as Option (don't unwrap by default)
                        let should_unwrap = if is_option {
                            match expected_type {
                                Some(Type::Option(expected_inner)) => {
                                    // Check if the inner types match
                                    match &current_type {
                                        Type::Option(actual_inner) => {
                                            // Only unwrap if inner types don't match
                                            // If they match, keep as Option
                                            actual_inner.as_ref() != expected_inner.as_ref()
                                        }
                                        _ => false, // Keep as Option if we can't determine
                                    }
                                }
                                Some(_) => {
                                    // Expected type is explicitly not an Option, so unwrap
                                    true
                                }
                                None => {
                                    // No expected type hint - keep as Option (don't unwrap by default)
                                    // This is safer and allows the caller to handle Option as needed
                                    false
                                }
                            }
                        } else {
                            false
                        };
                        
                        let field_access = if should_unwrap {
                            format!("{}.{}.unwrap()", closure_var, resolved_field_path)
                        } else if needs_clone {
                            format!("{}.{}.clone()", closure_var, resolved_field_path)
                        } else {
                            format!("{}.{}", closure_var, resolved_field_path)
                        };
                        
                        // Extract mutable API calls to temporary variables to avoid borrow checker issues
                        let (temp_decl, actual_node_id) = extract_mutable_api_call(&node_id);
                        
                        // Use read_node with the determined node type (type must be known via cast or variable annotation)
                        if !temp_decl.is_empty() {
                            return format!("{}{}api.read_node({}, |{}: &{}| {})", temp_decl, if temp_decl.ends_with(';') { " " } else { "" }, actual_node_id, closure_var, node_type, field_access);
                        } else {
                            return format!("api.read_node({}, |{}: &{}| {})", node_id, closure_var, node_type, field_access);
                        }
                    }
                }
                
                // Special case: if base is SelfAccess and field is a script variable,
                // generate self.field instead of self.node.field
                if matches!(base.as_ref(), Expr::SelfAccess) {
                    // Use cached HashSet for O(1) lookup instead of O(n) iteration
                    let script_ptr = script as *const Script as usize;
                    let is_script_member = SCRIPT_MEMBERS_CACHE.with(|cache| {
                        let mut cache_ref = cache.borrow_mut();

                        // Check if cache is valid for this script
                        let needs_rebuild = match cache_ref.as_ref() {
                            Some((cached_ptr, _)) => *cached_ptr != script_ptr,
                            None => true,
                        };

                        if needs_rebuild {
                            // Build HashSet with all script member names
                            let mut set = std::collections::HashSet::new();
                            for var in &script.variables {
                                set.insert(var.name.clone());
                            }
                            for func in &script.functions {
                                set.insert(func.name.clone());
                            }
                            *cache_ref = Some((script_ptr, set));
                        }

                        // Now we know cache exists and is valid
                        cache_ref.as_ref().unwrap().1.contains(field)
                    });

                    if is_script_member {
                        // This is a script field/method, access directly on self
                        // Need to use the renamed variable/function name
                        // Check if it's a variable or function
                        if let Some(var) = script.variables.iter().find(|v| v.name == *field) {
                            // It's a variable, use renamed variable name
                            let renamed_name = rename_variable(&var.name, var.typ.as_ref());
                            return format!("self.{}", renamed_name);
                        } else if script.functions.iter().any(|f| f.name == *field) {
                            // It's a function, use renamed function name
                            let renamed_name = rename_function(field);
                            return format!("self.{}", renamed_name);
                        } else {
                            // Fallback (shouldn't happen if is_script_member is true)
                            return format!("self.{}", field);
                        }
                    }
                    // Otherwise, it's a node field, use self.base.field
                }

                let base_type = script.infer_expr_type(base, current_func);

                match base_type {
                    Some(Type::Object) => {
                        // dynamic object (serde_json::Value)
                        let base_code = base.to_rust(needs_self, script, None, current_func, None);
                        format!("{}[\"{}\"].clone()", base_code, field)
                    }
                    Some(Type::Container(ContainerKind::Map, _)) => {
                        let base_code = base.to_rust(needs_self, script, None, current_func, None);
                        format!("{}[\"{}\"].clone()", base_code, field)
                    }
                    Some(Type::Container(ContainerKind::Array, _))
                    | Some(Type::Container(ContainerKind::FixedArray(_), _)) => {
                        // Special case: .Length or .length on arrays should convert to .len()
                        if field == "Length" || field == "length" || field == "len" {
                            let base_code = base.to_rust(needs_self, script, None, current_func, None);
                            format!("{}.len()", base_code)
                        } else {
                            // Vec or FixedArray (support access via integer index, not field name)
                            let base_code = base.to_rust(needs_self, script, None, current_func, None);
                            format!(
                                "/* Cannot perform field access '{}' on array or fixed array */ {}",
                                field, base_code
                            )
                        }
                    }
                    Some(Type::EngineStruct(_engine_struct)) => {
                        // Engine struct: regular .field access
                        // The base should already be generated correctly (either from read_node or direct access)
                        let base_code = base.to_rust(needs_self, script, None, current_func, None);
                        format!("{}.{}", base_code, field)
                    }
                    Some(Type::Custom(type_name)) => {
                        // typed struct: regular .field access
                        let base_code = base.to_rust(needs_self, script, None, current_func, None);

                        // Check if this is a node type and the base is a node ID variable (UUID or Option<Uuid>)
                        if is_node_type(&type_name) {
                            // Check if base_code is a node ID variable (ends with _id or is self.id)
                            // OR if it's an Option<Uuid> variable (from get_parent() or get_node())
                            // Node variables are renamed to {name}_id, and self.id is the script's node ID
                            let is_node_id_var = base_code.ends_with("_id") || base_code == "self.id";
                            
                            // Check if base is an Option<Uuid> variable (from get_parent() or get_node())
                            // Check in current function's locals first, then script-level variables
                            let is_option_uuid = if let Some(current_func) = current_func {
                                current_func.locals.iter().any(|v| v.name == base_code && matches!(v.typ.as_ref(), Some(Type::Option(inner)) if matches!(inner.as_ref(), Type::Uuid)))
                            } else {
                                script.get_variable_type(&base_code).map_or(false, |t| matches!(t, Type::Option(inner) if matches!(inner.as_ref(), Type::Uuid)))
                            };
                            
                            if is_node_id_var || is_option_uuid {
                                // Use api.read_node to access node properties
                                // Check if the result type requires cloning
                                if let Some(node_type) = string_to_node_type(type_name.as_str()) {
                                    let base_node_type = Type::Node(node_type);
                                    let result_type = script.get_member_type(&base_node_type, field);
                                    let needs_clone = result_type.as_ref().map_or(false, |t| t.requires_clone());
                                    
                                    // Check if the result type is Option<T> - only unwrap if expected type is not Option
                                    let is_option = matches!(result_type.as_ref(), Some(Type::Option(_)));
                                    
                                    // Extract variable name from node_id (e.g., "c_id" -> "c", "par" -> "par")
                                    let param_name = if base_code.ends_with("_id") {
                                        &base_code[..base_code.len() - 3]
                                    } else {
                                        &base_code
                                    };
                                    
                                    // If base is Option<Uuid>, unwrap it before passing to read_node
                                    let node_id_expr = if is_option_uuid {
                                        format!("{}.unwrap()", base_code)
                                    } else {
                                        base_code.clone()
                                    };
                                    
                                    // Resolve field name (e.g., "texture" -> "texture_id")
                                    let resolved_field = ENGINE_REGISTRY.resolve_field_name(&node_type, field);
                                    
                                    // Only unwrap if the expected type is explicitly NOT an Option
                                    // If both the field and expected type are Option, keep it as Option
                                    // If expected type is None, keep as Option (don't unwrap by default)
                                    let should_unwrap = if is_option {
                                        match expected_type {
                                            Some(Type::Option(expected_inner)) => {
                                                // Check if the inner types match
                                                match result_type.as_ref() {
                                                    Some(Type::Option(actual_inner)) => {
                                                        // Only unwrap if inner types don't match
                                                        // If they match, keep as Option
                                                        actual_inner.as_ref() != expected_inner.as_ref()
                                                    }
                                                    _ => false, // Keep as Option if we can't determine
                                                }
                                            }
                                            Some(_) => {
                                                // Expected type is explicitly not an Option, so unwrap
                                                true
                                            }
                                            None => {
                                                // No expected type hint - keep as Option (don't unwrap by default)
                                                // This is safer and allows the caller to handle Option as needed
                                                false
                                            }
                                        }
                                    } else {
                                        false
                                    };
                                    
                                    let field_access = if should_unwrap {
                                        format!("{}.{}.unwrap()", param_name, resolved_field)
                                    } else if needs_clone {
                                        format!("{}.{}.clone()", param_name, resolved_field)
                                    } else {
                                        format!("{}.{}", param_name, resolved_field)
                                    };
                                    
                                    // Extract mutable API calls to temporary variables to avoid borrow checker issues
                                    let (temp_decl, actual_node_id) = extract_mutable_api_call(&node_id_expr);
                                    if !temp_decl.is_empty() {
                                        return format!("{}{}api.read_node({}, |{}: &{}| {})", temp_decl, if temp_decl.ends_with(';') { " " } else { "" }, actual_node_id, param_name, type_name, field_access);
                                    } else {
                                        return format!("api.read_node({}, |{}: &{}| {})", node_id_expr, param_name, type_name, field_access);
                                    }
                                }
                            }
                        }
                        
                        // Also check if base_code is a UUID variable that represents a node (ends with _id)
                        // This handles cases where var b = new Sprite2D() creates b_id: Uuid
                        // We need to look up the original variable name to determine the node type
                        if base_code.ends_with("_id") && base_code != "self.id" {
                            // Extract original variable name (e.g., "b_id" -> "b")
                            let original_var_name = &base_code[..base_code.len() - 3];
                            
                            // Helper to find variable in nested blocks (for loops, if statements, etc.)
                            fn find_variable_in_body<'a>(name: &str, body: &'a [crate::scripting::ast::Stmt]) -> Option<&'a crate::scripting::ast::Variable> {
                                use crate::scripting::ast::Stmt;
                                for stmt in body {
                                    match stmt {
                                        Stmt::VariableDecl(var) if var.name == name => {
                                            return Some(var);
                                        }
                                        Stmt::If { then_body, else_body, .. } => {
                                            if let Some(v) = find_variable_in_body(name, then_body) {
                                                return Some(v);
                                            }
                                            if let Some(else_body) = else_body {
                                                if let Some(v) = find_variable_in_body(name, else_body) {
                                                    return Some(v);
                                                }
                                            }
                                        }
                                        Stmt::For { body: for_body, .. } | Stmt::ForTraditional { body: for_body, .. } => {
                                            if let Some(v) = find_variable_in_body(name, for_body) {
                                                return Some(v);
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                None
                            }
                            
                            // Look up the variable to see if it's a node type
                            // Try multiple lookup strategies to handle variables in different scopes (including loop-scoped)
                            let node_type_opt = if let Some(current_func) = current_func {
                                // Strategy 1: Check in function locals first
                                current_func.locals.iter()
                                    .find(|v| v.name == *original_var_name)
                                    .and_then(|v| {
                                        // Check declared type
                                        if let Some(ref typ) = v.typ {
                                            if type_is_node(typ) {
                                                return get_node_type(typ).cloned();
                                            }
                                        }
                                        // Check inferred type from value expression
                                        v.value.as_ref().and_then(|val| {
                                            let inferred = script.infer_expr_type(&val.expr, Some(current_func));
                                            if let Some(ref inferred_typ) = inferred {
                                                if type_is_node(inferred_typ) {
                                                    return get_node_type(inferred_typ).cloned();
                                                }
                                            }
                                            // Check if value is StructNew creating a node
                                            if let Expr::StructNew(ty_name, _) = &val.expr {
                                                return string_to_node_type(ty_name);
                                            }
                                            None
                                        })
                                    })
                                    // Strategy 2: Check in nested blocks (for loops, if statements, etc.)
                                    .or_else(|| {
                                        find_variable_in_body(original_var_name, &current_func.body)
                                            .and_then(|v| {
                                                // Check declared type
                                                if let Some(ref typ) = v.typ {
                                                    if type_is_node(typ) {
                                                        return get_node_type(typ).cloned();
                                                    }
                                                }
                                                // Check inferred type from value expression
                                                v.value.as_ref().and_then(|val| {
                                                    let inferred = script.infer_expr_type(&val.expr, Some(current_func));
                                                    if let Some(ref inferred_typ) = inferred {
                                                        if type_is_node(inferred_typ) {
                                                            return get_node_type(inferred_typ).cloned();
                                                        }
                                                    }
                                                    // Check if value is StructNew creating a node
                                                    if let Expr::StructNew(ty_name, _) = &val.expr {
                                                        return string_to_node_type(ty_name);
                                                    }
                                                    None
                                                })
                                            })
                                    })
                                    // Strategy 3: Check in params
                                    .or_else(|| {
                                        current_func.params.iter()
                                            .find(|p| p.name == *original_var_name)
                                            .and_then(|p| {
                                                if type_is_node(&p.typ) {
                                                    get_node_type(&p.typ).cloned()
                                                } else {
                                                    None
                                                }
                                            })
                                    })
                                    // Strategy 4: Try to infer type directly from the base expression
                                    // This works even if variable isn't found in locals or nested blocks
                                    .or_else(|| {
                                        if let Expr::Ident(_) = base.as_ref() {
                                            // Try to infer the type of the identifier directly
                                            if let Some(inferred_type) = script.infer_expr_type(base, Some(current_func)) {
                                                if type_is_node(&inferred_type) {
                                                    return get_node_type(&inferred_type).cloned();
                                                }
                                            }
                                        }
                                        None
                                    })
                            } else {
                                // Check script-level variables
                                script.get_variable_type(original_var_name)
                                    .and_then(|typ| {
                                        if type_is_node(&typ) {
                                            get_node_type(&typ).cloned()
                                        } else {
                                            None
                                        }
                                    })
                                    // Fallback: try to infer from the base expression
                                    .or_else(|| {
                                        if let Expr::Ident(_) = base.as_ref() {
                                            if let Some(inferred_type) = script.infer_expr_type(base, None) {
                                                if type_is_node(&inferred_type) {
                                                    return get_node_type(&inferred_type).cloned();
                                                }
                                            }
                                        }
                                        None
                                    })
                            };
                            
                            if let Some(node_type) = node_type_opt {
                                // This is a node UUID variable - use api.read_node
                                let node_type_name = format!("{:?}", node_type);
                                let base_node_type = Type::Node(node_type);
                                let result_type = script.get_member_type(&base_node_type, field);
                                let needs_clone = result_type.as_ref().map_or(false, |t| t.requires_clone());
                                let is_option = matches!(result_type.as_ref(), Some(Type::Option(_)));
                                
                                let param_name = original_var_name;
                                let resolved_field = ENGINE_REGISTRY.resolve_field_name(&node_type, field);
                                
                                let should_unwrap = if is_option {
                                    match expected_type {
                                        Some(Type::Option(expected_inner)) => {
                                            match result_type.as_ref() {
                                                Some(Type::Option(actual_inner)) => {
                                                    actual_inner.as_ref() != expected_inner.as_ref()
                                                }
                                                _ => false,
                                            }
                                        }
                                        Some(_) => true,
                                        None => false,
                                    }
                                } else {
                                    false
                                };
                                
                                let field_access = if should_unwrap {
                                    format!("{}.{}.unwrap()", param_name, resolved_field)
                                } else if needs_clone {
                                    format!("{}.{}.clone()", param_name, resolved_field)
                                } else {
                                    format!("{}.{}", param_name, resolved_field)
                                };
                                
                                let (temp_decl, actual_node_id) = extract_mutable_api_call(&base_code);
                                if !temp_decl.is_empty() {
                                    return format!("{}{}api.read_node({}, |{}: &{}| {})", temp_decl, if temp_decl.ends_with(';') { " " } else { "" }, actual_node_id, param_name, node_type_name, field_access);
                                } else {
                                    return format!("api.read_node({}, |{}: &{}| {})", base_code, param_name, node_type_name, field_access);
                                }
                            }
                        }

                        // Special handling for UINode.get_element - this will be handled in Call expression
                        // when it's cast to a specific type like UIText
                        format!("{}.{}", base_code, field)
                    }
                    Some(Type::Node(node_type)) => {
                        // Node type: check if base is a node ID variable
                        let base_code = base.to_rust(needs_self, script, None, current_func, None);
                        let is_node_id_var = base_code.ends_with("_id") || base_code == "self.id";
                        
                        if is_node_id_var {
                            // Get the node type name from the base type
                            let node_type_name = format!("{:?}", node_type);
                            
                            // Use api.read_node and check if cloning is needed
                            let base_node_type = Type::Node(node_type.clone());
                            let result_type = script.get_member_type(&base_node_type, field);
                            let needs_clone = result_type.as_ref().map_or(false, |t| t.requires_clone());
                            
                            // Check if the result type is Option<T> - if so, unwrap inside the closure
                            let is_option = matches!(result_type.as_ref(), Some(Type::Option(_)));
                            
                            // Extract variable name from node_id (e.g., "c_id" -> "c", "self.id" -> "self_node")
                            let param_name = if base_code.ends_with("_id") {
                                &base_code[..base_code.len() - 3]
                            } else if base_code == "self.id" {
                                "self_node"
                            } else {
                                "n"
                            };
                            
                            // Resolve field name (e.g., "texture" -> "texture_id")
                            let resolved_field = ENGINE_REGISTRY.resolve_field_name(&node_type, field);
                            
                            let field_access = if is_option {
                                format!("{}.{}.unwrap()", param_name, resolved_field)
                            } else if needs_clone {
                                format!("{}.{}.clone()", param_name, resolved_field)
                            } else {
                                format!("{}.{}", param_name, resolved_field)
                            };
                            
                            // Extract mutable API calls to temporary variables to avoid borrow checker issues
                            let (temp_decl, actual_node_id) = extract_mutable_api_call(&base_code);
                            if !temp_decl.is_empty() {
                                format!("{}{}api.read_node({}, |{}: &{}| {})", temp_decl, if temp_decl.ends_with(';') { " " } else { "" }, actual_node_id, param_name, node_type_name, field_access)
                            } else {
                                format!("api.read_node({}, |{}: &{}| {})", base_code, param_name, node_type_name, field_access)
                            }
                        } else {
                            format!("{}.{}", base_code, field)
                        }
                    }
                    Some(Type::DynNode) => {
                        // DynNode: generate match arms for all node types that have this field
                        let base_code = base.to_rust(needs_self, script, None, current_func, None);
                        let is_node_id_var = base_code.ends_with("_id") || base_code == "self.id";
                        
                        if is_node_id_var {
                            // Build field path from the expression (e.g., node.transform.position.x)
                            let mut field_path = vec![field.clone()];
                            let mut current_expr = base.as_ref();
                            while let Expr::MemberAccess(inner_base, inner_field) = current_expr {
                                field_path.push(inner_field.clone());
                                current_expr = inner_base.as_ref();
                            }
                            field_path.reverse(); // Now field_path is [node_base, transform, position, x]
                            
                            // Extract just the field path (skip the base identifier)
                            // For nested access like node.transform.position.x, we want [transform, position, x]
                            let field_path_only: Vec<String> = if field_path.len() > 1 {
                                field_path[1..].to_vec()
                            } else {
                                field_path.clone()
                            };
                            
                            // Find all node types that have this field path
                            let compatible_node_types = ENGINE_REGISTRY.narrow_nodes_by_fields(&field_path_only);
                            
                            if compatible_node_types.is_empty() {
                                // No compatible node types found, fallback to error or default behavior
                                format!("{}.{}", base_code, field)
                            } else {
                                // Generate match arms for all compatible node types
                                let mut match_arms = Vec::new();
                                for node_type in &compatible_node_types {
                                    let node_type_name = format!("{:?}", node_type);
                                    let base_node_type = Type::Node(*node_type);
                                    
                                    // Resolve the full field path to get the result type
                                    let result_type = ENGINE_REGISTRY.resolve_chain_from_node(node_type, &field_path_only);
                                    let needs_clone = result_type.as_ref().map_or(false, |t| t.requires_clone());
                                    let is_option = matches!(result_type.as_ref(), Some(Type::Option(_)));
                                    
                                    let param_name = "n";
                                    // Resolve field names in the path (e.g., "texture" -> "texture_id")
                                    let resolved_path: Vec<String> = field_path_only.iter()
                                        .map(|f| ENGINE_REGISTRY.resolve_field_name(node_type, f))
                                        .collect();
                                    let field_access_str = resolved_path.join(".");
                                    let field_access = if is_option {
                                        format!("{}.{}.unwrap()", param_name, field_access_str)
                                    } else if needs_clone {
                                        format!("{}.{}.clone()", param_name, field_access_str)
                                    } else {
                                        format!("{}.{}", param_name, field_access_str)
                                    };
                                    
                                    match_arms.push(format!(
                                        "NodeType::{} => api.read_node({}, |{}: &{}| {})",
                                        node_type_name, base_code, param_name, node_type_name, field_access
                                    ));
                                }
                                
                                // Generate match expression
                                format!(
                                    "match api.get_type({}) {{ {} _ => panic!(\"Node type not compatible with field access: {}\") }}",
                                    base_code,
                                    match_arms.join(", "),
                                    field_path_only.join(".")
                                )
                            }
                        } else {
                            format!("{}.{}", base_code, field)
                        }
                    }
                    _ => {
                        // fallback, assume normal member access
                        let base_code = base.to_rust(needs_self, script, None, current_func, None);
                        format!("{}.{}", base_code, field)
                    }
                }
            }
            Expr::SelfAccess => {
                // self ALWAYS becomes self.id - never store it as a variable
                // This ensures self is never renamed to t_id_self
                "self.id".to_string()
            }
            Expr::BaseAccess => {
                // BaseAccess is deprecated - use self directly
                "self".to_string()
            }
            Expr::EnumAccess(variant) => {
                match variant {
                    BuiltInEnumVariant::NodeType(node_type) => {
                        format!("NodeType::{:?}", node_type)
                    }
                }
            }
            Expr::Call(target, args) => {
                // Check for chained calls where an ApiCall returning Uuid is followed by
                // a NodeSugar API method that accepts Uuid as its first parameter
                if let Expr::MemberAccess(base, method) = target.as_ref() {
                    // Try to resolve the method as a NodeSugar API
                    if let Some(outer_api) = crate::lang::pup::api::PupNodeSugar::resolve_method(method) {
                        // Check if this API's first parameter is Uuid
                        if let Some(param_types) = outer_api.param_types() {
                            if let Some(first_param_type) = param_types.get(0) {
                                if matches!(first_param_type, Type::Uuid) {
                                    // Check if base is an ApiCall that returns Uuid, or a MemberAccess that should be treated as one
                                    let (inner_call_str, temp_var_name) = if let Expr::ApiCall(api, args) = base.as_ref() {
                                        // Direct ApiCall
                                        if let Some(return_type) = api.return_type() {
                                            if matches!(return_type, Type::Uuid) {
                                                let mut inner_call_str = api.to_rust(args, script, needs_self, current_func);
                                                // Fix any incorrect renaming of "api" to "__t_api" or "t_id_api"
                                                // The "api" identifier should NEVER be renamed - it's always the API parameter
                                                inner_call_str = inner_call_str.replace("__t_api.", "api.").replace("t_id_api.", "api.");
                                                let temp_var = match api {
                                                    ApiModule::NodeSugar(NodeSugarApi::GetParent) => "__parent_id",
                                                    ApiModule::NodeSugar(NodeSugarApi::GetChildByName) => "__child_id",
                                                    _ => "__temp_id",
                                                };
                                                (Some(inner_call_str), Some(temp_var.to_string()))
                                            } else {
                                                (None, None)
                                            }
                                        } else {
                                            (None, None)
                                        }
                                    } else if let Expr::MemberAccess(inner_base, inner_method) = base.as_ref() {
                                        // Handle nested MemberAccess like collision.get_parent()
                                        // Check if this is a NodeSugar API call
                                        if let Some(api) = crate::lang::pup::api::PupNodeSugar::resolve_method(inner_method) {
                                            if let Some(return_type) = api.return_type() {
                                                if matches!(return_type, Type::Uuid) {
                                                    // Create args for the inner API call - the base becomes the first arg
                                                    let inner_api_args = vec![*inner_base.clone()];
                                                    let mut inner_call_str = api.to_rust(&inner_api_args, script, needs_self, current_func);
                                                    // Fix any incorrect renaming of "api" to "__t_api" or "t_id_api"
                                                    // The "api" identifier should NEVER be renamed - it's always the API parameter
                                                    inner_call_str = inner_call_str.replace("__t_api.", "api.").replace("t_id_api.", "api.");
                                                    let temp_var = match api {
                                                        ApiModule::NodeSugar(NodeSugarApi::GetParent) => "__parent_id",
                                                        ApiModule::NodeSugar(NodeSugarApi::GetChildByName) => "__child_id",
                                                        _ => "__temp_id",
                                                    };
                                                    (Some(inner_call_str), Some(temp_var.to_string()))
                                                } else {
                                                    (None, None)
                                                }
                                            } else {
                                                (None, None)
                                            }
                                        } else {
                                            (None, None)
                                        }
                                    } else {
                                        (None, None)
                                    };
                                    
                                    if let (Some(inner_call_str), Some(temp_var)) = (inner_call_str, temp_var_name) {
                                        // This is a chained call: inner_api() returns Uuid,
                                        // and outer_api() accepts Uuid as first param
                                        // Both APIs require mutable borrows (all NodeSugar APIs take &mut self),
                                        // so we MUST extract the inner call to a temporary variable
                                        // to avoid borrow checker errors
                                        
                                        let temp_decl = format!("let {}: Uuid = {};", temp_var, inner_call_str);
                                        
                                        // Create an Ident expression for the temp variable
                                        let temp_var_expr = Expr::Ident(temp_var.clone());
                                        let outer_args = vec![temp_var_expr];
                                        
                                        // Generate the outer call with the temp variable as argument
                                        let outer_call = outer_api.to_rust(&outer_args, script, needs_self, current_func);
                                        
                                        // Combine temp declaration with outer call
                                        return format!("{}{}{}", temp_decl, if temp_decl.ends_with(';') { " " } else { "" }, outer_call);
                                    }
                                }
                            }
                        }
                    }
                }
                
                // ==============================================================
                // Extract the target function name, if possible
                // ==============================================================
                let func_name = Self::get_target_name(target);

                // Determine whether this is a local method on the current script
                let is_local_function = func_name
                    .as_ref()
                    .map(|name| script.functions.iter().any(|f| f.name == *name))
                    .unwrap_or(false);

                let is_engine_method = matches!(target.as_ref(), Expr::MemberAccess(base, _method))
                    && !is_local_function;

                // ✅ NEW: Look up the function to get parameter types
                let func_params = if let Some(name) = &func_name {
                    script
                        .functions
                        .iter()
                        .find(|f| f.name == *name)
                        .map(|f| &f.params)
                } else {
                    None
                };

                // ==============================================================
                // Convert each argument expression into Rust source code
                // with proper ownership semantics and type-aware cloning
                // ==============================================================
                let args_rust: Vec<String> = args
                    .iter()
                    .enumerate()
                    .map(|(i, arg)| {
                        // ✅ Get the expected type for this parameter position
                        let expected_type =
                            func_params.and_then(|params| params.get(i)).map(|p| &p.typ);

                        // Generate code for argument with expected type hint
                        let code = arg.to_rust(needs_self, script, expected_type, current_func, None);

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
                            | (Expr::Ident(_) | Expr::MemberAccess(..), Some(Type::Signal)) => {
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
                            (
                                Expr::BinaryOp(..) | Expr::Call(..) | Expr::Cast(..),
                                Some(Type::String),
                            )
                            | (
                                Expr::BinaryOp(..) | Expr::Call(..) | Expr::Cast(..),
                                Some(Type::Custom(_)),
                            )
                            | (
                                Expr::BinaryOp(..) | Expr::Call(..) | Expr::Cast(..),
                                Some(Type::Signal),
                            ) => {
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
                let mut target_str = target.to_rust(needs_self, script, None, current_func, None);

                // If this is a local user-defined function, prefix with `self.`
                if is_local_function && !target_str.starts_with("self.") {
                    target_str = format!("self.{}", func_name.unwrap());
                }

                // ==============================================================
                // Finally, build the Rust call string
                // Handles API injection and empty arg lists
                // ==============================================================
                if is_engine_method {
                    // ✅ Engine methods: just pass normal args
                    if args_rust.is_empty() {
                        format!("{}()", target_str)
                    } else {
                        format!("{}({})", target_str, args_rust.join(", "))
                    }
                } else if is_local_function {
                    // Local script functions: add api
                    if args_rust.is_empty() {
                        format!("{}(api);", target_str)
                    } else {
                        format!("{}({}, api);", target_str, args_rust.join(", "))
                    }
                } else {
                    // Fallback: treat as external function with api
                    if args_rust.is_empty() {
                        format!("{}(api);", target_str)
                    } else {
                        format!("{}({}, api);", target_str, args_rust.join(", "))
                    }
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
                            Some(Type::Container(ContainerKind::Map, types))
                                if types.len() == 2 =>
                            {
                                (&types[0], &types[1])
                            }
                            _ => (&Type::String, &Type::Object),
                        };

                        let entries: Vec<_> = pairs
                            .iter()
                            .map(|(k_expr, v_expr)| {
                                let raw_k = k_expr.to_rust(
                                    needs_self,
                                    script,
                                    Some(expected_key_type),
                                    current_func,
                                    None, // k_expr is Expr, no span available
                                );
                                let raw_v = v_expr.to_rust(
                                    needs_self,
                                    script,
                                    Some(expected_val_type),
                                    current_func,
                                    None, // v_expr is Expr, no span available
                                );

                                // For dynamic maps (String keys), convert numeric keys to strings
                                let k_final = if *expected_key_type == Type::String {
                                    let k_type = script.infer_expr_type(k_expr, current_func);
                                    match k_type {
                                        Some(Type::Number(_)) | Some(Type::Bool) => {
                                            format!("{}.to_string()", raw_k)
                                        }
                                        _ => {
                                            if Expr::should_clone_expr(
                                                &raw_k,
                                                k_expr,
                                                script,
                                                current_func,
                                            ) {
                                                format!("{}.clone()", raw_k)
                                            } else {
                                                raw_k
                                            }
                                        }
                                    }
                                } else {
                                    if Expr::should_clone_expr(&raw_k, k_expr, script, current_func)
                                    {
                                        format!("{}.clone()", raw_k)
                                    } else {
                                        raw_k
                                    }
                                };

                                // Wrap value in json!() if this is a dynamic map (Value type) or custom type
                                let v_final = if *expected_val_type == Type::Object
                                    || matches!(expected_val_type, Type::Custom(_))
                                {
                                    // For dynamic maps or custom types, wrap in json!()
                                    if Expr::should_clone_expr(&raw_v, v_expr, script, current_func)
                                    {
                                        format!("json!({}.clone())", raw_v)
                                    } else {
                                        format!("json!({})", raw_v)
                                    }
                                } else {
                                    // For typed maps, just clone if needed
                                    if Expr::should_clone_expr(&raw_v, v_expr, script, current_func)
                                    {
                                        format!("{}.clone()", raw_v)
                                    } else {
                                        raw_v
                                    }
                                };

                                format!("({}, {})", k_final, v_final)
                            })
                            .collect();

                        // Determine the correct HashMap type based on expected types
                        let final_code = if *expected_val_type == Type::Object
                            || matches!(expected_val_type, Type::Custom(_))
                        {
                            // Dynamic map: HashMap<String, Value>
                            format!("HashMap::<String, Value>::from([{}])", entries.join(", "))
                        } else {
                            // Typed map: HashMap<K, V>
                            let key_rust = expected_key_type.to_rust_type();
                            let val_rust = expected_val_type.to_rust_type();
                            format!(
                                "HashMap::<{}, {}>::from([{}])",
                                key_rust,
                                val_rust,
                                entries.join(", ")
                            )
                        };
                        final_code
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
                                let rendered =
                                    e.to_rust(needs_self, script, Some(elem_ty), current_func, None);

                                // If this is a custom type array or any[]/object[] array, wrap each element in json!()
                                let final_rendered = match elem_ty {
                                    Type::Custom(_) | Type::Object => {
                                        // Custom types and any[]/object[] arrays need to be serialized to Value
                                        format!("json!({})", rendered)
                                    }
                                    _ => {
                                        if Expr::should_clone_expr(
                                            &rendered,
                                            e,
                                            script,
                                            current_func,
                                        ) {
                                            format!("{}.clone()", rendered)
                                        } else {
                                            rendered
                                        }
                                    }
                                };
                                final_rendered
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
                    // Extract element type from expected_type if it's a Container
                    let elem_ty = match expected_type {
                        Some(Type::Container(ContainerKind::Array, types))
                        | Some(Type::Container(ContainerKind::FixedArray(_), types))
                            if !types.is_empty() =>
                        {
                            &types[0]
                        }
                        _ => &Type::Object,
                    };

                    let mut body: Vec<_> = elems
                        .iter()
                        .map(|e| {
                            // Pass element type to to_rust so literals get correct suffix (e.g., f64 for number[])
                            let rendered =
                                e.to_rust(needs_self, script, Some(elem_ty), current_func, None);

                            // If this is a custom type or any[]/object[] array, wrap in json!()
                            let final_rendered = match elem_ty {
                                Type::Custom(_) | Type::Object => {
                                    format!("json!({})", rendered)
                                }
                                _ => match e {
                                    Expr::Ident(_) | Expr::MemberAccess(..) => {
                                        let ty = script.infer_expr_type(e, current_func);
                                        if ty.as_ref().map_or(false, |t| t.requires_clone()) {
                                            format!("{}.clone()", rendered)
                                        } else {
                                            rendered
                                        }
                                    }
                                    _ => rendered,
                                },
                            };
                            final_rendered
                        })
                        .collect();

                    while body.len() < *size {
                        body.push("Default::default()".into());
                    }
                    if body.len() > *size {
                        body.truncate(*size);
                    }

                    // Check if expected type is Array (Vec<T>) - if so, convert FixedArray to vec![]
                    let should_convert_to_vec =
                        if let Some(Type::Container(ContainerKind::Array, _)) = expected_type {
                            true
                        } else {
                            false
                        };

                    let code = if should_convert_to_vec {
                        // Convert FixedArray to Vec for Array variable types
                        format!("vec![{}]", body.join(", "))
                    } else {
                        // Keep as fixed array [T; N]
                        format!("[{}]", body.join(", "))
                    };

                    if matches!(expected_type, Some(Type::Object)) {
                        format!("json!({})", code)
                    } else {
                        code
                    }
                }
            },
            Expr::StructNew(ty, args) => {
                use std::collections::HashMap;

                // Special case: For node types with no arguments, use api.create_node::<Type>()
                // This returns a Uuid, not a node instance
                if args.is_empty() && is_node_type(ty) {
                    return format!("api.create_node::<{}>()", ty);
                }

                // Special case: For engine structs, use their constructor functions
                if let Some(_engine_struct) = EngineStructKind::from_string(ty) {
                    // Engine structs like Vector2, Transform2D, etc. use ::new() constructors
                    if args.is_empty() {
                        // No args: use default constructor or ::default()
                        return format!("{}::default()", ty);
                    } else {
                        // With args: use ::new() constructor
                        let arg_codes: Vec<String> = args
                            .iter()
                            .map(|(_, expr)| {
                                expr.to_rust(needs_self, script, None, current_func, None)
                            })
                            .collect();
                        return format!("{}::new({})", ty, arg_codes.join(", "));
                    }
                }

                // --- Flatten structure hierarchy correctly ---
                fn gather_flat_fields<'a>(
                    s: &'a StructDef,
                    script: &'a Script,
                    out: &mut Vec<(&'a str, &'a Type, Option<&'a str>)>,
                ) {
                    if let Some(ref base) = s.base {
                        if let Some(basedef) = script.structs.iter().find(|b| &b.name == base) {
                            gather_flat_fields_with_parent(
                                basedef,
                                script,
                                out,
                                Some(base.as_str()),
                            );
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
                            gather_flat_fields_with_parent(
                                basedef,
                                script,
                                out,
                                Some(base.as_str()),
                            );
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
                    .unwrap_or_else(|| {
                        panic!(
                            "Struct not found: '{}'. Available structs: {:?}",
                            ty,
                            script.structs.iter().map(|s| &s.name).collect::<Vec<_>>()
                        )
                    });

                let mut flat_fields = Vec::new();
                gather_flat_fields(struct_def, script, &mut flat_fields);

                // Map arguments in order to flattened field list
                // ----------------------------------------------------------
                // Map each parsed (field_name, expr) to its real definition
                // ----------------------------------------------------------
                let mut field_exprs: Vec<(&str, &Type, Option<&str>, &Expr)> = Vec::new();

                for (field_name, expr) in args {
                    // look for a matching field by name anywhere in the flattened struct hierarchy
                    if let Some((fname, fty, parent)) = flat_fields
                        .iter()
                        .find(|(fname, _, _)| *fname == field_name.as_str())
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

                    let renamed_base_name = rename_struct(base_name);
                    let mut parts = String::new();

                    // Handle deeper bases first
                    if let Some(ref inner) = base_struct.base {
                        let inner_code =
                            build_base_init(inner, base_fields, script, needs_self, current_func);
                        parts.push_str(&format!("base: {}, ", inner_code));
                    }

                    // Write base's own fields
                    if let Some(local_fields) = base_fields.get(base_name) {
                        for (fname, fty, expr) in local_fields {
                            let mut expr_code =
                                expr.to_rust(needs_self, script, Some(fty), current_func, None);
                            let expr_type = script.infer_expr_type(expr, current_func);
                            let should_clone =
                                matches!(expr, Expr::Ident(_) | Expr::MemberAccess(..))
                                    && expr_type.as_ref().map_or(false, |ty| ty.requires_clone());
                            if should_clone {
                                expr_code = format!("{}.clone()", expr_code);
                            }
                            parts.push_str(&format!("{}: {}, ", fname, expr_code));
                        }
                    }

                    format!("{}::new({})", renamed_base_name, parts.trim_end_matches(", "))
                }

                // --- Build final top-level struct ---
                let mut code = String::new();

                // 1️⃣ Base (if exists)
                if let Some(ref base_name) = struct_def.base {
                    let base_code =
                        build_base_init(base_name, &base_fields, script, needs_self, current_func);
                    code.push_str(&format!("base: {}, ", base_code));
                }

                // 2️⃣ Derived-only fields
                for (fname, fty, expr) in &derived_fields {
                    let mut expr_code = expr.to_rust(needs_self, script, Some(fty), current_func, None);
                    let expr_type = script.infer_expr_type(expr, current_func);
                    let should_clone = matches!(expr, Expr::Ident(_) | Expr::MemberAccess(..))
                        && expr_type.as_ref().map_or(false, |ty| ty.requires_clone());
                    if should_clone {
                        expr_code = format!("{}.clone()", expr_code);
                    }
                    code.push_str(&format!("{}: {}, ", fname, expr_code));
                }

                // Use renamed struct name for custom structs (not node types or engine structs)
                let struct_name = if is_node_type(ty) || EngineStructKind::from_string(ty).is_some() {
                    ty.to_string()
                } else {
                    rename_struct(ty)
                };
                // Use struct literal syntax with renamed struct name
                format!("{} {{ {}..Default::default() }}", struct_name, code)
            }
            Expr::ApiCall(module, args) => {
                // Get expected param types (if defined for this API)
                let expected_param_types = module.param_types();

                // Check if first argument is an ApiCall that returns Uuid and this API takes Uuid as first param
                // If so, extract the inner call to a temp variable to avoid borrow checker errors
                let mut temp_decl_opt: Option<String> = None;
                let mut temp_var_opt: Option<String> = None;
                
                if let Some(param_types) = &expected_param_types {
                    if let Some(first_param_type) = param_types.get(0) {
                        if matches!(first_param_type, Type::Uuid) {
                            if let Some(first_arg) = args.get(0) {
                                // Check if first_arg is a Cast containing an ApiCall, or a direct ApiCall
                                let inner_api_call = if let Expr::Cast(inner_expr, _) = first_arg {
                                    if let Expr::ApiCall(inner_api, inner_args) = inner_expr.as_ref() {
                                        Some((inner_api, inner_args))
                                    } else {
                                        None
                                    }
                                } else if let Expr::ApiCall(inner_api, inner_args) = first_arg {
                                    Some((inner_api, inner_args))
                                } else {
                                    None
                                };
                                
                                if let Some((inner_api, inner_args)) = inner_api_call {
                                    if let Some(return_type) = inner_api.return_type() {
                                        if matches!(return_type, Type::Uuid | Type::DynNode) || 
                                           matches!(return_type, Type::Option(boxed) if matches!(boxed.as_ref(), Type::Uuid)) {
                                            // Both APIs require mutable borrows - extract inner call to temp variable
                                            let mut inner_call_str = inner_api.to_rust(inner_args, script, needs_self, current_func);
                                            
                                            // Fix any incorrect renaming of "api" to "__t_api" or "t_id_api"
                                            // The "api" identifier should NEVER be renamed - it's always the API parameter
                                            inner_call_str = inner_call_str.replace("__t_api.", "api.").replace("t_id_api.", "api.");
                                            
                                            // Generate temp variable name based on inner API
                                            let temp_var = match inner_api {
                                                ApiModule::NodeSugar(NodeSugarApi::GetParent) => "__parent_id".to_string(),
                                                ApiModule::NodeSugar(NodeSugarApi::GetChildByName) => "__child_id".to_string(),
                                                _ => "__temp_id".to_string(),
                                            };
                                            
                                            temp_decl_opt = Some(format!("let {}: Uuid = {};", temp_var, inner_call_str));
                                            temp_var_opt = Some(temp_var);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Generate argument code with expected type hints applied **now**
                let mut arg_strs: Vec<String> = args
                    .iter()
                    .enumerate()
                    .map(|(i, arg)| {
                        // If this is the first arg and we extracted it to a temp variable, use the temp var
                        if i == 0 && temp_var_opt.is_some() {
                            temp_var_opt.as_ref().unwrap().clone()
                        } else {
                            // Determine expected type for this argument
                            let expected_ty_hint = expected_param_types.as_ref().and_then(|v| v.get(i));

                            // Ask expression to render itself, with the hint
                            // Note: We don't have source span for individual args, pass None
                            // Get source span from arg if it's a TypedExpr (we don't have that context here)
                            arg.to_rust(needs_self, script, expected_ty_hint, current_func, None)
                        }
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

                // Generate the API call code
                // If we extracted the first arg to a temp variable, create a new args list with the temp var
                let api_call_args = if temp_var_opt.is_some() {
                    // Replace first arg with Ident expression for temp variable
                    let mut new_args = args.clone();
                    if let Some(temp_var) = &temp_var_opt {
                        new_args[0] = Expr::Ident(temp_var.clone());
                    }
                    new_args
                } else {
                    args.clone()
                };
                // Generate API call
                let api_call_code = module.to_rust(&api_call_args, script, needs_self, current_func);
                
                // If we have a temp declaration, prepend it
                if let Some(temp_decl) = &temp_decl_opt {
                    return format!("{}{}{}", temp_decl, if temp_decl.ends_with(';') { " " } else { "" }, api_call_code);
                }

                // If we have an expected_type and the API returns Object, cast the result
                // This handles cases like: let x: number = map.get("key");
                // BUT: Only apply cast if the map is actually dynamic (returns Value)
                // For static maps (e.g., HashMap<String, BigInt>), the API already returns the correct type
                if let Some(expected_ty) = expected_type {
                    let api_return_type = module.return_type();
                    if let Some(Type::Object) = api_return_type.as_ref() {
                        // Check if this is MapApi::Get and if the map is actually dynamic
                        let should_cast = if let ApiModule::MapOp(MapApi::Get) = module {
                            // For MapApi::Get, check if the map's value type is Object (dynamic)
                            // If it's not Object, then it's a static map and we shouldn't cast
                            if let Some(map_expr) = args.get(0) {
                                let map_value_type =
                                    script.infer_map_value_type(map_expr, current_func);
                                map_value_type.as_ref() == Some(&Type::Object)
                            } else {
                                true // Fallback: assume dynamic if we can't infer
                            }
                        } else {
                            true // For other APIs, apply cast if they return Object
                        };

                        if should_cast && *expected_ty != Type::Object {
                            // Generate cast from Value to expected type
                            match expected_ty {
                                Type::Number(NumberKind::Float(64)) => {
                                    format!("{}.as_f64().unwrap_or_default()", api_call_code)
                                }
                                Type::Number(NumberKind::Float(32)) => {
                                    format!("{}.as_f64().unwrap_or_default() as f32", api_call_code)
                                }
                                Type::Number(NumberKind::BigInt) => {
                                    // Value can be a string representation of BigInt or a number
                                    format!(
                                        "{}.as_str().and_then(|s| s.parse::<BigInt>().ok()).unwrap_or_else(|| BigInt::from({}.as_i64().unwrap_or_default()))",
                                        api_call_code, api_call_code
                                    )
                                }
                                Type::Number(NumberKind::Decimal) => {
                                    // Decimal is stored as f64 in Value, convert using from_str_exact
                                    format!(
                                        "rust_decimal::Decimal::from_str_exact(&{}.as_f64().unwrap_or_default().to_string()).unwrap_or_default()",
                                        api_call_code
                                    )
                                }
                                Type::String => {
                                    format!(
                                        "{}.as_str().unwrap_or_default().to_string()",
                                        api_call_code
                                    )
                                }
                                Type::Bool => {
                                    format!("{}.as_bool().unwrap_or_default()", api_call_code)
                                }
                                Type::Custom(custom_type) => {
                                    format!(
                                        "serde_json::from_value::<{}>({}).unwrap_or_default()",
                                        custom_type, api_call_code
                                    )
                                }
                                _ => api_call_code,
                            }
                        } else {
                            api_call_code
                        }
                    } else {
                        api_call_code
                    }
                } else {
                    api_call_code
                }
            }
            Expr::Range(start, end) => {
                // For ranges, ensure integer literals are typed as integers, not floats
                // Rust ranges require types that implement Step, which f32 doesn't
                // Check if start/end are number literals - if so, prefer i32 for ranges
                let start_inferred = script.infer_expr_type(start, current_func);
                let start_expected_type = match &**start {
                    Expr::Literal(Literal::Number(_)) => {
                        // For number literals in ranges, default to i32 unless already typed as integer
                        start_inferred
                            .map(|t| match t {
                                Type::Number(NumberKind::Float(_)) => {
                                    Type::Number(NumberKind::Signed(32))
                                }
                                other => other,
                            })
                            .or(Some(Type::Number(NumberKind::Signed(32))))
                    }
                    _ => start_inferred,
                };
                let end_inferred = script.infer_expr_type(end, current_func);
                let end_expected_type = match &**end {
                    Expr::Literal(Literal::Number(_)) => end_inferred
                        .map(|t| match t {
                            Type::Number(NumberKind::Float(_)) => {
                                Type::Number(NumberKind::Signed(32))
                            }
                            other => other,
                        })
                        .or(Some(Type::Number(NumberKind::Signed(32)))),
                    _ => end_inferred,
                };

                let start_code = start.to_rust(
                    needs_self,
                    script,
                    start_expected_type.as_ref(),
                    current_func,
                    None, // start is Expr, no span available
                );
                let end_code =
                    end.to_rust(needs_self, script, end_expected_type.as_ref(), current_func, None); // end is Expr, no span available
                format!("({}..{})", start_code, end_code)
            }
            Expr::Cast(inner, target_type) => {
                // Special case: if inner is SelfAccess, ALWAYS return self.id - never store it
                if matches!(inner.as_ref(), Expr::SelfAccess) {
                    return "self.id".to_string();
                }
                
                let inner_type = script.infer_expr_type(inner, current_func);
                // Don't pass target_type as expected_type - let the literal be its natural type, then cast

                // Special case: ui_node.get_element("name") as UIText
                // Convert get_element to get_element_clone with the target type
                if let Expr::Call(target, args) = inner.as_ref() {
                    if let Expr::MemberAccess(base, method) = target.as_ref() {
                        if method == "get_element" && args.len() == 1 {
                            // This is get_element call being cast - convert to get_element_clone
                            let base_code = base.to_rust(needs_self, script, None, current_func, None);
                            let arg_code = args[0].to_rust(needs_self, script, None, current_func, None);
                            if let Type::Custom(type_name) = target_type {
                                return format!(
                                    "{}.get_element_clone::<{}>({})",
                                    base_code, type_name, arg_code
                                );
                            }
                        }
                    }
                }

                let mut inner_code = inner.to_rust(needs_self, script, None, current_func, None);
                
                // Special case: if inner_code is "self" or contains t_id_self, fix it to self.id
                inner_code = if inner_code == "self" || inner_code.starts_with("t_id_self") {
                    "self.id".to_string()
                } else {
                    inner_code
                };
                
                // Fix any incorrect renaming of "api" to "__t_api" or "t_id_api"
                // The "api" identifier should NEVER be renamed - it's always the API parameter
                inner_code = inner_code.replace("__t_api.", "api.").replace("t_id_api.", "api.");

                match (&inner_type, target_type) {
                    // String → Numeric Type Conversions
                    (Some(Type::String), Type::Number(NumberKind::Signed(w))) => match w {
                        8 => format!("{}.parse::<i8>().unwrap_or_default()", inner_code),
                        16 => format!("{}.parse::<i16>().unwrap_or_default()", inner_code),
                        32 => format!("{}.parse::<i32>().unwrap_or_default()", inner_code),
                        64 => format!("{}.parse::<i64>().unwrap_or_default()", inner_code),
                        128 => format!("{}.parse::<i128>().unwrap_or_default()", inner_code),
                        _ => format!("{}.parse::<i32>().unwrap_or_default()", inner_code),
                    },

                    (Some(Type::String), Type::Number(NumberKind::Unsigned(w))) => match w {
                        8 => format!("{}.parse::<u8>().unwrap_or_default()", inner_code),
                        16 => format!("{}.parse::<u16>().unwrap_or_default()", inner_code),
                        32 => format!("{}.parse::<u32>().unwrap_or_default()", inner_code),
                        64 => format!("{}.parse::<u64>().unwrap_or_default()", inner_code),
                        128 => format!("{}.parse::<u128>().unwrap_or_default()", inner_code),
                        _ => format!("{}.parse::<u32>().unwrap_or_default()", inner_code),
                    },

                    (Some(Type::String), Type::Number(NumberKind::Float(w))) => match w {
                        32 => format!("{}.parse::<f32>().unwrap_or_default()", inner_code),
                        64 => format!("{}.parse::<f64>().unwrap_or_default()", inner_code),
                        _ => format!("{}.parse::<f32>().unwrap_or_default()", inner_code),
                    },

                    (Some(Type::String), Type::Number(NumberKind::Decimal)) => format!(
                        "Decimal::from_str({}.as_ref()).unwrap_or_default()",
                        inner_code
                    ),

                    (Some(Type::String), Type::Number(NumberKind::BigInt)) => format!(
                        "BigInt::from_str({}.as_ref()).unwrap_or_default()",
                        inner_code
                    ),

                    (Some(Type::String), Type::Bool) => {
                        format!("{}.parse::<bool>().unwrap_or_default()", inner_code)
                    }

                    // Numeric/Bool → String Conversions
                    (Some(Type::Number(_)), Type::String) | (Some(Type::Bool), Type::String) => {
                        format!("{}.to_string()", inner_code)
                    }

                    // String type conversions
                    // String -> CowStr (owned string to Cow)
                    (Some(Type::String), Type::CowStr) => {
                        // Optimize String::from("...") to Cow::Borrowed("...")
                        if let Some(captured_str) = inner_code.strip_prefix("String::from(\"")
                            .and_then(|s| s.strip_suffix("\")")) {
                            format!("Cow::Borrowed(\"{}\")", captured_str)
                        } else {
                            format!("{}.into()", inner_code)
                        }
                    }
                    // Option<String> -> Option<CowStr>
                    (Some(Type::Option(inner_from)), Type::Option(inner_to)) 
                        if matches!(inner_from.as_ref(), Type::String) && matches!(inner_to.as_ref(), Type::CowStr) => {
                        // Optimize Some(String::from("...")) to Some(Cow::Borrowed("..."))
                        if let Some(captured_str) = inner_code.strip_prefix("Some(String::from(\"")
                            .and_then(|s| s.strip_suffix("\"))")) {
                            format!("Some(Cow::Borrowed(\"{}\"))", captured_str)
                        } else {
                            format!("{}.map(|s| s.into())", inner_code)
                        }
                    }
                    // StrRef -> CowStr (borrowed string to Cow)
                    (Some(Type::StrRef), Type::CowStr) => {
                        format!("{}.into()", inner_code)
                    }
                    // CowStr -> String (Cow to owned String)
                    (Some(Type::CowStr), Type::String) => {
                        format!("{}.into_owned()", inner_code)
                    }
                    // CowStr -> StrRef (Cow to &str - only if Borrowed)
                    (Some(Type::CowStr), Type::StrRef) => {
                        format!("{}.as_ref()", inner_code)
                    }
                    // Node types -> Uuid (nodes are Uuid IDs)
                    (Some(Type::Node(_)), Type::Uuid) => {
                        // Special case: if inner_code is "self" or contains "self", ensure it's self.id
                        if inner_code == "self" || (inner_code.starts_with("self") && !inner_code.contains("self.id")) {
                            "self.id".to_string()
                        } else if inner_code == "self.id" || inner_code.ends_with(".id") {
                            // Already self.id or ends with .id - no cast needed, it's already Uuid
                            inner_code
                        } else {
                            inner_code // Already a Uuid, no conversion needed
                        }
                    }
                    // Uuid -> Node type (for type checking, just pass through)
                    (Some(Type::Uuid), Type::Node(_)) => {
                        inner_code // Already a Uuid, no conversion needed
                    }
                    // T -> Option<T> conversions (wrapping in Some)
                    (Some(from), Type::Option(inner)) if from == inner.as_ref() => {
                        format!("Some({})", inner_code)
                    }
                    // UuidOption (Option<Uuid>) -> Uuid
                    // This is for get_child_by_name() which returns Option<Uuid>
                    (Some(Type::Custom(from_name)), Type::Uuid)
                        if from_name == "UuidOption" =>
                    {
                        // Unwrap the Option<Uuid>
                        format!("{}.unwrap()", inner_code)
                    }

                    // BigInt → Signed Integer
                    (
                        Some(Type::Number(NumberKind::BigInt)),
                        Type::Number(NumberKind::Signed(w)),
                    ) => match w {
                        8 => format!("{}.to_i8().unwrap_or_default()", inner_code),
                        16 => format!("{}.to_i16().unwrap_or_default()", inner_code),
                        32 => format!("{}.to_i32().unwrap_or_default()", inner_code),
                        64 => format!("{}.to_i64().unwrap_or_default()", inner_code),
                        128 => format!("{}.to_i128().unwrap_or_default()", inner_code),
                        _ => format!("({}.to_i64().unwrap_or_default() as i{})", inner_code, w),
                    },

                    // BigInt → Unsigned Integer
                    (
                        Some(Type::Number(NumberKind::BigInt)),
                        Type::Number(NumberKind::Unsigned(w)),
                    ) => match w {
                        8 => format!("{}.to_u8().unwrap_or_default()", inner_code),
                        16 => format!("{}.to_u16().unwrap_or_default()", inner_code),
                        32 => format!("{}.to_u32().unwrap_or_default()", inner_code),
                        64 => format!("{}.to_u64().unwrap_or_default()", inner_code),
                        128 => format!("{}.to_u128().unwrap_or_default()", inner_code),
                        _ => format!("({}.to_u64().unwrap_or_default() as u{})", inner_code, w),
                    },

                    // BigInt ↔ Float
                    (
                        Some(Type::Number(NumberKind::BigInt)),
                        Type::Number(NumberKind::Float(32)),
                    ) => format!("{}.to_f32().unwrap_or_default()", inner_code),
                    (
                        Some(Type::Number(NumberKind::BigInt)),
                        Type::Number(NumberKind::Float(64)),
                    ) => format!("{}.to_f64().unwrap_or_default()", inner_code),
                    (
                        Some(Type::Number(NumberKind::Float(w))),
                        Type::Number(NumberKind::BigInt),
                    ) => match w {
                        32 => format!("BigInt::from({} as i32)", inner_code),
                        64 => format!("BigInt::from({} as i64)", inner_code),
                        _ => format!("BigInt::from({} as i64)", inner_code),
                    },

                    // BigInt → String
                    (Some(Type::Number(NumberKind::BigInt)), Type::String) => {
                        format!("{}.to_string()", inner_code)
                    }

                    // Decimal → Integer
                    (
                        Some(Type::Number(NumberKind::Decimal)),
                        Type::Number(NumberKind::Signed(w)),
                    ) => match w {
                        8 => format!("{}.to_i8().unwrap_or_default()", inner_code),
                        16 => format!("{}.to_i16().unwrap_or_default()", inner_code),
                        32 => format!("{}.to_i32().unwrap_or_default()", inner_code),
                        64 => format!("{}.to_i64().unwrap_or_default()", inner_code),
                        128 => format!("({}.to_i64().unwrap_or_default() as i{})", inner_code, w),
                        _ => format!("({}.to_i64().unwrap_or_default() as i{})", inner_code, w),
                    },
                    (
                        Some(Type::Number(NumberKind::Decimal)),
                        Type::Number(NumberKind::Unsigned(w)),
                    ) => match w {
                        8 => format!("{}.to_u8().unwrap_or_default()", inner_code),
                        16 => format!("{}.to_u16().unwrap_or_default()", inner_code),
                        32 => format!("{}.to_u32().unwrap_or_default()", inner_code),
                        64 => format!("{}.to_u64().unwrap_or_default()", inner_code),
                        128 => format!("({}.to_u64().unwrap_or_default() as u{})", inner_code, w),
                        _ => format!("({}.to_u64().unwrap_or_default() as u{})", inner_code, w),
                    },

                    // Decimal → Float
                    (
                        Some(Type::Number(NumberKind::Decimal)),
                        Type::Number(NumberKind::Float(32)),
                    ) => format!("{}.to_f32().unwrap_or_default()", inner_code),
                    (
                        Some(Type::Number(NumberKind::Decimal)),
                        Type::Number(NumberKind::Float(64)),
                    ) => format!("{}.to_f64().unwrap_or_default()", inner_code),

                    // Integer/Float → Decimal
                    (
                        Some(Type::Number(NumberKind::Signed(_) | NumberKind::Unsigned(_))),
                        Type::Number(NumberKind::Decimal),
                    ) => format!("Decimal::from({})", inner_code),

                    (
                        Some(Type::Number(NumberKind::Float(32))),
                        Type::Number(NumberKind::Decimal),
                    ) => format!(
                        "rust_decimal::prelude::FromPrimitive::from_f32({}).unwrap_or_default()",
                        inner_code
                    ),
                    (
                        Some(Type::Number(NumberKind::Float(64))),
                        Type::Number(NumberKind::Decimal),
                    ) => format!(
                        "rust_decimal::prelude::FromPrimitive::from_f64({}).unwrap_or_default()",
                        inner_code
                    ),

                    // Decimal ↔ BigInt
                    (Some(Type::Number(NumberKind::Decimal)), Type::Number(NumberKind::BigInt)) => {
                        format!("BigInt::from({}.to_i64().unwrap_or_default())", inner_code)
                    }
                    (Some(Type::Number(NumberKind::BigInt)), Type::Number(NumberKind::Decimal)) => {
                        format!("Decimal::from({}.to_i64().unwrap_or_default())", inner_code)
                    }

                    // Decimal → String
                    (Some(Type::Number(NumberKind::Decimal)), Type::String) => {
                        format!("{}.to_string()", inner_code)
                    }

                    // Standard Numeric Casts
                    (
                        Some(Type::Number(NumberKind::Signed(_))),
                        Type::Number(NumberKind::Signed(to_w)),
                    ) => format!("({} as i{})", inner_code, to_w),
                    (
                        Some(Type::Number(NumberKind::Signed(_))),
                        Type::Number(NumberKind::Unsigned(to_w)),
                    ) => format!("({} as u{})", inner_code, to_w),
                    (
                        Some(Type::Number(NumberKind::Unsigned(_))),
                        Type::Number(NumberKind::Unsigned(to_w)),
                    ) => format!("({} as u{})", inner_code, to_w),
                    (
                        Some(Type::Number(NumberKind::Unsigned(_))),
                        Type::Number(NumberKind::Signed(to_w)),
                    ) => format!("({} as i{})", inner_code, to_w),

                    (
                        Some(Type::Number(NumberKind::Signed(_) | NumberKind::Unsigned(_))),
                        Type::Number(NumberKind::Float(32)),
                    ) => format!("({} as f32)", inner_code),
                    (
                        Some(Type::Number(NumberKind::Signed(_) | NumberKind::Unsigned(_))),
                        Type::Number(NumberKind::Float(64)),
                    ) => format!("({} as f64)", inner_code),

                    (
                        Some(Type::Number(NumberKind::Float(_))),
                        Type::Number(NumberKind::Signed(w)),
                    ) => format!("({}.round() as i{})", inner_code, w),
                    (
                        Some(Type::Number(NumberKind::Float(_))),
                        Type::Number(NumberKind::Unsigned(w)),
                    ) => format!("({}.round() as u{})", inner_code, w),

                    (
                        Some(Type::Number(NumberKind::Float(32))),
                        Type::Number(NumberKind::Float(64)),
                    ) => format!("({} as f64)", inner_code),
                    (
                        Some(Type::Number(NumberKind::Float(64))),
                        Type::Number(NumberKind::Float(32)),
                    ) => format!("({} as f32)", inner_code),

                    (
                        Some(Type::Number(NumberKind::Signed(w))),
                        Type::Number(NumberKind::BigInt),
                    ) => match w {
                        32 => format!("BigInt::from({} as i32)", inner_code),
                        64 => format!("BigInt::from({} as i64)", inner_code),
                        _ => format!("BigInt::from({} as i64)", inner_code),
                    },
                    (
                        Some(Type::Number(NumberKind::Unsigned(w))),
                        Type::Number(NumberKind::BigInt),
                    ) => match w {
                        32 => format!("BigInt::from({} as u32)", inner_code),
                        64 => format!("BigInt::from({} as u64)", inner_code),
                        _ => format!("BigInt::from({} as u64)", inner_code),
                    },

                    // ==========================================================
                    // Bool → Number (for arithmetic operations)
                    // ==========================================================
                    (Some(Type::Bool), Type::Number(NumberKind::Float(32))) => {
                        format!("({} as u8 as f32)", inner_code)
                    }
                    (Some(Type::Bool), Type::Number(NumberKind::Float(64))) => {
                        format!("({} as u8 as f64)", inner_code)
                    }
                    (Some(Type::Bool), Type::Number(NumberKind::Signed(w))) => {
                        format!("({} as i{})", inner_code, w)
                    }
                    (Some(Type::Bool), Type::Number(NumberKind::Unsigned(w))) => {
                        format!("({} as u{})", inner_code, w)
                    }

                    // ==========================================================
                    // JSON Value (ContainerKind::Object) → Anything
                    // ==========================================================
                    (Some(Type::Object), target) => {
                        use NumberKind::*;
                        match target {
                            Type::Number(Signed(w)) => {
                                format!("{}.as_i64().unwrap_or_default() as i{}", inner_code, w)
                            }

                            Type::Number(Unsigned(w)) => {
                                format!("{}.as_u64().unwrap_or_default() as u{}", inner_code, w)
                            }

                            Type::Number(Float(w)) => match w {
                                32 => format!("{}.as_f64().unwrap_or_default() as f32", inner_code),
                                64 => format!("{}.as_f64().unwrap_or_default()", inner_code),
                                _ => format!("{}.as_f64().unwrap_or_default() as f64", inner_code),
                            },

                            Type::String => {
                                format!("{}.as_str().unwrap_or_default().to_string()", inner_code)
                            }

                            Type::Bool => format!("{}.as_bool().unwrap_or_default()", inner_code),

                            Type::Number(NumberKind::BigInt) => {
                                // Value to BigInt: try as string first (JSON serializes BigInt as string)
                                format!(
                                    "{}.as_str().map(|s| s.parse::<BigInt>().unwrap_or_default()).unwrap_or_else(|| BigInt::from({}.as_i64().unwrap_or_default()))",
                                    inner_code, inner_code
                                )
                            }

                            Type::Number(NumberKind::Decimal) => {
                                // Value to Decimal: try as string first, then use FromPrimitive for f64
                                format!(
                                    "{}.as_str().map(|s| Decimal::from_str(s).unwrap_or_default()).unwrap_or_else(|| rust_decimal::prelude::FromPrimitive::from_f64({}.as_f64().unwrap_or_default()).unwrap_or_default())",
                                    inner_code, inner_code
                                )
                            }

                            Type::Custom(name) => {
                                // Check if this is a cast from get_child_by_name (Option<Uuid>) to a node type
                                // Pattern: self.get_node("name") as Sprite2D
                                // Note: get_parent() now returns Node directly, so it doesn't need special handling here
                                if let Expr::ApiCall(
                                    ApiModule::NodeSugar(NodeSugarApi::GetChildByName),
                                    _,
                                ) = inner.as_ref()
                                {
                                // get_child_by_name returns Option<Uuid>, casting to node type just unwraps the Option
                                // Property access will use read_node/mutate_node under the hood
                                // Unwrap the Option - panic if child not found (user expects this behavior)
                                format!(
                                    "{}.unwrap_or_else(|| panic!(\"Child node not found\"))",
                                    inner_code
                                )
                                } else {
                                    format!(
                                        "serde_json::from_value::<{}>({}.clone()).unwrap_or_default()",
                                        name, inner_code
                                    )
                                }
                            }

                            Type::Container(ContainerKind::Array, inner) => format!(
                                "serde_json::from_value::<Vec<{}>>({}).unwrap_or_default()",
                                inner
                                    .get(0)
                                    .map_or("Value".to_string(), |t| t.to_rust_type()),
                                inner_code
                            ),

                            Type::Container(ContainerKind::Map, inner) => format!(
                                "serde_json::from_value::<HashMap<{}, {}>>({}).unwrap_or_default()",
                                inner
                                    .get(0)
                                    .map_or("String".to_string(), |k| k.to_rust_type()),
                                inner
                                    .get(1)
                                    .map_or("Value".to_string(), |v| v.to_rust_type()),
                                inner_code
                            ),

                            _ => format!("{}.clone()", inner_code),
                        }
                    }

                    // Option<Uuid> (from get_child_by_name or get_parent) to Custom type (node type)
                    // Pattern: self.get_node("name") as Sprite2D or col.get_parent() as Sprite2D
                    (Some(Type::Custom(from_name)), Type::Custom(to_name))
                        if from_name == "UuidOption" =>
                    {
                        // Special case: self.get_parent() as NodeType
                        // Read directly from self_node.parent.unwrap() instead of calling api.get_parent()
                        if let Expr::ApiCall(ApiModule::NodeSugar(NodeSugarApi::GetParent), args) = inner.as_ref() {
                            if let Some(Expr::SelfAccess) = args.get(0) {
                                // This is self.get_parent() - read directly from self_node.parent
                                let node_type_name = &script.node_type;
                                if is_node_type(to_name) {
                                    // Cast to node type - read parent directly
                                    return format!(
                                        "api.read_node(self.id, |self_node: &{}| self_node.parent.unwrap())",
                                        node_type_name
                                    );
                                }
                            }
                        }
                        // get_child_by_name and get_parent return Option<Uuid>, cast to node type
                        // Keep it as Option<Uuid> - property access will unwrap and use read_node/mutate_node
                        // The variable will be stored as Option<Uuid> and unwrapped when accessing properties
                        inner_code
                    }

                    // UIElement (from get_element) to specific UI element type
                    // Pattern: ui_node.get_element("bob") as UIText
                    (Some(Type::Custom(from_name)), Type::Custom(to_name))
                        if from_name == "UIElement" =>
                    {
                        // Check if this is a get_element call being cast
                        // Convert to get_element_clone call
                        if inner_code.contains(".get_element(") {
                            // Replace .get_element( with .get_element_clone::<Type>(
                            let new_code = inner_code.replace(
                                ".get_element(",
                                &format!(".get_element_clone::<{}>(", to_name),
                            );
                            format!("{}", new_code)
                        } else {
                            // Fallback for other UIElement casts
                            format!("{}.clone()", inner_code)
                        }
                    }

                    // Node to specific node type (e.g., Node as Sprite2D)
                    (Some(Type::Node(_)), Type::Custom(to_name)) if is_node_type(to_name) => {
                        // Cast from base Node to specific node type
                        // Casting to a node type just returns the UUID - property access will use read_node/mutate_node
                        // If inner_code is "self", use self.id. Otherwise, it's already a node ID variable (bob_id)
                        if inner_code == "self" {
                            "self.id".to_string()
                        } else {
                            inner_code.clone()
                        }
                    }
                    
                    // Custom type to Custom type (struct casts)
                    (Some(Type::Custom(from_name)), Type::Custom(to_name)) => {
                        if from_name == to_name {
                            inner_code
                        } else {
                            // Use serde_json conversion for struct casts - clone if it's a MemberAccess to avoid move
                            let cloned_code = if inner_code.contains("self.")
                                && !inner_code.contains(".clone()")
                            {
                                format!("{}.clone()", inner_code)
                            } else {
                                inner_code
                            };
                            format!(
                                "serde_json::from_value::<{}>(serde_json::to_value(&{}).unwrap_or_default()).unwrap_or_default()",
                                to_name, cloned_code
                            )
                        }
                    }

                    // Custom type to Custom type (from any type)
                    (_, Type::Custom(to_name)) => {
                        // Clone if it's a MemberAccess to avoid move
                        let cloned_code =
                            if inner_code.contains("self.") && !inner_code.contains(".clone()") {
                                format!("{}.clone()", inner_code)
                            } else {
                                inner_code
                            };
                        format!(
                            "serde_json::from_value::<{}>(serde_json::to_value(&{}).unwrap_or_default()).unwrap_or_default()",
                            to_name, cloned_code
                        )
                    }

                    _ => {
                        // For non-primitive types, try .into() instead of as cast
                        // This handles String -> CowStr and other conversions
                        if matches!(target_type, Type::CowStr | Type::String | Type::Custom(_)) {
                            format!("{}.into()", inner_code)
                        } else {
                            eprintln!(
                                "Warning: Unhandled cast from {:?} to {:?}",
                                inner_type, target_type
                            );
                            format!("({} as {})", inner_code, target_type.to_rust_type())
                        }
                    }
                }
            }
            Expr::Index(base, key) => {
                let base_type = script.infer_expr_type(base, current_func);
                let base_code = base.to_rust(needs_self, script, None, current_func, None);
                // Key type inference for Map access should be specific, otherwise it defaults to String
                let key_code =
                    if let Some(Type::Container(ContainerKind::Map, inner_types)) = &base_type {
                        let key_ty = inner_types.get(0).unwrap_or(&Type::String);
                        key.to_rust(needs_self, script, Some(key_ty), current_func, None)
                    } else {
                        // For arrays or objects, assume string key for now (or other default)
                        key.to_rust(needs_self, script, Some(&Type::String), current_func, None)
                    };

                match base_type {
                    // ----------------------------------------------------------
                    // ✅ Typed HashMap<K,V>
                    // ----------------------------------------------------------
                    Some(Type::Container(ContainerKind::Map, ref inner_types)) => {
                        let key_ty = inner_types.get(0).unwrap_or(&Type::String);
                        // No need to re-infer key_code, already done above with correct type
                        let final_key_code = if *key_ty == Type::String {
                            // For String keys, convert the key to string if it's not already
                            let key_type = script.infer_expr_type(key, current_func);
                            if matches!(key_type, Some(Type::Number(_)) | Some(Type::Bool)) {
                                format!("{}.to_string().as_str()", key_code)
                            } else {
                                format!("{}.as_str()", key_code)
                            }
                        } else {
                            format!("&{}", key_code)
                        };
                        format!(
                            "{}.get({}).cloned().unwrap_or_default()",
                            base_code, final_key_code
                        )
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
                    Some(Type::Container(ContainerKind::Array, ref inner_types)) => {
                        let index_code = key.to_rust(
                            needs_self,
                            script,
                            Some(&Type::Number(NumberKind::Unsigned(32))),
                            current_func,
                            None,
                        );

                        // Check if this is a custom type array (polymorphic - stored as Vec<Value>)
                        if let Some(inner_type) = inner_types.get(0) {
                            match inner_type {
                                Type::Custom(_) => {
                                    // Custom type arrays are stored as Vec<Value>, auto-cast on access
                                    let rust_type = inner_type.to_rust_type();
                                    format!(
                                        "serde_json::from_value::<{}>({}.get({} as usize).cloned().unwrap_or_default()).unwrap_or_default()",
                                        rust_type, base_code, index_code
                                    )
                                }
                                _ => {
                                    // Primitive types - direct access
                                    format!(
                                        "{}.get({} as usize).cloned().unwrap_or_default()",
                                        base_code, index_code
                                    )
                                }
                            }
                        } else {
                            // No inner type specified - treat as Vec<Value>
                            format!(
                                "{}.get({} as usize).cloned().unwrap_or_default()",
                                base_code, index_code
                            )
                        }
                    }

                    // ----------------------------------------------------------
                    // ✅ Fixed-size array: [T; N]
                    // ----------------------------------------------------------
                    Some(Type::Container(ContainerKind::FixedArray(_), _)) => {
                        // inner_types not needed for codegen here
                        let index_code = key.to_rust(
                            needs_self,
                            script,
                            Some(&Type::Number(NumberKind::Unsigned(32))),
                            current_func,
                            None,
                        );
                        // Result from .get() is cloned, so it's a T or Value, handled by infer_expr_type
                        format!(
                            "{}.get({} as usize).cloned().unwrap_or_default()",
                            base_code, index_code
                        )
                    }

                    // ----------------------------------------------------------
                    // Invalid or unsupported index base
                    // ----------------------------------------------------------
                    Some(Type::Custom(_)) => "/* invalid index on struct */".to_string(),
                    _ => "/* unsupported index expression */".to_string(),
                }
            }
            Expr::ObjectLiteral(items) => {
                let pairs: Vec<_> = items
                    .iter()
                    .map(|(k, v)| {
                        format!(
                            "\"{}\": {}",
                            k.as_deref().unwrap_or(""),
                            v.to_rust(needs_self, script, None, current_func, None)
                        )
                    })
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
                target.contains_api_call(script) || args.iter().any(|a| a.contains_api_call(script))
            }
            Expr::ContainerLiteral(_, data) => match data {
                ContainerLiteralData::Array(elements) => {
                    elements.iter().any(|e| e.contains_api_call(script))
                }
                ContainerLiteralData::Map(pairs) => pairs
                    .iter()
                    .any(|(k, v)| k.contains_api_call(script) || v.contains_api_call(script)),
                ContainerLiteralData::FixedArray(_, elements) => {
                    elements.iter().any(|e| e.contains_api_call(script))
                }
            },
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
            Literal::Number(raw) => match expected_type {
                Some(Type::Number(NumberKind::Signed(w))) => format!("{}i{}", raw, w),
                Some(Type::Number(NumberKind::Unsigned(w))) => format!("{}u{}", raw, w),
                Some(Type::Number(NumberKind::Float(w))) => match w {
                    32 => format!("{}f32", raw),
                    64 => format!("{}f64", raw),
                    _ => format!("{}f32", raw),
                },
                Some(Type::Number(NumberKind::Decimal)) => {
                    format!("Decimal::from_str(\"{}\").unwrap()", raw)
                }
                Some(Type::Number(NumberKind::BigInt)) => {
                    format!("BigInt::from_str(\"{}\").unwrap()", raw)
                }
                _ => format!("{}f32", raw),
            },

            Literal::String(s) => {
                match expected_type {
                    // For Cow<'static, str>, use Cow::Borrowed with string literal
                    Some(Type::CowStr) => {
                        format!("Cow::Borrowed(\"{}\")", s)
                    }
                    // For Option<CowStr>, use Some(Cow::Borrowed(...))
                    Some(Type::Option(inner)) if matches!(inner.as_ref(), Type::CowStr) => {
                        format!("Some(Cow::Borrowed(\"{}\"))", s)
                    }
                    // For StrRef (&str), use string literal
                    Some(Type::StrRef) => format!("\"{}\"", s),
                    // For String or unknown, create owned String
                    _ => format!("String::from(\"{}\")", s),
                }
            }

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
            Op::Lt => "<",
            Op::Gt => ">",
            Op::Le => "<=",
            Op::Ge => ">=",
            Op::Eq => "==",
            Op::Ne => "!=",
        }
    }

    fn to_rust_assign(&self) -> &'static str {
        match self {
            Op::Add => "+",
            Op::Sub => "-",
            Op::Mul => "*",
            Op::Div => "/",
            Op::Lt | Op::Gt | Op::Le | Op::Ge | Op::Eq | Op::Ne => {
                panic!("Comparison operators cannot be used in assignment")
            }
        }
    }
}

pub fn implement_script_boilerplate(
    struct_name: &str,
    script_vars: &[Variable],
    functions: &[Function],
    attributes_map: &std::collections::HashMap<String, Vec<String>>,
) -> String {
    let mut out = String::with_capacity(8192);
    let mut get_entries = String::with_capacity(512);
    let mut set_entries = String::with_capacity(512);
    let mut apply_entries = String::with_capacity(512);
    let mut dispatch_entries = String::with_capacity(4096);

    let mut public_var_count = 0;
    let mut exposed_var_count = 0;
    
    // Detect which lifecycle methods are implemented
    let has_init = functions.iter().any(|f| f.is_trait_method && f.name.to_lowercase() == "init");
    let has_update = functions.iter().any(|f| f.is_trait_method && f.name.to_lowercase() == "update");
    let has_fixed_update = functions.iter().any(|f| f.is_trait_method && f.name.to_lowercase() == "fixed_update");
    let has_draw = functions.iter().any(|f| f.is_trait_method && f.name.to_lowercase() == "draw");
    
    // Build the flags value
    let mut flags_value = 0u8;
    if has_init {
        flags_value |= 1; // ScriptFlags::HAS_INIT
    }
    if has_update {
        flags_value |= 2; // ScriptFlags::HAS_UPDATE
    }
    if has_fixed_update {
        flags_value |= 4; // ScriptFlags::HAS_FIXED_UPDATE
    }
    if has_draw {
        flags_value |= 8; // ScriptFlags::HAS_DRAW
    }

    //----------------------------------------------------
    // Generate VAR GET, SET, APPLY tables
    //----------------------------------------------------
    for var in script_vars {
        let name = &var.name;
        let renamed_name = rename_variable(name, var.typ.as_ref());
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
                                Some(serde_json::to_value(&script.{renamed_name}).unwrap_or_default())
                            }},"
                        )
                        .unwrap();
                    }
                }
            } else {
                writeln!(
                    get_entries,
                    "        {var_id}u64 => |script: &{struct_name}| -> Option<Value> {{
                        Some(json!(script.{renamed_name}))
                    }},"
                )
                .unwrap();
            }

            // ------------------------------
            // Special casing for Containers (SET)
            // ------------------------------
            if let Some(Type::Container(kind, elem_types)) = &var.typ {
                match kind {
                    ContainerKind::Array => {
                        let elem_ty = elem_types.get(0).unwrap_or(&Type::Object);
                        let elem_rs = elem_ty.to_rust_type();
                        let field_rust_type = var
                            .typ
                            .as_ref()
                            .map(|t| t.to_rust_type())
                            .unwrap_or_default();
                        // Check if field is Vec<Value> (for custom types or Object)
                        let is_value_vec = field_rust_type == "Vec<Value>"
                            || *elem_ty == Type::Object
                            || matches!(elem_ty, Type::Custom(_));
                        if *elem_ty != Type::Object {
                            if is_value_vec {
                                // Convert Vec<T> to Vec<Value>
                                writeln!(
                                    set_entries,
                                    "        {var_id}u64 => |script: &mut {struct_name}, val: Value| -> Option<()> {{
                                    if let Ok(vec_typed) = serde_json::from_value::<Vec<{elem_rs}>>(val) {{
                                        script.{renamed_name} = vec_typed.into_iter().map(|x| serde_json::to_value(x).unwrap_or_default()).collect();
                                        return Some(());
                                    }}
                                    None
                                }},"
                                ).unwrap();
                            } else {
                                writeln!(
                                    set_entries,
                                    "        {var_id}u64 => |script: &mut {struct_name}, val: Value| -> Option<()> {{
                                    if let Ok(vec_typed) = serde_json::from_value::<Vec<{elem_rs}>>(val) {{
                                        script.{renamed_name} = vec_typed;
                                        return Some(());
                                    }}
                                    None
                                }},"
                                ).unwrap();
                            }
                        } else {
                            writeln!(
                                set_entries,
                                "        {var_id}u64 => |script: &mut {struct_name}, val: Value| -> Option<()> {{
                                    if let Some(v) = val.as_array() {{
                                        script.{renamed_name} = v.clone();
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
                                        script.{renamed_name} = arr_typed;
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
                                        script.{renamed_name} = out;
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

                        let field_rust_type = var
                            .typ
                            .as_ref()
                            .map(|t| t.to_rust_type())
                            .unwrap_or_default();
                        // Check if field is HashMap<String, Value> (for custom types or Object)
                        let is_value_map = field_rust_type == "HashMap<String, Value>"
                            || *val_ty == Type::Object
                            || matches!(val_ty, Type::Custom(_));
                        if *val_ty != Type::Object {
                            if is_value_map {
                                // Convert HashMap<K, T> to HashMap<String, Value>
                                writeln!(
                                    set_entries,
                                    "        {var_id}u64 => |script: &mut {struct_name}, val: Value| -> Option<()> {{
                                    if let Ok(map_typed) = serde_json::from_value::<HashMap<{key_rs}, {val_rs}>>(val) {{
                                        script.{renamed_name} = map_typed.into_iter().map(|(k, v)| (k, serde_json::to_value(v).unwrap_or_default())).collect();
                                        return Some(());
                                    }}
                                    None
                                }},"
                                ).unwrap();
                            } else {
                                writeln!(
                                    set_entries,
                                    "        {var_id}u64 => |script: &mut {struct_name}, val: Value| -> Option<()> {{
                                    if let Ok(map_typed) = serde_json::from_value::<HashMap<{key_rs}, {val_rs}>>(val) {{
                                        script.{renamed_name} = map_typed;
                                        return Some(());
                                    }}
                                    None
                                }},"
                                ).unwrap();
                            }
                        } else {
                            writeln!(
                                set_entries,
                                "        {var_id}u64 => |script: &mut {struct_name}, val: Value| -> Option<()> {{
                                    if let Some(v) = val.as_object() {{
                                        script.{renamed_name} = v.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
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
                                script.{renamed_name} = v;
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
                                script.{renamed_name} = v{conv};
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
                        let field_rust_type = var
                            .typ
                            .as_ref()
                            .map(|t| t.to_rust_type())
                            .unwrap_or_default();
                        // Check if field is Vec<Value> (for custom types or Object)
                        let is_value_vec = field_rust_type == "Vec<Value>"
                            || *elem_ty == Type::Object
                            || matches!(elem_ty, Type::Custom(_));
                        if *elem_ty != Type::Object {
                            if is_value_vec {
                                // Convert Vec<T> to Vec<Value>
                                writeln!(
                                    apply_entries,
                                    "        {var_id}u64 => |script: &mut {struct_name}, val: &Value| {{
                                    if let Ok(vec_typed) = serde_json::from_value::<Vec<{elem_rs}>>(val.clone()) {{
                                        script.{renamed_name} = vec_typed.into_iter().map(|x| serde_json::to_value(x).unwrap_or_default()).collect();
                                    }}
                                }},"
                                ).unwrap();
                            } else {
                                writeln!(
                                    apply_entries,
                                    "        {var_id}u64 => |script: &mut {struct_name}, val: &Value| {{
                                    if let Ok(vec_typed) = serde_json::from_value::<Vec<{elem_rs}>>(val.clone()) {{
                                        script.{renamed_name} = vec_typed;
                                    }}
                                }},"
                                ).unwrap();
                            }
                        } else {
                            writeln!(
                                apply_entries,
                                "        {var_id}u64 => |script: &mut {struct_name}, val: &Value| {{
                                    if let Some(v) = val.as_array() {{
                                        script.{renamed_name} = v.clone();
                                    }}
                                }},"
                            )
                            .unwrap();
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
                                        script.{renamed_name} = arr_typed;
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
                                        script.{renamed_name} = out;
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

                        let field_rust_type = var
                            .typ
                            .as_ref()
                            .map(|t| t.to_rust_type())
                            .unwrap_or_default();
                        // Check if field is HashMap<String, Value> (for custom types or Object)
                        let is_value_map = field_rust_type == "HashMap<String, Value>"
                            || *val_ty == Type::Object
                            || matches!(val_ty, Type::Custom(_));
                        if *val_ty != Type::Object {
                            if is_value_map {
                                // Convert HashMap<K, T> to HashMap<String, Value>
                                writeln!(
                                    apply_entries,
                                    "        {var_id}u64 => |script: &mut {struct_name}, val: &Value| {{
                                    if let Ok(map_typed) = serde_json::from_value::<HashMap<{key_rs}, {val_rs}>>(val.clone()) {{
                                        script.{renamed_name} = map_typed.into_iter().map(|(k, v)| (k, serde_json::to_value(v).unwrap_or_default())).collect();
                                    }}
                                }},"
                                ).unwrap();
                            } else {
                                writeln!(
                                    apply_entries,
                                    "        {var_id}u64 => |script: &mut {struct_name}, val: &Value| {{
                                    if let Ok(map_typed) = serde_json::from_value::<HashMap<{key_rs}, {val_rs}>>(val.clone()) {{
                                        script.{renamed_name} = map_typed;
                                    }}
                                }},"
                                ).unwrap();
                            }
                        } else {
                            writeln!(
                                apply_entries,
                                "        {var_id}u64 => |script: &mut {struct_name}, val: &Value| {{
                                    if let Some(v) = val.as_object() {{
                                        script.{renamed_name} = v.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
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
                                script.{renamed_name} = v;
                            }}
                        }},"
                    )
                    .unwrap();
                } else {
                    writeln!(
                        apply_entries,
                        "        {var_id}u64 => |script: &mut {struct_name}, val: &Value| {{
                            if let Some(v) = val.{accessor}() {{
                                script.{renamed_name} = v{conv};
                            }}
                        }},"
                    )
                    .unwrap();
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
        let renamed_func_name = rename_function(func_name);

        let mut param_parsing = String::new();
        let mut param_list = String::new();

        if !func.params.is_empty() {
            for (i, param) in func.params.iter().enumerate() {
                // Rename parameter: node types get _id suffix, others keep original name
                let param_name = rename_variable(&param.name, Some(&param.typ));
                let parse_code = match &param.typ {
                    Type::String => format!(
                        "let {param_name} = params.get({i})
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();\n"
                    ),
                    Type::Number(NumberKind::Signed(w)) => format!(
                        "let {param_name} = params.get({i})
                            .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
                            .unwrap_or_default() as i{w};\n"
                    ),
                    Type::Number(NumberKind::Unsigned(w)) => format!(
                        "let {param_name} = params.get({i})
                            .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
                            .unwrap_or_default() as u{w};\n"
                    ),
                    Type::Number(NumberKind::Float(32)) => format!(
                        "let {param_name} = params.get({i})
                            .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))
                            .unwrap_or_default() as f32;\n"
                    ),
                    Type::Number(NumberKind::Float(64)) => format!(
                        "let {param_name} = params.get({i})
                            .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))
                            .unwrap_or_default();\n"
                    ),
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
                    Type::Custom(tn) if is_node_type(tn) => {
                        // For node types, parse UUID from string (nodes are just UUIDs)
                        format!(
                            "let {param_name} = params.get({i})
                            .and_then(|v| v.as_str())
                            .and_then(|s| Uuid::parse_str(s).ok())
                            .unwrap_or_default();\n"
                        )
                    },
                    Type::Custom(tn) => format!(
                        "let {param_name} = params.get({i})
                            .and_then(|v| serde_json::from_value::<{tn}>(v.clone()).ok())
                            .unwrap_or_default();\n"
                    ),
                    Type::Node(_) => {
                        // Handle Type::Node variant - nodes are just UUIDs
                        format!(
                            "let {param_name} = params.get({i})
                            .and_then(|v| v.as_str())
                            .and_then(|s| Uuid::parse_str(s).ok())
                            .unwrap_or_default();\n"
                        )
                    },
                    Type::EngineStruct(EngineStructKind::Texture) => {
                        // Handle Texture - textures are just UUIDs
                        format!(
                            "let {param_name} = params.get({i})
                            .and_then(|v| v.as_str())
                            .and_then(|s| Uuid::parse_str(s).ok())
                            .unwrap_or_default();\n"
                        )
                    },
                    Type::Uuid => {
                        // Handle Uuid - parse from string
                        format!(
                            "let {param_name} = params.get({i})
                            .and_then(|v| v.as_str())
                            .and_then(|s| Uuid::parse_str(s).ok())
                            .unwrap_or_default();\n"
                        )
                    },
                    Type::Option(boxed) if matches!(boxed.as_ref(), Type::Uuid) => {
                        // Handle Option<Uuid> - parse from string, return None if parsing fails
                        format!(
                            "let {param_name} = params.get({i})
                            .and_then(|v| v.as_str())
                            .and_then(|s| Uuid::parse_str(s).ok());\n"
                        )
                    },
                    _ => format!("let {param_name} = Default::default();\n"),
                };
                param_parsing.push_str(&parse_code);
            }

            param_list = func
                .params
                .iter()
                .map(|p| rename_variable(&p.name, Some(&p.typ)))
                .collect::<Vec<_>>()
                .join(", ");
            param_list.push_str(", ");
        }

        write!(
            dispatch_entries,
            "        {func_id}u64 => | script: &mut {struct_name}, params: &[Value], api: &mut ScriptApi<'_>| {{
{param_parsing}            script.{renamed_func_name}({param_list}api);
        }},\n"
        )
        .unwrap();
    }

    // MEMBER_TO_ATTRIBUTES_MAP and ATTRIBUTE_TO_MEMBERS_MAP are generated once at the top in to_rust(),
    // not here in the boilerplate to avoid duplicates

    //----------------------------------------------------
    // FINAL OUTPUT
    //----------------------------------------------------
    write!(
        out,
        r#"
impl ScriptObject for {struct_name} {{
    fn set_id(&mut self, id: Uuid) {{
        self.id = id;
    }}

    fn get_id(&self) -> Uuid {{
        self.id
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

    fn call_function(
        &mut self,
        id: u64,
        api: &mut ScriptApi<'_>,
        params: &[Value],
    ) {{
        if let Some(f) = DISPATCH_TABLE.get(&id) {{
            f(self, params, api);
        }}
    }}

    // Attributes

    fn attributes_of(&self, member: &str) -> Vec<String> {{
        MEMBER_TO_ATTRIBUTES_MAP
            .get(member)
            .map(|attrs| attrs.iter().map(|s| s.to_string()).collect())
            .unwrap_or_default()
    }}

    fn members_with(&self, attribute: &str) -> Vec<String> {{
        ATTRIBUTE_TO_MEMBERS_MAP
            .get(attribute)
            .map(|members| members.iter().map(|s| s.to_string()).collect())
            .unwrap_or_default()
    }}

    fn has_attribute(&self, member: &str, attribute: &str) -> bool {{
        MEMBER_TO_ATTRIBUTES_MAP
            .get(member)
            .map(|attrs| attrs.iter().any(|a| *a == attribute))
            .unwrap_or(false)
    }}
    
    fn script_flags(&self) -> ScriptFlags {{
        ScriptFlags::new({flags_value})
    }}
}}

// =========================== Static PHF Dispatch Tables ===========================

static VAR_GET_TABLE: phf::Map<u64, fn(&{struct_name}) -> Option<Value>> =
    phf::phf_map! {{
{get_entries}
    }};

static VAR_SET_TABLE: phf::Map<u64, fn(&mut {struct_name}, Value) -> Option<()>> =
    phf::phf_map! {{
{set_entries}
    }};

static VAR_APPLY_TABLE: phf::Map<u64, fn(&mut {struct_name}, &Value)> =
    phf::phf_map! {{
{apply_entries}
    }};

static DISPATCH_TABLE: phf::Map<
    u64,
    fn(&mut {struct_name}, &[Value], &mut ScriptApi<'_>),
> = phf::phf_map! {{
{dispatch_entries}
    }};
"#,
        struct_name = struct_name,
        get_entries = get_entries,
        set_entries = set_entries,
        apply_entries = apply_entries,
        dispatch_entries = dispatch_entries,
        flags_value = flags_value,
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

    fs::write(&file_path, contents).map_err(|e| format!("Failed to write file: {}", e))?;

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

    // Check if this entry already exists in the phf_map!
    let existing_entry = format!("\"{}\" =>", lower_name);
    if !current_content.contains(&existing_entry) {
        let registry_line = format!(
            "    \"{}\" => {}_create_script as CreateFn,\n",
            lower_name, lower_name
        );
        current_content = current_content.replace(
            "    // __PERRO_REGISTRY__",
            &format!("{}    // __PERRO_REGISTRY__", registry_line),
        );
    }

    fs::write(&lib_rs_path, current_content)
        .map_err(|e| format!("Failed to update lib.rs: {}", e))?;

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

pub fn derive_rust_perro_script(
    project_path: &Path,
    code: &str,
    struct_name: &str,
) -> Result<(), String> {
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

    let mut variables = Vec::new();
    let mut attributes_map = std::collections::HashMap::new();

    // Parse attributes from doc comments: ///@Expose, ///@OtherAttr, etc.
    // This regex matches: ///@AttributeName followed by a field or function
    let attr_re =
        Regex::new(r"///\s*@(\w+)[^\n]*\n\s*(?:pub\s+)?(\w+)(?:\s*:\s*[^,\n}]+)?[,}]?").unwrap();

    // First, collect all attributes for fields
    for cap in attr_re.captures_iter(&struct_body) {
        let attr_name = cap[1].to_string();
        let member_name = cap[2].to_string();

        // Skip if it's the node field
        if member_name == "node" {
            continue;
        }

        attributes_map
            .entry(member_name.clone())
            .or_insert_with(Vec::new)
            .push(attr_name);
    }

    // Parse exposed fields (///@expose)
    let expose_re =
        Regex::new(r"///\s*@expose[^\n]*\n\s*(?:pub\s+)?(\w+)\s*:\s*([^,]+),?").unwrap();
    for cap in expose_re.captures_iter(&struct_body) {
        let name = cap[1].to_string();
        let typ = cap[2].trim().to_string();
        let mut is_pub = false;
        if cap[0].contains("pub") {
            is_pub = true;
        }

        // Ensure Expose attribute is in the map
        attributes_map
            .entry(name.clone())
            .or_insert_with(Vec::new)
            .push("Expose".to_string());

        variables.push(Variable {
            name: name.clone(),
            typ: Some(Variable::parse_type(&typ)),
            value: None,
            is_exposed: true,
            is_public: is_pub,
            attributes: attributes_map.get(&name).cloned().unwrap_or_default(),
            span: None,
        });
    }

    // Parse public fields (pub field: Type)
    let pub_re = Regex::new(r"pub\s+(\w+)\s*:\s*([^,\n}]+)").unwrap();
    for cap in pub_re.captures_iter(&struct_body) {
        let name = cap[1].to_string();
        if name == "node" || variables.iter().any(|v| v.name == name) {
            continue;
        }
        let typ = cap[2].trim().to_string();
        variables.push(Variable {
            name: name.clone(),
            typ: Some(Variable::parse_type(&typ)),
            value: None,
            is_exposed: false,
            is_public: true,
            attributes: attributes_map.get(&name).cloned().unwrap_or_default(),
            span: None,
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

    // FIRST: Parse trait methods from impl Script for StructName { ... } block
    // Use a simpler approach: find the impl block start, then scan for trait methods
    let impl_script_marker = format!("impl Script for {}", actual_struct_name);
    if let Some(start_pos) = code.find(&impl_script_marker) {
        // Find the opening brace
        if let Some(brace_pos) = code[start_pos..].find('{') {
            let block_start = start_pos + brace_pos;
            
            // Search for trait methods after this point (they must be before the next impl block or EOF)
            let next_impl_pos = code[block_start..].find("impl ")
                .map(|p| block_start + p)
                .unwrap_or(code.len());
            
            let search_region = &code[block_start..next_impl_pos];
            
            // Find init, update, fixed_update, draw methods
            let fn_re = Regex::new(r"fn\s+(init|update|fixed_update|draw)\s*\(").unwrap();

            for fn_cap in fn_re.captures_iter(search_region) {
                let fn_name = fn_cap[1].to_string();

                functions.push(Function {
                    name: fn_name.clone(),
                    is_trait_method: true,  // Mark as trait method for flag detection
                    params: vec![],
                    return_type: Type::Void,
                    uses_self: false,
                    cloned_child_nodes: Vec::new(),
                    body: vec![],
                    locals: vec![],
                    attributes: vec![],
                    is_on_signal: false,
                    signal_name: None,
                    span: None,
                });
            }
        }
    }

    // SECOND: Find impl StructNameScript { ... } blocks (non-trait methods)
    let impl_block_re = Regex::new(&format!(
        r"impl\s+{}\s*\{{([^}}]*(?:\{{[^}}]*\}}[^}}]*)*)\}}",
        regex::escape(&format!("{}Script", to_pascal_case(struct_name)))
    ))
    .unwrap();

    if let Some(impl_cap) = impl_block_re.captures(&code) {
        let impl_body = &impl_cap[1];

        // Parse attributes for functions: ///@AttributeName before fn function_name
        let fn_attr_re = Regex::new(r"///\s*@(\w+)[^\n]*\n\s*fn\s+(\w+)").unwrap();
        for attr_cap in fn_attr_re.captures_iter(impl_body) {
            let attr_name = attr_cap[1].to_string();
            let fn_name = attr_cap[2].to_string();
            attributes_map
                .entry(fn_name.clone())
                .or_insert_with(Vec::new)
                .push(attr_name);
        }

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

                    params.push(Param { name, typ, span: None });
                }
            }

            // Parse return type
            let return_type = if return_str.is_empty() {
                Type::Void
            } else {
                Variable::parse_type(return_str)
            };

            functions.push(Function {
                name: fn_name.clone(),
                is_trait_method: false,
                params,
                return_type,
                uses_self: false,
                span: None,
                cloned_child_nodes: Vec::new(), // Will be populated during analyze_self_usage
                body: vec![],
                locals: vec![],
                attributes: attributes_map.get(&fn_name).cloned().unwrap_or_default(),
                is_on_signal: false,
                signal_name: None,
            });
        }
    }

    let final_contents = if let Some(actual_fn_name) = extract_create_script_fn_name(&code) {
        let expected_fn_name = format!("{}_create_script", lower_name);
        code.replace(&actual_fn_name, &expected_fn_name)
    } else {
        code.to_string()
    };

    // Don't generate MEMBER_NAMES and ATTRIBUTES_MAP here - let the boilerplate generate them
    // to avoid duplicates. The boilerplate will add them after the struct definition.
    let mut injected_code = final_contents.clone();
    let marker_pos = marker_re
        .find(&final_contents)
        .map(|m| m.end())
        .unwrap_or(0);
    let struct_pos = final_contents[marker_pos..]
        .find("struct ")
        .map(|p| marker_pos + p)
        .unwrap_or(0);

    // No need to inject/fix attributes field - we use MEMBER_TO_ATTRIBUTES_MAP directly in trait methods

    // For Rust scripts, remove any existing MEMBER_TO_ATTRIBUTES_MAP and ATTRIBUTE_TO_MEMBERS_MAP, then generate them once at the top
    // Match multiline from "static MEMBER_TO_ATTRIBUTES_MAP" to the closing "};"
    let member_to_attributes_map_re =
        Regex::new(r"(?s)static\s+MEMBER_TO_ATTRIBUTES_MAP\s*:.*?};").unwrap();
    // Also match old ATTRIBUTES_MAP name for backwards compatibility
    let attributes_map_re = Regex::new(r"(?s)static\s+ATTRIBUTES_MAP\s*:.*?};").unwrap();
    // Match multiline from "static ATTRIBUTE_TO_MEMBERS_MAP" to the closing "};"
    let attribute_to_members_map_re =
        Regex::new(r"(?s)static\s+ATTRIBUTE_TO_MEMBERS_MAP\s*:.*?};").unwrap();
    // Also remove any old MEMBER_NAMES if it exists
    let member_names_re = Regex::new(r"(?s)(pub\s+)?static\s+MEMBER_NAMES\s*:.*?];").unwrap();

    let mut cleaned_code = injected_code.clone();
    cleaned_code = member_names_re.replace_all(&cleaned_code, "").to_string();
    cleaned_code = member_to_attributes_map_re
        .replace_all(&cleaned_code, "")
        .to_string();
    cleaned_code = attributes_map_re.replace_all(&cleaned_code, "").to_string();
    cleaned_code = attribute_to_members_map_re
        .replace_all(&cleaned_code, "")
        .to_string();

    // Generate MEMBER_TO_ATTRIBUTES_MAP and ATTRIBUTE_TO_MEMBERS_MAP once at the top (before struct) - no need for separate MEMBER_NAMES
    // Build reverse index: attribute -> members
    let mut attribute_to_members: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    let mut attributes_map_code = String::new();
    attributes_map_code.push_str("static MEMBER_TO_ATTRIBUTES_MAP: Map<&'static str, &'static [&'static str]> = phf_map! {\n");
    for var in &variables {
        let attrs = attributes_map
            .get(&var.name)
            .cloned()
            .unwrap_or_else(|| var.attributes.clone());
        // Only store members that have attributes
        if !attrs.is_empty() {
            write!(attributes_map_code, "    \"{}\" => &[", var.name).unwrap();
            for (i, attr) in attrs.iter().enumerate() {
                if i > 0 {
                    attributes_map_code.push_str(", ");
                }
                write!(attributes_map_code, "\"{}\"", attr).unwrap();
                attribute_to_members
                    .entry(attr.clone())
                    .or_insert_with(Vec::new)
                    .push(var.name.clone());
            }
            attributes_map_code.push_str("],\n");
        }
    }
    for func in &functions {
        // Suffix function names with "()" to differentiate from variables
        let func_key = format!("{}()", func.name);
        let attrs = attributes_map
            .get(&func.name)
            .cloned()
            .unwrap_or_else(|| func.attributes.clone());
        // Only store members that have attributes
        if !attrs.is_empty() {
            write!(attributes_map_code, "    \"{}\" => &[", func_key).unwrap();
            for (i, attr) in attrs.iter().enumerate() {
                if i > 0 {
                    attributes_map_code.push_str(", ");
                }
                write!(attributes_map_code, "\"{}\"", attr).unwrap();
                attribute_to_members
                    .entry(attr.clone())
                    .or_insert_with(Vec::new)
                    .push(func_key.clone());
            }
            attributes_map_code.push_str("],\n");
        }
    }
    attributes_map_code.push_str("};\n\n");

    // Generate reverse index for O(1) attribute lookups
    attributes_map_code.push_str("static ATTRIBUTE_TO_MEMBERS_MAP: Map<&'static str, &'static [&'static str]> = phf_map! {\n");
    for (attr, members) in &attribute_to_members {
        write!(attributes_map_code, "    \"{}\" => &[", attr).unwrap();
        for (i, member) in members.iter().enumerate() {
            if i > 0 {
                attributes_map_code.push_str(", ");
            }
            write!(attributes_map_code, "\"{}\"", member).unwrap();
        }
        attributes_map_code.push_str("],\n");
    }
    attributes_map_code.push_str("};\n\n");

    // Find struct position in cleaned code
    let marker_pos = marker_re.find(&cleaned_code).map(|m| m.end()).unwrap_or(0);
    let struct_pos = cleaned_code[marker_pos..]
        .find("struct ")
        .map(|p| marker_pos + p)
        .unwrap_or(0);

    // Inject MEMBER_TO_ATTRIBUTES_MAP and ATTRIBUTE_TO_MEMBERS_MAP before the struct definition
    let mut final_code = cleaned_code;
    if struct_pos > 0 {
        final_code.insert_str(struct_pos, &attributes_map_code);
    }

    let boilerplate =
        implement_script_boilerplate(&actual_struct_name, &variables, &functions, &attributes_map);
    let combined = format!("{}\n\n{}", final_code, boilerplate);

    write_to_crate(project_path, &combined, struct_name)
}
