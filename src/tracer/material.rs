use super::geometry;
use super::ray;
use cgmath::*;
use rand::RngExt;

use std::collections::HashMap as Map;
use std::sync::Arc;

pub trait Material: Send + Sync {
    fn scatter(&self, r: &ray::Ray, n: &Vector3<f64>, p: &Vector3<f64>) -> Option<ray::Ray>;
    fn emitted(&self) -> Vector3<f64> {
        Vector3::new(0.0, 0.0, 0.0)
    }
}

pub struct Flat {}

impl Material for Flat {
    fn scatter(&self, _: &ray::Ray, n: &Vector3<f64>, p: &Vector3<f64>) -> Option<ray::Ray> {
        let direction = n + geometry::rand_usphere();
        let direction = direction / direction.dot(direction).sqrt();
        Some(ray::Ray::new_from(*p, direction))
    }
}

pub struct Metal {}

impl Material for Metal {
    fn scatter(&self, r: &ray::Ray, n: &Vector3<f64>, p: &Vector3<f64>) -> Option<ray::Ray> {
        let reflected = reflect(&(r.direction() / r.direction().magnitude()), n);
        Some(ray::Ray::new_from(*p, reflected))
    }
}

pub struct Dielectric {
    ref_index: f64,
}

impl Dielectric {
    pub fn new(r: f64) -> Dielectric {
        Dielectric { ref_index: r }
    }
}

impl Material for Dielectric {
    fn scatter(&self, r: &ray::Ray, n: &Vector3<f64>, p: &Vector3<f64>) -> Option<ray::Ray> {
        let incoming = r.direction().normalize();
        let (normal, ratio) = if incoming.dot(*n) < 0.0 {
            (*n, 1.0 / self.ref_index)
        } else {
            (-*n, self.ref_index)
        };
        let cosine = (-incoming.dot(normal)).clamp(0.0, 1.0);
        let direction = match refract(&incoming, &normal, ratio) {
            Some(refracted) if rand::rng().random::<f64>() >= schlick(cosine, self.ref_index) => {
                refracted
            }
            // Total internal reflection cannot produce a transmitted ray.
            _ => reflect(&incoming, &normal),
        };
        Some(ray::Ray::new_from(*p, direction))
    }
}

fn reflect(v: &Vector3<f64>, n: &Vector3<f64>) -> Vector3<f64> {
    v - 2.0 * v.dot(*n) * n
}

fn refract(v: &Vector3<f64>, n: &Vector3<f64>, ni_over_nt: f64) -> Option<Vector3<f64>> {
    let c1 = -v.dot(*n);
    let discriminant = 1.0 - ni_over_nt * ni_over_nt * (1.0 - c1 * c1);
    if discriminant < 0.0 {
        return None;
    }
    Some((ni_over_nt * v) + (ni_over_nt * c1 - discriminant.sqrt()) * n)
}

fn schlick(cos: f64, index: f64) -> f64 {
    let r0 = (1.0 - index) / (1.0 + index);
    let r0 = r0 * r0;
    r0 + (1.0 - r0) * (1.0 - cos).powf(5.0)
}

pub struct DiffuseLight {
    light_color: Vector3<f64>,
}

impl DiffuseLight {
    pub fn new(r: f64, g: f64, b: f64) -> DiffuseLight {
        DiffuseLight {
            light_color: Vector3::new(r, g, b),
        }
    }
}

impl Material for DiffuseLight {
    fn scatter(&self, _: &ray::Ray, _: &Vector3<f64>, _: &Vector3<f64>) -> Option<ray::Ray> {
        None
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
        all_materials.insert("flat", Arc::new(Flat {}));
        all_materials.insert("metal", Arc::new(Metal {}));
        all_materials.insert("glass", Arc::new(Dielectric::new(1.5))); //Default to standard glass
        all_materials.insert(
            "diffuse_light",
            Arc::new(DiffuseLight::new(1.0 * 2.5, 1.0 * 2.5, 0.98431372549 * 2.5)),
        ); //Sunlight at 5400K

        MaterialsFactory {
            materials_list: all_materials,
        }
    }

    pub fn get_material_by_key(&self, material_type: &str) -> &dyn Material {
        let res = self.materials_list.get(material_type);
        match res {
            Some(result) => result.as_ref(),
            None => {
                eprintln!("Unknown material '{material_type}'; using flat material");
                self.materials_list["flat"].as_ref()
            }
        }
    }
}

#[cfg(test)]
#[path = "material_tests.rs"]
mod tests;
