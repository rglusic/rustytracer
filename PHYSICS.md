# Physical transport model

RustyTracer estimates steady-state, unpolarized surface radiance in linear RGB.
Scene coordinates and sphere radii are in metres. RGB channels approximate spectral
transport. Wavelength-dependent dispersion requires spectral transport; polarization
requires carrying the light's polarization state.

## Transport estimator

At every surface, the integrator accumulates emitted radiance and, for continuous
BSDFs, one direct-light sample. Continuation weights are `f * abs(dot(n, wi)) / pdf`.
For ideal reflection/transmission, `pdf` is the discrete branch probability rather
than a density per steradian. Radiance from BSDF-sampled emitter hits is combined
with direct-light estimates using the power heuristic. Camera and delta-event
emitter hits retain weight one. Light selection probabilities are included in both
strategies. Area PDFs are converted to solid-angle PDFs with distance squared and
the emitter cosine; ray radiance itself does not receive an extra inverse-square factor.

From the fourth scattering event, Russian roulette uses
`survival = min(max_component(throughput) * eta_scale, 0.95)`. Surviving paths divide
throughput by survival. Transmission updates eta_scale to avoid terminating paths
merely because radiance changes across refractive boundaries. The 0.95 ceiling
bounds expected work even in a perfectly reflecting closed cavity; it affects
variance and execution time, not expected radiance.

The default `--max-bounces 0` has no depth truncation. An explicit nonzero limit
is a diagnostic approximation and prints a bias notice. A depth limit counts
scattering events: emission at the final reached vertex is still accumulated.

On Metal, each pixel retains its ray, throughput, accumulated radiance, RNG state,
sample index, depth, previous BSDF PDF, and medium. A dispatch advances at most 256
path vertices per pixel. Unfinished paths resume in subsequent dispatches. Batching
changes scheduling rather than which paths or samples are included. A completed
sample is added to the film exactly once.

## Materials

Objects accept a `mat` object. All input vectors must have three finite components;
reflectance is in [0,1], emission and absorption are nonnegative. Unknown material
names, fields, and incompatible parameters are errors.

```json
{"kind":"diffuse", "albedo":[0.65,0.25,0.1]}
{"kind":"mirror", "albedo":[0.95,0.95,0.95]}
{"kind":"conductor", "eta":[0.2,0.9,1.1], "k":[3.9,2.5,2.1], "roughness":0.25}
{"kind":"dielectric", "ior":1.5, "absorption":[0.1,0.02,0.01]}
{"kind":"emitter", "emission":[10,10,9.84], "two_sided":false}
```

The conductor constants above are illustrative RGB optical parameters, not a claim
of a measured material identity. `eta` and `k` describe the complex refractive
index relative to vacuum; at an immersed interface the constants are divided by
the surrounding medium IOR. The roughness parameter is in [0,1], with GGX alpha = roughness squared.
Zero roughness uses ideal specular reflection. Positive roughness uses the
single-scattering, height-correlated Smith GGX BRDF, with exact conductor Fresnel,
and a matched microfacet-normal sampling PDF. Samples below the macro surface
contribute zero without resampling. This is the standard single-scattering
microfacet approximation: microfacet interreflection is omitted, so rough metals
can be darker than a full multiple-scattering microsurface. No empirical energy
boost or radiance clamp compensates for that omitted effect.

Diffuse reflection is Lambertian, `albedo / pi`, sampled with `cos(theta) / pi`.
Mirrors represent ideal specular interfaces with the supplied RGB reflectance. Their constant reflectance is a declared ideal model, not a measured
conductor. Smooth dielectrics use the exact unpolarized Fresnel equations, Snell's
law, total internal reflection, and the radiance transmission factor
`(eta_incident / eta_transmitted)^2`.

Dielectric media are closed spheres. They must be disjoint or strictly nested;
intersecting or coincident dielectric boundaries are rejected. The smallest
containing dielectric determines the current medium, including cameras inside
objects. CPU and Metal store parent-medium relationships, avoiding a fixed-size
stack and repeated containment searches at secondary vertices. The medium changes
only on transmission; reflection preserves the incident medium.
IOR defaults to 1.5 for glass; ambient vacuum/air is modeled as IOR 1. Absorption
is a per-channel coefficient in inverse metres, with segment transmittance
`exp(-absorption * distance)`. Tint is therefore thickness-dependent rather than
multiplied once per surface crossing. Dielectric planes are rejected because the
current scene contract requires closed medium boundaries.

Any material may also specify emission. `two_sided` controls emission only;
opaque scattering surfaces can be viewed from either side. Emission is radiance,
not albedo, and may exceed one. Pure emitters absorb incident paths. Sphere lights
sample surface area; infinite emissive planes sample directions and remain ideal
infinite-extent sources. A constant `environment: [r,g,b]` is optional and defaults
to black. Both infinite-light strategies use explicit directional PDFs.

