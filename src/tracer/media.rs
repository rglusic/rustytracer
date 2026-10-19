//! Closed, disjoint or strictly nested dielectric spheres, prepared once per scene.
use super::geometry::{Hitable, Shape};
use super::material::{Color, Kind, black};
use cgmath::InnerSpace;
use std::sync::Arc;

pub(super) type MediumId = Option<usize>;

struct Boundary {
    object: usize,
    center: Color,
    radius: f64,
    ior: f64,
    absorption: Color,
    parent: MediumId,
}

pub(super) struct MediumHierarchy {
    boundaries: Vec<Boundary>,
    object_boundaries: Vec<MediumId>,
}

impl MediumHierarchy {
    pub fn new(objects: &[Arc<dyn Hitable>]) -> Self {
        let mut boundaries = Vec::new();
        let mut object_boundaries = vec![None; objects.len()];
        for (object, geometry) in objects.iter().enumerate() {
            let material = geometry.material();
            if material.kind == Kind::Dielectric
                && let Shape::Sphere { center, radius } = geometry.shape()
            {
                object_boundaries[object] = Some(boundaries.len());
                boundaries.push(Boundary {
                    object,
                    center,
                    radius,
                    ior: material.ior,
                    absorption: material.absorption,
                    parent: None,
                });
            }
        }
        let parents: Vec<MediumId> = boundaries
            .iter()
            .map(|inner| {
                boundaries
                    .iter()
                    .enumerate()
                    .filter(|(_, outer)| {
                        (inner.center - outer.center).magnitude() + inner.radius < outer.radius
                    })
                    .min_by(|(_, a), (_, b)| a.radius.total_cmp(&b.radius))
                    .map(|(index, _)| index)
            })
            .collect();
        for (boundary, parent) in boundaries.iter_mut().zip(parents) {
            boundary.parent = parent;
        }
        Self {
            boundaries,
            object_boundaries,
        }
    }

    /// Only primary origins need a containment search; boundary crossings use parents.
    pub fn at(&self, point: Color, excluded: Option<usize>) -> MediumId {
        let mut radius = f64::INFINITY;
        let mut selected = None;
        for (index, boundary) in self.boundaries.iter().enumerate() {
            if excluded != Some(boundary.object)
                && boundary.radius < radius
                && (point - boundary.center).magnitude2() < boundary.radius * boundary.radius
            {
                radius = boundary.radius;
                selected = Some(index);
            }
        }
        selected
    }

    pub fn properties(&self, medium: MediumId) -> (f64, Color) {
        medium.map_or((1., black()), |index| {
            let boundary = &self.boundaries[index];
            (boundary.ior, boundary.absorption)
        })
    }

    /// The incident medium changes only if the BSDF chooses transmission.
    pub fn transmitted(&self, object: usize, entering: bool) -> MediumId {
        let boundary = self.object_boundaries[object];
        if entering {
            boundary
        } else {
            boundary.and_then(|index| self.boundaries[index].parent)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracer::geometry::Sphere;
    use crate::tracer::material::{Material, white};

    fn glass(center: Color, radius: f64, ior: f64) -> Arc<dyn Hitable> {
        Arc::new(Sphere {
            center,
            radius,
            material: Material {
                kind: Kind::Dielectric,
                ior,
                absorption: Color::new(0.1, 0.2, 0.3),
                ..Material::diffuse(white())
            },
        })
    }

    #[test]
    fn nonconcentric_and_disjoint_media_choose_the_actual_parent() {
        let media = MediumHierarchy::new(&[
            glass(Color::new(6., 0., 0.), 1., 1.5),
            glass(Color::new(-8., 0., 0.), 2., 1.2),
            glass(Color::new(5., 0., 0.), 4., 1.33),
        ]);
        assert_eq!(
            media.properties(media.at(Color::new(6., 0., 0.), None)).0,
            1.5
        );
        assert_eq!(media.properties(media.transmitted(0, false)).0, 1.33);
        assert_eq!(media.properties(media.transmitted(1, false)).0, 1.);
        assert_eq!(media.properties(media.transmitted(2, true)).0, 1.33);
        assert_eq!(
            media.properties(media.at(Color::new(-8., 0., 0.), None)).0,
            1.2
        );
        assert_eq!(
            media.properties(media.at(Color::new(20., 0., 0.), None)).0,
            1.
        );
    }

    #[test]
    fn medium_nesting_has_no_fixed_depth_limit() {
        let objects: Vec<_> = (1..=128)
            .map(|radius| glass(black(), f64::from(radius), 1. + f64::from(radius) / 100.))
            .collect();
        let media = MediumHierarchy::new(&objects);
        let mut current = media.at(black(), None);
        for (object, geometry) in objects.iter().enumerate() {
            assert_eq!(media.properties(current).0, geometry.material().ior);
            assert_eq!(media.properties(current).1, Color::new(0.1, 0.2, 0.3));
            current = media.transmitted(object, false);
        }
        assert_eq!(media.properties(current), (1., black()));
    }
}
