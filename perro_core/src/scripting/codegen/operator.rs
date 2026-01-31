// Operator code generation
use crate::scripting::ast::Op;

impl Op {
    pub(crate) fn to_rust(&self) -> &'static str {
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
            Op::And => "&&",
        }
    }

    pub(crate) fn to_rust_assign(&self) -> &'static str {
        match self {
            Op::Add => "+",
            Op::Sub => "-",
            Op::Mul => "*",
            Op::Div => "/",
            Op::Lt | Op::Gt | Op::Le | Op::Ge | Op::Eq | Op::Ne | Op::And => {
                panic!("Comparison/logical operators cannot be used in assignment")
            }
        }
    }
}
