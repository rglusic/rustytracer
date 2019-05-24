use super::geometry::*;
use super::light::*;
use super::ray::*;
use cgmath::*;

pub struct photon {
    intensity: f64,
    ray: Ray
}

impl photon {
    pub fn new(intensity: f64, ray: Ray) -> photon {
        photon { intensity: intensity, ray: ray }
    }
}

pub fn gen_photons(start: Vector3<f64>, num: i64)  -> Vec<photon> {
    let mut map: Vec<photon> = Vec::new();
    for i in 0..num {
        map.push(photon::new(1.0, ray::new_from(start, rand_usphere())));
    }
    map
}