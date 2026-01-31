//! C# Node API Registry
//! Defines the public-facing API for each node type that scripts can access.
//! Uses PascalCase naming conventions for C#.
//! This is separate from engine_registry which is purely internal Rust representation.

use crate::node_registry::NodeType;
use crate::scripting::node_api_common::{NodeApiField, NodeApiMethod, NodeApiRegistry};
use crate::structs::engine_registry::{NodeFieldRef, NodeMethodRef};
use std::ops::{Deref, DerefMut};

/// C#'s node API registry (newtype wrapper)
pub struct CSharpNodeApiRegistry(NodeApiRegistry);

impl Deref for CSharpNodeApiRegistry {
    type Target = NodeApiRegistry;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for CSharpNodeApiRegistry {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl CSharpNodeApiRegistry {
    pub fn new() -> Self {
        let mut registry = Self(NodeApiRegistry::new());
        registry.register_all();
        registry
    }

    /// Register all node APIs with C# naming conventions (PascalCase)
    fn register_all(&mut self) {
        // Base Node - minimal public API with NodeSugar built-in methods
        self.register_node(
            NodeType::Node,
            None,
            vec![NodeApiField {
                script_name: "Name",
                rust_field: NodeFieldRef::NodeName,
            }],
            vec![
                NodeApiMethod {
                    script_name: "GetVar",
                    rust_method: NodeMethodRef::GetVar,
                },
                NodeApiMethod {
                    script_name: "SetVar",
                    rust_method: NodeMethodRef::SetVar,
                },
                NodeApiMethod {
                    script_name: "GetNode",
                    rust_method: NodeMethodRef::GetChildByName,
                },
                NodeApiMethod {
                    script_name: "GetParent",
                    rust_method: NodeMethodRef::GetParent,
                },
                NodeApiMethod {
                    script_name: "AddChild",
                    rust_method: NodeMethodRef::AddChild,
                },
                NodeApiMethod {
                    script_name: "ClearChildren",
                    rust_method: NodeMethodRef::ClearChildren,
                },
                NodeApiMethod {
                    script_name: "GetType",
                    rust_method: NodeMethodRef::GetType,
                },
                NodeApiMethod {
                    script_name: "GetParentType",
                    rust_method: NodeMethodRef::GetParentType,
                },
                NodeApiMethod {
                    script_name: "Remove",
                    rust_method: NodeMethodRef::Remove,
                },
                NodeApiMethod {
                    script_name: "Call",
                    rust_method: NodeMethodRef::CallFunction,
                },
                NodeApiMethod {
                    script_name: "CallDeferred",
                    rust_method: NodeMethodRef::CallDeferred,
                },
            ],
        );

        // Node2D
        self.register_node(
            NodeType::Node2D,
            Some(NodeType::Node),
            vec![
                NodeApiField {
                    script_name: "Transform",
                    rust_field: NodeFieldRef::Node2DTransform,
                },
                NodeApiField {
                    script_name: "GlobalTransform",
                    rust_field: NodeFieldRef::Node2DGlobalTransform,
                },
                NodeApiField {
                    script_name: "Pivot",
                    rust_field: NodeFieldRef::Node2DPivot,
                },
                NodeApiField {
                    script_name: "Visible",
                    rust_field: NodeFieldRef::Node2DVisible,
                },
                NodeApiField {
                    script_name: "ZIndex",
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
                    script_name: "Texture",
                    rust_field: NodeFieldRef::Sprite2DTextureId,
                },
                NodeApiField {
                    script_name: "Region",
                    rust_field: NodeFieldRef::Sprite2DRegion,
                },
            ],
            vec![],
        );

        // Area2D
        self.register_node(NodeType::Area2D, Some(NodeType::Node2D), vec![], vec![]);

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
                    script_name: "ShapeType",
                    rust_field: NodeFieldRef::Shape2DShapeType,
                },
                NodeApiField {
                    script_name: "Color",
                    rust_field: NodeFieldRef::Shape2DColor,
                },
                NodeApiField {
                    script_name: "Filled",
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
                    script_name: "Zoom",
                    rust_field: NodeFieldRef::Camera2DZoom,
                },
                NodeApiField {
                    script_name: "Active",
                    rust_field: NodeFieldRef::Camera2DActive,
                },
            ],
            vec![],
        );

        // UINode
        self.register_node(NodeType::UINode, Some(NodeType::Node), vec![], vec![]);

        // Node3D
        self.register_node(
            NodeType::Node3D,
            Some(NodeType::Node),
            vec![
                NodeApiField {
                    script_name: "Transform",
                    rust_field: NodeFieldRef::Node3DTransform,
                },
                NodeApiField {
                    script_name: "GlobalTransform",
                    rust_field: NodeFieldRef::Node3DGlobalTransform,
                },
                NodeApiField {
                    script_name: "Pivot",
                    rust_field: NodeFieldRef::Node3DPivot,
                },
                NodeApiField {
                    script_name: "Visible",
                    rust_field: NodeFieldRef::Node3DVisible,
                },
            ],
            vec![],
        );

        // MeshInstance3D
        self.register_node(
            NodeType::MeshInstance3D,
            Some(NodeType::Node3D),
            vec![NodeApiField {
                script_name: "mesh",
                rust_field: NodeFieldRef::MeshInstance3DMeshId,
            }],
            vec![],
        );

        // Camera3D
        self.register_node(NodeType::Camera3D, Some(NodeType::Node3D), vec![], vec![]);

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

/// Global node API registry for C#
pub static CSHARP_NODE_API: once_cell::sync::Lazy<CSharpNodeApiRegistry> =
    once_cell::sync::Lazy::new(|| CSharpNodeApiRegistry::new());
