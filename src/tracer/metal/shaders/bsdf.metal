// The equations and sampling conventions match material.rs, using Metal f32.
float dielectric_fresnel(float c, float eta_i, float eta_t) {
    c = clamp(c, 0.0f, 1.0f);
    float ratio = eta_i / eta_t, sin2 = ratio * ratio * max(0.0f, 1 - c * c);
    if (sin2 >= 1)
        return 1;
    float t = sqrt(1 - sin2);
    float rs = (eta_i * c - eta_t * t) / (eta_i * c + eta_t * t),
          rp = (eta_t * c - eta_i * t) / (eta_t * c + eta_i * t);
    return 0.5f * (rs * rs + rp * rp);
}
float3 conductor_fresnel(float c, float3 eta, float3 k) {
    c = clamp(c, 0.0f, 1.0f);
    float s2 = 1 - c * c;
    float3 t0 = eta * eta - k * k - s2;
    float3 a2b2 = sqrt(t0 * t0 + 4 * eta * eta * k * k),
           a = sqrt(max(float3(0), (a2b2 + t0) * 0.5f));
    float3 t1 = a2b2 + c * c, t2 = 2 * c * a, rs = (t1 - t2) / (t1 + t2);
    float3 t3 = c * c * a2b2 + s2 * s2, t4 = t2 * s2;
    return 0.5f * rs * (1 + (t3 - t4) / (t3 + t4));
}
float ggx_lambda(float c, float a) {
    return 0.5f * (sqrt(1 + a * a * max(0.0f, 1 - c * c) / (c * c)) - 1);
}
struct Evaluation {
    float3 value;
    float pdf;
};
Evaluation evaluate_bsdf(Material m, float3 wo, float3 wi, float3 n) {
    float no = dot(n, wo), ni = dot(n, wi);
    if (no <= 0 || ni <= 0)
        return {float3(0), 0};
    if (m.kind == MaterialDiffuse)
        return {m.albedo.xyz / PI, ni / PI};
    if (m.kind == MaterialConductor && m.eta_roughness.w > 0) {
        float3 h = normalize(wo + wi);
        float nh = max(0.0f, dot(n, h)), oh = abs(dot(wo, h));
        if (oh == 0)
            return {float3(0), 0};
        float a = m.eta_roughness.w * m.eta_roughness.w;
        float3 tangent = cross(n, h);
        float denominator = dot(tangent, tangent) + nh * nh * a * a;
        float ratio = a / denominator;
        float d = ratio * ratio / PI;
        float g = 1 / (1 + ggx_lambda(no, a) + ggx_lambda(ni, a));
        return {conductor_fresnel(oh, m.eta_roughness.xyz, m.k_ior.xyz) * (d * g / (4 * no * ni)),
                d * nh / (4 * oh)};
    }
    return {float3(0), 0};
}
struct BsdfSample {
    float3 wi, value;
    float pdf, eta;
    bool delta, transmission, valid;
};
BsdfSample sample_bsdf(Material m, float3 wo, float3 n, float eta_i, float eta_t,
                       thread uint &rng) {
    float no = dot(n, wo);
    BsdfSample s;
    s.valid = false;
    s.delta = false;
    s.transmission = false;
    s.eta = 1;
    if (no <= 0 || m.kind == MaterialEmitter)
        return s;
    if (m.kind == MaterialDiffuse) {
        float u = random_value(rng), v = random_value(rng), r = sqrt(u), phi = 2 * PI * v;
        s.wi = local_to_world(float3(r * cos(phi), r * sin(phi), sqrt(1 - u)), n);
    } else if (m.kind == MaterialConductor && m.eta_roughness.w > 0) {
        float u = random_value(rng), a = m.eta_roughness.w * m.eta_roughness.w;
        float slope = a * sqrt(u / (1 - u)), phi = 2 * PI * random_value(rng);
        float3 h = local_to_world(normalize(float3(slope * cos(phi), slope * sin(phi), 1)), n);
        if (dot(wo, h) <= 0)
            return s;
        s.wi = reflect(-wo, h);
        if (dot(n, s.wi) <= 0)
            return s;
    } else if (m.kind == MaterialConductor || m.kind == MaterialMirror) {
        s.wi = reflect(-wo, n);
        s.value =
            (m.kind == MaterialMirror ? m.albedo.xyz
                                      : conductor_fresnel(no, m.eta_roughness.xyz, m.k_ior.xyz)) /
            no;
        s.pdf = 1;
        s.delta = true;
        s.valid = true;
        return s;
    } else {
        float ratio = eta_i / eta_t, sin2 = ratio * ratio * max(0.0f, 1 - no * no),
              f = dielectric_fresnel(no, eta_i, eta_t);
        s.delta = true;
        if (sin2 >= 1 || random_value(rng) < f) {
            s.wi = reflect(-wo, n);
            s.value = float3(f / no);
            s.pdf = f;
        } else {
            float ct = sqrt(1 - sin2);
            s.wi = -ratio * wo + (ratio * no - ct) * n;
            s.value = float3((1 - f) * ratio * ratio / ct);
            s.pdf = 1 - f;
            s.transmission = true;
            s.eta = eta_t / eta_i;
        }
        s.valid = s.pdf > 0;
        return s;
    }
    Evaluation e = evaluate_bsdf(m, wo, s.wi, n);
    s.value = e.value;
    s.pdf = e.pdf;
    s.valid = e.pdf > 0;
    return s;
}
bool is_delta(Material m) {
    return m.kind == MaterialDielectric || m.kind == MaterialMirror ||
           (m.kind == MaterialConductor && m.eta_roughness.w == 0);
}
