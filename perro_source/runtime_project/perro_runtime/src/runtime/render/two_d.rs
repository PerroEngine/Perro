//! 2D scene extraction into render bridge commands.

use super::Runtime;
use super::state::UiButtonVisualState;
use ahash::AHashSet;
use perro_ids::{NodeID, ParticleProfileRef, SignalID, TextureID, TileSetRef};
use perro_input_api::MouseButton;
use perro_nodes::{
    SceneNodeData, Shape2D, particle_emitter_2d::ParticleEmitterSimMode2D, water_impact_strength,
};
use perro_particle_math::compile_expression;
use perro_physics::{ShapeKind2D, tilemap_shape_descs_2d, triangle_points_2d};
use perro_render_bridge::{
    AmbientLight2DState, Camera2DState, CameraStreamCommand, CameraStreamSourceState, Command2D,
    ParticlePath2D, ParticleProfile2D, ParticleSimulationMode2D, PointLight2DState,
    PointParticles2DState, RayLight2DState, Rect2DCommand, RenderCommand, ResourceCommand,
    ShadowCaster2DShapeState, ShadowCaster2DState, SpotLight2DState, Sprite2DCommand,
    TileMap2DCommand, UiCommand, UiRectState, UiTextAlignState, Water2DState, WaterBodyQueryState,
    WaterCoastlineShape2D, WaterIdleModeState, WaterImpact2D, WaterLinkState, WaterShapeState,
};
use perro_runtime_render::{sprite_2d_texture_request, tilemap_2d_texture_request};
use perro_structs::{BitMask, UVector2, Vector2};
use perro_variant::Variant;
use std::borrow::Cow;
use std::sync::Arc;

const PARTICLE_PATH_CACHE_MAX: usize = 256;

struct Sprite2DEmit {
    texture: TextureID,
    texture_region: Option<[f32; 4]>,
    flip_x: bool,
    flip_y: bool,
    model: [[f32; 3]; 3],
    tint: perro_structs::Color,
    size_override: Option<[f32; 2]>,
    z_index: i32,
}

pub(crate) use perro_render_bridge::TileSet2D as ParsedTileset2D;
#[cfg(test)]
pub(crate) use perro_render_bridge::TileSetTile2D as ParsedTile2D;
#[cfg(test)]
pub(crate) use perro_render_bridge::{
    TileSetCollisionShape2D as ParsedTileCollisionShape2D, TileSetShape2D,
};

#[path = "two_d/assets.rs"]
mod assets;
#[path = "two_d/extract.rs"]
mod extract;
#[path = "two_d/interaction.rs"]
mod interaction;
#[path = "two_d/water.rs"]
mod water;

fn viewport_clip(viewport: Vector2) -> [f32; 4] {
    [0.0, 0.0, viewport.x.max(1.0), viewport.y.max(1.0)]
}

fn text_align_state_2d(align: perro_ui::UiTextAlign) -> UiTextAlignState {
    match align {
        perro_ui::UiTextAlign::Start => UiTextAlignState::Start,
        perro_ui::UiTextAlign::Center => UiTextAlignState::Center,
        perro_ui::UiTextAlign::End => UiTextAlignState::End,
    }
}

fn label_2d_rect(
    transform: perro_structs::Transform2D,
    size: Vector2,
    camera: Option<&Camera2DState>,
    viewport: Vector2,
    z_index: i32,
) -> UiRectState {
    let mut center = transform.position;
    let mut rotation = transform.rotation;
    let mut zoom = 1.0;
    if let Some(camera) = camera {
        let x = center.x - camera.position[0];
        let y = center.y - camera.position[1];
        let sin = (-camera.rotation_radians).sin();
        let cos = (-camera.rotation_radians).cos();
        center = Vector2::new(x * cos - y * sin, x * sin + y * cos);
        rotation -= camera.rotation_radians;
        zoom = camera.zoom.max(0.0001);
    }
    let scale = transform.scale;
    UiRectState {
        center: [center.x * zoom, center.y * zoom],
        size: [
            (size.x * scale.x.abs() * zoom)
                .max(0.001)
                .min(viewport.x.max(1.0)),
            (size.y * scale.y.abs() * zoom)
                .max(0.001)
                .min(viewport.y.max(1.0)),
        ],
        pivot: [0.5, 0.5],
        rotation_radians: rotation,
        z_index,
    }
}

fn button_2d_style(
    button: &perro_nodes::Button2D,
    state: UiButtonVisualState,
) -> &perro_ui::UiStyle {
    if !button.input_enabled {
        return &button.style;
    }
    match state {
        UiButtonVisualState::Neutral => &button.style,
        UiButtonVisualState::Hover => &button.hover_style,
        UiButtonVisualState::Pressed => &button.pressed_style,
    }
}

