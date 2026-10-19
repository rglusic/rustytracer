use super::*;

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

#[test]
#[ignore = "Requires an Apple GPU"]
fn gpu_continues_long_cavity_paths_without_truncation() {
    let mut config = settings(17, 9, 512, Traversal::Linear);
    config.batch_size = 1;
    let value = json!({"camera":{"lookfrom":[0,0,0],"lookat":[0,0,-1],"fov":45},"planes":[],
        "spheres":[{"center":[0,0,0],"radius":2,"mat":{"kind":"diffuse","albedo":[0.8,0.8,0.8],"emission":[0.2,0.2,0.2],"two_sided":true}}]});
    let world = World::from_reader(value.to_string().as_bytes(), 17, 9).unwrap();
    for light_sampling in [false, true] {
        config.light_sampling = light_sampling;
        let mut renderer = gpu(&world, config);
        let frame = renderer.render(config).unwrap();
        assert!(renderer.last_dispatches > config.image.samples_per_pixel as usize);
        let mean =
            frame.linear.iter().map(|p| p[0] as f64).sum::<f64>() / frame.linear.len() as f64;
        assert!(
            (mean - 1.).abs() < 0.02,
            "cavity energy {mean}, MIS {light_sampling}"
        );
    }
}
