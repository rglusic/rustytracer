extern crate image;
extern crate cgmath;
extern crate rand;
mod ray;
mod geometry;
mod camera;
mod material;
use std::fs::File;
use std::io::prelude::*;
use cgmath::*;
use rand::*;
//use cgmath::prelude::*;

pub fn generate_jpg(width: u32, height: u32) -> std::io::Result<()> {
    let mut img: image::RgbaImage = image::ImageBuffer::new(width, height);
    let ns = 32;
    let mut data: Vec<geometry::Sphere> = Vec::new();
    let mat_flat = material::Flat{};
    let mat_metal = material::Metal{};
    let mat_glass = material::Dielectric::new(1.5);

    data.push(geometry::Sphere::new(
        Vector3::new(0.0, 0.0, 0.3), 
        100.0, Vector3::new(0.0, -100.5, 0.0), 
        &mat_flat as &material::Material)
    );
    data.push(geometry::Sphere::new(
        Vector3::new(2.0, 2.0, 2.0), 
        0.5, Vector3::new(-2.0, 0.0, -1.0), 
        &mat_glass as &material::Material)
    );
    data.push(geometry::Sphere::new(
    Vector3::new(0.7, 0.7, 0.7), 
        0.5, Vector3::new(1.0, 0.0, -2.5),
        &mat_metal as &material::Material)
    );
    data.push(geometry::Sphere::new(
        Vector3::new(1.0, 0.0, 0.0), 
        0.5, Vector3::new(0.0, 0.0, -1.0),
        &mat_flat as &material::Material)
    );

    let cam = camera::Camera::new(
        Vector3::new(-4.0, 0.0, -0.0),
        Vector3::new(-0.0, 0.0, -1.0),
        Vector3::new(0.0, -1.0, 0.0),
        90.0,
        (width/height) as f64
    );

    let mut rng = rand::thread_rng();
    for i in 0..width {
        for j in (0..height).rev() {
            let mut col = Vector3::new(0.0, 0.0, 0.0);
            for _ in 0..ns {
                let r1: f64 = rng.gen();
                let r2: f64 = rng.gen();
                let u = ((i as f64) + r1) / (width as f64);
                let v = ((j as f64) + r2) / (height as f64);
                let r = cam.get_ray(u, v);
                col += geometry::color(&r, &data, 0, std::f64::MAX);
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
    img.save("image.jpg")
}

pub fn generate() -> std::io::Result<()> {
        let mut f = File::create("image.ppm")?;
        let nx = 800;
        let ny = 600;
        let ns = 32;
        let header = format!("P3\n{} {} \n255\n", nx, ny);
        let header = header.as_bytes();
        f.write_all(header)?;

        let mut data: Vec<geometry::Sphere> = Vec::new();
        let mat_flat = material::Flat{};
        let mat_metal = material::Metal{};
        let mat_glass = material::Dielectric::new(1.5);
    
        data.push(geometry::Sphere::new(
            Vector3::new(0.0, 0.0, 0.3), 
            100.0, Vector3::new(0.0, -100.5, 0.0), 
            &mat_flat as &material::Material)
        );
        data.push(geometry::Sphere::new(
            Vector3::new(2.0, 2.0, 2.0), 
            0.5, Vector3::new(-2.0, 0.0, -1.0), 
            &mat_glass as &material::Material)
        );
        data.push(geometry::Sphere::new(
        Vector3::new(0.7, 0.7, 0.7), 
            0.5, Vector3::new(1.0, 0.0, -2.5),
            &mat_metal as &material::Material)
        );
        data.push(geometry::Sphere::new(
            Vector3::new(1.0, 0.0, 0.0), 
            0.5, Vector3::new(0.0, 0.0, -1.0),
            &mat_flat as &material::Material)
        );

        let cam = camera::Camera::new(
            Vector3::new(-4.0, 0.0, -0.0),
            Vector3::new(-0.0, 0.0, -1.0),
            Vector3::new(0.0, 1.0, 0.0),
            90.0,
            (nx/ny) as f64
        );

        let mut rng = rand::thread_rng();
        for j in (0..(ny-1)).rev() {
            for i in 0..nx {
                let mut col = Vector3::new(0.0, 0.0, 0.0);
                for _ in 0..ns {
                    let r1: f64 = rng.gen();
                    let r2: f64 = rng.gen();
                    let u = ((i as f64) + r1) / (nx as f64);
                    let v = ((j as f64) + r2) / (ny as f64);
                    let r = cam.get_ray(u, v);
                    col += geometry::color(&r, &data, 0, std::f64::MAX);
                }
                    col /= ns as f64;
                    col = Vector3::new(col.x.sqrt(), col.y.sqrt(), col.z.sqrt());
                    let ir = (255.99*col.x) as i64;
                    let ig = (255.99*col.y) as i64;
                    let ib = (255.99*col.z) as i64;

                    let output = format!("{} {} {}\n", ir, ig, ib);
                    f.write_all(output.as_bytes())?;
            }
        }
        Ok(())
}