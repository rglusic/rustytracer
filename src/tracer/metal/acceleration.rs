use super::data::Sphere;
use super::{MetalObject, buffer_from_slice};
use crate::tracer::Result;
use bytemuck::{Pod, Zeroable};
use objc2_foundation::{NSArray, NSString};
use objc2_metal::*;
use std::io;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Bounds {
    minimum: [f32; 3],
    maximum: [f32; 3],
}

pub struct Hardware {
    pub acceleration: MetalObject<dyn MTLAccelerationStructure>,
    pub table: MetalObject<dyn MTLIntersectionFunctionTable>,
    pub pipeline: MetalObject<dyn MTLComputePipelineState>,
}

impl Hardware {
    pub fn new(
        device: &objc2::runtime::ProtocolObject<dyn MTLDevice>,
        queue: &objc2::runtime::ProtocolObject<dyn MTLCommandQueue>,
        library: &objc2::runtime::ProtocolObject<dyn MTLLibrary>,
        spheres: &[Sphere],
        sphere_buffer: &objc2::runtime::ProtocolObject<dyn MTLBuffer>,
        kernel: &str,
    ) -> Result<Self> {
        let bounds: Vec<Bounds> = spheres
            .iter()
            .map(|sphere| {
                let [x, y, z, radius] = sphere.center_radius;
                Bounds {
                    minimum: [
                        (x - radius).next_down(),
                        (y - radius).next_down(),
                        (z - radius).next_down(),
                    ],
                    maximum: [
                        (x + radius).next_up(),
                        (y + radius).next_up(),
                        (z + radius).next_up(),
                    ],
                }
            })
            .collect();
        if bounds
            .iter()
            .any(|b| b.minimum.iter().chain(&b.maximum).any(|v| !v.is_finite()))
        {
            return Err(io::Error::other("Sphere bounds exceed the f32 range").into());
        }
        let buffer = buffer_from_slice(device, &bounds)?;
        let geometry = MTLAccelerationStructureBoundingBoxGeometryDescriptor::descriptor();
        geometry.setBoundingBoxBuffer(Some(&buffer));
        geometry.setBoundingBoxCount(spheres.len());
        geometry.setOpaque(false);
        geometry.setIntersectionFunctionTableOffset(0);
        // SAFETY: Bounds is a POD pair of packed float3 values, exactly 24 bytes.
        unsafe {
            geometry.setBoundingBoxStride(size_of::<Bounds>());
        }
        let descriptor = MTLPrimitiveAccelerationStructureDescriptor::descriptor();
        descriptor.setGeometryDescriptors(Some(&NSArray::from_slice(&[
            &*geometry as &MTLAccelerationStructureGeometryDescriptor
        ])));
        let sizes = device.accelerationStructureSizesWithDescriptor(&descriptor);
        let acceleration = device
            .newAccelerationStructureWithSize(sizes.accelerationStructureSize)
            .ok_or_else(|| {
                io::Error::other("Could not allocate ray-tracing acceleration structure")
            })?;
        let scratch = device
            .newBufferWithLength_options(
                sizes.buildScratchBufferSize,
                MTLResourceOptions::StorageModePrivate,
            )
            .ok_or_else(|| {
                io::Error::other("Could not allocate acceleration build scratch buffer")
            })?;
        let command = queue
            .commandBuffer()
            .ok_or_else(|| io::Error::other("Could not create acceleration build command"))?;
        let encoder = command
            .accelerationStructureCommandEncoder()
            .ok_or_else(|| io::Error::other("Could not create acceleration build encoder"))?;
        encoder.buildAccelerationStructure_descriptor_scratchBuffer_scratchBufferOffset(
            &acceleration,
            &descriptor,
            &scratch,
            0,
        );
        encoder.endEncoding();
        command.commit();
        command.waitUntilCompleted();
        if let Some(error) = command.error() {
            return Err(io::Error::other(format!("Acceleration build failed: {error}")).into());
        }

        let intersection = library
            .newFunctionWithName(&NSString::from_str("intersect_sphere"))
            .ok_or_else(|| io::Error::other("Metal sphere intersection function is missing"))?;
        let function = library
            .newFunctionWithName(&NSString::from_str(kernel))
            .ok_or_else(|| io::Error::other("Metal ray-tracing kernel is missing"))?;
        let linked = MTLLinkedFunctions::new();
        linked.setFunctions(Some(&NSArray::from_slice(&[&*intersection])));
        let pipeline_descriptor = MTLComputePipelineDescriptor::new();
        pipeline_descriptor.setComputeFunction(Some(&function));
        pipeline_descriptor.setLinkedFunctions(Some(&linked));
        let pipeline = device
            .newComputePipelineStateWithDescriptor_options_reflection_error(
                &pipeline_descriptor,
                MTLPipelineOption::None,
                None,
            )
            .map_err(|error| {
                io::Error::other(format!("Hardware ray-tracing pipeline failed: {error}"))
            })?;
        let table_descriptor = MTLIntersectionFunctionTableDescriptor::new();
        table_descriptor.setFunctionCount(1);
        let table = pipeline
            .newIntersectionFunctionTableWithDescriptor(&table_descriptor)
            .ok_or_else(|| io::Error::other("Could not create intersection function table"))?;
        let handle = pipeline
            .functionHandleWithFunction(&intersection)
            .ok_or_else(|| io::Error::other("Could not link sphere intersection function"))?;
        table.setFunction_atIndex(Some(&handle), 0);
        // SAFETY: immutable sphere records are retained by MetalRenderer throughout
        // all dispatches. Binding zero matches intersect_sphere's argument layout.
        unsafe {
            table.setBuffer_offset_atIndex(Some(sphere_buffer), 0, 0);
        }
        Ok(Self {
            acceleration,
            table,
            pipeline,
        })
    }
}
