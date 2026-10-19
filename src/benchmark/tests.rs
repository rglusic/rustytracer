use super::*;
use crate::tracer::{Frame, RenderOptions};
use clap::Parser;
use std::fs;
use std::path::Path;
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
        render_output(
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
    render_output(
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
    let error = render_output(
        &output,
        Path::new(env!("CARGO_MANIFEST_DIR")).join("worlds/closed_room.json"),
        RenderOptions::default(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("PNG, JPEG or OpenEXR"));
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

fn render_output(
    path: impl AsRef<Path>,
    scene: impl AsRef<Path>,
    options: RenderOptions,
) -> Result<()> {
    run(command(path.as_ref(), scene.as_ref(), options))
}

fn command(path: &Path, scene: &Path, options: RenderOptions) -> Cli {
    let mut cli = Cli::parse_from(["rustytracer", "--backend", "cpu"]);
    cli.output = path.to_owned();
    cli.scene = scene.to_owned();
    cli.width = options.width;
    cli.height = options.height;
    cli.samples = options.samples_per_pixel;
    cli
}

#[test]
fn application_reports_raw_reference_error_and_timed_runs() {
    let directory = TestDirectory::new();
    let scene = Path::new(env!("CARGO_MANIFEST_DIR")).join("worlds/physical_room.json");
    let reference = directory.0.join("reference.exr");
    let options = RenderOptions {
        width: 3,
        height: 2,
        samples_per_pixel: 8,
    };
    render_output(&reference, &scene, options).unwrap();
    for (repeats, warmup) in [(1, 0), (2, 1)] {
        let output = directory.0.join("display.png");
        let metrics = directory.0.join("new/metrics/report.json");
        let mut cli = command(&output, &scene, options);
        cli.reference = Some(reference.clone());
        cli.metrics = Some(metrics.clone());
        cli.exposure = -2.;
        cli.benchmark = repeats;
        cli.warmup = warmup;
        run(cli).unwrap();
        let report: serde_json::Value =
            serde_json::from_slice(&fs::read(metrics).unwrap()).unwrap();
        assert_eq!(report["integrator"], "physical-rgb-v1");
        assert_eq!(report["backend"], "cpu");
        assert_eq!(report["display_exposure_ev"], -2.);
        assert_eq!(report["reference"], reference.to_str().unwrap());
        assert_eq!(report["warmup_runs"], warmup);
        assert_eq!(report["spheres"], 3);
        assert_eq!(report["planes"], 6);
        let runs = report["runs"].as_array().unwrap();
        assert_eq!(runs.len(), repeats as usize);
        for run in runs {
            assert_eq!(run["radiance_mse"], 0.);
            assert!(run["gpu_seconds"].is_null());
            let render = run["render_seconds"].as_f64().unwrap();
            let encoding = run["encoding_seconds"].as_f64().unwrap();
            let total = run["render_and_encoding_seconds"].as_f64().unwrap();
            assert!((total - render - encoding).abs() < 1e-12);
            assert!((run["samples_per_second"].as_f64().unwrap() * render - 48.).abs() < 1e-9);
        }
        let times: Vec<f64> = runs
            .iter()
            .map(|run| run["render_and_encoding_seconds"].as_f64().unwrap())
            .collect();
        let median = times.iter().sum::<f64>() / f64::from(repeats);
        assert!(
            (report["median_render_and_encoding_seconds"]
                .as_f64()
                .unwrap()
                - median)
                .abs()
                < 1e-12
        );
        assert_eq!(
            report["p95_render_and_encoding_seconds"].as_f64().unwrap(),
            times.iter().copied().fold(0., f64::max)
        );
        if warmup == 0 {
            let cold = report["setup_seconds"].as_f64().unwrap() + times[0];
            assert!((report["cold_end_to_end_seconds"].as_f64().unwrap() - cold).abs() < 1e-12);
        }
    }
}
