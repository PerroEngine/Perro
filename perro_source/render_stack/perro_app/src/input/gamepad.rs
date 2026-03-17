use crate::App;
use perro_graphics::GraphicsBackend;

#[derive(Default)]
pub struct GamepadInput;

impl GamepadInput {
    pub fn new() -> Self {
        Self
    }

    pub fn begin_frame<B: GraphicsBackend>(&mut self, _app: &mut App<B>) {}
}
