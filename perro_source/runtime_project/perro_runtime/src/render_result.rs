use perro_ids::{MaterialID, MeshID, TextureID};

#[derive(Debug, Clone)]
pub enum RuntimeRenderResult {
    Mesh(MeshID),
    Texture(TextureID),
    Material(MaterialID),
    Failed(String),
}
