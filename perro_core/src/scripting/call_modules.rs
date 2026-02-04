// ----------------------------------------------------------------
// Unified Call Module Enum - wraps all call types
// This allows the AST to use a single enum for all API/resource/node calls
// ----------------------------------------------------------------

use crate::{
    api_modules::ApiModule, resource_modules::ResourceModule,
    structs::engine_registry::NodeMethodRef,
};

/// Unified enum for all call types (module APIs, resources, and node methods)
#[derive(Debug, Clone)]
pub enum CallModule {
    /// Module APIs (JSON, Time, OS, Console, Input, Math)
    Module(ApiModule),
    /// Resource APIs (Signal, Texture, Mesh, Scene, Shape, Array, Map)
    Resource(ResourceModule),
    /// Node methods from engine_registry (get_parent, get_node, etc.) - uses NodeMethodRef
    NodeMethod(NodeMethodRef),
}

use crate::ast::*;

impl CallModule {
    /// Primary entry point for code generation - routes to appropriate handler
    pub fn to_rust(
        &self,
        args: &[Expr],
        script: &Script,
        needs_self: bool,
        current_func: Option<&Function>,
    ) -> String {
        match self {
            CallModule::Module(api) => api.to_rust(args, script, needs_self, current_func),
            CallModule::Resource(resource) => {
                resource.to_rust(args, script, needs_self, current_func)
            }
            CallModule::NodeMethod(method_ref) => {
                use crate::api_bindings::generate_rust_args;
                use crate::scripting::ast::Type;
                use crate::structs::engine_bindings::EngineMethodCodegen;
                // Receiver (node id) is first arg; param_types() omits it, so prepend DynNode so Option<NodeID> gets unwrapped
                let expected_arg_types = method_ref.param_types().map(|p| {
                    let mut full = vec![Type::DynNode];
                    full.extend(p);
                    full
                });
                let rust_args_strings = generate_rust_args(
                    args,
                    script,
                    needs_self,
                    current_func,
                    expected_arg_types.as_ref(),
                );
                method_ref.to_rust_prepared(
                    args,
                    &rust_args_strings,
                    script,
                    needs_self,
                    current_func,
                )
            }
        }
    }

    pub fn return_type(&self) -> Option<Type> {
        match self {
            CallModule::Module(api) => api.return_type(),
            CallModule::Resource(resource) => resource.return_type(),
            CallModule::NodeMethod(method_ref) => method_ref.return_type(),
        }
    }

    pub fn param_types(&self) -> Option<Vec<Type>> {
        match self {
            CallModule::Module(api) => api.param_types(),
            CallModule::Resource(resource) => resource.param_types(),
            CallModule::NodeMethod(method_ref) => method_ref.param_types(),
        }
    }

    /// Get script-side parameter names (what PUP users see)
    pub fn param_names(&self) -> Option<Vec<&'static str>> {
        match self {
            CallModule::Module(api) => api.param_names(),
            CallModule::Resource(resource) => resource.param_names(),
            CallModule::NodeMethod(method_ref) => method_ref.param_names(),
        }
    }
}
