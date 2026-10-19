use super::Settings;
use super::geometry::{Hitable, Shape, Surface};
use super::material::{Color, Kind, attenuate, black, maximum, multiply, white};
use super::ray::Ray;
use super::sampler::Sampler;
use cgmath::InnerSpace;
use std::sync::Arc;

pub struct Scene {
    pub(super) renderables: Vec<Arc<dyn Hitable>>,
    pub(super) lights: Vec<usize>,
    pub(super) environment: Color,
}
pub(super) struct Intersection {
    pub index: usize,
    pub distance: f64,
    pub surface: Surface,
}
impl Scene {
    #[cfg(test)]
    pub fn new(renderables: Vec<Arc<dyn Hitable>>) -> Self {
        Self::with_environment(renderables, black())
    }
    pub fn with_environment(renderables: Vec<Arc<dyn Hitable>>, environment: Color) -> Self {
        let lights = renderables
            .iter()
            .enumerate()
            .filter_map(|(i, o)| (maximum(o.material().emission) > 0.).then_some(i))
            .collect();
        Self {
            renderables,
            lights,
            environment,
        }
    }
    pub(super) fn intersect(&self, ray: &Ray, maximum: f64) -> Option<Intersection> {
        let mut closest = maximum;
        let mut index = None;
        for (i, object) in self.renderables.iter().enumerate() {
            let distance = object.hit(ray, closest);
            if distance > 0. && distance < closest {
                closest = distance;
                index = Some(i);
            }
        }
        index.map(|index| Intersection {
            index,
            distance: closest,
            surface: self.renderables[index].surface(ray, closest),
        })
    }
    /// Visibility needs only a blocker, not its shading surface or the closest hit.
    pub(super) fn occluded(&self, ray: &Ray, maximum: f64) -> bool {
        self.renderables.iter().any(|object| {
            let distance = object.hit(ray, maximum);
            distance > 0. && distance < maximum
        })
    }
    /// Nested spheres have a unique smallest containing volume. No fixed-depth medium stack.
    pub(super) fn medium(&self, point: Color, excluded: Option<usize>) -> (f64, Color) {
        let mut radius = f64::INFINITY;
        let mut medium = (1., black());
        for (i, object) in self.renderables.iter().enumerate() {
            let m = object.material();
            if m.kind != Kind::Dielectric || excluded == Some(i) {
                continue;
            }
            if let Shape::Sphere { center, radius: r } = object.shape()
                && r < radius
                && (point - center).magnitude2() < r * r
            {
                radius = r;
                medium = (m.ior, m.absorption);
            }
        }
        medium
    }
    pub fn render(&self, primary: &Ray, settings: Settings, rng: &mut Sampler) -> Color {
        let mut ray = Ray::new_from(*primary.origin(), primary.direction().normalize());
        let mut beta = white();
        let mut radiance = black();
        let mut eta_scale = 1.;
        let mut depth = 0u64;
        let mut previous_pdf = 0.;
        let mut previous_delta = true;
        let mut previous_point = *primary.origin();
        loop {
            let (eta_i, absorption) = self.medium(*ray.origin(), None);
            let Some(hit) = self.intersect(&ray, f64::INFINITY) else {
                let weight = if previous_delta || !settings.light_sampling {
                    1.
                } else {
                    power_heuristic(previous_pdf, self.environment_pdf())
                };
                radiance += multiply(beta, self.environment) * weight;
                break;
            };
            beta = multiply(beta, attenuate(absorption, hit.distance));
            let m = self.renderables[hit.index].material().in_medium(eta_i);
            let surface = hit.surface;
            let wo = -*ray.direction();
            let front = surface.normal.dot(wo) > 0.;
            let n = if front {
                surface.normal
            } else {
                -surface.normal
            };
            let weight = if previous_delta || !settings.light_sampling {
                1.
            } else {
                power_heuristic(
                    previous_pdf,
                    self.light_pdf(hit.index, previous_point, surface),
                )
            };
            radiance += multiply(beta, m.emitted(surface.normal, wo)) * weight;
            if m.kind == Kind::Emitter
                || (settings.max_bounces != 0 && depth >= u64::from(settings.max_bounces))
            {
                break;
            }
            if settings.light_sampling && !m.is_delta() {
                radiance += multiply(beta, self.direct_light(surface, wo, n, &m, absorption, rng));
            }
            let eta_t = if front {
                m.ior
            } else {
                self.medium(surface.point, Some(hit.index)).0
            };
            let Some(sample) = m.sample(wo, n, eta_i, eta_t, rng) else {
                break;
            };
            if sample.pdf <= 0. {
                break;
            }
            beta = multiply(beta, sample.weight(n));
            if maximum(beta) <= 0. {
                break;
            }
            if sample.transmission {
                eta_scale *= sample.eta * sample.eta;
            }
            depth += 1;
            // The cap below changes variance, not expectation: survivors divide by survival.
            if depth >= 4 {
                let survival = (maximum(beta) * eta_scale).min(0.95);
                if rng.next() >= survival {
                    break;
                }
                beta /= survival;
            }
            previous_delta = sample.delta;
            previous_pdf = sample.pdf;
            previous_point = surface.point;
            ray = Ray::new_from(surface.offset(sample.wi), sample.wi.normalize());
        }
        radiance
    }
}
pub fn power_heuristic(a: f64, b: f64) -> f64 {
    if a == 0. {
        return 0.;
    }
    // Ratio form avoids squaring potentially very large solid-angle PDFs.
    let ratio = b / a;
    1. / (1. + ratio * ratio)
}

#[cfg(test)]
#[path = "scene_tests.rs"]
mod tests;
