use std::sync::Arc;
use std::fs::File;
use std::io::BufReader;
use super::geometry;
use cgmath::Vector3;
use crate::tracer::cgmath::InnerSpace;
use super::camera;

use serde::Deserialize;
use serde_json::*;

#[derive(Deserialize, Debug)]
pub struct Camera {
    lookfrom: Vec<f64>,
    lookat:   Vec<f64>,
    fov: f64
}

#[derive(Deserialize, Debug)]
pub struct Plane {
    origin: Vec<f64>,
    normal: Vec<f64>,
    color:  Vec<f64>,
    mat: String
}

#[derive(Deserialize, Debug)]
pub struct Sphere {
    color:  Vec<f64>,
    radius: f64,
    center: Vec<f64>,
    mat: String
}

#[derive(Deserialize, Debug)]
struct WorldJSON {
    pub camera: Camera,
    pub planes: Vec<Plane>,
    pub spheres: Vec<Sphere>,
}

pub struct World {
    camera:   Arc<camera::Camera>,
    hitables: Vec<Arc<dyn geometry::Hitable>>
}

impl World {
    pub fn new(filename: &'static str, width: u32, height: u32) -> Result<World> {
        let file = File::open(filename).unwrap();
        let reader = BufReader::new(file);

        let json: WorldJSON = match serde_json::from_reader(reader) {
            Ok(res) => res,
            Err(e) => {
                println!("Error parsing file: {}", e);
                return Err(e);
            }
        };

        let json_cam = json.camera;
        let lookfrom = Vector3::new(json_cam.lookfrom[0], json_cam.lookfrom[1], json_cam.lookfrom[2]);
        let lookat = Vector3::new(json_cam.lookat[0], json_cam.lookat[1], json_cam.lookat[2]);

        let camera = camera::Camera::new(
            lookfrom,
            lookat,
            Vector3::new(0.0, 1.0, 0.0),
            json_cam.fov,
            (width/height).into()
        );


        let mut hitables: Vec<Arc<dyn geometry::Hitable>> = vec![];
        for plane in json.planes {
            hitables.push(Arc::new(geometry::Plane::new(
                Vector3::new(plane.origin[0], plane.origin[1], plane.origin[2]),
                Vector3::new(plane.normal[0], plane.normal[1], plane.normal[2]),
                Vector3::new(plane.color[0], plane.color[1], plane.color[2]),
                plane.mat.clone()
            )));
        }

        for sphere in json.spheres {
            hitables.push(Arc::new(geometry::Sphere::new(
                Vector3::new(sphere.color[0], sphere.color[1], sphere.color[2]),
                sphere.radius,
                Vector3::new(sphere.center[0], sphere.center[1], sphere.center[2]),
                sphere.mat.clone()
            )));
        }

        Ok(World { camera: Arc::new(camera), hitables: hitables})
    }

    pub fn get_hitables(&self) -> Vec<Arc<dyn geometry::Hitable>> {
        self.hitables.clone()
    }

    pub fn get_camera(&self) -> Arc<camera::Camera> {
        self.camera.clone()
    }
}