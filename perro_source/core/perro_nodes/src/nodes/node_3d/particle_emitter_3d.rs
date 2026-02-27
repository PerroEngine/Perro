use crate::node_3d::Node3D;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParticleEmitterSimMode3D {
    Default,
    Cpu,
    GpuVertex,
    GpuCompute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParticleEmitterRenderMode3D {
    Point,
    Billboard,
}

#[derive(Clone, Debug)]
pub struct ParticleEmitter3D {
    pub base: Node3D,
    pub active: bool,
    pub looping: bool,
    pub prewarm: bool,
    pub spawn_rate: f32,
    pub seed: u32,
    pub params: Vec<f32>,
    pub profile: String,
    pub sim_mode: ParticleEmitterSimMode3D,
    pub render_mode: ParticleEmitterRenderMode3D,
}

impl Deref for ParticleEmitter3D {
    type Target = Node3D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for ParticleEmitter3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl ParticleEmitter3D {
    pub fn new() -> Self {
        Self {
            base: Node3D::new(),
            active: true,
            looping: true,
            prewarm: false,
            spawn_rate: 256.0,
            seed: 1,
            params: Vec::new(),
            profile: String::new(),
            sim_mode: ParticleEmitterSimMode3D::Default,
            render_mode: ParticleEmitterRenderMode3D::Point,
        }
    }
}

impl Default for ParticleEmitter3D {
    fn default() -> Self {
        Self::new()
    }
}
