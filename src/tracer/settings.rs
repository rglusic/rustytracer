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
            max_bounces: 64,
            batch_size: 32,
            threadgroup_size: 128,
            progress: false,
        }
    }
}

impl Settings {
    pub fn validate(self) -> Result<()> {
        self.image.validate()?;
        if !(1..=256).contains(&self.max_bounces)
            || !(1..=1024).contains(&self.batch_size)
            || !(1..=1024).contains(&self.threadgroup_size)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Bounces must be 1..=256; batch size and threadgroup size must be 1..=1024",
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
