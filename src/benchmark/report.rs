//! Typed timing report. Field names retain the benchmark tools' JSON contract.
use super::output::create_parent;
use crate::cli::Cli;
use crate::tracer::{Backend, Renderer, Result, Settings};
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Serialize)]
pub(super) struct Run {
    pub render_seconds: f64,
    pub gpu_seconds: Option<f64>,
    pub encoding_seconds: f64,
    pub render_and_encoding_seconds: f64,
    pub samples_per_second: f64,
    pub radiance_mse: Option<f64>,
}

#[derive(Serialize)]
pub(super) struct Report<'a> {
    integrator: &'static str,
    display_exposure_ev: f64,
    reference: Option<&'a Path>,
    settings: Settings,
    scene: &'a Path,
    backend: Backend,
    device: &'a str,
    traversal: &'a str,
    spheres: usize,
    planes: usize,
    setup_seconds: f64,
    cold_end_to_end_seconds: f64,
    pub median_render_and_encoding_seconds: f64,
    p95_render_and_encoding_seconds: f64,
    warmup_runs: u32,
    runs: Vec<Run>,
}

impl<'a> Report<'a> {
    /// CLI validation guarantees at least one timed run.
    pub fn new(
        cli: &'a Cli,
        renderer: &'a Renderer,
        settings: Settings,
        cold_seconds: Option<f64>,
        runs: Vec<Run>,
    ) -> Self {
        let mut times: Vec<f64> = runs
            .iter()
            .map(|run| run.render_and_encoding_seconds)
            .collect();
        times.sort_by(f64::total_cmp);
        let middle = times.len() / 2;
        let median = if times.len().is_multiple_of(2) {
            (times[middle - 1] + times[middle]) / 2.
        } else {
            times[middle]
        };
        let p95 = times[((times.len() as f64 * 0.95).ceil() as usize).saturating_sub(1)];
        let (spheres, planes) = renderer.geometry_counts();
        Self {
            integrator: "physical-rgb-v1",
            display_exposure_ev: cli.exposure,
            reference: cli.reference.as_deref(),
            settings,
            scene: &cli.scene,
            backend: renderer.backend,
            device: &renderer.device,
            traversal: &renderer.traversal,
            spheres,
            planes,
            setup_seconds: renderer.setup_seconds,
            cold_end_to_end_seconds: cold_seconds
                .unwrap_or(renderer.setup_seconds + runs[0].render_and_encoding_seconds),
            median_render_and_encoding_seconds: median,
            p95_render_and_encoding_seconds: p95,
            warmup_runs: cli.warmup,
            runs,
        }
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        create_parent(path)?;
        fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}
