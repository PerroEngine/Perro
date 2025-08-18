
#[derive(Debug, Clone)]
pub struct Script {
    pub node_type: String,
    pub exports: Vec<Variable>,
    pub variables: Vec<Variable>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub typ: Option<Type>, 
    pub value: Option<Expr>, 
}



impl Variable {
        pub fn rust_type(&self) -> String {
        match &self.typ {
            Some(Type::Float)  => "f32".to_string(),
            Some(Type::Int)    => "i32".to_string(),
            Some(Type::Number) => "f32".to_string(),
            Some(Type::Bool)   => "bool".to_string(),
            Some(Type::String) => "String".to_string(),
            Some(Type::Void)   => panic!("Void type cannot be used as field type"),
            Some(Type::Custom(name)) => name.clone(),
            None => panic!("Cannot convert None type to Rust — type inference not resolved yet"),
        }
    }

    pub fn rust_initialization(&self, script: &Script) -> String {
        if let Some(expr) = &self.value {
            // Generate Rust code from expression, pass type if needed for type hints
            expr.to_rust(false, script, self.typ.as_ref())
        } else {
            self.default_value()
        }
    }


    pub fn default_value(&self) -> String {
    match &self.typ {
        Some(Type::Float)  => "0.0f32".to_string(),
        Some(Type::Int)    => "0i32".to_string(),
        Some(Type::Number) => "0.0f32".to_string(),
        Some(Type::Bool)   => "false".to_string(),
        Some(Type::String) => "\"\".to_string()".to_string(),
        Some(Type::Void)   => panic!("Void type cannot be used as field type"),
        Some(Type::Custom(_)) => "Default::default()".to_string(),
        None => panic!("Cannot generate default value for inferred type"),
    }
}

}




#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
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
    Float,
    Int,
    Number,
    Void,
    Bool,
    String,
    Custom(String),
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Expr(Expr),
    VariableDecl(Variable),
    Assign(String, Expr),
    AssignOp(String, Op, Expr),
    MemberAssign(Expr, Expr), // <-- These are crucial!
    MemberAssignOp(Expr, Op, Expr), // <-- These are crucial!
    ScriptAssign(String /*var*/, String /*field*/, Expr /*rhs*/),
    ScriptAssignOp(String /*var*/, String, Op /*+=, etc.*/, Expr /*rhs*/),
    Call(String, Vec<Expr>),
    Pass,
}

#[derive(Debug, Clone)]
pub enum Builtin {
    GetDelta,
    GetTime,
    GetUnixTime,
    IsEditor,
    FrameCount,
    TargetFps,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Ident(String),
    Literal(Literal),
    BinaryOp(Box<Expr>, Op, Box<Expr>),
    MemberAccess(Box<Expr>, String),
    ScriptAccess(Box<Expr>, String),
    SelfAccess,
    Call(String, Vec<Expr>),
}

#[derive(Debug, Clone)]
pub enum Literal {
    Int(i32),
    Float(f32),
    Number(f32),
    String(String),
    Bool(bool)
}

#[derive(Debug, Clone)]
pub enum Op {
    Add, Sub, Mul, Div,
}