//! 3D scene extraction into render bridge commands.

use super::Runtime;
use glam::{Mat4, Quat, Vec3};
use perro_ids::{
    MaterialID, MeshID, NodeID, ParticleProfileRef, parse_hashed_source_uri, string_to_u64,
};
use perro_nodes::{
    CameraProjection, MeshSurfaceBinding, SceneNodeData, Shape3D,
    particle_emitter_3d::{ParticleEmitterSimMode3D, ParticleType},
    water_impact_strength,
};
use perro_particle_math::compile_expression;
use perro_render_bridge::{
    AmbientLight3DState, Camera3DState, CameraProjectionState, CameraStream3DState,
    CameraStreamCommand, Command3D, Decal3DState, DenseInstancePose3D, EnvironmentMap3DState,
    LODOptions3D, Material3D, MaterialParamOverride3D, MeshBlendOptions3D, MeshSurfaceBinding3D,
    ParticlePath3D, ParticleProfile3D, ParticleRenderMode3D, ParticleSimulationMode3D,
    PointLight3DState, PointParticles3DState, RayLight3DState, RenderCommand, ResourceCommand,
    SkeletonPalette, Sky3DState, SkyShaderPass3DState, SkyTime3DState, SpotLight3DState, UiCommand,
    UiImageScaleState, UiRectState, UiTextAlignState, Water3DState, WaterBodyQueryState,
    WaterCoastlineShape3D, WaterIdleModeState, WaterImpact3D, WaterLinkState, WaterShapeState,
};
use perro_resource_api::sub_apis::{MaterialAPI, MeshAPI};
use perro_runtime_render::{material_3d_request, mesh_3d_request};
use perro_structs::{BitMask, Vector2, Vector3};
use std::borrow::Cow;
use std::sync::Arc;

const PARTICLE_PATH_CACHE_MAX: usize = 256;

type Camera3DPick = (
    (u64, u32, u32),
    NodeID,
    perro_structs::Transform3D,
    CameraProjection,
    BitMask,
    perro_structs::PostProcessSet,
    perro_structs::AudioListenerOptions,
);

#[inline]
fn mirror_matrix_3d(flip_x: bool, flip_y: bool, flip_z: bool) -> Mat4 {
    Mat4::from_scale(Vec3::new(
        if flip_x { -1.0 } else { 1.0 },
        if flip_y { -1.0 } else { 1.0 },
        if flip_z { -1.0 } else { 1.0 },
    ))
}

#[path = "three_d/helpers.rs"]
mod helpers;
use helpers::*;
pub(crate) use helpers::{
    build_skeleton_palette, derived_particle_budget as derived_particle_budget_3d,
    resolve_particle_profile, resolve_particle_render_mode, resolve_particle_sim_mode,
};

#[path = "three_d/assets.rs"]
mod assets;
#[path = "three_d/extract.rs"]
mod extract;
#[path = "three_d/water.rs"]
mod water;

fn signed_collision_shape_scale(
    scale: perro_structs::Vector3,
    flip: (bool, bool, bool),
) -> perro_structs::Vector3 {
    perro_structs::Vector3::new(
        if flip.0 {
            -scale.x.abs()
        } else {
            scale.x.abs()
        },
        if flip.1 {
            -scale.y.abs()
        } else {
            scale.y.abs()
        },
        if flip.2 {
            -scale.z.abs()
        } else {
            scale.z.abs()
        },
    )
}

fn camera_projection_state(projection: &CameraProjection) -> CameraProjectionState {
    match projection {
        CameraProjection::Perspective {
            fov_y_degrees,
            near,
            far,
        } => CameraProjectionState::Perspective {
            fov_y_degrees: *fov_y_degrees,
            near: *near,
            far: *far,
        },
        CameraProjection::Orthographic { size, near, far } => CameraProjectionState::Orthographic {
            size: *size,
            near: *near,
            far: *far,
        },
        CameraProjection::Frustum {
            left,
            right,
            bottom,
            top,
            near,
            far,
        } => CameraProjectionState::Frustum {
            left: *left,
            right: *right,
            bottom: *bottom,
            top: *top,
            near: *near,
            far: *far,
        },
    }
}

