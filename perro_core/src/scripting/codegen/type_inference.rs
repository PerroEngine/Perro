// Type inference for Script AST
use crate::ast::{BuiltInEnumVariant, *};
use crate::resource_modules::{ArrayResource, MapResource};
use crate::scripting::ast::{ContainerKind, Expr, Literal, NumberKind, Type};
use crate::structs::engine_registry::ENGINE_REGISTRY;
use crate::structs::engine_structs::EngineStruct as EngineStructKind;

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
        use NumberKind::*;
        use Type::*;
        if from == to {
            return expr.to_string();
        }

        // Special case: if expr is "self.id" or already ends with ".id", and target is node id (DynNode), no cast needed
        if expr == "self.id" || (expr.ends_with(".id") && matches!(to, Type::DynNode)) {
            return expr.to_string();
        }

        // Special case: if expr is "self" and target type is node id (DynNode), just return "self.id"
        if expr == "self" && matches!(to, Type::DynNode) {
            return "self.id".to_string();
        }

        // Concrete NodeID: Node/DynNode or NodeID::from_u32(...) — no cast or .expect() needed
        if matches!(to, Type::DynNode) {
            if matches!(from, Type::Node(_) | Type::DynNode) {
                return expr.to_string();
            }
            if expr.starts_with("NodeID::from_u32(") {
                return expr.to_string();
            }
        }

        // Direct handling for common conversions
        match (from, to) {
            (from_ty, Type::Option(inner)) if from_ty == inner.as_ref() => {
                return format!("Some({})", expr);
            }
            // Option<NodeID> -> NodeID (e.g. get_child_by_name result assigned to NodeID)
            (Type::Option(inner), Type::DynNode) if matches!(inner.as_ref(), Type::DynNode) => {
                return format!("{}.expect(\"Child node not found\")", expr);
            }
            // UuidOption (script name for Option<NodeID>) -> NodeID
            (Type::Custom(name), Type::DynNode) if name == "UuidOption" => {
                return format!("{}.expect(\"Child node not found\")", expr);
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
            // String -> CowStr conversion (for node names, etc.)
            (String, CowStr) => {
                // If expr is already a Cow::Borrowed or Cow::Owned, don't add .into()
                if expr.starts_with("Cow::Borrowed(") || expr.starts_with("Cow::Owned(") {
                    return expr.to_string();
                }
                // For String -> CowStr, just return the String as-is
                // Methods like set_name accept impl Into<Cow<'static, str>>, so String works directly
                // This avoids ambiguous .into() calls that the compiler can't resolve
                return expr.to_string();
            }
            // StrRef -> CowStr conversion
            (StrRef, CowStr) => {
                return format!("Cow::Borrowed({})", expr);
            }
            // Vector3 -> Vector2 (drop z)
            (EngineStruct(EngineStructKind::Vector3), EngineStruct(EngineStructKind::Vector2)) => {
                return format!("Vector2::new({}.x, {}.y)", expr, expr);
            }
            // Vector2 -> Vector3 (pad z with 0)
            (EngineStruct(EngineStructKind::Vector2), EngineStruct(EngineStructKind::Vector3)) => {
                return format!("Vector3::new({}.x, {}.y, 0.0)", expr, expr);
            }
            // Vector3 (Euler degrees) -> Quaternion (3D rotation)
            (EngineStruct(EngineStructKind::Vector3), EngineStruct(EngineStructKind::Quaternion)) => {
                // Avoid evaluating expr 3x if it's complex
                return format!(
                    "{{ let __e = {expr}; Quaternion::from_euler_degrees(__e.x, __e.y, __e.z) }}"
                );
            }
            // Quaternion -> f32 (2D rotation angle)
            (EngineStruct(EngineStructKind::Quaternion), Number(Float(32))) => {
                // Never apply to_rotation_2d() to a simple numeric literal (e.g. 5.0f32).
                // That would mean the literal was wrongly inferred as Quaternion (e.g. Shape2D
                // radius/width/height); pass through as-is.
                let trimmed = expr.trim();
                let is_bare_number = trimmed
                    .strip_suffix("f32")
                    .or_else(|| trimmed.strip_suffix("f64"))
                    .map(|s| s.trim().parse::<f64>().is_ok())
                    .unwrap_or(false)
                    || trimmed.parse::<f64>().is_ok();
                if !is_bare_number {
                    return format!("{}.to_rotation_2d()", expr);
                }
                return expr.to_string();
            }
            // f32 -> Quaternion (2D rotation)
            (Number(Float(32)), EngineStruct(EngineStructKind::Quaternion)) => {
                return format!("Quaternion::from_rotation_2d({})", expr);
            }
            // CowStr -> CowStr (no conversion needed, but handle if already a Cow)
            (CowStr, CowStr) => {
                // If expr is already a Cow::Borrowed or Cow::Owned, return as-is
                if expr.starts_with("Cow::Borrowed(") || expr.starts_with("Cow::Owned(") {
                    return expr.to_string();
                }
                // Otherwise, it's already a Cow, just return it
                return expr.to_string();
            }
            _ => {}
        }

        // Special case: if expr is already the target type (e.g., c_par_id is already NodeID), no cast needed
        // Check if expr ends with _id and target is DynNode (node variables are already NodeID)
        if expr.ends_with("_id") && matches!(to, Type::DynNode) {
            return expr.to_string();
        }

        // Special case: if expr is a variable name that's already the target type, no cast needed
        // This prevents unnecessary casts when the variable is already a node id
        if !expr.contains(' ') && !expr.contains('(') && !expr.contains('.') {
            // It's a simple variable name - check if we can skip the cast
            if matches!(to, Type::DynNode) && (expr.ends_with("_id") || expr == "self.id") {
                return expr.to_string();
            }
        }

        // Value (Object/Any) to BigInt/Decimal: use proper extraction, not primitive "as" cast
        match (from, to) {
            (Type::Object | Type::Any, Type::Number(NumberKind::BigInt)) => {
                return format!(
                    "({}.as_str().map(|s| s.parse::<BigInt>().unwrap_or_default()).unwrap_or_else(|| BigInt::from({}.as_i64().unwrap_or_default())))",
                    expr, expr
                );
            }
            (Type::Object | Type::Any, Type::Number(NumberKind::Decimal)) => {
                return format!(
                    "({}.as_str().map(|s| rust_decimal::Decimal::from_str(s).unwrap_or_default()).unwrap_or_else(|| rust_decimal::prelude::FromPrimitive::from_f64({}.as_f64().unwrap_or_default()).unwrap_or_default()))",
                    expr, expr
                );
            }
            // BigInt -> unsigned (e.g. map key): use to_u*(), not .as_u64() (Value) or "as" cast
            (Type::Number(NumberKind::BigInt), Type::Number(NumberKind::Unsigned(w))) => {
                return match w {
                    8 => format!("{}.to_u8().unwrap_or_default()", expr),
                    16 => format!("{}.to_u16().unwrap_or_default()", expr),
                    32 => format!("{}.to_u32().unwrap_or_default()", expr),
                    64 => format!("{}.to_u64().unwrap_or_default()", expr),
                    128 => format!("{}.to_u128().unwrap_or_default()", expr),
                    _ => format!("({}.to_u64().unwrap_or_default() as u{})", expr, w),
                };
            }
            _ => {}
        }

        // For now, use simple cast syntax
        // Complex casts will be handled by the Expr::Cast implementation in legacy
        format!("({} as {})", expr, to.to_rust_type())
    }

    pub fn is_struct_field(&self, name: &str) -> bool {
        // Check if the name matches a variable directly
        if self.variables.iter().any(|v| v.name == name) {
            return true;
        }
        // If the name ends with _id, check if the original variable name (without _id) exists
        // This handles cases where node variables are renamed (e.g., s -> s_id)
        if name.ends_with("_id") {
            let original_name = &name[..name.len() - 3];
            if self.variables.iter().any(|v| v.name == original_name) {
                return true;
            }
        }
        false
    }

    pub fn get_variable_type(&self, name: &str) -> Option<&Type> {
        self.variables
            .iter()
            .find(|v| v.name == name)
            .and_then(|v| v.typ.as_ref())
    }

    /// Return the declared type of a variable (Variable.typ) for script-level vars and, when
    /// current_func is set, for that function's locals and params. Used to make struct field
    /// access deterministic: prefer declared type so we always emit .field not ["field"].
    pub fn get_declared_variable_type(
        &self,
        name: &str,
        current_func: Option<&Function>,
    ) -> Option<Type> {
        let original_name = if name.starts_with("__t_") { &name[4..] } else { name };
        if let Some(func) = current_func {
            if let Some(local) = func
                .locals
                .iter()
                .find(|v| v.name == name || v.name == original_name)
            {
                return local.typ.clone();
            }
            fn find_var_in_body<'a>(name: &str, body: &'a [Stmt]) -> Option<&'a Variable> {
                use crate::scripting::ast::Stmt;
                for stmt in body {
                    match stmt {
                        Stmt::VariableDecl(v) if v.name == name => return Some(v),
                        Stmt::If { then_body, else_body, .. } => {
                            if let Some(v) = find_var_in_body(name, then_body) {
                                return Some(v);
                            }
                            if let Some(b) = else_body {
                                if let Some(v) = find_var_in_body(name, b) {
                                    return Some(v);
                                }
                            }
                        }
                        Stmt::For { body: b, .. } | Stmt::ForTraditional { body: b, .. } => {
                            if let Some(v) = find_var_in_body(name, b) {
                                return Some(v);
                            }
                        }
                        _ => {}
                    }
                }
                None
            }
            if let Some(local) = find_var_in_body(name, &func.body).or_else(|| find_var_in_body(original_name, &func.body)) {
                return local.typ.clone();
            }
            if let Some(param) = func.params.iter().find(|p| p.name == name || p.name == original_name) {
                return Some(param.typ.clone());
            }
        }
        self.get_variable_type(name)
            .or_else(|| self.get_variable_type(original_name))
            .cloned()
    }

    /// Check if an identifier is a loop variable by searching for for loops that use it
    fn is_loop_variable(&self, name: &str, body: &[crate::scripting::ast::Stmt]) -> bool {
        use crate::scripting::ast::Stmt;
        for stmt in body {
            match stmt {
                Stmt::For {
                    var_name: loop_var, ..
                } if loop_var == name => {
                    return true;
                }
                Stmt::For {
                    body: loop_body, ..
                }
                | Stmt::ForTraditional {
                    body: loop_body, ..
                } => {
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

        // Do not use a cache keyed by pointer address: addresses differ between runs, so cache
        // hits/misses are non-deterministic and cause different types → different codegen.
        // Always recompute for deterministic output.

        let result = match expr {
            Expr::Literal(lit) => {
                let inferred = self.infer_literal_type(lit, None);
                // eprintln!("[INFER_TYPE] Literal {:?} -> {:?}", lit, inferred);
                inferred
            }
            Expr::Ident(name) => {
                // Strip __t_ prefix if present to get original variable name for lookup
                let original_name = if name.starts_with("__t_") {
                    &name[4..] // Strip "__t_" prefix
                } else {
                    name
                };

                if let Some(func) = current_func {
                    // Also search for variable declarations in nested blocks (if/for/etc).
                    // Parser only collects top-level locals, but nested locals are still valid identifiers.
                    fn find_var_in_stmt<'a>(
                        name: &str,
                        stmt: &'a Stmt,
                    ) -> std::option::Option<&'a Variable> {
                        match stmt {
                            Stmt::VariableDecl(v) if v.name == name => Some(v),
                            Stmt::If {
                                then_body,
                                else_body,
                                ..
                            } => then_body
                                .iter()
                                .find_map(|s| find_var_in_stmt(name, s))
                                .or_else(|| {
                                    else_body.as_ref().and_then(|b| {
                                        b.iter().find_map(|s| find_var_in_stmt(name, s))
                                    })
                                }),
                            Stmt::For { body, .. } | Stmt::ForTraditional { body, .. } => {
                                body.iter().find_map(|s| find_var_in_stmt(name, s))
                            }
                            _ => None,
                        }
                    }
                    fn find_var_in_body<'a>(
                        name: &str,
                        body: &'a [Stmt],
                    ) -> std::option::Option<&'a Variable> {
                        body.iter().find_map(|s| find_var_in_stmt(name, s))
                    }

                    // 1. Local variable (check both original and renamed name)
                    if let Some(local) = func
                        .locals
                        .iter()
                        .find(|v| v.name == *name || v.name == original_name)
                    {
                        if let Some(t) = &local.typ {
                            Some(t.clone())
                        } else if let Some(val) = &local.value {
                            self.infer_expr_type(&val.expr, current_func)
                        } else {
                            None
                        }
                    }
                    // 1b. Nested local variable (declared inside if/for/etc.)
                    else if let Some(local) = find_var_in_body(name, &func.body)
                        .or_else(|| find_var_in_body(original_name, &func.body))
                    {
                        if let Some(t) = &local.typ {
                            Some(t.clone())
                        } else if let Some(val) = &local.value {
                            self.infer_expr_type(&val.expr, current_func)
                        } else {
                            None
                        }
                    }
                    // 2. Function parameter (check both original and renamed name)
                    else if let Some(param) = func
                        .params
                        .iter()
                        .find(|p| p.name == *name || p.name == original_name)
                    {
                        Some(param.typ.clone())
                    }
                    // 3. Check if it's a loop variable (use original name for lookup)
                    else if self.is_loop_variable(original_name, &func.body) {
                        Some(Type::Number(NumberKind::Signed(32)))
                    }
                    // 4. Script-level variable or exposed field
                    else {
                        // Try both original and renamed name, then global nodes (e.g. Root). Chain Option<Type>.
                        self.get_variable_type(name)
                            .cloned()
                            .or_else(|| self.get_variable_type(original_name).cloned())
                            .or_else(|| {
                                self.global_name_to_node_id
                                    .get(name)
                                    .map(|_| Type::Node(crate::node_registry::NodeType::Node))
                            })
                            .or_else(|| {
                                self.global_name_to_node_id
                                    .get(original_name)
                                    .map(|_| Type::Node(crate::node_registry::NodeType::Node))
                            })
                    }
                } else {
                    // Try both original and renamed name, then global nodes (e.g. Root). Chain Option<Type>.
                    self.get_variable_type(name)
                        .cloned()
                        .or_else(|| self.get_variable_type(original_name).cloned())
                        .or_else(|| {
                            self.global_name_to_node_id
                                .get(name)
                                .map(|_| Type::Node(crate::node_registry::NodeType::Node))
                        })
                        .or_else(|| {
                            self.global_name_to_node_id
                                .get(original_name)
                                .map(|_| Type::Node(crate::node_registry::NodeType::Node))
                        })
                }
            }
            Expr::Range(_, _) => Some(Type::Container(
                ContainerKind::Array,
                vec![Type::Number(NumberKind::Signed(32))],
            )),
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
            Expr::EnumAccess(variant) => {
                // Enum access like NODE_TYPE.Sprite2D returns NodeType enum
                match variant {
                    BuiltInEnumVariant::NodeType(_) => Some(Type::NodeType),
                }
            }
            Expr::MemberAccess(base, field) => {
                // Check if this is a module constant access (e.g., PoopyButt.v)
                if let Expr::Ident(mod_name) = base.as_ref() {
                    if let Some(module_vars) = self.module_variables.get(mod_name) {
                        if let Some(var) = module_vars.iter().find(|v| v.name == *field) {
                            // Found the module constant, return its type
                            // If the type is explicitly set, use it; otherwise infer from the value
                            if let Some(typ) = &var.typ {
                                return Some(typ.clone());
                            } else if let Some(value) = &var.value {
                                // Fallback: infer type from the constant's value
                                return self.infer_expr_type(&value.expr, current_func);
                            }
                        }
                    }
                }

                // Otherwise, treat as normal member access
                let base_type = self.infer_expr_type(base, current_func)?;
                self.get_member_type(&base_type, field)
            }
            Expr::Call(target, _args) => match &**target {
                Expr::Ident(fname) => self.get_function_return_type(fname),
                Expr::MemberAccess(base, method) => {
                    // Check if this is a module function call (e.g., PoopyButt.return_true)
                    if let Expr::Ident(mod_name) = base.as_ref() {
                        if let Some(module_funcs) = self.module_functions.get(mod_name) {
                            if let Some(func) = module_funcs.iter().find(|f| f.name == *method) {
                                return Some(func.return_type.clone());
                            }
                        }
                    }

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
                crate::call_modules::CallModule::Resource(
                    crate::resource_modules::ResourceModule::MapOp(MapResource::Get),
                ) => {
                    if let Some(Type::Container(ContainerKind::Map, ref params)) =
                        self.infer_expr_type(&args[0], current_func)
                    {
                        return params.get(1).cloned();
                    }
                    Some(Type::Object)
                }
                crate::call_modules::CallModule::Resource(
                    crate::resource_modules::ResourceModule::ArrayOp(ArrayResource::Pop),
                ) => {
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
            Expr::ObjectLiteral(_) => Some(Type::Any), // Use Any as the preferred dynamic type
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
                    Type::Container(container_kind, inner_types) => match container_kind {
                        ContainerKind::Array => {
                            if matches!(inner_types.first(), Some(Type::Object | Type::Any)) {
                                Some(Type::Any)
                            } else {
                                inner_types.first().cloned()
                            }
                        }
                        ContainerKind::Map => {
                            if matches!(inner_types.last(), Some(Type::Object | Type::Any)) {
                                Some(Type::Any)
                            } else {
                                inner_types.last().cloned()
                            }
                        }
                        ContainerKind::FixedArray(_) => inner_types.first().cloned(),
                    },
                    Type::Object | Type::Any => Some(Type::Any),
                    _ => None,
                }
            }
            Expr::BaseAccess => Some(Custom(self.node_type.clone())),
        };

        result
    }

    pub(crate) fn infer_literal_type(
        &self,
        lit: &Literal,
        expected_type: Option<&Type>,
    ) -> Option<Type> {
        match lit {
            Literal::Number(n) => {
                if let Some(expected) = expected_type {
                    Some(expected.clone())
                } else {
                    // If number contains a decimal point, infer as f32, otherwise i32
                    if n.contains('.') {
                        Some(Type::Number(NumberKind::Float(32)))
                    } else {
                        Some(Type::Number(NumberKind::Signed(32)))
                    }
                }
            }
            Literal::Bool(_) => Some(Type::Bool),
            Literal::String(_) | Literal::Interpolated(_) => match expected_type {
                Some(Type::CowStr) => Some(Type::CowStr),
                Some(Type::StrRef) => Some(Type::StrRef),
                _ => Some(Type::String),
            },
            Literal::Null => {
                // null can be assigned to any Option<T>
                // If we have an expected type that's Option<T>, use it; otherwise return None (will be inferred from context)
                expected_type.and_then(|t| {
                    if matches!(t, Type::Option(_)) {
                        Some(t.clone())
                    } else {
                        None
                    }
                })
            }
        }
    }

    pub(crate) fn promote_types(&self, left: &Type, right: &Type) -> Option<Type> {
        if left == right {
            return Some(left.clone());
        }

        match (left, right) {
            (Type::DynNode, Type::Node(_)) | (Type::Node(_), Type::DynNode) => Some(Type::DynNode),
            (Type::DynNode, Type::Custom(tn)) | (Type::Custom(tn), Type::DynNode)
                if is_node_type(tn) =>
            {
                Some(Type::DynNode)
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
            // DynNode field unification: use widest type so assignees get correct inference
            (
                Type::EngineStruct(EngineStructKind::Vector2),
                Type::EngineStruct(EngineStructKind::Vector3),
            )
            | (
                Type::EngineStruct(EngineStructKind::Vector3),
                Type::EngineStruct(EngineStructKind::Vector2),
            ) => Some(Type::EngineStruct(EngineStructKind::Vector3)),
            (
                Type::EngineStruct(EngineStructKind::Transform2D),
                Type::EngineStruct(EngineStructKind::Transform3D),
            )
            | (
                Type::EngineStruct(EngineStructKind::Transform3D),
                Type::EngineStruct(EngineStructKind::Transform2D),
            ) => Some(Type::EngineStruct(EngineStructKind::Transform3D)),
            (
                Type::EngineStruct(EngineStructKind::Quaternion),
                Type::Number(NumberKind::Float(32)),
            )
            | (
                Type::Number(NumberKind::Float(32)),
                Type::EngineStruct(EngineStructKind::Quaternion),
            ) => Some(Type::EngineStruct(EngineStructKind::Quaternion)),
            // Value (Any/Object) op Number -> use the Number type so e.g. var c = b * 2 infers c as f32
            (Type::Any | Type::Object, Type::Number(_)) => Some(right.clone()),
            (Type::Number(_), Type::Any | Type::Object) => Some(left.clone()),
            _ => Some(left.clone()),
        }
    }

    /// Unify types that can appear from a DynNode field (e.g. position: Vector2|Vector3 -> Vector3).
    fn unify_dynnode_field_types(types: &[Type]) -> Option<Type> {
        if types.is_empty() {
            return None;
        }
        if types.len() == 1 {
            return Some(types[0].clone());
        }
        let mut result = types[0].clone();
        for t in &types[1..] {
            if let Some(unified) = Self::unify_two_field_types(&result, t) {
                result = unified;
            }
        }
        Some(result)
    }

    fn unify_two_field_types(a: &Type, b: &Type) -> Option<Type> {
        use NumberKind::*;
        use Type::*;
        if a == b {
            return Some(a.clone());
        }
        match (a, b) {
            (EngineStruct(EngineStructKind::Vector2), EngineStruct(EngineStructKind::Vector3))
            | (EngineStruct(EngineStructKind::Vector3), EngineStruct(EngineStructKind::Vector2)) => {
                Some(Type::EngineStruct(EngineStructKind::Vector3))
            }
            (
                EngineStruct(EngineStructKind::Transform2D),
                EngineStruct(EngineStructKind::Transform3D),
            )
            | (
                EngineStruct(EngineStructKind::Transform3D),
                EngineStruct(EngineStructKind::Transform2D),
            ) => Some(Type::EngineStruct(EngineStructKind::Transform3D)),
            (EngineStruct(EngineStructKind::Quaternion), Number(Float(32)))
            | (Number(Float(32)), EngineStruct(EngineStructKind::Quaternion)) => {
                Some(Type::EngineStruct(EngineStructKind::Quaternion))
            }
            _ => Some(a.clone()),
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
                // Script variables (e.g. self.typed_big_int) are on the same script instance as the node
                if let Some(var) = self.variables.iter().find(|v| v.name == member) {
                    return var.typ.clone();
                }
                // Use PUP_NODE_API to get the script type (e.g., Texture instead of Option<Uuid>)
                use crate::scripting::lang::pup::node_api::PUP_NODE_API;
                let fields = PUP_NODE_API.get_fields(node_type);
                if let Some(api_field) = fields.iter().find(|f| f.script_name == member) {
                    // Return the script-side type (e.g., Texture, not Option<Uuid>)
                    Some(api_field.get_script_type())
                } else {
                    // Fallback to engine registry if field not found in node API
                    let rust_field = ENGINE_REGISTRY.resolve_field_name(node_type, member);
                    ENGINE_REGISTRY.get_field_type_node(node_type, &rust_field)
                }
            }
            Type::DynNode => {
                let nodes_with_field = ENGINE_REGISTRY.find_nodes_with_field(member);
                if nodes_with_field.is_empty() {
                    return None;
                }
                // Collect types from all nodes that have this field and unify (e.g. Vector2|Vector3 -> Vector3)
                // so that assignees like "var vecas: Vector2 = c_name" get correct RHS type and implicit cast.
                let types: Vec<Type> = nodes_with_field
                    .iter()
                    .filter_map(|node_type| ENGINE_REGISTRY.get_field_type_node(node_type, member))
                    .collect();
                Self::unify_dynnode_field_types(&types)
            }
            Type::EngineStruct(engine_struct) => {
                ENGINE_REGISTRY.get_field_type_struct(engine_struct, member)
            }
            Type::Custom(type_name) => {
                if type_name == "ParentType" {
                    match member {
                        "id" => return Some(Type::DynNode),
                        "node_type" => return Some(Type::NodeType),
                        _ => return None,
                    }
                }

                if type_name == &self.node_type {
                    if let Some(var) = self.variables.iter().find(|v| v.name == member) {
                        return var.typ.clone();
                    }
                    // Script's node_type is the engine type (e.g. Camera2D); resolve member from engine registry
                    if let Some(node_type_enum) = string_to_node_type(type_name) {
                        if let Some(ty) = ENGINE_REGISTRY.get_field_type_node(&node_type_enum, member) {
                            return Some(ty);
                        }
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
