//! Ressort amorti, résolu de façon analytique : position et vitesse exactes à tout instant.

/// Paramètres physiques d'un ressort.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpringParams {
    pub mass: f64,
    pub stiffness: f64,
    pub damping: f64,
}

impl SpringParams {
    /// Façon SwiftUI : `response` = période du ressort sans amortissement (s), `damping_fraction` = 1
    /// sans rebond, moins de 1 rebondit.
    pub fn from_response(response: f64, damping_fraction: f64) -> Self {
        let response = response.max(1e-3);
        let w = std::f64::consts::TAU / response;
        Self {
            mass: 1.0,
            stiffness: w * w,
            damping: 2.0 * damping_fraction.max(0.0) * w,
        }
    }
}

/// Écart à la cible `x` et vitesse `v` au temps `t`, pour un écart initial `x0` et une vitesse
/// initiale `v0` (unités par seconde).
pub fn spring_at(p: SpringParams, x0: f64, v0: f64, t: f64) -> (f64, f64) {
    let m = p.mass.max(1e-6);
    let k = p.stiffness.max(1e-6);
    let c = p.damping.max(0.0);
    let w0 = (k / m).sqrt();
    let zeta = c / (2.0 * (k * m).sqrt());
    if (zeta - 1.0).abs() < 1e-6 {
        // Amortissement critique.
        let e = (-w0 * t).exp();
        let b = v0 + w0 * x0;
        let x = e * (x0 + b * t);
        let v = e * (b - w0 * (x0 + b * t));
        (x, v)
    } else if zeta < 1.0 {
        let wd = w0 * (1.0 - zeta * zeta).sqrt();
        let a = zeta * w0;
        let e = (-a * t).exp();
        let b = (v0 + a * x0) / wd;
        let (s, co) = (wd * t).sin_cos();
        let x = e * (x0 * co + b * s);
        let v = e * ((b * wd - a * x0) * co - (x0 * wd + a * b) * s);
        (x, v)
    } else {
        let r = (zeta * zeta - 1.0).sqrt();
        let r1 = -w0 * (zeta - r);
        let r2 = -w0 * (zeta + r);
        let c2 = (v0 - r1 * x0) / (r2 - r1);
        let c1 = x0 - c2;
        let (e1, e2) = ((r1 * t).exp(), (r2 * t).exp());
        (c1 * e1 + c2 * e2, c1 * r1 * e1 + c2 * r2 * e2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numeric_v(p: SpringParams, x0: f64, v0: f64, t: f64) -> f64 {
        let h = 1e-5;
        (spring_at(p, x0, v0, t + h).0 - spring_at(p, x0, v0, t - h).0) / (2.0 * h)
    }

    #[test]
    fn trois_regimes() {
        for zeta in [0.3, 1.0, 2.5] {
            let p = SpringParams::from_response(0.5, zeta);
            let (x, v) = spring_at(p, 1.0, 0.0, 0.0);
            assert!((x - 1.0).abs() < 1e-9 && v.abs() < 1e-9, "ζ={zeta}");
            for t in [0.05, 0.2, 0.7] {
                let (_, v) = spring_at(p, 1.0, 2.0, t);
                assert!((v - numeric_v(p, 1.0, 2.0, t)).abs() < 1e-4, "ζ={zeta} t={t}");
            }
            // Au repos après quelques secondes.
            let (x, v) = spring_at(p, 1.0, 0.0, 6.0);
            assert!(x.abs() < 1e-3 && v.abs() < 1e-2, "ζ={zeta}: {x} {v}");
        }
        // Sous-amorti : dépasse la cible.
        let p = SpringParams::from_response(0.5, 0.3);
        assert!((0..100).any(|i| spring_at(p, 1.0, 0.0, i as f64 * 0.01).0 < -0.1));
    }
}
