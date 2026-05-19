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
    scale: u32,
) -> [u32; 4] {
    let scale = scale.max(1) as f32;
    let min_x = (primitive.clip_rect.min.x * scale).floor().max(0.0) as u32;
    let min_y = (primitive.clip_rect.min.y * scale).floor().max(0.0) as u32;
    let max_x = (primitive.clip_rect.max.x * scale)
        .ceil()
        .min(viewport[0] as f32)
        .max(min_x as f32) as u32;
    let max_y = (primitive.clip_rect.max.y * scale)
        .ceil()
        .min(viewport[1] as f32)
        .max(min_y as f32) as u32;
    [min_x, min_y, max_x - min_x, max_y - min_y]
}

pub(super) fn supersampled_size(viewport: [u32; 2]) -> [u32; 2] {
    [
        viewport[0].max(1).saturating_mul(UI_SUPERSAMPLE_SCALE),
        viewport[1].max(1).saturating_mul(UI_SUPERSAMPLE_SCALE),
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
