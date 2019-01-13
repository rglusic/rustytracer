use super::geometry::*;
use super::light::*;
use super::ray::*;
use cgmath::*;

pub struct Scene<'a> {
    renderables: &'a [&'a Hitable],
    lights: &'a [Light]
}

impl<'a> Scene<'a> {
    pub fn new(render_list: &'a [&'a Hitable], light_list: &'a [Light]) -> Scene<'a> {
        Scene { renderables: render_list, lights: light_list }
    }

    pub fn get_closest_intersection(&self, ray: &Ray) -> f64 {
        let mut t = std::f64::MAX;
        for hitable in self.renderables {
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

    pub fn render(&self, ray: &Ray, depth: i64, t_max: f64) -> Vector3<f64> {
        let t = self.get_closest_intersection(ray);
        for hitable in self.renderables {
            let t2 = hitable.hit(ray, t_max);
            if t != t2 {
                continue;
            }

            let p = ray.point_at_parameter(t2);
            let n = hitable.get_norm_at_p(&p);
            let mut attenuation: f64 = 0.0;
            for light in self.lights {
                let op = light.origin() - p;
                let op_mag = op.magnitude();
                let shadow = Ray::new_from(p, *light.origin());
                let closest = self.get_closest_intersection(&shadow);
                let shadow = shadow.point_at_parameter(closest);
                if shadow.magnitude() != op_mag {
                    attenuation = 0.1;
                    continue;
                }
                let falloff = light.intensity()*1.0/op.dot(op);
                attenuation += op.dot(n);
                attenuation *= falloff;
                attenuation = attenuation.max(0.1).min(1.0);
            }

            if depth < 10 {
                let col = self.render(&hitable.get_material().scatter(ray, &n, &p), depth+1, t_max);
                if col == Vector3::new(0.0, 0.0, 0.0) && *hitable.get_color() != Vector3::new(1.0, 1.0, 1.0) {
                    return attenuation * Vector3::new(hitable.get_color().x, hitable.get_color().y, hitable.get_color().z);
                }
                return attenuation * Vector3::new(hitable.get_color().x * col.x, hitable.get_color().y * col.y, hitable.get_color().z * col.z);
            }
        }
        Vector3::new(0.0, 0.0, 0.0)
    }
}