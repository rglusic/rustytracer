use crate::cli::Cli;
use crate::tracer::{Renderer, Result};
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
}

pub fn run(cli: Cli) -> Result<()> {
    if cli.list_devices {
        println!("{}", crate::tracer::device_description());
        return Ok(());
    }
    let settings = cli.settings();
    settings.validate()?;
    let format = image::ImageFormat::from_path(&cli.output)?;
    if !matches!(format, image::ImageFormat::Png | image::ImageFormat::Jpeg) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Output must be a PNG or JPEG image",
        )
        .into());
    }
    create_parent(&cli.output)?;
    let mut renderer = Renderer::new(&cli.scene, settings)?;
    eprintln!(
        "{}; traversal {}; setup {:.3}s",
        renderer.device, renderer.traversal, renderer.setup_seconds
    );
    for _ in 0..cli.warmup {
        renderer.render(settings)?;
    }
    let mut runs = Vec::new();
    for index in 0..cli.benchmark {
        let frame = renderer.render(settings)?;
        let encoding = Instant::now();
        frame
            .image(settings.image)?
            .save_with_format(&cli.output, format)?;
        let encoding_seconds = encoding.elapsed().as_secs_f64();
        let samples = f64::from(cli.width) * f64::from(cli.height) * f64::from(cli.samples);
        let run = Run {
            render_seconds: frame.render_seconds,
            gpu_seconds: frame.gpu_seconds,
            encoding_seconds,
            render_and_encoding_seconds: frame.render_seconds + encoding_seconds,
            samples_per_second: samples / frame.render_seconds,
        };
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
        "settings": settings,
        "scene": cli.scene,
        "backend": renderer.backend,
        "device": renderer.device,
        "traversal": renderer.traversal,
        "spheres": spheres,
        "planes": planes,
        "setup_seconds": renderer.setup_seconds,
        "cold_end_to_end_seconds": renderer.setup_seconds + runs[0].render_and_encoding_seconds,
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

fn create_parent(path: &Path) -> io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}
