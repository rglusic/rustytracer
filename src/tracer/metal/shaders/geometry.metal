// Analytic intersections, conservative surface offsets, and visibility queries.
float sphere_distance(float3 origin, float3 direction, float4 sphere, float maximum) {
    float3 oc = origin - sphere.xyz;
    float a = dot(direction, direction), b = dot(oc, direction);
    if (a == 0)
        return 0;
    float3 perpendicular = oc - (b / a) * direction;
    float discriminant = a * (sphere.w * sphere.w - dot(perpendicular, perpendicular));
    if (discriminant < 0)
        return 0;
    float root = sqrt(discriminant), q = -b - copysign(root, b);
    float length = metal::length(oc);
    float c = (length - sphere.w) * (length + sphere.w);
    float near = q == 0 ? -b / a : q / a, far = q == 0 ? -b / a : c / q;
    if (near > far) {
        float swap = near;
        near = far;
        far = swap;
    }
    if (near > 0 && near < maximum)
        return near;
    return far > 0 && far < maximum ? far : 0;
}
struct Hit {
    float distance;
    float3 normal;
    uint index;
    bool sphere, found;
};
struct Surface {
    float3 point, normal, error;
};
float gamma_error() {
    float e = 16.0f * 0.5f * FLT_EPSILON;
    return e / (1 - e);
}
Surface hit_surface(float3 origin, float3 direction, Hit hit, device const Sphere *spheres,
                    device const Plane *planes) {
    float3 point = origin + hit.distance * direction, scale;
    if (hit.sphere) {
        Sphere sphere = spheres[hit.index];
        point = sphere.center_radius.xyz +
                sphere.center_radius.w * normalize(point - sphere.center_radius.xyz);
        scale = abs(sphere.center_radius.xyz) + sphere.center_radius.w;
    } else
        scale = abs(planes[hit.index].origin.xyz);
    return {point, hit.normal,
            (abs(origin) + abs(hit.distance * direction) + abs(point) + scale) * gamma_error()};
}
float3 offset_origin(Surface s, float3 direction) {
    float d = dot(abs(s.normal), s.error);
    float3 offset = s.normal * (dot(s.normal, direction) >= 0 ? d : -d);
    float3 p = s.point + offset;
    for (uint i = 0; i < 3; ++i) {
        if (offset[i] > 0)
            p[i] = nextafter(p[i], INFINITY);
        else if (offset[i] < 0)
            p[i] = nextafter(p[i], -INFINITY);
    }
    return p;
}
Hit intersect_planes(float3 origin, float3 direction, constant Parameters &p,
                     device const Plane *planes, float maximum = MAXFLOAT) {
    Hit hit;
    hit.distance = maximum;
    hit.found = false;
    for (uint i = 0; i < p.plane_count; ++i) {
        float denominator = dot(direction, planes[i].normal.xyz);
        if (denominator == 0)
            continue;
        float t = dot(planes[i].origin.xyz - origin, planes[i].normal.xyz) / denominator;
        if (t > 0 && t < hit.distance)
            hit = {t, planes[i].normal.xyz, i, false, true};
    }
    return hit;
}
Hit intersect_linear(float3 origin, float3 direction, constant Parameters &p,
                     device const Sphere *spheres, device const Plane *planes,
                     float maximum = MAXFLOAT) {
    Hit hit = intersect_planes(origin, direction, p, planes, maximum);
    for (uint i = 0; i < p.sphere_count; ++i) {
        float t = sphere_distance(origin, direction, spheres[i].center_radius, hit.distance);
        if (t > 0)
            hit = {t, normalize(origin + t * direction - spheres[i].center_radius.xyz), i, true,
                   true};
    }
    return hit;
}
bool occluded_planes(float3 origin, float3 direction, constant Parameters &p,
                     device const Plane *planes, float maximum) {
    for (uint i = 0; i < p.plane_count; ++i) {
        float denominator = dot(direction, planes[i].normal.xyz);
        if (denominator == 0)
            continue;
        float t = dot(planes[i].origin.xyz - origin, planes[i].normal.xyz) / denominator;
        if (t > 0 && t < maximum)
            return true;
    }
    return false;
}
bool occluded_linear(float3 origin, float3 direction, constant Parameters &p,
                     device const Sphere *spheres, device const Plane *planes, float maximum) {
    if (occluded_planes(origin, direction, p, planes, maximum))
        return true;
    for (uint i = 0; i < p.sphere_count; ++i) {
        if (sphere_distance(origin, direction, spheres[i].center_radius, maximum) > 0)
            return true;
    }
    return false;
}
