use super::*;

#[derive(Debug, Clone)]
pub enum ResourceCommand {
    CreateMesh {
        request: RenderRequestID,
        id: MeshID,
        source: String,
        reserved: bool,
    },
    CreateRuntimeMesh {
        request: RenderRequestID,
        id: MeshID,
        source: String,
        reserved: bool,
        mesh: Mesh3D,
    },
    CreateRuntimeMeshBytes {
        request: RenderRequestID,
        id: MeshID,
        source: String,
        reserved: bool,
        bytes: Arc<[u8]>,
    },
    WriteMeshData {
        id: MeshID,
        mesh: Mesh3D,
    },
    CreateTexture {
        request: RenderRequestID,
        id: TextureID,
        source: String,
        reserved: bool,
    },
    CreateRuntimeTexture {
        request: RenderRequestID,
        id: TextureID,
        source: String,
        reserved: bool,
        width: u32,
        height: u32,
        rgba: Arc<[u8]>,
    },
    CreateRuntimeTextureBytes {
        request: RenderRequestID,
        id: TextureID,
        source: String,
        reserved: bool,
        bytes: Arc<[u8]>,
    },
    CreateMaterial {
        request: RenderRequestID,
        id: MaterialID,
        material: Material3D,
        source: Option<String>,
        reserved: bool,
    },
    SetSceneResourceRefs {
        textures: Vec<(TextureID, Vec<NodeID>)>,
        meshes: Vec<(MeshID, Vec<NodeID>)>,
        materials: Vec<(MaterialID, Vec<NodeID>)>,
    },
    WriteMaterialData {
        id: MaterialID,
        material: Material3D,
    },
    SetMeshReserved {
        id: MeshID,
        reserved: bool,
    },
    SetTextureReserved {
        id: TextureID,
        reserved: bool,
    },
    SetMaterialReserved {
        id: MaterialID,
        reserved: bool,
    },
    DropMesh {
        id: MeshID,
    },
    DropTexture {
        id: TextureID,
    },
    DropMaterial {
        id: MaterialID,
    },
}

#[derive(Debug, Clone)]
pub enum Command2D {
    UpsertCameraStream {
        node: NodeID,
        stream: Box<CameraStreamState>,
        sprite: Sprite2DCommand,
    },
    UpsertSprite {
        node: NodeID,
        sprite: Sprite2DCommand,
    },
    UpsertTileMap {
        node: NodeID,
        tilemap: TileMap2DCommand,
    },
    UpsertRect {
        node: NodeID,
        rect: Rect2DCommand,
    },
    UpsertPointParticles {
        node: NodeID,
        particles: Box<PointParticles2DState>,
    },
    UpsertWater {
        node: NodeID,
        water: Box<Water2DState>,
    },
    SetAmbientLight {
        node: NodeID,
        light: AmbientLight2DState,
    },
    SetRayLight {
        node: NodeID,
        light: RayLight2DState,
    },
    SetPointLight {
        node: NodeID,
        light: PointLight2DState,
    },
    SetSpotLight {
        node: NodeID,
        light: SpotLight2DState,
    },
    RemoveNode {
        node: NodeID,
    },
    SetCamera {
        camera: Camera2DState,
    },
    DrawShape {
        draw: DrawShape2DCommand,
    },
}

