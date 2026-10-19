use super::*;

#[test]
fn host_buffer_layouts_match_metal() {
    assert_eq!(size_of::<data::Sphere>(), 112);
    assert_eq!(size_of::<data::Plane>(), 128);
    assert_eq!(size_of::<data::Parameters>(), 128);
    assert_eq!(size_of::<data::PathState>(), 128);
    assert_eq!(size_of::<data::Material>(), 96);
    assert_eq!(std::mem::align_of::<data::Parameters>(), 16);
    assert_eq!(std::mem::offset_of!(data::Parameters, width), 64);
    assert_eq!(std::mem::offset_of!(data::Sphere, material), 16);
    assert_eq!(std::mem::offset_of!(data::Material, kind), 80);
    assert_eq!(std::mem::offset_of!(data::Material, parent_medium), 88);
    assert_eq!(std::mem::offset_of!(data::PathState, completed_samples), 88);
    assert_eq!(std::mem::offset_of!(data::PathState, medium_id), 96);
    assert_eq!(std::mem::offset_of!(data::PathState, previous_pdf), 116);
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
fn gpu_visibility_matches_closest_hits_and_strict_shadow_endpoints() {
    let value = json!({
        "camera":{"lookfrom":[0,0,0],"lookat":[0,0,-1],"fov":90},
        "planes":[{"origin":[0,-2,0],"normal":[0,1,0],"mat":"flat"}],
        "spheres":[{"center":[0,0,-3],"radius":1,"mat":"glass"}]
    });
    let world = World::from_reader(value.to_string().as_bytes(), 32, 1).unwrap();
    for (traversal, name) in [
        (Traversal::Linear, "probe_visibility_linear"),
        (Traversal::Hardware, "probe_visibility_hardware"),
    ] {
        let config = settings(32, 1, 1, traversal);
        let frame = probe(&world, config, name);
        for (pixel, value) in frame.linear.iter().enumerate() {
            assert_eq!(value[0], value[1], "{traversal:?}: visibility ray {pixel}");
        }
        assert_eq!(frame.linear[8][0], 0., "Endpoint itself is excluded");
        assert_eq!(
            frame.linear[16][0], 1.,
            "Hit just before endpoint is included"
        );
        assert!(frame.linear[24..].iter().all(|p| p[0] == 0.));
    }
}
