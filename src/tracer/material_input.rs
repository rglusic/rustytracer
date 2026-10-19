//! Scene-facing material schema and explicit migration of the historical string names.
use super::Result;
use super::material::{Color, Kind, Material, black, white};
use serde::Deserialize;
use std::io;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum MaterialInput {
    Name(String),
    Parameters(Box<Parameters>),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    pub kind: Kind,
    pub albedo: Option<[f64; 3]>,
    pub emission: Option<[f64; 3]>,
    pub roughness: Option<f64>,
    pub ior: Option<f64>,
    pub eta: Option<[f64; 3]>,
    pub k: Option<[f64; 3]>,
    pub absorption: Option<[f64; 3]>,
    #[serde(default)]
    pub two_sided: bool,
}

impl MaterialInput {
    pub fn resolve(&self, color: [f64; 3]) -> Result<Material> {
        let legacy = checked(color, "Object color", false)?;
        let mut m = Material::diffuse(legacy);
        match self {
            Self::Name(name) => {
                m.kind = match name.as_str() {
                    "flat" => Kind::Diffuse,
                    // A legacy tinted perfect reflector is an ideal mirror, not a measured metal.
                    "metal" => Kind::Mirror,
                    "glass" => Kind::Dielectric,
                    "diffuse_light" => Kind::Emitter,
                    _ => return Err(invalid(format!("Unknown material '{name}'"))),
                };
                if m.kind == Kind::Emitter {
                    m.emission = legacy;
                    m.albedo = black();
                }
                if m.kind == Kind::Dielectric && legacy != white() {
                    return Err(invalid(
                        "Glass color must be white; specify absorption per metre for tinted glass",
                    ));
                }
            }
            Self::Parameters(p) => {
                m.kind = p.kind;
                if p.albedo.is_some() && !matches!(p.kind, Kind::Diffuse | Kind::Mirror) {
                    return Err(invalid(
                        "Albedo is only defined for diffuse and ideal mirror materials",
                    ));
                }
                if matches!(p.kind, Kind::Dielectric | Kind::Conductor) && legacy != white() {
                    return Err(invalid(
                        "Use absorption for glass or eta/k for conductors instead of object color",
                    ));
                }
                m.albedo = checked(
                    p.albedo
                        .unwrap_or(if matches!(p.kind, Kind::Diffuse | Kind::Mirror) {
                            color
                        } else {
                            [0.; 3]
                        }),
                    "Albedo",
                    true,
                )?;
                m.emission = checked(p.emission.unwrap_or([0.; 3]), "Emission", false)?;
                m.absorption = checked(p.absorption.unwrap_or([0.; 3]), "Absorption", false)?;
                m.ior = p.ior.unwrap_or(1.5);
                m.roughness = p.roughness.unwrap_or(0.);
                m.two_sided = p.two_sided;
                if m.kind == Kind::Emitter {
                    if p.emission.is_none() {
                        return Err(invalid("Emitter requires explicit emission radiance"));
                    }
                    m.albedo = black();
                }
                if m.kind == Kind::Conductor {
                    m.eta = checked(
                        p.eta.ok_or_else(|| {
                            invalid("Conductor requires eta and k optical constants")
                        })?,
                        "eta",
                        false,
                    )?;
                    m.k = checked(
                        p.k.ok_or_else(|| {
                            invalid("Conductor requires eta and k optical constants")
                        })?,
                        "k",
                        false,
                    )?;
                    if p.albedo.is_some() {
                        return Err(invalid("Conductor reflectance uses eta and k, not albedo"));
                    }
                    if [m.eta.x, m.eta.y, m.eta.z, m.k.x, m.k.y, m.k.z]
                        .iter()
                        .any(|v| *v <= 0. || !(16. * v.powi(4)).is_normal())
                    {
                        return Err(invalid("Conductor eta and k must be positive"));
                    }
                } else if p.eta.is_some() || p.k.is_some() {
                    return Err(invalid("eta and k require a conductor"));
                }
                if m.kind != Kind::Dielectric && (p.ior.is_some() || p.absorption.is_some()) {
                    return Err(invalid("IOR and absorption require a dielectric"));
                }
                if m.kind == Kind::Dielectric && p.albedo.is_some() {
                    return Err(invalid(
                        "Dielectric tint is defined by absorption, not albedo",
                    ));
                }
                if m.kind != Kind::Conductor && p.roughness.is_some() {
                    return Err(invalid("Roughness currently requires a conductor"));
                }
            }
        }
        if matches!(m.kind, Kind::Diffuse | Kind::Mirror) {
            checked(m.albedo.into(), "Reflectance", true)?;
        }
        if !m.ior.is_finite()
            || m.ior < 1.
            || !m.ior.powi(2).is_finite()
            || !m.ior.recip().powi(2).is_normal()
            || !m.roughness.is_finite()
            || !(0. ..=1.).contains(&m.roughness)
        {
            return Err(invalid(
                "IOR must be finite and at least one; roughness must be in [0,1]",
            ));
        }
        if m.roughness > 0. && !(m.roughness.powi(4)).is_normal() {
            return Err(invalid("Roughness is below representable precision"));
        }
        Ok(m)
    }
}
fn checked(v: [f64; 3], name: &str, reflectance: bool) -> Result<Color> {
    if v.iter()
        .any(|v| !v.is_finite() || *v < 0. || (reflectance && *v > 1.))
    {
        return Err(invalid(format!(
            "{name} must be finite, nonnegative{}",
            if reflectance { " and at most one" } else { "" }
        )));
    }
    Ok(v.into())
}
fn invalid(message: impl Into<String>) -> Box<dyn std::error::Error + Send + Sync> {
    io::Error::new(io::ErrorKind::InvalidData, message.into()).into()
}
