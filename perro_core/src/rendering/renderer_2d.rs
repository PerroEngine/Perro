use crate::{
    rendering::{PrimitiveRenderer, RenderLayer, TextureManager},
    structs2d::{Transform2D, Vector2},
    ui_elements::ui_container::CornerRadius,
};
use wgpu::{Device, Queue, RenderPass};

pub struct Renderer2D {}

impl Renderer2D {
    pub fn new() -> Self {
        println!("🟦 2D Renderer initialized");
        Self {}
    }

    pub fn queue_rect(
        &mut self,
        primitive_renderer: &mut PrimitiveRenderer,
        uuid: crate::uid32::NodeID,
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
            uuid.as_uid32(),
            RenderLayer::World2D,
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

    pub fn queue_texture(
        &mut self,
        primitive_renderer: &mut PrimitiveRenderer,
        texture_manager: &mut TextureManager,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        uuid: crate::uid32::NodeID,
        texture_path: &str,
        transform: Transform2D,
        pivot: Vector2,
        z_index: i32,
        created_timestamp: u64,
    ) {
        primitive_renderer.queue_texture(
            uuid.as_uid32(),
            RenderLayer::World2D,
            texture_path,
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
        uuid: crate::uid32::NodeID,
        text: &str,
        font_size: f32,
        transform: Transform2D,
        pivot: Vector2,
        color: crate::structs::Color,
        z_index: i32,
        created_timestamp: u64,
        device: &Device,
        queue: &Queue,
    ) {
        primitive_renderer.queue_text(
            uuid.as_uid32(),
            RenderLayer::World2D,
            text,
            font_size,
            transform,
            pivot,
            color,
            z_index,
            created_timestamp,
            device,
            queue,
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
            RenderLayer::World2D,
            rpass,
            texture_manager,
            device,
            queue,
            camera_bind_group,
            vertex_buffer,
        );
    }
}