fn fallback_camera_3d_state() -> Camera3DState {
    Camera3DState {
        position: [0.0, 0.0, 6.0],
        rotation: [0.0, 0.0, 0.0, 1.0],
        projection: CameraProjectionState::Perspective {
            fov_y_degrees: 60.0,
            near: 0.1,
            far: 1000.0,
        },
        render_mask: BitMask::NONE,
        post_processing: Arc::from([]),
        audio_options: perro_structs::AudioListenerOptions::new(),
    }
}

fn viewport_clip_3d(viewport: Vector2) -> [f32; 4] {
    [0.0, 0.0, viewport.x.max(1.0), viewport.y.max(1.0)]
}

fn text_align_state_3d(align: perro_ui::UiTextAlign) -> UiTextAlignState {
    match align {
        perro_ui::UiTextAlign::Start => UiTextAlignState::Start,
        perro_ui::UiTextAlign::Center => UiTextAlignState::Center,
        perro_ui::UiTextAlign::End => UiTextAlignState::End,
    }
}

fn label_3d_content_size(rect_size: [f32; 2], padding: perro_ui::UiRect) -> [f32; 2] {
    [
        (rect_size[0] * (1.0 - padding.left.max(0.0) - padding.right.max(0.0))).max(0.001),
        (rect_size[1] * (1.0 - padding.top.max(0.0) - padding.bottom.max(0.0))).max(0.001),
    ]
}

// Canonical layout space for every Label3D: center pinned at the origin and
// size derived only from (label size aspect, font_size). The painter projects
// the tessellation onto the world quad by normalizing against this same rect,
// so the center value never affects output — pinning it (instead of passing
// the projected screen center) keeps the draw byte-stable across camera and
// label motion, which is what lets the painter's per-node cache reuse the
// tessellation every frame instead of re-shaping + re-tessellating.
fn label_3d_canonical_layout_rect(size: Vector2, font_size: f32) -> UiRectState {
    let height = font_size.max(1.0);
    let aspect = (size.x.abs() / size.y.abs().max(0.001)).max(0.001);
    UiRectState {
        center: [0.0, 0.0],
        size: [(height * aspect).max(1.0), height],
        pivot: [0.5, 0.5],
        rotation_radians: 0.0,
        z_index: 0,
    }
}

// Billboard orientation for a non-locked Label3D: camera rotation, so the
// quad is parallel to the image plane (all four corners share one view depth
// -> projects to an exact screen-aligned rectangle, like the old rect path).
fn label_billboard_transform_3d(
    mut transform: perro_structs::Transform3D,
    camera: &Camera3DState,
) -> perro_structs::Transform3D {
    transform.rotation = perro_structs::Quaternion::new(
        camera.rotation[0],
        camera.rotation[1],
        camera.rotation[2],
        camera.rotation[3],
    );
    transform
}

fn sprite_3d_uv(
    texture_region: Option<[f32; 4]>,
    flip_x: bool,
    flip_y: bool,
) -> ([f32; 2], [f32; 2]) {
    let (mut min, mut max) = if let Some([x, y, w, h]) = texture_region {
        ([x, y], [x + w, y + h])
    } else {
        ([0.0, 0.0], [1.0, 1.0])
    };
    if flip_x {
        std::mem::swap(&mut min[0], &mut max[0]);
    }
    if flip_y {
        std::mem::swap(&mut min[1], &mut max[1]);
    }
    (min, max)
}

