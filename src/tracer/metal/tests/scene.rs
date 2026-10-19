//! Scene preparation needs no GPU device, so these checks also run on hosted macOS CI.
use super::*;

#[test]
fn packed_scene_preserves_nonconcentric_media_and_cpu_light_order() {
    let value = json!({
        "camera":{"lookfrom":[6,0,0],"lookat":[5,0,-1],"fov":45},
        "environment":[0.1,0.2,0.3],
        "planes":[{"origin":[0,-10,0],"normal":[0,1,0],"mat":{"kind":"emitter","emission":[1,2,3]}}],
        "spheres":[
            {"center":[6,0,0],"radius":1,"mat":{"kind":"dielectric","ior":1.5}},
            {"center":[-8,0,0],"radius":2,"mat":{"kind":"dielectric","ior":1.2}},
            {"center":[5,0,0],"radius":4,"mat":{"kind":"dielectric","ior":1.33}},
            {"center":[30,0,0],"radius":1,"mat":{"kind":"emitter","emission":[4,5,6]}}
        ]
    });
    let config = settings(3, 2, 1, Traversal::Linear);
    let world = World::from_reader(value.to_string().as_bytes(), 3, 2).unwrap();
    let data = SceneData::new(&world, config).unwrap();
    assert_eq!(data.parameters.camera_medium, 1);
    assert_eq!(data.spheres[0].material.parent_medium, 3);
    assert_eq!(data.spheres[1].material.parent_medium, 0);
    assert_eq!(data.spheres[2].material.parent_medium, 0);
    assert_eq!(data.lights, [[1, 0, 0, 0], [0, 3, 0, 0], [2, 0, 0, 0]]);
    assert_eq!(data.parameters.light_count, 3);
}

#[test]
fn packed_scene_rejects_media_that_become_ambiguous_in_f32() {
    let config = settings(3, 2, 1, Traversal::Linear);
    for (camera, spheres, message) in [
        (
            json!([1.00000001, 0, 0]),
            json!([{"center":[0,0,0],"radius":1,"mat":"glass"}]),
            "Camera medium is ambiguous",
        ),
        (
            json!([0, 0, 4]),
            json!([
                {"center":[0,0,0],"radius":1,"mat":"glass"},
                {"center":[0,0,0],"radius":1.00000001,"mat":"glass"}
            ]),
            "Dielectric boundaries overlap",
        ),
    ] {
        let value = json!({
            "camera":{"lookfrom":camera,"lookat":[0,0,-1],"fov":45},
            "planes":[],"spheres":spheres
        });
        // Both scenes are valid in the CPU's f64 geometry.
        let world = World::from_reader(value.to_string().as_bytes(), 3, 2).unwrap();
        let error = SceneData::new(&world, config)
            .err()
            .expect("Ambiguous packed media accepted");
        assert!(error.to_string().contains(message), "{error}");
    }
}
