// Type inference for Script AST
use crate::api_modules::*;
use crate::ast::*;
use crate::scripting::ast::{ContainerKind, Expr, Literal, NumberKind, Type};
use crate::structs::engine_registry::ENGINE_REGISTRY;
use crate::structs::engine_structs::EngineStruct as EngineStructKind;

use super::cache::{get_cached_type, set_cached_type};
use super::utils::{is_node_type, string_to_node_type};

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
        if expr == "self" && matches!(to, Type::Uuid) {
            return "self.id".to_string();
        }
        
        // Direct handling for common conversions
        match (from, to) {
            (from_ty, Type::Option(inner)) if from_ty == inner.as_ref() => {
                return format!("Some({})", expr);
            }
            (Number(Signed(_) | Unsigned(_)), Number(Float(32))) => {
                return format!("({} as f32)", expr);
            }
            (Number(Signed(_) | Unsigned(_)), Number(Float(64))) => {
                return format!("({} as f64)", expr);
            }
            // String -> StrRef conversion
            (String, StrRef) => {
                return format!("{}.as_str()", expr);
            }
            // StrRef -> String conversion
            (StrRef, String) => {
                return format!("{}.to_string()", expr);
            }
            _ => {}
        }
        
        // Special case: if expr is already the target type (e.g., c_par_id is already Uuid), no cast needed
        // Check if expr ends with _id and target is Uuid (node variables are already Uuid)
        if expr.ends_with("_id") && matches!(to, Type::Uuid) {
            return expr.to_string();
        }
        
        // Special case: if expr is a variable name that's already the target type, no cast needed
        // This prevents unnecessary casts like (c_par_id as Uuid) when c_par_id is already Uuid
        if !expr.contains(' ') && !expr.contains('(') && !expr.contains('.') {
            // It's a simple variable name - check if we can skip the cast
            // For Uuid types, variables ending in _id are already Uuid
            if matches!(to, Type::Uuid) && (expr.ends_with("_id") || expr == "self.id") {
                return expr.to_string();
            }
        }
        
        // For now, use simple cast syntax
        // Complex casts will be handled by the Expr::Cast implementation in legacy
        format!("({} as {})", expr, to.to_rust_type())
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

        // Check cache first for performance
        let cache_key = expr as *const Expr as usize;
        if let Some(cached) = get_cached_type(cache_key) {
            // eprintln!("[INFER_TYPE] CACHE HIT for expr {:?} -> {:?}", expr, cached);
            return cached;
        }
        // eprintln!("[INFER_TYPE] CACHE MISS for expr {:?}", expr);

        let result = match expr {
            Expr::Literal(lit) => {
                let inferred = self.infer_literal_type(lit, None);
                // eprintln!("[INFER_TYPE] Literal {:?} -> {:?}", lit, inferred);
                inferred
            },
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
                    // 3. Check if it's a loop variable
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
            Expr::Range(_, _) => {
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
                    _ => Some(Number(NumberKind::Float(32))),
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
                        return params.get(1).cloned();
                    }
                    Some(Type::Object)
                }
                ApiModule::ArrayOp(ArrayApi::Pop) => {
                    if let Some(Type::Container(ContainerKind::Array, ref params)) =
                        self.infer_expr_type(&args[0], current_func)
                    {
                        return params.get(0).cloned();
                    }
                    Some(Type::Object)
                }
                _ => api.return_type(),
            },
            Expr::StructNew(ty_name, _fields) => {
                if let Some(node_type) = string_to_node_type(ty_name) {
                    Some(Type::Node(node_type))
                } else if let Some(engine_struct) = EngineStructKind::from_string(ty_name) {
                    Some(Type::EngineStruct(engine_struct))
                } else {
                    Some(Custom(ty_name.clone()))
                }
            }
            Expr::SelfAccess => {
                if let Some(node_type) = string_to_node_type(&self.node_type) {
                    Some(Type::Node(node_type))
                } else {
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
                    Type::Container(container_kind, inner_types) => {
                        match container_kind {
                            ContainerKind::Array => {
                                if inner_types.first() == Some(&Type::Object) {
                                    Some(Type::Object)
                                } else {
                                    inner_types.first().cloned()
                                }
                            }
                            ContainerKind::Map => {
                                if inner_types.last() == Some(&Type::Object) {
                                    Some(Type::Object)
                                } else {
                                    inner_types.last().cloned()
                                }
                            }
                            ContainerKind::FixedArray(_) => {
                                inner_types.first().cloned()
                            }
                        }
                    }
                    Type::Object => Some(Type::Object),
                    _ => None,
                }
            }
            Expr::BaseAccess => Some(Custom(self.node_type.clone())),
            _ => None,
        };

        // Cache the result
        // eprintln!("[INFER_TYPE] CACHING result for expr {:?} -> {:?}", expr, result);
        set_cached_type(cache_key, result.clone());

        result
    }

    pub(crate) fn infer_literal_type(&self, lit: &Literal, expected_type: Option<&Type>) -> Option<Type> {
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
                match expected_type {
                    Some(Type::CowStr) => Some(Type::CowStr),
                    Some(Type::StrRef) => Some(Type::StrRef),
                    _ => Some(Type::String),
                }
            }
        }
    }

    pub(crate) fn promote_types(&self, left: &Type, right: &Type) -> Option<Type> {
        if left == right {
            return Some(left.clone());
        }

        match (left, right) {
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

    pub(crate) fn get_member_type(&self, base_type: &Type, member: &str) -> Option<Type> {
        fn get_struct_field_type_recursive<'a>(
            structs: &'a [StructDef],
            struct_name: &str,
            field_name: &str,
        ) -> Option<Type> {
            let struct_def = structs.iter().find(|s| s.name == struct_name)?;

            if let Some(f) = struct_def.fields.iter().find(|f| f.name == field_name) {
                return Some(f.typ.clone());
            }

            if let Some(ref base_name) = struct_def.base {
                if structs.iter().any(|b| &b.name == base_name) {
                    return get_struct_field_type_recursive(structs, base_name, field_name);
                }
            }
            None
        }

        match base_type {
            Type::Node(node_type) => {
                let rust_field = ENGINE_REGISTRY.resolve_field_name(node_type, member);
                ENGINE_REGISTRY.get_field_type_node(node_type, &rust_field)
            }
            Type::Uuid => {
                let nodes_with_field = ENGINE_REGISTRY.find_nodes_with_field(member);
                if nodes_with_field.is_empty() {
                    return None;
                }
                if let Some(first_node) = nodes_with_field.first() {
                    ENGINE_REGISTRY.get_field_type_node(first_node, member)
                } else {
                    None
                }
            }
            Type::EngineStruct(engine_struct) => {
                ENGINE_REGISTRY.get_field_type_struct(engine_struct, member)
            }
            Type::Custom(type_name) => {
                if type_name == "ParentType" {
                    match member {
                        "id" => return Some(Type::Uuid),
                        "node_type" => return Some(Type::Custom("NodeType".into())),
                        _ => return None,
                    }
                }
                
                if type_name == &self.node_type {
                    if let Some(var) = self.variables.iter().find(|v| v.name == member) {
                        return var.typ.clone();
                    }
                }

                get_struct_field_type_recursive(&self.structs, type_name, member)
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
}

