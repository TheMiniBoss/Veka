//! Espaces de couleur : sRGB, linéaire, Oklab et OkLCH (<https://bottosson.github.io/posts/oklab/>).

use veka_foundation::{Color, linear_to_srgb, srgb_to_linear};

/// Espace dans lequel un dégradé (ou une animation de couleur) interpole.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ColorSpace {
    /// sRGB encodé (comme la plupart des outils de design : milieux ternes entre complémentaires).
    Srgb,
    /// Lumière linéaire.
    Linear,
    /// Oklab, perceptuel : dégradés réguliers et propres (défaut).
    #[default]
    Oklab,
    /// Oklab en coordonnées polaires (luminance, chroma, teinte) : garde la saturation, la teinte prend le
    /// plus court chemin.
    OkLch,
}

/// Couleur Oklab (L entre 0 et 1, a et b à peu près entre -0,4 et 0,4).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Oklab {
    pub l: f32,
    pub a: f32,
    pub b: f32,
}

/// Oklab polaire. Teinte en degrés (0 à 360).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OkLch {
    pub l: f32,
    pub c: f32,
    pub h: f32,
}

/// Composantes linéaires (r, g, b) → Oklab.
pub fn linear_to_oklab([r, g, b]: [f32; 3]) -> Oklab {
    let l = 0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b;
    let m = 0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b;
    let s = 0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b;
    let (l, m, s) = (l.cbrt(), m.cbrt(), s.cbrt());
    Oklab {
        l: 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        a: 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        b: 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    }
}

/// Oklab → composantes linéaires (r, g, b), non bornées.
pub fn oklab_to_linear(c: Oklab) -> [f32; 3] {
    let l = c.l + 0.396_337_78 * c.a + 0.215_803_76 * c.b;
    let m = c.l - 0.105_561_346 * c.a - 0.063_854_17 * c.b;
    let s = c.l - 0.089_484_18 * c.a - 1.291_485_5 * c.b;
    let (l, m, s) = (l * l * l, m * m * m, s * s * s);
    [
        4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
        -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
        -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
    ]
}

pub fn oklab_to_oklch(c: Oklab) -> OkLch {
    let chroma = (c.a * c.a + c.b * c.b).sqrt();
    let h = if chroma < 1e-6 {
        0.0
    } else {
        c.b.atan2(c.a).to_degrees().rem_euclid(360.0)
    };
    OkLch { l: c.l, c: chroma, h }
}

pub fn oklch_to_oklab(c: OkLch) -> Oklab {
    let h = c.h.to_radians();
    Oklab {
        l: c.l,
        a: c.c * h.cos(),
        b: c.c * h.sin(),
    }
}

/// Une couleur exprimée dans un espace, alpha **prémultiplié** dans les composantes (pour qu'un stop
/// transparent n'assombrisse pas le dégradé). `w` = alpha.
pub fn to_space(color: Color, space: ColorSpace) -> [f32; 4] {
    let a = color.a.clamp(0.0, 1.0);
    let srgb = [color.r, color.g, color.b];
    let lin = srgb.map(srgb_to_linear);
    let v = match space {
        ColorSpace::Srgb => srgb,
        ColorSpace::Linear => lin,
        ColorSpace::Oklab => {
            let c = linear_to_oklab(lin);
            [c.l, c.a, c.b]
        }
        ColorSpace::OkLch => {
            // La teinte n'est pas prémultipliée (ce n'est pas une grandeur additive).
            let c = oklab_to_oklch(linear_to_oklab(lin));
            return [c.l * a, c.c * a, c.h, a];
        }
    };
    [v[0] * a, v[1] * a, v[2] * a, a]
}

/// Inverse de [`to_space`], vers une couleur sRGB non prémultipliée.
pub fn from_space(v: [f32; 4], space: ColorSpace) -> Color {
    let a = v[3].clamp(0.0, 1.0);
    if a <= 1e-6 {
        return Color::Clear;
    }
    let un = |x: f32| x / a;
    let srgb = match space {
        ColorSpace::Srgb => [un(v[0]), un(v[1]), un(v[2])],
        ColorSpace::Linear => [un(v[0]), un(v[1]), un(v[2])].map(linear_to_srgb),
        ColorSpace::Oklab => oklab_to_linear(Oklab {
            l: un(v[0]),
            a: un(v[1]),
            b: un(v[2]),
        })
        .map(linear_to_srgb),
        ColorSpace::OkLch => oklab_to_linear(oklch_to_oklab(OkLch {
            l: un(v[0]),
            c: un(v[1]),
            h: v[2],
        }))
        .map(linear_to_srgb),
    };
    Color::rgba(
        srgb[0].clamp(0.0, 1.0),
        srgb[1].clamp(0.0, 1.0),
        srgb[2].clamp(0.0, 1.0),
        a,
    )
}

