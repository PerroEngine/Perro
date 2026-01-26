// ----------------------------------------------------------------
// Resource API Enums - Types/resources that can be instantiated
// These are agnostic and don't necessarily have exact API equivalents
// They apply to Nodes and resources like Signal, Texture, Shape, Array, Map
// ----------------------------------------------------------------

/// Unified enum for all resource APIs
#[derive(Debug, Clone)]
pub enum ResourceModule {
    Signal(SignalResource),
    Texture(TextureResource),
    Shape(ShapeResource),
    ArrayOp(ArrayResource),
    MapOp(MapResource),
}

#[derive(Debug, Clone)]
pub enum SignalResource {
    New,
    Connect,
    Emit,
    EmitDeferred,
}

#[derive(Debug, Clone)]
pub enum TextureResource {
    Load, 
    CreateFromBytes,
    GetWidth, 
    GetHeight, 
    GetSize, 
}

#[derive(Debug, Clone)]
pub enum ShapeResource {
    Rectangle, 
    Circle,    
    Square,    
    Triangle,  
}

#[derive(Debug, Clone)]
pub enum ArrayResource {
    Push,
    Pop,
    Insert,
    Remove,
    Len,

    New,
}

#[derive(Debug, Clone)]
pub enum MapResource {
    Insert,
    Remove,
    Get,
    Contains,
    Len,
    Clear,

    New,
}