fn world_rect_3d(
    transform: perro_structs::Transform3D,
    size: Vector2,
    camera: &Camera3DState,
    viewport: Vector2,
) -> Option<UiRectState> {
    let view_proj = camera_view_proj_3d(camera, viewport);
    let center = Vec3::new(
        transform.position.x,
        transform.position.y,
        transform.position.z,
    );
    let rotation = Quat::from_xyzw(
        transform.rotation.x,
        transform.rotation.y,
        transform.rotation.z,
        transform.rotation.w,
    );
    let rotation = if rotation.is_finite() && rotation.length_squared() > 1.0e-6 {
        rotation.normalize()
    } else {
        Quat::IDENTITY
    };
    let right = rotation * Vec3::X * (size.x * transform.scale.x.abs() * 0.5);
    let up = rotation * Vec3::Y * (size.y * transform.scale.y.abs() * 0.5);
    let center_screen = project_world_to_ui(center, view_proj, viewport)?;
    // Use projection derivatives at the center. Projecting full corners makes
    // labels vanish when a large or steeply rotated corner crosses the near
    // plane even though the label center remains visible.
    let width = projected_axis_size_3d(center, right, view_proj, viewport)?.max(0.001);
    let height = projected_axis_size_3d(center, up, view_proj, viewport)?.max(0.001);
    Some(UiRectState {
        center: center_screen,
        size: [width, height],
        pivot: [0.5, 0.5],
        rotation_radians: 0.0,
        z_index: 0,
    })
}

fn label_projected_quad_3d(
    transform: perro_structs::Transform3D,
    size: Vector2,
    camera: &Camera3DState,
    viewport: Vector2,
) -> Option<[[f32; 4]; 4]> {
    let rotation = Quat::from_xyzw(
        transform.rotation.x,
        transform.rotation.y,
        transform.rotation.z,
        transform.rotation.w,
    );
    let rotation = if rotation.is_finite() && rotation.length_squared() > 1.0e-6 {
        rotation.normalize()
    } else {
        Quat::IDENTITY
    };
    let center = Vec3::from(transform.position);
    let right = rotation * Vec3::X * (size.x * transform.scale.x.abs() * 0.5);
    let up = rotation * Vec3::Y * (size.y * transform.scale.y.abs() * 0.5);
    let view_proj = camera_view_proj_3d(camera, viewport);
    let project = |point: Vec3| {
        let clip = view_proj * point.extend(1.0);
        clip.is_finite().then_some([clip.x, clip.y, clip.z, clip.w])
    };
    Some([
        project(center - right + up)?,
        project(center + right + up)?,
        project(center + right - up)?,
        project(center - right - up)?,
    ])
}

fn projected_axis_size_3d(
    center: Vec3,
    half_axis: Vec3,
    view_proj: Mat4,
    viewport: Vector2,
) -> Option<f32> {
    let clip = view_proj * center.extend(1.0);
    let delta = view_proj * half_axis.extend(0.0);
    if !clip.is_finite() || !delta.is_finite() || clip.w <= 1.0e-6 {
        return None;
    }
    let inv_w_sq = 1.0 / (clip.w * clip.w);
    let dx = (delta.x * clip.w - clip.x * delta.w) * inv_w_sq * viewport.x.max(1.0);
    let dy = (delta.y * clip.w - clip.y * delta.w) * inv_w_sq * viewport.y.max(1.0);
    let size = dx.hypot(dy);
    size.is_finite().then_some(size)
}

fn world_rect_front_facing_3d(
    transform: perro_structs::Transform3D,
    camera: &Camera3DState,
) -> bool {
    let rotation = Quat::from_xyzw(
        transform.rotation.x,
        transform.rotation.y,
        transform.rotation.z,
        transform.rotation.w,
    );
    let rotation = if rotation.is_finite() && rotation.length_squared() > 1.0e-6 {
        rotation.normalize()
    } else {
        Quat::IDENTITY
    };
    let normal = rotation * Vec3::Z;
    let to_camera = Vec3::from(camera.position) - Vec3::from(transform.position);
    normal.dot(to_camera) > 0.0
}

