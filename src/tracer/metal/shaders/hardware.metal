#include <metal_raytracing>
using namespace raytracing;
struct SphereIntersection { bool accept [[accept_intersection]]; float distance [[distance]]; };
[[intersection(bounding_box)]]
SphereIntersection intersect_sphere(float3 origin [[origin]],float3 direction [[direction]],float minimum [[min_distance]],
                                    float maximum [[max_distance]],uint primitive [[primitive_id]],device const Sphere *spheres [[buffer(0)]]) {
    float t=sphere_distance(origin,direction,spheres[primitive].center_radius,maximum);
    return {t>=minimum && t>0,t};
}
Hit intersect_hardware(float3 origin,float3 direction,constant Parameters &p,device const Sphere *spheres,device const Plane *planes,
                       primitive_acceleration_structure acceleration,intersection_function_table<> functions,float maximum=MAXFLOAT) {
    Hit hit=intersect_planes(origin,direction,p,planes,maximum);
    ray r; r.origin=origin; r.direction=direction; r.min_distance=0; r.max_distance=hit.distance;
    intersector<> tester; tester.assume_geometry_type(geometry_type::bounding_box);
    auto result=tester.intersect(r,acceleration,functions);
    if(result.type!=intersection_type::none && result.distance<hit.distance) {
        Sphere sphere=spheres[result.primitive_id];
        hit={result.distance,normalize(origin+result.distance*direction-sphere.center_radius.xyz),result.primitive_id,true,true};
    }
    return hit;
}
struct HardwareQuery {
    constant Parameters *p; device const Sphere *spheres; device const Plane *planes;
    primitive_acceleration_structure acceleration; intersection_function_table<> functions;
    Hit intersect(float3 origin,float3 direction,float maximum=MAXFLOAT) const {
        return intersect_hardware(origin,direction,*p,spheres,planes,acceleration,functions,maximum);
    }
    bool occluded(float3 origin,float3 direction,float maximum=MAXFLOAT) const {
        if (maximum<=0) return false;
        if (occluded_planes(origin,direction,*p,planes,maximum)) return true;
        ray r; r.origin=origin; r.direction=direction; r.min_distance=0; r.max_distance=maximum;
        intersector<> tester; tester.assume_geometry_type(geometry_type::bounding_box);
        tester.accept_any_intersection(true);
        return tester.intersect(r,acceleration,functions).type!=intersection_type::none;
    }
};
kernel void trace_hardware(constant Parameters &p [[buffer(0)]],device const Sphere *spheres [[buffer(1)]],
                           device const Plane *planes [[buffer(2)]],device float4 *accumulation [[buffer(3)]],
                           primitive_acceleration_structure acceleration [[buffer(4)]],intersection_function_table<> functions [[buffer(5)]],
                           device PathState *paths [[buffer(6)]],device atomic_uint *unfinished [[buffer(7)]],
                           device const uint4 *lights [[buffer(8)]],uint pixel [[thread_position_in_grid]]) {
    HardwareQuery query={&p,spheres,planes,acceleration,functions};
    trace_paths(pixel,query,p,spheres,planes,accumulation,paths,unfinished,lights);
}
