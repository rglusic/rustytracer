use crate::tracer::material::{Kind, Material as CpuMaterial};
use crate::tracer::world::World;
use crate::tracer::{Result, Settings};
use bytemuck::{Pod, Zeroable};
use cgmath::{InnerSpace, Vector3};
use std::io;

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Material {
    pub albedo: [f32; 4],
    pub emission: [f32; 4],
    pub eta_roughness: [f32; 4],
    pub k_ior: [f32; 4],
    pub absorption: [f32; 4],
    pub kind: u32,
    pub two_sided: u32,
    pub parent_medium: u32,
    pub reserved: u32,
}
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Sphere {
    pub center_radius: [f32; 4],
    pub material: Material,
}
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Plane {
    pub origin: [f32; 4],
    pub normal: [f32; 4],
    pub material: Material,
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
    pub environment: [f32; 4],
    pub reset: u32,
    pub light_count: u32,
    pub light_sampling: u32,
    pub camera_medium: u32,
}
/// Persistent per-pixel continuation, exactly matching the Metal PathState.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct PathState {
    pub origin: [f32; 4],
    pub direction: [f32; 4],
    pub beta: [f32; 4],
    pub radiance: [f32; 4],
    pub previous_point: [f32; 4],
    pub rng: u32,
    pub depth: u32,
    pub completed_samples: u32,
    pub previous_delta: u32,
    pub medium_id: u32,
    pub reserved_medium: [u32; 3],
    pub eta_scale: f32,
    pub previous_pdf: f32,
    pub reserved_weights: [f32; 2],
}
pub struct SceneData {
    pub spheres: Vec<Sphere>,
    pub planes: Vec<Plane>,
    pub lights: Vec<[u32; 4]>,
    pub parameters: Parameters,
}
impl SceneData {
    pub fn new(world: &World, settings: Settings) -> Result<Self> {
        let d = &world.description;
        let mut camera = [[0.; 4]; 4];
        for (packed, v) in camera.iter_mut().zip(world.get_camera().vectors()) {
            *packed = pack(v, 0.)?;
        }
        let center_direction = Vector3::new(
            camera[1][0] + 0.5 * camera[2][0] + 0.5 * camera[3][0] - camera[0][0],
            camera[1][1] + 0.5 * camera[2][1] + 0.5 * camera[3][1] - camera[0][1],
            camera[1][2] + 0.5 * camera[2][2] + 0.5 * camera[3][2] - camera[0][2],
        );
        if !center_direction.magnitude2().is_normal() {
            return Err(io::Error::other("Camera direction loses precision in Metal f32").into());
        }
        let mut spheres = d
            .spheres
            .iter()
            .map(|s| {
                let center_radius = pack(s.center, s.radius)?;
                let r = center_radius[3];
                if r <= 0.
                    || !(r * r).is_normal()
                    || center_radius[..3]
                        .iter()
                        .any(|c| *c + r == *c || *c - r == *c)
                {
                    return Err(io::Error::other(
                        "Sphere is outside representable Metal f32 precision",
                    )
                    .into());
                }
                Ok(Sphere {
                    center_radius,
                    material: material(s.mat.resolve(s.color)?)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let planes = d
            .planes
            .iter()
            .map(|p| {
                Ok(Plane {
                    origin: pack(p.origin, 0.)?,
                    normal: pack(Vector3::from(p.normal).normalize().into(), 0.)?,
                    material: material(p.mat.resolve(p.color)?)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let mut camera_medium = 0;
        let mut camera_radius = f64::INFINITY;
        for i in 0..spheres.len() {
            if spheres[i].material.kind != Kind::Dielectric as u32 {
                continue;
            }
            let a = &d.spheres[i];
            let packed_center = Vector3::new(
                spheres[i].center_radius[0],
                spheres[i].center_radius[1],
                spheres[i].center_radius[2],
            );
            let packed_radius = spheres[i].center_radius[3];
            let packed_camera = Vector3::new(camera[0][0], camera[0][1], camera[0][2]);
            let packed_distance = (packed_camera - packed_center).magnitude();
            let actual_inside =
                (Vector3::from(d.camera.lookfrom) - Vector3::from(a.center)).magnitude() < a.radius;
            if actual_inside != (packed_distance < packed_radius)
                || (packed_distance - packed_radius).abs()
                    <= 16. * f32::EPSILON * (packed_distance + packed_radius)
            {
                return Err(
                    io::Error::other("Camera medium is ambiguous at Metal f32 precision").into(),
                );
            }
            for (j, b) in spheres.iter().enumerate() {
                if i == j || b.material.kind != Kind::Dielectric as u32 {
                    continue;
                }
                let center =
                    Vector3::new(b.center_radius[0], b.center_radius[1], b.center_radius[2]);
                let distance = (center - packed_center).magnitude();
                let radius = b.center_radius[3];
                if distance < radius + packed_radius
                    && distance + radius.min(packed_radius) >= radius.max(packed_radius)
                {
                    return Err(io::Error::other(
                        "Dielectric boundaries overlap at Metal f32 precision",
                    )
                    .into());
                }
            }
            let mut parent = 0;
            let mut parent_radius = f64::INFINITY;
            for (j, b) in d.spheres.iter().enumerate() {
                if i != j
                    && spheres[j].material.kind == Kind::Dielectric as u32
                    && b.radius < parent_radius
                    && (Vector3::from(a.center) - Vector3::from(b.center)).magnitude() + a.radius
                        < b.radius
                {
                    parent = u32::try_from(j)? + 1;
                    parent_radius = b.radius;
                }
            }
            spheres[i].material.parent_medium = parent;
            if a.radius < camera_radius
                && (Vector3::from(d.camera.lookfrom) - Vector3::from(a.center)).magnitude()
                    < a.radius
            {
                camera_medium = u32::try_from(i)? + 1;
                camera_radius = a.radius;
            }
        }
        let mut lights = Vec::new();
        // Match the CPU scene's plane-then-sphere light ordering for seeded parity.
        for (i, p) in planes.iter().enumerate() {
            if p.material.emission[..3].iter().any(|x| *x > 0.) {
                lights.push([1, i.try_into()?, 0, 0]);
            }
        }
        for (i, s) in spheres.iter().enumerate() {
            if s.material.emission[..3].iter().any(|x| *x > 0.) {
                lights.push([0, i.try_into()?, 0, 0]);
            }
        }
        if d.environment.iter().any(|x| *x > 0.) {
            lights.push([2, 0, 0, 0]);
        }
        let parameters = Parameters {
            camera,
            width: settings.image.width,
            height: settings.image.height,
            sample_start: 0,
            sample_count: 0,
            sphere_count: spheres.len().try_into()?,
            plane_count: planes.len().try_into()?,
            seed: settings.seed,
            max_bounces: settings.max_bounces,
            environment: pack(d.environment, 0.)?,
            reset: 1,
            light_count: lights.len().try_into()?,
            light_sampling: settings.light_sampling.into(),
            camera_medium,
        };
        Ok(Self {
            spheres,
            planes,
            lights,
            parameters,
        })
    }
}
fn material(m: CpuMaterial) -> Result<Material> {
    let result = Material {
        albedo: pack(m.albedo.into(), 0.)?,
        emission: pack(m.emission.into(), 0.)?,
        eta_roughness: pack(m.eta.into(), m.roughness)?,
        k_ior: pack(m.k.into(), m.ior)?,
        absorption: pack(m.absorption.into(), 0.)?,
        kind: m.kind as u32,
        two_sided: m.two_sided.into(),
        parent_medium: 0,
        reserved: 0,
    };
    if m.kind == Kind::Conductor
        && result.eta_roughness[..3]
            .iter()
            .chain(&result.k_ior[..3])
            .any(|v| !(16. * v.powi(4)).is_normal())
    {
        return Err(io::Error::other("Optical constants exceed Metal f32 arithmetic range").into());
    }
    if !result.k_ior[3].powi(2).is_finite() || !result.k_ior[3].recip().powi(2).is_normal() {
        return Err(io::Error::other("IOR exceeds Metal f32 arithmetic range").into());
    }
    if m.roughness > 0. && !result.eta_roughness[3].powi(4).is_normal() {
        return Err(io::Error::other("Roughness below Metal f32 precision").into());
    }
    Ok(result)
}
fn pack(v: [f64; 3], w: f64) -> Result<[f32; 4]> {
    let packed = [v[0] as f32, v[1] as f32, v[2] as f32, w as f32];
    if packed
        .iter()
        .any(|v| !v.is_finite() || (*v != 0. && !v.is_normal()))
        || v.iter()
            .zip(&packed[..3])
            .any(|(original, packed)| *original != 0. && *packed == 0.)
        || (w != 0. && packed[3] == 0.)
    {
        return Err(io::Error::other("Scene value outside Metal f32 range").into());
    }
    Ok(packed)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_values_outside_gpu_range() {
        assert!(pack([1e100, 0., 0.], 0.).is_err());
        assert!(pack([f64::NAN, 0., 0.], 0.).is_err());
        assert!(pack([1e-100, 0., 0.], 0.).is_err());
    }
}
