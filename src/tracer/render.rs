use super::Result;
use super::camera::Camera;
use super::sampler::Sampler;
use super::scene::Scene;
use super::settings::Settings;
use cgmath::Vector3;
#[cfg(test)]
use image::RgbImage;
use serde::Serialize;
use std::io;
use std::thread;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct RenderOptions {
    pub width: u32,
    pub height: u32,
    pub samples_per_pixel: u32,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            width: 200,
            height: 100,
            samples_per_pixel: 5000,
        }
    }
}

impl RenderOptions {
    pub fn validate(self) -> Result<()> {
        if self.width == 0 || self.height == 0 || self.samples_per_pixel == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Image dimensions and samples per pixel must be greater than zero",
            )
            .into());
        }
        Ok(())
    }
}

#[cfg(test)]
pub(super) fn render(scene: &Scene, camera: &Camera, options: RenderOptions) -> Result<RgbImage> {
    let workers = thread::available_parallelism().map_or(1, |count| count.get());
    render_with_workers(scene, camera, options, workers)
}

#[cfg(test)]
fn render_with_workers(
    scene: &Scene,
    camera: &Camera,
    options: RenderOptions,
    workers: usize,
) -> Result<RgbImage> {
    let settings = Settings {
        image: options,
        ..Settings::default()
    };
    let linear = render_linear(scene, camera, settings, workers)?;
    super::frame::to_rgb(&linear, options)
}

pub(super) fn render_linear(
    scene: &Scene,
    camera: &Camera,
    settings: Settings,
    workers: usize,
) -> Result<Vec<[f32; 4]>> {
    let options = settings.image;
    options.validate()?;
    if workers == 0 {
        return Err(
            io::Error::new(io::ErrorKind::InvalidInput, "Worker count must be positive").into(),
        );
    }

    let byte_count = u64::from(options.width)
        .checked_mul(u64::from(options.height))
        .and_then(|pixels| pixels.checked_mul(16))
        .and_then(|bytes| usize::try_from(bytes).ok())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Image dimensions are too large",
            )
        })?;
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(byte_count / 16)?;
    pixels.resize(byte_count / 16, [0.0; 4]);

    let pixels_per_worker = pixels.len().div_ceil(workers);
    thread::scope(|scope| -> Result<()> {
        let mut handles = Vec::new();
        // Every worker owns a disjoint slice, including any remainder pixels.
        for (section, buffer) in pixels.chunks_mut(pixels_per_worker).enumerate() {
            handles.push(scope.spawn(move || {
                render_section(buffer, section * pixels_per_worker, scene, camera, settings);
            }));
        }

        // Join every worker before reporting a failure, so a panic cannot leave
        // the caller waiting for a message that will never arrive.
        let mut failed = false;
        for handle in handles {
            failed |= handle.join().is_err();
        }
        if failed {
            return Err(io::Error::other("A render worker panicked").into());
        }
        Ok(())
    })?;

    Ok(pixels)
}

fn render_section(
    pixels: &mut [[f32; 4]],
    first_pixel: usize,
    scene: &Scene,
    camera: &Camera,
    settings: Settings,
) {
    let options = settings.image;
    let width = options.width as usize;
    for (offset, pixel) in pixels.iter_mut().enumerate() {
        let index = first_pixel + offset;
        let x = index % width;
        let y = index / width;
        let mut color = Vector3::new(0.0, 0.0, 0.0);
        for sample in 0..options.samples_per_pixel {
            let mut rng = Sampler::new(settings.seed, index as u32, sample);
            let u = (x as f64 + rng.next()) / f64::from(options.width);
            let v = (y as f64 + rng.next()) / f64::from(options.height);
            color += scene.render(&camera.get_ray(u, v), settings, &mut rng);
        }
        color /= f64::from(options.samples_per_pixel);
        *pixel = [color.x as f32, color.y as f32, color.z as f32, 0.0];
    }
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