/// Interpolation de deux valeurs d'un espace (en OkLCH, la teinte prend le plus court chemin).
pub fn lerp_in_space(a: [f32; 4], b: [f32; 4], t: f32, space: ColorSpace) -> [f32; 4] {
    let l = |x: f32, y: f32| x + (y - x) * t;
    let mut out = [l(a[0], b[0]), l(a[1], b[1]), l(a[2], b[2]), l(a[3], b[3])];
    if space == ColorSpace::OkLch {
        // Une couleur sans chroma (gris) prend la teinte de l'autre.
        let (ha, hb) = match (a[1] <= 1e-5, b[1] <= 1e-5) {
            (true, false) => (b[2], b[2]),
            (false, true) => (a[2], a[2]),
            _ => (a[2], b[2]),
        };
        let mut d = (hb - ha).rem_euclid(360.0);
        if d > 180.0 {
            d -= 360.0;
        }
        out[2] = (ha + d * t).rem_euclid(360.0);
    }
    out
}

/// De `a` à `b` en `t`, interpolé dans `space`.
pub fn mix(a: Color, b: Color, t: f32, space: ColorSpace) -> Color {
    from_space(
        lerp_in_space(to_space(a, space), to_space(b, space), t, space),
        space,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Color, b: Color) -> bool {
        (a.r - b.r).abs() < 2e-3
            && (a.g - b.g).abs() < 2e-3
            && (a.b - b.b).abs() < 2e-3
            && (a.a - b.a).abs() < 1e-4
    }

    #[test]
    fn oklab_aller_retour() {
        for c in [
            Color::Red,
            Color::Blue,
            Color::hex(0x34C759),
            Color::White,
            Color::Black,
            Color::Gray,
        ] {
            for space in [
                ColorSpace::Srgb,
                ColorSpace::Linear,
                ColorSpace::Oklab,
                ColorSpace::OkLch,
            ] {
                let back = from_space(to_space(c, space), space);
                assert!(close(back, c), "{c:?} {space:?} → {back:?}");
            }
        }
    }

    #[test]
    fn blanc_en_oklab() {
        let w = linear_to_oklab([1.0, 1.0, 1.0]);
        assert!((w.l - 1.0).abs() < 1e-3 && w.a.abs() < 1e-3 && w.b.abs() < 1e-3);
    }

    #[test]
    fn extremites() {
        let (a, b) = (Color::Blue, Color::Yellow);
        for space in [ColorSpace::Srgb, ColorSpace::Oklab, ColorSpace::OkLch] {
            assert!(close(mix(a, b, 0.0, space), a));
            assert!(close(mix(a, b, 1.0, space), b));
        }
    }

    #[test]
    fn milieu_oklab_plus_lumineux_que_srgb() {
        // Bleu → jaune : en sRGB le milieu est un gris terne ; en Oklab il garde sa luminance.
        let srgb = mix(Color::Blue, Color::Yellow, 0.5, ColorSpace::Srgb);
        let ok = mix(Color::Blue, Color::Yellow, 0.5, ColorSpace::Oklab);
        let lum = |c: Color| linear_to_oklab([c.r, c.g, c.b].map(srgb_to_linear)).l;
        assert!(lum(ok) > lum(srgb));
    }

    #[test]
    fn transparent_n_assombrit_pas() {
        // Rouge → transparent : le milieu reste rouge (seul l'alpha baisse).
        let m = mix(Color::Red, Color::Clear, 0.5, ColorSpace::Oklab);
        assert!(close(m, Color::Red.with_alpha(0.5)), "{m:?}");
    }

    #[test]
    fn teinte_par_le_plus_court_chemin() {
        let a = [0.5, 0.1, 350.0, 1.0];
        let b = [0.5, 0.1, 10.0, 1.0];
        let m = lerp_in_space(a, b, 0.5, ColorSpace::OkLch);
        assert!(m[2] < 1.0 || m[2] > 359.0, "{m:?}");
    }
}
