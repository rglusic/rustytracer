// Persistent path advancement. Dispatch limits schedule work without truncation.
void initialize_path(thread PathState &path, uint pixel, uint sample, constant Parameters &p) {
    uint rng = sample_seed(p.seed, pixel, sample);
    path.origin = float4(p.camera[0].xyz, 0);
    path.direction = float4(primary_direction(pixel, p, rng), 0);
    path.beta = float4(1, 1, 1, 0);
    path.radiance = 0;
    path.previous_point = path.origin;
    path.rng = rng;
    path.depth = 0;
    path.completed_samples = sample;
    path.previous_delta = 1;
    path.medium_id = p.camera_medium;
    path.eta_scale = 1;
    path.previous_pdf = 0;
    for (uint i = 0; i < 3; ++i)
        path.reserved_medium[i] = 0;
    for (uint i = 0; i < 2; ++i)
        path.reserved_weights[i] = 0;
}
template <typename Query>
bool advance_path(thread PathState &path, Query query, constant Parameters &p,
                  device const Sphere *spheres, device const Plane *planes,
                  device const uint4 *lights) {
    float eta_i = path.medium_id ? spheres[path.medium_id - 1].material.k_ior.w : 1;
    float3 absorption =
        path.medium_id ? spheres[path.medium_id - 1].material.absorption.xyz : float3(0);
    Hit hit = query.intersect(path.origin.xyz, path.direction.xyz);
    if (!hit.found) {
        float weight = path.previous_delta || !p.light_sampling
                           ? 1
                           : power_heuristic(path.previous_pdf, environment_pdf(p));
        path.radiance.xyz += path.beta.xyz * p.environment.xyz * weight;
        return true;
    }
    path.beta.xyz *= exp(-absorption * hit.distance);
    Surface surface = hit_surface(path.origin.xyz, path.direction.xyz, hit, spheres, planes);
    Material m = hit_material(hit, spheres, planes);
    if (m.kind == MaterialConductor) {
        m.eta_roughness.xyz /= eta_i;
        m.k_ior.xyz /= eta_i;
    }
    float3 wo = -path.direction.xyz;
    bool front = dot(surface.normal, wo) > 0;
    float3 n = front ? surface.normal : -surface.normal;
    float weight =
        path.previous_delta || !p.light_sampling
            ? 1
            : power_heuristic(path.previous_pdf, endpoint_pdf(hit, surface, path.previous_point.xyz,
                                                              p, spheres, planes));
    path.radiance.xyz += path.beta.xyz * emitted(m, surface.normal, wo) * weight;
    if (m.kind == MaterialEmitter || (p.max_bounces && path.depth >= p.max_bounces))
        return true;
    uint rng = path.rng;
    if (p.light_sampling && !is_delta(m))
        path.radiance.xyz += path.beta.xyz * direct_light(query, surface, wo, n, m, absorption, p,
                                                          spheres, planes, lights, rng);
    uint target_medium =
        m.kind == MaterialDielectric ? (front ? hit.index + 1 : m.parent_medium) : 0;
    float eta_t = target_medium ? spheres[target_medium - 1].material.k_ior.w : 1;
    if (m.kind != MaterialDielectric)
        eta_t = m.k_ior.w;
    BsdfSample sample = sample_bsdf(m, wo, n, eta_i, eta_t, rng);
    if (!sample.valid)
        return true;
    path.beta.xyz *= sample.value * (abs(dot(n, sample.wi)) / sample.pdf);
    if (max(max(path.beta.x, path.beta.y), path.beta.z) <= 0)
        return true;
    if (sample.transmission) {
        path.eta_scale *= sample.eta * sample.eta;
        path.medium_id = target_medium;
    }
    path.depth++;
    if (path.depth >= 4) {
        float survival =
            min(max(max(path.beta.x, path.beta.y), path.beta.z) * path.eta_scale, 0.95f);
        if (random_value(rng) >= survival)
            return true;
        path.beta.xyz /= survival;
    }
    path.previous_point = float4(surface.point, 0);
    path.previous_pdf = sample.pdf;
    path.previous_delta = sample.delta;
    path.rng = rng;
    path.origin = float4(offset_origin(surface, sample.wi), 0);
    path.direction = float4(normalize(sample.wi), 0);
    return false;
}
template <typename Query>
void trace_paths(uint pixel, Query query, constant Parameters &p, device const Sphere *spheres,
                 device const Plane *planes, device float4 *accumulation, device PathState *paths,
                 device atomic_uint *unfinished, device const uint4 *lights) {
    if (pixel >= p.width * p.height)
        return;
    PathState path;
    float3 sum;
    if (p.reset) {
        initialize_path(path, pixel, 0, p);
        sum = 0;
    } else {
        path = paths[pixel];
        sum = accumulation[pixel].xyz;
    }
    uint target = p.sample_start + p.sample_count;
    // Bound GPU work without truncating a path. Its full state survives the dispatch.
    for (uint step = 0; step < min(p.sample_count * 16, 256u) && path.completed_samples < target;
         ++step) {
        if (advance_path(path, query, p, spheres, planes, lights)) {
            sum += path.radiance.xyz;
            initialize_path(path, pixel, path.completed_samples + 1, p);
        }
    }
    paths[pixel] = path;
    accumulation[pixel] = float4(sum, 0);
    if (path.completed_samples < target)
        atomic_fetch_add_explicit(unfinished, 1u, memory_order_relaxed);
}
