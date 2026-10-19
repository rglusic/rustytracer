mod camera;
mod frame;
mod geometry;
mod material;
mod ray;
mod render;
mod renderer;
mod sampler;
mod scene;
mod settings;
mod world;

use std::error::Error;
#[cfg(test)]
use std::fs;
#[cfg(test)]
use std::path::Path;
#[cfg(test)]
use std::time::Instant;

#[cfg(all(target_os = "macos", feature = "metal"))]
use objc2_metal::MTLDevice;

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

#[cfg(test)]
pub fn generate_with_options(
    path: impl AsRef<Path>,
    world_path: impl AsRef<Path>,
    options: RenderOptions,
) -> Result<()> {
    options.validate()?;
    let started = Instant::now();
    let world = world::World::new(world_path, options.width, options.height)?;
    let scene = scene::Scene::new(world.get_hitables());
    let camera = world.get_camera();
    let path = path.as_ref();

    // Report unsupported output formats before starting a potentially long render.
    let format = image::ImageFormat::from_path(path)?;
    if !matches!(format, image::ImageFormat::Png | image::ImageFormat::Jpeg) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Output must be a PNG or JPEG image",
        )
        .into());
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    println!(
        "Rendering {} x {} at {} samples per pixel",
        options.width, options.height, options.samples_per_pixel
    );
    let image = render::render(&scene, &camera, options)?;
    image.save_with_format(path, format)?;
    println!(
        "Saved {} in {:.2} seconds",
        path.display(),
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

#[cfg(test)]
mod tests;
