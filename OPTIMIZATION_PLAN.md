# Renderer optimization and cleanup plan

Status: all three approved milestones completed and validated.
Branch: `renderer-optimization-cleanup`, based on physics
commit `43426ba68a2f0e2bbb7751dcab37f97b3ca043d3`.

## Investigation completed

The committed [baseline report](benchmarks/optimization-baseline-m4-pro-2026-10-19.json)
records the measurements below. All renders used the existing physical integrator,
MIS, unlimited depth, seed 42, Rust 1.99.1 release, and the M4 Pro MacBook Pro.
The measured executable was verified against a fresh locked release build; its
SHA-256 was unchanged. Profiling captures remain in `target/optimization-baseline`.

- At 1920x1080 and 128 samples, a nine-configuration Metal sweep found that batch
  size mattered more than threadgroup size. The current 32/128 configuration took
  1.575s; 64/64 took 1.533s, including PNG encoding.
- An alternating baseline/candidate/candidate/baseline confirmation, with three
  timed renders after one warmup in each process, gave combined medians of 1.583s
  and 1.534s respectively: about 3.1% lower elapsed time. This is a candidate tuning
  improvement, not an implemented change or a guarantee for other scenes.
- A 20-second CPU Time Profiler window attributed about 29.3% of sampled CPU time
  to direct-light sampling, 26.5% to closest intersection, and 5.5% to medium
  lookup. These inclusive percentages overlap. Next-representable rounding also
  accounted for substantial samples; its geometric error guarantees must remain.
  The profiler stopped the process at its time limit, so this capture is a
  workload sample rather than a completed-frame timing measurement.
- The Metal trace recorded 24 command submissions across one warmup and five
  timed renders, or four batches per render. In the uninstrumented confirmation,
  render wall time exceeded summed GPU command duration by only about 1.9ms.
  This gives little evidence for prioritizing a CPU/GPU scheduling rewrite for
  the full-HD room. GPU counters and shader timelines were not enabled, so this
  trace does not identify an arithmetic or memory throughput limiter.
- At 256x144 and 16 samples, hardware traversal lost to linear traversal with 16
  spheres, but won with 64, 256, 1024, and 4096 spheres. The existing 64-sphere auto
  threshold remains supported by these scenes. Full run data includes setup and
  encoding times as well as GPU time.

## Milestone 1: reduce light visibility work

Primary files: `scene.rs`, `light_sampling.rs`, and the Metal query and integrator
shaders.

1. Add a CPU occlusion query that stops at the first blocker and avoids constructing
   a full shading surface. Add corresponding linear and hardware Metal queries;
   hardware shadow queries accept any valid intersection.
2. Use these queries for sphere-light and environment visibility. Infinite-plane
   light selection still needs the identity and distance of the closest hit.
3. Reject light samples with zero BSDF contribution or zero sided emission before
   computing offsets and tracing a shadow ray. Preserve random-number consumption,
   light selection probabilities, endpoint PDFs, and MIS weights.
4. Check finite shadow endpoints, rays originating inside geometry, self-shadow
   exclusion through the existing error bounds, and glass blockers. Keep the
   existing closed-surface transport model and all geometric precision safeguards.

Validate visibility against the existing closest-hit result across edge cases,
compare seeded linear-radiance images before and after, run analytical lighting
and actual GPU tests, and benchmark the room and dense scenes. Commit and push
this milestone after checks pass.

## Milestone 2: cache CPU media and confirm GPU tuning

Primary files: `scene.rs`, scene preparation, Metal medium metadata, and settings.

1. Precompute dielectric parent relationships for CPU transport, as the GPU
   already does. Determine the primary ray's initial medium once; update it only
   when a dielectric transmission crosses a boundary. Reflection retains its
   medium. Preserve arbitrary nesting without a fixed-size stack.
2. Avoid absorption exponentials for exactly zero absorption. Preserve Beer
   attenuation and incident-medium conductor constants for absorbing media.
3. Repeat batch/threadgroup comparisons on the physical room, dense hardware
   scenes, and a long-path cavity. Change defaults only if the measurements support
   a useful tradeoff. Preserve exact per-pixel sample counts and resumable paths.

Validate cameras inside media, nested entry/exit, internal reflection, absorption,
and immersed conductors. Preserve CPU worker and GPU batch determinism. Run lint,
physical tests, GPU validation, and sequential timing comparisons, then commit
and push the verified changes. A scheduling or wavefront rewrite would require
new profiling evidence and a separate plan.

## Milestone 3: code cleanup and reorganization

1. Separate Metal pipeline/resource preparation from command encoding and render
   orchestration. Keep unsafe buffer interop small, documented, and adjacent to
   its resource lifetime guarantees.
