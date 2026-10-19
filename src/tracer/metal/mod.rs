mod acceleration;
mod data;

use super::frame::Frame;
use super::world::World;
use super::{Result, Settings};
use data::SceneData;
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
use objc2_metal::*;
use std::io;
use std::ptr::NonNull;
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
    pipeline: MetalObject<dyn MTLComputePipelineState>,
    spheres: MetalObject<dyn MTLBuffer>,
    planes: MetalObject<dyn MTLBuffer>,
    accumulation: MetalObject<dyn MTLBuffer>,
    paths: MetalObject<dyn MTLBuffer>,
    unfinished: MetalObject<dyn MTLBuffer>,
    lights: MetalObject<dyn MTLBuffer>,
    data: SceneData,
    hardware: Option<acceleration::Hardware>,
    #[cfg(test)]
    library: MetalObject<dyn MTLLibrary>,
}

impl MetalRenderer {
    pub fn new(
        device: MetalObject<dyn MTLDevice>,
        world: &World,
        settings: Settings,
    ) -> Result<Self> {
        autoreleasepool(|_| {
            if !device.hasUnifiedMemory() {
                return Err(io::Error::other(
                    "This Metal backend requires an Apple unified-memory GPU",
                )
                .into());
            }
            let data = SceneData::new(world, settings)?;
            let source = String::from(concat!(
                include_str!("shaders/common.metal"),
                "\n",
                include_str!("shaders/bsdf.metal"),
                "\n",
                include_str!("shaders/integrator.metal"),
                "\n",
                include_str!("shaders/linear.metal"),
                "\n",
                include_str!("shaders/hardware.metal")
            ));
            #[cfg(test)]
            let source = format!("{source}\n{}", include_str!("shaders/probes.metal"));
            let options = MTLCompileOptions::new();
            options.setLanguageVersion(MTLLanguageVersion::Version3_1);
            options.setMathMode(MTLMathMode::Safe);
            let library = device
                .newLibraryWithSource_options_error(&NSString::from_str(&source), Some(&options))
                .map_err(|error| {
                    io::Error::other(format!("Metal shader compilation failed: {error}"))
                })?;
            let function = library
                .newFunctionWithName(&NSString::from_str("trace_linear"))
                .ok_or_else(|| io::Error::other("Metal trace_linear function is missing"))?;
            let pipeline = device
                .newComputePipelineStateWithFunction_error(&function)
                .map_err(|error| {
                    io::Error::other(format!("Metal pipeline creation failed: {error}"))
                })?;
            let spheres = buffer_from_slice(&device, &data.spheres)?;
            let planes = buffer_from_slice(&device, &data.planes)?;
            let byte_count = settings.image.width as usize * settings.image.height as usize * 16;
            if (byte_count as u64) * 9 > device.recommendedMaxWorkingSetSize() / 2 {
                return Err(io::Error::other("Image exceeds the GPU working-memory budget").into());
            }
            let accumulation = device
                .newBufferWithLength_options(byte_count, MTLResourceOptions::StorageModeShared)
                .ok_or_else(|| io::Error::other("Could not allocate Metal accumulation buffer"))?;
            let paths = device
                .newBufferWithLength_options(
                    (byte_count / 16) * size_of::<data::PathState>(),
                    MTLResourceOptions::StorageModeShared,
                )
                .ok_or_else(|| io::Error::other("Could not allocate Metal path continuations"))?;
            let unfinished = buffer_from_slice(&device, &[0u32])?;
            let lights = buffer_from_slice(&device, &data.lights)?;
            let queue = device
                .newCommandQueue()
                .ok_or_else(|| io::Error::other("Could not create Metal command queue"))?;
            let supports_hardware =
                device.supportsRaytracing() && device.supportsFamily(MTLGPUFamily::Apple9);
            let use_hardware = match settings.traversal {
                super::Traversal::Hardware => {
                    if !supports_hardware || data.spheres.is_empty() {
                        return Err(io::Error::other("Hardware traversal requires an M3 or newer GPU and at least one sphere").into());
                    }
                    true
                }
                super::Traversal::Auto => supports_hardware && data.spheres.len() >= 64,
                super::Traversal::Linear => false,
            };
            let hardware = if use_hardware {
                Some(acceleration::Hardware::new(
                    &device,
                    &queue,
                    &library,
                    &data.spheres,
                    &spheres,
                    "trace_hardware",
                )?)
            } else {
                None
            };
            let pipeline = hardware
                .as_ref()
                .map_or(pipeline, |hardware| hardware.pipeline.clone());
            Ok(Self {
                device,
                queue,
                pipeline,
                spheres,
                planes,
                accumulation,
                paths,
                unfinished,
                lights,
                data,
                hardware,
                #[cfg(test)]
                library,
            })
        })
    }