fn image_button_2d_tint(
    button: &perro_nodes::ImageButton2D,
    state: UiButtonVisualState,
) -> perro_structs::Color {
    if !button.input_enabled {
        return button.tint;
    }
    match state {
        UiButtonVisualState::Neutral => button.tint,
        UiButtonVisualState::Hover => button.hover_tint,
        UiButtonVisualState::Pressed => button.pressed_tint,
    }
}

fn nine_slice_button_2d_tint(
    button: &perro_nodes::NineSliceButton2D,
    state: UiButtonVisualState,
) -> perro_structs::Color {
    if !button.input_enabled {
        return button.tint;
    }
    match state {
        UiButtonVisualState::Neutral => button.tint,
        UiButtonVisualState::Hover => button.hover_tint,
        UiButtonVisualState::Pressed => button.pressed_tint,
    }
}

fn button_2d_inactive_from_data(data: &SceneNodeData) -> Option<bool> {
    match data {
        SceneNodeData::Button2D(button) => Some(!button.input_enabled),
        SceneNodeData::ImageButton2D(button) => Some(!button.input_enabled),
        SceneNodeData::NineSliceButton2D(button) => Some(!button.input_enabled),
        _ => None,
    }
}

fn button_2d_cursor_icon(data: &SceneNodeData) -> Option<perro_ui::CursorIcon> {
    match data {
        SceneNodeData::Button2D(button) => Some(button.cursor_icon),
        SceneNodeData::ImageButton2D(button) => Some(button.cursor_icon),
        SceneNodeData::NineSliceButton2D(button) => Some(button.cursor_icon),
        _ => None,
    }
}

struct Button2DHitData<'a> {
    visible: bool,
    size: Vector2,
    z_index: i32,
    render_layers: BitMask,
    input_enabled: bool,
    mouse_filter: perro_ui::UiMouseFilter,
    input_mask: &'a perro_ui::UiInputMask,
}

fn button_2d_hit_data(data: &SceneNodeData) -> Option<Button2DHitData<'_>> {
    match data {
        SceneNodeData::Button2D(button) => Some(Button2DHitData {
            visible: button.visible,
            size: button.size,
            z_index: button.z_index,
            render_layers: button.render_layers,
            input_enabled: button.input_enabled,
            mouse_filter: button.mouse_filter,
            input_mask: &button.input_mask,
        }),
        SceneNodeData::ImageButton2D(button) => Some(Button2DHitData {
            visible: button.visible,
            size: button.size,
            z_index: button.z_index,
            render_layers: button.render_layers,
            input_enabled: button.input_enabled,
            mouse_filter: button.mouse_filter,
            input_mask: &button.input_mask,
        }),
        SceneNodeData::NineSliceButton2D(button) => Some(Button2DHitData {
            visible: button.visible,
            size: button.size,
            z_index: button.z_index,
            render_layers: button.render_layers,
            input_enabled: button.input_enabled,
            mouse_filter: button.mouse_filter,
            input_mask: &button.input_mask,
        }),
        _ => None,
    }
}

fn button_2d_custom_event_signals<'a>(
    data: &'a SceneNodeData,
    event: &str,
) -> Option<&'a [SignalID]> {
    match data {
        SceneNodeData::Button2D(button) => Some(match event {
            "hover_enter" => &button.hover_signals,
            "hover_exit" => &button.hover_exit_signals,
            "pressed" => &button.pressed_signals,
            "released" => &button.released_signals,
            "click" => &button.clicked_signals,
            _ => &[],
        }),
        SceneNodeData::ImageButton2D(button) => Some(match event {
            "hover_enter" => &button.hover_signals,
            "hover_exit" => &button.hover_exit_signals,
            "pressed" => &button.pressed_signals,
            "released" => &button.released_signals,
            "click" => &button.clicked_signals,
            _ => &[],
        }),
        SceneNodeData::NineSliceButton2D(button) => Some(match event {
            "hover_enter" => &button.hover_signals,
            "hover_exit" => &button.hover_exit_signals,
            "pressed" => &button.pressed_signals,
            "released" => &button.released_signals,
            "click" => &button.clicked_signals,
            _ => &[],
        }),
        _ => None,
    }
}

fn button_2d_named_event(event: &str) -> &str {
    match event {
        "click" => "clicked",
        other => other,
    }
}