2. Format shaders into readable functions, replace numeric material tags with
   named constants, and separate direct lighting from path advancement. Keep
   embedded library assembly and offline shader compilation consistent.
3. Organize GPU tests by geometry, physical transport, and scheduling, with shared
   fixtures. Keep analytical assertions and tolerances intact.
4. Clarify old ray/camera field names and remove stale comments and imports.
   Remove the inactive `light.rs`, broken `photonmap.rs`, and empty `spectral.rs`
   stubs; their history remains in Git. They are outside the active module tree.
5. Consolidate repeated benchmark command/report helpers where useful and update
   the README architecture map and reproduction commands.

Keep cleanup commits separate from behavioral optimizations so each is reviewable.
Run formatting, strict Clippy with and without Metal, CPU tests, standalone Metal
shader compilation, and actual GPU tests with API/shader validation. Check Rust
1.88 support and the Linux/macOS/Windows CI matrix. Re-run representative timings
after reorganization to catch regressions. Commit and push each verified stage.

## Acceptance criteria

Changes retain the physical transport equations, unbiased termination, raw HDR
radiance, supported scenes, and deterministic sampling contract. Performance
claims require before/after measurements with the same compiler, workload, and
measurement method. Keep targeted optimizations that improve measured workloads;
investigate regressions before accepting them. Record the final revision and
results in a new benchmark report.

## Implementation record

Milestone 1 completed: CPU/Metal occlusion queries, strict endpoint and early-exit
tests, and CPU rejection of zero-contribution light samples. All 40 standard and
14 actual GPU tests passed; CPU and both GPU traversal images had exactly zero
MSE against the saved 320x180, 256-sample EXR baselines. Strict Clippy passed with
and without Metal, and the standalone Metal library compiled.

Early light rejection was tested on Metal but removed: an alternating comparison
showed a small-room regression. Isolating the change restored baseline GPU time
while retaining dedicated occlusion queries. The dense-scene measurements support
retaining the latter, including 4096-sphere hardware traversal at 0.0234s versus
the recorded 0.0281s baseline (PNG included). Final measurements will recheck the
combined implementation with the same workloads.

Milestone 2 completed: CPU paths retain their medium, with parent relationships
prepared once per scene, and zero absorption returns unit transmittance directly.
Tests now include 128 nested media, nonconcentric and disjoint boundaries, and a
CPU inside-glass internal-reflection series. The 43 standard and 14 GPU tests pass.
The original CPU and GPU EXR baselines still match exactly.

The room, 64-sphere and 4096-sphere hardware scenes, and emitting cavity all favored
64-sample batches with 64-thread groups. CLI and programmatic settings now share
these defaults. The tuning script accepts configurable workloads and sweep ranges.
A GPU zero-absorption branch gave no clear gain and was removed; GPU Beer
attenuation retains its original expression. CPU/GPU transport equations remain
unchanged. Final benchmarks will measure the retained implementation together.

Milestone 3 completed: Metal buffers, pipeline preparation, and dispatch each have
their own modules. Shaders are formatted and organized by sampling, geometry,
BSDFs, direct lighting, and path advancement. A source manifest drives both the
Rust embedding build and standalone compiler check. Named material and path fields
retain their layouts, with additional offset assertions. GPU tests are grouped by
geometry, scheduling, and transport; benchmark tools share invocation/report code.
The inactive legacy stubs were removed, and ray/camera names and architecture
documentation were clarified.

Rust 1.99.1 and Rust 1.88 tests pass with and without Metal. Strict Clippy and
format checks pass, as do all 14 GPU tests with Metal API/shader validation. The
final CPU and both GPU images have exactly zero MSE against the original EXRs.
The analytical/PBRT validation reproduces its previous results, including PBRT
reference MSE 0.0000011811. The implementation commit `368c53a` passed the full
[CI matrix](https://github.com/rglusic/rustytracer/actions/runs/39327627936).

The [final report](benchmarks/optimization-m4-pro-2026-10-19.json) records fresh
sequential before/after measurements: full-HD CPU time, including PNG, fell from
26.160s to 19.208s (26.6%); Metal linear fell from 1.573s to 1.529s (2.8%). Dense
hardware cases improved by about 8–14%. The shortest 16-sphere linear case was
0.35ms slower, so a larger alternating followup was run; that workload improved
from 0.3443s to 0.3319s. All raw case timings and measurement conditions are retained.

## References checked during investigation

- [Apple: shadow queries accepting any intersection](https://developer.apple.com/videos/play/wwdc2021/10150/)
- [Apple: scaling compute workloads and interpreting GPU gaps](https://developer.apple.com/videos/play/wwdc2022/10159/)
- [Apple: calculating threadgroup and grid sizes](https://developer.apple.com/documentation/metal/calculating-threadgroup-and-grid-sizes)