fn project_world_to_ui(world: Vec3, view_proj: Mat4, viewport: Vector2) -> Option<[f32; 2]> {
    let clip = view_proj * world.extend(1.0);
    if !clip.is_finite() || clip.w <= 1.0e-6 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    // wgpu clip convention: depth 0..1. A point between the camera and the
    // near plane lands at ndc.z < 0 with a tiny positive w — without this
    // bound it passes and its projected rect explodes across the screen.
    if ndc.z < 0.0 || ndc.z > 1.0 {
        return None;
    }
    Some([
        ndc.x * viewport.x.max(1.0) * 0.5,
        ndc.y * viewport.y.max(1.0) * 0.5,
    ])
}

fn camera_view_proj_3d(camera: &Camera3DState, viewport: Vector2) -> Mat4 {
    let aspect = viewport.x.max(1.0) / viewport.y.max(1.0);
    let proj = projection_matrix_3d(camera.projection, aspect);
    let pos = Vec3::from(camera.position);
    let rot = Quat::from_xyzw(
        camera.rotation[0],
        camera.rotation[1],
        camera.rotation[2],
        camera.rotation[3],
    );
    let rot = if rot.is_finite() && rot.length_squared() > 1.0e-6 {
        rot.normalize()
    } else {
        Quat::IDENTITY
    };
    proj * Mat4::from_rotation_translation(rot, pos).inverse()
}

fn projection_matrix_3d(projection: CameraProjectionState, aspect: f32) -> Mat4 {
    match projection {
        CameraProjectionState::Perspective {
            fov_y_degrees,
            near,
            far,
        } => Mat4::perspective_rh(
            perspective_fov_y_radians_3d(fov_y_degrees),
            aspect.max(1.0e-6),
            sanitize_near_3d(near),
            sanitize_far_3d(far, sanitize_near_3d(near)),
        ),
        CameraProjectionState::Orthographic { size, near, far } => {
            let half_h = if size.is_finite() {
                (size.abs() * 0.5).max(1.0e-3)
            } else {
                5.0
            };
            let half_w = half_h * aspect.max(1.0e-6);
            let near = sanitize_near_3d(near);
            let far = sanitize_far_3d(far, near);
            Mat4::orthographic_rh(-half_w, half_w, -half_h, half_h, near, far)
        }
        CameraProjectionState::Frustum {
            left,
            right,
            bottom,
            top,
            near,
            far,
        } => {
            let near = sanitize_near_3d(near);
            let far = sanitize_far_3d(far, near);
            let (left, right) = sanitize_range_3d(left, right, -1.0, 1.0);
            let (bottom, top) = sanitize_range_3d(bottom, top, -1.0, 1.0);
            Mat4::frustum_rh(left, right, bottom, top, near, far)
        }
    }
}

fn perspective_fov_y_radians_3d(fov_y_degrees: f32) -> f32 {
    if fov_y_degrees.is_finite() {
        fov_y_degrees
            .to_radians()
            .clamp(10.0f32.to_radians(), 120.0f32.to_radians())
    } else {
        60.0f32.to_radians()
    }
}

fn sanitize_near_3d(near: f32) -> f32 {
    if near.is_finite() {
        near.max(1.0e-3)
    } else {
        0.1
    }
}

fn sanitize_far_3d(far: f32, near: f32) -> f32 {
    if far.is_finite() {
        far.max(near + 1.0e-3)
    } else {
        (near + 1000.0).max(near + 1.0e-3)
    }
}

fn sanitize_range_3d(min: f32, max: f32, fallback_min: f32, fallback_max: f32) -> (f32, f32) {
    let mut a = if min.is_finite() { min } else { fallback_min };
    let mut b = if max.is_finite() { max } else { fallback_max };
    if (b - a).abs() < 1.0e-6 {
        a = fallback_min;
        b = fallback_max;
    }
    if b < a {
        std::mem::swap(&mut a, &mut b);
    }
    (a, b)
}

