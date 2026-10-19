use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

struct TestDirectory(std::path::PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("rustytracer-{}-{nonce}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn saves_and_decodes_png_and_jpeg_in_new_output_directories() {
    let directory = TestDirectory::new();
    let scene_path = directory.0.join("emitter.json");
    fs::write(
        &scene_path,
        r#"{
            "camera": {"lookfrom": [0, 0, 0], "lookat": [0, 0, -1], "fov": 90},
            "planes": [{"origin": [0, 0, -1], "normal": [0, 0, 1], "color": [1, 1, 1], "mat": "diffuse_light"}],
            "spheres": []
        }"#,
    )
    .unwrap();
    for extension in ["png", "jpg"] {
        let output = directory
            .0
            .join("new/nested")
            .join(format!("image.{extension}"));
        generate_with_options(
            &output,
            &scene_path,
            RenderOptions {
                width: 17,
                height: 9,
                samples_per_pixel: 2,
            },
        )
        .unwrap();
        let decoded = image::open(output).unwrap().to_rgb8();
        assert_eq!(decoded.dimensions(), (17, 9));
        assert!(decoded.pixels().all(|pixel| pixel.0 == [255, 255, 255]));
    }
}

#[test]
fn existing_closed_room_scene_renders_a_nonempty_image() {
    let directory = TestDirectory::new();
    let output = directory.0.join("closed-room.png");
    generate_with_options(
        &output,
        Path::new(env!("CARGO_MANIFEST_DIR")).join("worlds/closed_room.json"),
        RenderOptions {
            width: 80,
            height: 40,
            samples_per_pixel: 64,
        },
    )
    .unwrap();
    let image = image::open(output).unwrap().to_rgb8();
    assert_eq!(image.dimensions(), (80, 40));
    assert!(image.as_raw().iter().any(|channel| *channel > 0));
}

#[test]
fn unsupported_output_formats_fail_before_rendering() {
    let directory = TestDirectory::new();
    let output = directory.0.join("unsupported.gif");
    let error = generate_with_options(
        &output,
        Path::new(env!("CARGO_MANIFEST_DIR")).join("worlds/closed_room.json"),
        RenderOptions::default(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("PNG or JPEG"));
    assert!(!output.exists());
}

#[test]
fn openexr_round_trip_preserves_hdr_radiance_and_ignores_display_exposure() {
    let directory = TestDirectory::new();
    let output = directory.0.join("linear.exr");
    let frame = Frame {
        linear: vec![[0.18, 2., 100., 0.]],
        render_seconds: 0.,
        gpu_seconds: None,
    };
    frame
        .save(
            RenderOptions {
                width: 1,
                height: 1,
                samples_per_pixel: 1,
            },
            &output,
            image::ImageFormat::OpenExr,
            -5.,
        )
        .unwrap();
    let image = image::open(output).unwrap().to_rgb32f();
    assert_eq!(image.get_pixel(0, 0).0, [0.18, 2., 100.]);
}
