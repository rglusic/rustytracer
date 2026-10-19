# RustyTracer

A physically based RGB path tracer with native Metal acceleration on Apple silicon
and an f64 multithreaded CPU reference. JSON scenes describe spheres, infinite
planes, diffuse surfaces, mirrors, conductors, absorbing glass, and area emitters.

## Quick start

The pinned toolchain is Rust 1.99.1; Rust 1.88 is also supported. Metal requires
Apple silicon and macOS 15 or newer. Hardware sphere traversal requires M3 or newer.

```sh
rustup toolchain install 1.99.1 --profile minimal --component clippy --component rustfmt
rustup run 1.99.1 cargo run --release --locked -- --list-devices
rustup run 1.99.1 cargo run --release --locked -- \
  --backend metal --width 1920 --height 1080 --samples 256 \
  --output output/physical.png
```

Run from the repository root. Explicit rustup commands work when Homebrew Cargo
comes first on PATH. Release builds are important for CPU performance. The default
scene is [`worlds/physical_room.json`](worlds/physical_room.json), with an absorbing
glass sphere, rough conductor, and explicitly specified area-light radiance.

For unclipped, scene-linear floating-point output:

```sh
./target/release/rustytracer --backend metal --width 1920 --height 1080 \
  --samples 1024 --output output/physical.exr
```

PNG/JPEG apply display exposure and sRGB encoding. `--exposure -2` reduces display
exposure by two stops; OpenEXR always stores raw radiance. Transport, MSE, and HDR
output are independent of display clipping.

## Physics

The integrator uses matched BSDF values and PDFs, exact dielectric and conductor
Fresnel, Snell refraction, Beer absorption, compensated Russian roulette, and
area-light/BSDF multiple importance sampling (MIS). The default has no bounce cap.
GPU paths continue across bounded dispatches with persistent state. Rays use
scale-aware floating-point offsets instead of a fixed world-distance cutoff.

[`PHYSICS.md`](PHYSICS.md) documents equations, material parameters, units, legacy
migration, finite-precision behavior, and model limitations. Transport is linear
RGB and unpolarized. Rough conductors use the standard single-scattering Smith GGX
model, which omits microfacet interreflection; no empirical brightness correction
is applied. Spectral dispersion requires wavelength-based transport.

## Render options

| Option | Default | Purpose |
| --- | --- | --- |
| `--backend auto/cpu/metal` | `auto` | Auto prefers a compatible Apple GPU |
| `--traversal auto/linear/hardware` | `auto` | GPU sphere-intersection strategy |
| `--scene PATH` | `worlds/physical_room.json` | JSON scene |
| `--output PATH` | `output/image.png` | PNG, JPEG, or float OpenEXR; replaces existing file |
| `--width`, `--height` | `200`, `100` | Resolution |
| `--samples` | `5000` | Samples per pixel |
| `--seed` | `1` | Reproducible random streams |
| `--max-bounces` | `0` | Unlimited; explicit 1..256 caps introduce truncation bias |
| `--batch-size` | `64` | GPU sample target per batch, 1..1024 |
| `--threadgroup-size` | `64` | Threads per group, subject to SIMD/pipeline limits |
| `--no-light-sampling` | off | Disable MIS/direct sampling for estimator comparisons |
| `--exposure` | `0` | Display exposure in stops, -64..64 |
| `--progress` | off | GPU sample-batch progress |
| `--benchmark N` | `1` | Timed renders reusing scene resources and pipelines |
| `--warmup N` | `0` | Untimed renders before measurement |
| `--metrics PATH` | unset | Timing/workload JSON |
| `--reference PATH` | unset | Same-resolution scene-linear EXR for radiance MSE |
| `--list-devices` | off | GPU capabilities |

Auto traversal uses direct analytic sphere intersection below 64 spheres and
acceleration structures from 64 spheres on supported GPUs. Infinite planes remain
analytic. Custom hardware intersection functions preserve exact sphere geometry.
Explicit hardware mode requires at least one sphere and a supported GPU.

Auto selects CPU if no compatible GPU exists. Shader, allocation, and execution
errors are reported. Scenes reject unknown materials, invalid reflectance, and
ambiguous intersecting dielectric boundaries. Metal also validates f32 range.

## Scenes and materials

Distances are metres. Albedo is linear reflectance in [0,1]; emission is linear
radiance and can exceed one. Absorption coefficients are inverse metres. Camera
FOV is vertical and in degrees. Normals are normalized during loading.

```json
{
  "camera": {"lookfrom":[0,0,5], "lookat":[0,0,0], "fov":45},
  "environment": [1,1,1],
  "planes": [],
  "spheres": [{
    "center":[0,0,0], "radius":1,
    "mat": {"kind":"dielectric", "ior":1.5, "absorption":[0.1,0.02,0.01]}
  }]
}
```

The original `worlds/closed_room.json` remains loadable. Legacy `metal` means its
historical ideal mirror; measured-metal models require explicit conductor eta/k.
Legacy emitter color now specifies its radiance, with the hardcoded intensity
removed. See the migration table in PHYSICS.md for all historical names.

## Measurements and validation

The [committed physics report](benchmarks/physics-m4-pro-2026-10-19.json) records analytical convergence, PBRT comparison,
and fresh CPU/Metal timings. `--reference` MSE is computed on raw radiance before
exposure or encoding. Backend setup, render wall time, GPU time, encoding, cold
first-render total, median and p95 are reported separately. MSE computation is
excluded from render and encoding timers. Timed repeats use the same seed; use
independent seeds to measure statistical convergence.

