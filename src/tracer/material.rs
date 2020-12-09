use cgmath::*;
use super::ray;
use super::geometry;
use rand::*;

use std::sync::Arc;
use std::collections::HashMap as Map;
use std::error::Error;

pub trait Material: Send + Sync {
    fn scatter(&self, r: &ray::Ray, n: &Vector3<f64>, p: &Vector3<f64>) -> (ray::Ray, f64);
    fn emitted(&self) -> Vector3<f64> {
        Vector3::new(0.0,0.0,0.0)
    }
    fn importance_scatter(&self, r_in: &ray::Ray, r_scatter: &ray::Ray) -> f64 {
        return 0.0;
    }
}

pub struct Flat {}

impl Material for Flat {
    fn scatter(&self, r: &ray::Ray, n: &Vector3<f64>, p: &Vector3<f64>) -> (ray::Ray, f64) {
        let direction = n + geometry::rand_usphere();
        let direction = direction/direction.dot(direction).sqrt();
        let pdf = n.dot(*r.direction())/3.14159;
        (ray::Ray::new_from(*p, direction), pdf)
    }
    fn importance_scatter(&self, r_in: &ray::Ray, r_scatter: &ray::Ray) -> f64 {
        return 0.0;
    }
}

pub struct Metal {}

impl Material for Metal {
    fn scatter(&self, r: &ray::Ray, n: &Vector3<f64>, p: &Vector3<f64>) -> (ray::Ray, f64) {
        let reflected = reflect(&(r.direction() / r.direction().magnitude()), n);
        (ray::Ray::new_from(*p, reflected), 0.0)
    }
}

pub struct Dielectric {
    ref_index: f64
}

impl Dielectric {
    pub fn new(r: f64) -> Dielectric {
        Dielectric { ref_index: r }
    }
}

impl Material for Dielectric {
    fn scatter(&self, r: &ray::Ray, n: &Vector3<f64>, p: &Vector3<f64>) -> (ray::Ray, f64) {
        let ni_over_nt: f64;
        let outward_normal: Vector3<f64>;
        let cos: f64;
        let reflection = reflect(r.direction(), n);
        //in
        if r.direction().dot(*n) < 0.0 {
            outward_normal = *n;
            ni_over_nt = 1.0 / self.ref_index;
            cos = -1.0 * r.direction().dot(*n)/r.direction().magnitude();
        } else { //out
            outward_normal = -1.0 * n;
            ni_over_nt = self.ref_index;
            cos = self.ref_index * r.direction().dot(*n)/r.direction().magnitude();
        }
        let refraction = refract(r.direction(), &outward_normal, ni_over_nt);
        let reflect_prob: f64 = schlick(cos, self.ref_index);
        let mut rng = rand::thread_rng();
        let rng: f64 = rng.gen();

        if rng < reflect_prob {
            return (ray::Ray::new_from(*p, reflection), 0.0);
        }
        
        (ray::Ray::new_from(*p, refraction), 0.0)
    }
}

fn reflect(v: &Vector3<f64>, n: &Vector3<f64>) -> Vector3<f64> {
    v - 2.0 * v.dot(*n) * n
}

fn refract(v: &Vector3<f64>, n: &Vector3<f64>, ni_over_nt: f64) -> Vector3<f64> {
    let c1 = -1.0 * v.dot(*n);
    let c2 = (1.0 - ni_over_nt*ni_over_nt * (1.0 - c1*c1)).sqrt();
    (ni_over_nt * v) + (ni_over_nt * c1 - c2) * n
}

fn schlick(cos: f64, index: f64) -> f64 {
    let r0 = (1.0-index)/(1.0+index);
    let r0 = r0*r0;
    r0 + (1.0-r0)*(1.0-cos).powf(5.0)
}

pub struct DiffuseLight {
    light_color: Vector3<f64>,
}

impl DiffuseLight {
    pub fn new(r:f64, g: f64, b: f64) -> DiffuseLight {
        DiffuseLight {light_color: Vector3::new(r,g,b)}
    }
}

impl Material for DiffuseLight {
    fn scatter(&self, _: &ray::Ray, _: &Vector3<f64>, _: &Vector3<f64>) -> (ray::Ray, f64) {
        //Terminate ray at light source.
        (ray::Ray::new_from(Vector3::new(0.0,0.0,0.0), Vector3::new(0.0,0.0,0.0)), 0.0)
    }
    fn emitted(&self) -> Vector3<f64> {
        self.light_color
    }
}

pub struct MaterialsFactory {
    materials_list: Map<&'static str, Arc<dyn Material>>,
}

impl MaterialsFactory {
    pub fn new() -> MaterialsFactory {
        let mut all_materials: Map<&'static str, Arc<dyn Material>> = Map::new();
        all_materials.insert("flat", Arc::new(Flat{}));
        all_materials.insert("metal", Arc::new(Metal{}));
        all_materials.insert("glass", Arc::new(Dielectric::new(1.5))); //Default to standard glass
        all_materials.insert("diffuse_light", Arc::new(DiffuseLight::new(1.0*2.5, 1.0*2.5, 0.98431372549*2.5))); //Sunlight at 5400K

        MaterialsFactory {materials_list: all_materials}
    }

    pub fn get_material_by_key(&self, material_type: String) -> Arc<dyn Material> {
        let res = self.materials_list.get(material_type.as_str());
        match res {
            Some(result) => {
                result.clone()
            },
            None => {
                println!("Error, material '{}' not found. Returning flat material", material_type);
                self.materials_list.get("flat").unwrap().clone()
            }
        }
    }
}