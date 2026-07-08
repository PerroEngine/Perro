use crate::node_2d::Node2D;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParticleEmitterSimMode2D {
    Default,
    Cpu,
}

#[derive(Clone, Debug)]
pub struct ParticleEmitter2D {
    pub base: Node2D,
    pub active: bool,
    pub looping: bool,
    pub prewarm: bool,
    pub spawn_rate: f32,
    pub seed: u32,
    pub params: Vec<f32>,
    pub profile: String,
    pub sim_mode: ParticleEmitterSimMode2D,
    #[doc(hidden)]
    pub internal_simulation_time: f32,
    #[doc(hidden)]
    pub internal_prev_active: bool,
    #[doc(hidden)]
    pub internal_finished_emitted: bool,
    #[doc(hidden)]
    pub internal_lifetime_max: f32,
}

impl Deref for ParticleEmitter2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for ParticleEmitter2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl ParticleEmitter2D {
    pub fn new() -> Self {
        Self {
            base: Node2D::new(),
            active: true,
            looping: true,
            prewarm: false,
            spawn_rate: 256.0,
            seed: 1,
            params: Vec::new(),
            profile: String::new(),
            sim_mode: ParticleEmitterSimMode2D::Default,
            internal_simulation_time: 0.0,
            internal_prev_active: true,
            internal_finished_emitted: false,
            internal_lifetime_max: 1.0,
        }
    }
}

impl Default for ParticleEmitter2D {
    fn default() -> Self {
        Self::new()
    }
}
