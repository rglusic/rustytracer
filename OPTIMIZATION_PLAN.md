# Renderer optimization and cleanup plan

Status: proposed, awaiting approval before implementation under the supplied
AGENTS.md instructions. Branch: `renderer-optimization-cleanup`, based on physics
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

## References checked during investigation

- [Apple: shadow queries accepting any intersection](https://developer.apple.com/videos/play/wwdc2021/10150/)
- [Apple: scaling compute workloads and interpreting GPU gaps](https://developer.apple.com/videos/play/wwdc2022/10159/)
- [Apple: calculating threadgroup and grid sizes](https://developer.apple.com/documentation/metal/calculating-threadgroup-and-grid-sizes)
