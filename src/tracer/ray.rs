use cgmath::*;
//use cgmath::prelude::*;

pub struct Ray {
    a: Vector3<f64>,
    b: Vector3<f64>
}

impl Ray {
    pub fn new_from(a: Vector3<f64>, b: Vector3<f64>) -> Ray {
        Ray { a: a, b: b }
    }

    pub fn origin(&self) -> &Vector3<f64> {
        &self.a
    }

    pub fn direction(&self) -> &Vector3<f64> {
        &self.b
    }

    pub fn point_at_parameter(&self, t: f64) -> Vector3<f64> {
        self.a + t*self.b
    }
}