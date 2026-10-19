// Deterministic random streams and local/world direction sampling.
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
float3 uniform_sphere(float u, float v) {
    float z = 1 - 2 * u, r = sqrt(max(0.0f, 1 - z * z)), phi = 2 * PI * v;
    return float3(r * cos(phi), r * sin(phi), z);
}
float3 local_to_world(float3 v, float3 n) {
    float3 helper = abs(n.z) < 0.999f ? float3(0, 0, 1) : float3(0, 1, 0);
    float3 t = normalize(cross(helper, n));
    return v.x * t + v.y * cross(n, t) + v.z * n;
}
float3 primary_direction(uint pixel, constant Parameters &p, thread uint &state) {
    float u = (float(pixel % p.width) + random_value(state)) / float(p.width);
    float v = (float(pixel / p.width) + random_value(state)) / float(p.height);
    return normalize(p.camera[1].xyz + u * p.camera[2].xyz + v * p.camera[3].xyz - p.camera[0].xyz);
}
