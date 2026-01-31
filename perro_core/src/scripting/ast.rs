use std::collections::HashMap;

use crate::scripting::source_span::SourceSpan;
use crate::{engine_structs::EngineStruct, node_registry::NodeType};

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
    pub language: Option<String>,    // Language identifier (e.g., "pup", "typescript", "csharp")
    pub module_names: std::collections::HashSet<String>, // Known module names (e.g., "Utils") for module access detection
    pub module_name_to_identifier: std::collections::HashMap<String, String>, // Map module name -> file identifier (e.g., "Utils" -> "module_pup")
    pub module_functions: std::collections::HashMap<String, Vec<Function>>, // Map module name -> list of functions for type inference
    pub module_variables: std::collections::HashMap<String, Vec<Variable>>, // Map module name -> list of variables (constants) for type inference
    /// When generating module function bodies: module-level constants (for Ident/Assign resolution with transpiled names)
    pub module_scope_variables: Option<Vec<Variable>>,
    /// True if this script is a @global (always extends Node internally, no explicit extend).
    pub is_global: bool,
    /// Known global names (e.g., "Utils", "Root") for global access detection.
    pub global_names: std::collections::HashSet<String>,
    /// Map global name -> NodeID. Set by transpiler: "Root" -> 1; @global names -> 2, 3, 4... in alphabetical order. Single source of truth for codegen.
    pub global_name_to_node_id: std::collections::HashMap<String, u32>,
    /// Rust struct name for this script (e.g. "TypesTsScript"). Set at codegen start so mutate_node closure type is valid when node_type is empty.
    pub rust_struct_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Module {
    pub module_name: String, // The module name from @module Name
    pub variables: Vec<Variable>,
    pub functions: Vec<Function>,
    pub structs: Vec<StructDef>,
    pub verbose: bool,
    pub attributes: HashMap<String, Vec<String>>, // member name -> list of attribute names
    pub source_file: Option<String>, // Original source file path (e.g., "res://utils.pup")
    pub language: Option<String>,    // Language identifier (e.g., "pup")
}

