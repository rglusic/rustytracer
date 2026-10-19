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
    World::from_reader(
        include_bytes!("../../../worlds/closed_room.json").as_slice(),
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
    if settings.traversal == Traversal::Hardware {
        let hardware = acceleration::Hardware::new(
            &renderer.device,
            &renderer.queue,
            &renderer.library,
            &renderer.data.spheres,
            &renderer.spheres,
            name,
        )
        .unwrap();
        renderer.pipeline = hardware.pipeline.clone();
        renderer.hardware = Some(hardware);
    } else {
        let function = renderer
            .library
            .newFunctionWithName(&NSString::from_str(name))
            .unwrap();
        renderer.pipeline = renderer
            .device
            .newComputePipelineStateWithFunction_error(&function)
            .unwrap();
    }
    renderer.render(settings).unwrap()
}

#[test]
fn host_buffer_layouts_match_metal() {
    assert_eq!(size_of::<data::Sphere>(), 48);
    assert_eq!(size_of::<data::Plane>(), 64);
    assert_eq!(size_of::<data::Parameters>(), 96);
    assert_eq!(std::mem::align_of::<data::Parameters>(), 16);
    assert_eq!(std::mem::offset_of!(data::Parameters, width), 64);
    assert_eq!(std::mem::offset_of!(data::Sphere, material), 32);
}

#[test]
#[ignore = "Requires an Apple GPU; run with --ignored --test-threads=1"]
fn gpu_random_stream_matches_cpu_exactly() {
    let config = settings(33, 1, 1, Traversal::Linear);
    let frame = probe(&room(config), config, "probe_rng");
    for (pixel, output) in frame.linear.iter().enumerate() {
        let mut rng = crate::tracer::sampler::Sampler::new(config.seed, pixel as u32, 7);
        for value in &output[..3] {
            assert_eq!(*value, rng.next() as f32);
        }
    }
}

#[test]
#[ignore = "Requires an M3 or newer Apple GPU"]
fn gpu_intersections_match_cpu_for_inside_tangent_miss_and_nonunit_rays() {
    let scene = json!({
        "camera": {"lookfrom": [0, 0, 0], "lookat": [0, 0, -1], "fov": 90},
        "planes": [{"origin": [0, -2, 0], "normal": [0, 1, 0], "color": [1, 1, 1], "mat": "flat"}],
        "spheres": [{"center": [0, 0, -3], "radius": 1, "color": [1, 1, 1], "mat": "glass"}]
    });
    let config = settings(8, 1, 1, Traversal::Linear);
    let world = World::from_reader(scene.to_string().as_bytes(), 8, 1).unwrap();
    let linear = probe(&world, config, "probe_linear");
    let hardware = probe(
        &world,
        Settings {
            traversal: Traversal::Hardware,
            ..config
        },
        "probe_hardware",
    );
    let expected = [2.0, -1.0, 1.0, 3.0, 1.0, -1.0, -1.0, 0.002];
    for (pixel, expected) in expected.into_iter().enumerate() {
        assert!(
            (linear.linear[pixel][0] - expected as f32).abs() < 2e-4,
            "linear pixel {pixel}: {:?}",
            linear.linear[pixel]
        );
        assert!(
            (hardware.linear[pixel][0] - expected as f32).abs() < 2e-4,
            "hardware pixel {pixel}: {:?}",
            hardware.linear[pixel]
        );
    }
}

#[test]
#[ignore = "Requires an Apple GPU"]
fn gpu_glass_total_internal_reflection_stays_finite_for_nonunit_rays() {
    let config = settings(3, 1, 1, Traversal::Linear);
    let frame = probe(&room(config), config, "probe_glass");
    for pixel in frame.linear {
        assert!((pixel[0] - 0.9).abs() < 1e-6);
        assert!((pixel[1] + 0.4358899).abs() < 1e-6);
        assert!(pixel.iter().all(|v| v.is_finite()));
    }
}

