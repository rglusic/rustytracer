use super::*;

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
        eprintln!(
            "{name} vs f64 CPU mean absolute radiance error: {mae}; CPU mean {} GPU mean {} CPU max {} GPU max {}",
            cpu.iter().map(|p| p[0]).sum::<f32>() / cpu.len() as f32,
            image.iter().map(|p| p[0]).sum::<f32>() / image.len() as f32,
            cpu.iter().map(|p| p[0]).fold(0f32, f32::max),
            image.iter().map(|p| p[0]).fold(0f32, f32::max)
        );
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
#[ignore = "Requires an M3 or newer Apple GPU"]
fn gpu_mixed_material_scene_matches_cpu() {
    let config = settings(64, 40, 512, Traversal::Hardware);
    let mut value: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../../../worlds/closed_room.json")).unwrap();
    for plane in value["planes"].as_array_mut().unwrap() {
        plane["color"] = json!([0.65, 0.65, 0.65]);
    }
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
#[ignore = "Requires an M3 or newer Apple GPU"]
fn gpu_diffuse_furnace_matches_analytic_radiance() {
    let mut config = settings(32, 16, 1024, Traversal::Linear);
    let value = json!({"camera":{"lookfrom":[0,0,0],"lookat":[0,0,-1],"fov":45},
        "environment":[2,2,2], "planes":[{"origin":[0,0,-1],"normal":[0,0,1],"color":[0.2,0.5,0.9],"mat":"flat"}],
        "spheres":[{"center":[100,0,0],"radius":1,"color":[0,0,0],"mat":"flat"}]});
    let world = World::from_reader(value.to_string().as_bytes(), 32, 16).unwrap();
    let cpu_scene =
        crate::tracer::scene::Scene::with_environment(world.get_hitables(), [2., 2., 2.].into());
    let cpu =
        crate::tracer::render::render_linear(&cpu_scene, &world.get_camera(), config, 8).unwrap();
    for traversal in [Traversal::Linear, Traversal::Hardware] {
        config.traversal = traversal;
        let frame = gpu(&world, config).render(config).unwrap();
        let mut error = 0.;
        for (a, b) in frame.linear.iter().zip(&cpu) {
            for c in 0..3 {
                error += (a[c] - b[c]).abs() as f64;
            }
        }
        assert!(
            error / ((cpu.len() * 3) as f64) < 1e-5,
            "furnace f64/f32 error {error}"
        );
        for (c, expected) in [0.4, 1., 1.8].into_iter().enumerate() {
            let mean =
                frame.linear.iter().map(|p| p[c] as f64).sum::<f64>() / frame.linear.len() as f64;
            assert!(
                (mean - expected).abs() < 0.003,
                "{traversal:?} channel {c}: {mean}"
            );
        }
    }
}

#[test]
#[ignore = "Requires an M3 or newer Apple GPU"]
fn gpu_nested_glass_absorption_and_rough_conductors_match_cpu() {
    let config = settings(40, 24, 1024, Traversal::Hardware);
    let cases = [
        json!([
            {"center":[0,0,0],"radius":2,"mat":{"kind":"dielectric","ior":1.33}},
            {"center":[0,0,0],"radius":1,"mat":{"kind":"dielectric","ior":1.5,"absorption":[0.2,0.4,0.8]}}
        ]),
        json!([
            {"center":[0,0,0],"radius":2,"mat":{"kind":"conductor","eta":[0.2,0.9,1.1],"k":[3.9,2.5,2.1],"roughness":0.4}}
        ]),
        json!([
            {"center":[0,0,0],"radius":2,"mat":{"kind":"dielectric","ior":1.33}},
            {"center":[0,0,0],"radius":1,"mat":{"kind":"conductor","eta":[0.2,0.9,1.1],"k":[3.9,2.5,2.1],"roughness":0.4}}
        ]),
    ];
    for spheres in cases {
        let value = json!({"camera":{"lookfrom":[0,0,5],"lookat":[0,0,0],"fov":45},"environment":[1,1,1],"planes":[],"spheres":spheres});
        let world = World::from_reader(value.to_string().as_bytes(), 40, 24).unwrap();
        let scene = crate::tracer::scene::Scene::with_environment(
            world.get_hitables(),
            [1., 1., 1.].into(),
        );
        let cpu =
            crate::tracer::render::render_linear(&scene, &world.get_camera(), config, 8).unwrap();
        for traversal in [Traversal::Linear, Traversal::Hardware] {
            let config = Settings {
                traversal,
                ..config
            };
            let frame = gpu(&world, config).render(config).unwrap();
            let error = frame
                .linear
                .iter()
                .zip(&cpu)
                .flat_map(|(a, b)| (0..3).map(move |i| (a[i] - b[i]).abs() as f64))
                .sum::<f64>()
                / (cpu.len() * 3) as f64;
            assert!(error < 0.001, "{traversal:?}: material error {error}");
        }
    }
}

#[test]
#[ignore = "Requires an M3 or newer Apple GPU"]
fn gpu_mixed_light_selection_and_emission_match_cpu() {
    let config = settings(24, 16, 1024, Traversal::Linear);
    let value = json!({"camera":{"lookfrom":[0,0,0],"lookat":[0,0,-1],"fov":45},"environment":[0.1,0.1,0.1],
        "planes":[{"origin":[0,0,-1],"normal":[0,0,1],"mat":{"kind":"diffuse","albedo":[0.5,0.5,0.5]}},
                  {"origin":[0,0,5],"normal":[0,0,-1],"mat":{"kind":"emitter","emission":[0.5,0.7,1]}}],
        "spheres":[{"center":[0,0,3],"radius":1,"mat":{"kind":"emitter","emission":[4,3,2]}}]});
    let world = World::from_reader(value.to_string().as_bytes(), 24, 16).unwrap();
    let scene =
        crate::tracer::scene::Scene::with_environment(world.get_hitables(), [0.1, 0.1, 0.1].into());
    let cpu = crate::tracer::render::render_linear(&scene, &world.get_camera(), config, 8).unwrap();
    for traversal in [Traversal::Linear, Traversal::Hardware] {
        let config = Settings {
            traversal,
            ..config
        };
        let frame = gpu(&world, config).render(config).unwrap();
        let error = frame
            .linear
            .iter()
            .zip(&cpu)
            .flat_map(|(a, b)| (0..3).map(move |i| (a[i] - b[i]).abs() as f64))
            .sum::<f64>()
            / (cpu.len() * 3) as f64;
        assert!(error < 0.0001, "Multiple-light PDF/selection error {error}");
    }
}

#[test]
#[ignore = "Requires an M3 or newer Apple GPU"]
fn gpu_camera_inside_absorbing_glass_matches_analytic_radiance() {
    let config = settings(16, 8, 256, Traversal::Linear);
    let value = json!({"camera":{"lookfrom":[0,0,0],"lookat":[0,0,-1],"fov":45},"environment":[1,1,1],"planes":[],
        "spheres":[{"center":[0,0,0],"radius":2,"mat":{"kind":"dielectric","ior":1.5,"absorption":[0.1,0.2,0.3]}}]});
    let world = World::from_reader(value.to_string().as_bytes(), 16, 8).unwrap();
    for traversal in [Traversal::Linear, Traversal::Hardware] {
        let config = Settings {
            traversal,
            ..config
        };
        let mut renderer = gpu(&world, config);
        assert_eq!(renderer.data.parameters.camera_medium, 1);
        let frame = renderer.render(config).unwrap();
        for (channel, sigma) in [0.1_f64, 0.2, 0.3].into_iter().enumerate() {
            // First segment is 2 m; each internal reflection adds a 4 m diameter.
            let expected = 2.25 * 0.96 * (-2. * sigma).exp() / (1. - 0.04 * (-4. * sigma).exp());
            let mean = frame.linear.iter().map(|p| p[channel] as f64).sum::<f64>()
                / frame.linear.len() as f64;
            assert!(
                (mean - expected).abs() < 0.01,
                "{traversal:?}: inside-glass radiance {mean}, expected {expected}"
            );
        }
    }
}
