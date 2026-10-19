use crate::tracer::world::World;
use crate::tracer::{Result, Settings};
use bytemuck::{Pod, Zeroable};
use cgmath::{InnerSpace, Vector3};
use std::io;

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Sphere {
    pub center_radius: [f32; 4],
    pub color: [f32; 4],
    pub material: [u32; 4],
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Plane {
    pub origin: [f32; 4],
    pub normal: [f32; 4],
    pub color: [f32; 4],
    pub material: [u32; 4],
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Parameters {
    pub camera: [[f32; 4]; 4],
    pub width: u32,
    pub height: u32,
    pub sample_start: u32,
    pub sample_count: u32,
    pub sphere_count: u32,
    pub plane_count: u32,
    pub seed: u32,
    pub max_bounces: u32,
}

pub struct SceneData {
    pub spheres: Vec<Sphere>,
    pub planes: Vec<Plane>,
    pub parameters: Parameters,
}

impl SceneData {
    pub fn new(world: &World, settings: Settings) -> Result<Self> {
        let description = &world.description;
        let mut camera = [[0.0; 4]; 4];
        for (packed, vector) in camera.iter_mut().zip(world.get_camera().vectors()) {
            *packed = pack(vector, 0.0)?;
        }
        let center_direction = Vector3::new(
            camera[1][0] + 0.5 * camera[2][0] + 0.5 * camera[3][0] - camera[0][0],
            camera[1][1] + 0.5 * camera[2][1] + 0.5 * camera[3][1] - camera[0][1],
            camera[1][2] + 0.5 * camera[2][2] + 0.5 * camera[3][2] - camera[0][2],
        );
        if !center_direction.magnitude2().is_normal() {
            return Err(io::Error::other(
                "Camera direction loses precision in Metal's f32 representation",
            )
            .into());
        }
        let spheres = description
            .spheres
            .iter()
            .map(|sphere| {
                let center_radius = pack(sphere.center, sphere.radius)?;
                if center_radius[3] <= 0.0 || !(center_radius[3] * center_radius[3]).is_normal() {
                    return Err(
                        io::Error::other("Sphere radius is outside Metal's f32 range").into(),
                    );
                }
                Ok(Sphere {
                    center_radius,
                    color: pack(sphere.color, 0.0)?,
                    material: [material_id(&sphere.mat), 0, 0, 0],
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let planes = description
            .planes
            .iter()
            .map(|plane| {
                let normal = Vector3::from(plane.normal).normalize();
                Ok(Plane {
                    origin: pack(plane.origin, 0.0)?,
                    normal: pack(normal.into(), 0.0)?,
                    color: pack(plane.color, 0.0)?,
                    material: [material_id(&plane.mat), 0, 0, 0],
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            spheres,
            planes,
            parameters: Parameters {
                camera,
                width: settings.image.width,
                height: settings.image.height,
                sample_start: 0,
                sample_count: 0,
                sphere_count: description.spheres.len().try_into()?,
                plane_count: description.planes.len().try_into()?,
                seed: settings.seed,
                max_bounces: settings.max_bounces,
            },
        })
    }
}

fn pack(vector: [f64; 3], fourth: f64) -> Result<[f32; 4]> {
    let packed = [
        vector[0] as f32,
        vector[1] as f32,
        vector[2] as f32,
        fourth as f32,
    ];
    if packed.iter().any(|value| !value.is_finite()) {
        return Err(io::Error::other("Scene contains a value outside Metal's f32 range").into());
    }
    Ok(packed)
}

fn material_id(name: &str) -> u32 {
    match name {
        "metal" => 1,
        "glass" => 2,
        "diffuse_light" => 3,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_values_outside_gpu_range() {
        assert!(pack([1e100, 0.0, 0.0], 0.0).is_err());
        assert!(pack([f64::NAN, 0.0, 0.0], 0.0).is_err());
    }
}
