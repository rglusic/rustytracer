use super::Result;
use super::camera::Camera;
use super::scene::Scene;
use cgmath::Vector3;
use image::RgbImage;
use rand::RngExt;
use std::io;
use std::thread;

#[derive(Clone, Copy, Debug)]
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

pub(super) fn render(scene: &Scene, camera: &Camera, options: RenderOptions) -> Result<RgbImage> {
    let workers = thread::available_parallelism().map_or(1, |count| count.get());
    render_with_workers(scene, camera, options, workers)
}

fn render_with_workers(
    scene: &Scene,
    camera: &Camera,
    options: RenderOptions,
    workers: usize,
) -> Result<RgbImage> {
    options.validate()?;
    if workers == 0 {
        return Err(
            io::Error::new(io::ErrorKind::InvalidInput, "Worker count must be positive").into(),
        );
    }

    let byte_count = u64::from(options.width)
        .checked_mul(u64::from(options.height))
        .and_then(|pixels| pixels.checked_mul(3))
        .and_then(|bytes| usize::try_from(bytes).ok())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Image dimensions are too large",
            )
        })?;
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(byte_count)?;
    pixels.resize(byte_count, 0);

    let pixels_per_worker = (byte_count / 3).div_ceil(workers);
    thread::scope(|scope| -> Result<()> {
        let mut handles = Vec::new();
        // Every worker owns a disjoint slice, including any remainder pixels.
        for (section, buffer) in pixels.chunks_mut(pixels_per_worker * 3).enumerate() {
            handles.push(scope.spawn(move || {
                render_section(buffer, section * pixels_per_worker, scene, camera, options);
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

    RgbImage::from_raw(options.width, options.height, pixels)
        .ok_or_else(|| io::Error::other("Invalid rendered image buffer").into())
}

fn render_section(
    pixels: &mut [u8],
    first_pixel: usize,
    scene: &Scene,
    camera: &Camera,
    options: RenderOptions,
) {
    let mut rng = rand::rng();
    let width = options.width as usize;
    for (offset, pixel) in pixels.chunks_exact_mut(3).enumerate() {
        let index = first_pixel + offset;
        let x = index % width;
        let y = index / width;
        let mut color = Vector3::new(0.0, 0.0, 0.0);
        for _ in 0..options.samples_per_pixel {
            let u = (x as f64 + rng.random::<f64>()) / f64::from(options.width);
            let v = (y as f64 + rng.random::<f64>()) / f64::from(options.height);
            color += scene.render(&camera.get_ray(u, v), 0, f64::MAX);
        }
        color /= f64::from(options.samples_per_pixel);
        pixel.copy_from_slice(&[
            (255.99 * color.x.clamp(0.0, 1.0)) as u8,
            (255.99 * color.y.clamp(0.0, 1.0)) as u8,
            (255.99 * color.z.clamp(0.0, 1.0)) as u8,
        ]);
    }
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
