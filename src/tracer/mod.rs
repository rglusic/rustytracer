mod camera;
mod frame;
mod geometry;
mod light_sampling;
mod material;
mod material_input;
mod media;
#[cfg(test)]
mod physics_tests;
mod ray;
mod render;
mod renderer;
mod sampler;
mod scene;
mod settings;
mod world;

use std::error::Error;

#[cfg(all(target_os = "macos", feature = "metal"))]
use objc2_metal::MTLDevice;

pub use frame::Frame;
pub use render::RenderOptions;
pub use renderer::Renderer;
pub use settings::{Backend, Settings, Traversal};

#[cfg(all(target_os = "macos", feature = "metal"))]
mod metal;

pub fn device_description() -> String {
    #[cfg(all(target_os = "macos", feature = "metal"))]
    if let Some(device) = metal::default_device() {
        return format!(
            "{}; unified memory: {}; ray tracing: {}",
            device.name(),
            device.hasUnifiedMemory(),
            device.supportsRaytracing()
        );
    }
    "CPU available; no Metal device or backend available".to_owned()
}

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
