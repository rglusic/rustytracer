//! Native Metal backend. Scene buffers stay resident across synchronous renders.
mod acceleration;
mod buffers;
mod data;
mod dispatch;
mod pipeline;

use super::frame::Frame;
use super::world::World;
use super::{Result, Settings};
use buffers::Buffers;
use data::SceneData;
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::ProtocolObject;
use objc2_metal::{MTLCommandQueue, MTLCreateSystemDefaultDevice, MTLDevice};
use pipeline::Pipeline;
use std::io;
use std::time::Instant;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {}

type MetalObject<T> = Retained<ProtocolObject<T>>;

pub(super) fn default_device() -> Option<MetalObject<dyn MTLDevice>> {
    if !objc2::available!(macos = 15.0) {
        return None;
    }
    MTLCreateSystemDefaultDevice().filter(|device| device.hasUnifiedMemory())
}

pub(super) struct MetalRenderer {
    device: MetalObject<dyn MTLDevice>,
    queue: MetalObject<dyn MTLCommandQueue>,
    pipeline: Pipeline,
    buffers: Buffers,
    data: SceneData,
    #[cfg(test)]
    last_dispatches: usize,
}

impl MetalRenderer {
    pub fn new(
        device: MetalObject<dyn MTLDevice>,
        world: &World,
        settings: Settings,
    ) -> Result<Self> {
        settings.validate()?;
        autoreleasepool(|_| {
            if !device.hasUnifiedMemory() {
                return Err(io::Error::other(
                    "This Metal backend requires an Apple unified-memory GPU",
                )
                .into());
            }
            let data = SceneData::new(world, settings)?;
            let queue = device
                .newCommandQueue()
                .ok_or_else(|| io::Error::other("Could not create Metal command queue"))?;
            let buffers = Buffers::new(&device, &data)?;
            let pipeline =
                Pipeline::new(&device, &queue, &data, &buffers.spheres, settings.traversal)?;
            Ok(Self {
                device,
                queue,
                pipeline,
                buffers,
                data,
                #[cfg(test)]
                last_dispatches: 0,
            })
        })
    }

    pub fn device_name(&self) -> String {
        self.device.name().to_string()
    }

    pub fn traversal_name(&self) -> &'static str {
        if self.pipeline.hardware.is_some() {
            "hardware"
        } else {
            "linear"
        }
    }

    pub fn render(&mut self, settings: Settings) -> Result<Frame> {
        settings.validate()?;
        autoreleasepool(|_| self.render_inner(settings))
    }

    fn render_inner(&mut self, settings: Settings) -> Result<Frame> {
        let started = Instant::now();
        #[cfg(test)]
        {
            self.last_dispatches = 0;
        }
        if self.data.parameters.width != settings.image.width
            || self.data.parameters.height != settings.image.height
        {
            return Err(
                io::Error::other("Renderer resolution cannot change after initialization").into(),
            );
        }
        self.pipeline.validate_threads(settings.threadgroup_size)?;
        let mut gpu_seconds = 0.;
        let mut reset = 1;
        for start in (0..settings.image.samples_per_pixel).step_by(settings.batch_size as usize) {
            let mut parameters = self.data.parameters;
            parameters.sample_start = start;
            parameters.sample_count = settings
                .batch_size
                .min(settings.image.samples_per_pixel - start);
            parameters.seed = settings.seed;
            parameters.max_bounces = settings.max_bounces;
            parameters.light_sampling = settings.light_sampling.into();
            loop {
                parameters.reset = reset;
                #[cfg(test)]
                {
                    self.last_dispatches += 1;
                }
                gpu_seconds += self.dispatch(&parameters, settings.threadgroup_size)?;
                reset = 0;
                // dispatch waits for completion before the CPU reads shared storage.
                if self.buffers.unfinished_count() == 0 {
                    break;
                }
            }
            if settings.progress {
                eprintln!(
                    "Samples: {}/{}",
                    (start + settings.batch_size).min(settings.image.samples_per_pixel),
                    settings.image.samples_per_pixel
                );
            }
        }
        let linear = self.buffers.download(settings.image.samples_per_pixel);
        Ok(Frame {
            linear,
            render_seconds: started.elapsed().as_secs_f64(),
            gpu_seconds: Some(gpu_seconds),
        })
    }
}

#[cfg(test)]
mod tests;
