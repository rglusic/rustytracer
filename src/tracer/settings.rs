use super::{RenderOptions, Result};
use clap::ValueEnum;
use serde::Serialize;
use std::io;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Backend {
    #[default]
    Auto,
    Cpu,
    Metal,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Traversal {
    #[default]
    Auto,
    Linear,
    Hardware,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Settings {
    pub image: RenderOptions,
    pub backend: Backend,
    pub traversal: Traversal,
    pub seed: u32,
    pub max_bounces: u32,
    pub light_sampling: bool,
    pub batch_size: u32,
    pub threadgroup_size: usize,
    pub progress: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            image: RenderOptions::default(),
            backend: Backend::Auto,
            traversal: Traversal::Auto,
            seed: 1,
            max_bounces: 0,
            light_sampling: true,
            batch_size: Self::DEFAULT_BATCH_SIZE,
            threadgroup_size: Self::DEFAULT_THREADGROUP_SIZE,
            progress: false,
        }
    }
}

impl Settings {
    /// Measured across room, dense hardware traversal, and long-path cavity workloads.
    pub const DEFAULT_BATCH_SIZE: u32 = 64;
    pub const DEFAULT_THREADGROUP_SIZE: usize = 64;

    pub fn validate(self) -> Result<()> {
        self.image.validate()?;
        if self.max_bounces > 256
            || !(1..=1024).contains(&self.batch_size)
            || !(1..=1024).contains(&self.threadgroup_size)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Bounce limit must be 0 (unlimited) or 1..=256; batch size and threadgroup size must be 1..=1024",
            )
            .into());
        }
        if self.image.width.checked_mul(self.image.height).is_none() {
            return Err(
                io::Error::new(io::ErrorKind::InvalidInput, "Image has too many pixels").into(),
            );
        }
        Ok(())
    }
}
