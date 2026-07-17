use super::RuntimeResourceApi;
use perro_resource_api::api::ViewportAPI;
use perro_structs::Vector2;

impl ViewportAPI for RuntimeResourceApi {
    #[inline]
    fn viewport_size(&self) -> Vector2 {
        let (width, height) = RuntimeResourceApi::viewport_size(self);
        Vector2::new(width as f32, height as f32)
    }
}
