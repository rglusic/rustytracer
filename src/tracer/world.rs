use super::Result;
use super::camera;
use super::geometry;
use super::material::{Kind, Material};
use super::material_input::MaterialInput;
use cgmath::{InnerSpace, Vector3};
use serde::Deserialize;
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::Path;
use std::sync::Arc;

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub(super) struct Camera {
    pub lookfrom: [f64; 3],
    pub lookat: [f64; 3],
    pub fov: f64,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub(super) struct Plane {
    pub origin: [f64; 3],
    pub normal: [f64; 3],
    #[serde(default = "white_color")]
    pub color: [f64; 3],
    pub mat: MaterialInput,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub(super) struct Sphere {
    #[serde(default = "white_color")]
    pub color: [f64; 3],
    pub radius: f64,
    pub center: [f64; 3],
    pub mat: MaterialInput,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub(super) struct WorldJson {
    pub camera: Camera,
    #[serde(default)]
    pub environment: [f64; 3],
    pub planes: Vec<Plane>,
    pub spheres: Vec<Sphere>,
}

pub struct World {
    camera: Arc<camera::Camera>,
    hitables: Vec<Arc<dyn geometry::Hitable>>,
    pub(super) description: WorldJson,
}

impl World {
    pub fn new(filename: impl AsRef<Path>, width: u32, height: u32) -> Result<World> {
        let path = filename.as_ref();
        let file = File::open(path).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("Cannot open scene '{}': {error}", path.display()),
            )
        })?;
        Self::from_reader(BufReader::new(file), width, height).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid scene '{}': {error}", path.display()),
            )
            .into()
        })
    }

    pub(super) fn from_reader(reader: impl Read, width: u32, height: u32) -> Result<World> {
        if width == 0 || height == 0 {
            return Err(invalid_scene("Image dimensions must be greater than zero").into());
        }
        let json: WorldJson = serde_json::from_reader(reader)?;
        let lookfrom = checked_vector(json.camera.lookfrom, "Camera position")?;
        let lookat = checked_vector(json.camera.lookat, "Camera target")?;
        let view = lookfrom - lookat;
        if !valid_direction(view) || !valid_direction(Vector3::unit_y().cross(view)) {
            return Err(invalid_scene(
                "Camera must have a nonzero view direction that is not parallel to its up vector",
            )
            .into());
        }
        if !json.camera.fov.is_finite() || json.camera.fov <= 0.0 || json.camera.fov >= 180.0 {
            return Err(
                invalid_scene("Camera field of view must be between 0 and 180 degrees").into(),
            );
        }
        let camera = camera::Camera::new(
            lookfrom,
            lookat,
            Vector3::unit_y(),
            json.camera.fov,
            f64::from(width) / f64::from(height),
        );

        checked_color(json.environment)?;
        let mut hitables: Vec<Arc<dyn geometry::Hitable>> = Vec::new();
        for plane in &json.planes {
            let normal = checked_vector(plane.normal, "Plane normal")?;
            if !valid_direction(normal) {
                return Err(invalid_scene("Plane normals must have nonzero finite length").into());
            }
            let material = plane.mat.resolve(plane.color)?;
            if material.kind == Kind::Dielectric {
                return Err(invalid_scene("Dielectric boundaries require closed spheres").into());
            }
            hitables.push(Arc::new(geometry::Plane {
                origin: checked_vector(plane.origin, "Plane origin")?,
                normal: normal.normalize(),
                material,
            }));
        }
        for sphere in &json.spheres {
            if sphere.radius <= 0.0 || !(sphere.radius * sphere.radius).is_finite() {
                return Err(invalid_scene("Sphere radius must be positive and finite").into());
            }
            hitables.push(Arc::new(geometry::Sphere {
                material: sphere.mat.resolve(sphere.color)?,
                radius: sphere.radius,
                center: checked_vector(sphere.center, "Sphere center")?,
            }));
        }
        validate_media(&json)?;
        Ok(World {
            camera: Arc::new(camera),
            hitables,
            description: json,
        })
    }

    pub fn get_hitables(&self) -> Vec<Arc<dyn geometry::Hitable>> {
        self.hitables.clone()
    }

    pub fn get_camera(&self) -> Arc<camera::Camera> {
        Arc::clone(&self.camera)
    }
}

fn invalid_scene(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn checked_vector(values: [f64; 3], name: &str) -> io::Result<Vector3<f64>> {
    if values.iter().any(|value| !value.is_finite()) {
        return Err(invalid_scene(format!("{name} must contain finite numbers")));
    }
    Ok(Vector3::from(values))
}

fn checked_color(values: [f64; 3]) -> io::Result<Vector3<f64>> {
    if values.iter().any(|value| *value < 0.0) {
        return Err(invalid_scene("Colors must be nonnegative"));
    }
    checked_vector(values, "Color")
}

fn valid_direction(direction: Vector3<f64>) -> bool {
    let squared_length = direction.magnitude2();
    squared_length.is_finite() && squared_length > 0.0
}

#[cfg(test)]
#[path = "world_tests.rs"]
mod tests;

fn white_color() -> [f64; 3] {
    [1.; 3]
}
fn validate_media(json: &WorldJson) -> Result<()> {
    let materials: Vec<Material> = json
        .spheres
        .iter()
        .map(|s| s.mat.resolve(s.color))
        .collect::<Result<_>>()?;
    for (i, a) in json.spheres.iter().enumerate() {
        if materials[i].kind != Kind::Dielectric {
            continue;
        }
        let camera_distance =
            (Vector3::from(json.camera.lookfrom) - Vector3::from(a.center)).magnitude();
        if (camera_distance - a.radius).abs() <= geometry::gamma(16) * (camera_distance + a.radius)
        {
            return Err(invalid_scene("Camera lies on an ambiguous dielectric boundary").into());
        }
        for (j, b) in json.spheres.iter().enumerate().skip(i + 1) {
            if materials[j].kind != Kind::Dielectric {
                continue;
            }
            let distance = (Vector3::from(a.center) - Vector3::from(b.center)).magnitude();
            if distance < a.radius + b.radius
                && distance + a.radius.min(b.radius) >= a.radius.max(b.radius)
            {
                return Err(invalid_scene(
                    "Dielectric spheres must be disjoint or strictly nested",
                )
                .into());
            }
        }
    }
    Ok(())
}
