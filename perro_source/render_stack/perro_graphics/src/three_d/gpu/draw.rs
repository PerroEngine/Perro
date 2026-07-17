use super::*;

pub(super) struct DrawBatchPush {
    pub(super) render_path: RenderPath3D,
    pub(super) mesh: MeshRange,
    pub(super) instance_start: u32,
    pub(super) instance_count: u32,
    pub(super) double_sided: bool,
    pub(super) packed_lod: bool,
    pub(super) material_kind: MaterialPipelineKind,
    pub(super) alpha_mode: u8,
    pub(super) base_color_texture_slot: u32,
    pub(super) material_texture_key: MaterialTextureKey,
    pub(super) local_bounds: ([f32; 3], f32),
    pub(super) occlusion_query: Option<u32>,
    pub(super) disable_hiz_occlusion: bool,
    pub(super) casts_shadows: bool,
    pub(super) receives_shadows: bool,
    pub(super) mesh_blend: bool,
    pub(super) mesh_blend_screen: bool,
    pub(super) mesh_blend_params: u32,
    pub(super) mesh_blend_depth: bool,
    pub(super) blend_layers: u32,
    pub(super) blend_mask: u32,
}

pub(super) fn push_draw_batch(draw_batches: &mut Vec<DrawBatch>, batch: DrawBatchPush) {
    let DrawBatchPush {
        render_path,
        mesh,
        instance_start,
        instance_count,
        double_sided,
        packed_lod,
        material_kind,
        alpha_mode,
        base_color_texture_slot,
        material_texture_key,
        local_bounds,
        occlusion_query,
        disable_hiz_occlusion,
        casts_shadows,
        receives_shadows,
        mesh_blend,
        mesh_blend_screen,
        mesh_blend_params,
        mesh_blend_depth,
        blend_layers,
        blend_mask,
    } = batch;
    if instance_count == 0 {
        return;
    }
    let state_key = draw_batch_state_key(
        render_path,
        false,
        double_sided,
        alpha_mode,
        packed_lod,
        &material_kind,
    );
    let render_state = render_state_key(
        state_key,
        material_texture_key.state_hash(),
        mesh.index_start,
        mesh.base_vertex,
        false,
        alpha_mode,
        mesh_blend,
    );
    let order_index = next_draw_batch_order(draw_batches);
    let (local_center, local_radius) = local_bounds;
    if occlusion_query.is_none()
        && let Some(prev) = draw_batches.last_mut()
    {
        let prev_end = prev.instance_start.saturating_add(prev.instance_count);
        let same_mesh = prev.mesh.index_start == mesh.index_start
            && prev.mesh.index_count == mesh.index_count
            && prev.mesh.base_vertex == mesh.base_vertex;
        let same_batch_state = prev.state_key == state_key
            && prev.path == render_path
            && prev.packed_lod == packed_lod
            && prev.double_sided == double_sided
            && prev.material_kind == material_kind
            && prev.alpha_mode == alpha_mode
            && !prev.draw_on_top
            && prev.base_color_texture_slot == base_color_texture_slot
            && prev.material_texture_key == material_texture_key
            && prev.occlusion_query.is_none()
            && prev.casts_shadows == casts_shadows
            && prev.receives_shadows == receives_shadows
            && prev.mesh_blend == mesh_blend
            && prev.mesh_blend_screen == mesh_blend_screen
            && prev.mesh_blend_params == mesh_blend_params
            && prev.mesh_blend_depth == mesh_blend_depth
            && prev.blend_layers == blend_layers
            && prev.blend_mask == blend_mask;
        if same_mesh && same_batch_state && prev_end == instance_start {
            prev.instance_count = prev.instance_count.saturating_add(instance_count);
            prev.disable_hiz_occlusion |= disable_hiz_occlusion;
            // Same mesh (checked above), so both bounds share one local space;
            // keep the tight enclosing sphere. The cull upload expands it into a
            // merged per-instance world sphere for multi-instance batches.
            let (center, radius) = enclose_local_spheres(
                (prev.local_center, prev.local_radius),
                (local_center, local_radius.max(0.0)),
            );
            prev.local_center = center;
            prev.local_radius = radius;
            return;
        }
    }
    draw_batches.push(DrawBatch {
        state_key,
        render_state,
        mesh,
        instance_start,
        instance_count,
        path: render_path,
        packed_lod,
        double_sided,
        material_kind,
        alpha_mode,
        draw_on_top: false,
        base_color_texture_slot,
        material_texture_key,
        local_center,
        local_radius: local_radius.max(0.0),
        occlusion_query,
        disable_hiz_occlusion,
        casts_shadows,
        receives_shadows,
        mesh_blend,
        mesh_blend_screen,
        mesh_blend_params,
        mesh_blend_depth,
        blend_layers,
        blend_mask,
        order_index,
    });
}

// Normalized rgb in the color lanes, max-component / EMISSIVE_PACK_MAX in the
// alpha lane, so emissive keeps HDR magnitude through the unorm8 pack.

#[path = "draw/material.rs"]
mod material;
pub(super) use material::*;
#[path = "draw/batch.rs"]
mod batch;
pub(super) use batch::*;
#[path = "draw/commands.rs"]
mod commands;
pub(super) use commands::*;
