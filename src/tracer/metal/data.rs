//! Host-side POD layouts matching shaders/common.metal exactly.
use bytemuck::{Pod, Zeroable};

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Material {
    pub albedo: [f32; 4],
    pub emission: [f32; 4],
    pub eta_roughness: [f32; 4],
    pub k_ior: [f32; 4],
    pub absorption: [f32; 4],
    pub kind: u32,
    pub two_sided: u32,
    pub parent_medium: u32,
    pub reserved: u32,
}
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Sphere {
    pub center_radius: [f32; 4],
    pub material: Material,
}
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Plane {
    pub origin: [f32; 4],
    pub normal: [f32; 4],
    pub material: Material,
}
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Parameters {
    pub camera: [[f32; 4]; 4],
    pub width: u32,
    pub height: u32,
    pub sample_start: u32,
    pub sample_count: u32,
    pub sphere_count: u32,
    pub plane_count: u32,
    pub seed: u32,
    pub max_bounces: u32,
    pub environment: [f32; 4],
    pub reset: u32,
    pub light_count: u32,
    pub light_sampling: u32,
    pub camera_medium: u32,
}
/// Persistent per-pixel continuation, exactly matching the Metal PathState.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct PathState {
    pub origin: [f32; 4],
    pub direction: [f32; 4],
    pub beta: [f32; 4],
    pub radiance: [f32; 4],
    pub previous_point: [f32; 4],
    pub rng: u32,
    pub depth: u32,
    pub completed_samples: u32,
    pub previous_delta: u32,
    pub medium_id: u32,
    pub reserved_medium: [u32; 3],
    pub eta_scale: f32,
    pub previous_pdf: f32,
    pub reserved_weights: [f32; 2],
}
