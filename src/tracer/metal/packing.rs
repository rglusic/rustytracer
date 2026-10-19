//! Checked f64-to-f32 conversion at the CPU/Metal boundary.
use super::data::{Material, Plane, Sphere};
use crate::tracer::Result;
use crate::tracer::material::{Kind, Material as CpuMaterial};
use crate::tracer::world::{self, World};
use cgmath::{InnerSpace, Vector3};
use std::io;

pub(super) fn camera(world: &World) -> Result<[[f32; 4]; 4]> {
    let mut camera = [[0.; 4]; 4];
    for (packed, vector) in camera.iter_mut().zip(world.get_camera().vectors()) {
        *packed = pack(vector, 0.)?;
    }
    let center_direction = Vector3::new(
        camera[1][0] + 0.5 * camera[2][0] + 0.5 * camera[3][0] - camera[0][0],
        camera[1][1] + 0.5 * camera[2][1] + 0.5 * camera[3][1] - camera[0][1],
        camera[1][2] + 0.5 * camera[2][2] + 0.5 * camera[3][2] - camera[0][2],
    );
    if !center_direction.magnitude2().is_normal() {
        return Err(io::Error::other("Camera direction loses precision in Metal f32").into());
    }
    Ok(camera)
}

pub(super) fn sphere(sphere: &world::Sphere) -> Result<Sphere> {
    let center_radius = pack(sphere.center, sphere.radius)?;
    let radius = center_radius[3];
    if radius <= 0.
        || !(radius * radius).is_normal()
        || center_radius[..3]
            .iter()
            .any(|center| *center + radius == *center || *center - radius == *center)
    {
        return Err(io::Error::other("Sphere is outside representable Metal f32 precision").into());
    }
    Ok(Sphere {
        center_radius,
        material: material(sphere.mat.resolve(sphere.color)?)?,
    })
}

pub(super) fn plane(plane: &world::Plane) -> Result<Plane> {
    Ok(Plane {
        origin: pack(plane.origin, 0.)?,
        normal: pack(Vector3::from(plane.normal).normalize().into(), 0.)?,
        material: material(plane.mat.resolve(plane.color)?)?,
    })
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
pub(super) fn pack(v: [f64; 3], w: f64) -> Result<[f32; 4]> {
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