#[inline]
fn render_mask_matches(camera_mask: BitMask, render_layers: BitMask) -> bool {
    !camera_mask.intersects(render_layers)
}

pub(crate) fn water_idle_mode_state(mode: perro_nodes::WaterIdleMode) -> WaterIdleModeState {
    match mode {
        perro_nodes::WaterIdleMode::Calm => WaterIdleModeState::Calm,
        perro_nodes::WaterIdleMode::Sine => WaterIdleModeState::Sine,
        perro_nodes::WaterIdleMode::Chop => WaterIdleModeState::Chop,
        perro_nodes::WaterIdleMode::Storm => WaterIdleModeState::Storm,
        perro_nodes::WaterIdleMode::River => WaterIdleModeState::River,
    }
}

pub(crate) fn water_shape_state(shape: perro_nodes::WaterShape) -> WaterShapeState {
    match shape {
        perro_nodes::WaterShape::Circle { radius } => WaterShapeState::Circle { radius },
        perro_nodes::WaterShape::Cylinder {
            radius,
            half_height,
        } => WaterShapeState::Cylinder {
            radius,
            half_height,
        },
        _ => WaterShapeState::Rect,
    }
}

pub(crate) fn water_render_size(water: perro_nodes::WaterSurfaceParams) -> [f32; 2] {
    let size = water.shape.surface_size();
    [size.x, size.y]
}

fn simple_surfaces_match(
    surfaces: &[MeshSurfaceBinding],
    retained: &[MeshSurfaceBinding3D],
) -> bool {
    surfaces.len() == retained.len()
        && surfaces
            .iter()
            .zip(retained.iter())
            .all(|(surface, retained)| {
                surface.material == retained.material
                    && surface.modulate == retained.modulate
                    && surface.overrides.is_empty()
                    && retained.overrides.is_empty()
            })
}

fn water_local_point_3d(
    inv_transform: glam::Mat4,
    point: perro_structs::Vector3,
) -> perro_structs::Vector3 {
    inv_transform.transform_point3(point.into()).into()
}

fn water_global_point_3d(
    transform: perro_structs::Transform3D,
    point: perro_structs::Vector3,
) -> perro_structs::Vector3 {
    transform.to_mat4().transform_point3(point.into()).into()
}

fn water_local_axis_xz(
    water_transform: perro_structs::Transform3D,
    shape_transform: perro_structs::Transform3D,
    axis: perro_structs::Vector3,
) -> [f32; 2] {
    let world_axis = shape_transform.rotation.rotate_vector3(axis);
    let local_axis = water_transform
        .rotation
        .inverse()
        .rotate_vector3(world_axis);
    let len = (local_axis.x * local_axis.x + local_axis.z * local_axis.z)
        .sqrt()
        .max(0.0001);
    [local_axis.x / len, local_axis.z / len]
}

fn water_surface_corners(size: perro_structs::Vector2) -> [perro_structs::Vector3; 4] {
    let half = size * 0.5;
    [
        perro_structs::Vector3::new(-half.x, 0.0, -half.y),
        perro_structs::Vector3::new(half.x, 0.0, -half.y),
        perro_structs::Vector3::new(-half.x, 0.0, half.y),
        perro_structs::Vector3::new(half.x, 0.0, half.y),
    ]
}

