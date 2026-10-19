use super::{RenderOptions, Result};
use image::RgbImage;
use std::io;

pub struct Frame {
    pub linear: Vec<[f32; 4]>,
    pub render_seconds: f64,
    pub gpu_seconds: Option<f64>,
}

impl Frame {
    pub fn image(&self, options: RenderOptions) -> Result<RgbImage> {
        to_rgb(&self.linear, options)
    }
}

pub(super) fn to_rgb(linear: &[[f32; 4]], options: RenderOptions) -> Result<RgbImage> {
    let mut rgb = Vec::with_capacity(linear.len() * 3);
    for pixel in linear {
        for channel in &pixel[..3] {
            if !channel.is_finite() {
                return Err(io::Error::other("Renderer produced a non-finite color").into());
            }
            rgb.push((255.99 * channel.clamp(0.0, 1.0)) as u8);
        }
    }
    RgbImage::from_raw(options.width, options.height, rgb)
        .ok_or_else(|| io::Error::other("Invalid rendered image buffer").into())
}
