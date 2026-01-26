//! TypeScript Node API Registry
//! Defines the public-facing API for each node type that scripts can access.
//! Uses camelCase naming conventions for TypeScript.
//! This is separate from engine_registry which is purely internal Rust representation.

use crate::node_registry::NodeType;
use crate::scripting::node_api_common::{NodeApiField, NodeApiMethod, NodeApiRegistry};
use crate::structs::engine_registry::{NodeFieldRef, NodeMethodRef};
use std::ops::{Deref, DerefMut};

/// TypeScript's node API registry (newtype wrapper)
pub struct TypeScriptNodeApiRegistry(NodeApiRegistry);

impl Deref for TypeScriptNodeApiRegistry {
    type Target = NodeApiRegistry;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TypeScriptNodeApiRegistry {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl TypeScriptNodeApiRegistry {
    pub fn new() -> Self {
        let mut registry = Self(NodeApiRegistry::new());
        registry.register_all();
        registry
    }

    /// Register all node APIs with TypeScript naming conventions (camelCase)
    fn register_all(&mut self) {
        // Base Node - minimal public API with NodeSugar built-in methods
        self.register_node(
            NodeType::Node,
            None,
            vec![
                NodeApiField {
                    script_name: "name",
                    rust_field: NodeFieldRef::NodeName,
                },
            ],
            vec![
                NodeApiMethod {
                    script_name: "getVar",
                    rust_method: NodeMethodRef::GetVar,
                },
                NodeApiMethod {
                    script_name: "setVar",
                    rust_method: NodeMethodRef::SetVar,
                },
                NodeApiMethod {
                    script_name: "getNode",
                    rust_method: NodeMethodRef::GetChildByName,
                },
                NodeApiMethod {
                    script_name: "getParent",
                    rust_method: NodeMethodRef::GetParent,
                },
                NodeApiMethod {
                    script_name: "addChild",
                    rust_method: NodeMethodRef::AddChild,
                },
                NodeApiMethod {
                    script_name: "clearChildren",
                    rust_method: NodeMethodRef::ClearChildren,
                },
                NodeApiMethod {
                    script_name: "getType",
                    rust_method: NodeMethodRef::GetType,
                },
                NodeApiMethod {
                    script_name: "getParentType",
                    rust_method: NodeMethodRef::GetParentType,
                },
                NodeApiMethod {
                    script_name: "remove",
                    rust_method: NodeMethodRef::Remove,
                },
            ],
        );

        // Node2D
        self.register_node(
            NodeType::Node2D,
            Some(NodeType::Node),
            vec![
                NodeApiField {
                    script_name: "transform",
                    rust_field: NodeFieldRef::Node2DTransform,
                },
                NodeApiField {
                    script_name: "pivot",
                    rust_field: NodeFieldRef::Node2DPivot,
                },
                NodeApiField {
                    script_name: "visible",
                    rust_field: NodeFieldRef::Node2DVisible,
                },
                NodeApiField {
                    script_name: "zIndex",
                    rust_field: NodeFieldRef::Node2DZIndex,
                },
            ],
            vec![],
        );

        // Sprite2D
        self.register_node(
            NodeType::Sprite2D,
            Some(NodeType::Node2D),
            vec![
                NodeApiField {
                    script_name: "texture",
                    rust_field: NodeFieldRef::Sprite2DTextureId,
                },
                NodeApiField {
                    script_name: "region",
                    rust_field: NodeFieldRef::Sprite2DRegion,
                },
            ],
            vec![],
        );

        // Area2D
        self.register_node(
            NodeType::Area2D,
            Some(NodeType::Node2D),
            vec![],
            vec![],
        );

        // CollisionShape2D
        self.register_node(
            NodeType::CollisionShape2D,
            Some(NodeType::Node2D),
            vec![],
            vec![],
        );

        // ShapeInstance2D
        self.register_node(
            NodeType::ShapeInstance2D,
            Some(NodeType::Node2D),
            vec![
                NodeApiField {
                    script_name: "shapeType",
                    rust_field: NodeFieldRef::Shape2DShapeType,
                },
                NodeApiField {
                    script_name: "color",
                    rust_field: NodeFieldRef::Shape2DColor,
                },
                NodeApiField {
                    script_name: "filled",
                    rust_field: NodeFieldRef::Shape2DFilled,
                },
            ],
            vec![],
        );

        // Camera2D
        self.register_node(
            NodeType::Camera2D,
            Some(NodeType::Node2D),
            vec![
                NodeApiField {
                    script_name: "zoom",
                    rust_field: NodeFieldRef::Camera2DZoom,
                },
                NodeApiField {
                    script_name: "active",
                    rust_field: NodeFieldRef::Camera2DActive,
                },
            ],
            vec![],
        );

        // UINode
        self.register_node(
            NodeType::UINode,
            Some(NodeType::Node),
            vec![],
            vec![],
        );

        // Node3D
        self.register_node(
            NodeType::Node3D,
            Some(NodeType::Node),
            vec![
                NodeApiField {
                    script_name: "transform",
                    rust_field: NodeFieldRef::Node3DTransform,
                },
                NodeApiField {
                    script_name: "pivot",
                    rust_field: NodeFieldRef::Node3DPivot,
                },
                NodeApiField {
                    script_name: "visible",
                    rust_field: NodeFieldRef::Node3DVisible,
                },
            ],
            vec![],
        );

        // MeshInstance3D
        self.register_node(
            NodeType::MeshInstance3D,
            Some(NodeType::Node3D),
            vec![],
            vec![],
        );

        // Camera3D
        self.register_node(
            NodeType::Camera3D,
            Some(NodeType::Node3D),
            vec![],
            vec![],
        );

        // DirectionalLight3D
        self.register_node(
            NodeType::DirectionalLight3D,
            Some(NodeType::Node3D),
            vec![],
            vec![],
        );

        // OmniLight3D
        self.register_node(
            NodeType::OmniLight3D,
            Some(NodeType::Node3D),
            vec![],
            vec![],
        );

        // SpotLight3D
        self.register_node(
            NodeType::SpotLight3D,
            Some(NodeType::Node3D),
            vec![],
            vec![],
        );
    }

}

/// Global node API registry for TypeScript
pub static TYPESCRIPT_NODE_API: once_cell::sync::Lazy<TypeScriptNodeApiRegistry> =
    once_cell::sync::Lazy::new(|| TypeScriptNodeApiRegistry::new());
