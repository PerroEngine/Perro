use crate::{
    rendering::{PrimitiveRenderer, RenderLayer, TextureManager},
    structs2d::{Transform2D, Vector2},
    ui_elements::ui_container::CornerRadius,
};
use wgpu::{Device, Queue, RenderPass};

pub struct RendererUI {}

impl RendererUI {
    pub fn new() -> Self {
        println!("🟩 UI Renderer initialized");
        Self {}
    }

    pub fn queue_panel(
        &mut self,
        primitive_renderer: &mut PrimitiveRenderer,
        uuid: uuid::Uuid,
        transform: Transform2D,
        size: Vector2,
        pivot: Vector2,
        color: crate::structs::Color,
        corner_radius: Option<CornerRadius>,
        border_thickness: f32,
        is_border: bool,
        z_index: i32,
    ) {
        primitive_renderer.queue_rect(
            uuid,
            RenderLayer::UI,
            transform,
            size,
            pivot,
            color,
            corner_radius,
            border_thickness,
            is_border,
            z_index,
        );
    }

    pub fn queue_image(
        &mut self,
        primitive_renderer: &mut PrimitiveRenderer,
        texture_manager: &mut TextureManager,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        uuid: uuid::Uuid,
        texture_path: &str,
        transform: Transform2D,
        pivot: Vector2,
        z_index: i32,
    ) {
        primitive_renderer.queue_texture(
            uuid,
            RenderLayer::UI,
            texture_path,
            transform,
            pivot,
            z_index,
            texture_manager,
            device,
            queue,
        );
    }

    pub fn queue_text(
        &mut self,
        primitive_renderer: &mut PrimitiveRenderer,
        uuid: uuid::Uuid,
        text: &str,
        font_size: f32,
        transform: Transform2D,
        pivot: Vector2,
        color: crate::structs::Color,
        z_index: i32,
    ) {
        primitive_renderer.queue_text(
            uuid,
            RenderLayer::UI,
            text,
            font_size,
            transform,
            pivot,
            color,
            z_index,
        );
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
