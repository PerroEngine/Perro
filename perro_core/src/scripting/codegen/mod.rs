// Code generation module - refactored from monolithic codegen.rs
// This module handles transpiling AST to Rust code

mod analysis;
mod boilerplate;
mod cache;
mod expression;
mod file_io;
mod function;
mod literal;
mod operator;
mod script;
mod statement;
mod struct_def;
mod type_inference;
mod utils;

// Re-export public API from new modules
pub use boilerplate::{implement_script_boilerplate, implement_script_boilerplate_internal};
pub use file_io::{derive_rust_perro_script, write_to_crate};
pub use utils::{
    get_node_type, is_node_type, rename_function, rename_struct, rename_variable, type_is_node,
};

// Note: Expr, TypedExpr, and Stmt types are available from crate::scripting::ast
// The impl blocks for these types are in expression.rs and statement.rs
