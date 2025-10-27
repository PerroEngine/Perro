use crate::lang::ast_modules::ApiModule;


#[derive(Debug, Clone)]
pub struct Script {
    pub node_type: String,
    pub exposed: Vec<Variable>,
    pub variables: Vec<Variable>,
    pub functions: Vec<Function>,

    pub structs: Vec<StructDef>,
}

#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub typ: Option<Type>, 
    pub value: Option<TypedExpr>, 
}



impl Variable {
    pub fn rust_type(&self) -> String {
        match &self.typ {
            Some(t) => t.to_rust_type(),
            None => panic!("Type inference unresolved for variable"),
        }
    }

   pub fn parse_type(s: &str) -> Type {
    match s {
        // Signed
        "i8" | "i16" | "i32" | "i64" | "i128" => {
            let size = s.trim_start_matches('i').parse().unwrap();
            Type::Number(NumberKind::Signed(size))
        }

        // Unsigned
        "u8" | "u16" | "u32" | "u64" | "u128" => {
            let size = s.trim_start_matches('u').parse().unwrap();
            Type::Number(NumberKind::Unsigned(size))
        }

        // Float
        "f16" => Type::Number(NumberKind::Float(16)),
        "f32" => Type::Number(NumberKind::Float(32)),
        "f64" => Type::Number(NumberKind::Float(64)),
        "f128" => Type::Number(NumberKind::Float(128)),

        "bool" => Type::Bool,
        "String" => Type::String,
        "&str" => Type::StrRef,
        other => Type::Custom(other.to_string()),
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

        Type::Number(NumberKind::BigInt) => { 
            ("as_str", format!(".parse::<BigInt>().unwrap()"))
        }

        Type::Bool =>
            ("as_bool", "".into()),

        Type::String | Type::StrRef =>
            ("as_str", ".to_string()".into()),

        Type::Script =>
            ("as_str", ".parse().unwrap()".into()),

        Type::Custom(type_name) => {
            ("__CUSTOM__", type_name.clone())
        }

        Type::Void =>
            panic!("Void invalid"),
    }
}


pub fn rust_initialization(&self, script: &Script) -> String {
if let Some(expr) = &self.value {
    expr.to_rust(false, script) // let TypedExpr handle type propagation
} else {
    self.default_value()
}
}


pub fn default_value(&self) -> String {
    match &self.typ {
        Some(Type::Number(NumberKind::Signed(w))) => format!("0i{}", w),
        Some(Type::Number(NumberKind::Unsigned(w))) => format!("0u{}", w),
        Some(Type::Number(NumberKind::Float(w))) => match w {
            16 => "half::f16::from_f32(0.0)".to_string(),
            32 => "0.0f32".to_string(),
            64 => "0.0f64".to_string(),
            128 => "0.0f128".to_string(),
            _ => "0.0f64".to_string(),
        },

        Some(Type::Number(NumberKind::Decimal)) => "Decimal::from_str(\"0\").unwrap()".to_string(),
        Some(Type::Number(NumberKind::BigInt)) => "BigInt::from_str(\"0\").unwrap()".to_string(),

        Some(Type::Bool) => "false".into(),
        Some(Type::Script) => "None".into(),
        Some(Type::String) => "\"\".to_string()".into(),
        Some(Type::StrRef) => "\"\"".into(),
        Some(Type::Custom(_)) => "Default::default()".into(),
        Some(Type::Void) => panic!("Void invalid"),
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
    Custom(String),
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
            Type::Script => "Option<ScriptType>".to_string(),
            Type::Custom(name) => name.clone(),
            Type::Void => "()".to_string(),
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
        (Type::Number(Float(s)), Type::Number(Float(t))) if s < t => true,

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

}

#[derive(Debug, Clone, PartialEq)]
pub enum NumberKind {
    Signed(u8),   // width: 8,16,32,64,128
    Unsigned(u8), // width: ^
    Float(u8),    // 16,32,64,128
    Decimal,
    BigInt
}





#[derive(Debug, Clone)]
pub enum Stmt {
    Expr(TypedExpr),
    VariableDecl(Variable),                    // Update Variable to use TypedExpr internally
    Assign(String, TypedExpr),
    AssignOp(String, Op, TypedExpr),
    MemberAssign(TypedExpr, TypedExpr),        // Both LHS and RHS need types
    MemberAssignOp(TypedExpr, Op, TypedExpr),  // Both need types
    ScriptAssign(String, String, TypedExpr),   // RHS needs type
    ScriptAssignOp(String, String, Op, TypedExpr), // RHS needs type
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

    ObjectLiteral(Vec<(String, Expr)>),

    ApiCall(ApiModule, Vec<Expr>),
}

#[derive(Debug, Clone)]
pub struct TypedExpr {
    pub expr: Expr,
    pub inferred_type: Option<Type>,  // Gets filled during type inference
}

#[derive(Debug, Clone)]
pub enum Literal {
    Number(String),           // "5", "3.14", "999999999", etc.
    String(String),           
    Bool(bool),
    Interpolated(String),
}

#[derive(Debug, Clone)]
pub enum Op {
    Add, Sub, Mul, Div,
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