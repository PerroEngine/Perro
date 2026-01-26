//! Shared Node API Registry
//! Common types and registry implementation for all languages.
//! Each language module provides its own naming convention and registry instance.

use std::collections::HashMap;
use crate::ast::Type;
use crate::node_registry::NodeType;
use crate::structs::engine_registry::{NodeFieldRef, NodeMethodRef};

/// Represents a field exposed to scripts
/// This is purely a name mapping - types come from engine_registry via NodeFieldRef
#[derive(Debug, Clone)]
pub struct NodeApiField {
    /// The name scripts use (language-specific naming convention)
    pub script_name: &'static str,
    /// Strongly-typed reference to the engine_registry field (which contains the type)
    pub rust_field: NodeFieldRef,
}

/// Represents a method exposed to scripts
/// Similar to NodeApiField - uses NodeMethodRef to get types from engine_registry
#[derive(Debug, Clone)]
pub struct NodeApiMethod {
    /// The name scripts use (language-specific naming convention)
    pub script_name: &'static str,
    /// Strongly-typed reference to the engine_registry method (which contains the types)
    pub rust_method: NodeMethodRef,
}

/// Node API definition - what's exposed to scripts for a node type
#[derive(Debug, Clone)]
pub struct NodeApiDef {
    pub node_type: NodeType,
    pub fields: Vec<NodeApiField>,
    pub methods: Vec<NodeApiMethod>,
}

/// Registry of node APIs - what scripts can access
/// This is a generic registry that works for all languages.
/// Each language creates its own instance with language-specific naming.
pub struct NodeApiRegistry {
    /// Node type -> API definition
    apis: HashMap<NodeType, NodeApiDef>,
}

impl NodeApiField {
    /// Get the script type from engine_registry via NodeFieldRef
    pub fn get_script_type(&self) -> Type {
        self.rust_field.rust_type()
            .unwrap_or(Type::Object)
    }
}

impl NodeApiMethod {
    /// Get the return type from engine_registry via NodeMethodRef
    pub fn get_return_type(&self) -> Option<Type> {
        self.rust_method.return_type()
    }
    
    /// Get the parameter types from engine_registry via NodeMethodRef
    pub fn get_param_types(&self) -> Option<Vec<Type>> {
        self.rust_method.param_types()
    }
    
    /// Get the parameter names from engine_registry via NodeMethodRef
    /// Returns script-side parameter names (what PUP users see), not internal Rust names
    pub fn get_param_names(&self) -> Option<Vec<&'static str>> {
        self.rust_method.param_names()
    }
}

impl NodeApiRegistry {
    pub fn new() -> Self {
        Self {
            apis: HashMap::new(),
        }
    }

    /// Get the API definition for a node type
    pub fn get_api(&self, node_type: &NodeType) -> Option<&NodeApiDef> {
        self.apis.get(node_type)
    }

    /// Get all fields exposed to scripts for a node type (including inherited)
    pub fn get_fields(&self, node_type: &NodeType) -> Vec<&NodeApiField> {
        let mut fields = Vec::new();
        let mut current_type = Some(*node_type);
        let mut seen = std::collections::HashSet::new();

        while let Some(nt) = current_type {
            if let Some(api) = self.apis.get(&nt) {
                for field in &api.fields {
                    if seen.insert(&field.script_name) {
                        fields.push(field);
                    }
                }
            }
            // Walk up inheritance chain
            current_type = crate::structs::engine_registry::ENGINE_REGISTRY.node_bases.get(&nt).cloned();
        }

        fields
    }

    /// Get all methods exposed to scripts for a node type (including inherited)
    pub fn get_methods(&self, node_type: &NodeType) -> Vec<&NodeApiMethod> {
        let mut methods = Vec::new();
        let mut current_type = Some(*node_type);
        let mut seen = std::collections::HashSet::new();

        while let Some(nt) = current_type {
            if let Some(api) = self.apis.get(&nt) {
                for method in &api.methods {
                    if seen.insert(&method.script_name) {
                        methods.push(method);
                    }
                }
            }
            // Walk up inheritance chain
            current_type = crate::structs::engine_registry::ENGINE_REGISTRY.node_bases.get(&nt).cloned();
        }

        methods
    }

    /// Register a node API definition
    pub fn register_node(
        &mut self,
        node_type: NodeType,
        _base: Option<NodeType>,
        fields: Vec<NodeApiField>,
        methods: Vec<NodeApiMethod>,
    ) {
        self.apis.insert(
            node_type,
            NodeApiDef {
                node_type,
                fields,
                methods,
            },
        );
    }
}
