//! Output validation, image encoding, and reference comparison outside render timers.
use super::report::Run;
use crate::cli::Cli;
use crate::tracer::{Frame, RenderOptions, Result};
use image::ImageFormat;
use std::fs;
use std::io;
use std::path::Path;
use std::time::Instant;

pub(super) struct Output<'a> {
    path: &'a Path,
    format: ImageFormat,
    exposure: f64,
    reference: Option<Vec<[f32; 3]>>,
}

impl<'a> Output<'a> {
    pub fn new(cli: &'a Cli) -> Result<Self> {
        let format = ImageFormat::from_path(&cli.output)?;
        if !matches!(
            format,
            ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::OpenExr
        ) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Output must be PNG, JPEG or OpenEXR",
            )
            .into());
        }
        if !cli.exposure.is_finite() || !(-64. ..=64.).contains(&cli.exposure) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Exposure must be finite and between -64 and 64 EV",
            )
            .into());
        }
        create_parent(&cli.output)?;
        let reference = cli
            .reference
            .as_deref()
            .map(|path| load_reference(path, (cli.width, cli.height)))
            .transpose()?;
        Ok(Self {
            path: &cli.output,
            format,
            exposure: cli.exposure,
            reference,
        })
    }

    pub fn encode(&self, frame: Frame, options: RenderOptions) -> Result<Run> {
        let encoding = Instant::now();
        frame.save(options, self.path, self.format, self.exposure)?;
        let encoding_seconds = encoding.elapsed().as_secs_f64();
        let samples = f64::from(options.width)
            * f64::from(options.height)
            * f64::from(options.samples_per_pixel);
        // Compare raw radiance after encoding has been timed; exposure never affects MSE.
        let radiance_mse = self.reference.as_deref().map(|reference| {
            frame
                .linear
                .iter()
                .zip(reference)
                .flat_map(|(a, b)| (0..3).map(move |i| (f64::from(a[i]) - f64::from(b[i])).powi(2)))
                .sum::<f64>()
                / (frame.linear.len() * 3) as f64
        });
        Ok(Run {
            render_seconds: frame.render_seconds,
            gpu_seconds: frame.gpu_seconds,
            encoding_seconds,
            render_and_encoding_seconds: frame.render_seconds + encoding_seconds,
            samples_per_second: samples / frame.render_seconds,
            radiance_mse,
        })
    }
}

fn load_reference(path: &Path, resolution: (u32, u32)) -> Result<Vec<[f32; 3]>> {
    if ImageFormat::from_path(path)? != ImageFormat::OpenExr {
        return Err(io::Error::other("Reference must be a scene-linear OpenEXR image").into());
    }
    let image = image::open(path)?.to_rgb32f();
    if image.dimensions() != resolution {
        return Err(
            io::Error::other("Reference resolution does not match render resolution").into(),
        );
    }
    let values: Vec<[f32; 3]> = image.pixels().map(|p| p.0).collect();
    if values.iter().flatten().any(|v| !v.is_finite() || *v < 0.) {
        return Err(io::Error::other("Reference contains invalid radiance").into());
    }
    Ok(values)
}

pub(super) fn create_parent(path: &Path) -> io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}
