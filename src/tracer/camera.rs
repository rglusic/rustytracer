use cgmath::*;
//use cgmath::prelude::*;
use super::ray;
use std::f64;

pub struct Camera {
    origin: Vector3<f64>,
    lower_left_corner: Vector3<f64>,
    horizontal: Vector3<f64>,
    vertical: Vector3<f64>
}

impl Camera {
    pub fn new(lookfrom: Vector3<f64>, lookat: Vector3<f64>, vup: Vector3<f64>, vfov: f64, aspect: f64) -> Camera {
        let theta = (vfov*f64::consts::PI/180.0)/2.0;
        let half_height = theta.tan();
        let half_width = aspect * half_height;

        let w = lookfrom - lookat;
        let w = w/w.magnitude();
        let u = vup.cross(w);
        let u = u/u.magnitude();
        let v = -w.cross(u); //Negative due to the coordinate system used by image formats being used.

        Camera { 
            lower_left_corner: lookfrom - half_width*u - half_height*v - w,
            horizontal: 2.0*half_width*u,
            vertical: 2.0*half_height*v,
            origin: lookfrom
        }
    }

    pub fn get_ray(&self, u: f64, v: f64) -> ray::Ray {
        ray::Ray::new_from(self.origin, self.lower_left_corner + u*self.horizontal + v*self.vertical - self.origin)
    }
}