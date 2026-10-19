// Next-event estimation and matching solid-angle PDFs for MIS.
Material hit_material(Hit hit, device const Sphere *spheres, device const Plane *planes) {
    return hit.sphere ? spheres[hit.index].material : planes[hit.index].material;
}
float3 emitted(Material m, float3 n, float3 outgoing) {
    return m.two_sided || dot(n, outgoing) > 0 ? m.emission.xyz : float3(0);
}
float power_heuristic(float a, float b) {
    if (a == 0)
        return 0;
    float r = b / a;
    return 1 / (1 + r * r);
}

// Query supplies the same intersection semantics for direct and accelerated traversal.
float environment_pdf(constant Parameters &p) {
    return any(p.environment.xyz > 0) ? 1 / (4 * PI * p.light_count) : 0;
}
float endpoint_pdf(Hit h, Surface s, float3 from, constant Parameters &p,
                   device const Sphere *spheres, device const Plane *planes) {
    if (!any(hit_material(h, spheres, planes).emission.xyz > 0))
        return 0;
    if (!h.sphere)
        return 1 / (4 * PI * p.light_count);
    float3 delta = s.point - from;
    float cosine = abs(dot(s.normal, -normalize(delta))), r = spheres[h.index].center_radius.w;
    return cosine == 0 ? 0 : dot(delta, delta) / (4 * PI * r * r * cosine * p.light_count);
}
template <typename Query>
float3 direct_light(Query query, Surface s, float3 wo, float3 n, Material m, float3 absorption,
                    constant Parameters &p, device const Sphere *spheres,
                    device const Plane *planes, device const uint4 *lights, thread uint &rng) {
    if (p.light_count == 0)
        return 0;
    uint selected = min(uint(random_value(rng) * p.light_count), p.light_count - 1);
    uint4 light = lights[selected];
    float u = random_value(rng), v = random_value(rng);
    float3 wi = uniform_sphere(u, v), le;
    float distance = INFINITY, pdf = 1 / (4 * PI * p.light_count);
    if (light.x == LightEnvironment) {
        if (query.occluded(offset_origin(s, wi), wi))
            return 0;
        le = p.environment.xyz;
    } else if (light.x == LightPlane) {
        Hit h = query.intersect(offset_origin(s, wi), wi);
        if (!h.found || h.sphere || h.index != light.y)
            return 0;
        le = emitted(planes[light.y].material, h.normal, -wi);
        distance = h.distance;
    } else {
        Sphere sphere = spheres[light.y];
        float3 normal = wi, point = sphere.center_radius.xyz + sphere.center_radius.w * normal;
        float3 delta = point - s.point;
        distance = length(delta);
        if (distance == 0)
            return 0;
        wi = delta / distance;
        float cosine = abs(dot(normal, -wi));
        if (cosine == 0)
            return 0;
        pdf *= distance * distance / (sphere.center_radius.w * sphere.center_radius.w * cosine);
        Surface target = {point, normal,
                          (abs(sphere.center_radius.xyz) + abs(point) + sphere.center_radius.w) *
                              gamma_error()};
        float3 from = offset_origin(s, wi), to = offset_origin(target, -wi), shadow = to - from;
        if (query.occluded(from, normalize(shadow), length(shadow)))
            return 0;
        le = emitted(sphere.material, normal, -wi);
    }
    Evaluation e = evaluate_bsdf(m, wo, wi, n);
    if (e.pdf == 0)
        return 0;
    float3 transmittance = isfinite(distance) ? exp(-absorption * distance)
                                              : select(float3(0), float3(1), absorption == 0);
    return e.value * le * transmittance * (abs(dot(n, wi)) * power_heuristic(pdf, e.pdf) / pdf);
}
