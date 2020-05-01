use super::ray::*;
use super::material::*;
use cgmath::*;
use rand::*;

pub trait Hitable {
    fn hit(&self, r: &Ray, t_max: f64) -> f64;
    fn get_center(&self) -> &Vector3<f64>;
    fn get_radius(&self) -> f64;
    fn get_color(&self) -> &Vector3<f64>;
    fn get_material(&self) -> &dyn Material;
    fn get_norm_at_p(&self, p: &Vector3<f64>) -> Vector3<f64>;
}

pub struct Plane<'a> {
    origin: Vector3<f64>,
    normal: Vector3<f64>,
    color:  Vector3<f64>,
    mat:    &'a dyn Material
}

impl<'a> Plane<'a> {
    pub fn new(org: Vector3<f64>, n: Vector3<f64>, col: Vector3<f64>, m: &'a dyn Material) -> Plane<'a> {
        Plane { origin: org, normal: n, color: col, mat: m }
    }
}

impl<'a> Hitable for Plane<'a> {
    fn hit(&self, r: &Ray, t_max: f64) -> f64 {
        let denom = r.direction().dot(self.normal);
        if denom.abs() > 0.001 {
            let t = (self.origin - r.origin()).dot(self.normal)/denom;
            if t >= 0.001 && t < t_max {
                return t;
            }
        }
        0.0
    }

    fn get_center(&self) -> &Vector3<f64> {
        &self.origin
    }

    fn get_radius(&self) -> f64 {
        0.0
    }

    fn get_color(&self) -> &Vector3<f64> {
        &self.color
    }

    fn get_material(&self) -> & dyn Material {
        self.mat
    }

    fn get_norm_at_p(&self, _: &Vector3<f64>) -> Vector3<f64> {
        self.normal
    }
}

pub struct Sphere<'a> {
    color: Vector3<f64>,
    radius: f64,
    center: Vector3<f64>,
    mat: &'a dyn Material
}

impl<'a> Sphere<'a> {
    pub fn new(col: Vector3<f64>, r: f64, c: Vector3<f64>, m: &'a dyn Material) -> Sphere<'a> {
        Sphere { color: col, radius: r, center: c, mat: m }
    }
}

//https://en.wikipedia.org/wiki/Line%E2%80%93sphere_intersection
impl<'a> Hitable for Sphere<'a> {
    fn hit(&self, r: &Ray, t_max: f64) -> f64 {
        let oc = r.origin() - self.center;
        let a = r.direction().dot(*r.direction());
        let b = oc.dot(*r.direction());
        let c = oc.dot(oc) - (self.radius*self.radius);
        let discriminant = b*b - a*c;
        let t = (-b - discriminant.sqrt())/a;
        if (t < t_max) && (t > 0.001) {
            return t;
        }
        let t = (-b + discriminant.sqrt())/a;
        if (t < t_max) && (t > 0.001) {
            return t;
        }
        0.0
    }

    fn get_center(&self) -> &Vector3<f64> {
        &self.center
    }

    fn get_radius(&self) -> f64 {
        self.radius
    }

    fn get_color(&self) -> &Vector3<f64> {
        &self.color
    }

    fn get_material(&self) -> & dyn Material {
        self.mat
    }

    fn get_norm_at_p(&self, p: &Vector3<f64>) -> Vector3<f64> {
        let n = p - self.center;
        n/n.magnitude()
    }
}

pub fn rand_usphere() -> Vector3<f64> {
    let mut rng = rand::thread_rng();
    let mut p: Vector3<f64> = 2.0 * Vector3::new(rng.gen(), rng.gen(), rng.gen()) - Vector3::new(1.0, 1.0, 1.0);
    
    while p.dot(p) >= 1.0 {
        p = 2.0 * Vector3::new(rng.gen(), rng.gen(), rng.gen()) - Vector3::new(1.0, 1.0, 1.0);
    }
    p
}