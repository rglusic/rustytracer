use super::Settings;
use super::geometry::{Hitable, Plane, Shape, Sphere};
use super::material::{Color, Kind, Material, black};
use super::ray::Ray;
use super::sampler::Sampler;
use super::scene::Scene;
use super::world::World;
use cgmath::InnerSpace;
use serde_json::json;
use std::sync::Arc;

fn load(value: serde_json::Value) -> World {
    World::from_reader(value.to_string().as_bytes(), 1, 1).unwrap()
}
fn primary() -> Ray {
    Ray::new_from(black(), -Color::unit_z())
}
fn estimate(scene: &Scene, settings: Settings, count: u32, seed: u32) -> Color {
    let mut sum = black();
    for i in 0..count {
        sum += scene.render(&primary(), settings, &mut Sampler::new(seed, 0, i));
    }
    sum / f64::from(count)
}
#[test]
fn diffuse_furnace_recovers_analytical_radiance() {
    let albedo = Color::new(0.2, 0.5, 0.9);
    let objects: Vec<Arc<dyn Hitable>> = vec![Arc::new(Plane {
        origin: -Color::unit_z(),
        normal: Color::unit_z(),
        material: Material::diffuse(albedo),
    })];
    let scene = Scene::with_environment(objects, Color::new(2., 2., 2.));
    for light_sampling in [false, true] {
        let value = estimate(
            &scene,
            Settings {
                light_sampling,
                ..Settings::default()
            },
            100_000,
            12,
        );
        assert!((value - 2. * albedo).magnitude() < 0.01, "{value:?}");
    }
}
#[test]
fn spherical_light_matches_analytic_irradiance_and_mis_reduces_variance() {
    let w = load(
        json!({"camera":{"lookfrom":[0,0,0],"lookat":[0,0,-1],"fov":30},
        "planes":[{"origin":[0,0,-1],"normal":[0,0,1],"color":[0.5,0.5,0.5],"mat":"flat"}],
        "spheres":[{"center":[0,0,3],"radius":1,"mat":{"kind":"emitter","emission":[4,4,4]}}]}),
    );
    let scene = Scene::with_environment(w.get_hitables(), black());
    let expected = 0.5 * 4. / 16.;
    let mut errors = [0.; 2];
    for (mode, light_sampling) in [false, true].into_iter().enumerate() {
        let mut mean = 0.;
        for seed in 0..24 {
            let value = estimate(
                &scene,
                Settings {
                    max_bounces: 1,
                    light_sampling,
                    ..Settings::default()
                },
                4096,
                seed,
            )
            .x;
            mean += value / 24.;
            errors[mode] += (value - expected).powi(2);
        }
        assert!((mean - expected).abs() < 0.003, "mode {mode}: {mean}");
    }
    assert!(errors[1] < errors[0], "MIS MSE {errors:?}");
}
#[test]
fn ray_spawning_and_intersections_work_across_scene_scales() {
    for scale in [1e-8, 1e-4, 1., 1e4, 1e8] {
        let sphere = Sphere {
            center: Color::new(0., 0., -3. * scale),
            radius: scale,
            material: Material::diffuse(Color::new(0.5, 0.5, 0.5)),
        };
        let ray = Ray::new_from(black(), -Color::unit_z());
        let t = sphere.hit(&ray, f64::INFINITY);
        assert!((t / scale - 2.).abs() < 1e-12);
        let s = sphere.surface(&ray, t);
        assert_eq!(
            sphere.hit(
                &Ray::new_from(s.offset(Color::unit_z()), Color::unit_z()),
                f64::INFINITY
            ),
            0.
        );
        let inward = Ray::new_from(s.offset(-Color::unit_z()), -Color::unit_z());
        assert!((sphere.hit(&inward, f64::INFINITY) / scale - 2.).abs() < 1e-10);
        let plane = Plane {
            origin: Color::new(0., 0., -scale),
            normal: Color::unit_z(),
            material: sphere.material,
        };
        let grazing = Ray::new_from(black(), Color::new(1., 0., -1e-8));
        assert!((plane.hit(&grazing, f64::INFINITY) / (scale * 1e8) - 1.).abs() < 1e-12);
    }
}
#[test]
fn nested_dielectrics_choose_the_actual_surrounding_medium() {
    let w = load(
        json!({"camera":{"lookfrom":[0,0,4],"lookat":[0,0,0],"fov":30},"planes":[],
        "spheres":[{"center":[0,0,0],"radius":2,"mat":{"kind":"dielectric","ior":1.33}},
                   {"center":[0,0,0],"radius":1,"mat":{"kind":"dielectric","ior":1.5,"absorption":[0.1,0.2,0.3]}}]}),
    );
    let scene = Scene::with_environment(w.get_hitables(), black());
    assert_eq!(scene.medium(Color::new(0., 0., 3.), None).0, 1.);
    assert_eq!(scene.medium(Color::new(0., 0., 1.5), None).0, 1.33);
    assert_eq!(scene.medium(black(), None).0, 1.5);
    assert_eq!(scene.medium(black(), Some(1)).0, 1.33);
    assert!(matches!(scene.renderables[0].shape(), Shape::Sphere { .. }));
}
#[test]
fn roulette_and_unlimited_depth_do_not_lose_enclosure_energy() {
    // Concentric spheres: a diffuse interior surface always sees the enclosing emitter.
    let light = Material {
        kind: Kind::Emitter,
        emission: Color::new(2., 2., 2.),
        two_sided: true,
        ..Material::diffuse(black())
    };
    let scene = Scene::with_environment(
        vec![Arc::new(Sphere {
            center: black(),
            radius: 2.,
            material: light,
        })],
        black(),
    );
    let value = estimate(&scene, Settings::default(), 1000, 31);
    assert!((value - Color::new(2., 2., 2.)).magnitude() < 1e-12);
}
