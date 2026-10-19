#include <metal_stdlib>
using namespace metal;
constant float PI = 3.14159265358979323846f;
struct Material { float4 albedo, emission, eta_roughness, k_ior, absorption; uint4 flags; };
struct Sphere { float4 center_radius; Material material; };
struct Plane { float4 origin, normal; Material material; };
struct Parameters {
    float4 camera[4];
    uint width, height, sample_start, sample_count;
    uint sphere_count, plane_count, seed, max_bounces;
    float4 environment;
    uint reset, light_count, light_sampling, camera_medium;
};
struct PathState {
    float4 origin, direction, beta, radiance, previous_point;
    uint4 counters, medium;
    float4 weights;
};
static_assert(sizeof(Material)==96, "Material layout");
static_assert(sizeof(Sphere)==112, "Sphere layout");
static_assert(sizeof(Plane)==128, "Plane layout");
static_assert(sizeof(Parameters)==128, "Parameters layout");
static_assert(sizeof(PathState)==128, "PathState layout");
uint mix_bits(uint x) { x=(x^(x>>16))*0x7feb352du; x=(x^(x>>15))*0x846ca68bu; return x^(x>>16); }
uint sample_seed(uint seed,uint pixel,uint sample) { return mix_bits(seed^mix_bits(pixel)^mix_bits(sample+0x9e3779b9u)); }
float random_value(thread uint &state) {
    state=state*747796405u+2891336453u;
    uint word=((state>>((state>>28u)+4u))^state)*277803737u;
    return float(((word>>22u)^word)>>8u)*(1.0f/16777216.0f);
}
float3 uniform_sphere(float u,float v) {
    float z=1-2*u, r=sqrt(max(0.0f,1-z*z)), phi=2*PI*v;
    return float3(r*cos(phi),r*sin(phi),z);
}
float3 local_to_world(float3 v,float3 n) {
    float3 helper=abs(n.z)<0.999f ? float3(0,0,1) : float3(0,1,0);
    float3 t=normalize(cross(helper,n));
    return v.x*t+v.y*cross(n,t)+v.z*n;
}
float3 primary_direction(uint pixel,constant Parameters &p,thread uint &state) {
    float u=(float(pixel%p.width)+random_value(state))/float(p.width);
    float v=(float(pixel/p.width)+random_value(state))/float(p.height);
    return normalize(p.camera[1].xyz+u*p.camera[2].xyz+v*p.camera[3].xyz-p.camera[0].xyz);
}
float sphere_distance(float3 origin,float3 direction,float4 sphere,float maximum) {
    float3 oc=origin-sphere.xyz;
    float a=dot(direction,direction), b=dot(oc,direction);
    if (a==0) return 0;
    float3 perpendicular=oc-(b/a)*direction;
    float discriminant=a*(sphere.w*sphere.w-dot(perpendicular,perpendicular));
    if (discriminant<0) return 0;
    float root=sqrt(discriminant), q=-b-copysign(root,b);
    float length=metal::length(oc);
    float c=(length-sphere.w)*(length+sphere.w);
    float near=q==0 ? -b/a : q/a, far=q==0 ? -b/a : c/q;
    if (near>far) { float swap=near; near=far; far=swap; }
    if (near>0 && near<maximum) return near;
    return far>0 && far<maximum ? far : 0;
}
struct Hit { float distance; float3 normal; uint index; bool sphere, found; };
struct Surface { float3 point,normal,error; };
float gamma_error() { float e=16.0f*0.5f*FLT_EPSILON; return e/(1-e); }
Surface hit_surface(float3 origin,float3 direction,Hit hit,device const Sphere *spheres,device const Plane *planes) {
    float3 point=origin+hit.distance*direction, scale;
    if (hit.sphere) {
        Sphere sphere=spheres[hit.index];
        point=sphere.center_radius.xyz+sphere.center_radius.w*normalize(point-sphere.center_radius.xyz);
        scale=abs(sphere.center_radius.xyz)+sphere.center_radius.w;
    } else scale=abs(planes[hit.index].origin.xyz);
    return {point,hit.normal,(abs(origin)+abs(hit.distance*direction)+abs(point)+scale)*gamma_error()};
}
float3 offset_origin(Surface s,float3 direction) {
    float d=dot(abs(s.normal),s.error);
    float3 offset=s.normal*(dot(s.normal,direction)>=0 ? d : -d);
    float3 p=s.point+offset;
    for (uint i=0;i<3;++i) {
        if (offset[i]>0) p[i]=nextafter(p[i],INFINITY);
        else if (offset[i]<0) p[i]=nextafter(p[i],-INFINITY);
    }
    return p;
}
Hit intersect_planes(float3 origin,float3 direction,constant Parameters &p,device const Plane *planes,float maximum=MAXFLOAT) {
    Hit hit; hit.distance=maximum; hit.found=false;
    for (uint i=0;i<p.plane_count;++i) {
        float denominator=dot(direction,planes[i].normal.xyz);
        if (denominator==0) continue;
        float t=dot(planes[i].origin.xyz-origin,planes[i].normal.xyz)/denominator;
        if (t>0 && t<hit.distance) hit={t,planes[i].normal.xyz,i,false,true};
    }
    return hit;
}
Hit intersect_linear(float3 origin,float3 direction,constant Parameters &p,device const Sphere *spheres,device const Plane *planes,float maximum=MAXFLOAT) {
    Hit hit=intersect_planes(origin,direction,p,planes,maximum);
    for (uint i=0;i<p.sphere_count;++i) {
        float t=sphere_distance(origin,direction,spheres[i].center_radius,hit.distance);
        if (t>0) hit={t,normalize(origin+t*direction-spheres[i].center_radius.xyz),i,true,true};
    }
    return hit;
}
Material hit_material(Hit hit,device const Sphere *spheres,device const Plane *planes) { return hit.sphere ? spheres[hit.index].material : planes[hit.index].material; }
float3 emitted(Material m,float3 n,float3 outgoing) { return m.flags.y || dot(n,outgoing)>0 ? m.emission.xyz : float3(0); }
float power_heuristic(float a,float b) { if(a==0) return 0; float r=b/a; return 1/(1+r*r); }
