mod geometry;
mod scene;
mod scheduling;
mod transport;

use super::*;
use crate::tracer::{Backend, RenderOptions, Traversal};
use serde_json::json;

fn settings(width: u32, height: u32, samples: u32, traversal: Traversal) -> Settings {
    Settings {
        image: RenderOptions {
            width,
            height,
            samples_per_pixel: samples,
        },
        backend: Backend::Metal,
        traversal,
        seed: 42,
        ..Settings::default()
    }
}

fn room(settings: Settings) -> World {
    // Absorbing walls make pointwise f32/f64 parity meaningful;
    // furnace and cavity energy are tested separately against analytic expectations.
    let mut value: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../../../worlds/closed_room.json")).unwrap();
    for plane in value["planes"].as_array_mut().unwrap() {
        plane["color"] = json!([0.65, 0.65, 0.65]);
    }
    World::from_reader(
        value.to_string().as_bytes(),
        settings.image.width,
        settings.image.height,
    )
    .unwrap()
}

fn gpu(world: &World, settings: Settings) -> MetalRenderer {
    MetalRenderer::new(
        default_device().expect("This test requires an actual Metal GPU"),
        world,
        settings,
    )
    .unwrap()
}

fn probe(world: &World, settings: Settings, name: &str) -> Frame {
    let mut renderer = gpu(world, settings);
    renderer
        .pipeline
        .select_probe(
            &renderer.device,
            &renderer.queue,
            &renderer.data,
            &renderer.buffers.spheres,
            name,
        )
        .unwrap();
    renderer.render(settings).unwrap()
}
