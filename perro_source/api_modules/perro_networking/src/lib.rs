#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web_stub;

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;
