use std::collections::HashMap;

use crate::{api_modules::ApiModule, engine_structs::EngineStruct, node_registry::NodeType};

#[derive(Debug, Clone)]
pub struct Script {
    pub node_type: String,
    pub variables: Vec<Variable>,
    pub functions: Vec<Function>,

    pub structs: Vec<StructDef>,

    pub verbose: bool,
}

#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub typ: Option<Type>,
    pub value: Option<TypedExpr>,
    pub is_public: bool,
    pub is_exposed: bool,
}

impl Variable {
    pub fn rust_type(&self) -> String {
        match &self.typ {
            Some(t) => t.to_rust_type(),
            None => panic!("Type inference unresolved for variable: {}", self.name),
        }
    }

    pub fn parse_type(s: &str) -> Type {
        match s {
            // Primitive numeric
            "i8" | "i16" | "i32" | "i64" | "i128" => {
                let w = s.trim_start_matches('i').parse().unwrap();
                Type::Number(NumberKind::Signed(w))
            }
            "u8" | "u16" | "u32" | "u64" | "u128" => {
                let w = s.trim_start_matches('u').parse().unwrap();
                Type::Number(NumberKind::Unsigned(w))
            }
            "f32" => Type::Number(NumberKind::Float(32)),
            "f64" => Type::Number(NumberKind::Float(64)),

            // Other primitives
            "bool" => Type::Bool,
            "String" => Type::String,
            "&str" => Type::StrRef,

            // Containers
            "object" | "Object" | "Value" | "Any" | "any" => Type::Object,

            // Containers — always with type arguments!
            "HashMap" | "Map" | "map" => Type::Container(
                ContainerKind::Map,
                vec![Type::String, Type::Object], // default key: String, value: Object
            ),
            "Vec" | "Array" | "array" => Type::Container(ContainerKind::Array, vec![Type::Object]),

            // Everything else
            name => Type::Custom(name.to_string()),
        }
    }

