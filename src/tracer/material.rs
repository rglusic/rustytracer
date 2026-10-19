//! Linear RGB BSDFs. Continuous PDFs are per steradian; delta PDFs are probability masses.
use super::sampler::Sampler;
use cgmath::{ElementWise, InnerSpace, Vector3};
use serde::Deserialize;
use std::f64::consts::PI;

pub type Color = Vector3<f64>;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u32)]
pub enum Kind {
    #[serde(alias = "flat")]
    Diffuse = 0,
    Conductor = 1,
    #[serde(alias = "glass")]
    Dielectric = 2,
    #[serde(alias = "diffuse_light")]
    Emitter = 3,
    Mirror = 4,
}

#[derive(Clone, Copy, Debug)]
pub struct Material {
    pub kind: Kind,
    pub albedo: Color,
    pub emission: Color,
    pub eta: Color,
    pub k: Color,
    pub absorption: Color,
    pub roughness: f64,
    pub ior: f64,
    pub two_sided: bool,
}

impl Material {
    pub const fn diffuse(albedo: Color) -> Self {
        Self {
            kind: Kind::Diffuse,
            albedo,
            emission: Color::new(0., 0., 0.),
            eta: Color::new(1., 1., 1.),
            k: Color::new(0., 0., 0.),
            absorption: Color::new(0., 0., 0.),
            roughness: 0.,
            ior: 1.5,
            two_sided: false,
        }
    }
    /// Optical constants are absolute (vacuum-relative); the interface needs relative indices.
    pub fn in_medium(mut self, ior: f64) -> Self {
        if self.kind == Kind::Conductor {
            self.eta /= ior;
            self.k /= ior;
        }
        self
    }
    pub fn emitted(&self, normal: Color, outgoing: Color) -> Color {
        if self.two_sided || normal.dot(outgoing) > 0. {
            self.emission
        } else {
            black()
        }
    }
    pub fn is_delta(&self) -> bool {
        matches!(self.kind, Kind::Mirror | Kind::Dielectric)
            || (self.kind == Kind::Conductor && self.roughness == 0.)
    }
    pub fn evaluate(&self, wo: Color, wi: Color, n: Color) -> (Color, f64) {
        let no = n.dot(wo);
        let ni = n.dot(wi);
        if no <= 0. || ni <= 0. {
            return (black(), 0.);
        }
        match self.kind {
            Kind::Diffuse => (self.albedo / PI, ni / PI),
            Kind::Conductor if self.roughness > 0. => {
                let h = (wo + wi).normalize();
                let nh = n.dot(h).max(0.);
                let oh = wo.dot(h).abs();
                if oh == 0. {
                    return (black(), 0.);
                }
                let alpha = self.roughness * self.roughness;
                let d = ggx_d(nh, n.cross(h).magnitude2(), alpha);
                let g = 1. / (1. + ggx_lambda(no, alpha) + ggx_lambda(ni, alpha));
                (
                    conductor_fresnel(oh, self.eta, self.k) * (d * g / (4. * no * ni)),
                    d * nh / (4. * oh),
                )
            }
            _ => (black(), 0.),
        }
    }
    pub fn sample(
        &self,
        wo: Color,
        n: Color,
        eta_i: f64,
        eta_t: f64,
        rng: &mut Sampler,
    ) -> Option<BsdfSample> {
        let no = n.dot(wo);
        if no <= 0. {
            return None;
        }
        match self.kind {
            Kind::Emitter => None,
            Kind::Diffuse => {
                let wi = cosine_hemisphere(n, rng.next(), rng.next());
                let (value, pdf) = self.evaluate(wo, wi, n);
                Some(BsdfSample::continuous(wi, value, pdf))
            }
            Kind::Mirror => Some(BsdfSample::delta(
                reflect(wo, n),
                self.albedo / no,
                1.,
                false,
                1.,
            )),
            Kind::Conductor if self.roughness == 0. => Some(BsdfSample::delta(
                reflect(wo, n),
                conductor_fresnel(no, self.eta, self.k) / no,
                1.,
                false,
                1.,
            )),
            Kind::Conductor => {
                let u = rng.next();
                let alpha = self.roughness * self.roughness;
                let slope = alpha * (u / (1. - u)).sqrt();
                let phi = 2. * PI * rng.next();
                let h = local_to_world(
                    Color::new(slope * phi.cos(), slope * phi.sin(), 1.).normalize(),
                    n,
                );
                if wo.dot(h) <= 0. {
                    return None;
                }
                let wi = reflect(wo, h);
                if n.dot(wi) <= 0. {
                    return None;
                }
                let (value, pdf) = self.evaluate(wo, wi, n);
                Some(BsdfSample::continuous(wi, value, pdf))
            }
            Kind::Dielectric => {
                let ratio = eta_i / eta_t;
                let sin2_t = ratio * ratio * (1. - no * no).max(0.);
                let fresnel = dielectric_fresnel(no, eta_i, eta_t);
                if sin2_t >= 1. || rng.next() < fresnel {
                    Some(BsdfSample::delta(
                        reflect(wo, n),
                        white() * (fresnel / no),
                        fresnel,
                        false,
                        1.,
                    ))
                } else {
                    let ct = (1. - sin2_t).sqrt();
                    let wi = -ratio * wo + (ratio * no - ct) * n;
                    Some(BsdfSample::delta(
                        wi,
                        white() * ((1. - fresnel) * ratio * ratio / ct),
                        1. - fresnel,
                        true,
                        eta_t / eta_i,
                    ))
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct BsdfSample {
    pub wi: Color,
    pub value: Color,
    pub pdf: f64,
    pub delta: bool,
    pub transmission: bool,
    pub eta: f64,
}
impl BsdfSample {
    fn continuous(wi: Color, value: Color, pdf: f64) -> Self {
        Self {
            wi,
            value,
            pdf,
            delta: false,
            transmission: false,
            eta: 1.,
        }
    }
    fn delta(wi: Color, value: Color, pdf: f64, transmission: bool, eta: f64) -> Self {
        Self {
            wi,
            value,
            pdf,
            delta: true,
            transmission,
            eta,
        }
    }
    pub fn weight(&self, n: Color) -> Color {
        self.value * (n.dot(self.wi).abs() / self.pdf)
    }
}
pub fn black() -> Color {
    Color::new(0., 0., 0.)
}
pub fn white() -> Color {
    Color::new(1., 1., 1.)
}
pub fn maximum(v: Color) -> f64 {
    v.x.max(v.y).max(v.z)
}
pub fn reflect(wo: Color, n: Color) -> Color {
    2. * wo.dot(n) * n - wo
}
pub fn multiply(a: Color, b: Color) -> Color {
    a.mul_element_wise(b)
}
pub fn attenuate(sigma: Color, distance: f64) -> Color {
    if sigma == black() {
        return white();
    }
    Color::new(
        (-sigma.x * distance).exp(),
        (-sigma.y * distance).exp(),
        (-sigma.z * distance).exp(),
    )
}

pub fn dielectric_fresnel(cosine: f64, eta_i: f64, eta_t: f64) -> f64 {
    let c = cosine.clamp(0., 1.);
    let sin2 = (eta_i / eta_t).powi(2) * (1. - c * c).max(0.);
    if sin2 >= 1. {
        return 1.;
    }
    let t = (1. - sin2).sqrt();
    let rs = (eta_i * c - eta_t * t) / (eta_i * c + eta_t * t);
    let rp = (eta_t * c - eta_i * t) / (eta_t * c + eta_i * t);
    (rs * rs + rp * rp) * 0.5
}
pub fn conductor_fresnel(cosine: f64, eta: Color, k: Color) -> Color {
    let c = cosine.clamp(0., 1.);
    let s2 = 1. - c * c;
    let channel = |eta: f64, k: f64| {
        let t0 = eta * eta - k * k - s2;
        let a2b2 = (t0 * t0 + 4. * eta * eta * k * k).sqrt();
        let a = ((a2b2 + t0) * 0.5).max(0.).sqrt();
        let t1 = a2b2 + c * c;
        let t2 = 2. * c * a;
        let rs = (t1 - t2) / (t1 + t2);
        let t3 = c * c * a2b2 + s2 * s2;
        let t4 = t2 * s2;
        0.5 * rs * (1. + (t3 - t4) / (t3 + t4))
    };
    Color::new(
        channel(eta.x, k.x),
        channel(eta.y, k.y),
        channel(eta.z, k.z),
    )
}
fn ggx_d(c: f64, sin2: f64, a: f64) -> f64 {
    let d = sin2 + c * c * a * a;
    let ratio = a / d;
    ratio * ratio / PI
}
fn ggx_lambda(c: f64, a: f64) -> f64 {
    0.5 * ((1. + a * a * (1. - c * c).max(0.) / (c * c)).sqrt() - 1.)
}
pub fn local_to_world(v: Color, n: Color) -> Color {
    let helper = if n.z.abs() < 0.999 {
        Color::unit_z()
    } else {
        Color::unit_y()
    };
    let tangent = helper.cross(n).normalize();
    v.x * tangent + v.y * n.cross(tangent) + v.z * n
}
pub fn cosine_hemisphere(n: Color, u: f64, v: f64) -> Color {
    let r = u.sqrt();
    let phi = 2. * PI * v;
    local_to_world(Color::new(r * phi.cos(), r * phi.sin(), (1. - u).sqrt()), n)
}
pub fn uniform_sphere(u: f64, v: f64) -> Color {
    let z = 1. - 2. * u;
    let r = (1. - z * z).max(0.).sqrt();
    let phi = 2. * PI * v;
    Color::new(r * phi.cos(), r * phi.sin(), z)
}
#[cfg(test)]
#[path = "material_tests.rs"]
mod tests;
