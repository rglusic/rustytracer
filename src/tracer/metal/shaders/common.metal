#include <metal_stdlib>
using namespace metal;

struct Sphere { float4 center_radius; float4 color; uint4 material; };
struct Plane { float4 origin; float4 normal; float4 color; uint4 material; };
struct Parameters {
    float4 camera[4];
    uint width, height, sample_start, sample_count;
    uint sphere_count, plane_count, seed, max_bounces;
};
static_assert(sizeof(Sphere) == 48, "Sphere layout");
static_assert(sizeof(Plane) == 64, "Plane layout");
static_assert(sizeof(Parameters) == 96, "Parameters layout");

uint mix_bits(uint x) {
    x = (x ^ (x >> 16)) * 0x7feb352du;
    x = (x ^ (x >> 15)) * 0x846ca68bu;
    return x ^ (x >> 16);
}
uint sample_seed(uint seed, uint pixel, uint sample) {
    return mix_bits(seed ^ mix_bits(pixel) ^ mix_bits(sample + 0x9e3779b9u));
}
float random_value(thread uint &state) {
    state = state * 747796405u + 2891336453u;
    uint word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return float(((word >> 22u) ^ word) >> 8u) * (1.0f / 16777216.0f);
}
float3 random_sphere(thread uint &state) {
    for (;;) {
        float x = random_value(state);
        float y = random_value(state);
        float z = random_value(state);
        float3 point = 2.0f * float3(x, y, z) - 1.0f;
        if (dot(point, point) < 1.0f) return point;
    }
}

float sphere_distance(float3 origin, float3 direction, float4 sphere, float maximum) {
    float3 oc = origin - sphere.xyz;
    float a = dot(direction, direction);
    float b = dot(oc, direction);
    float c = dot(oc, oc) - sphere.w * sphere.w;
    float discriminant = b * b - a * c;
    if (a == 0.0f || discriminant < 0.0f) return 0.0f;
    float root = sqrt(discriminant);
    float t = (-b - root) / a;
    if (t > 0.001f && t < maximum) return t;
    t = (-b + root) / a;
    return t > 0.001f && t < maximum ? t : 0.0f;
}

struct Hit {
    float distance;
    float3 normal;
    float3 color;
    uint material;
    bool found;
};

Hit intersect_planes(float3 origin, float3 direction, constant Parameters &p, device const Plane *planes) {
    Hit hit;
    hit.distance = MAXFLOAT;
    hit.found = false;
    for (uint i = 0; i < p.plane_count; ++i) {
        float denominator = dot(direction, planes[i].normal.xyz);
        if (abs(denominator) <= 0.001f) continue;
        float t = dot(planes[i].origin.xyz - origin, planes[i].normal.xyz) / denominator;
        if (t >= 0.001f && t < hit.distance) {
            hit = { t, planes[i].normal.xyz, planes[i].color.xyz, planes[i].material.x, true };
        }
    }
    return hit;
}

Hit intersect_linear(float3 origin, float3 direction, constant Parameters &p,
                     device const Sphere *spheres, device const Plane *planes) {
    Hit hit = intersect_planes(origin, direction, p, planes);
    for (uint i = 0; i < p.sphere_count; ++i) {
        float t = sphere_distance(origin, direction, spheres[i].center_radius, hit.distance);
        if (t > 0.0f) {
            hit = {t, normalize(origin + t * direction - spheres[i].center_radius.xyz),
                   spheres[i].color.xyz, spheres[i].material.x, true};
        }
    }
    return hit;
}

float3 scatter(float3 direction, float3 normal, uint material, thread uint &state) {
    if (material == 0) return normalize(normal + random_sphere(state));
    float3 incoming = normalize(direction);
    if (material == 1) return reflect(incoming, normal);
    bool entering = dot(incoming, normal) < 0.0f;
    float3 outward = entering ? normal : -normal;
    float ratio = entering ? (1.0f / 1.5f) : 1.5f;
    float cosine = clamp(-dot(incoming, outward), 0.0f, 1.0f);
    float discriminant = 1.0f - ratio * ratio * (1.0f - cosine * cosine);
    float q = 1.0f - cosine;
    float probability = 0.04f + 0.96f * q * q * q * q * q;
    if (discriminant < 0.0f || random_value(state) < probability) return reflect(incoming, outward);
    return ratio * incoming + (ratio * cosine - sqrt(discriminant)) * outward;
}

float3 primary_direction(uint pixel, constant Parameters &p, thread uint &state) {
    float u = (float(pixel % p.width) + random_value(state)) / float(p.width);
    float v = (float(pixel / p.width) + random_value(state)) / float(p.height);
    return p.camera[1].xyz + u * p.camera[2].xyz + v * p.camera[3].xyz - p.camera[0].xyz;
}
