use perro_ids::{MaterialID, MeshID, NodeID, TextureID};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderRequestID(pub u64);

impl RenderRequestID {
    #[inline]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }
}

#[derive(Debug, Clone)]
pub enum RenderCommand {
    CreateMesh {
        request: RenderRequestID,
        owner: NodeID,
    },
    CreateTexture {
        request: RenderRequestID,
        owner: NodeID,
    },
    CreateMaterial {
        request: RenderRequestID,
        owner: NodeID,
    },
    Draw {
        mesh: MeshID,
        material: MaterialID,
        node: NodeID,
    },
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
