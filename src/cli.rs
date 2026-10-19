use crate::tracer::{Backend, RenderOptions, Settings, Traversal};
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(version, about = "A CPU and native Metal path tracer")]
pub struct Cli {
    #[arg(long, default_value = "worlds/physical_room.json")]
    pub scene: PathBuf,
    #[arg(short, long, default_value = "output/image.png")]
    pub output: PathBuf,
    #[arg(long, value_enum, default_value_t = Backend::Auto)]
    pub backend: Backend,
    #[arg(long, value_enum, default_value_t = Traversal::Auto)]
    pub traversal: Traversal,
    #[arg(long, default_value_t = 200, value_parser = clap::value_parser!(u32).range(1..))]
    pub width: u32,
    #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u32).range(1..))]
    pub height: u32,
    #[arg(long, default_value_t = 5000, value_parser = clap::value_parser!(u32).range(1..))]
    pub samples: u32,
    #[arg(long, default_value_t = 1)]
    pub seed: u32,
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u32).range(..=256))]
    pub max_bounces: u32,
    /// Disable direct light sampling for estimator comparisons.
    #[arg(long)]
    pub no_light_sampling: bool,
    /// Display exposure in stops; EXR always stores raw scene-linear radiance.
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    pub exposure: f64,
    #[arg(long, default_value_t = Settings::DEFAULT_BATCH_SIZE, value_parser = clap::value_parser!(u32).range(1..=1024))]
    pub batch_size: u32,
    #[arg(long, default_value_t = Settings::DEFAULT_THREADGROUP_SIZE)]
    pub threadgroup_size: usize,
    /// Number of timed renders, reusing the device, pipelines, and scene buffers.
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=100))]
    pub benchmark: u32,
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u32).range(..=100))]
    pub warmup: u32,
    /// Write timing and workload metadata as JSON.
    #[arg(long)]
    pub metrics: Option<PathBuf>,
    /// Linear OpenEXR reference for per-run radiance MSE.
    #[arg(long)]
    pub reference: Option<PathBuf>,
    #[arg(long)]
    pub progress: bool,
    #[arg(long)]
    pub list_devices: bool,
}

impl Cli {
    pub fn settings(&self) -> Settings {
        Settings {
            image: RenderOptions {
                width: self.width,
                height: self.height,
                samples_per_pixel: self.samples,
            },
            backend: self.backend,
            traversal: self.traversal,
            seed: self.seed,
            max_bounces: self.max_bounces,
            light_sampling: !self.no_light_sampling,
            batch_size: self.batch_size,
            threadgroup_size: self.threadgroup_size,
            progress: self.progress,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_workloads_and_backend_names() {
        for arguments in [
            vec!["--width", "0"],
            vec!["--samples", "0"],
            vec!["--backend", "vulkan"],
            vec!["--batch-size", "0"],
            vec!["--benchmark", "0"],
            vec!["--max-bounces", "257"],
        ] {
            assert!(Cli::try_parse_from(std::iter::once("rustytracer").chain(arguments)).is_err());
        }
    }

    #[test]
    fn preserves_explicit_backend_and_deterministic_settings() {
        let cli = Cli::try_parse_from([
            "rustytracer",
            "--backend",
            "metal",
            "--traversal",
            "hardware",
            "--seed",
            "42",
            "--width",
            "1920",
            "--height",
            "1080",
        ])
        .unwrap();
        let settings = cli.settings();
        assert_eq!(settings.backend, Backend::Metal);
        assert_eq!(settings.traversal, Traversal::Hardware);
        assert_eq!(settings.seed, 42);
        assert_eq!(settings.image.width, 1920);
    }
}
