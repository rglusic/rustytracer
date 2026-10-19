kernel void trace_linear(constant Parameters &p [[buffer(0)]],
                         device const Sphere *spheres [[buffer(1)]],
                         device const Plane *planes [[buffer(2)]],
                         device float4 *accumulation [[buffer(3)]],
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
            Hit hit = intersect_linear(origin, direction, p, spheres, planes);
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
