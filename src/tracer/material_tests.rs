use super::*;

#[test]
fn glass_totally_reflects_without_nan_at_steep_exit_angles() {
    let normal = Vector3::unit_y();
    let incoming = Vector3::new(0.9, 0.4358898943540673, 0.0).normalize();
    assert!(refract(&incoming, &-normal, 1.5).is_none());
    for scale in [0.1, 1.0, 10.0] {
        let ray = ray::Ray::new_from(Vector3::new(0.0, 0.0, 0.0), scale * incoming);
        let scattered = Dielectric::new(1.5)
            .scatter(
                &ray,
                &normal,
                &Vector3::new(0.0, 0.0, 0.0),
                &mut Sampler::new(1, 0, 0),
            )
            .unwrap();
        let expected = Vector3::new(incoming.x, -incoming.y, 0.0);
        assert!((*scattered.direction() - expected).magnitude() < 1e-12);
        assert!((scattered.direction().magnitude() - 1.0).abs() < 1e-12);
    }
}

#[test]
fn refraction_at_normal_incidence_preserves_direction() {
    let transmitted = refract(&-Vector3::unit_y(), &Vector3::unit_y(), 1.0 / 1.5).unwrap();
    assert!((transmitted + Vector3::unit_y()).magnitude() < 1e-12);
}

#[test]
fn emitters_terminate_paths() {
    let origin = Vector3::new(0.0, 0.0, 0.0);
    let ray = ray::Ray::new_from(origin, -Vector3::unit_y());
    let light = DiffuseLight::new(1.0, 2.0, 3.0);
    assert!(
        light
            .scatter(
                &ray,
                &Vector3::unit_y(),
                &origin,
                &mut Sampler::new(1, 0, 0)
            )
            .is_none()
    );
    assert_eq!(light.emitted(), Vector3::new(1.0, 2.0, 3.0));
}

#[test]
fn unknown_materials_keep_the_flat_fallback() {
    let factory = MaterialsFactory::new();
    let material = factory.get_material_by_key("unknown-material");
    let origin = Vector3::new(0.0, 0.0, 0.0);
    let ray = ray::Ray::new_from(origin, -Vector3::unit_y());
    let scattered = material
        .scatter(
            &ray,
            &Vector3::unit_y(),
            &origin,
            &mut Sampler::new(1, 0, 0),
        )
        .unwrap();
    assert!(scattered.direction().y > 0.0);
    assert!((scattered.direction().magnitude() - 1.0).abs() < 1e-12);
}
