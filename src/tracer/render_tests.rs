use super::*;
use crate::tracer::geometry::Hitable;
use crate::tracer::ray::Ray;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct EmissiveSurface {
    samples: Arc<AtomicUsize>,
    panic_on_hit: bool,
}

impl Hitable for EmissiveSurface {
    fn hit(&self, _: &Ray, _: f64) -> f64 {
        assert!(!self.panic_on_hit, "Injected worker failure");
        1.0
    }

    fn get_color(&self) -> &Vector3<f64> {
        static WHITE: Vector3<f64> = Vector3::new(1.0, 1.0, 1.0);
        &WHITE
    }

    fn get_material(&self) -> &str {
        "diffuse_light"
    }

    fn get_norm_at_p(&self, _: &Vector3<f64>) -> Vector3<f64> {
        // Each path reaches this emitter once and terminates there.
        self.samples.fetch_add(1, Ordering::Relaxed);
        Vector3::unit_z()
    }
}

fn camera() -> Camera {
    Camera::new(
        Vector3::new(0.0, 0.0, 0.0),
        -Vector3::unit_z(),
        Vector3::unit_y(),
        90.0,
        1.0,
    )
}

#[test]
fn covers_every_pixel_with_exact_sample_count_for_any_worker_partition() {
    for (width, height) in [(1, 1), (3, 2), (17, 3), (201, 2)] {
        for workers in [1, 8, 64] {
            for samples_per_pixel in [1, 3] {
                let samples = Arc::new(AtomicUsize::new(0));
                let scene = Scene::new(vec![Arc::new(EmissiveSurface {
                    samples: Arc::clone(&samples),
                    panic_on_hit: false,
                })]);
                let image = render_with_workers(
                    &scene,
                    &camera(),
                    RenderOptions {
                        width,
                        height,
                        samples_per_pixel,
                    },
                    workers,
                )
                .unwrap();
                assert_eq!(image.dimensions(), (width, height));
                assert!(image.pixels().all(|pixel| pixel.0 == [255, 255, 255]));
                assert_eq!(
                    samples.load(Ordering::Relaxed),
                    (width * height * samples_per_pixel) as usize
                );
            }
        }
    }
}

#[test]
fn rejects_zero_dimensions_samples_and_workers() {
    let scene = Scene::new(Vec::new());
    for (width, height, samples_per_pixel, workers) in
        [(0, 1, 1, 1), (1, 0, 1, 1), (1, 1, 0, 1), (1, 1, 1, 0)]
    {
        assert!(
            render_with_workers(
                &scene,
                &camera(),
                RenderOptions {
                    width,
                    height,
                    samples_per_pixel,
                },
                workers,
            )
            .is_err()
        );
    }
}

#[test]
fn rejects_overflowing_image_dimensions_without_allocating() {
    let result = render_with_workers(
        &Scene::new(Vec::new()),
        &camera(),
        RenderOptions {
            width: u32::MAX,
            height: u32::MAX,
            samples_per_pixel: 1,
        },
        1,
    );
    assert!(result.unwrap_err().to_string().contains("too large"));
}

#[test]
fn worker_panics_are_returned_as_errors() {
    let scene = Scene::new(vec![Arc::new(EmissiveSurface {
        samples: Arc::new(AtomicUsize::new(0)),
        panic_on_hit: true,
    })]);
    let result = render_with_workers(
        &scene,
        &camera(),
        RenderOptions {
            width: 3,
            height: 2,
            samples_per_pixel: 1,
        },
        4,
    );
    assert!(result.unwrap_err().to_string().contains("worker panicked"));
}