#[derive(Debug, Clone)]
pub enum Command3D {
    UpsertCameraStream {
        node: NodeID,
        stream: Box<CameraStreamState>,
        quad: CameraStream3DState,
    },
    Draw {
        mesh: MeshID,
        surfaces: Arc<[MeshSurfaceBinding3D]>,
        node: NodeID,
        model: [[f32; 4]; 4],
        skeleton: Option<SkeletonPalette>,
        blend_shape_weights: Arc<[f32]>,
        meshlet_override: Option<bool>,
        lod: LODOptions3D,
        blend: MeshBlendOptions3D,
        cast_shadows: bool,
        receive_shadows: bool,
    },
    DrawMulti {
        mesh: MeshID,
        surfaces: Arc<[MeshSurfaceBinding3D]>,
        node: NodeID,
        instance_mats: Arc<[[[f32; 4]; 4]]>,
        skeleton: Option<SkeletonPalette>,
        blend_shape_weights: Arc<[f32]>,
        meshlet_override: Option<bool>,
        lod: LODOptions3D,
        blend: MeshBlendOptions3D,
        cast_shadows: bool,
        receive_shadows: bool,
    },
    DrawMultiDense {
        mesh: MeshID,
        surfaces: Arc<[MeshSurfaceBinding3D]>,
        node: NodeID,
        node_model: [[f32; 4]; 4],
        instance_scale: f32,
        instances: Arc<[DenseInstancePose3D]>,
        blend_shape_weights: Arc<[f32]>,
        meshlet_override: Option<bool>,
        lod: LODOptions3D,
        blend: MeshBlendOptions3D,
        cast_shadows: bool,
        receive_shadows: bool,
    },
    DrawDebugPoint3D {
        node: NodeID,
        position: [f32; 3],
        size: f32,
        color: [f32; 4],
    },
    DrawDebugLine3D {
        node: NodeID,
        start: [f32; 3],
        end: [f32; 3],
        thickness: f32,
        color: [f32; 4],
    },
    SetCamera {
        camera: Camera3DState,
    },
    SetAmbientLight {
        node: NodeID,
        light: AmbientLight3DState,
    },
    SetSky {
        node: NodeID,
        sky: Box<Sky3DState>,
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
    UpsertPointParticles {
        node: NodeID,
        particles: Box<PointParticles3DState>,
    },
    UpsertWater {
        node: NodeID,
        water: Box<Water3DState>,
    },
    RemoveNode {
        node: NodeID,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct DenseInstancePose3D {
    pub position: [f32; 3],
    pub scale: [f32; 3],
    pub rotation: [f32; 4],
    pub has_blend_shape_weight_override: bool,
    pub blend_shape_weights: Arc<[f32]>,
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum RenderCommand {
    Resource(ResourceCommand),
    CameraStream(CameraStreamCommand),
    TwoD(Command2D),
    ThreeD(Box<Command3D>),
    Ui(UiCommand),
    PostProcessing(PostProcessingCommand),
    VisualAccessibility(VisualAccessibilityCommand),
}

#[derive(Debug, Clone)]
pub enum PostProcessingCommand {
    SetGlobal(PostProcessSet),
    AddGlobalNamed {
        name: Cow<'static, str>,
        effect: PostProcessEffect,
    },
    AddGlobalUnnamed(PostProcessEffect),
    RemoveGlobalByName(Cow<'static, str>),
    RemoveGlobalByIndex(usize),
    ClearGlobal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VisualAccessibilityCommand {
    EnableColorBlind {
        mode: ColorBlindFilter,
        strength: f32,
    },
    DisableColorBlind,
}

#[derive(Debug, Clone)]
pub enum RenderEvent {
    MeshCreated {
        request: RenderRequestID,
        id: MeshID,
        mesh: Option<Mesh3D>,
    },
    TextureCreated {
        request: RenderRequestID,
        id: TextureID,
    },
    TextureLoaded {
        id: TextureID,
    },
    MaterialCreated {
        request: RenderRequestID,
        id: MaterialID,
    },
    MaterialLoaded {
        id: MaterialID,
    },
    MeshDropped {
        id: MeshID,
    },
    TextureDropped {
        id: TextureID,
    },
    MaterialDropped {
        id: MaterialID,
    },
    WaterSamples {
        samples: Arc<[WaterSampleState]>,
    },
    WaterBodySamples {
        samples: Arc<[WaterBodySampleState]>,
    },
    Failed {
        request: RenderRequestID,
        reason: String,
    },
}

#[derive(Debug, Clone)]
pub enum CameraStreamCommand {
    Upsert {
        node: NodeID,
        state: Box<CameraStreamState>,
    },
    RemoveNode {
        node: NodeID,
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
