#include <metal_stdlib>
using namespace metal;
constant float PI = 3.14159265358979323846f;
enum MaterialKind : uint {
    MaterialDiffuse = 0,
    MaterialConductor = 1,
    MaterialDielectric = 2,
    MaterialEmitter = 3,
    MaterialMirror = 4
};
enum LightKind : uint { LightSphere = 0, LightPlane = 1, LightEnvironment = 2 };
struct Material {
    float4 albedo, emission, eta_roughness, k_ior, absorption;
    uint kind, two_sided, parent_medium, reserved;
};
struct Sphere {
    float4 center_radius;
    Material material;
};
struct Plane {
    float4 origin, normal;
    Material material;
};
struct Parameters {
    float4 camera[4];
    uint width, height, sample_start, sample_count;
    uint sphere_count, plane_count, seed, max_bounces;
    float4 environment;
    uint reset, light_count, light_sampling, camera_medium;
};
struct PathState {
    float4 origin, direction, beta, radiance, previous_point;
    uint rng, depth, completed_samples, previous_delta;
    uint medium_id, reserved_medium[3];
    float eta_scale, previous_pdf, reserved_weights[2];
};
static_assert(sizeof(Material) == 96, "Material layout");
static_assert(sizeof(Sphere) == 112, "Sphere layout");
static_assert(sizeof(Plane) == 128, "Plane layout");
static_assert(sizeof(Parameters) == 128, "Parameters layout");
static_assert(sizeof(PathState) == 128, "PathState layout");
