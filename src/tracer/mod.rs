mod camera;
mod geometry;
mod material;
mod ray;
mod render;
mod scene;
mod world;

use std::error::Error;
use std::fs;
use std::path::Path;
use std::time::Instant;

pub use render::RenderOptions;

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

pub fn generate(path: &str, width: u32, height: u32) -> Result<()> {
    generate_with_options(
        path,
        "worlds/closed_room.json",
        RenderOptions {
            width,
            height,
            ..RenderOptions::default()
        },
    )
}

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
