use cgmath::*;
use super::ray;
use super::geometry;
use rand::*;

pub trait Material {
    fn scatter(&self, r: &ray::Ray, n: &Vector3<f64>, p: &Vector3<f64>) -> ray::Ray;
}

pub struct Flat {}

impl Material for Flat {
    fn scatter(&self, _: &ray::Ray, n: &Vector3<f64>, p: &Vector3<f64>) -> ray::Ray {
        let target = p + n + geometry::rand_usphere();
        ray::Ray::new_from(*p, target-p)
    }
}

pub struct Metal {}

impl Material for Metal {
    fn scatter(&self, r: &ray::Ray, n: &Vector3<f64>, p: &Vector3<f64>) -> ray::Ray {
        let reflected = reflect(&(r.direction() / r.direction().magnitude()), n);
        ray::Ray::new_from(*p, reflected)
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
    fn scatter(&self, r: &ray::Ray, n: &Vector3<f64>, p: &Vector3<f64>) -> ray::Ray {
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
            //return ray::Ray::new_from(*p, reflection);
        }
        
        ray::Ray::new_from(*p, refraction)
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