## Legacy scene migration

The original `worlds/closed_room.json` remains loadable. Historical string materials
have explicit meanings:

| Name | Meaning of object color |
| --- | --- |
| `flat` | Lambertian reflectance, each channel at most one |
| `metal` | Reflectance of the historical ideal mirror; use conductor parameters for a physical metal |
| `glass` | Must be white; IOR 1.5, clear interior |
| `diffuse_light` | Emitted radiance; the old hardcoded 2.5 multiplier is removed |

The new default `worlds/physical_room.json` specifies materials and light radiance
explicitly. Old images change because the biased termination and diffuse law were
removed. The old perfectly white room also has much higher long-path variance
under compensated roulette; it is not an appropriate pointwise f32/f64 comparison
fixture at a modest sample count.

## Numerical geometry and film

Sphere roots use closest-approach geometry for the discriminant and a stable
quadratic-root pair. Hit points are reprojected onto the sphere. Secondary and
shadow rays use scale-dependent floating-point error bounds and next-representable
rounding along the geometric normal, including both shadow-ray endpoints. Plane
parallelism uses a zero denominator test rather than rejecting a fixed band of
grazing directions. There is no fixed world-distance intersection cutoff.

CPU calculations use f64 and Metal uses f32. GPU scene packing rejects non-finite,
underflowed, and unsupported-range parameters, as well as camera directions that
collapse at f32 precision. These remain finite-precision geometric calculations;
very large translations, tiny separations, and tangent configurations require care.

The film stores unclipped scene-linear radiance. OpenEXR saves RGB float data
without applying display exposure. PNG/JPEG apply `2^exposure_ev`, clip to display
range, then use the piecewise sRGB transfer function and 8-bit quantization.
Exposure and display clipping do not alter transport or MSE calculations.

## Verification and reproducibility

Unit tests cover BSDF/PDF consistency, Fresnel at normal and Brewster angles,
Snell refraction, total internal reflection, conductor passivity and reciprocity,
absorption, invalid scenes, HDR round trips, and ray spawning across scales from
1e-8 to 1e8. Analytical scene tests include a diffuse furnace, spherical emitter
irradiance, and an emitting diffuse cavity with `L = Le / (1 - reflectance)`.
The cavity exercises compensated roulette and demonstrates the bias of an explicit
depth cap. Actual GPU tests include resuming unfinished cavity paths, nested glass,
rough conductors, and both traversal modes, under Apple API/shader validation. A
camera at the centre of an absorbing glass sphere is checked against its analytic
internal-reflection series, including the initial medium and radiance scaling.

```sh
python3 scripts/validate_physics.py
python3 scripts/validate_physics.py --pbrt /path/to/pbrt-v3/build/pbrt
```

The validation script compares 16x16 radiance images against the spherical-light
analytic result 0.125, at four sample counts and four independent seeds, with and
without MIS. It writes per-run MSE and render time under `target/physics-validation`.
`validation/sphere-light.pbrt` is an independently rendered, matching PBRT v3 RGB
scene. Its finite receiver covers the full camera footprint; the outgoing
Lambertian hemisphere never returns to the plane, so extending the receiver to an
infinite plane does not change the light paths. PBRT uses its own sampler, direct
light strategy and geometry code.

PBRT v3 is used for this comparison because its RGB transport matches this engine's
color model. Build instructions are in the upstream repository. The validation
run used commit `13d871faae88233b327d04cda24022b8bb0093ee`, a release build with
`PBRT_FLOAT_AS_DOUBLE=ON`, and 32768 samples per pixel in each renderer. The
reference checks a Lambertian spherical-light scene; it does not certify every
material or difficult caustic configuration. Detailed results are retained in the
committed physics benchmark report.

## References

- [PBRT: path transport and MIS](https://pbr-book.org/4ed/Light_Transport_I_Surface_Reflection/A_Better_Path_Tracer)
- [PBRT: Lambertian reflection](https://pbr-book.org/4ed/Reflection_Models/Diffuse_Reflection)
- [PBRT: dielectric transport](https://pbr-book.org/4ed/Reflection_Models/Dielectric_BSDF)
- [PBRT: conductor reflection](https://pbr-book.org/4ed/Reflection_Models/Conductor_BRDF)
- [PBRT: microfacet assumptions and limitations](https://pbr-book.org/4ed/Reflection_Models/Roughness_Using_Microfacet_Theory)
- [PBRT: floating-point geometry](https://pbr-book.org/4ed/Shapes/Managing_Rounding_Error)
- [PBRT v3 source and build instructions](https://github.com/mmp/pbrt-v3)
