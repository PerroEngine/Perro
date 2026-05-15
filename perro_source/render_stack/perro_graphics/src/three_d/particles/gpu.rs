use super::shaders::{
    create_point_particles_compute_render_shader_module,
    create_point_particles_compute_shader_module, create_point_particles_gpu_shader_module,
    create_point_particles_shader_module,
};
use ahash::AHashMap;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};
use perro_ids::NodeID;
use perro_particle_math::{Op, ParticleEvalInput, Program, compile_expression, eval_ops_particle};
use perro_render_bridge::{
    Camera3DState, CameraProjectionState, ParticlePath3D, ParticleRenderMode3D,
    ParticleSimulationMode3D, PointParticles3DState,
};

#[path = "gpu/buffers.rs"]
mod buffers;
#[path = "gpu/emitters.rs"]
mod emitters;
#[path = "gpu/helpers.rs"]
mod helpers;
#[path = "gpu/init.rs"]
mod init;
#[path = "gpu/prepare.rs"]
mod prepare;
#[path = "gpu/render_pass.rs"]
mod render_pass;

use helpers::*;

const PARTICLE_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
    inv_view_size: [f32; 2],
    _pad: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PointParticleGpu {
    world_pos: [f32; 3],
    size_alpha: [f32; 2],
    color: [f32; 4],
    emissive: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuEmitterParticle {
    model_0: [f32; 4],
    model_1: [f32; 4],
    model_2: [f32; 4],
    model_3: [f32; 4],
    gravity_path: [f32; 4], // xyz gravity, w path kind
    color_start: [f32; 4],
    color_end: [f32; 4],
    emissive_point: [f32; 4],   // xyz emissive, w size
    life_speed: [f32; 4],       // life_min, life_max, speed_min, speed_max
    size_spread_rate: [f32; 4], // size_min, size_max, spread_radians, emission_rate
    time_path: [f32; 4],        // simulation_time, path_a, path_b, simulation_delta
    counts_seed: [u32; 4],      // start, count, max_alive_budget, seed
    flags: [u32; 4],            // looping, prewarm, spin_bits, spawn_origin_base
    custom_ops_xy: [u32; 4],    // x_off, x_len, y_off, y_len
    custom_ops_zp: [u32; 4],    // z_off, z_len, params_off, params_len
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuEmitterParams {
    emitter_count: u32,
    particle_count: u32,
    _pad: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuComputedParticle {
    world_pos: [f32; 4],
    color: [f32; 4],
    emissive: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuExprOp {
    words: [u32; 4], // opcode, arg_bits, reserved, reserved
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InstanceRange {
    start: u32,
    count: u32,
    path_kind: u32,
}

#[derive(Clone, Copy)]
struct SpawnOriginEntry {
    origin: [f32; 3],
    rotation: [f32; 4],
    last_seen_generation: u64,
}

struct SpawnRingState {
    base: u32,
    capacity: u32,
    slot_spawn_keys: Vec<u32>,
}

pub struct PreparePointParticles3D<'a> {
    pub camera: Camera3DState,
    pub emitters: &'a [(NodeID, PointParticles3DState)],
    pub width: u32,
    pub height: u32,
}

pub struct GpuPointParticles3D {
    cpu_pipeline: wgpu::RenderPipeline,
    cpu_billboard_pipeline: wgpu::RenderPipeline,
    hybrid_pipeline: wgpu::RenderPipeline,
    hybrid_billboard_pipeline: wgpu::RenderPipeline,
    compute_pipeline: wgpu::ComputePipeline,
    compute_render_pipeline: wgpu::RenderPipeline,
    compute_render_billboard_pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bg: wgpu::BindGroup,
    hybrid_emitters_bgl: wgpu::BindGroupLayout,
    hybrid_params_buffer: wgpu::Buffer,
    hybrid_params_bg: wgpu::BindGroup,
    compute_bgl: wgpu::BindGroupLayout,
    compute_bg: wgpu::BindGroup,
    compute_render_bgl: wgpu::BindGroupLayout,
    compute_render_bg: wgpu::BindGroup,
    particle_buffer: wgpu::Buffer,
    particle_capacity: usize,
    billboard_particle_buffer: wgpu::Buffer,
    billboard_particle_capacity: usize,
    staged: Vec<PointParticleGpu>,
    staged_billboards: Vec<PointParticleGpu>,
    hybrid_emitters: Vec<GpuEmitterParticle>,
    hybrid_emitter_buffer: wgpu::Buffer,
    hybrid_emitter_capacity: usize,
    hybrid_particle_emitter_map: Vec<u32>,
    hybrid_particle_emitter_buffer: wgpu::Buffer,
    hybrid_particle_emitter_capacity: usize,
    hybrid_particle_spawn_origins: Vec<[f32; 4]>,
    hybrid_particle_spawn_origin_buffer: wgpu::Buffer,
    hybrid_particle_spawn_origin_capacity: usize,
    hybrid_particle_spawn_rotations: Vec<[f32; 4]>,
    hybrid_particle_spawn_rotation_buffer: wgpu::Buffer,
    hybrid_particle_spawn_rotation_capacity: usize,
    hybrid_particle_count: u32,
    hybrid_has_point: bool,
    hybrid_has_billboard: bool,
    hybrid_point_ranges: Vec<InstanceRange>,
    hybrid_billboard_ranges: Vec<InstanceRange>,
    compute_emitters: Vec<GpuEmitterParticle>,
    compute_emitter_buffer: wgpu::Buffer,
    compute_emitter_capacity: usize,
    compute_particle_emitter_map: Vec<u32>,
    compute_particle_emitter_buffer: wgpu::Buffer,
    compute_particle_emitter_capacity: usize,
    compute_particle_spawn_origins: Vec<[f32; 4]>,
    compute_particle_spawn_origin_buffer: wgpu::Buffer,
    compute_particle_spawn_origin_capacity: usize,
    compute_particle_spawn_rotations: Vec<[f32; 4]>,
    compute_particle_spawn_rotation_buffer: wgpu::Buffer,
    compute_particle_spawn_rotation_capacity: usize,
    compute_params_buffer: wgpu::Buffer,
    compute_particle_buffer: wgpu::Buffer,
    compute_particle_capacity: usize,
    compute_particle_count: u32,
    compute_has_point: bool,
    compute_has_billboard: bool,
    compute_point_ranges: Vec<InstanceRange>,
    compute_billboard_ranges: Vec<InstanceRange>,
    compute_expr_ops: Vec<GpuExprOp>,
    compute_expr_op_buffer: wgpu::Buffer,
    compute_expr_op_capacity: usize,
    compute_custom_params: Vec<f32>,
    compute_custom_param_buffer: wgpu::Buffer,
    compute_custom_param_capacity: usize,
    compiled_exprs: Vec<Program>,
    compiled_expr_lookup: AHashMap<String, usize>,
    eval_stack: Vec<f32>,
    emitter_order: Vec<usize>,
    hybrid_spawn_rings: AHashMap<NodeID, SpawnRingState>,
    hybrid_spawn_origin_dirty_slots: Vec<u32>,
    hybrid_spawn_rotation_dirty_slots: Vec<u32>,
    compute_spawn_rings: AHashMap<NodeID, SpawnRingState>,
    compute_spawn_origin_dirty_slots: Vec<u32>,
    compute_spawn_rotation_dirty_slots: Vec<u32>,
    spawn_origin_cache: AHashMap<NodeID, AHashMap<u32, SpawnOriginEntry>>,
    spawn_origin_generation: u64,
    hybrid_map_fingerprint: u64,
    hybrid_map_uploaded_fingerprint: u64,
    hybrid_map_uploaded_count: usize,
    compute_map_fingerprint: u64,
    compute_map_uploaded_fingerprint: u64,
    compute_map_uploaded_count: usize,
}
