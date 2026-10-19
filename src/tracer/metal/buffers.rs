//! Owned scene, film, and continuation buffers with isolated CPU pointer access.
use super::MetalObject;
use super::data::{PathState, SceneData};
use crate::tracer::Result;
use objc2::runtime::ProtocolObject;
use objc2_metal::{MTLBuffer, MTLDevice, MTLResourceOptions};
use std::io;
use std::ptr::NonNull;

pub(super) struct Buffers {
    pub spheres: MetalObject<dyn MTLBuffer>,
    pub planes: MetalObject<dyn MTLBuffer>,
    pub lights: MetalObject<dyn MTLBuffer>,
    pub accumulation: MetalObject<dyn MTLBuffer>,
    pub paths: MetalObject<dyn MTLBuffer>,
    pub unfinished: MetalObject<dyn MTLBuffer>,
    pub pixel_count: usize,
}

impl Buffers {
    pub fn new(device: &ProtocolObject<dyn MTLDevice>, data: &SceneData) -> Result<Self> {
        let pixel_count = data.parameters.width as usize * data.parameters.height as usize;
        let accumulation_bytes = pixel_count * size_of::<[f32; 4]>();
        let path_bytes = pixel_count * size_of::<PathState>();
        if (accumulation_bytes + path_bytes) as u64 > device.recommendedMaxWorkingSetSize() / 2 {
            return Err(io::Error::other("Image exceeds the GPU working-memory budget").into());
        }
        Ok(Self {
            spheres: buffer_from_slice(device, &data.spheres)?,
            planes: buffer_from_slice(device, &data.planes)?,
            lights: buffer_from_slice(device, &data.lights)?,
            accumulation: shared_buffer(device, accumulation_bytes, "radiance accumulation")?,
            paths: shared_buffer(device, path_bytes, "path continuations")?,
            unfinished: buffer_from_slice(device, &[0u32])?,
            pixel_count,
        })
    }

    // These helpers are called only between completed synchronous dispatches.
    pub fn reset_unfinished(&self) {
        // SAFETY: the counter holds one u32, and the preceding GPU command completed.
        unsafe {
            self.unfinished.contents().as_ptr().cast::<u32>().write(0);
        }
    }

    pub fn unfinished_count(&self) -> u32 {
        // SAFETY: the completed dispatch initialized this CPU-readable shared counter.
        unsafe { self.unfinished.contents().as_ptr().cast::<u32>().read() }
    }

    pub fn download(&self, samples: u32) -> Vec<[f32; 4]> {
        // SAFETY: all commands completed and initialized exactly pixel_count float4s.
        // The buffer is shared storage and remains owned for the lifetime of this slice.
        let values = unsafe {
            std::slice::from_raw_parts(
                self.accumulation.contents().as_ptr().cast::<[f32; 4]>(),
                self.pixel_count,
            )
        };
        let inverse_samples = 1. / samples as f32;
        values
            .iter()
            .map(|pixel| {
                [
                    pixel[0] * inverse_samples,
                    pixel[1] * inverse_samples,
                    pixel[2] * inverse_samples,
                    0.,
                ]
            })
            .collect()
    }
}

fn shared_buffer(
    device: &ProtocolObject<dyn MTLDevice>,
    bytes: usize,
    name: &str,
) -> Result<MetalObject<dyn MTLBuffer>> {
    device
        .newBufferWithLength_options(bytes, MTLResourceOptions::StorageModeShared)
        .ok_or_else(|| io::Error::other(format!("Could not allocate Metal {name} buffer")).into())
}

pub(super) fn buffer_from_slice<T: bytemuck::Pod>(
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
