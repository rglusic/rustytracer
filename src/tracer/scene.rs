use super::geometry::*;
use super::material::*;
use super::ray::*;
use cgmath::*;
use rand::RngExt;
use std::sync::Arc;

pub struct Scene {
    renderables: Vec<Arc<dyn Hitable>>,
    materials: MaterialsFactory,
}

impl Scene {
    pub fn new(render_list: Vec<Arc<dyn Hitable>>) -> Scene {
        Scene {
            renderables: render_list,
            materials: MaterialsFactory::new(),
        }
    }

    pub fn get_closest_intersection(&self, ray: &Ray) -> f64 {
        let mut t = f64::MAX;
        for hitable in &*self.renderables {
            let t2 = hitable.hit(ray, f64::MAX);
            if t > t2 && t2 != 0.0 {
                t = t2;
            }
        }

        if t == f64::MAX {
            t = -1.0;
        }
        t
    }

    pub fn render(&self, ray: &Ray, depth: i64, t_max: f64) -> Vector3<f64> {
        // Bound recursion even if random termination repeatedly keeps a path alive.
        if depth >= 64 {
            return Vector3::new(0.0, 0.0, 0.0);
        }
        if depth > 3 {
            let rng = rand::rng().random::<f64>();
            if rng > 0.3 {
                return Vector3::new(0.0, 0.0, 0.0);
            }
        }

        let t = self.get_closest_intersection(ray);
        for hitable in &*self.renderables {
            let t2 = hitable.hit(ray, t_max);
            if t != t2 {
                continue;
            }

            let p = ray.point_at_parameter(t2);
            let n = hitable.get_norm_at_p(&p);
            let material = self.materials.get_material_by_key(hitable.get_material());
            let emitted = material.emitted();
            let Some(new_ray) = material.scatter(ray, &n, &p) else {
                return emitted;
            };

            let col = self.render(&new_ray, depth + 1, t_max);
            return emitted
                + Vector3::new(
                    hitable.get_color().x * col.x,
                    hitable.get_color().y * col.y,
                    hitable.get_color().z * col.z,
                );
        }
        Vector3::new(0.0, 0.0, 0.0)
    }
}
