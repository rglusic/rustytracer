#include <metal_raytracing>
using namespace raytracing;

struct SphereIntersection {
    bool accept [[accept_intersection]];
    float distance [[distance]];
};

[[intersection(bounding_box)]]
SphereIntersection intersect_sphere(float3 origin [[origin]],
                                    float3 direction [[direction]],
                                    float minimum [[min_distance]],
                                    float maximum [[max_distance]],
                                    uint primitive [[primitive_id]],
                                    device const Sphere *spheres [[buffer(0)]]) {
    float t = sphere_distance(origin, direction, spheres[primitive].center_radius, maximum);
    return {t >= minimum && t > 0.0f, t};
}

Hit intersect_hardware(float3 origin, float3 direction, constant Parameters &p,
                       device const Sphere *spheres, device const Plane *planes,
                       primitive_acceleration_structure acceleration,
                       intersection_function_table<> functions) {
    Hit hit = intersect_planes(origin, direction, p, planes);
    ray r;
    r.origin = origin;
    r.direction = direction;
    r.min_distance = 0.001f;
    r.max_distance = hit.distance;
    intersector<> tester;
    tester.assume_geometry_type(geometry_type::bounding_box);
    auto result = tester.intersect(r, acceleration, functions);
    if (result.type != intersection_type::none && result.distance < hit.distance) {
        Sphere sphere = spheres[result.primitive_id];
        hit = {result.distance, normalize(origin + result.distance * direction - sphere.center_radius.xyz),
               sphere.color.xyz, sphere.material.x, true};
    }
    return hit;
}

kernel void trace_hardware(constant Parameters &p [[buffer(0)]],
                           device const Sphere *spheres [[buffer(1)]],
                           device const Plane *planes [[buffer(2)]],
                           device float4 *accumulation [[buffer(3)]],
                           primitive_acceleration_structure acceleration [[buffer(4)]],
                           intersection_function_table<> functions [[buffer(5)]],
                           uint pixel [[thread_position_in_grid]]) {
    if (pixel >= p.width * p.height) return;
    float3 sum = p.sample_start == 0 ? float3(0.0f) : accumulation[pixel].xyz;
    for (uint s = 0; s < p.sample_count; ++s) {
        uint state = sample_seed(p.seed, pixel, p.sample_start + s);
        float3 origin = p.camera[0].xyz;
        float3 direction = primary_direction(pixel, p, state);
        float3 throughput = 1.0f;
        for (uint depth = 0; depth < p.max_bounces; ++depth) {
            if (depth > 3 && random_value(state) > 0.3f) break;
            Hit hit = intersect_hardware(origin, direction, p, spheres, planes, acceleration, functions);
            if (!hit.found) break;
            if (hit.material == 3) {
                sum += throughput * float3(2.5f, 2.5f, 0.98431372549f * 2.5f);
                break;
            }
            origin += hit.distance * direction;
            direction = scatter(direction, hit.normal, hit.material, state);
            throughput *= hit.color;
        }
    }
    accumulation[pixel] = float4(sum, 0.0f);
}
