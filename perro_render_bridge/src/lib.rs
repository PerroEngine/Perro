use perro_ids::{MaterialID, MeshID, NodeID, TextureID};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderRequestID(pub u64);

impl RenderRequestID {
    #[inline]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Camera2DState {
    pub position: [f32; 2],
    pub rotation_radians: f32,
    pub zoom: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera3DState {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub zoom: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmbientLight3DState {
    pub color: [f32; 3],
    pub intensity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayLight3DState {
    pub direction: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointLight3DState {
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpotLight3DState {
    pub position: [f32; 3],
    pub direction: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
    pub inner_angle_radians: f32,
    pub outer_angle_radians: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material3D {
    pub base_color_factor: [f32; 4],
    pub roughness_factor: f32,
    pub metallic_factor: f32,
    pub occlusion_strength: f32,
    pub emissive_factor: [f32; 3],
    pub alpha_mode: u32, // 0=OPAQUE, 1=MASK, 2=BLEND
    pub alpha_cutoff: f32,
    pub double_sided: bool,
    pub normal_scale: f32,
    // Texture slot indices (glTF material-local index or NONE).
    pub base_color_texture: u32,
    pub metallic_roughness_texture: u32,
    pub normal_texture: u32,
    pub occlusion_texture: u32,
    pub emissive_texture: u32,
}

pub const MATERIAL_TEXTURE_NONE: u32 = u32::MAX;

impl Default for Material3D {
    fn default() -> Self {
        Self {
            base_color_factor: [0.85, 0.85, 0.85, 1.0],
            roughness_factor: 0.5,
            metallic_factor: 0.0,
            occlusion_strength: 1.0,
            emissive_factor: [0.0, 0.0, 0.0],
            alpha_mode: 0,
            alpha_cutoff: 0.5,
            double_sided: false,
            normal_scale: 1.0,
            base_color_texture: MATERIAL_TEXTURE_NONE,
            metallic_roughness_texture: MATERIAL_TEXTURE_NONE,
            normal_texture: MATERIAL_TEXTURE_NONE,
            occlusion_texture: MATERIAL_TEXTURE_NONE,
            emissive_texture: MATERIAL_TEXTURE_NONE,
        }
    }
}

impl Default for Camera3DState {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            zoom: 1.0,
        }
    }
}

impl Default for Camera2DState {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0],
            rotation_radians: 0.0,
            zoom: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Rect2DCommand {
    pub center: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
    pub z_index: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sprite2DCommand {
    pub texture: TextureID,
    pub model: [[f32; 3]; 3],
    pub z_index: i32,
}

#[derive(Debug, Clone)]
pub enum ResourceCommand {
    CreateMesh {
        request: RenderRequestID,
        owner: NodeID,
        source: String,
    },
    CreateTexture {
        request: RenderRequestID,
        owner: NodeID,
        source: String,
    },
    CreateMaterial {
        request: RenderRequestID,
        owner: NodeID,
        material: Material3D,
        source: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub enum Command2D {
    UpsertSprite {
        node: NodeID,
        sprite: Sprite2DCommand,
    },
    UpsertRect {
        node: NodeID,
        rect: Rect2DCommand,
    },
    RemoveNode {
        node: NodeID,
    },
    SetCamera {
        camera: Camera2DState,
    },
}

#[derive(Debug, Clone)]
pub enum Command3D {
    Draw {
        mesh: MeshID,
        material: MaterialID,
        node: NodeID,
        model: [[f32; 4]; 4],
    },
    SetCamera {
        camera: Camera3DState,
    },
    SetAmbientLight {
        node: NodeID,
        light: AmbientLight3DState,
    },
    SetRayLight {
        node: NodeID,
        light: RayLight3DState,
    },
    SetPointLight {
        node: NodeID,
        light: PointLight3DState,
    },
    SetSpotLight {
        node: NodeID,
        light: SpotLight3DState,
    },
    RemoveNode {
        node: NodeID,
    },
}

#[derive(Debug, Clone)]
pub enum RenderCommand {
    Resource(ResourceCommand),
    TwoD(Command2D),
    ThreeD(Command3D),
}

#[derive(Debug, Clone)]
pub enum RenderEvent {
    MeshCreated {
        request: RenderRequestID,
        id: MeshID,
    },
    TextureCreated {
        request: RenderRequestID,
        id: TextureID,
    },
    MaterialCreated {
        request: RenderRequestID,
        id: MaterialID,
    },
    Failed {
        request: RenderRequestID,
        reason: String,
    },
}

pub trait RenderBridge {
    fn submit(&mut self, command: RenderCommand);

    fn submit_many<I>(&mut self, commands: I)
    where
        I: IntoIterator<Item = RenderCommand>,
    {
        for command in commands {
            self.submit(command);
        }
    }

    fn drain_events(&mut self, out: &mut Vec<RenderEvent>);
}