```sh
rustup run 1.99.1 cargo build --release --locked
python3 scripts/validate_physics.py
python3 scripts/validate_physics.py --pbrt /path/to/pbrt-v3/build/pbrt
python3 scripts/benchmark.py
python3 scripts/tune_metal.py
```

The analytical benchmark uses four sample counts and four seeds for each CPU/GPU
mode. In the recorded run, MIS reduced radiance MSE by about 5.2x at 1024 samples.
The matched PBRT v3 RGB reference comparison had MSE about 0.00000118. The reference
scene and its scope are documented in PHYSICS.md; it verifies more than CPU/GPU
agreement while remaining a focused lighting test.

On the M4 Pro MacBook Pro (12 CPU cores, 16 GPU cores, 24 GiB), the new physical
room at 1920x1080 and 128 samples took a median **26.494s on CPU** and
**1.582s on Metal linear**, including PNG encoding: **16.7x faster**.
Forced hardware traversal took 2.533s for this three-sphere scene;
auto correctly retains linear traversal for small scenes. These are three sequential
timed runs after one warmup, Rust 1.99.1 release, macOS 27.0.1, seed 42, MIS enabled,
and unlimited depth. CPU uses f64; Metal uses f32.

Reproduce these timings with each backend:

```sh
./target/release/rustytracer --backend cpu --width 1920 --height 1080 \
  --samples 128 --seed 42 --benchmark 3 --warmup 1 \
  --output target/cpu.png --metrics target/cpu.json
./target/release/rustytracer --backend metal --traversal linear --width 1920 --height 1080 \
  --samples 128 --seed 42 --benchmark 3 --warmup 1 \
  --output target/metal.png --metrics target/metal.json
```

The earlier [`benchmarks/m4-pro-2026-10-19.json`](benchmarks/m4-pro-2026-10-19.json)
contains historical pre-physics timing data. Its 21.4x and 118.4x speedups describe
the older estimator and scene, and are not current physical-renderer results.
Fresh physical benchmarks use `target/performance-physics` and the new scene.

## Development

```sh
rustup run 1.99.1 cargo fmt --all -- --check
rustup run 1.99.1 cargo clippy --locked --all-targets --all-features -- -D warnings
rustup run 1.99.1 cargo clippy --locked --all-targets --no-default-features -- -D warnings
rustup run 1.99.1 cargo test --locked --all-features
rustup run 1.99.1 cargo test --locked --no-default-features
xcodebuild -downloadComponent metalToolchain
python3 scripts/check_shaders.py --check-format
MTL_DEBUG_LAYER=1 MTL_SHADER_VALIDATION=1 \
  rustup run 1.99.1 cargo test --locked --all-features -- \
  --ignored --test-threads=1 --nocapture
```

The dedicated GPU tests require actual Apple hardware, with M3 or newer for
hardware traversal. They cover RNG equivalence, BSDFs, CPU/GPU radiance agreement,
analytic furnace and cavity energy, nested glass and absorption, rough conductors,
batch determinism, path continuation, geometry edge cases, and invalid dispatches.
Ordinary CI runs CPU tests on Linux/macOS/Windows, including Rust 1.88 on Linux;
macOS also compiles Metal shaders. GPU runtime tests run explicitly on hardware.

To omit Apple GPU dependencies, build with `--no-default-features`.

Metal sources follow the repository's `.clang-format` configuration. Run
`python3 scripts/check_shaders.py --format` to format and compile them with the
Xcode tools. Compiler module caches stay under `target/shader-check`.

The tuning script accepts scene, traversal, resolution, sample count, repeat count,
and sweep ranges. For example, after generating dense scenes with the benchmark:

```sh
python3 scripts/tune_metal.py --scene target/performance-physics/spheres-4096.json \
  --traversal hardware --width 640 --height 360 --samples 128 \
  --batches 16 32 64 --threads 64 128
```

## Architecture

- `world.rs`, `material_input.rs`: validated scene and physical material contract.
- `material.rs`, `scene.rs`, `light_sampling.rs`: f64 BSDFs, transport, and MIS.
- `geometry.rs`: intersections, projected hit points, and secondary-ray spawning.
- `media.rs`: prepared dielectric hierarchy and CPU medium transitions.
- `render.rs`, `sampler.rs`: deterministic per-pixel/sample CPU work and RNG.
- `metal/data.rs`: checked POD layouts and medium/light metadata.
- `metal/mod.rs`: render orchestration and resumable sample batches.
- `metal/buffers.rs`, `metal/dispatch.rs`: owned GPU storage and command encoding.
- `metal/pipeline.rs`, `metal/acceleration.rs`: shader pipelines and hardware traversal.
- `metal/shaders/`: shared types, sampling, geometry, BSDFs, lighting, and path advancement.
- `metal/tests/`: GPU geometry, scheduling, and physical transport tests.
- `build.rs`, `metal/shaders/sources.txt`: one source order for embedded and offline shader compilation.
- `scripts/benchmark_common.py`: shared command execution and timing-report handling.
- `frame.rs`: radiance validation, float EXR, exposure, and sRGB display encoding.

GPU scene resources remain resident. The persistent path state uses 128 bytes per
pixel, plus a 16-byte radiance accumulation buffer. CPU reads follow command
completion. Unsafe interop is isolated in the Metal module, with host layout tests
and shader static assertions. Material and continuation fields have explicit names
and tested offsets while retaining the 96-byte material and 128-byte path layouts.

## License

MIT; see `LICENSE.txt`. Third-party notices remain in `Licenses/`.
