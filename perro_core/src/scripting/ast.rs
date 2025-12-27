use std::collections::HashMap;

use crate::{api_modules::ApiModule, engine_structs::EngineStruct, node_registry::NodeType};
use crate::scripting::source_span::SourceSpan;

/// Built-in enum variants available in the scripting language
/// This represents enum access like NODE_TYPE.Sprite2D (SCREAMING_SNAKE_CASE for enum names)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltInEnumVariant {
    NodeType(NodeType),
}

#[derive(Debug, Clone)]
pub struct Script {
    pub script_name: Option<String>, // The script name from @script Name
    pub node_type: String,
    pub variables: Vec<Variable>,
    pub functions: Vec<Function>,

    pub structs: Vec<StructDef>,

    pub verbose: bool,
    pub attributes: HashMap<String, Vec<String>>, // member name -> list of attribute names
    pub source_file: Option<String>, // Original source file path (e.g., "res://player.pup")
    pub language: Option<String>, // Language identifier (e.g., "pup", "typescript", "csharp")
}

#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub typ: Option<Type>,
    pub value: Option<TypedExpr>,
    pub is_public: bool,
    pub is_exposed: bool,
    pub attributes: Vec<String>, // List of attribute names
    pub span: Option<SourceSpan>, // Source location of this variable declaration
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

            // Built-in types
            "NodeType" => Type::NodeType,

            // Containers
            "object" | "Object" | "Value" | "Any" | "any" => Type::Object,

            // Containers — always with type arguments!
            "HashMap" | "Map" | "map" => Type::Container(
                ContainerKind::Map,
                vec![Type::String, Type::Object], // default key: String, value: Object
            ),
            "Vec" | "Array" | "array" => Type::Container(ContainerKind::Array, vec![Type::Object]),

            // Check engine registry for node types
            name => {
                use crate::structs::engine_registry::ENGINE_REGISTRY;
                // Check if it's a node type in the engine registry
                if let Some(node_type) = ENGINE_REGISTRY.node_defs.keys().find(|nt| {
                    format!("{:?}", nt) == name
                }) {
                    Type::Node(node_type.clone())
                } else {
                    Type::Custom(name.to_string())
                }
            }
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
            Type::String | Type::StrRef | Type::CowStr => ("as_str", ".to_string()".into()),
            Type::Uuid => ("as_str", ".to_string()".into()), // UUIDs are serialized as strings
            Type::Option(_) => ("as_str", ".to_string()".into()), // Options are handled specially
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

            Type::Signal => ("as_u64", "".into()),
            Type::NodeType => ("as_str", ".parse::<NodeType>().unwrap()".into()), // NodeType is serialized as string
            Type::Custom(type_name) => ("__CUSTOM__", type_name.clone()),
            Type::Void => panic!("Void invalid"),
            Type::Node(_node_type) => ("__NODE__", "NodeType".to_owned()),
            Type::DynNode => ("as_str", ".parse::<Uuid>().unwrap()".into()), // DynNode is Uuid, serialized as string
            Type::EngineStruct(es) => {
                use crate::engine_structs::EngineStruct;
                match es {
                    EngineStruct::Texture => ("__CUSTOM__", "Option<Uuid>".to_string()), // Texture is Option<Uuid>, use custom deserialization
                    _ => ("__ENGINE_STRUCT__", "EngineStruct".to_owned()), // Other engine structs use the generic method
                }
            },
        }
    }

    pub fn rust_initialization(&self, script: &Script, current_func: Option<&Function>) -> String {
        if let Some(expr) = &self.value {
            // Use the variable's declared type as the expected type for the expression
            // This ensures literals get the correct suffix (e.g., 42f64 instead of 42f32)
            let expected_type = self.typ.as_ref();
            // Call expr.expr.to_rust directly with the variable's type as expected_type
            expr.expr
                .to_rust(false, script, expected_type, current_func, expr.span.as_ref())
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
    pub cloned_child_nodes: Vec<String>, // Variable names that hold cloned child nodes (from self.get_node("name") as Type)
    pub return_type: Type,
    pub attributes: Vec<String>, // List of attribute names
    pub is_on_signal: bool, // True if this function was defined with "on SIGNALNAME()" syntax
    pub signal_name: Option<String>, // The signal name if this is an on-signal function
    pub span: Option<SourceSpan>, // Source location of this function definition
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub typ: Type,
    pub span: Option<SourceSpan>, // Source location of this parameter
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Number(NumberKind),
    Bool,
    String,
    StrRef,
    CowStr, // Cow<'static, str> - for node name and other borrowed strings
    Uuid, // uuid::Uuid
    Option(Box<Type>), // Option<T>
    Signal, // u64 - signal ID type
    Void,

    Container(ContainerKind, Vec<Type>),
    Object,

    Node(NodeType), // Node instance type (UUID)
    DynNode, // Dynamic node type - no type resolution at compile time, resolved to UUID at runtime
    NodeType, // NodeType enum itself (e.g., from get_type())
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
            Type::CowStr => "Cow<'static, str>".to_string(),
            Type::Uuid => "Uuid".to_string(),
            Type::Option(inner) => format!("Option<{}>", inner.to_rust_type()),

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

            Type::Signal => "u64".to_string(),
            Type::Custom(name) => {
                // Rename custom structs with __t_ prefix (but not node types or engine structs)
                use crate::scripting::codegen::{is_node_type, rename_struct};
                if is_node_type(name) {
                    // Node types should be Type::Node, but handle gracefully
                    name.clone()
                } else {
                    rename_struct(name)
                }
            },
            Type::Node(_) => "Uuid".to_string(), // Nodes are now Uuid IDs
            Type::DynNode => "Uuid".to_string(), // DynNode is also a Uuid ID
            Type::NodeType => "NodeType".to_string(), // NodeType enum
            Type::EngineStruct(es) => {
                // Texture becomes Option<Uuid> in Rust (it's a handle, not a real struct)
                // Other engine structs (Vector2, Color, etc.) are real structs
                use crate::engine_structs::EngineStruct;
                match es {
                    EngineStruct::Texture => "Option<Uuid>".to_string(),
                    _ => format!("{:?}", es), // Other engine structs are real types
                }
            },
            Type::Void => "()".to_string(),
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
            Type::Signal => "0u64".into(),
            Type::String => "String::new()".into(),
            Type::StrRef => "\"\"".into(),
            Type::CowStr => "Cow::Borrowed(\"\")".into(),
            Type::Uuid => "Uuid::nil()".into(),
            Type::Option(_) => "None".into(),

            Type::Object => "json!({})".into(),

            Type::Container(Map, _) => "HashMap::new()".into(),
            Type::Container(Array, _) => "Vec::new()".into(),
            Type::Container(FixedArray(size), params) => {
                let elem_val = params
                    .get(0)
                    .map(|p| p.rust_default_value())
                    .unwrap_or_else(|| "Default::default()".into());
                format!("[{}; {}]", elem_val, size)
            }

            Type::Custom(_) => {
                "Default::default()".to_string()
            }
            Type::EngineStruct(es) => {
                use crate::engine_structs::EngineStruct;
                match es {
                    EngineStruct::Texture => "None".to_string(), // Texture is Option<Uuid>, default is None
                    _ => format!("{}::default()", format!("{:?}", es)), // Other engine structs implement Default
                }
            }
            Type::Node(_) => {
                // Nodes are Uuid IDs, use nil since it will be set later
                "Uuid::nil()".to_string()
            }
            Type::DynNode => {
                // DynNode is also a Uuid ID
                "Uuid::nil()".to_string()
            }
            Type::NodeType => {
                // NodeType enum default is NodeType::Node
                "NodeType::Node".to_string()
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

            // String type conversions
            // String -> CowStr (owned string can become Cow::Owned)
            (Type::String, Type::CowStr) => true,
            // StrRef -> CowStr (borrowed string can become Cow::Borrowed)
            (Type::StrRef, Type::CowStr) => true,
            // CowStr -> String (Cow can be converted to owned String)
            (Type::CowStr, Type::String) => true,
            // Node types all convert to Uuid (they are Uuid IDs)
            (Type::Node(_), Type::Uuid) => true,
            // Uuid can be treated as any Node type (for type checking)
            (Type::Uuid, Type::Node(_)) => true,
            // DynNode conversions
            (Type::DynNode, Type::Uuid) => true,
            (Type::Uuid, Type::DynNode) => true,
            (Type::DynNode, Type::Node(_)) => true, // DynNode can be cast to any Node type
            (Type::Node(_), Type::DynNode) => true, // Any Node type can become DynNode

            // T -> Option<T> conversions (wrapping in Some)
            (from, Type::Option(inner)) if *from == *inner.as_ref() => true,

            _ => false,
        }
    }

    pub fn is_copy_type(&self) -> bool {
        use crate::engine_structs::EngineStruct as ES;
        use NumberKind::*;
        use Type::*;

        match self {
            // all numeric primitives are Copy
            Number(Signed(_)) | Number(Unsigned(_)) | Number(Float(_)) | Bool => true,
            // Uuid is also Copy
            Uuid => true,
            // Uuid
            Node(_) => true,
            // Uuid
            DynNode => true,
            // EngineStructs implement Copy
            EngineStruct(es) => match es {
                // These implement Copy
                ES::Vector2 | ES::Vector3 
                | ES::Transform2D | ES::Transform3D
                | ES::Color | ES::Rect 
                | ES::Quaternion | ES::ShapeType2D => true,
                
                ES::Texture => true,
                // Texture is a Uuid handle, which implements Copy
            },
            _ => false,
        }
    }

    pub fn requires_clone(&self) -> bool {
        match self {
            // Option<Copy> is also Copy, so it doesn't require clone
            Type::Option(inner) => inner.requires_clone(),
            _ => !self.is_copy_type(),
        }
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
    If {
        condition: TypedExpr,
        then_body: Vec<Stmt>,
        else_body: Option<Vec<Stmt>>,
    },
    For {
        var_name: String,
        iterable: TypedExpr,
        body: Vec<Stmt>,
    },
    ForTraditional {
        init: Option<Box<Stmt>>,      // var i = 0
        condition: Option<TypedExpr>, // i < 10
        increment: Option<Box<Stmt>>, // i++
        body: Vec<Stmt>,
    },
    Pass,
}

impl Stmt {
    /// Get the source span for this statement, if available
    pub fn span(&self) -> Option<&SourceSpan> {
        match self {
            Stmt::Expr(expr) => expr.span.as_ref(),
            Stmt::VariableDecl(var) => var.span.as_ref(),
            Stmt::Assign(_, expr) | Stmt::AssignOp(_, _, expr) => expr.span.as_ref(),
            Stmt::MemberAssign(lhs, _) | Stmt::MemberAssignOp(lhs, _, _) => lhs.span.as_ref(),
            Stmt::ScriptAssign(_, _, expr) | Stmt::ScriptAssignOp(_, _, _, expr) => expr.span.as_ref(),
            Stmt::IndexAssign(_, _, expr) | Stmt::IndexAssignOp(_, _, _, expr) => expr.span.as_ref(),
            Stmt::If { condition, .. } => condition.span.as_ref(),
            Stmt::For { iterable, .. } => iterable.span.as_ref(),
            Stmt::ForTraditional { condition, .. } => condition.as_ref().and_then(|c| c.span.as_ref()),
            Stmt::Pass => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Expr {
    Ident(String),
    Literal(Literal),
    BinaryOp(Box<Expr>, Op, Box<Expr>),
    MemberAccess(Box<Expr>, String),
    EnumAccess(BuiltInEnumVariant), // EnumAccess on a built-in enum variant
    SelfAccess,
    BaseAccess,
    Call(Box<Expr>, Vec<Expr>),
    Cast(Box<Expr>, Type),

    ObjectLiteral(Vec<(Option<String>, Expr)>),
    ContainerLiteral(ContainerKind, ContainerLiteralData),
    Index(Box<Expr>, Box<Expr>),

    StructNew(String, Vec<(String, Expr)>),

    ApiCall(ApiModule, Vec<Expr>),
    Range(Box<Expr>, Box<Expr>), // start..end
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
    pub span: Option<SourceSpan>, // Source location of this expression
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
    Lt, // <
    Gt, // >
    Le, // <=
    Ge, // >=
    Eq, // ==
    Ne, // !=
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
    pub attributes: Vec<String>, // List of attribute names
}
