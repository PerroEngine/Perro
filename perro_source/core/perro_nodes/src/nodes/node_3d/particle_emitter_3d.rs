use crate::node_3d::Node3D;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug)]
pub struct ParticleEmitter3D {
    pub base: Node3D,
    pub active: bool,
    pub looping: bool,
    pub prewarm: bool,
    pub max_particles: u32,
    pub emission_rate: f32,
    pub duration: f32,
    pub lifetime_min: f32,
    pub lifetime_max: f32,
    pub speed_min: f32,
    pub speed_max: f32,
    pub spread_radians: f32,
    pub point_size: f32,
    pub size_min: f32,
    pub size_max: f32,
    pub gravity: [f32; 3],
    pub color_start: [f32; 4],
    pub color_end: [f32; 4],
    pub emissive: [f32; 3],
    pub seed: u32,
    pub params: Vec<f32>,
    pub particle: String,
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
            max_particles: 4096,
            emission_rate: 256.0,
            duration: 0.0,
            lifetime_min: 0.6,
            lifetime_max: 1.4,
            speed_min: 1.0,
            speed_max: 3.0,
            spread_radians: std::f32::consts::FRAC_PI_3,
            point_size: 6.0,
            size_min: 0.65,
            size_max: 1.35,
            gravity: [0.0, -3.0, 0.0],
            color_start: [1.0, 1.0, 1.0, 1.0],
            color_end: [1.0, 0.4, 0.1, 0.0],
            emissive: [0.0, 0.0, 0.0],
            seed: 1,
            params: Vec::new(),
            particle: String::new(),
        }
    }
}

impl Default for ParticleEmitter3D {
    fn default() -> Self {
        Self::new()
    }
}