#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub typ: Option<Type>,
    pub value: Option<TypedExpr>,
    pub is_public: bool,
    pub is_exposed: bool,
    pub is_const: bool, // True if declared with 'const', false for 'var' or 'let'
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
            "object" | "Object" | "Value" => Type::Object, // Legacy support
            "Any" | "any" => Type::Any,                    // Preferred dynamic type

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
                if let Some(node_type) = ENGINE_REGISTRY
                    .node_defs
                    .keys()
                    .find(|nt| format!("{:?}", nt) == name)
                {
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
            Type::Option(_) => ("as_str", ".to_string()".into()), // Options are handled specially
            // Signal is now Type::Signal, handled above

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
            Type::Object | Type::Any => ("as_object", ".clone().into()".into()),

            Type::Signal => ("as_u64", "".into()),
            Type::NodeType => ("as_str", ".parse::<NodeType>().unwrap()".into()), // NodeType is serialized as string
            Type::ScriptApi => {
                panic!("ScriptApi cannot be deserialized from JSON - it's injected by the runtime")
            }
            Type::Custom(type_name) => {
                use crate::scripting::codegen::{is_node_type, rename_struct};
                if is_node_type(type_name) {
                    ("__CUSTOM__", type_name.clone())
                } else {
                    ("__CUSTOM__", rename_struct(type_name))
                }
            }
            Type::Void => panic!("Void invalid"),
            Type::Node(_node_type) => ("__NODE__", "NodeType".to_owned()),
            Type::DynNode => ("__NODE__", "NodeType".to_owned()), // DynNode is NodeID at runtime, same as Node types
            Type::EngineStruct(es) => {
                use crate::engine_structs::EngineStruct;
                match es {
                    EngineStruct::Texture => ("__CUSTOM__", "Option<TextureID>".to_string()), // Texture is Option<TextureID>, use custom deserialization
                    EngineStruct::Mesh => ("__CUSTOM__", "Option<MeshID>".to_string()), // Mesh is Option<MeshID>, use custom deserialization
                    _ => ("__ENGINE_STRUCT__", "EngineStruct".to_owned()), // Other engine structs use the generic method
                }
            }
        }
    }

    pub fn rust_initialization(&self, script: &Script, current_func: Option<&Function>) -> String {
        if let Some(expr) = &self.value {
            // Use the variable's declared type as the expected type for the expression
            // This ensures literals get the correct suffix (e.g., 42f64 instead of 42f32)
            let expected_type = self.typ.as_ref();
            // Call expr.expr.to_rust directly with the variable's type as expected_type
            expr.expr.to_rust(
                false,
                script,
                expected_type,
                current_func,
                expr.span.as_ref(),
            )
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
    pub attributes: Vec<String>,     // List of attribute names
    pub is_on_signal: bool, // True if this function was defined with "on SIGNALNAME()" syntax
    pub signal_name: Option<String>, // The signal name if this is an on-signal function
    pub is_lifecycle_method: bool, // True if this function was defined with "on init()" syntax (lifecycle method, not callable)
    pub span: Option<SourceSpan>,  // Source location of this function definition
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
    CowStr,            // Cow<'static, str> - for node name and other borrowed strings
    Option(Box<Type>), // Option<T>
    Signal,            // u64 - signal ID type
    Void,

    Container(ContainerKind, Vec<Type>),
    Object, // serde_json::Value - dynamic any type (legacy name, use Any)
    Any,    // serde_json::Value - dynamic any type (preferred name)

    Node(NodeType), // Node instance type - maps to NodeID in Rust
    DynNode, // Dynamic node type - no type resolution at compile time, resolved to UUID at runtime
    NodeType, // NodeType enum itself (e.g., from get_type())
    EngineStruct(EngineStruct),
    ScriptApi, // ScriptApi<'a> - runtime API context (internal use only)
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
            Type::Option(inner) => format!("Option<{}>", inner.to_rust_type()),

            // ---- Containers ----
            Type::Container(ContainerKind::Map, params) => {
                let k = params
                    .get(0)
                    .map_or("String".to_string(), |p| p.to_rust_type());
                let v = params.get(1).map_or("Value".to_string(), |p| {
                    // For custom types, use Value to allow polymorphism
                    match p {
                        Type::Custom(_) => "Value".to_string(), // User-defined custom types use Value for polymorphism
                        Type::Any => "Value".to_string(),
                        _ => p.to_rust_type(),
                    }
                });
                format!("HashMap<{}, {}>", k, v)
            }
            Type::Container(ContainerKind::Array, params) => {
                let val = params.get(0).map_or("Value".to_string(), |p| {
                    // For custom types, use Value to allow polymorphism
                    match p {
                        Type::Custom(_) => "Value".to_string(), // User-defined custom types use Value for polymorphism
                        Type::Any => "Value".to_string(),
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

            // ---- "Object" and "Any" (serde_json::Value) ----
            Type::Object | Type::Any => "Value".to_string(),

            Type::Signal => "u64".to_string(),
            Type::ScriptApi => "&mut ScriptApi<'_>".to_string(),
            Type::Custom(name) => {
                // Custom types are user-defined structs - rename with __t_ prefix
                // (but not node types or engine structs)
                use crate::scripting::codegen::{is_node_type, rename_struct};
                if is_node_type(name) {
                    // Node types should be Type::Node, but handle gracefully
                    name.clone()
                } else {
                    rename_struct(name)
                }
            }
            Type::Node(_) => "NodeID".to_string(), // Nodes map to NodeID
            Type::DynNode => "NodeID".to_string(), // DynNode also maps to NodeID
            Type::NodeType => "NodeType".to_string(), // NodeType enum
            Type::EngineStruct(es) => {
                // Engine structs map to their type-safe ID types where applicable
                use crate::engine_structs::EngineStruct;
                match es {
                    // Script-facing "Texture" and "Mesh" are optional handles (stored as Option<...> on nodes).
                    EngineStruct::Texture => "Option<TextureID>".to_string(),
                    EngineStruct::Mesh => "Option<MeshID>".to_string(),
                    _ => format!("{:?}", es), // Other engine structs are real types (Vector2, Color, etc.)
                }
            }
            Type::Void => "()".to_string(),
        }
    }

    /// Convert a Type to a PUP-friendly string representation
    /// PUP doesn't have Option types, so Option<T> becomes T? (nullable)
    pub fn to_pup_type(&self) -> String {
        use ContainerKind::*;
        use NumberKind::*;

        match self {
            Type::Number(Signed(32)) => "int".to_string(),
            Type::Number(Signed(64)) => "int64".to_string(),
            Type::Number(Signed(16)) => "int16".to_string(),
            Type::Number(Signed(8)) => "int8".to_string(),
            Type::Number(Signed(128)) => "int128".to_string(),
            Type::Number(Signed(w)) => format!("i{}", w),
            Type::Number(Unsigned(32)) => "uint".to_string(),
            Type::Number(Unsigned(64)) => "uint64".to_string(),
            Type::Number(Unsigned(16)) => "uint16".to_string(),
            Type::Number(Unsigned(8)) => "uint8".to_string(),
            Type::Number(Unsigned(128)) => "uint128".to_string(),
            Type::Number(Unsigned(w)) => format!("u{}", w),
            Type::Number(Float(32)) => "float".to_string(),
            Type::Number(Float(64)) => "double".to_string(),
            Type::Number(Float(w)) => format!("f{}", w),
            Type::Number(Decimal) => "decimal".to_string(),
            Type::Number(BigInt) => "bigint".to_string(),

            Type::Bool => "bool".to_string(),
            Type::String | Type::StrRef | Type::CowStr => "string".to_string(),

            // Options don't exist in PUP - treat as nullable with ?
            Type::Option(inner) => {
                // Special case: Option<[f32; 4]> is Rect? (check before recursing)
                if let Type::Container(FixedArray(4), inner_types) = inner.as_ref() {
                    if let Some(Type::Number(Float(32))) = inner_types.first() {
                        return "Rect?".to_string();
                    }
                }
                // For other types, add ? suffix
                format!("{}?", inner.to_pup_type())
            }

            // Containers
            Type::Container(Array, types) => {
                if let Some(inner) = types.first() {
                    format!("Array[{}]", inner.to_pup_type())
                } else {
                    "Array".to_string()
                }
            }
            Type::Container(Map, types) => {
                if types.len() >= 2 {
                    format!(
                        "Map<[{}: {}]>",
                        types[0].to_pup_type(),
                        types[1].to_pup_type()
                    )
                } else {
                    "Map".to_string()
                }
            }
            Type::Container(FixedArray(size), types) => {
                // Special case: [f32; 4] is Rect
                if *size == 4 {
                    if let Some(Type::Number(Float(32))) = types.first() {
                        return "Rect".to_string();
                    }
                }
                if let Some(inner) = types.first() {
                    format!("[{}; {}]", inner.to_pup_type(), size)
                } else {
                    format!("[unknown; {}]", size)
                }
            }

            Type::Any | Type::Object => "any".to_string(),
            Type::Signal => "signal".to_string(),
            Type::Node(node_type) => format!("{:?}", node_type),
            Type::DynNode => "Node".to_string(),
            Type::NodeType => "NODE_TYPE".to_string(),
            Type::Custom(name) => name.clone(),
            Type::EngineStruct(es) => {
                // Engine structs display as-is (Vector2, Rect, etc.)
                format!("{:?}", es)
            }
            Type::Void => "void".to_string(),
            Type::ScriptApi => "ScriptApi".to_string(),
        }
    }

    /// Convert a Type to a C#-friendly string representation
    pub fn to_csharp_type(&self) -> String {
        use ContainerKind::*;
        use NumberKind::*;

        match self {
            Type::Number(Signed(32)) => "int".to_string(),
            Type::Number(Signed(64)) => "long".to_string(),
            Type::Number(Signed(16)) => "short".to_string(),
            Type::Number(Signed(8)) => "sbyte".to_string(),
            Type::Number(Signed(128)) => "Int128".to_string(),
            Type::Number(Signed(w)) => format!("Int{}", w),
            Type::Number(Unsigned(32)) => "uint".to_string(),
            Type::Number(Unsigned(64)) => "ulong".to_string(),
            Type::Number(Unsigned(16)) => "ushort".to_string(),
            Type::Number(Unsigned(8)) => "byte".to_string(),
            Type::Number(Unsigned(128)) => "UInt128".to_string(),
            Type::Number(Unsigned(w)) => format!("UInt{}", w),
            Type::Number(Float(32)) => "float".to_string(),
            Type::Number(Float(64)) => "double".to_string(),
            Type::Number(Float(w)) => format!("Float{}", w),
            Type::Number(Decimal) => "decimal".to_string(),
            Type::Number(BigInt) => "BigInteger".to_string(),

            Type::Bool => "bool".to_string(),
            Type::String | Type::StrRef | Type::CowStr => "string".to_string(),

            Type::Option(inner) => format!("{}?", inner.to_csharp_type()),

            Type::Container(Array, types) => {
                if let Some(inner) = types.first() {
                    format!("{}[]", inner.to_csharp_type())
                } else {
                    "object[]".to_string()
                }
            }
            Type::Container(Map, types) => {
                if types.len() >= 2 {
                    format!(
                        "Dictionary<{}, {}>",
                        types[0].to_csharp_type(),
                        types[1].to_csharp_type()
                    )
                } else {
                    "Dictionary<string, object>".to_string()
                }
            }
            Type::Container(FixedArray(size), types) => {
                if let Some(inner) = types.first() {
                    format!("{}[{}]", inner.to_csharp_type(), size)
                } else {
                    format!("object[{}]", size)
                }
            }

            Type::Any | Type::Object => "object".to_string(),
            Type::Signal => "ulong".to_string(),
            Type::Node(node_type) => format!("{:?}", node_type),
            Type::DynNode => "Node".to_string(),
            Type::NodeType => "NodeType".to_string(),
            Type::Custom(name) => name.clone(),
            Type::EngineStruct(es) => format!("{:?}", es),
            Type::Void => "void".to_string(),
            Type::ScriptApi => "ScriptApi".to_string(),
        }
    }

    /// Convert a Type to a TypeScript-friendly string representation
    pub fn to_typescript_type(&self) -> String {
        use ContainerKind::*;
        use NumberKind::*;

        match self {
            Type::Number(Signed(32))
            | Type::Number(Signed(64))
            | Type::Number(Signed(16))
            | Type::Number(Signed(8))
            | Type::Number(Signed(128)) => "number".to_string(),
            Type::Number(Signed(w)) => format!("number /* i{} */", w),
            Type::Number(Unsigned(32))
            | Type::Number(Unsigned(64))
            | Type::Number(Unsigned(16))
            | Type::Number(Unsigned(8))
            | Type::Number(Unsigned(128)) => "number".to_string(),
            Type::Number(Unsigned(w)) => format!("number /* u{} */", w),
            Type::Number(Float(32)) | Type::Number(Float(64)) => "number".to_string(),
            Type::Number(Float(w)) => format!("number /* f{} */", w),
            Type::Number(Decimal) => "number".to_string(),
            Type::Number(BigInt) => "bigint".to_string(),

            Type::Bool => "boolean".to_string(),
            Type::String | Type::StrRef | Type::CowStr => "string".to_string(),

            Type::Option(inner) => format!("{} | null", inner.to_typescript_type()),

            Type::Container(Array, types) => {
                if let Some(inner) = types.first() {
                    format!("{}[]", inner.to_typescript_type())
                } else {
                    "any[]".to_string()
                }
            }
            Type::Container(Map, types) => {
                if types.len() >= 2 {
                    format!(
                        "Map<{}, {}>",
                        types[0].to_typescript_type(),
                        types[1].to_typescript_type()
                    )
                } else {
                    "Map<string, any>".to_string()
                }
            }
            Type::Container(FixedArray(size), types) => {
                if let Some(inner) = types.first() {
                    format!("[{}; {}]", inner.to_typescript_type(), size)
                } else {
                    format!("[any; {}]", size)
                }
            }

            Type::Any | Type::Object => "any".to_string(),
            Type::Signal => "number".to_string(),
            Type::Node(node_type) => format!("{:?}", node_type),
            Type::DynNode => "Node".to_string(),
            Type::NodeType => "NodeType".to_string(),
            Type::Custom(name) => name.clone(),
            Type::EngineStruct(es) => format!("{:?}", es),
            Type::Void => "void".to_string(),
            Type::ScriptApi => "ScriptApi".to_string(),
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
            Type::Option(_) => "None".into(),

            Type::Object | Type::Any => "json!({})".into(),

            Type::Container(Map, _) => "HashMap::new()".into(),
            Type::Container(Array, _) => "Vec::new()".into(),
            Type::Container(FixedArray(size), params) => {
                let elem_val = params
                    .get(0)
                    .map(|p| p.rust_default_value())
                    .unwrap_or_else(|| "Default::default()".into());
                format!("[{}; {}]", elem_val, size)
            }

            Type::Custom(_) => "Default::default()".to_string(),
            Type::EngineStruct(es) => {
                use crate::engine_structs::EngineStruct;
                match es {
                    EngineStruct::Texture => "None".to_string(), // Texture is Option<TextureID>, default is None
                    EngineStruct::Mesh => "None".to_string(), // Mesh is Option<MeshID>, default is None
                    _ => format!("{}::default()", format!("{:?}", es)), // Other engine structs implement Default
                }
            }
            Type::Node(_) => {
                // Nodes are NodeID, use nil since it will be set later
                "NodeID::nil()".to_string()
            }
            Type::DynNode => {
                // DynNode is also a NodeID
                "NodeID::nil()".to_string()
            }
            Type::NodeType => {
                // NodeType enum default is NodeType::Node
                "NodeType::Node".to_string()
            }
            Type::ScriptApi => {
                panic!("ScriptApi cannot have a default value - it's injected by the runtime")
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
            // String -> StrRef (String can be converted to &str via .as_str())
            (Type::String, Type::StrRef) => true,
            // StrRef -> String (borrowed string can become owned String)
            (Type::StrRef, Type::String) => true,
            // String -> CowStr (owned string can become Cow::Owned)
            (Type::String, Type::CowStr) => true,
            // StrRef -> CowStr (borrowed string can become Cow::Borrowed)
            (Type::StrRef, Type::CowStr) => true,
            // CowStr -> String (Cow can be converted to owned String)
            (Type::CowStr, Type::String) => true,
            // Node / DynNode conversions (all are NodeID in Rust)
            (Type::DynNode, Type::Node(_)) => true, // DynNode can be cast to any Node type
            (Type::Node(_), Type::DynNode) => true, // Any Node type can become DynNode
            // Option<NodeID> -> NodeID (e.g. get_child_by_name result passed to get_script_var_id)
            (Type::Option(inner), Type::DynNode) if matches!(inner.as_ref(), Type::DynNode) => true,
            // UuidOption (script name for Option<NodeID>) -> NodeID
            (Type::Custom(name), Type::DynNode) if name == "UuidOption" => true,

            // T -> Option<T> conversions (wrapping in Some)
            (from, Type::Option(inner)) if *from == *inner.as_ref() => true,

            // Vector3 ↔ Vector2 (drop/pad z); Quaternion ↔ f32 (2D rotation)
            (
                Type::EngineStruct(EngineStruct::Vector3),
                Type::EngineStruct(EngineStruct::Vector2),
            ) => true,
            (
                Type::EngineStruct(EngineStruct::Vector2),
                Type::EngineStruct(EngineStruct::Vector3),
            ) => true,
            // Vector3 (Euler degrees) -> Quaternion (3D rotation)
            (
                Type::EngineStruct(EngineStruct::Vector3),
                Type::EngineStruct(EngineStruct::Quaternion),
            ) => true,
            (Type::EngineStruct(EngineStruct::Quaternion), Type::Number(Float(32))) => true,
            (Type::Number(Float(32)), Type::EngineStruct(EngineStruct::Quaternion)) => true,

            // Any and Object are equivalent (both are serde_json::Value)
            (Type::Any, Type::Object) | (Type::Object, Type::Any) => true,
            // Any type can convert to Any/Object (dynamic type)
            (_, Type::Any) | (_, Type::Object) => true,
            // Any/Object can convert to any type (dynamic type)
            (Type::Any, _) | (Type::Object, _) => true,

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
            // Node types are Copy (they're NodeID which is Copy)
            Node(_) => true,
            // DynNode is Copy (resolved to NodeID at runtime)
            DynNode => true,
            // NodeType enum implements Copy
            NodeType => true,
            // EngineStructs implement Copy
            EngineStruct(es) => match es {
                // These implement Copy
                ES::Vector2
                | ES::Vector3
                | ES::Transform2D
                | ES::Transform3D
                | ES::Color
                | ES::Rect
                | ES::Quaternion
                | ES::Shape2D => true,

                ES::Texture => true,
                // Texture is a TextureID (u64), Copy
                ES::Mesh => true,
                // Mesh is a MeshID (u64), Copy
            },
            // ScriptApi is a reference type, not Copy
            ScriptApi => false,
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
    Return(Option<TypedExpr>), // return expr; or return;
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
            Stmt::ScriptAssign(_, _, expr) | Stmt::ScriptAssignOp(_, _, _, expr) => {
                expr.span.as_ref()
            }
            Stmt::IndexAssign(_, _, expr) | Stmt::IndexAssignOp(_, _, _, expr) => {
                expr.span.as_ref()
            }
            Stmt::If { condition, .. } => condition.span.as_ref(),
            Stmt::For { iterable, .. } => iterable.span.as_ref(),
            Stmt::ForTraditional { condition, .. } => {
                condition.as_ref().and_then(|c| c.span.as_ref())
            }
            Stmt::Return(expr) => expr.as_ref().and_then(|e| e.span.as_ref()),
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

    ApiCall(crate::call_modules::CallModule, Vec<Expr>),
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
    pub span: Option<SourceSpan>,    // Source location of this expression
}

#[derive(Debug, Clone)]
pub enum Literal {
    Number(String), // "5", "3.14", "999999999", etc.
    String(String),
    Bool(bool),
    Null, // null literal for Option<T> types
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
