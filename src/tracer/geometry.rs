use super::ray::*;
use super::material::*;
use cgmath::*;
use rand::*;

pub trait Hitable {
    fn hit(&self, r: &Ray, t_max: f64) -> f64;
    fn get_center(&self) -> &Vector3<f64>;
    fn get_radius(&self) -> f64;
    fn get_color(&self) -> &Vector3<f64>;
    fn get_material(&self) -> &Material;
}

pub struct Plane<'a> {
    origin: Vector3<f64>,
    normal: Vector3<f64>,
    color:  Vector3<f64>,
    mat:    &'a Material
}

impl<'a> Plane<'a> {
    pub fn new(org: Vector3<f64>, n: Vector3<f64>, col: Vector3<f64>, m: &'a Material) -> Plane<'a> {
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

    fn get_material(&self) -> &Material {
        self.mat
    }
}

pub struct Sphere<'a> {
    color: Vector3<f64>,
    radius: f64,
    center: Vector3<f64>,
    mat: &'a Material
}

impl<'a> Sphere<'a> {
    pub fn new(col: Vector3<f64>, r: f64, c: Vector3<f64>, m: &'a Material) -> Sphere<'a> {
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

    fn get_material(&self) -> &Material {
        self.mat
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

//Returns the distance to the closest object.
pub fn scene_intersection(ray: &Ray, table: &[&Hitable]) -> f64 {
    let mut t = std::f64::MAX;
    for hitable in table {
        let t2 = hitable.hit(ray, std::f64::MAX);
        if t > t2 && t2 != 0.0 {
            t = t2;
        }
    }

    if t == std::f64::MAX {
        t = 0.0;
    }
    //println!("t value: {}", t);
    t
}

pub fn color(ray: &Ray, table: &[&Hitable], depth: i64, t_max: f64) -> Vector3<f64> {
    let t = scene_intersection(ray, table);
    for hitable in table {
        let t2 = hitable.hit(ray, t_max);
        if t != t2 {
            //println!("t Value: {}", t);
            continue;
        }
        let p = ray.point_at_parameter(t2);
        let n = p - hitable.get_center();
        let n = n/n.magnitude();
        if t > 0.001 && depth < 5 {
            let col = color(&hitable.get_material().scatter(ray, &n, &p), &table, depth+1, t_max);
            return 0.5 * Vector3::new(hitable.get_color().x * col.x, hitable.get_color().y * col.y, hitable.get_color().z * col.z);
        } 
    }
    //let unit_d = ray.direction()/ray.direction().magnitude();
    //let t = 0.5 * (unit_d.y + 1.0);
    //(1.0 - t) * Vector3::new(1.0, 1.0, 1.0) + t * Vector3::new(0.5, 0.7, 1.0)
    Vector3::new(1.0, 1.0, 1.0)
}