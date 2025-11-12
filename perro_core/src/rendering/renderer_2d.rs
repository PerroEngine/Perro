use wgpu::{Device, Queue, RenderPass};
use crate::{
    ui_elements::ui_container::CornerRadius,
    structs2d::{Transform2D, Vector2},
    rendering::{PrimitiveRenderer, RenderLayer, TextureManager},
};

pub struct Renderer2D {
}

impl Renderer2D {
    pub fn new() -> Self {
        Self {}
    }

    pub fn queue_rect(&mut self, 
                      primitive_renderer: &mut PrimitiveRenderer,
                      uuid: uuid::Uuid, 
                      transform: Transform2D,
                      size: Vector2,
                      pivot: Vector2,
                      color: crate::structs2d::Color,
                      corner_radius: Option<CornerRadius>,
                      border_thickness: f32,
                      is_border: bool,
                      z_index: i32) {
        primitive_renderer.queue_rect(
            uuid, 
            RenderLayer::World2D, 
            transform, size, pivot, color, corner_radius, border_thickness, is_border, z_index
        );
    }

    pub fn queue_texture(&mut self, 
                        primitive_renderer: &mut PrimitiveRenderer,
                        uuid: uuid::Uuid,
                        texture_path: &str,
                        transform: Transform2D,
                        pivot: Vector2,
                        z_index: i32) {
        primitive_renderer.queue_texture(
            uuid, 
            RenderLayer::World2D, 
            texture_path, transform, pivot, z_index
        );
    }

    pub fn queue_text(&mut self,
                      primitive_renderer: &mut PrimitiveRenderer,
                      uuid: uuid::Uuid,
                      text: &str,
                      font_size: f32,
                      transform: Transform2D,
                      pivot: Vector2,
                      color: crate::structs2d::Color,
                      z_index: i32) {
        primitive_renderer.queue_text(
            uuid,
            RenderLayer::World2D,
            text,
            font_size,
            transform,
            pivot,
            color,
            z_index
        );
    }

    pub fn render(&mut self,
                  primitive_renderer: &mut PrimitiveRenderer,
                  rpass: &mut RenderPass<'_>, 
                  texture_manager: &mut TextureManager,
                  device: &Device,
                  queue: &Queue,
                  camera_bind_group: &wgpu::BindGroup,
                  vertex_buffer: &wgpu::Buffer) {
        
        primitive_renderer.render_layer(
            RenderLayer::World2D, rpass, texture_manager, 
            device, queue, camera_bind_group, vertex_buffer
        );
    }
}