fn collect_button_2d_events(
    node: NodeID,
    prev: UiButtonVisualState,
    next: UiButtonVisualState,
    out: &mut Vec<(NodeID, &'static str)>,
) {
    if prev == next {
        return;
    }
    if prev == UiButtonVisualState::Neutral && next != UiButtonVisualState::Neutral {
        out.push((node, "hover_enter"));
    }
    if prev != UiButtonVisualState::Neutral && next == UiButtonVisualState::Neutral {
        out.push((node, "hover_exit"));
    }
    if prev != UiButtonVisualState::Pressed && next != UiButtonVisualState::Pressed {
        return;
    }
    if prev != UiButtonVisualState::Pressed && next == UiButtonVisualState::Pressed {
        out.push((node, "pressed"));
    }
    if prev == UiButtonVisualState::Pressed && next != UiButtonVisualState::Pressed {
        out.push((node, "released"));
    }
    if prev == UiButtonVisualState::Pressed && next == UiButtonVisualState::Hover {
        out.push((node, "click"));
    }
}

#[inline]
fn render_mask_matches(camera_mask: BitMask, render_layers: BitMask) -> bool {
    !camera_mask.intersects(render_layers)
}

fn shadow_caster_2d_state(
    transform: perro_structs::Transform2D,
    z_index: i32,
    shape: Shape2D,
) -> Option<ShadowCaster2DState> {
    if let Shape2D::Triangle {
        kind,
        width,
        height,
    } = shape
    {
        let local = triangle_points_2d(kind, width, height)?;
        let points = local.map(|point| transform_point_2d(transform, [point.x, point.y]));
        return triangle_shadow_caster(points, z_index);
    }
    let (half_extents, shape) = match shape {
        Shape2D::Quad { width, height } => (
            [
                width.abs() * transform.scale.x.abs() * 0.5,
                height.abs() * transform.scale.y.abs() * 0.5,
            ],
            ShadowCaster2DShapeState::Quad,
        ),
        Shape2D::Circle { radius } => {
            let scaled_radius = radius.abs() * transform.scale.x.abs().max(transform.scale.y.abs());
            (
                [scaled_radius, scaled_radius],
                ShadowCaster2DShapeState::Circle,
            )
        }
        Shape2D::Triangle { .. } => unreachable!(),
    };
    (half_extents[0].is_finite()
        && half_extents[1].is_finite()
        && half_extents[0] > 0.0
        && half_extents[1] > 0.0)
        .then_some(ShadowCaster2DState {
            center: [transform.position.x, transform.position.y],
            half_extents,
            rotation_radians: transform.rotation,
            shape,
            z_index,
        })
}

pub(crate) fn build_tilemap_shadow_casters(
    tilemap: &perro_nodes::TileMap2D,
    global: perro_structs::Transform2D,
    tileset: &ParsedTileset2D,
) -> Vec<ShadowCaster2DState> {
    let descs = tilemap_shape_descs_2d(
        tilemap,
        BitMask::ALL,
        BitMask::NONE,
        0.0,
        0.0,
        0.0,
        Some(tileset),
    );
    let mut out = Vec::with_capacity(descs.len());
    for desc in descs {
        match desc.shape {
            ShapeKind2D::Primitive(shape) => {
                let world = compose_transform_2d(global, desc.local);
                if let Some(caster) = shadow_caster_2d_state(world, tilemap.z_index, shape) {
                    out.push(caster);
                }
            }
            ShapeKind2D::Polygon(points) => {
                let hull = convex_hull_2d(points);
                if hull.len() < 3 {
                    continue;
                }
                let local = compose_transform_2d(global, desc.local);
                let first = transform_point_2d(local, [hull[0].x, hull[0].y]);
                for pair in hull[1..].windows(2) {
                    let points = [
                        first,
                        transform_point_2d(local, [pair[0].x, pair[0].y]),
                        transform_point_2d(local, [pair[1].x, pair[1].y]),
                    ];
                    if let Some(caster) = triangle_shadow_caster(points, tilemap.z_index) {
                        out.push(caster);
                    }
                }
            }
        }
    }
    out
}

fn triangle_shadow_caster(points: [[f32; 2]; 3], z_index: i32) -> Option<ShadowCaster2DState> {
    if !points.iter().flatten().all(|v| v.is_finite()) {
        return None;
    }
    let min = [
        points
            .iter()
            .map(|point| point[0])
            .fold(f32::INFINITY, f32::min),
        points
            .iter()
            .map(|point| point[1])
            .fold(f32::INFINITY, f32::min),
    ];
    let max = [
        points
            .iter()
            .map(|point| point[0])
            .fold(f32::NEG_INFINITY, f32::max),
        points
            .iter()
            .map(|point| point[1])
            .fold(f32::NEG_INFINITY, f32::max),
    ];
    let half_extents = [(max[0] - min[0]) * 0.5, (max[1] - min[1]) * 0.5];
    (half_extents[0] > 0.0 && half_extents[1] > 0.0).then_some(ShadowCaster2DState {
        center: [(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5],
        half_extents,
        rotation_radians: 0.0,
        shape: ShadowCaster2DShapeState::TrianglePoints(points),
        z_index,
    })
}

fn compose_transform_2d(
    parent: perro_structs::Transform2D,
    local: perro_structs::Transform2D,
) -> perro_structs::Transform2D {
    perro_structs::Transform2D::new(
        Vector2::from(transform_point_2d(
            parent,
            [local.position.x, local.position.y],
        )),
        parent.rotation + local.rotation,
        Vector2::new(
            parent.scale.x * local.scale.x,
            parent.scale.y * local.scale.y,
        ),
    )
}

fn transform_point_2d(transform: perro_structs::Transform2D, point: [f32; 2]) -> [f32; 2] {
    let (sin_r, cos_r) = transform.rotation.sin_cos();
    let x = point[0] * transform.scale.x;
    let y = point[1] * transform.scale.y;
    [
        transform.position.x + x * cos_r - y * sin_r,
        transform.position.y + x * sin_r + y * cos_r,
    ]
}

fn convex_hull_2d(mut points: Vec<Vector2>) -> Vec<Vector2> {
    points.retain(|point| point.x.is_finite() && point.y.is_finite());
    points.sort_unstable_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points.dedup_by(|a, b| a.x == b.x && a.y == b.y);
    if points.len() <= 2 {
        return points;
    }
    let mut hull = Vec::with_capacity(points.len() * 2);
    for point in points.iter().copied() {
        while hull.len() >= 2 && cross_2d(hull[hull.len() - 2], hull[hull.len() - 1], point) <= 0.0
        {
            hull.pop();
        }
        hull.push(point);
    }
    let lower_len = hull.len();
    for point in points.iter().rev().skip(1).copied() {
        while hull.len() > lower_len
            && cross_2d(hull[hull.len() - 2], hull[hull.len() - 1], point) <= 0.0
        {
            hull.pop();
        }
        hull.push(point);
    }
    hull.pop();
    hull
}

fn cross_2d(a: Vector2, b: Vector2, c: Vector2) -> f32 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

fn camera_stream_aspect_ratio(aspect_ratio: f32, resolution: UVector2) -> f32 {
    if aspect_ratio.is_finite() && aspect_ratio > 0.0 {
        aspect_ratio
    } else {
        resolution.x.max(1) as f32 / resolution.y.max(1) as f32
    }
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

fn water_local_point_2d(
    inv_transform: glam::Mat3,
    point: perro_structs::Vector2,
) -> perro_structs::Vector2 {
    let p = inv_transform * glam::Vec3::new(point.x, point.y, 1.0);
    perro_structs::Vector2::new(p.x, p.y)
}

fn water_global_point_2d(
    transform: perro_structs::Transform2D,
    point: perro_structs::Vector2,
) -> perro_structs::Vector2 {
    let p = transform.to_mat3() * glam::Vec3::new(point.x, point.y, 1.0);
    perro_structs::Vector2::new(p.x, p.y)
}

fn water_surface_corners(size: perro_structs::Vector2) -> [perro_structs::Vector2; 4] {
    let half = size * 0.5;
    [
        perro_structs::Vector2::new(-half.x, -half.y),
        perro_structs::Vector2::new(half.x, -half.y),
        perro_structs::Vector2::new(-half.x, half.y),
        perro_structs::Vector2::new(half.x, half.y),
    ]
}

fn water_overlap_bounds_2d(
    water: &perro_nodes::WaterSurfaceParams,
    water_transform: perro_structs::Transform2D,
    other: perro_nodes::WaterSurfaceParams,
    other_transform: perro_structs::Transform2D,
) -> Option<(perro_structs::Vector2, perro_structs::Vector2)> {
    let water_inv = water_transform.to_mat3().inverse();
    let other_inv = other_transform.to_mat3().inverse();
    let mut points = Vec::new();
    for corner in water_surface_corners(other.shape.surface_size()) {
        let world = water_global_point_2d(other_transform, corner);
        let local = water_local_point_2d(water_inv, world);
        if water.shape.contains_surface(local) {
            points.push(local);
        }
    }
    for corner in water_surface_corners(water.shape.surface_size()) {
        let world = water_global_point_2d(water_transform, corner);
        let other_local = water_local_point_2d(other_inv, world);
        if other.shape.contains_surface(other_local) {
            points.push(corner);
        }
    }
    let other_center = water_local_point_2d(water_inv, other_transform.position);
    if water.shape.contains_surface(other_center) {
        points.push(other_center);
    }
    let water_center_in_other = water_local_point_2d(other_inv, water_transform.position);
    if other.shape.contains_surface(water_center_in_other) {
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

pub(crate) fn resolve_tileset_2d(
    runtime: &mut Runtime,
    source: &TileSetRef,
) -> Option<Arc<ParsedTileset2D>> {
    if source.is_empty() {
        return None;
    }
    let source_hash = source.id().as_u64();
    while let Ok((hash, tileset)) = runtime.render_2d.tileset_load_rx.try_recv() {
        runtime.render_2d.pending_tileset_loads.remove(&hash);
        if let Some(tileset) = tileset {
            runtime
                .render_2d
                .tileset_cache
                .insert(hash, Arc::new(tileset));
        }
    }
    if let Some(tileset) = runtime.render_2d.tileset_cache.get(&source_hash) {
        return Some(tileset.clone());
    }
    let static_tileset = if runtime.provider_mode() == crate::runtime_project::ProviderMode::Static
    {
        runtime
            .project()
            .and_then(|project| project.static_tileset_lookup)
            .map(|lookup| lookup(source_hash))
            .filter(|bytes| !bytes.is_empty())
    } else {
        None
    };
    if let Some(bytes) = static_tileset {
        let tileset = Arc::new(perro_render_bridge::decode_tileset_2d_binary(bytes)?);
        runtime
            .render_2d
            .tileset_cache
            .insert(source_hash, tileset.clone());
        return Some(tileset);
    }

    if runtime.render_2d.pending_tileset_loads.insert(source_hash) {
        let source = source.source().to_string();
        let tx = runtime.render_2d.tileset_load_tx.clone();
        #[cfg(not(target_arch = "wasm32"))]
        rayon::spawn(move || {
            let tileset = perro_io::load_asset(source.as_str())
                .ok()
                .and_then(|bytes| {
                    std::str::from_utf8(&bytes)
                        .ok()
                        .and_then(perro_render_bridge::parse_ptileset_source)
                });
            let _ = tx.send((source_hash, tileset));
        });
        #[cfg(target_arch = "wasm32")]
        {
            let tileset = perro_io::load_asset(source.as_str())
                .ok()
                .and_then(|bytes| {
                    std::str::from_utf8(&bytes)
                        .ok()
                        .and_then(perro_render_bridge::parse_ptileset_source)
                });
            let _ = tx.send((source_hash, tileset));
        }
    }
    None
}

pub(crate) struct TilemapSpriteBuild<'a> {
    pub texture: TextureID,
    pub width: u32,
    pub height: u32,
    pub z_index: i32,
    pub empty_tile: i32,
    pub tint: perro_structs::Color,
    pub base_model: [[f32; 3]; 3],
    pub tiles: &'a [i32],
    pub tileset: &'a ParsedTileset2D,
}

pub(crate) fn build_tilemap_sprites(build: TilemapSpriteBuild<'_>) -> Vec<Sprite2DCommand> {
    let max = (build.width as usize)
        .saturating_mul(build.height as usize)
        .min(build.tiles.len());
    let mut out = Vec::with_capacity(max);
    let [tw, th] = build.tileset.tile_size;
    for (idx, tile_id) in build.tiles.iter().take(max).copied().enumerate() {
        if tile_id == build.empty_tile {
            continue;
        }
        let Some(tile) = build.tileset.tile(tile_id) else {
            continue;
        };
        let x = (idx as u32 % build.width) as f32 * tw;
        let y = (idx as u32 / build.width) as f32 * th;
        let model = mul_mat3(build.base_model, translation_mat3(x, -y));
        let atlas_x = tile.atlas[0] as f32 * tw;
        let atlas_y = tile.atlas[1] as f32 * th;
        out.push(Sprite2DCommand {
            texture: build.texture,
            model,
            tint: build.tint,
            uv_min: [atlas_x, atlas_y],
            uv_max: [atlas_x + tw, atlas_y + th],
            uv_normalized: false,
            size: [tw, th],
            z_index: build.z_index,
        });
    }
    out
}

fn translation_mat3(x: f32, y: f32) -> [[f32; 3]; 3] {
    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [x, y, 1.0]]
}

fn mul_mat3(a: [[f32; 3]; 3], b: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let mut out = [[0.0; 3]; 3];
    for c in 0..3 {
        for r in 0..3 {
            out[c][r] = a[0][r] * b[c][0] + a[1][r] * b[c][1] + a[2][r] * b[c][2];
        }
    }
    out
}

pub(crate) fn direction_from_rotation_2d(rotation: f32) -> [f32; 2] {
    [rotation.sin(), -rotation.cos()]
}

pub(crate) fn shadow_softness_2d(softness: f32) -> f32 {
    if softness.is_finite() {
        softness.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

pub(crate) fn derived_particle_budget(spawn_rate: f32, lifetime_max: f32) -> u32 {
    if spawn_rate <= 0.0 || lifetime_max <= 0.0 {
        return 1;
    }
    let budget = (spawn_rate * lifetime_max).ceil() as u32 + 2;
    budget.clamp(1, 1_000_000)
}

pub(crate) fn resolve_particle_sim_mode_2d(
    mode: ParticleEmitterSimMode2D,
) -> ParticleSimulationMode2D {
    match mode {
        ParticleEmitterSimMode2D::Default | ParticleEmitterSimMode2D::Cpu => {
            ParticleSimulationMode2D::Cpu
        }
    }
}

pub(crate) fn resolve_particle_profile_2d(
    runtime: &mut Runtime,
    source: &ParticleProfileRef,
) -> Option<ParticleProfile2D> {
    let source_path = source.source().trim();
    if source_path.is_empty() {
        return None;
    }
    let source_key = source.id().as_u64();
    while let Ok((loaded_source, profile)) = runtime.render_2d.particle_path_load_rx.try_recv() {
        let loaded_key = ParticleProfileRef::from(loaded_source.as_str())
            .id()
            .as_u64();
        runtime
            .render_2d
            .pending_particle_path_loads
            .remove(&loaded_key);
        if let Some(profile) = profile {
            cache_particle_profile_2d(runtime, loaded_key, profile);
        }
    }
    if let Some(path) = runtime.render_2d.particle_path_cache.get(&source_key) {
        return Some(path.clone());
    }
    let parsed = if runtime.provider_mode() == crate::runtime_project::ProviderMode::Static {
        if let Some(inline) = source_path.strip_prefix("inline://") {
            parse_pparticle_source_2d(inline)?
        } else if let Some(lookup) = runtime
            .project()
            .and_then(|project| project.static_particle_lookup)
        {
            particle_profile_2d_from_3d(lookup(source_key))
        } else if runtime
            .render_2d
            .pending_particle_path_loads
            .insert(source_key)
        {
            spawn_particle_profile_2d_load(
                source_path.to_string(),
                runtime.render_2d.particle_path_load_tx.clone(),
            );
            return None;
        } else {
            return None;
        }
    } else if let Some(inline) = source_path.strip_prefix("inline://") {
        parse_pparticle_source_2d(inline)?
    } else if runtime
        .render_2d
        .pending_particle_path_loads
        .insert(source_key)
    {
        spawn_particle_profile_2d_load(
            source_path.to_string(),
            runtime.render_2d.particle_path_load_tx.clone(),
        );
        return None;
    } else {
        return None;
    };
    cache_particle_profile_2d(runtime, source_key, parsed.clone());
    Some(parsed)
}

fn cache_particle_profile_2d(runtime: &mut Runtime, source_key: u64, parsed: ParticleProfile2D) {
    if !runtime
        .render_2d
        .particle_path_cache
        .contains_key(&source_key)
    {
        while runtime.render_2d.particle_path_cache.len() >= PARTICLE_PATH_CACHE_MAX {
            let Some(evict_key) = runtime.render_2d.particle_path_cache_order.pop_front() else {
                break;
            };
            runtime.render_2d.particle_path_cache.remove(&evict_key);
        }
        runtime
            .render_2d
            .particle_path_cache_order
            .push_back(source_key);
    }
    runtime
        .render_2d
        .particle_path_cache
        .insert(source_key, parsed);
}

fn spawn_particle_profile_2d_load(
    source: String,
    tx: std::sync::mpsc::Sender<(String, Option<ParticleProfile2D>)>,
) {
    #[cfg(not(target_arch = "wasm32"))]
    rayon::spawn(move || {
        let profile = perro_io::load_asset(source.as_str())
            .ok()
            .and_then(|bytes| {
                std::str::from_utf8(&bytes)
                    .ok()
                    .and_then(parse_pparticle_source_2d)
            });
        let _ = tx.send((source, profile));
    });
    #[cfg(target_arch = "wasm32")]
    {
        let profile = perro_io::load_asset(source.as_str())
            .ok()
            .and_then(|bytes| {
                std::str::from_utf8(&bytes)
                    .ok()
                    .and_then(parse_pparticle_source_2d)
            });
        let _ = tx.send((source, profile));
    }
}

fn particle_profile_2d_from_3d(
    profile: &perro_render_bridge::ParticleProfile3D,
) -> ParticleProfile2D {
    let path = match profile.path {
        perro_render_bridge::ParticlePath3D::None => ParticlePath2D::None,
        perro_render_bridge::ParticlePath3D::Ballistic => ParticlePath2D::Ballistic,
        perro_render_bridge::ParticlePath3D::Spiral {
            angular_velocity,
            radius,
        } => ParticlePath2D::Spiral {
            angular_velocity,
            radius,
        },
        perro_render_bridge::ParticlePath3D::NoiseDrift {
            amplitude,
            frequency,
        } => ParticlePath2D::NoiseDrift {
            amplitude,
            frequency,
        },
        perro_render_bridge::ParticlePath3D::FlatDisk { radius } => {
            ParticlePath2D::FlatDisk { radius }
        }
        perro_render_bridge::ParticlePath3D::OrbitY { .. }
        | perro_render_bridge::ParticlePath3D::Custom { .. }
        | perro_render_bridge::ParticlePath3D::CustomCompiled { .. } => ParticlePath2D::None,
    };
    ParticleProfile2D {
        path,
        expr_x_ops: profile.expr_x_ops.clone(),
        expr_y_ops: profile.expr_y_ops.clone(),
        lifetime_min: profile.lifetime_min,
        lifetime_max: profile.lifetime_max,
        speed_min: profile.speed_min,
        speed_max: profile.speed_max,
        spread_radians: profile.spread_radians,
        size: profile.size,
        size_min: profile.size_min,
        size_max: profile.size_max,
        force: [profile.force[0], profile.force[1]],
        color_start: profile.color_start,
        color_end: profile.color_end,
        spin_angular_velocity: profile.spin_angular_velocity,
    }
}

fn parse_pparticle_source_2d(source: &str) -> Option<ParticleProfile2D> {
    let mut profile = ParticleProfile2D::default();
    let mut preset: Option<String> = None;
    let mut preset_param_a = 1.0f32;
    let mut preset_param_b = 1.0f32;
    let mut expr_x = String::from("0.0");
    let mut expr_y = String::from("0.0");
    let mut has_expr_x = false;
    let mut has_expr_y = false;
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let (key, value) = line.split_once('=')?;
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        match key.as_str() {
            "preset" => preset = Some(value.to_ascii_lowercase()),
            "preset_param_a" => {
                preset_param_a = value.parse::<f32>().ok().unwrap_or(preset_param_a);
            }
            "preset_param_b" => {
                preset_param_b = value.parse::<f32>().ok().unwrap_or(preset_param_b);
            }
            "x" => {
                expr_x = value.to_string();
                has_expr_x = true;
            }
            "y" => {
                expr_y = value.to_string();
                has_expr_y = true;
            }
            "force" => {
                if let Some(v) = parse_vec2_or_vec3_literal_2d(value) {
                    profile.force = v;
                }
            }
            "force_x" => profile.force[0] = value.parse::<f32>().ok()?,
            "force_y" => profile.force[1] = value.parse::<f32>().ok()?,
            "lifetime_min" => {
                profile.lifetime_min = value.parse::<f32>().ok().unwrap_or(profile.lifetime_min);
            }
            "lifetime_max" => {
                profile.lifetime_max = value.parse::<f32>().ok().unwrap_or(profile.lifetime_max);
            }
            "speed_min" => {
                profile.speed_min = value.parse::<f32>().ok().unwrap_or(profile.speed_min)
            }
            "speed_max" => {
                profile.speed_max = value.parse::<f32>().ok().unwrap_or(profile.speed_max)
            }
            "spread_radians" => {
                profile.spread_radians =
                    value.parse::<f32>().ok().unwrap_or(profile.spread_radians);
            }
            "size" => profile.size = value.parse::<f32>().ok().unwrap_or(profile.size),
            "size_min" => profile.size_min = value.parse::<f32>().ok().unwrap_or(profile.size_min),
            "size_max" => profile.size_max = value.parse::<f32>().ok().unwrap_or(profile.size_max),
            "color_start" => {
                if let Some(v) = parse_vec4_literal_2d(value) {
                    profile.color_start = v.into();
                }
            }
            "color_end" => {
                if let Some(v) = parse_vec4_literal_2d(value) {
                    profile.color_end = v.into();
                }
            }
            "spin" => {
                profile.spin_angular_velocity = value
                    .parse::<f32>()
                    .ok()
                    .unwrap_or(profile.spin_angular_velocity);
            }
            _ => {}
        }
    }
    profile.path = match preset.as_deref() {
        None => ParticlePath2D::None,
        Some("ballistic") => ParticlePath2D::Ballistic,
        Some("spiral") => ParticlePath2D::Spiral {
            angular_velocity: preset_param_a,
            radius: preset_param_b.abs(),
        },
        Some("noise_drift") => ParticlePath2D::NoiseDrift {
            amplitude: preset_param_a.abs(),
            frequency: preset_param_b.abs(),
        },
        Some("flat_disk") => ParticlePath2D::FlatDisk {
            radius: preset_param_a.abs(),
        },
        Some("orbit_y") | Some(_) => ParticlePath2D::None,
    };
    if has_expr_x || has_expr_y {
        profile.expr_x_ops = Some(Cow::Owned(compile_expression(&expr_x).ok()?.ops().to_vec()));
        profile.expr_y_ops = Some(Cow::Owned(compile_expression(&expr_y).ok()?.ops().to_vec()));
    }
    Some(profile)
}

fn parse_vec2_or_vec3_literal_2d(raw: &str) -> Option<[f32; 2]> {
    let raw = raw.trim();
    let inner = raw.strip_prefix('(')?.strip_suffix(')')?;
    let mut it = inner.split(',').map(|v| v.trim().parse::<f32>().ok());
    Some([it.next()??, it.next()??])
}

fn parse_vec4_literal_2d(raw: &str) -> Option<[f32; 4]> {
    let raw = raw.trim();
    let inner = raw.strip_prefix('(')?.strip_suffix(')')?;
    let mut it = inner.split(',').map(|v| v.trim().parse::<f32>().ok());
    Some([it.next()??, it.next()??, it.next()??, it.next()??])
}

fn sprite_region_uv(region: Option<[f32; 4]>) -> ([f32; 2], [f32; 2], [f32; 2]) {
    let Some([x, y, w, h]) = region else {
        return ([0.0, 0.0], [0.0, 0.0], [0.0, 0.0]);
    };
    if !(x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite()) || w <= 0.0 || h <= 0.0 {
        return ([0.0, 0.0], [1.0, 1.0], [0.0, 0.0]);
    }
    ([x, y], [x + w, y + h], [w, h])
}

fn build_nine_slice_sprites(
    texture: TextureID,
    region: Option<[f32; 4]>,
    base_model: [[f32; 3]; 3],
    size: Vector2,
    margins: [f32; 4],
    tint: perro_structs::Color,
    z_index: i32,
) -> Vec<Sprite2DCommand> {
    let auto = margins.iter().all(|margin| *margin == 0.0);
    let ([u0, v0], [u3, v3], region_size) = if auto && region.is_none() {
        ([0.0, 0.0], [1.0, 1.0], [1.0, 1.0])
    } else {
        sprite_region_uv(region)
    };
    let w = size.x.max(0.0);
    let h = size.y.max(0.0);
    let margins = if auto {
        [w / 3.0, h / 3.0, w / 3.0, h / 3.0]
    } else {
        margins
    };
    let [l, t, r, b] = clamp_nine_margins(margins, w, h);
    let uv_w = (u3 - u0).max(region_size[0]);
    let uv_h = (v3 - v0).max(region_size[1]);
    let (ul, ur, vt, vb) = if auto {
        (uv_w / 3.0, uv_w / 3.0, uv_h / 3.0, uv_h / 3.0)
    } else {
        let ul = l.min(uv_w);
        let vt = t.min(uv_h);
        (
            ul,
            r.min((uv_w - ul).max(0.0)),
            vt,
            b.min((uv_h - vt).max(0.0)),
        )
    };
    let xs = [-w * 0.5, -w * 0.5 + l, w * 0.5 - r, w * 0.5];
    let ys = [-h * 0.5, -h * 0.5 + b, h * 0.5 - t, h * 0.5];
    let us = [u0, u0 + ul, u3 - ur, u3];
    let vs = [v0, v0 + vb, v3 - vt, v3];
    let mut out = Vec::with_capacity(9);
    for y in 0..3 {
        for x in 0..3 {
            let sw = xs[x + 1] - xs[x];
            let sh = ys[y + 1] - ys[y];
            if sw <= 0.0 || sh <= 0.0 {
                continue;
            }
            let cx = (xs[x] + xs[x + 1]) * 0.5;
            let cy = (ys[y] + ys[y + 1]) * 0.5;
            out.push(Sprite2DCommand {
                texture,
                model: mul_mat3(base_model, translation_mat3(cx, cy)),
                tint,
                uv_min: [us[x], vs[y]],
                uv_max: [us[x + 1], vs[y + 1]],
                uv_normalized: auto && region.is_none(),
                size: [sw, sh],
                z_index,
            });
        }
    }
    out
}

fn clamp_nine_margins(margins: [f32; 4], w: f32, h: f32) -> [f32; 4] {
    let mut l = margins[0].max(0.0);
    let mut t = margins[1].max(0.0);
    let mut r = margins[2].max(0.0);
    let mut b = margins[3].max(0.0);
    let sx = (w / (l + r).max(w)).min(1.0);
    let sy = (h / (t + b).max(h)).min(1.0);
    l *= sx;
    r *= sx;
    t *= sy;
    b *= sy;
    [l, t, r, b]
}

#[cfg(test)]
#[path = "../../../tests/unit/runtime_render_2d_tests.rs"]
mod tests;
