//! Code generation: AST → Rust.
//!
//! **Language-agnostic design:** Codegen consumes the shared AST (Script, Module, Expr, Stmt).
//! It does not branch on source language for control flow or naming. The only language-specific
//! coupling is node/engine API resolution: "is this member a node API field?" and "is this
//! module call an engine API (Time, Console, …)?" are currently answered by PUP's APIs
//! (`PUP_NODE_API`, `PupAPI`). For full language-agnosticism we would either (a) resolve those
//! at parse time and store in the AST, or (b) pass a backend (e.g. `dyn NodeApi` / `dyn EngineApi`)
//! into codegen so each language (PUP, TypeScript, C#) supplies its own resolver.

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
