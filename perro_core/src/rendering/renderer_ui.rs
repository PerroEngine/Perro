use crate::{
    ids::UIElementID,
    rendering::{PrimitiveRenderer, RenderLayer, TextureManager},
    structs2d::{Transform2D, Vector2},
    ui_elements::{ui_container::CornerRadius, ui_text::TextAlignment},
};
use wgpu::{Device, Queue, RenderPass};

pub struct RendererUI {}

impl RendererUI {
    pub fn new() -> Self {
        Self {}
    }

    pub fn queue_panel(
        &mut self,
        primitive_renderer: &mut PrimitiveRenderer,
        id: UIElementID,
        transform: Transform2D,
        size: Vector2,
        pivot: Vector2,
        color: crate::structs::Color,
        corner_radius: Option<CornerRadius>,
        border_thickness: f32,
        is_border: bool,
        z_index: i32,
        created_timestamp: u64,
    ) {
        primitive_renderer.queue_rect(
            id.as_u64(),
            RenderLayer::UI,
            transform,
            size,
            pivot,
            color,
            corner_radius,
            border_thickness,
            is_border,
            z_index,
            created_timestamp,
        );
    }

    pub fn queue_image(
        &mut self,
        primitive_renderer: &mut PrimitiveRenderer,
        texture_manager: &mut TextureManager,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        id: u64,
        texture_path: &str,
        transform: Transform2D,
        pivot: Vector2,
        z_index: i32,
        created_timestamp: u64,
    ) {
        let texture_id = match texture_manager.get_or_load_texture_id(texture_path, device, queue) {
            Ok(id) => id,
            Err(_) => return,
        };
        primitive_renderer.queue_texture(
            id,
            RenderLayer::UI,
            texture_id,
            transform,
            pivot,
            z_index,
            created_timestamp,
            texture_manager,
            device,
            queue,
        );
    }

    pub fn queue_text(
        &mut self,
        primitive_renderer: &mut PrimitiveRenderer,
        id: u64,
        text: &str,
        font_size: f32,
        transform: Transform2D,
        pivot: Vector2,
        color: crate::structs::Color,
        z_index: i32,
        created_timestamp: u64,
        font_spec: Option<&str>,
        device: &Device,
        queue: &Queue,
    ) {
        primitive_renderer.queue_text_aligned_with_font(
            id,
            RenderLayer::UI,
            text,
            font_size,
            transform,
            pivot,
            color,
            z_index,
            created_timestamp,
            TextAlignment::Left,
            TextAlignment::Center,
            font_spec,
            device,
            queue,
        );
    }

    pub fn queue_text_aligned(
        &mut self,
        primitive_renderer: &mut PrimitiveRenderer,
        id: UIElementID,
        text: &str,
        font_size: f32,
        transform: Transform2D,
        pivot: Vector2,
        color: crate::structs::Color,
        z_index: i32,
        created_timestamp: u64,
        align_h: TextAlignment,
        align_v: TextAlignment,
        font_spec: Option<&str>,
        device: &Device,
        queue: &Queue,
    ) {
        primitive_renderer.queue_text_aligned_with_font(
            id.as_u64(),
            RenderLayer::UI,
            text,
            font_size,
            transform,
            pivot,
            color,
            z_index,
            created_timestamp,
            align_h,
            align_v,
            font_spec,
            device,
            queue,
        );
    }

    /// Remove a panel from the render cache
    /// Call this when an element becomes invisible
    pub fn remove_panel(&mut self, primitive_renderer: &mut PrimitiveRenderer, id: UIElementID) {
        primitive_renderer.remove_rect(id.as_u64());
    }

    /// Remove text from the render cache
    /// Call this when an element becomes invisible
    pub fn remove_text(&mut self, primitive_renderer: &mut PrimitiveRenderer, id: UIElementID) {
        primitive_renderer.remove_text(id.as_u64());
    }

    pub fn render(
        &mut self,
        primitive_renderer: &mut PrimitiveRenderer,
        rpass: &mut RenderPass<'_>,
        texture_manager: &mut TextureManager,
        device: &Device,
        queue: &Queue,
        camera_bind_group: &wgpu::BindGroup,
        vertex_buffer: &wgpu::Buffer,
    ) {
        primitive_renderer.render_layer(
            RenderLayer::UI,
            rpass,
            texture_manager,
            device,
            queue,
            camera_bind_group,
            vertex_buffer,
        );
    }
}
