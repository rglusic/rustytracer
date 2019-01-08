use super::ray::*;
use super::material;
use cgmath::*;
use rand::*;

pub trait Hitable {
    fn hit(&self, r: &Ray, t_max: f64) -> f64;
}

pub struct Sphere<'a> {
    color: Vector3<f64>,
    radius: f64,
    center: Vector3<f64>,
    mat: &'a material::Material,
    inter: f64,
}

impl<'a> Sphere<'a> {
    pub fn new(col: Vector3<f64>, r: f64, c: Vector3<f64>, m: &'a material::Material) -> Sphere<'a> {
        Sphere { color: col, radius: r, center: c, mat: m, inter: 0.0 }
    }

    pub fn get_center(&self) -> &Vector3<f64> {
        &self.center
    }

    pub fn get_radius(&self) -> f64 {
        self.radius
    }

    pub fn get_color(&self) -> &Vector3<f64> {
        &self.color
    }

    pub fn get_inter(&self) -> f64 {
        self.inter
    }
}

impl<'a> Hitable for Sphere<'a> {
    fn hit(&self, r: &Ray, t_max: f64) -> f64 {
        let oc = r.origin() - self.center;
        let a = r.direction().dot(*r.direction());
        let b = oc.dot(*r.direction());
        let c = oc.dot(oc) - (self.radius*self.radius);
        let discriminant = b*b - a*c;
        let t = (-b - discriminant.sqrt())/a;
        if (t < t_max) && (t > 0.001) {
            //self.inter = t;
            return t;
        }
        let t = (-b + discriminant.sqrt())/a;
        if (t < t_max) && (t > 0.001) {
            //self.inter = t;
            return t;
        }
        0.0
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
pub fn scene_intersection(ray: &Ray, table: &[Sphere]) -> f64 {
    let mut t = std::f64::MAX;
    for sphere in table {
        let t2 = sphere.hit(ray, std::f64::MAX);
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

pub fn color(ray: &Ray, table: &[Sphere], depth: i64, t_max: f64) -> Vector3<f64> {
    let t = scene_intersection(ray, table);
    for sphere in table {
        let t2 = sphere.hit(ray, t_max);
        if t != t2 {
            //println!("t Value: {}", t);
            continue;
        }
        let p = ray.point_at_parameter(t2);
        let n = p - sphere.get_center();
        let n = n/n.magnitude();
        if t > 0.001 && depth < 5 {
            let col = color(&sphere.mat.scatter(ray, &n, &p), &table, depth+1, t_max);
            return 0.5 * Vector3::new(sphere.get_color().x * col.x, sphere.get_color().y * col.y, sphere.get_color().z * col.z);
        } 
    }
    //let unit_d = ray.direction()/ray.direction().magnitude();
    //let t = 0.5 * (unit_d.y + 1.0);
    //(1.0 - t) * Vector3::new(1.0, 1.0, 1.0) + t * Vector3::new(0.5, 0.7, 1.0)
    Vector3::new(1.0, 1.0, 1.0)
}