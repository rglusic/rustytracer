use super::*;
use crate::tracer::geometry::{Plane, Shape, Sphere};
use crate::tracer::material::{Material, uniform_sphere};
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn occlusion_matches_closest_hit_for_scaled_inside_and_grazing_rays() {
    for scale in [1e-8, 1., 1e8] {
        let material = Material::diffuse(white());
        let scene = Scene::new(vec![
            Arc::new(Sphere {
                center: Color::new(0., 0., -3. * scale),
                radius: scale,
                material: Material {
                    kind: Kind::Dielectric,
                    ..material
                },
            }),
            Arc::new(Plane {
                origin: Color::new(0., -2. * scale, 0.),
                normal: Color::unit_y(),
                material,
            }),
        ]);
        let mut rng = Sampler::new(42, 0, 0);
        for _ in 0..2048 {
            let origin =
                Color::new(rng.next() - 0.5, rng.next() - 0.5, rng.next() - 0.5) * (8. * scale);
            let direction = uniform_sphere(rng.next(), rng.next());
            let ray = Ray::new_from(origin, direction);
            for maximum in [0., scale, 5. * scale, f64::INFINITY] {
                assert_eq!(
                    scene.occluded(&ray, maximum),
                    scene.intersect(&ray, maximum).is_some()
                );
            }
        }
        for (origin, direction) in [
            (black(), -Color::unit_z()),
            (Color::new(scale, 0., 0.), -Color::unit_z()),
            (Color::new(0., 0., -3. * scale), Color::unit_z()),
            (black(), Color::new(1., -1e-8, 0.)),
        ] {
            let ray = Ray::new_from(origin, direction);
            assert_eq!(
                scene.occluded(&ray, f64::INFINITY),
                scene.intersect(&ray, f64::INFINITY).is_some()
            );
        }
    }
}

#[test]
fn occlusion_excludes_the_endpoint_and_preserves_surface_offsets() {
    let sphere = Sphere {
        center: Color::new(0., 0., -3.),
        radius: 1.,
        material: Material::diffuse(white()),
    };
    let ray = Ray::new_from(black(), -Color::unit_z());
    let surface = sphere.surface(&ray, 2.);
    let scene = Scene::new(vec![Arc::new(sphere)]);
    assert!(!scene.occluded(&ray, 2.));
    assert!(scene.occluded(&ray, 2_f64.next_up()));
    assert!(!scene.occluded(
        &Ray::new_from(surface.offset(Color::unit_z()), Color::unit_z()),
        f64::INFINITY
    ));
    assert!(scene.occluded(
        &Ray::new_from(surface.offset(-Color::unit_z()), -Color::unit_z()),
        f64::INFINITY
    ));
}

struct VisibilityOnly {
    calls: Arc<AtomicUsize>,
    panic_on_hit: bool,
}

impl Hitable for VisibilityOnly {
    fn hit(&self, _: &Ray, _: f64) -> f64 {
        assert!(!self.panic_on_hit, "Unnecessary visibility traversal");
        self.calls.fetch_add(1, Ordering::Relaxed);
        1.
    }
    fn material(&self) -> &Material {
        static MATERIAL: Material = Material::diffuse(Color::new(0.5, 0.5, 0.5));
        &MATERIAL
    }
    fn get_norm_at_p(&self, _: &Color) -> Color {
        panic!("A visibility query must not build a shading surface");
    }
    fn shape(&self) -> Shape {
        Shape::Plane { origin: black() }
    }
}

#[test]
fn occlusion_stops_at_first_blocker_without_shading_it() {
    let calls = Arc::new(AtomicUsize::new(0));
    let scene = Scene::new(vec![
        Arc::new(VisibilityOnly {
            calls: calls.clone(),
            panic_on_hit: false,
        }),
        Arc::new(VisibilityOnly {
            calls: calls.clone(),
            panic_on_hit: true,
        }),
    ]);
    assert!(scene.occluded(&Ray::new_from(black(), Color::unit_z()), 2.));
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[test]
fn zero_light_samples_skip_visibility_and_preserve_random_draws() {
    let surface = Surface {
        point: black(),
        normal: Color::unit_z(),
        error: black(),
    };
    let material = Material::diffuse(white());
    for sphere_light in [false, true] {
        let mut objects: Vec<Arc<dyn Hitable>> = vec![Arc::new(VisibilityOnly {
            calls: Arc::new(AtomicUsize::new(0)),
            panic_on_hit: true,
        })];
        if sphere_light {
            objects.push(Arc::new(Sphere {
                center: Color::new(0., 0., 3.),
                radius: 1.,
                material: Material {
                    kind: Kind::Emitter,
                    emission: white(),
                    ..material
                },
            }));
        }
        let scene = Scene::with_environment(objects, if sphere_light { black() } else { white() });
        // Environment: lower hemisphere. Sphere: far, non-emitting side.
        let seed = (0..100)
            .find(|seed| {
                let mut rng = Sampler::new(*seed, 0, 0);
                rng.next();
                let direction = uniform_sphere(rng.next(), rng.next());
                if sphere_light {
                    direction.z > 0.
                } else {
                    direction.z < 0.
                }
            })
            .unwrap();
        let mut rng = Sampler::new(seed, 0, 0);
        let mut expected = Sampler::new(seed, 0, 0);
        for _ in 0..3 {
            expected.next();
        }
        assert_eq!(
            scene.direct_light(
                surface,
                Color::unit_z(),
                Color::unit_z(),
                &material,
                black(),
                &mut rng
            ),
            black()
        );
        assert_eq!(rng.next(), expected.next());
    }
}
