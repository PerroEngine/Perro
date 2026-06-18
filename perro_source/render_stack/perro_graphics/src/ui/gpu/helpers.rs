use super::*;

pub(super) fn font_delta_required_size(
    delta_size: [u32; 2],
    origin: [usize; 2],
    texture_size: [u32; 2],
) -> [u32; 2] {
    let origin_x = origin[0].min(u32::MAX as usize) as u32;
    let origin_y = origin[1].min(u32::MAX as usize) as u32;
    let required_width = origin_x.saturating_add(delta_size[0]);
    let required_height = origin_y.saturating_add(delta_size[1]);
    [
        texture_size[0].max(required_width).max(1),
        texture_size[1].max(required_height).max(1),
    ]
}

pub(super) fn clip_rect_scaled(
    primitive: &ClippedPrimitive,
    viewport: [u32; 2],
    scale: [f32; 2],
) -> [u32; 4] {
    let scale_x = scale[0].max(0.0001);
    let scale_y = scale[1].max(0.0001);
    let min_x = (primitive.clip_rect.min.x * scale_x).floor().max(0.0) as u32;
    let min_y = (primitive.clip_rect.min.y * scale_y).floor().max(0.0) as u32;
    let max_x = (primitive.clip_rect.max.x * scale_x)
        .ceil()
        .min(viewport[0] as f32)
        .max(min_x as f32) as u32;
    let max_y = (primitive.clip_rect.max.y * scale_y)
        .ceil()
        .min(viewport[1] as f32)
        .max(min_y as f32) as u32;
    [min_x, min_y, max_x - min_x, max_y - min_y]
}

pub(super) fn supersampled_size(viewport: [u32; 2], max_dimension: u32) -> [u32; 2] {
    let width = viewport[0].max(1).saturating_mul(UI_SUPERSAMPLE_SCALE);
    let height = viewport[1].max(1).saturating_mul(UI_SUPERSAMPLE_SCALE);
    let (width, height) = crate::gpu::capped_render_size(width, height, max_dimension);
    [width, height]
}

pub(super) fn viewport_scale(viewport: [u32; 2], render_viewport: [u32; 2]) -> [f32; 2] {
    [
        render_viewport[0].max(1) as f32 / viewport[0].max(1) as f32,
        render_viewport[1].max(1) as f32 / viewport[1].max(1) as f32,
    ]
}

pub(super) fn upload_or_grow_buffer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    current: Option<wgpu::Buffer>,
    capacity_bytes: &mut u64,
    label: &'static str,
    usage: wgpu::BufferUsages,
    bytes: &[u8],
) -> Option<wgpu::Buffer> {
    if bytes.is_empty() {
        return current;
    }
    let required = bytes.len() as u64;
    if let Some(buffer) = current
        && *capacity_bytes >= required
    {
        queue.write_buffer(&buffer, 0, bytes);
        return Some(buffer);
    }
    let capacity = required.next_power_of_two();
    *capacity_bytes = capacity;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: capacity,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&buffer, 0, bytes);
    Some(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_delta_required_size_covers_partial_origin() {
        assert_eq!(
            font_delta_required_size([55, 12], [90, 4], [55, 16]),
            [145, 16]
        );
    }

    #[test]
    fn font_delta_required_size_keeps_atlas_size() {
        assert_eq!(
            font_delta_required_size([55, 12], [0, 0], [2048, 32]),
            [2048, 32]
        );
    }
}
