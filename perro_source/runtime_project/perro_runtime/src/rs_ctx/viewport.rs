use super::RuntimeResourceApi;
use perro_resource_context::api::ViewportAPI;
use perro_structs::Vector2;

impl ViewportAPI for RuntimeResourceApi {
    #[inline]
    fn viewport_size(&self) -> Vector2 {
        let (width, height) = *self
            .viewport_size
            .lock()
            .expect("resource api viewport mutex poisoned");
        Vector2::new(width as f32, height as f32)
    }
}
