//! Courbes à durée : la progression (0 à 1) en fonction du temps (0 à 1).

use crate::animation::Curve;

/// Progression adoucie en `t` (0 à 1). Un ressort n'est pas une courbe à durée : renvoie `t`.
pub fn ease(curve: &Curve, t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    match *curve {
        Curve::Linear => t,
        Curve::EaseIn => cubic_bezier(0.42, 0.0, 1.0, 1.0, t),
        Curve::EaseOut => cubic_bezier(0.0, 0.0, 0.58, 1.0, t),
        Curve::EaseInOut => cubic_bezier(0.42, 0.0, 0.58, 1.0, t),
        Curve::CircEaseIn => 1.0 - (1.0 - t * t).max(0.0).sqrt(),
        Curve::CircEaseOut => (1.0 - (t - 1.0) * (t - 1.0)).max(0.0).sqrt(),
        Curve::CircEaseInOut => {
            if t < 0.5 {
                (1.0 - (1.0 - 4.0 * t * t).max(0.0).sqrt()) * 0.5
            } else {
                let u = -2.0 * t + 2.0;
                ((1.0 - u * u).max(0.0).sqrt() + 1.0) * 0.5
            }
        }
        Curve::Bezier(x1, y1, x2, y2) => cubic_bezier(x1 as f64, y1 as f64, x2 as f64, y2 as f64, t),
        Curve::Spring { .. } => t,
    }
}

/// `cubic-bezier(x1, y1, x2, y2)` de CSS en `x` : on cherche le paramètre dont l'abscisse vaut `x`
/// (Newton, puis dichotomie si besoin), et on renvoie son ordonnée.
pub fn cubic_bezier(x1: f64, y1: f64, x2: f64, y2: f64, x: f64) -> f64 {
    let x1 = x1.clamp(0.0, 1.0);
    let x2 = x2.clamp(0.0, 1.0);
    // B(s) = 3(1-s)²s p1 + 3(1-s)s² p2 + s³, en polynôme : a s³ + b s² + c s.
    let coef = |p1: f64, p2: f64| {
        let c = 3.0 * p1;
        let b = 3.0 * (p2 - p1) - c;
        let a = 1.0 - c - b;
        (a, b, c)
    };
    let (ax, bx, cx) = coef(x1, x2);
    let (ay, by, cy) = coef(y1, y2);
    let bx_at = |s: f64| ((ax * s + bx) * s + cx) * s;
    let dx_at = |s: f64| (3.0 * ax * s + 2.0 * bx) * s + cx;
    let mut s = x;
    for _ in 0..8 {
        let err = bx_at(s) - x;
        if err.abs() < 1e-7 {
            return ((ay * s + by) * s + cy) * s;
        }
        let d = dx_at(s);
        if d.abs() < 1e-6 {
            break;
        }
        s -= err / d;
    }
    let (mut lo, mut hi) = (0.0, 1.0);
    s = x;
    for _ in 0..60 {
        let v = bx_at(s);
        if (v - x).abs() < 1e-7 {
            break;
        }
        if v < x {
            lo = s;
        } else {
            hi = s;
        }
        s = (lo + hi) * 0.5;
    }
    ((ay * s + by) * s + cy) * s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bornes_et_symetrie() {
        for c in [
            Curve::Linear,
            Curve::EaseIn,
            Curve::EaseOut,
            Curve::EaseInOut,
            Curve::CircEaseIn,
            Curve::CircEaseOut,
            Curve::CircEaseInOut,
        ] {
            assert!(ease(&c, 0.0).abs() < 1e-6, "{c:?}");
            assert!((ease(&c, 1.0) - 1.0).abs() < 1e-6, "{c:?}");
        }
        assert!((ease(&Curve::EaseInOut, 0.5) - 0.5).abs() < 1e-6);
        assert!((ease(&Curve::CircEaseInOut, 0.5) - 0.5).abs() < 1e-6);
        // Ease in : lent au début.
        assert!(ease(&Curve::EaseIn, 0.25) < 0.25);
        assert!(ease(&Curve::EaseOut, 0.25) > 0.25);
    }

    #[test]
    fn bezier_comme_css() {
        // cubic-bezier(0.25, 0.1, 0.25, 1) (« ease » de CSS) à 50 % ≈ 0,8024.
        assert!((cubic_bezier(0.25, 0.1, 0.25, 1.0, 0.5) - 0.8024).abs() < 1e-3);
        // Diagonale = linéaire.
        assert!((cubic_bezier(1.0 / 3.0, 1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0, 0.3) - 0.3).abs() < 1e-5);
        // Dépassement (y hors de 0..1) permis.
        assert!(cubic_bezier(0.3, 1.6, 0.6, 1.0, 0.6) > 1.0);
    }
}
