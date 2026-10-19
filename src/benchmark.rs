use crate::cli::Cli;
use crate::tracer::{Renderer, Result, Settings};
use serde::Serialize;
use std::fs;
use std::io;
use std::path::Path;
use std::time::Instant;

#[derive(Serialize)]
struct Run {
    render_seconds: f64,
    gpu_seconds: Option<f64>,
    encoding_seconds: f64,
    render_and_encoding_seconds: f64,
    samples_per_second: f64,
    radiance_mse: Option<f64>,
}

pub fn run(cli: Cli) -> Result<()> {
    if cli.list_devices {
        println!("{}", crate::tracer::device_description());
        return Ok(());
    }
    let settings = cli.settings();
    settings.validate()?;
    let format = image::ImageFormat::from_path(&cli.output)?;
    if !matches!(
        format,
        image::ImageFormat::Png | image::ImageFormat::Jpeg | image::ImageFormat::OpenExr
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
    if settings.max_bounces != 0 {
        eprintln!(
            "Explicit bounce limit {} truncates transport and introduces bias",
            settings.max_bounces
        );
    }
    create_parent(&cli.output)?;
    let reference = if let Some(path) = &cli.reference {
        if image::ImageFormat::from_path(path)? != image::ImageFormat::OpenExr {
            return Err(io::Error::other("Reference must be a scene-linear OpenEXR image").into());
        }
        let image = image::open(path)?.to_rgb32f();
        if image.dimensions() != (cli.width, cli.height) {
            return Err(
                io::Error::other("Reference resolution does not match render resolution").into(),
            );
        }
        let values: Vec<[f32; 3]> = image.pixels().map(|p| p.0).collect();
        if values.iter().flatten().any(|v| !v.is_finite() || *v < 0.) {
            return Err(io::Error::other("Reference contains invalid radiance").into());
        }
        Some(values)
    } else {
        None
    };
    let mut renderer = Renderer::new(&cli.scene, settings)?;
    eprintln!(
        "{}; traversal {}; setup {:.3}s",
        renderer.device, renderer.traversal, renderer.setup_seconds
    );
    let mut cold_seconds = None;
    for index in 0..cli.warmup {
        let frame = renderer.render(settings)?;
        if index == 0 {
            let run = encode_frame(
                frame,
                settings,
                &cli.output,
                format,
                cli.exposure,
                reference.as_deref(),
            )?;
            cold_seconds = Some(renderer.setup_seconds + run.render_and_encoding_seconds);
        }
    }
    let mut runs = Vec::new();
    for index in 0..cli.benchmark {
        let frame = renderer.render(settings)?;
        let run = encode_frame(
            frame,
            settings,
            &cli.output,
            format,
            cli.exposure,
            reference.as_deref(),
        )?;
        eprintln!(
            "Run {}: render {:.4}s, encode {:.4}s, {:.2} million samples/s",
            index + 1,
            run.render_seconds,
            run.encoding_seconds,
            run.samples_per_second / 1e6
        );
        runs.push(run);
    }
    let mut times: Vec<f64> = runs
        .iter()
        .map(|run| run.render_and_encoding_seconds)
        .collect();
    times.sort_by(f64::total_cmp);
    let median = if times.len().is_multiple_of(2) {
        (times[times.len() / 2 - 1] + times[times.len() / 2]) / 2.0
    } else {
        times[times.len() / 2]
    };
    let p95 = times[((times.len() as f64 * 0.95).ceil() as usize).saturating_sub(1)];
    let (spheres, planes) = renderer.geometry_counts();
    let report = serde_json::json!({
        "integrator": "physical-rgb-v1",
        "display_exposure_ev": cli.exposure,
        "reference": cli.reference,
        "settings": settings,
        "scene": cli.scene,
        "backend": renderer.backend,
        "device": renderer.device,
        "traversal": renderer.traversal,
        "spheres": spheres,
        "planes": planes,
        "setup_seconds": renderer.setup_seconds,
        "cold_end_to_end_seconds": cold_seconds.unwrap_or(renderer.setup_seconds + runs[0].render_and_encoding_seconds),
        "median_render_and_encoding_seconds": median,
        "p95_render_and_encoding_seconds": p95,
        "warmup_runs": cli.warmup,
        "runs": runs,
    });
    if let Some(path) = cli.metrics {
        create_parent(&path)?;
        fs::write(path, serde_json::to_string_pretty(&report)?)?;
    }
    println!(
        "Saved {}; median {:.4}s including encoding",
        cli.output.display(),
        median
    );
    Ok(())
}

fn encode_frame(
    frame: crate::tracer::Frame,
    settings: Settings,
    path: &Path,
    format: image::ImageFormat,
    exposure: f64,
    reference: Option<&[[f32; 3]]>,
) -> Result<Run> {
    let encoding = Instant::now();
    frame.save(settings.image, path, format, exposure)?;
    let encoding_seconds = encoding.elapsed().as_secs_f64();
    let samples = f64::from(settings.image.width)
        * f64::from(settings.image.height)
        * f64::from(settings.image.samples_per_pixel);
    let radiance_mse = reference.map(|reference| {
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

fn create_parent(path: &Path) -> io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}
