//! Safe-math shader compilation and traversal-specific compute pipeline preparation.
use super::MetalObject;
use super::acceleration::Hardware;
use super::scene::SceneData;
use crate::tracer::{Result, Traversal};
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
use objc2_metal::*;
use std::io;

pub(super) struct Pipeline {
    pub state: MetalObject<dyn MTLComputePipelineState>,
    pub hardware: Option<Hardware>,
    #[cfg(test)]
    library: MetalObject<dyn MTLLibrary>,
}

impl Pipeline {
    pub fn new(
        device: &ProtocolObject<dyn MTLDevice>,
        queue: &ProtocolObject<dyn MTLCommandQueue>,
        data: &SceneData,
        spheres: &ProtocolObject<dyn MTLBuffer>,
        traversal: Traversal,
    ) -> Result<Self> {
        let source = String::from(include_str!(concat!(env!("OUT_DIR"), "/rustytracer.metal")));
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
        let supported = device.supportsRaytracing() && device.supportsFamily(MTLGPUFamily::Apple9);
        let use_hardware = match traversal {
            Traversal::Hardware => {
                if !supported || data.spheres.is_empty() {
                    return Err(io::Error::other(
                        "Hardware traversal requires an M3 or newer GPU and at least one sphere",
                    )
                    .into());
                }
                true
            }
            Traversal::Auto => supported && data.spheres.len() >= 64,
            Traversal::Linear => false,
        };
        let hardware = if use_hardware {
            Some(Hardware::new(
                device,
                queue,
                &library,
                &data.spheres,
                spheres,
                "trace_hardware",
            )?)
        } else {
            None
        };
        let state = if let Some(hardware) = &hardware {
            hardware.pipeline.clone()
        } else {
            compute_pipeline(device, &library, "trace_linear")?
        };
        Ok(Self {
            state,
            hardware,
            #[cfg(test)]
            library,
        })
    }

    pub fn validate_threads(&self, threads: usize) -> Result<()> {
        let maximum = self.state.maxTotalThreadsPerThreadgroup();
        let execution_width = self.state.threadExecutionWidth();
        if threads > maximum || !threads.is_multiple_of(execution_width) {
            return Err(io::Error::other(format!(
                "Threadgroup size must be a multiple of {execution_width} and at most {maximum}"
            ))
            .into());
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn select_probe(
        &mut self,
        device: &ProtocolObject<dyn MTLDevice>,
        queue: &ProtocolObject<dyn MTLCommandQueue>,
        data: &SceneData,
        spheres: &ProtocolObject<dyn MTLBuffer>,
        name: &str,
    ) -> Result<()> {
        if self.hardware.is_some() {
            let hardware =
                Hardware::new(device, queue, &self.library, &data.spheres, spheres, name)?;
            self.state = hardware.pipeline.clone();
            self.hardware = Some(hardware);
        } else {
            self.state = compute_pipeline(device, &self.library, name)?;
        }
        Ok(())
    }
}

fn compute_pipeline(
    device: &ProtocolObject<dyn MTLDevice>,
    library: &ProtocolObject<dyn MTLLibrary>,
    name: &str,
) -> Result<MetalObject<dyn MTLComputePipelineState>> {
    let function = library
        .newFunctionWithName(&NSString::from_str(name))
        .ok_or_else(|| io::Error::other(format!("Metal {name} function is missing")))?;
    device
        .newComputePipelineStateWithFunction_error(&function)
        .map_err(|error| {
            io::Error::other(format!("Metal pipeline creation failed: {error}")).into()
        })
}
