//! Application workflow: prepare once, warm up, measure, and save a report.
mod output;
mod report;

use crate::cli::Cli;
use crate::tracer::{Renderer, Result};
use output::Output;
use report::Report;

pub fn run(cli: Cli) -> Result<()> {
    if cli.list_devices {
        println!("{}", crate::tracer::device_description());
        return Ok(());
    }
    let settings = cli.settings();
    settings.validate()?;
    // Reject invalid output or reference images before device setup or rendering.
    let output = Output::new(&cli)?;
    if settings.max_bounces != 0 {
        eprintln!(
            "Explicit bounce limit {} truncates transport and introduces bias",
            settings.max_bounces
        );
    }
    let mut renderer = Renderer::new(&cli.scene, settings)?;
    eprintln!(
        "{}; traversal {}; setup {:.3}s",
        renderer.device, renderer.traversal, renderer.setup_seconds
    );
    let mut cold_seconds = None;
    for index in 0..cli.warmup {
        let frame = renderer.render(settings)?;
        if index == 0 {
            let run = output.encode(frame, settings.image)?;
            cold_seconds = Some(renderer.setup_seconds + run.render_and_encoding_seconds);
        }
    }
    let mut runs = Vec::new();
    for index in 0..cli.benchmark {
        let frame = renderer.render(settings)?;
        let run = output.encode(frame, settings.image)?;
        eprintln!(
            "Run {}: render {:.4}s, encode {:.4}s, {:.2} million samples/s",
            index + 1,
            run.render_seconds,
            run.encoding_seconds,
            run.samples_per_second / 1e6
        );
        runs.push(run);
    }
    let report = Report::new(&cli, &renderer, settings, cold_seconds, runs);
    if let Some(path) = &cli.metrics {
        report.write(path)?;
    }
    println!(
        "Saved {}; median {:.4}s including encoding",
        cli.output.display(),
        report.median_render_and_encoding_seconds
    );
    Ok(())
}

#[cfg(test)]
mod tests;
