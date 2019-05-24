extern crate image;
extern crate cgmath;
extern crate rand;
mod ray;
mod geometry;
mod camera;
mod material;
mod light;
mod scene;
use cgmath::*;
use rand::*;
use crate::tracer::image::GenericImage;
use crate::tracer::light::*;
use crate::tracer::scene::*;

pub fn generate(path: &str, width: u32, height: u32) -> std::io::Result<()> {
    let mut img = image::DynamicImage::new_rgb8(width, height);
    let ns = 32;
    let mut data: Vec<&geometry::Hitable> = Vec::new();
    let mat_flat = material::Flat{};
    let mat_metal = material::Metal{};
    let mat_glass = material::Dielectric::new(1.07);

    let plane1 = geometry::Plane::new(
        Vector3::new(0.0, -1.0, 0.0),
        Vector3::new(0.0, 1.0, 0.0),
        Vector3::new(0.7, 0.7, 0.0),
        &mat_flat);
    
    let sphere2 = geometry::Sphere::new(
        Vector3::new(1.0, 1.0, 1.0), 
        1.5, Vector3::new(-3.0, 0.5, 0.0), 
        &mat_glass);

    let sphere3 = geometry::Sphere::new(
        Vector3::new(1.0, 1.0, 1.0), 
        1.0, Vector3::new(2.0, 0.0, 0.0),
        &mat_metal);

    let sphere4 = geometry::Sphere::new(
        Vector3::new(1.0, 0.0, 0.0), 
        1.0, Vector3::new(0.0, 0.0, 0.0),
        &mat_flat);
    let sphere_light = geometry::Sphere::new(
        Vector3::new(1.0, 1.0, 1.0), 
        0.05, Vector3::new(-1.0, 2.0, 2.0),
        &mat_flat);

    data.push(&plane1);
    data.push(&sphere2);
    data.push(&sphere3);
    data.push(&sphere4);
    data.push(&sphere_light);

    let cam = camera::Camera::new(
        Vector3::new(-5.0, 2.0, 3.0),
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(0.0, 1.0, 0.0),
        100.0,
        (width/height) as f64
    );
    let mut light_source = Vec::new();
    light_source.push(Light::new(Vector3::new(-1.0, 2.0, 2.0), 32.0));
    let scene = Scene::new(&data, &light_source);

    let mut rng = rand::thread_rng();
        for i in 0..width {
            for j in 0..height {
            let mut col = Vector3::new(0.0, 0.0, 0.0);
            for _ in 0..ns {
                let r1: f64 = rng.gen();
                let r2: f64 = rng.gen();
                let u = ((i as f64) + r1) / (width as f64);
                let v = ((j as f64) + r2) / (height as f64);
                let r = cam.get_ray(u, v);
                col += scene.render(&r, 0, std::f64::MAX);
            }
                col /= ns as f64;
                col = Vector3::new(col.x.sqrt(), col.y.sqrt(), col.z.sqrt());
                let ir = (255.99*col.x) as u8;
                let ig = (255.99*col.y) as u8;
                let ib = (255.99*col.z) as u8;
                let mut pix = [0;4];
                pix[0] = ir;
                pix[1] = ig;
                pix[2] = ib;
                pix[3] = 1;
                img.put_pixel(i, j, image::Rgba(pix));
        }
    }
    img.save(path)
}