use super::*;

#[test]
fn diffuse_samples_match_brdf_and_pdf_and_preserve_albedo() {
    let m = Material::diffuse(Color::new(0.2, 0.5, 0.9));
    let mut rng = Sampler::new(1, 0, 0);
    let n = Color::unit_z();
    let mut mean_cosine = 0.;
    for _ in 0..100_000 {
        let s = m.sample(n, n, 1., 1., &mut rng).unwrap();
        assert!((s.weight(n) - m.albedo).magnitude() < 1e-12);
        assert!((s.pdf - s.wi.z / PI).abs() < 1e-12);
        mean_cosine += s.wi.z;
    }
    assert!((mean_cosine / 100_000. - 2. / 3.).abs() < 0.003);
}
#[test]
fn fresnel_matches_normal_incidence_brewster_and_total_internal_reflection() {
    assert!((dielectric_fresnel(1., 1., 1.5) - 0.04).abs() < 1e-14);
    let c = (1_f64 / (1. + 1.5_f64.powi(2))).sqrt();
    let rs = ((c - 1.5 * (1. - (1. - c * c) / 2.25).sqrt())
        / (c + 1.5 * (1. - (1. - c * c) / 2.25).sqrt()))
    .powi(2);
    assert!((dielectric_fresnel(c, 1., 1.5) - 0.5 * rs).abs() < 1e-14);
    assert_eq!(dielectric_fresnel(0.4358898943540673, 1.5, 1.), 1.);
    assert!((dielectric_fresnel(0.5, 1., 1.)).abs() < 1e-14);
}
#[test]
fn glass_transmission_has_radiance_scaling_and_snells_law() {
    let m = Material {
        kind: Kind::Dielectric,
        ..Material::diffuse(white())
    };
    let wo = Color::new(0.6, 0., 0.8);
    let n = Color::unit_z();
    let mut rng = Sampler::new(2, 0, 0);
    let mut transmissions = 0;
    for _ in 0..10000 {
        let s = m.sample(wo, n, 1., 1.5, &mut rng).unwrap();
        if s.transmission {
            transmissions += 1;
            assert!((s.wi.x + 0.4).abs() < 1e-12);
            assert!((s.wi.magnitude() - 1.).abs() < 1e-12);
            assert!((s.weight(n).x - 1. / 2.25).abs() < 1e-12);
            assert_eq!(s.eta, 1.5);
        } else {
            assert!((s.weight(n).x - 1.).abs() < 1e-12);
        }
    }
    assert!(transmissions > 9400);
}
#[test]
fn glass_total_internal_reflection_is_finite() {
    let m = Material {
        kind: Kind::Dielectric,
        ..Material::diffuse(white())
    };
    let wo = Color::new(0.9, 0., 0.4358898943540673);
    let s = m
        .sample(wo, Color::unit_z(), 1.5, 1., &mut Sampler::new(1, 0, 0))
        .unwrap();
    assert!(!s.transmission);
    assert_eq!(s.pdf, 1.);
    assert!((s.weight(Color::unit_z()) - white()).magnitude() < 1e-12);
}
#[test]
fn conductor_fresnel_matches_normal_incidence_and_stays_passive() {
    let eta = Color::new(0.2, 0.9, 1.1);
    let k = Color::new(3.9, 2.5, 2.1);
    let f = conductor_fresnel(1., eta, k);
    for i in 0..3 {
        assert!(
            (f[i] - ((eta[i] - 1.).powi(2) + k[i] * k[i]) / ((eta[i] + 1.).powi(2) + k[i] * k[i]))
                .abs()
                < 1e-12
        );
    }
    for i in 0..1000 {
        let f = conductor_fresnel(i as f64 / 999., eta, k);
        assert!((0..3).all(|i| (0. ..=1.).contains(&f[i])));
    }
}
#[test]
fn rough_conductor_is_reciprocal_and_does_not_create_energy() {
    let m = Material {
        kind: Kind::Conductor,
        eta: Color::new(0.2, 0.9, 1.1),
        k: Color::new(3.9, 2.5, 2.1),
        roughness: 0.6,
        ..Material::diffuse(white())
    };
    let n = Color::unit_z();
    let wo = Color::new(0.6, 0., 0.8);
    let mut rng = Sampler::new(3, 0, 0);
    let mut energy = black();
    for _ in 0..100_000 {
        if let Some(s) = m.sample(wo, n, 1., 1., &mut rng) {
            assert!((m.evaluate(wo, s.wi, n).0 - m.evaluate(s.wi, wo, n).0).magnitude() < 1e-9);
            energy += s.weight(n);
        }
    }
    energy /= 100_000.;
    assert!((0..3).all(|i| energy[i] > 0. && energy[i] <= 1.));
}
#[test]
fn emission_is_explicit_and_sided_and_absorption_depends_on_distance() {
    let m = Material {
        kind: Kind::Emitter,
        emission: Color::new(1., 2., 3.),
        ..Material::diffuse(black())
    };
    assert_eq!(m.emitted(Color::unit_z(), Color::unit_z()), m.emission);
    assert_eq!(m.emitted(Color::unit_z(), -Color::unit_z()), black());
    let sigma = Color::new(0.1, 0.2, 0.3);
    assert!(
        (attenuate(sigma, 2.) - multiply(attenuate(sigma, 1.), attenuate(sigma, 1.))).magnitude()
            < 1e-14
    );
}

#[test]
fn conductor_fresnel_uses_the_surrounding_medium() {
    let absolute = Material {
        kind: Kind::Conductor,
        eta: Color::new(0.2, 0.9, 1.1),
        k: Color::new(3.9, 2.5, 2.1),
        ..Material::diffuse(white())
    };
    let immersed = absolute.in_medium(1.33);
    let sample = immersed
        .sample(
            Color::unit_z(),
            Color::unit_z(),
            1.33,
            1.,
            &mut Sampler::new(1, 0, 0),
        )
        .unwrap();
    for c in 0..3 {
        let expected = ((absolute.eta[c] - 1.33).powi(2) + absolute.k[c].powi(2))
            / ((absolute.eta[c] + 1.33).powi(2) + absolute.k[c].powi(2));
        assert!((sample.weight(Color::unit_z())[c] - expected).abs() < 1e-12);
    }
}
