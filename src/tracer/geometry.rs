use super::material::{Color, Material};
use super::ray::Ray;
use cgmath::{InnerSpace, Vector3};

#[derive(Clone, Copy, Debug)]
pub enum Shape {
    Plane { origin: Color },
    Sphere { center: Color, radius: f64 },
}

pub trait Hitable: Send + Sync {
    fn hit(&self, r: &Ray, t_max: f64) -> f64;
    fn material(&self) -> &Material;
    fn get_norm_at_p(&self, p: &Color) -> Color;
    fn shape(&self) -> Shape;
    fn surface(&self, r: &Ray, t: f64) -> Surface {
        let point = r.point_at_parameter(t);
        let (point, scale) = match self.shape() {
            Shape::Sphere { center, radius } => {
                // Reproject the computed intersection onto the analytic surface.
                (
                    center + radius * (point - center).normalize(),
                    abs(center) + Color::new(radius, radius, radius),
                )
            }
            Shape::Plane { origin, .. } => (point, abs(origin)),
        };
        let normal = self.get_norm_at_p(&point);
        let error = (abs(*r.origin()) + abs(t * r.direction()) + abs(point) + scale) * gamma(16);
        Surface {
            point,
            normal,
            error,
        }
    }
}
pub struct Plane {
    pub origin: Color,
    pub normal: Color,
    pub material: Material,
}
impl Hitable for Plane {
    fn hit(&self, r: &Ray, maximum: f64) -> f64 {
        let denominator = r.direction().dot(self.normal);
        if denominator == 0. {
            return 0.;
        }
        let t = (self.origin - r.origin()).dot(self.normal) / denominator;
        if t > 0. && t < maximum { t } else { 0. }
    }
    fn material(&self) -> &Material {
        &self.material
    }
    fn get_norm_at_p(&self, _: &Color) -> Color {
        self.normal
    }
    fn shape(&self) -> Shape {
        Shape::Plane {
            origin: self.origin,
        }
    }
}
pub struct Sphere {
    pub center: Color,
    pub radius: f64,
    pub material: Material,
}
impl Hitable for Sphere {
    fn hit(&self, r: &Ray, maximum: f64) -> f64 {
        sphere_distance(
            *r.origin(),
            *r.direction(),
            self.center,
            self.radius,
            maximum,
        )
    }
    fn material(&self) -> &Material {
        &self.material
    }
    fn get_norm_at_p(&self, p: &Color) -> Color {
        (*p - self.center).normalize()
    }
    fn shape(&self) -> Shape {
        Shape::Sphere {
            center: self.center,
            radius: self.radius,
        }
    }
}
pub fn sphere_distance(
    origin: Color,
    direction: Color,
    center: Color,
    radius: f64,
    maximum: f64,
) -> f64 {
    let oc = origin - center;
    let a = direction.magnitude2();
    if a == 0. {
        return 0.;
    }
    let b = oc.dot(direction);
    // Closest approach avoids subtracting two large, nearly equal squared distances.
    let perpendicular = oc - (b / a) * direction;
    let discriminant = a * (radius * radius - perpendicular.magnitude2());
    if discriminant < 0. {
        return 0.;
    }
    let root = discriminant.sqrt();
    let c = (oc.magnitude() - radius) * (oc.magnitude() + radius);
    let q = -b - root.copysign(b);
    let (mut near, mut far) = if q == 0. {
        (-b / a, -b / a)
    } else {
        (q / a, c / q)
    };
    if near > far {
        std::mem::swap(&mut near, &mut far);
    }
    if near > 0. && near < maximum {
        near
    } else if far > 0. && far < maximum {
        far
    } else {
        0.
    }
}
#[derive(Clone, Copy)]
pub struct Surface {
    pub point: Color,
    pub normal: Color,
    pub error: Color,
}
impl Surface {
    pub fn offset(self, direction: Color) -> Color {
        let distance = abs(self.normal).dot(self.error);
        let offset = self.normal
            * if self.normal.dot(direction) >= 0. {
                distance
            } else {
                -distance
            };
        let p = self.point + offset;
        Vector3::new(
            round_away(p.x, offset.x),
            round_away(p.y, offset.y),
            round_away(p.z, offset.z),
        )
    }
}
fn round_away(v: f64, offset: f64) -> f64 {
    if offset > 0. {
        v.next_up()
    } else if offset < 0. {
        v.next_down()
    } else {
        v
    }
}
pub fn abs(v: Color) -> Color {
    Color::new(v.x.abs(), v.y.abs(), v.z.abs())
}
pub fn gamma(n: u32) -> f64 {
    let e = f64::EPSILON * 0.5 * f64::from(n);
    e / (1. - e)
}
