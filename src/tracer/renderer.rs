use super::frame::Frame;
use super::scene::Scene;
use super::world::World;
use super::{Backend, Result, Settings};
use std::io;
use std::path::Path;
use std::time::Instant;

pub struct Renderer {
    world: World,
    scene: Scene,
    pub backend: Backend,
    pub device: String,
    pub traversal: String,
    pub setup_seconds: f64,
    resolution: (u32, u32),
    #[cfg(all(target_os = "macos", feature = "metal"))]
    gpu: Option<super::metal::MetalRenderer>,
}

impl Renderer {
    pub fn new(path: impl AsRef<Path>, settings: Settings) -> Result<Self> {
        settings.validate()?;
        let started = Instant::now();
        let world = World::new(path, settings.image.width, settings.image.height)?;
        let scene =
            Scene::with_environment(world.get_hitables(), world.description.environment.into());
        let mut renderer = Self {
            world,
            scene,
            backend: Backend::Cpu,
            device: "CPU (f64)".to_owned(),
            traversal: "linear".to_owned(),
            setup_seconds: 0.0,
            resolution: (settings.image.width, settings.image.height),
            #[cfg(all(target_os = "macos", feature = "metal"))]
            gpu: None,
        };
        #[cfg(all(target_os = "macos", feature = "metal"))]
        if settings.backend != Backend::Cpu {
            if let Some(device) = super::metal::default_device() {
                let gpu = super::metal::MetalRenderer::new(device, &renderer.world, settings)?;
                renderer.backend = Backend::Metal;
                renderer.device = gpu.device_name();
                renderer.traversal = gpu.traversal_name().to_owned();
                renderer.gpu = Some(gpu);
            } else if settings.backend == Backend::Metal {
                return Err(io::Error::other("No Metal device is available").into());
            } else {
                eprintln!("No Metal device is available; selecting CPU");
            }
        }
        #[cfg(not(all(target_os = "macos", feature = "metal")))]
        if settings.backend == Backend::Metal {
            return Err(
                io::Error::other("Metal requires macOS and the metal Cargo feature").into(),
            );
        }
        if renderer.backend == Backend::Cpu && settings.traversal == super::Traversal::Hardware {
            return Err(io::Error::other("Hardware traversal requires the Metal backend").into());
        }
        renderer.setup_seconds = started.elapsed().as_secs_f64();
        Ok(renderer)
    }

    pub fn geometry_counts(&self) -> (usize, usize) {
        (
            self.world.description.spheres.len(),
            self.world.description.planes.len(),
        )
    }

    pub fn render(&mut self, settings: Settings) -> Result<Frame> {
        settings.validate()?;
        if (settings.image.width, settings.image.height) != self.resolution {
            return Err(
                io::Error::other("Renderer resolution cannot change after initialization").into(),
            );
        }
        #[cfg(all(target_os = "macos", feature = "metal"))]
        if let Some(gpu) = &mut self.gpu {
            return gpu.render(settings);
        }
        let started = Instant::now();
        let workers = std::thread::available_parallelism().map_or(1, |count| count.get());
        let linear =
            super::render::render_linear(&self.scene, &self.world.get_camera(), settings, workers)?;
        Ok(Frame {
            linear,
            render_seconds: started.elapsed().as_secs_f64(),
            gpu_seconds: None,
        })
    }
}