    pub fn device_name(&self) -> String {
        self.device.name().to_string()
    }
    pub fn traversal_name(&self) -> &'static str {
        if self.hardware.is_some() {
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
        let count = settings.image.width as usize * settings.image.height as usize;
        if self.data.parameters.width != settings.image.width
            || self.data.parameters.height != settings.image.height
        {
            return Err(
                io::Error::other("Renderer resolution cannot change after initialization").into(),
            );
        }
        let maximum = self.pipeline.maxTotalThreadsPerThreadgroup();
        let execution_width = self.pipeline.threadExecutionWidth();
        if settings.threadgroup_size > maximum
            || !settings.threadgroup_size.is_multiple_of(execution_width)
        {
            return Err(io::Error::other(format!(
                "Threadgroup size must be a multiple of {execution_width} and at most {maximum}"
            ))
            .into());
        }
        let mut gpu_seconds = 0.0;
        let mut reset = 1;
        for start in (0..settings.image.samples_per_pixel).step_by(settings.batch_size as usize) {
            loop {
                // SAFETY: shared counter has one u32, and the previous command completed.
                unsafe {
                    self.unfinished.contents().as_ptr().cast::<u32>().write(0);
                }
                let command = self
                    .queue
                    .commandBuffer()
                    .ok_or_else(|| io::Error::other("Could not create Metal command buffer"))?;
                let encoder = command
                    .computeCommandEncoder()
                    .ok_or_else(|| io::Error::other("Could not create Metal compute encoder"))?;
                let mut parameters = self.data.parameters;
                parameters.sample_start = start;
                parameters.sample_count = settings
                    .batch_size
                    .min(settings.image.samples_per_pixel - start);
                parameters.seed = settings.seed;
                parameters.max_bounces = settings.max_bounces;
                parameters.reset = reset;
                parameters.light_sampling = settings.light_sampling.into();
                encoder.setComputePipelineState(&self.pipeline);
                // SAFETY: POD layouts match the shader's static assertions. All indices
                // are fixed, buffers are large enough and retained until GPU completion.
                unsafe {
                    encoder.setBytes_length_atIndex(
                        NonNull::from(&parameters).cast(),
                        size_of_val(&parameters),
                        0,
                    );
                    encoder.setBuffer_offset_atIndex(Some(&self.spheres), 0, 1);
                    encoder.setBuffer_offset_atIndex(Some(&self.planes), 0, 2);
                    encoder.setBuffer_offset_atIndex(Some(&self.accumulation), 0, 3);
                    encoder.setBuffer_offset_atIndex(Some(&self.paths), 0, 6);
                    encoder.setBuffer_offset_atIndex(Some(&self.unfinished), 0, 7);
                    encoder.setBuffer_offset_atIndex(Some(&self.lights), 0, 8);
                    if let Some(hardware) = &self.hardware {
                        encoder.setAccelerationStructure_atBufferIndex(
                            Some(&hardware.acceleration),
                            4,
                        );
                        encoder
                            .setIntersectionFunctionTable_atBufferIndex(Some(&hardware.table), 5);
                        encoder.useResource_usage((*self.spheres).as_ref(), MTLResourceUsage::Read);
                    }
                    encoder.dispatchThreads_threadsPerThreadgroup(
                        MTLSize {
                            width: count,
                            height: 1,
                            depth: 1,
                        },
                        MTLSize {
                            width: settings.threadgroup_size,
                            height: 1,
                            depth: 1,
                        },
                    );
                }
                encoder.endEncoding();
                command.commit();
                command.waitUntilCompleted();
                if let Some(error) = command.error() {
                    return Err(io::Error::other(format!("Metal command failed: {error}")).into());
                }
                if command.status() != MTLCommandBufferStatus::Completed {
                    return Err(
                        io::Error::other("Metal command did not complete successfully").into(),
                    );
                }
                gpu_seconds += command.GPUEndTime() - command.GPUStartTime();
                reset = 0;
                // SAFETY: completed GPU command wrote the initialized shared counter.
                let remaining = unsafe { self.unfinished.contents().as_ptr().cast::<u32>().read() };
                if remaining == 0 {
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
        // SAFETY: shared storage is CPU-readable; every command has completed.
        // The buffer has exactly count float4 elements and the GPU initialized all pixels.
        let values = unsafe {
            std::slice::from_raw_parts(
                self.accumulation.contents().as_ptr().cast::<[f32; 4]>(),
                count,
            )
        };
        let inverse_samples = 1.0 / settings.image.samples_per_pixel as f32;
        let linear = values
            .iter()
            .map(|pixel| {
                [
                    pixel[0] * inverse_samples,
                    pixel[1] * inverse_samples,
                    pixel[2] * inverse_samples,
                    0.0,
                ]
            })
            .collect();
        Ok(Frame {
            linear,
            render_seconds: started.elapsed().as_secs_f64(),
            gpu_seconds: Some(gpu_seconds),
        })
    }
}

#[cfg(test)]
mod tests;

fn buffer_from_slice<T: bytemuck::Pod>(
    device: &ProtocolObject<dyn MTLDevice>,
    values: &[T],
) -> Result<MetalObject<dyn MTLBuffer>> {
    let bytes: &[u8] = bytemuck::cast_slice(values);
    if bytes.is_empty() {
        return device
            .newBufferWithLength_options(
                size_of::<T>().max(16),
                MTLResourceOptions::StorageModeShared,
            )
            .ok_or_else(|| io::Error::other("Could not allocate empty Metal buffer").into());
    }
    // SAFETY: Metal copies the valid POD slice before this call returns.
    unsafe {
        device.newBufferWithBytes_length_options(
            NonNull::new(bytes.as_ptr().cast_mut().cast()).unwrap(),
            bytes.len(),
            MTLResourceOptions::StorageModeShared,
        )
    }
    .ok_or_else(|| io::Error::other("Could not allocate Metal scene buffer").into())
}
