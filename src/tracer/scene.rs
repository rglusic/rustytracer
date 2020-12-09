use super::geometry::*;
use super::light::*;
use super::ray::*;
use super::material::*;
use cgmath::*;
use std::sync::Arc;
extern crate rand;

use rand::*;

pub struct Scene {
    renderables: Vec<Arc<dyn Hitable>>,
    materials: MaterialsFactory,
}

impl Scene {
    pub fn new(render_list: Vec<Arc<dyn Hitable>>) -> Scene {
        Scene { renderables: render_list, materials: MaterialsFactory::new() }
    }

    pub fn get_closest_intersection(&self, ray: &Ray) -> f64 {
        let mut t = std::f64::MAX;
        for hitable in &*self.renderables {
            let t2 = hitable.hit(ray, std::f64::MAX);
            if t > t2 && t2 != 0.0 {
                t = t2;
            }
        }

        if t == std::f64::MAX {
            t = -1.0;
        }
        t        
    }

    //fn _shadow_march(&self, p: &Vector3<f64>, n: &Vector3<f64>) -> f64 {
    //    let mut attenuation: f64 = 0.0;
    //    for light in &*self.lights.clone() {
    //        let op = light.origin() - *p;
    //        let op_mag = op.magnitude();
    //        let shadow = Ray::new_from(*p, *light.origin());
    //        let closest = self.get_closest_intersection(&shadow);
    //        let shadow = shadow.point_at_parameter(closest);
    //        if shadow.magnitude() != op_mag {
    //            attenuation = 0.1;
    //            continue;
    //        }
    //        let falloff = light.intensity()/op.dot(op);
    //        attenuation += op.dot(*n);
    //        attenuation *= falloff;
    //        attenuation = attenuation.max(0.1).min(1.0);
    //    }
    //    attenuation
    //}

    pub fn render(&self, ray: &Ray, depth: i64, t_max: f64) -> Vector3<f64> {
        //Prevent infinite loops.
        if depth > 3 {
            let rng = rand::thread_rng().gen::<f64>();
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
            let emitted = self.materials.get_material_by_key(hitable.get_material()).emitted();

            let (new_ray, scattered) = self.materials.get_material_by_key(hitable.get_material()).scatter(ray, &n, &p);

            let col = self.render(&new_ray, depth+1, t_max);
            return emitted + Vector3::new(hitable.get_color().x * col.x, hitable.get_color().y * col.y, hitable.get_color().z * col.z);
        }
        return Vector3::new(0.0,0.0,0.0);
    }

}