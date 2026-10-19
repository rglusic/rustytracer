use super::geometry::Hitable;
use super::material::MaterialsFactory;
use super::ray::Ray;
use super::sampler::Sampler;
use cgmath::Vector3;
use std::sync::Arc;

pub struct Scene {
    renderables: Vec<Arc<dyn Hitable>>,
    materials: MaterialsFactory,
}

impl Scene {
    pub fn new(renderables: Vec<Arc<dyn Hitable>>) -> Self {
        Self {
            renderables,
            materials: MaterialsFactory::new(),
        }
    }

    pub fn render(&self, primary: &Ray, max_bounces: u32, rng: &mut Sampler) -> Vector3<f64> {
        let mut ray = Ray::new_from(*primary.origin(), *primary.direction());
        let mut throughput = Vector3::new(1.0, 1.0, 1.0);
        for depth in 0..max_bounces {
            // Preserve the existing integrator's termination rule in both backends.
            if depth > 3 && rng.next() > 0.3 {
                break;
            }
            let mut closest = f64::MAX;
            let mut object = None;
            for hitable in &self.renderables {
                let distance = hitable.hit(&ray, closest);
                if distance > 0.0 && distance < closest {
                    closest = distance;
                    object = Some(hitable);
                }
            }
            let Some(object) = object else { break };
            let point = ray.point_at_parameter(closest);
            let normal = object.get_norm_at_p(&point);
            let material = self.materials.get_material_by_key(object.get_material());
            let Some(scattered) = material.scatter(&ray, &normal, &point, rng) else {
                let emitted = material.emitted();
                return Vector3::new(
                    throughput.x * emitted.x,
                    throughput.y * emitted.y,
                    throughput.z * emitted.z,
                );
            };
            let color = object.get_color();
            throughput = Vector3::new(
                throughput.x * color.x,
                throughput.y * color.y,
                throughput.z * color.z,
            );
            ray = scattered;
        }
        Vector3::new(0.0, 0.0, 0.0)
    }
}
