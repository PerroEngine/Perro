use crate::App;
use perro_graphics::GraphicsBackend;

#[derive(Default)]
pub struct JoyConInput;

impl JoyConInput {
    pub fn new() -> Self {
        Self
    }

    pub fn begin_frame<B: GraphicsBackend>(&mut self, _app: &mut App<B>) {}
}