#[test]
#[ignore = "Requires an M3 or newer Apple GPU"]
fn gpu_images_match_cpu_reference_and_hardware_matches_linear() {
    let config = settings(64, 32, 1024, Traversal::Linear);
    let world = room(config);
    let cpu_scene = crate::tracer::scene::Scene::new(world.get_hitables());
    let cpu =
        crate::tracer::render::render_linear(&cpu_scene, &world.get_camera(), config, 8).unwrap();
    let linear = gpu(&world, config).render(config).unwrap();
    let accelerated_config = Settings {
        traversal: Traversal::Hardware,
        ..config
    };
    let hardware = gpu(&world, accelerated_config)
        .render(accelerated_config)
        .unwrap();
    for (name, image) in [("linear", &linear.linear), ("hardware", &hardware.linear)] {
        let mae: f64 = image
            .iter()
            .zip(&cpu)
            .flat_map(|(a, b)| (0..3).map(move |i| f64::from((a[i] - b[i]).abs())))
            .sum::<f64>()
            / (cpu.len() * 3) as f64;
        eprintln!("{name} vs f64 CPU mean absolute radiance error: {mae}");
        assert!(
            mae < 0.002,
            "{name} CPU/GPU agreement exceeded tolerance: {mae}"
        );
        assert!(image.iter().flatten().all(|v| v.is_finite()));
    }
    let maximum = linear
        .linear
        .iter()
        .zip(&hardware.linear)
        .flat_map(|(a, b)| (0..3).map(move |i| (a[i] - b[i]).abs()))
        .fold(0.0f32, f32::max);
    assert!(
        maximum < 0.02,
        "Hardware traversal changed radiance by {maximum}"
    );
}

#[test]
#[ignore = "Requires an Apple GPU"]
fn gpu_batch_sizes_and_reused_buffers_do_not_change_samples() {
    let config = settings(37, 19, 67, Traversal::Linear);
    let world = room(config);
    let mut renderer = gpu(&world, config);
    let initial = renderer.render(config).unwrap();
    for batch_size in [1, 7, 32, 64, 128] {
        let frame = renderer
            .render(Settings {
                batch_size,
                ..config
            })
            .unwrap();
        assert_eq!(
            initial.linear, frame.linear,
            "Changing batch size {batch_size} changed samples or accumulation"
        );
    }
}

#[test]
#[ignore = "Requires an Apple GPU"]
fn gpu_handles_tiny_images_empty_scenes_and_emitters() {
    for (width, height) in [(1, 1), (3, 2), (17, 9)] {
        for emissive in [false, true] {
            let config = settings(width, height, 3, Traversal::Linear);
            let scene = json!({
                "camera": {"lookfrom": [0,0,0], "lookat": [0,0,-1], "fov": 90},
                "planes": if emissive { json!([{"origin": [0,0,-1],"normal": [0,0,1],"color": [1,1,1],"mat":"diffuse_light"}]) } else { json!([]) },
                "spheres": []
            });
            let world = World::from_reader(scene.to_string().as_bytes(), width, height).unwrap();
            let frame = gpu(&world, config).render(config).unwrap();
            let rgb = frame.image(config.image).unwrap();
            assert!(
                rgb.pixels()
                    .all(|pixel| pixel.0 == if emissive { [255; 3] } else { [0; 3] })
            );
        }
    }
}

#[test]
#[ignore = "Requires an M3 or newer Apple GPU"]
fn gpu_mixed_material_scene_matches_cpu() {
    let config = settings(64, 40, 512, Traversal::Hardware);
    let mut value: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../../worlds/closed_room.json")).unwrap();
    let spheres = value["spheres"].as_array_mut().unwrap();
    for (x, material) in [(-2.0, "metal"), (2.0, "flat")] {
        spheres.push(
            json!({"center": [x,0,0], "radius": 0.8, "color": [0.7,0.8,0.9], "mat": material}),
        );
    }
    let world = World::from_reader(value.to_string().as_bytes(), 64, 40).unwrap();
    let scene = crate::tracer::scene::Scene::new(world.get_hitables());
    let cpu = crate::tracer::render::render_linear(&scene, &world.get_camera(), config, 8).unwrap();
    let actual = gpu(&world, config).render(config).unwrap();
    let error = actual
        .linear
        .iter()
        .zip(&cpu)
        .flat_map(|(a, b)| (0..3).map(move |i| f64::from((a[i] - b[i]).abs())))
        .sum::<f64>()
        / (cpu.len() * 3) as f64;
    assert!(error < 0.002, "Mixed-material error {error}");
}

#[test]
#[ignore = "Requires an Apple GPU"]
fn gpu_rejects_invalid_dispatch_settings_and_resolution_changes() {
    let config = settings(3, 2, 1, Traversal::Linear);
    let world = room(config);
    let mut renderer = gpu(&world, config);
    assert!(
        renderer
            .render(Settings {
                batch_size: 0,
                ..config
            })
            .is_err()
    );
    assert!(
        renderer
            .render(Settings {
                threadgroup_size: 1,
                ..config
            })
            .is_err()
    );
    let different = Settings {
        image: RenderOptions {
            width: 2,
            height: 3,
            samples_per_pixel: 1,
        },
        ..config
    };
    assert!(renderer.render(different).is_err());
}
