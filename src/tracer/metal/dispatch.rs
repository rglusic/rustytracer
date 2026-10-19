//! A dispatch retains its bindings until completion before any CPU buffer access.
use super::MetalRenderer;
use super::data::Parameters;
use crate::tracer::Result;
use objc2_metal::*;
use std::io;
use std::ptr::NonNull;

impl MetalRenderer {
    pub(super) fn dispatch(&self, parameters: &Parameters, threads: usize) -> Result<f64> {
        self.buffers.reset_unfinished();
        let command = self
            .queue
            .commandBuffer()
            .ok_or_else(|| io::Error::other("Could not create Metal command buffer"))?;
        let encoder = command
            .computeCommandEncoder()
            .ok_or_else(|| io::Error::other("Could not create Metal compute encoder"))?;
        encoder.setComputePipelineState(&self.pipeline.state);
        // SAFETY: POD layouts match shader assertions, binding indices match the kernels,
        // and the owning renderer retains every buffer until this command completes.
        unsafe {
            encoder.setBytes_length_atIndex(
                NonNull::from(parameters).cast(),
                size_of_val(parameters),
                0,
            );
            encoder.setBuffer_offset_atIndex(Some(&self.buffers.spheres), 0, 1);
            encoder.setBuffer_offset_atIndex(Some(&self.buffers.planes), 0, 2);
            encoder.setBuffer_offset_atIndex(Some(&self.buffers.accumulation), 0, 3);
            encoder.setBuffer_offset_atIndex(Some(&self.buffers.paths), 0, 6);
            encoder.setBuffer_offset_atIndex(Some(&self.buffers.unfinished), 0, 7);
            encoder.setBuffer_offset_atIndex(Some(&self.buffers.lights), 0, 8);
            if let Some(hardware) = &self.pipeline.hardware {
                encoder.setAccelerationStructure_atBufferIndex(Some(&hardware.acceleration), 4);
                encoder.setIntersectionFunctionTable_atBufferIndex(Some(&hardware.table), 5);
                encoder.useResource_usage((*self.buffers.spheres).as_ref(), MTLResourceUsage::Read);
            }
            encoder.dispatchThreads_threadsPerThreadgroup(
                MTLSize {
                    width: self.buffers.pixel_count,
                    height: 1,
                    depth: 1,
                },
                MTLSize {
                    width: threads,
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
            return Err(io::Error::other("Metal command did not complete successfully").into());
        }
        Ok(command.GPUEndTime() - command.GPUStartTime())
    }
}
