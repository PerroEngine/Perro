// Code generation module - refactored from monolithic codegen.rs
// This module handles transpiling AST to Rust code

mod cache;
mod utils;
mod type_inference;
mod script;
mod struct_def;
mod function;
mod statement;
mod expression;
mod literal;
mod operator;
mod boilerplate;
mod file_io;
mod analysis;

// Re-export public API from new modules
pub use utils::{is_node_type, rename_function, rename_struct, rename_variable, type_is_node, get_node_type};
pub use file_io::{write_to_crate, derive_rust_perro_script};
pub use boilerplate::implement_script_boilerplate;

// Note: Expr, TypedExpr, and Stmt types are available from crate::scripting::ast
// The impl blocks for these types are in expression.rs and statement.rs

