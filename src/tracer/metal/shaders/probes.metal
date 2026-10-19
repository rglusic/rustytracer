kernel void probe_rng(constant Parameters &p [[buffer(0)]],
                      device float4 *output [[buffer(3)]], uint pixel [[thread_position_in_grid]]) {
    uint state = sample_seed(p.seed, pixel, 7);
    float a = random_value(state), b = random_value(state), c = random_value(state);
    output[pixel] = float4(a, b, c, 0);
}

void probe_ray(uint pixel, thread float3 &origin, thread float3 &direction) {
    origin = float3(0);
    direction = float3(0, 0, -1);
    switch (pixel) {
        case 1: direction = float3(1, 0, 0); break;
        case 2: origin = float3(0, 0, -3); break;
        case 3: origin = float3(1, 0, 0); break;
        case 4: direction *= 2; break;
        case 5: direction = float3(0, 0, 1); break;
        case 6: origin = float3(0, -2, 0); break;
        case 7: origin = float3(0, 0, -3.998f); break;
    }
}

kernel void probe_linear(constant Parameters &p [[buffer(0)]], device const Sphere *spheres [[buffer(1)]],
                         device const Plane *planes [[buffer(2)]], device float4 *output [[buffer(3)]],
                         uint pixel [[thread_position_in_grid]]) {
    float3 origin, direction;
    probe_ray(pixel, origin, direction);
    Hit hit = intersect_linear(origin, direction, p, spheres, planes);
    output[pixel] = float4(hit.found ? hit.distance : -1.0f, hit.found ? hit.normal.x : 0.0f, hit.found ? hit.normal.y : 0.0f, 0);
}

kernel void probe_hardware(constant Parameters &p [[buffer(0)]], device const Sphere *spheres [[buffer(1)]],
                           device const Plane *planes [[buffer(2)]], device float4 *output [[buffer(3)]],
                           primitive_acceleration_structure acceleration [[buffer(4)]],
                           intersection_function_table<> functions [[buffer(5)]],
                           uint pixel [[thread_position_in_grid]]) {
    float3 origin, direction;
    probe_ray(pixel, origin, direction);
    Hit hit = intersect_hardware(origin, direction, p, spheres, planes, acceleration, functions);
    output[pixel] = float4(hit.found ? hit.distance : -1.0f, hit.found ? hit.normal.x : 0.0f, hit.found ? hit.normal.y : 0.0f, 0);
}

kernel void probe_glass(constant Parameters &p [[buffer(0)]], device float4 *output [[buffer(3)]],
                        uint pixel [[thread_position_in_grid]]) {
    uint state = sample_seed(p.seed, pixel, 0);
    float scale = pixel == 0 ? 0.1f : (pixel == 1 ? 1.0f : 10.0f);
    Material m = {}; m.flags.x = 2;
    float3 wo = -normalize(scale * float3(0.9f, 0.4358898943540673f, 0));
    float3 reflected = sample_bsdf(m, wo, float3(0,-1,0), 1.5f, 1.0f, state).wi;
    output[pixel] = float4(reflected, 0);
}

template<typename Query>
float4 probe_visibility(uint pixel,Query query) {
    float3 origin,direction;
    probe_ray(pixel%8,origin,direction);
    float maximum=pixel/8==0 ? MAXFLOAT : (pixel/8==1 ? 2.0f : (pixel/8==2 ? nextafter(2.0f,INFINITY) : 0.0f));
    return float4(query.occluded(origin,direction,maximum),query.intersect(origin,direction,maximum).found,0,0);
}
kernel void probe_visibility_linear(constant Parameters &p [[buffer(0)]],device const Sphere *spheres [[buffer(1)]],
                                    device const Plane *planes [[buffer(2)]],device float4 *output [[buffer(3)]],
                                    uint pixel [[thread_position_in_grid]]) {
    output[pixel]=probe_visibility(pixel,LinearQuery{&p,spheres,planes});
}
kernel void probe_visibility_hardware(constant Parameters &p [[buffer(0)]],device const Sphere *spheres [[buffer(1)]],
                                      device const Plane *planes [[buffer(2)]],device float4 *output [[buffer(3)]],
                                      primitive_acceleration_structure acceleration [[buffer(4)]],intersection_function_table<> functions [[buffer(5)]],
                                      uint pixel [[thread_position_in_grid]]) {
    output[pixel]=probe_visibility(pixel,HardwareQuery{&p,spheres,planes,acceleration,functions});
}
