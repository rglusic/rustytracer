use cgmath::*;

pub struct Light {
    origin: Vector3<f64>,
    intensity: f64
}

impl Light {
    pub fn new(org: Vector3<f64>, int: f64) -> Light {
        Light { origin: org, intensity: int }
    }

    pub fn origin(&self) -> &Vector3<f64> {
        &self.origin
    }
    
    pub fn intensity(&self) -> f64 {
        self.intensity
    }
}