//! Prepare checked GPU scene records and the medium/light metadata used by shaders.
use super::data::{Parameters, Plane, Sphere};
use super::packing;
use crate::tracer::material::Kind;
use crate::tracer::world::{World, WorldJson};
use crate::tracer::{Result, Settings};
use cgmath::{InnerSpace, Vector3};
use std::io;

pub(super) struct SceneData {
    pub spheres: Vec<Sphere>,
    pub planes: Vec<Plane>,
    pub lights: Vec<[u32; 4]>,
    pub parameters: Parameters,
}

impl SceneData {
    pub fn new(world: &World, settings: Settings) -> Result<Self> {
        let description = &world.description;
        let camera = packing::camera(world)?;
        let mut spheres = description
            .spheres
            .iter()
            .map(packing::sphere)
            .collect::<Result<Vec<_>>>()?;
        let planes = description
            .planes
            .iter()
            .map(packing::plane)
            .collect::<Result<Vec<_>>>()?;
        let camera_medium = prepare_media(description, &camera, &mut spheres)?;
        let lights = prepare_lights(&planes, &spheres, description.environment)?;
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
            environment: packing::pack(description.environment, 0.)?,
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

/// Shader medium IDs are sphere index + 1; zero denotes ambient air.
/// Validate the packed boundaries before preserving the original f64 hierarchy.
fn prepare_media(
    description: &WorldJson,
    camera: &[[f32; 4]; 4],
    spheres: &mut [Sphere],
) -> Result<u32> {
    let mut camera_medium = 0;
    let mut camera_radius = f64::INFINITY;
    for index in 0..spheres.len() {
        if spheres[index].material.kind != Kind::Dielectric as u32 {
            continue;
        }
        let original = &description.spheres[index];
        validate_camera_medium(
            description.camera.lookfrom,
            camera[0],
            original,
            &spheres[index],
        )?;
        validate_boundary_separation(index, spheres)?;
        let mut parent = 0;
        let mut parent_radius = f64::INFINITY;
        for (outer_index, outer) in description.spheres.iter().enumerate() {
            if index != outer_index
                && spheres[outer_index].material.kind == Kind::Dielectric as u32
                && outer.radius < parent_radius
                && (Vector3::from(original.center) - Vector3::from(outer.center)).magnitude()
                    + original.radius
                    < outer.radius
            {
                parent = u32::try_from(outer_index)? + 1;
                parent_radius = outer.radius;
            }
        }
        spheres[index].material.parent_medium = parent;
        if original.radius < camera_radius
            && (Vector3::from(description.camera.lookfrom) - Vector3::from(original.center))
                .magnitude()
                < original.radius
        {
            camera_medium = u32::try_from(index)? + 1;
            camera_radius = original.radius;
        }
    }
    Ok(camera_medium)
}

fn validate_camera_medium(
    origin: [f64; 3],
    packed_origin: [f32; 4],
    original: &crate::tracer::world::Sphere,
    packed: &Sphere,
) -> Result<()> {
    let center = Vector3::new(
        packed.center_radius[0],
        packed.center_radius[1],
        packed.center_radius[2],
    );
    let radius = packed.center_radius[3];
    let camera = Vector3::new(packed_origin[0], packed_origin[1], packed_origin[2]);
    let distance = (camera - center).magnitude();
    let actual_inside =
        (Vector3::from(origin) - Vector3::from(original.center)).magnitude() < original.radius;
    if actual_inside != (distance < radius)
        || (distance - radius).abs() <= 16. * f32::EPSILON * (distance + radius)
    {
        return Err(io::Error::other("Camera medium is ambiguous at Metal f32 precision").into());
    }
    Ok(())
}

fn validate_boundary_separation(index: usize, spheres: &[Sphere]) -> Result<()> {
    let inner = &spheres[index];
    let center = Vector3::new(
        inner.center_radius[0],
        inner.center_radius[1],
        inner.center_radius[2],
    );
    let radius = inner.center_radius[3];
    for (other_index, other) in spheres.iter().enumerate() {
        if index == other_index || other.material.kind != Kind::Dielectric as u32 {
            continue;
        }
        let other_center = Vector3::new(
            other.center_radius[0],
            other.center_radius[1],
            other.center_radius[2],
        );
        let distance = (other_center - center).magnitude();
        let other_radius = other.center_radius[3];
        if distance < other_radius + radius
            && distance + other_radius.min(radius) >= other_radius.max(radius)
        {
            return Err(
                io::Error::other("Dielectric boundaries overlap at Metal f32 precision").into(),
            );
        }
    }
    Ok(())
}

fn prepare_lights(
    planes: &[Plane],
    spheres: &[Sphere],
    environment: [f64; 3],
) -> Result<Vec<[u32; 4]>> {
    let mut lights = Vec::new();
    // Match the CPU scene's plane-then-sphere light ordering for seeded parity.
    for (index, plane) in planes.iter().enumerate() {
        if plane.material.emission[..3].iter().any(|value| *value > 0.) {
            lights.push([1, index.try_into()?, 0, 0]);
        }
    }
    for (index, sphere) in spheres.iter().enumerate() {
        if sphere.material.emission[..3]
            .iter()
            .any(|value| *value > 0.)
        {
            lights.push([0, index.try_into()?, 0, 0]);
        }
    }
    if environment.iter().any(|value| *value > 0.) {
        lights.push([2, 0, 0, 0]);
    }
    Ok(lights)
}
