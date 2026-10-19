//! Next-event estimation and matching endpoint PDFs for multiple importance sampling.
use super::geometry::{Shape, Surface, abs, gamma};
use super::material::{Color, Material, attenuate, black, maximum, multiply, uniform_sphere};
use super::ray::Ray;
use super::sampler::Sampler;
use super::scene::{Scene, power_heuristic};
use cgmath::InnerSpace;
use std::f64::consts::PI;

impl Scene {
    fn light_count(&self) -> usize {
        self.lights.len() + usize::from(maximum(self.environment) > 0.)
    }
    pub(super) fn environment_pdf(&self) -> f64 {
        if maximum(self.environment) > 0. {
            1. / (4. * PI * self.light_count() as f64)
        } else {
            0.
        }
    }
    pub(super) fn light_pdf(&self, index: usize, from: Color, surface: Surface) -> f64 {
        if maximum(self.renderables[index].material().emission) == 0. {
            return 0.;
        }
        match self.renderables[index].shape() {
            Shape::Sphere { radius, .. } => {
                let delta = surface.point - from;
                let cosine = surface.normal.dot(-delta.normalize()).abs();
                if cosine == 0. {
                    0.
                } else {
                    delta.magnitude2()
                        / (4. * PI * radius * radius * cosine * self.light_count() as f64)
                }
            }
            Shape::Plane { .. } => 1. / (4. * PI * self.light_count() as f64),
        }
    }
    pub(super) fn direct_light(
        &self,
        surface: Surface,
        wo: Color,
        n: Color,
        material: &Material,
        absorption: Color,
        rng: &mut Sampler,
    ) -> Color {
        let count = self.light_count();
        if count == 0 {
            return black();
        }
        let selection = (rng.next() * count as f64) as usize;
        let mut pdf = 1. / (4. * PI * count as f64);
        let (wi, emitted, distance) = if selection == self.lights.len() {
            let wi = uniform_sphere(rng.next(), rng.next());
            if self
                .intersect(&Ray::new_from(surface.offset(wi), wi), f64::INFINITY)
                .is_some()
            {
                return black();
            }
            (wi, self.environment, f64::INFINITY)
        } else {
            let index = self.lights[selection];
            let light = &self.renderables[index];
            match light.shape() {
                Shape::Sphere { center, radius } => {
                    let normal = uniform_sphere(rng.next(), rng.next());
                    let point = center + radius * normal;
                    let delta = point - surface.point;
                    let distance = delta.magnitude();
                    if distance == 0. {
                        return black();
                    }
                    let wi = delta / distance;
                    let cosine = normal.dot(-wi).abs();
                    if cosine == 0. {
                        return black();
                    }
                    pdf *= distance * distance / (radius * radius * cosine);
                    let target = Surface {
                        point,
                        normal,
                        error: (abs(center) + abs(point) + Color::new(radius, radius, radius))
                            * gamma(16),
                    };
                    let from = surface.offset(wi);
                    let to = target.offset(-wi);
                    let shadow = to - from;
                    if self
                        .intersect(&Ray::new_from(from, shadow.normalize()), shadow.magnitude())
                        .is_some()
                    {
                        return black();
                    }
                    (wi, light.material().emitted(normal, -wi), distance)
                }
                Shape::Plane { .. } => {
                    let wi = uniform_sphere(rng.next(), rng.next());
                    let Some(hit) =
                        self.intersect(&Ray::new_from(surface.offset(wi), wi), f64::INFINITY)
                    else {
                        return black();
                    };
                    if hit.index != index {
                        return black();
                    }
                    (
                        wi,
                        light.material().emitted(hit.surface.normal, -wi),
                        hit.distance,
                    )
                }
            }
        };
        let (value, bsdf_pdf) = material.evaluate(wo, wi, n);
        if bsdf_pdf == 0. {
            return black();
        }
        let transmission = if distance.is_finite() {
            attenuate(absorption, distance)
        } else {
            Color::new(
                if absorption.x == 0. { 1. } else { 0. },
                if absorption.y == 0. { 1. } else { 0. },
                if absorption.z == 0. { 1. } else { 0. },
            )
        };
        multiply(multiply(value, emitted), transmission)
            * (n.dot(wi).abs() * power_heuristic(pdf, bsdf_pdf) / pdf)
    }
}
