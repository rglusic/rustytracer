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
    MTLCreateSystemDefaultDevice()
}

pub(super) struct MetalRenderer {
    device: MetalObject<dyn MTLDevice>,
    queue: MetalObject<dyn MTLCommandQueue>,
    pipeline: MetalObject<dyn MTLComputePipelineState>,
    spheres: MetalObject<dyn MTLBuffer>,
    planes: MetalObject<dyn MTLBuffer>,
    accumulation: MetalObject<dyn MTLBuffer>,
    data: SceneData,
}

impl MetalRenderer {
    pub fn new(
        device: MetalObject<dyn MTLDevice>,
        world: &World,
        settings: Settings,
    ) -> Result<Self> {
        autoreleasepool(|_| {
            if settings.traversal == super::Traversal::Hardware {
                return Err(io::Error::other(
                    "Hardware traversal is not enabled in this compute milestone",
                )
                .into());
            }
            if !device.hasUnifiedMemory() {
                return Err(io::Error::other(
                    "This Metal backend requires an Apple unified-memory GPU",
                )
                .into());
            }
            let data = SceneData::new(world, settings)?;
            let source = concat!(
                include_str!("shaders/common.metal"),
                "\n",
                include_str!("shaders/linear.metal")
            );
            let options = MTLCompileOptions::new();
            options.setLanguageVersion(MTLLanguageVersion::Version3_1);
            let library = device
                .newLibraryWithSource_options_error(&NSString::from_str(source), Some(&options))
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
            if byte_count as u64 > device.recommendedMaxWorkingSetSize() / 2 {
                return Err(io::Error::other("Image exceeds the GPU working-memory budget").into());
            }
            let accumulation = device
                .newBufferWithLength_options(byte_count, MTLResourceOptions::StorageModeShared)
                .ok_or_else(|| io::Error::other("Could not allocate Metal accumulation buffer"))?;
            let queue = device
                .newCommandQueue()
                .ok_or_else(|| io::Error::other("Could not create Metal command queue"))?;
            Ok(Self {
                device,
                queue,
                pipeline,
                spheres,
                planes,
                accumulation,
                data,
            })
        })
    }

    pub fn device_name(&self) -> String {
        self.device.name().to_string()
    }
    pub fn traversal_name(&self) -> &'static str {
        "linear"
    }

    pub fn render(&mut self, settings: Settings) -> Result<Frame> {
        autoreleasepool(|_| self.render_inner(settings))
    }

    fn render_inner(&mut self, settings: Settings) -> Result<Frame> {
        let started = Instant::now();
        let count = settings.image.width as usize * settings.image.height as usize;
        if self.accumulation.length() != count * 16 {
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
        for start in (0..settings.image.samples_per_pixel).step_by(settings.batch_size as usize) {
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
            gpu_seconds += command.GPUEndTime() - command.GPUStartTime();
            if settings.progress {
                eprintln!(
                    "Samples: {}/{}",
                    start + parameters.sample_count,
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

fn buffer_from_slice<T: bytemuck::Pod>(
    device: &ProtocolObject<dyn MTLDevice>,
    values: &[T],
) -> Result<MetalObject<dyn MTLBuffer>> {
    let bytes: &[u8] = bytemuck::cast_slice(values);
    if bytes.is_empty() {
        return device
            .newBufferWithLength_options(16, MTLResourceOptions::StorageModeShared)
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