fn water_overlap_bounds_3d(
    water: &perro_nodes::WaterSurfaceParams,
    water_transform: perro_structs::Transform3D,
    other: perro_nodes::WaterSurfaceParams,
    other_transform: perro_structs::Transform3D,
) -> Option<(perro_structs::Vector2, perro_structs::Vector2)> {
    let water_inv = water_transform.to_mat4().inverse();
    let other_inv = other_transform.to_mat4().inverse();
    let mut points = Vec::new();
    for corner in water_surface_corners(other.shape.surface_size()) {
        let world = water_global_point_3d(other_transform, corner);
        let local = water_local_point_3d(water_inv, world);
        let surface = perro_structs::Vector2::new(local.x, local.z);
        if water.shape.contains_surface(surface) {
            points.push(surface);
        }
    }
    for corner in water_surface_corners(water.shape.surface_size()) {
        let world = water_global_point_3d(water_transform, corner);
        let other_local = water_local_point_3d(other_inv, world);
        if other
            .shape
            .contains_surface(perro_structs::Vector2::new(other_local.x, other_local.z))
        {
            points.push(perro_structs::Vector2::new(corner.x, corner.z));
        }
    }
    let other_center = water_local_point_3d(water_inv, other_transform.position);
    let other_center_surface = perro_structs::Vector2::new(other_center.x, other_center.z);
    if water.shape.contains_surface(other_center_surface) {
        points.push(other_center_surface);
    }
    let water_center_in_other = water_local_point_3d(other_inv, water_transform.position);
    if other.shape.contains_surface(perro_structs::Vector2::new(
        water_center_in_other.x,
        water_center_in_other.z,
    )) {
        points.push(perro_structs::Vector2::ZERO);
    }
    if points.is_empty() {
        return None;
    }
    let mut min = points[0];
    let mut max = points[0];
    for point in points.into_iter().skip(1) {
        min.x = min.x.min(point.x);
        min.y = min.y.min(point.y);
        max.x = max.x.max(point.x);
        max.y = max.y.max(point.y);
    }
    (min.x < max.x && min.y < max.y).then_some((min, max))
}

fn water_link_overlap_weight(local: perro_structs::Vector2, link: &WaterLinkState) -> f32 {
    let cx = ((link.overlap_min[0] + link.overlap_max[0]) * 0.5 - local.x).abs();
    let cy = ((link.overlap_min[1] + link.overlap_max[1]) * 0.5 - local.y).abs();
    let hx = (link.overlap_max[0] - link.overlap_min[0]).abs() * 0.5 + link.blend_width;
    let hy = (link.overlap_max[1] - link.overlap_min[1]).abs() * 0.5 + link.blend_width;
    let edge = (1.0 - (cx / hx.max(0.001))).min(1.0 - (cy / hy.max(0.001)));
    let t = edge.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Compares a retained `Sky3DState` against a live `Sky3D` node without
/// allocating a new `Sky3DState` first. Mirrors the derived `PartialEq` on
/// `Sky3DState` field-for-field, so callers can skip the SetSky command and
/// its Arc allocations when nothing actually changed.
fn sky_3d_state_matches(retained: &Sky3DState, sky: &perro_nodes::Sky3D) -> bool {
    retained.day_colors[..] == sky.palette.day_colors[..]
        && retained.evening_colors[..] == sky.palette.evening_colors[..]
        && retained.night_colors[..] == sky.palette.night_colors[..]
        && retained.horizon_colors[..] == sky.palette.horizon_colors[..]
        && retained.time.time_of_day == sky.time.time_of_day
        && retained.time.paused == sky.time.paused
        && retained.time.scale == sky.time.scale
        && retained.shaders.len() == sky.shaders.len()
        && retained
            .shaders
            .iter()
            .zip(sky.shaders.iter())
            .all(|(retained_shader, shader)| {
                retained_shader.path == shader.path
                    && retained_shader.params[..] == shader.params[..]
            })
        && match (&retained.environment, &sky.environment) {
            (Some(retained), Some(environment)) => {
                retained.source == environment.source
                    && retained.intensity == environment.intensity
                    && retained.rotation_degrees == environment.rotation_degrees
            }
            (None, None) => true,
            _ => false,
        }
}

#[cfg(test)]
#[path = "../../../tests/unit/runtime_render_3d_tests.rs"]
mod tests;
