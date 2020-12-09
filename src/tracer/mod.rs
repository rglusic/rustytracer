extern crate image;
extern crate cgmath;
extern crate rand;
mod ray;
mod geometry;
mod camera;
mod material;
mod light;
mod scene;
mod world;
use cgmath::*;
use rand::*;
use crate::tracer::image::GenericImage;

use crate::tracer::scene::*;
use std::thread;
use std::sync::mpsc;
use std::sync::Arc;

use std::time::Instant;

pub fn render_section(
    width_seg: Vec<u32>,
    height: u32,
    width: u32,
    scene: Arc<scene::Scene>,
    cam: Arc<camera::Camera>,
    ns: u32,
    timer_start: std::sync::mpsc::Sender<f64>,
) -> Vec<Vec<u32>> {
    let mut ret_vec: Vec<Vec<u32>> = Vec::new();
    let mut rng = rand::thread_rng();
    for i in width_seg[0]..width_seg[1] {
        for j in 0..height {
            let tstart = Instant::now();
            let mut col = Vector3::new(0.0, 0.0, 0.0);
            //Sample per pixel.
            for _ in 0..=ns {
                let r1: f64 = rng.gen();
                let r2: f64 = rng.gen();
                let u = ((i as f64) + r1) / (width as f64);
                let v = ((j as f64) + r2) / (height as f64);
                let r = cam.get_ray(u, v);
                col += scene.render(&r, 0, std::f64::MAX);
            }

            col /= (ns) as f64;
            //col = Vector3::new(col.x.sqrt(), col.y.sqrt(), col.z.sqrt());
            col.x = col.x.min(1.0);
            col.y = col.y.min(1.0);
            col.z = col.z.min(1.0);

            let ir = (255.99*col.x) as u8;
            let ig = (255.99*col.y) as u8;
            let ib = (255.99*col.z) as u8;
            let mut pix = [0;4];
            pix[0] = ir;
            pix[1] = ig;
            pix[2] = ib;
            pix[3] = 1;
            ret_vec.push(vec![
                i, 
                j as u32,
                pix[0] as u32,
                pix[1] as u32,
                pix[2] as u32,
                pix[3] as u32,
                1
                ]);
            
            timer_start.send(tstart.elapsed().as_secs_f64()).unwrap();
        }
    }
    ret_vec
}

pub fn generate(path: &str, width: u32, height: u32) -> std::io::Result<()> {
    let current_time = Instant::now();
    let mut img = image::DynamicImage::new_rgb8(width, height);
    let ns = 5000;
    let world = world::World::new("worlds/closed_room.json", width, height)?;

    let scene = Arc::new(Scene::new(world.get_hitables()));
    let num_of_threads = 8;

    let width_list = 0..width;
    let width_len  = width_list.len();
    let width_per_thread = width_len/num_of_threads;
    let mut width_parallel: Vec<Vec<u32>> = Vec::new();

    for i in 1..=num_of_threads {
        let entry = ((i-1) as u32)*(width_per_thread as u32);
        let last_entry = (i as u32)*(width_per_thread as u32);
        width_parallel.push(vec![entry, last_entry]);
    }

    //Send a render_section() result back to main thread.
    let (tx,rx) = mpsc::channel();

    //timer
    let (timer_send, timer_recv) = mpsc::channel();

    let mut thread_data = Vec::new();
    let cam = world.get_camera();
    
    for width_seg in width_parallel {
        let scene = scene.clone();
        let cam = cam.clone();
        let tx = tx.clone();
        let timer_send = timer_send.clone();
        thread::spawn(move || {
            tx.send(render_section(width_seg, height, width, scene, cam, ns, timer_send)).unwrap();
        });
    }

    let mut collect_time = true;
    for _ in 0..num_of_threads {
        //Calculate time remaining.
        if collect_time {
            //Take thirty values for an average.
            let mut possible_timer: Vec<f64> = vec![];
            while possible_timer.len() < 500 {
                possible_timer.push(timer_recv.recv().unwrap_or(0.0));
            }
            let average: f64 = possible_timer.iter().sum::<f64>() / (possible_timer.len() as f64);
            if average != 0.0 {
                println!("Time per pixel: {:?} sec(s), 
                    estimated time remaining: {:?} min(s)", 
                    average,

                    average 
                    / 60.0 
                    * ((((width as f64) 
                    * (height as f64))))
                    / (num_of_threads as f64)
                );
            }
            collect_time = false;
        }

        //Find all threads values.
        thread_data.push(rx.recv().unwrap());
    }

    println!("Threads finished, compiling image.");

    for first in &*thread_data {
        for pix in first {
            let mut pixels = [0u8;4];
            pixels[0] = (pix[2] as u8).into();
            pixels[1] = (pix[3] as u8).into();
            pixels[2] = (pix[4] as u8).into();
            pixels[3] = (pix[5] as u8).into();
            img.put_pixel(pix[0], pix[1], image::Rgba(pixels));
        }
    }

    println!("Total time taken: {:?} min(s)", current_time.elapsed().as_secs_f64()/60.0);

    img.save(path)
}