struct LinearQuery {
    constant Parameters *p; device const Sphere *spheres; device const Plane *planes;
    Hit intersect(float3 origin,float3 direction,float maximum=MAXFLOAT) const {
        return intersect_linear(origin,direction,*p,spheres,planes,maximum);
    }
    bool occluded(float3 origin,float3 direction,float maximum=MAXFLOAT) const {
        return occluded_linear(origin,direction,*p,spheres,planes,maximum);
    }
};
kernel void trace_linear(constant Parameters &p [[buffer(0)]],device const Sphere *spheres [[buffer(1)]],
                         device const Plane *planes [[buffer(2)]],device float4 *accumulation [[buffer(3)]],
                         device PathState *paths [[buffer(6)]],device atomic_uint *unfinished [[buffer(7)]],
                         device const uint4 *lights [[buffer(8)]],uint pixel [[thread_position_in_grid]]) {
    LinearQuery query={&p,spheres,planes};
    trace_paths(pixel,query,p,spheres,planes,accumulation,paths,unfinished,lights);
}