    pub fn json_access(&self) -> (&'static str, String) {
        match self.typ.as_ref().unwrap() {
            Type::Number(NumberKind::Signed(w)) => {
                if *w == 128 {
                    ("as_i128", "".into())
                } else {
                    ("as_i64", format!(" as i{}", w))
                }
            }
            Type::Number(NumberKind::Unsigned(w)) => {
                if *w == 128 {
                    ("as_u128", "".into())
                } else {
                    ("as_u64", format!(" as u{}", w))
                }
            }
            Type::Number(NumberKind::Float(w)) => {
                let rust_ty = match *w {
                    32 => "f32".to_string(),
                    64 => "f64".to_string(),
                    _ => "f64".to_string(),
                };
                ("as_f64", format!(" as {}", rust_ty))
            }
            Type::Number(NumberKind::Decimal) => {
                ("as_str", format!(".parse::<Decimal>().unwrap()"))
            }
            Type::Number(NumberKind::BigInt) => ("as_str", format!(".parse::<BigInt>().unwrap()")),
            Type::Bool => ("as_bool", "".into()),
            Type::String | Type::StrRef => ("as_str", ".to_string()".into()),
            Type::Custom(type_name) if type_name == "Signal" => ("as_u64", format!(" as u64")),

            // Containers
            Type::Container(ContainerKind::Array, params) => {
                let inner = params.get(0).unwrap_or(&Type::Object);
                match inner {
                    Type::Object => ("as_array", ".clone()".into()),
                    inner => (
                        "__CUSTOM__",
                        format!(
                            "v.as_array().map(|a| a.iter()
                            .map(|x| serde_json::from_value::<{}>(x.clone()).unwrap_or_default())
                            .collect::<Vec<{}>>()
                        ).unwrap_or_default()",
                            inner.to_rust_type(),
                            inner.to_rust_type()
                        ),
                    ),
                }
            }

            // ---- Fixed Array [T; N] ----
            Type::Container(ContainerKind::FixedArray(size), params) => {
                let inner = params.get(0).unwrap_or(&Type::Object);
                match inner {
                // dynamic fixed arrays (array of Value)
                Type::Object => ("as_array", format!(
                    ".as_array().map(|a| {{
                        let mut out: [{}; {}] = [Default::default(); {}];
                        for (i, val) in a.iter().enumerate().take({}) {{
                            out[i] = val.clone();
                        }}
                        out
                    }}).unwrap_or_default()",
                    inner.to_rust_type(), size, size, size
                )),
                // typed fixed arrays
                inner => (
                    "__CUSTOM__",
                    format!(
                        "v.as_array().map(|a| {{
                            let mut out: [{}; {}] = [Default::default(); {}];
                            for (i, val) in a.iter().enumerate().take({}) {{
                                out[i] = serde_json::from_value::<{}>(val.clone()).unwrap_or_default();
                            }}
                            out
                        }}).unwrap_or_default()",
                        inner.to_rust_type(),
                        size,
                        size,
                        size,
                        inner.to_rust_type()
                    )
                ),
            }
            }

            // ---- Map ---
            Type::Container(ContainerKind::Map, params) => {
                let key_ty = params.get(0).unwrap_or(&Type::String);
                let val_ty = params.get(1).unwrap_or(&Type::Object);
                match val_ty {
                Type::Object => ("as_object", ".iter().map(|(k, v)| (k.clone(), v.clone())).collect()".into()),
                inner => (
                    "__CUSTOM__",
                    format!(
                        "v.as_object().map(|obj| obj.iter()
                            .map(|(k, v)| (k.clone(), serde_json::from_value::<{}>(v.clone()).unwrap_or_default()))
                            .collect::<HashMap<{}, {}>>()
                        ).unwrap_or_default()",
                        inner.to_rust_type(),
                        key_ty.to_rust_type(),
                        inner.to_rust_type()
                    )
                ),
            }
            }
            Type::Object => ("as_object", ".clone().into()".into()),

            Type::Script => ("as_str", ".parse().unwrap()".into()),
            Type::Custom(type_name) => ("__CUSTOM__", type_name.clone()),
            Type::Void => panic!("Void invalid"),
            Type::Node(node_type) => ("__NODE__", "NodeType".to_owned()),
            Type::EngineStruct(engine_struct) => ("__ENGINE_STRUCT__", "EngineStruct".to_owned()),
        }
    }

    pub fn rust_initialization(&self, script: &Script, current_func: Option<&Function>) -> String {
        if let Some(expr) = &self.value {
            expr.to_rust(false, script, current_func) // let TypedExpr handle type propagation
        } else {
            self.default_value()
        }
    }

    pub fn default_value(&self) -> String {
        match &self.typ {
            Some(t) => t.rust_default_value(),
            None => panic!("Type inference unresolved"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    pub locals: Vec<Variable>,
    pub body: Vec<Stmt>,
    pub is_trait_method: bool,
    pub uses_self: bool,
    pub return_type: Type,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub typ: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Number(NumberKind),
    Bool,
    String,
    StrRef,
    Script,
    Void,

    Container(ContainerKind, Vec<Type>),
    Object,

    Node(NodeType),
    EngineStruct(EngineStruct),
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ContainerKind {
    Map,
    Array,
    FixedArray(usize),
}

impl Type {
    pub fn to_rust_type(&self) -> String {
        match self {
            Type::Number(NumberKind::Signed(w)) => match w {
                8 | 16 | 32 | 64 | 128 => format!("i{}", w),
                _ => panic!("Unsupported signed integer width: {}", w),
            },

            Type::Number(NumberKind::Unsigned(w)) => match w {
                8 | 16 | 32 | 64 | 128 => format!("u{}", w),
                _ => panic!("Unsupported unsigned integer width: {}", w),
            },

            Type::Number(NumberKind::Float(w)) => match w {
                32 => "f32".to_string(),
                64 => "f64".to_string(),
                _ => panic!("Unsupported float width: {}", w),
            },

            Type::Number(NumberKind::Decimal) => "Decimal".to_string(),
            Type::Number(NumberKind::BigInt) => "BigInt".to_string(),

            Type::Bool => "bool".to_string(),
            Type::String => "String".to_string(),
            Type::StrRef => "&'static str".to_string(),

            // ---- Containers ----
            Type::Container(ContainerKind::Map, params) => {
                let k = params
                    .get(0)
                    .map_or("String".to_string(), |p| p.to_rust_type());
                let v = params.get(1).map_or("Value".to_string(), |p| {
                    // For custom types, use Value to allow polymorphism
                    match p {
                        Type::Custom(_) => "Value".to_string(),
                        _ => p.to_rust_type(),
                    }
                });
                format!("HashMap<{}, {}>", k, v)
            }
            Type::Container(ContainerKind::Array, params) => {
                let val = params.get(0).map_or("Value".to_string(), |p| {
                    // For custom types, use Value to allow polymorphism
                    match p {
                        Type::Custom(_) => "Value".to_string(),
                        _ => p.to_rust_type(),
                    }
                });
                format!("Vec<{}>", val)
            }
            Type::Container(ContainerKind::FixedArray(size), params) => {
                let val = params
                    .get(0)
                    .map_or("Value".to_string(), |p| p.to_rust_type());
                format!("[{}; {}]", val, size)
            }

            // ---- "Object" (serde_json::Value) ----
            Type::Object => "Value".to_string(),

            Type::Script => "Option<ScriptType>".to_string(),
            Type::Custom(name) if name == "Signal" => "u64".to_string(),
            Type::Custom(name) => name.clone(),
            Type::Void => "()".to_string(),
            _ => "".to_string(),
        }
    }

    pub fn rust_default_value(&self) -> String {
        use ContainerKind::*;
        match self {
            Type::Number(NumberKind::Signed(w)) => format!("0i{}", w),
            Type::Number(NumberKind::Unsigned(w)) => format!("0u{}", w),
            Type::Number(NumberKind::Float(w)) => match w {
                32 => "0.0f32".to_string(),
                64 => "0.0f64".to_string(),
                _ => "0.0f64".to_string(),
            },
            Type::Number(NumberKind::Decimal) => "Decimal::from_str(\"0\").unwrap()".to_string(),
            Type::Number(NumberKind::BigInt) => "BigInt::from_str(\"0\").unwrap()".to_string(),

            Type::Bool => "false".into(),
            Type::Script => "None".into(),
            Type::String => "String::new()".into(),
            Type::StrRef => "\"\"".into(),

            Type::Object => "json!({})".into(),

            Type::Container(HashMap, _) => "HashMap::new()".into(),
            Type::Container(Array, _) => "Vec::new()".into(),
            Type::Container(FixedArray(size), params) => {
                let elem_val = params
                    .get(0)
                    .map(|p| p.rust_default_value())
                    .unwrap_or_else(|| "Default::default()".into());
                format!("[{}; {}]", elem_val, size)
            }

            Type::Custom(_) | Type::EngineStruct(_) | Type::Node(_) => {
                "Default::default()".to_string()
            }
            Type::Void => panic!("Cannot make default for void"),
        }
    }

    pub fn can_implicitly_convert_to(&self, target: &Type) -> bool {
        use NumberKind::*;
        match (self, target) {
            (a, b) if a == b => true,

            // integer widening (signed)
            (Type::Number(Signed(s)), Type::Number(Signed(t))) if s < t => true,
            (Type::Number(Unsigned(s)), Type::Number(Unsigned(t))) if s < t => true,

            // unsigned → signed (widening)
            (Type::Number(Unsigned(s)), Type::Number(Signed(t))) if t >= s => true,

            // int → float
            (Type::Number(Signed(_)) | Type::Number(Unsigned(_)), Type::Number(Float(_))) => true,

            // float widening (f32 → f64)
            (Type::Number(Float(32)), Type::Number(Float(64))) => true,

            // int → BigInt
            (Type::Number(Signed(_)) | Type::Number(Unsigned(_)), Type::Number(BigInt)) => true,

            // ✅ NEW rules
            // int → Decimal
            (Type::Number(Signed(_)) | Type::Number(Unsigned(_)), Type::Number(Decimal)) => true,
            // float → Decimal
            (Type::Number(Float(_)), Type::Number(Decimal)) => true,

            _ => false,
        }
    }

    pub fn is_copy_type(&self) -> bool {
        use NumberKind::*;
        use Type::*;

        match self {
            // all numeric primitives are Copy
            Number(Signed(_)) | Number(Unsigned(_)) | Number(Float(_)) | Bool => true,
            _ => false,
        }
    }

    pub fn requires_clone(&self) -> bool {
        !self.is_copy_type()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum NumberKind {
    Signed(u8),   // width: 8,16,32,64,128
    Unsigned(u8), // width: ^
    Float(u8),    // 16,32,64,128
    Decimal,
    BigInt,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Expr(TypedExpr),
    VariableDecl(Variable), // Update Variable to use TypedExpr internally
    Assign(String, TypedExpr),
    AssignOp(String, Op, TypedExpr),
    MemberAssign(TypedExpr, TypedExpr), // Both LHS and RHS need types
    MemberAssignOp(TypedExpr, Op, TypedExpr), // Both need types
    ScriptAssign(String, String, TypedExpr), // RHS needs type
    ScriptAssignOp(String, String, Op, TypedExpr), // RHS needs type
    IndexAssign(Box<Expr>, Box<Expr>, TypedExpr),
    IndexAssignOp(Box<Expr>, Box<Expr>, Op, TypedExpr),
    Pass,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Ident(String),
    Literal(Literal),
    BinaryOp(Box<Expr>, Op, Box<Expr>),
    MemberAccess(Box<Expr>, String),
    SelfAccess,
    BaseAccess,
    Call(Box<Expr>, Vec<Expr>),
    Cast(Box<Expr>, Type),

    ObjectLiteral(Vec<(Option<String>, Expr)>),
    ContainerLiteral(ContainerKind, ContainerLiteralData),
    Index(Box<Expr>, Box<Expr>),

    StructNew(String, Vec<(String, Expr)>),

    ApiCall(ApiModule, Vec<Expr>),
}

#[derive(Debug, Clone)]
pub enum ContainerLiteralData {
    Array(Vec<Expr>),
    Map(Vec<(Expr, Expr)>),
    FixedArray(usize, Vec<Expr>),
}

#[derive(Debug, Clone)]
pub struct TypedExpr {
    pub expr: Expr,
    pub inferred_type: Option<Type>, // Gets filled during type inference
}

#[derive(Debug, Clone)]
pub enum Literal {
    Number(String), // "5", "3.14", "999999999", etc.
    String(String),
    Bool(bool),
    Interpolated(String),
}

#[derive(Debug, Clone)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub base: Option<String>,
    pub fields: Vec<StructField>,
    pub methods: Vec<Function>,
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub name: String,
    pub typ: Type,
}
