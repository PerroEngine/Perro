use perro_graphics::GraphicsBackend;
use perro_render_bridge::RenderEvent;
use perro_runtime::Runtime;

pub struct App<B: GraphicsBackend> {
    pub runtime: Runtime,
    pub graphics: B,
    command_buffer: Vec<perro_render_bridge::RenderCommand>,
    event_buffer: Vec<RenderEvent>,
}

impl<B: GraphicsBackend> App<B> {
    pub fn new(runtime: Runtime, graphics: B) -> Self {
        Self {
            runtime,
            graphics,
            command_buffer: Vec::new(),
            event_buffer: Vec::new(),
        }
    }

    pub fn with_empty_runtime(graphics: B) -> Self {
        Self::new(Runtime::new(), graphics)
    }

    pub fn frame(&mut self, delta_time: f32) {
        self.runtime.update(delta_time);

        self.runtime.drain_render_commands(&mut self.command_buffer);
        self.graphics
            .submit_many(self.command_buffer.drain(..));

        self.graphics.drain_events(&mut self.event_buffer);
        self.runtime
            .apply_render_events(self.event_buffer.drain(..));

        self.graphics.draw_frame();
    }
}

pub mod entry;
pub mod winit_runner;
