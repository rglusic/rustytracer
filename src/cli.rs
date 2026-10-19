use crate::tracer::{Backend, RenderOptions, Settings, Traversal};
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(version, about = "A CPU and native Metal path tracer")]
pub struct Cli {
    #[arg(long, default_value = "worlds/closed_room.json")]
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
    #[arg(long, default_value_t = 64, value_parser = clap::value_parser!(u32).range(1..=256))]
    pub max_bounces: u32,
    #[arg(long, default_value_t = 32, value_parser = clap::value_parser!(u32).range(1..=1024))]
    pub batch_size: u32,
    #[arg(long, default_value_t = 128)]
    pub threadgroup_size: usize,
    /// Number of timed renders, reusing the device, pipelines, and scene buffers.
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=100))]
    pub benchmark: u32,
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u32).range(..=100))]
    pub warmup: u32,
    /// Write timing and workload metadata as JSON.
    #[arg(long)]
    pub metrics: Option<PathBuf>,
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
            batch_size: self.batch_size,
            threadgroup_size: self.threadgroup_size,
            progress: self.progress,
        }
    }
}
