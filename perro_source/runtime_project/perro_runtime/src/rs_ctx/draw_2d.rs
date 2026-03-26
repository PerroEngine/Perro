use super::RuntimeResourceApi;
use perro_render_bridge::{Command2D, DrawShape2DCommand, RenderCommand};
use perro_resource_context::sub_apis::Draw2DAPI;
use perro_structs::{DrawShape2D, Vector2};

impl Draw2DAPI for RuntimeResourceApi {
    fn draw_2d_shape(&self, shape: DrawShape2D, position: Vector2) {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state
            .queued_commands
            .push(RenderCommand::TwoD(Command2D::DrawShape {
                draw: DrawShape2DCommand {
                    shape,
                    position: [position.x, position.y],
                },
            }));
    }
}
