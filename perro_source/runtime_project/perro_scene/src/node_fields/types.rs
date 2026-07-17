#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeField {
    Node2D(Node2DField),
    Node3D(Node3DField),
    Camera2D(Camera2DField),
    CameraStream(CameraStreamField),
    Webcam(WebcamField),
    Button2D(Button2DField),
    ImageButton2D(Button2DField),
    NineSliceButton2D(Button2DField),
    NineSlice2D(Button2DField),
    Sprite2D(Sprite2DField),
    Sprite3D(Sprite2DField),
    AnimatedSprite2D(AnimatedSprite2DField),
    ParticleEmitter2D(ParticleEmitter2DField),
    WaterBody2D(WaterBodyField),
    Light2D(Light2DField),
    RayLight2D(RayLight2DField),
    PointLight2D(PointLight2DField),
    SpotLight2D(SpotLight2DField),
    TileMap2D(TileMap2DField),
    Skeleton2D(Skeleton2DField),
    BoneAttachment2D(BoneAttachment2DField),
    IKTarget2D(IKTarget2DField),
    PhysicsBoneChain2D(PhysicsBoneChain2DField),
    BoneCollider2D(BoneCollider2DField),
    CollisionShape2D(CollisionShape2DField),
    StaticBody2D(StaticBody2DField),
    RigidBody2D(RigidBody2DField),
    CharacterBody2D(CharacterBodyField),
    PhysicsForceEmitter2D(PhysicsForceEmitterField),
    Area2D(Area2DField),
    PinJoint2D(Joint2DField),
    DistanceJoint2D(DistanceJoint2DField),
    FixedJoint2D(Joint2DField),
    MeshInstance3D(MeshInstance3DField),
    Skeleton3D(Skeleton3DField),
    BoneAttachment3D(BoneAttachment3DField),
    IKTarget3D(IKTarget3DField),
    PhysicsBoneChain3D(PhysicsBoneChain3DField),
    BoneCollider3D(BoneCollider3DField),
    Camera3D(Camera3DField),
    ParticleEmitter3D(ParticleEmitter3DField),
    WaterBody3D(WaterBodyField),
    AnimationPlayer(AnimationPlayerField),
    AnimationTree(AnimationTreeField),
    Light3D(Light3DField),
    Sky3D(Sky3DField),
    RayLight3D(RayLight3DField),
    PointLight3D(PointLight3DField),
    SpotLight3D(SpotLight3DField),
    CollisionShape3D(CollisionShape3DField),
    StaticBody3D(StaticBody3DField),
    RigidBody3D(RigidBody3DField),
    CharacterBody3D(CharacterBodyField),
    PhysicsForceEmitter3D(PhysicsForceEmitterField),
    Area3D(Area3DField),
    BallJoint3D(Joint3DField),
    HingeJoint3D(HingeJoint3DField),
    FixedJoint3D(Joint3DField),
    UiNode(UiNodeField),
    UiImage(UiImageField),
    UiImageButton(UiImageField),
    UiNineSliceButton(UiImageField),
    UiNineSlice(UiImageField),
    UiAnimatedImage(UiAnimatedImageField),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraStreamField {
    Camera,
    Resolution,
    Width,
    Height,
    AspectRatio,
    AspectMode,
    PostProcessing,
    Enabled,
    Size,
    ZIndex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebcamField {
    Device,
    Resolution,
    Width,
    Height,
    Fps,
    Mirror,
    CpuFrames,
    Enabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Node2DField {
    Position,
    Rotation,
    Scale,
    Visible,
    Modulate,
    SelfModulate,
    ChildrenModulate,
    ZIndex,
    RenderLayers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Node3DField {
    Position,
    Rotation,
    Scale,
    Visible,
    Modulate,
    SelfModulate,
    ChildrenModulate,
    RenderLayers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiNodeField {
    Position,
    Scale,
    Rotation,
    Visible,
    Modulate,
    SelfModulate,
    ChildrenModulate,
    InputEnabled,
    ClipChildren,
    ZIndex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Camera2DField {
    Zoom,
    RenderMask,
    PostProcessing,
    AudioOptions,
    AudioMask,
    Active,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sprite2DField {
    Texture,
    TextureRegion,
    FlipX,
    FlipY,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button2DField {
    Size,
    Texture,
    TextureRegion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimatedSprite2DField {
    Texture,
    Animations,
    FlipX,
    FlipY,
    CurrentAnimation,
    CurrentFrame,
    FpsScale,
    Playing,
    Looping,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParticleEmitter2DField {
    Active,
    Looping,
    Prewarm,
    SpawnRate,
    Seed,
    Params,
    Profile,
    SimMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaterBodyField {
    Shape,
    Resolution,
    RenderResolution,
    VerticesPerMeter,
    SimCellsPerMeter,
    RenderVerticesPerMeter,
    Depth,
    Flow,
    Wind,
    IdleMode,
    WaveSpeed,
    WaveScale,
    WaveLength,
    WakeStrength,
    FoamStrength,
    Damping,
    Buoyancy,
    Drag,
    SampleReadbackRate,
    LodNearDistance,
    LodMidDistance,
    LodFarDistance,
    LodMinResolution,
    CollisionLayers,
    CollisionMask,
    LinkLayers,
    LinkMask,
    BlendWidth,
    WaveTransfer,
    FlowTransfer,
    DeepColor,
    ShallowColor,
    ShallowDepth,
    SkyBias,
    Optics,
    Material,
    Transparency,
    Reflectivity,
    Roughness,
    FresnelPower,
    NormalStrength,
    RippleScale,
    FoamColor,
    FoamAmount,
    CrestFoamThreshold,
    CausticStrength,
    RefractionStrength,
    ScatteringStrength,
    DistanceFogStrength,
    Coastline,
    Debug,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Light2DField {
    Color,
    Intensity,
    CastShadows,
    ShadowSoftness,
    ShadowSamples,
    Active,
    RenderLayers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RayLight2DField {
    Visible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointLight2DField {
    Range,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpotLight2DField {
    Range,
    InnerAngleRadians,
    OuterAngleRadians,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileMap2DField {
    Tileset,
    Width,
    Height,
    EmptyTile,
    Tiles,
    CollisionEnabled,
    CollisionLayers,
    CollisionMask,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skeleton2DField {
    Skeleton,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollisionShape2DField {
    Shape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaticBody2DField {
    Enabled,
    CollisionLayers,
    CollisionMask,
    Friction,
    Restitution,
    Density,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RigidBody2DField {
    Enabled,
    CollisionLayers,
    CollisionMask,
    ContinuousCollisionDetection,
    Mass,
    LinearVelocity,
    AngularVelocity,
    GravityScale,
    LinearDamping,
    AngularDamping,
    CanSleep,
    LockRotation,
    Friction,
    Restitution,
    Density,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area2DField {
    Enabled,
    CollisionLayers,
    CollisionMask,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhysicsForceEmitterField {
    Enabled,
    Profile,
    Radius,
    Strength,
    Duration,
    Pulse,
    Falloff,
    AffectBodies,
    AffectWater,
    CollisionLayers,
    CollisionMask,
    Vectors,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Joint2DField {
    BodyA,
    BodyB,
    AnchorA,
    AnchorB,
    Enabled,
    CollideConnected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DistanceJoint2DField {
    Common(Joint2DField),
    MinDistance,
    MaxDistance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshInstance3DField {
    Mesh,
    Material,
    Surfaces,
    Model,
    Skeleton,
    BlendShapeWeights,
    FlipX,
    FlipY,
    FlipZ,
    InstanceGrid,
    Meshlets,
    MinLod,
    MaxLod,
    CastShadows,
    ReceiveShadows,
    Blend,
    BlendEnabled,
    BlendScreen,
    BlendNormals,
    BlendLayers,
    BlendMask,
    BlendDistance,
    BlendMinDistance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skeleton3DField {
    Skeleton,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoneAttachment2DField {
    Skeleton,
    BoneIndex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IKTarget2DField {
    Skeleton,
    BoneIndex,
    ChainLength,
    Iterations,
    Tolerance,
    Weight,
    MatchRotation,
    Solver,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhysicsBoneChain2DField {
    Skeleton,
    BoneIndex,
    ChainLength,
    Enabled,
    Gravity,
    Damping,
    Stiffness,
    Radius,
    Collisions,
    Iterations,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoneCollider2DField {
    Enabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoneAttachment3DField {
    Skeleton,
    BoneIndex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IKTarget3DField {
    Skeleton,
    BoneIndex,
    ChainLength,
    Iterations,
    Tolerance,
    Weight,
    MatchRotation,
    Solver,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhysicsBoneChain3DField {
    Skeleton,
    BoneIndex,
    ChainLength,
    Enabled,
    Gravity,
    Damping,
    Stiffness,
    Radius,
    Collisions,
    Iterations,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoneCollider3DField {
    Enabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Camera3DField {
    Zoom,
    RenderMask,
    Projection,
    PerspectiveFovYDegrees,
    PerspectiveNear,
    PerspectiveFar,
    OrthographicSize,
    OrthographicNear,
    OrthographicFar,
    FrustumLeft,
    FrustumRight,
    FrustumBottom,
    FrustumTop,
    FrustumNear,
    FrustumFar,
    PostProcessing,
    AudioOptions,
    AudioMask,
    Active,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParticleEmitter3DField {
    Active,
    Looping,
    Prewarm,
    SpawnRate,
    Seed,
    Params,
    Profile,
    SimMode,
    RenderMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationPlayerField {
    Animation,
    Bindings,
    Speed,
    Paused,
    Playback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationTreeField {
    Tree,
    Animations,
    Bindings,
    Speed,
    Paused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Light3DField {
    Color,
    Intensity,
    CastShadows,
    Shadow,
    ShadowStrength,
    ShadowDepthBias,
    ShadowNormalBias,
    Active,
    RenderLayers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RayLight3DField {
    Visible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sky3DField {
    Palette,
    DayColors,
    EveningColors,
    NightColors,
    HorizonColors,
    Environment,
    Time,
    TimeOfDay,
    TimePaused,
    TimeScale,
    Shaders,
    Active,
    RenderLayers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointLight3DField {
    Range,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpotLight3DField {
    Range,
    InnerAngleRadians,
    OuterAngleRadians,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollisionShape3DField {
    Shape,
    Trimesh,
    FlipX,
    FlipY,
    FlipZ,
    Debug,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaticBody3DField {
    Enabled,
    CollisionLayers,
    CollisionMask,
    Friction,
    Restitution,
    Density,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RigidBody3DField {
    Enabled,
    CollisionLayers,
    CollisionMask,
    ContinuousCollisionDetection,
    Mass,
    LinearVelocity,
    AngularVelocity,
    GravityScale,
    LinearDamping,
    AngularDamping,
    CanSleep,
    Friction,
    Restitution,
    Density,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharacterBodyField {
    Enabled,
    CollisionLayers,
    CollisionMask,
    Friction,
    Restitution,
    Density,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area3DField {
    Enabled,
    CollisionLayers,
    CollisionMask,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Joint3DField {
    BodyA,
    BodyB,
    AnchorA,
    AnchorB,
    Enabled,
    CollideConnected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HingeJoint3DField {
    Common(Joint3DField),
    Axis,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiImageField {
    Texture,
    TextureRegion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiAnimatedImageField {
    Texture,
    Animations,
    CurrentAnimation,
    CurrentFrame,
    FpsScale,
    Playing,
    Looping,
    TextureRegion,
}
