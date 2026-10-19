use super::{RenderOptions, Result};
use image::{ImageFormat, Rgb32FImage, RgbImage};
use std::io;
use std::path::Path;

pub struct Frame {
    pub linear: Vec<[f32; 4]>,
    pub render_seconds: f64,
    pub gpu_seconds: Option<f64>,
}
impl Frame {
    #[cfg(all(test, target_os = "macos", feature = "metal"))]
    pub fn image(&self, options: RenderOptions) -> Result<RgbImage> {
        to_rgb(&self.linear, options)
    }
    pub fn save(
        &self,
        options: RenderOptions,
        path: &Path,
        format: ImageFormat,
        exposure_ev: f64,
    ) -> Result<()> {
        validate(&self.linear, options)?;
        if !exposure_ev.is_finite() || !(-64. ..=64.).contains(&exposure_ev) {
            return Err(
                io::Error::other("Exposure must be finite and between -64 and 64 EV").into(),
            );
        }
        if format == ImageFormat::OpenExr {
            // EXR stores raw scene-linear radiance, independent of display exposure.
            let data = self
                .linear
                .iter()
                .flat_map(|p| p[..3].iter().copied())
                .collect();
            Rgb32FImage::from_raw(options.width, options.height, data)
                .ok_or_else(|| io::Error::other("Invalid HDR buffer"))?
                .save_with_format(path, format)?;
        } else {
            display_image(&self.linear, options, exposure_ev)?.save_with_format(path, format)?;
        }
        Ok(())
    }
}
fn validate(linear: &[[f32; 4]], options: RenderOptions) -> Result<()> {
    if linear.len() as u64 != u64::from(options.width) * u64::from(options.height) {
        return Err(io::Error::other("Invalid rendered image buffer").into());
    }
    if linear
        .iter()
        .flat_map(|p| &p[..3])
        .any(|c| !c.is_finite() || *c < 0.)
    {
        return Err(io::Error::other("Renderer produced negative or non-finite radiance").into());
    }
    Ok(())
}
#[cfg(test)]
pub(super) fn to_rgb(linear: &[[f32; 4]], options: RenderOptions) -> Result<RgbImage> {
    display_image(linear, options, 0.)
}
fn display_image(
    linear: &[[f32; 4]],
    options: RenderOptions,
    exposure_ev: f64,
) -> Result<RgbImage> {
    validate(linear, options)?;
    let gain = exposure_ev.exp2();
    let data = linear
        .iter()
        .flat_map(|p| {
            p[..3].iter().map(|c| {
                (255. * linear_to_srgb((f64::from(*c) * gain).clamp(0., 1.))).round() as u8
            })
        })
        .collect();
    RgbImage::from_raw(options.width, options.height, data)
        .ok_or_else(|| io::Error::other("Invalid rendered image buffer").into())
}
fn linear_to_srgb(value: f64) -> f64 {
    if value <= 0.0031308 {
        12.92 * value
    } else {
        1.055 * value.powf(1. / 2.4) - 0.055
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn srgb_encoding_has_correct_black_white_breakpoint_and_midgray() {
        assert_eq!(linear_to_srgb(0.), 0.);
        assert!((linear_to_srgb(1.) - 1.).abs() < 1e-14);
        assert!((linear_to_srgb(0.0031308) - 0.040449936).abs() < 1e-12);
        assert!((linear_to_srgb(0.18) - 0.4613561295).abs() < 1e-9);
        let image = to_rgb(
            &[[0.18, 1., 4., 0.]],
            RenderOptions {
                width: 1,
                height: 1,
                samples_per_pixel: 1,
            },
        )
        .unwrap();
        assert_eq!(image.get_pixel(0, 0).0, [118, 255, 255]);
    }
    #[test]
    fn invalid_radiance_and_buffer_dimensions_are_errors() {
        let options = RenderOptions {
            width: 1,
            height: 1,
            samples_per_pixel: 1,
        };
        for bad in [-1., f32::NAN, f32::INFINITY] {
            assert!(to_rgb(&[[bad, 0., 0., 0.]], options).is_err());
        }
        assert!(to_rgb(&[], options).is_err());
    }
}
