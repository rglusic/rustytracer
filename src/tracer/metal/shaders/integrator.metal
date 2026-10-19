// Query supplies the same intersection semantics for direct and accelerated traversal.
float environment_pdf(constant Parameters &p) { return any(p.environment.xyz>0) ? 1/(4*PI*p.light_count) : 0; }
float endpoint_pdf(Hit h,Surface s,float3 from,constant Parameters &p,device const Sphere *spheres,device const Plane *planes) {
    if(!any(hit_material(h,spheres,planes).emission.xyz>0)) return 0;
    if(!h.sphere) return 1/(4*PI*p.light_count);
    float3 delta=s.point-from;
    float cosine=abs(dot(s.normal,-normalize(delta))),r=spheres[h.index].center_radius.w;
    return cosine==0 ? 0 : dot(delta,delta)/(4*PI*r*r*cosine*p.light_count);
}
template<typename Query>
float3 direct_light(Query query,Surface s,float3 wo,float3 n,Material m,float3 absorption,constant Parameters &p,
                    device const Sphere *spheres,device const Plane *planes,device const uint4 *lights,thread uint &rng) {
    if(p.light_count==0) return 0;
    uint selected=min(uint(random_value(rng)*p.light_count),p.light_count-1);
    uint4 light=lights[selected];
    float u=random_value(rng),v=random_value(rng);
    float3 wi=uniform_sphere(u,v),le;
    float distance=INFINITY,pdf=1/(4*PI*p.light_count);
    if(light.x==2) {
        if(query.intersect(offset_origin(s,wi),wi).found) return 0;
        le=p.environment.xyz;
    } else if(light.x==1) {
        Hit h=query.intersect(offset_origin(s,wi),wi);
        if(!h.found || h.sphere || h.index!=light.y) return 0;
        le=emitted(planes[light.y].material,h.normal,-wi); distance=h.distance;
    } else {
        Sphere sphere=spheres[light.y];
        float3 normal=wi,point=sphere.center_radius.xyz+sphere.center_radius.w*normal;
        float3 delta=point-s.point; distance=length(delta);
        if(distance==0) return 0;
        wi=delta/distance;
        float cosine=abs(dot(normal,-wi));
        if(cosine==0) return 0;
        pdf*=distance*distance/(sphere.center_radius.w*sphere.center_radius.w*cosine);
        Surface target={point,normal,(abs(sphere.center_radius.xyz)+abs(point)+sphere.center_radius.w)*gamma_error()};
        float3 from=offset_origin(s,wi),to=offset_origin(target,-wi),shadow=to-from;
        if(query.intersect(from,normalize(shadow),length(shadow)).found) return 0;
        le=emitted(sphere.material,normal,-wi);
    }
    Evaluation e=evaluate_bsdf(m,wo,wi,n);
    if(e.pdf==0) return 0;
    float3 transmittance=isfinite(distance) ? exp(-absorption*distance) : select(float3(0),float3(1),absorption==0);
    return e.value*le*transmittance*(abs(dot(n,wi))*power_heuristic(pdf,e.pdf)/pdf);
}
void initialize_path(thread PathState &s,uint pixel,uint sample,constant Parameters &p) {
    uint rng=sample_seed(p.seed,pixel,sample);
    s.origin=float4(p.camera[0].xyz,0); s.direction=float4(primary_direction(pixel,p,rng),0);
    s.beta=float4(1,1,1,0); s.radiance=0; s.previous_point=s.origin;
    s.counters=uint4(rng,0,sample,1); s.medium=uint4(p.camera_medium,0,0,0); s.weights=float4(1,0,0,0);
}
template<typename Query>
bool advance_path(thread PathState &s,Query query,constant Parameters &p,device const Sphere *spheres,
                  device const Plane *planes,device const uint4 *lights) {
    float eta_i=s.medium.x ? spheres[s.medium.x-1].material.k_ior.w : 1;
    float3 absorption=s.medium.x ? spheres[s.medium.x-1].material.absorption.xyz : float3(0);
    Hit hit=query.intersect(s.origin.xyz,s.direction.xyz);
    if(!hit.found) {
        float weight=s.counters.w || !p.light_sampling ? 1 : power_heuristic(s.weights.y,environment_pdf(p));
        s.radiance.xyz+=s.beta.xyz*p.environment.xyz*weight; return true;
    }
    s.beta.xyz*=exp(-absorption*hit.distance);
    Surface surface=hit_surface(s.origin.xyz,s.direction.xyz,hit,spheres,planes);
    Material m=hit_material(hit,spheres,planes);
    float3 wo=-s.direction.xyz;
    bool front=dot(surface.normal,wo)>0;
    float3 n=front ? surface.normal : -surface.normal;
    float weight=s.counters.w || !p.light_sampling ? 1 : power_heuristic(s.weights.y,endpoint_pdf(hit,surface,s.previous_point.xyz,p,spheres,planes));
    s.radiance.xyz+=s.beta.xyz*emitted(m,surface.normal,wo)*weight;
    if(m.flags.x==3 || (p.max_bounces && s.counters.y>=p.max_bounces)) return true;
    uint rng=s.counters.x;
    if(p.light_sampling && !is_delta(m)) s.radiance.xyz+=s.beta.xyz*direct_light(query,surface,wo,n,m,absorption,p,spheres,planes,lights,rng);
    uint target_medium=m.flags.x==2 ? (front ? hit.index+1 : m.flags.z) : 0;
    float eta_t=target_medium ? spheres[target_medium-1].material.k_ior.w : 1;
    if(m.flags.x!=2) eta_t=m.k_ior.w;
    BsdfSample sample=sample_bsdf(m,wo,n,eta_i,eta_t,rng);
    if(!sample.valid) return true;
    s.beta.xyz*=sample.value*(abs(dot(n,sample.wi))/sample.pdf);
    if(max(max(s.beta.x,s.beta.y),s.beta.z)<=0) return true;
    if(sample.transmission) { s.weights.x*=sample.eta*sample.eta; s.medium.x=target_medium; }
    s.counters.y++;
    if(s.counters.y>=4) {
        float survival=min(max(max(s.beta.x,s.beta.y),s.beta.z)*s.weights.x,0.95f);
        if(random_value(rng)>=survival) return true;
        s.beta.xyz/=survival;
    }
    s.previous_point=float4(surface.point,0); s.weights.y=sample.pdf; s.counters.w=sample.delta; s.counters.x=rng;
    s.origin=float4(offset_origin(surface,sample.wi),0); s.direction=float4(normalize(sample.wi),0);
    return false;
}
template<typename Query>
void trace_paths(uint pixel,Query query,constant Parameters &p,device const Sphere *spheres,device const Plane *planes,
                 device float4 *accumulation,device PathState *paths,device atomic_uint *unfinished,device const uint4 *lights) {
    if(pixel>=p.width*p.height) return;
    PathState s;
    float3 sum;
    if(p.reset) { initialize_path(s,pixel,0,p); sum=0; } else { s=paths[pixel]; sum=accumulation[pixel].xyz; }
    uint target=p.sample_start+p.sample_count;
    // Bound GPU work without truncating a path. Its full state survives the dispatch.
    for(uint step=0;step<min(p.sample_count*16,256u) && s.counters.z<target;++step) {
        if(advance_path(s,query,p,spheres,planes,lights)) {
            sum+=s.radiance.xyz;
            initialize_path(s,pixel,s.counters.z+1,p);
        }
    }
    paths[pixel]=s; accumulation[pixel]=float4(sum,0);
    if(s.counters.z<target) atomic_fetch_add_explicit(unfinished,1u,memory_order_relaxed);
}
