use crate::resources::ResourceStore;
use glam::{Mat4, Quat, Vec3};
use perro_ids::{MaterialID, MeshID, NodeID};
use perro_render_bridge::{
    AmbientLight3DState, Camera3DState, CameraProjectionState, PointLight3DState, RayLight3DState,
    SpotLight3DState,
};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Draw3DKind {
    Mesh(MeshID),
    Terrain64,
    DebugPointCube,
    DebugEdgeCylinder,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Draw3DInstance {
    pub node: NodeID,
    pub kind: Draw3DKind,
    pub material: Option<MaterialID>,
    pub model: [[f32; 4]; 4],
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Renderer3DStats {
    pub accepted_draws: u32,
    pub rejected_draws: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Lighting3DState {
    pub ambient_light: Option<AmbientLight3DState>,
    pub ray_light: Option<RayLight3DState>,
    pub point_lights: [Option<PointLight3DState>; MAX_POINT_LIGHTS],
    pub spot_lights: [Option<SpotLight3DState>; MAX_SPOT_LIGHTS],
}

pub const MAX_POINT_LIGHTS: usize = 8;
pub const MAX_SPOT_LIGHTS: usize = 8;

pub struct Renderer3D {
    queued_draws: Vec<Draw3DInstance>,
    queued_debug_points: Vec<QueuedDebugPoint3D>,
    queued_debug_lines: Vec<QueuedDebugLine3D>,
    frame_debug_draws: Vec<Draw3DInstance>,
    retained_draws: HashMap<NodeID, Draw3DInstance>,
    ambient_lights: HashMap<NodeID, AmbientLight3DState>,
    ray_lights: HashMap<NodeID, RayLight3DState>,
    point_lights: HashMap<NodeID, PointLight3DState>,
    spot_lights: HashMap<NodeID, SpotLight3DState>,
    camera: Camera3DState,
}

impl Renderer3D {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_camera(&mut self, camera: Camera3DState) {
        self.camera = camera;
    }

    pub fn queue_draw(
        &mut self,
        node: NodeID,
        mesh: MeshID,
        material: MaterialID,
        model: [[f32; 4]; 4],
    ) {
        self.queued_draws.push(Draw3DInstance {
            node,
            kind: Draw3DKind::Mesh(mesh),
            material: Some(material),
            model,
        });
    }

    pub fn queue_terrain(&mut self, node: NodeID, model: [[f32; 4]; 4]) {
        self.queued_draws.push(Draw3DInstance {
            node,
            kind: Draw3DKind::Terrain64,
            material: None,
            model,
        });
    }

    pub fn queue_debug_point(&mut self, node: NodeID, position: [f32; 3], size: f32) {
        self.queued_debug_points.push(QueuedDebugPoint3D {
            node,
            position,
            size,
        });
    }

    pub fn queue_debug_line(
        &mut self,
        node: NodeID,
        start: [f32; 3],
        end: [f32; 3],
        thickness: f32,
    ) {
        self.queued_debug_lines.push(QueuedDebugLine3D {
            node,
            start,
            end,
            thickness,
        });
    }

    pub fn remove_node(&mut self, node: NodeID) {
        self.retained_draws.remove(&node);
        self.ambient_lights.remove(&node);
        self.ray_lights.remove(&node);
        self.point_lights.remove(&node);
        self.spot_lights.remove(&node);
    }

    pub fn set_ambient_light(&mut self, node: NodeID, light: AmbientLight3DState) {
        self.ambient_lights.insert(node, light);
    }

    pub fn set_ray_light(&mut self, node: NodeID, light: RayLight3DState) {
        self.ray_lights.insert(node, light);
    }

    pub fn set_point_light(&mut self, node: NodeID, light: PointLight3DState) {
        self.point_lights.insert(node, light);
    }

    pub fn set_spot_light(&mut self, node: NodeID, light: SpotLight3DState) {
        self.spot_lights.insert(node, light);
    }

    pub fn prepare_frame(
        &mut self,
        resources: &ResourceStore,
    ) -> (Camera3DState, Renderer3DStats, Lighting3DState) {
        let mut stats = Renderer3DStats::default();
        self.frame_debug_draws.clear();

        for draw in self.queued_draws.drain(..) {
            let material_ready = draw.material.map(|id| resources.has_material(id)).unwrap_or(true);
            let mesh_ready = match draw.kind {
                Draw3DKind::Mesh(mesh) => resources.has_mesh(mesh),
                Draw3DKind::Terrain64 | Draw3DKind::DebugPointCube | Draw3DKind::DebugEdgeCylinder => true,
            };
            let draw_ready = match draw.kind {
                Draw3DKind::Mesh(_) => mesh_ready && material_ready,
                Draw3DKind::Terrain64 | Draw3DKind::DebugPointCube | Draw3DKind::DebugEdgeCylinder => material_ready,
            };
            if draw_ready {
                self.retained_draws.insert(draw.node, draw);
                stats.accepted_draws = stats.accepted_draws.saturating_add(1);
            } else {
                if let Some(retained) = self.retained_draws.get_mut(&draw.node) {
                    // Keep previous mesh/material bindings until replacements exist,
                    // but continue applying latest transform updates.
                    retained.model = draw.model;
                    if mesh_ready {
                        retained.kind = draw.kind;
                    }
                    if material_ready {
                        retained.material = draw.material;
                    }
                }
                stats.rejected_draws = stats.rejected_draws.saturating_add(1);
            }
        }

        for point in self.queued_debug_points.drain(..) {
            self.frame_debug_draws.push(Draw3DInstance {
                node: point.node,
                kind: Draw3DKind::DebugPointCube,
                material: None,
                model: debug_point_model(point.position, point.size).to_cols_array_2d(),
            });
        }
        for line in self.queued_debug_lines.drain(..) {
            if let Some(model) = debug_line_model(line.start, line.end, line.thickness) {
                self.frame_debug_draws.push(Draw3DInstance {
                    node: line.node,
                    kind: Draw3DKind::DebugEdgeCylinder,
                    material: None,
                    model: model.to_cols_array_2d(),
                });
            }
        }

        let mut lighting = Lighting3DState::default();
        if let Some((_, ambient)) = self.ambient_lights.iter().next() {
            lighting.ambient_light = Some(*ambient);
        }
        if let Some((_, ray)) = self.ray_lights.iter().next() {
            lighting.ray_light = Some(*ray);
        }
        for (slot, (_, light)) in lighting
            .point_lights
            .iter_mut()
            .zip(self.point_lights.iter())
        {
            *slot = Some(*light);
        }
        for (slot, (_, light)) in lighting.spot_lights.iter_mut().zip(self.spot_lights.iter()) {
            *slot = Some(*light);
        }

        (self.camera, stats, lighting)
    }

    pub fn retained_draw(&self, node: NodeID) -> Option<Draw3DInstance> {
        self.retained_draws.get(&node).copied()
    }

    pub fn retained_draw_count(&self) -> usize {
        self.retained_draws.len()
    }

    pub fn retained_draws(&self) -> impl Iterator<Item = Draw3DInstance> + '_ {
        self.retained_draws.values().copied()
    }

    pub fn all_draws(&self) -> impl Iterator<Item = Draw3DInstance> + '_ {
        self.retained_draws
            .values()
            .copied()
            .chain(self.frame_debug_draws.iter().copied())
    }

    pub fn camera(&self) -> Camera3DState {
        self.camera
    }
}

impl Default for Renderer3D {
    fn default() -> Self {
        Self {
            queued_draws: Vec::new(),
            queued_debug_points: Vec::new(),
            queued_debug_lines: Vec::new(),
            frame_debug_draws: Vec::new(),
            retained_draws: HashMap::new(),
            ambient_lights: HashMap::new(),
            ray_lights: HashMap::new(),
            point_lights: HashMap::new(),
            spot_lights: HashMap::new(),
            // Keep a usable fallback view if no Camera3D node is active.
            camera: Camera3DState {
                position: [0.0, 0.0, 6.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                projection: CameraProjectionState::Perspective {
                    fov_y_degrees: 60.0,
                    near: 0.1,
                    far: 1000.0,
                },
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct QueuedDebugPoint3D {
    node: NodeID,
    position: [f32; 3],
    size: f32,
}

#[derive(Debug, Clone, Copy)]
struct QueuedDebugLine3D {
    node: NodeID,
    start: [f32; 3],
    end: [f32; 3],
    thickness: f32,
}

fn debug_point_model(position: [f32; 3], size: f32) -> Mat4 {
    let scale = Vec3::splat(size.max(0.001));
    Mat4::from_scale_rotation_translation(scale, Quat::IDENTITY, Vec3::from_array(position))
}

fn debug_line_model(start: [f32; 3], end: [f32; 3], thickness: f32) -> Option<Mat4> {
    let a = Vec3::from_array(start);
    let b = Vec3::from_array(end);
    let delta = b - a;
    let len = delta.length();
    if !len.is_finite() || len <= 1.0e-5 {
        return None;
    }
    let dir = delta / len;
    let up = Vec3::Y;
    let rot = Quat::from_rotation_arc(up, dir);
    let center = (a + b) * 0.5;
    let radius = thickness.max(0.001);
    let scale = Vec3::new(radius, len, radius);
    Some(Mat4::from_scale_rotation_translation(scale, rot, center))
}
