pub mod nodes;
pub use nodes::*;

pub mod structs;
pub use structs::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Transform2D;
    use crate::Vector2;
    use crate::mesh_instance_3d::MeshInstance3D;
    use crate::node_2d::node_2d::Node2D;
    use crate::node_3d::node_3d::Node3D;
    use crate::sprite_2d::Sprite2D;
    use perro_ids::NodeID;
    use std::borrow::Cow;

    #[test]
    fn test_create_and_print_scene_nodes() {
        // Create a base Node
        let mut root = SceneNode::new(SceneNodeData::Node);
        root.id = NodeID::new(1);
        root.name = Cow::Borrowed("Root");

        println!("=== Base Node ===");
        println!("{:#?}", root);
        println!("Type: {}", root.node_type());
        println!("Spatial: {:?}", root.spatial());
        println!("Is 2D: {}, Is 3D: {}", root.is_2d(), root.is_3d());
        println!();

        // Create a Node2D
        let mut node_2d = SceneNode::new(SceneNodeData::Node2D(Node2D {
            transform: Transform2D {
                position: Vector2::new(100.0, 200.0),
                rotation: 0.0,
                scale: Vector2::new(1.0, 1.0),
            },
            visible: true,
            z_index: 0,
        }));
        node_2d.id = NodeID::new(2);
        node_2d.name = Cow::Borrowed("Player");
        node_2d.parent = NodeID::new(1);

        println!("=== Node2D ===");
        println!("{:#?}", node_2d);
        println!("Type: {}", node_2d.node_type());
        println!("Spatial: {:?}", node_2d.spatial());
        println!("Is 2D: {}, Is 3D: {}", node_2d.is_2d(), node_2d.is_3d());
        println!();

        // Create a Sprite2D (contains Node2D as base)
        let mut sprite = SceneNode::new(SceneNodeData::Sprite2D(Sprite2D {
            base: Node2D {
                transform: Transform2D {
                    position: Vector2::new(0.0, 0.0),
                    rotation: 0.0,
                    scale: Vector2::new(1.0, 1.0),
                },
                visible: true,
                z_index: 1,
            },
            texture_id: perro_ids::TextureID::nil(),
        }));
        sprite.id = NodeID::new(3);
        sprite.name = Cow::Borrowed("PlayerSprite");
        sprite.parent = NodeID::new(2);

        println!("=== Sprite2D ===");
        println!("{:#?}", sprite);
        println!("Type: {}", sprite.node_type());
        println!("Spatial: {:?}", sprite.spatial());
        println!("Is 2D: {}, Is 3D: {}", sprite.is_2d(), sprite.is_3d());
        println!();

        // Create a MeshInstance3D (contains Node3D as base)
        let mut mesh = SceneNode::new(SceneNodeData::MeshInstance3D(MeshInstance3D {
            base: Node3D::default(),
            mesh_id: perro_ids::MeshID::nil(),
            material_id: perro_ids::MaterialID::nil(),
        }));
        mesh.id = NodeID::new(4);
        mesh.name = Cow::Borrowed("Character");

        println!("=== MeshInstance3D ===");
        println!("{:#?}", mesh);
        println!("Type: {}", mesh.node_type());
        println!("Spatial: {:?}", mesh.spatial());
        println!("Is 2D: {}, Is 3D: {}", mesh.is_2d(), mesh.is_3d());
        println!();

        // Test adding children
        root.add_child(NodeID::new(2));
        node_2d.add_child(NodeID::new(3));

        println!("=== Root with children ===");
        println!("{:#?}", root);
        println!("Children: {:?}", root.children_slice());
        println!();

        println!("=== Node2D with children ===");
        println!("{:#?}", node_2d);
        println!("Children: {:?}", node_2d.children_slice());
        println!();

        // Show how Sprite2D inherits Node2D properties
        if let SceneNodeData::Sprite2D(sprite_data) = &sprite.data {
            println!("=== Sprite2D Base Properties ===");
            println!("Position: {:?}", sprite_data.base.transform.position);
            println!("Visible: {}", sprite_data.base.visible);
            println!("Z-Index: {}", sprite_data.base.z_index);
        }
    }

    #[test]
    fn test_node_hierarchy() {
        println!("=== Building Scene Tree ===");

        let mut root = SceneNode::new(SceneNodeData::Node);
        root.id = NodeID::new(1);
        root.name = Cow::Borrowed("Root");

        let mut player = SceneNode::new(SceneNodeData::Node2D(Node2D::new()));
        player.id = NodeID::new(2);
        player.name = Cow::Borrowed("Player");
        player.parent = root.id;

        let mut sprite = SceneNode::new(SceneNodeData::Sprite2D(Sprite2D {
            base: Node2D::new(),
            texture_id: perro_ids::TextureID::nil(),
        }));
        sprite.id = NodeID::new(3);
        sprite.name = Cow::Borrowed("Sprite");
        sprite.parent = player.id;

        root.add_child(player.id);
        player.add_child(sprite.id);

        println!("Root -> {:?}", root.children_slice());
        println!("Player -> {:?}", player.children_slice());
        println!("Sprite parent: {:?}", sprite.parent);
    }
}
