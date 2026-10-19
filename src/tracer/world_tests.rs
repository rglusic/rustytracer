use super::*;
use serde_json::{Value, json};

fn fixture() -> Value {
    serde_json::from_slice(include_bytes!("../../worlds/closed_room.json")).unwrap()
}

fn load(value: &Value, width: u32, height: u32) -> Result<World> {
    World::from_reader(value.to_string().as_bytes(), width, height)
}

#[test]
fn loads_the_existing_scene_format() {
    let world = load(&fixture(), 200, 100).unwrap();
    assert_eq!(world.get_hitables().len(), 8);
}

#[test]
fn preserves_fractional_and_portrait_aspect_ratios() {
    let scene = json!({
        "camera": {"lookfrom": [0, 0, 0], "lookat": [0, 0, -1], "fov": 90},
        "planes": [],
        "spheres": []
    });
    for (width, height, aspect) in [(3, 2, 1.5), (1, 2, 0.5)] {
        let world = load(&scene, width, height).unwrap();
        let ray = world.get_camera().get_ray(1.0, 0.5);
        assert!((ray.direction().x - aspect).abs() < 1e-12);
        assert!((ray.direction().z + 1.0).abs() < 1e-12);
    }
}

#[test]
fn rejects_short_long_and_nonnumeric_vectors() {
    for malformed in [json!([1, 2]), json!([1, 2, 3, 4]), json!([0, "bad", 0])] {
        for pointer in [
            "/camera/lookfrom",
            "/camera/lookat",
            "/planes/0/origin",
            "/planes/0/normal",
            "/planes/0/color",
            "/spheres/0/center",
            "/spheres/0/color",
        ] {
            let mut value = fixture();
            *value.pointer_mut(pointer).unwrap() = malformed.clone();
            assert!(
                load(&value, 3, 2).is_err(),
                "Accepted {pointer}: {malformed}"
            );
        }
    }
}

#[test]
fn rejects_degenerate_camera_settings() {
    for fov in [0.0, -10.0, 180.0, 200.0] {
        let mut value = fixture();
        value["camera"]["fov"] = json!(fov);
        assert!(load(&value, 3, 2).is_err());
    }
    for target in [json!([0, 1, 7]), json!([0, 0, 7])] {
        let mut value = fixture();
        value["camera"]["lookat"] = target;
        assert!(load(&value, 3, 2).is_err());
    }
    for (width, height) in [(0, 2), (2, 0)] {
        assert!(load(&fixture(), width, height).is_err());
    }
}

#[test]
fn validates_geometry_and_normalizes_plane_normals() {
    let mut value = fixture();
    value["planes"][0]["normal"] = json!([0, 5, 0]);
    let world = load(&value, 3, 2).unwrap();
    let normal = world.get_hitables()[0].get_norm_at_p(&Vector3::new(0.0, 0.0, 0.0));
    assert!((normal - Vector3::unit_y()).magnitude() < 1e-12);

    value["planes"][0]["normal"] = json!([0, 0, 0]);
    assert!(load(&value, 3, 2).is_err());
    for radius in [0.0, -1.0, 1e300] {
        let mut value = fixture();
        value["spheres"][0]["radius"] = json!(radius);
        assert!(load(&value, 3, 2).is_err());
    }
    let mut value = fixture();
    value["planes"][0]["color"] = json!([-1, 0, 0]);
    assert!(load(&value, 3, 2).is_err());
}

#[test]
fn missing_and_malformed_scenes_return_errors() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("worlds/missing-scene.json");
    let error = World::new(path, 3, 2)
        .err()
        .expect("Missing scene was accepted");
    assert!(error.to_string().contains("missing-scene.json"));
    assert!(World::from_reader(b"{ malformed JSON".as_slice(), 3, 2).is_err());
}

#[test]
fn rejects_unphysical_and_ambiguous_materials() {
    for mat in [
        json!("unknown"),
        json!({"kind":"diffuse","albedo":[1.1,0,0]}),
        json!({"kind":"dielectric","ior":0.5}),
        json!({"kind":"dielectric","absorption":[-1,0,0]}),
        json!({"kind":"conductor"}),
        json!({"kind":"conductor","eta":[0,1,1],"k":[1,1,1]}),
        json!({"kind":"diffuse","roughness":0.5}),
        json!({"kind":"emitter"}),
        json!({"kind":"diffuse","emision":[1,1,1]}),
    ] {
        let mut value = fixture();
        value["spheres"][0]["mat"] = mat.clone();
        assert!(load(&value, 3, 2).is_err(), "accepted {mat}");
    }
    let mut value = fixture();
    value["planes"][0]["mat"] = json!("glass");
    assert!(load(&value, 3, 2).is_err());
    let mut value = fixture();
    value["spheres"][1]["mat"] = json!("glass");
    value["spheres"][1]["center"] = json!([0, 1, 0]);
    assert!(load(&value, 3, 2).is_err());
}
#[test]
fn emitter_color_is_radiance_and_reflectance_cannot_exceed_one() {
    let mut value = fixture();
    value["spheres"][1]["color"] = json!([1, 2, 3]);
    let world = load(&value, 3, 2).unwrap();
    assert_eq!(
        world.get_hitables()[7].material().emission,
        Vector3::new(1., 2., 3.)
    );
    value["planes"][0]["color"] = json!([1.01, 1, 1]);
    assert!(load(&value, 3, 2).is_err());
}
