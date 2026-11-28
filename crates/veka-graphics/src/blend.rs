//! Modes de fusion (étape 8) : formules de la norme W3C *Compositing and Blending*, celles de SwiftUI,
//! CSS et Figma.
//!
//! Le renderer wgpu applique les mêmes formules dans son shader (`group.wgsl`) ; [`composite`] est la
//! version processeur, qui sert de référence aux tests et aux renderers sans shader.

/// Comment un groupe se mélange avec ce qui est dessiné derrière lui.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BlendMode {
    /// Par-dessus, sans mélange.
    #[default]
    Normal = 0,
    Multiply = 1,
    Screen = 2,
    Overlay = 3,
    Darken = 4,
    Lighten = 5,
    ColorDodge = 6,
    ColorBurn = 7,
    SoftLight = 8,
    HardLight = 9,
    Difference = 10,
    Exclusion = 11,
    Hue = 12,
    Saturation = 13,
    Color = 14,
    Luminosity = 15,
    /// Le groupe ne se dessine que là où le fond est déjà dessiné.
    SourceAtop = 16,
    /// Le groupe passe **derrière** ce qui est déjà dessiné.
    DestinationOver = 17,
    /// Le groupe efface le fond (perce un trou à sa forme).
    DestinationOut = 18,
    PlusDarker = 19,
    PlusLighter = 20,
}

impl BlendMode {
    /// Tous les modes, dans l'ordre de leur numéro.
    pub const ALL: [BlendMode; 21] = [
        BlendMode::Normal,
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::Darken,
        BlendMode::Lighten,
        BlendMode::ColorDodge,
        BlendMode::ColorBurn,
        BlendMode::SoftLight,
        BlendMode::HardLight,
        BlendMode::Difference,
        BlendMode::Exclusion,
        BlendMode::Hue,
        BlendMode::Saturation,
        BlendMode::Color,
        BlendMode::Luminosity,
        BlendMode::SourceAtop,
        BlendMode::DestinationOver,
        BlendMode::DestinationOut,
        BlendMode::PlusDarker,
        BlendMode::PlusLighter,
    ];

    /// Les noms, comme en SwiftUI (`.multiply`, `.colorDodge`…), dans l'ordre de [`ALL`](Self::ALL).
    pub const NAMES: [&'static str; 21] = [
        "normal",
        "multiply",
        "screen",
        "overlay",
        "darken",
        "lighten",
        "colorDodge",
        "colorBurn",
        "softLight",
        "hardLight",
        "difference",
        "exclusion",
        "hue",
        "saturation",
        "color",
        "luminosity",
        "sourceAtop",
        "destinationOver",
        "destinationOut",
        "plusDarker",
        "plusLighter",
    ];

    pub fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }

    pub fn from_name(name: &str) -> Option<BlendMode> {
        Self::NAMES.iter().position(|n| *n == name).map(|i| Self::ALL[i])
    }

    pub fn from_index(i: u32) -> Option<BlendMode> {
        Self::ALL.get(i as usize).copied()
    }

    /// Un mode qui ne fait que de la composition alpha (normal et découpes) : calculé en lumière
    /// linéaire, comme le reste du rendu. Les autres mélangent les couleurs en sRGB, comme les outils
    /// de dessin.
    pub fn is_alpha_only(self) -> bool {
        matches!(
            self,
            BlendMode::Normal
                | BlendMode::SourceAtop
                | BlendMode::DestinationOver
                | BlendMode::DestinationOut
        )
    }
}

fn srgb_encode(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn srgb_decode(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

type Rgb = [f32; 3];

fn lum(c: Rgb) -> f32 {
    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}

fn clip_color(c: Rgb) -> Rgb {
    let l = lum(c);
    let n = c[0].min(c[1]).min(c[2]);
    let x = c[0].max(c[1]).max(c[2]);
    let mut c = c;
    if n < 0.0 {
        let d = (l - n).max(1e-6);
        c = c.map(|v| l + (v - l) * l / d);
    }
    if x > 1.0 {
        let d = (x - l).max(1e-6);
        c = c.map(|v| l + (v - l) * (1.0 - l) / d);
    }
    c
}

fn set_lum(c: Rgb, l: f32) -> Rgb {
    let d = l - lum(c);
    clip_color(c.map(|v| v + d))
}

fn sat(c: Rgb) -> f32 {
    c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])
}

fn set_sat(c: Rgb, s: f32) -> Rgb {
    let mx = c[0].max(c[1]).max(c[2]);
    let mn = c[0].min(c[1]).min(c[2]);
    if mx > mn {
        c.map(|v| (v - mn) * s / (mx - mn))
    } else {
        [0.0; 3]
    }
}

/// B(cb, cs) d'un mode séparable, composante par composante.
fn separable(mode: BlendMode, b: f32, s: f32) -> f32 {
    match mode {
        BlendMode::Multiply => b * s,
        BlendMode::Screen => b + s - b * s,
        BlendMode::Overlay => separable(BlendMode::HardLight, s, b),
        BlendMode::Darken => b.min(s),
        BlendMode::Lighten => b.max(s),
        BlendMode::ColorDodge => {
            if b <= 0.0 {
                0.0
            } else if s >= 1.0 {
                1.0
            } else {
                (b / (1.0 - s)).min(1.0)
            }
        }
        BlendMode::ColorBurn => {
            if b >= 1.0 {
                1.0
            } else if s <= 0.0 {
                0.0
            } else {
                1.0 - ((1.0 - b) / s).min(1.0)
            }
        }
        BlendMode::HardLight => {
            if s <= 0.5 {
                b * 2.0 * s
            } else {
                let s2 = 2.0 * s - 1.0;
                b + s2 - b * s2
            }
        }
        BlendMode::SoftLight => {
            if s <= 0.5 {
                b - (1.0 - 2.0 * s) * b * (1.0 - b)
            } else {
                let d = if b <= 0.25 {
                    ((16.0 * b - 12.0) * b + 4.0) * b
                } else {
                    b.sqrt()
                };
                b + (2.0 * s - 1.0) * (d - b)
            }
        }
        BlendMode::Difference => (b - s).abs(),
        BlendMode::Exclusion => b + s - 2.0 * b * s,
        _ => s,
    }
}

/// B(cb, cs) pour les couleurs non prémultipliées, en sRGB.
fn mix(mode: BlendMode, b: Rgb, s: Rgb) -> Rgb {
    match mode {
        BlendMode::Hue => set_lum(set_sat(s, sat(b)), lum(b)),
        BlendMode::Saturation => set_lum(set_sat(b, sat(s)), lum(b)),
        BlendMode::Color => set_lum(s, lum(b)),
        BlendMode::Luminosity => set_lum(b, lum(s)),
        m => [0, 1, 2].map(|i| separable(m, b[i], s[i])),
    }
}

/// Compose `source` (le groupe) sur `backdrop` avec `mode`. Les deux sont en lumière linéaire,
/// prémultipliés, comme les pixels du renderer ; le résultat aussi.
pub fn composite(mode: BlendMode, backdrop: [f32; 4], source: [f32; 4]) -> [f32; 4] {
    let (ab, a_s) = (backdrop[3].clamp(0.0, 1.0), source[3].clamp(0.0, 1.0));
    let unpremul = |p: [f32; 4], a: f32| -> Rgb {
        if a > 0.0 {
            [p[0] / a, p[1] / a, p[2] / a]
        } else {
            [0.0; 3]
        }
    };
    match mode {
        // Composition alpha : en linéaire, sur les valeurs prémultipliées.
        BlendMode::Normal => std::array::from_fn(|i| source[i] + backdrop[i] * (1.0 - a_s)),
        BlendMode::SourceAtop => {
            let mut o: [f32; 4] = std::array::from_fn(|i| source[i] * ab + backdrop[i] * (1.0 - a_s));
            o[3] = ab;
            o
        }
        BlendMode::DestinationOver => std::array::from_fn(|i| backdrop[i] + source[i] * (1.0 - ab)),
        BlendMode::DestinationOut => backdrop.map(|v| v * (1.0 - a_s)),
        _ => {
            // Mélange des couleurs : en sRGB, non prémultipliées.
            let cb = unpremul(backdrop, ab).map(srgb_encode);
            let cs = unpremul(source, a_s).map(srgb_encode);
            let (rgb, ao): (Rgb, f32) = match mode {
                BlendMode::PlusLighter => {
                    let ao = (a_s + ab).min(1.0);
                    let c = [0, 1, 2].map(|i| (cs[i] * a_s + cb[i] * ab).min(1.0));
                    (c, ao)
                }
                BlendMode::PlusDarker => {
                    let ao = (a_s + ab).min(1.0);
                    let c =
                        [0, 1, 2].map(|i| (ao - ((a_s - cs[i] * a_s) + (ab - cb[i] * ab)).min(ao)).max(0.0));
                    (c, ao)
                }
                m => {
                    let bl = mix(m, cb, cs);
                    let ao = a_s + ab * (1.0 - a_s);
                    let c = [0, 1, 2].map(|i| {
                        let cs2 = (1.0 - ab) * cs[i] + ab * bl[i].clamp(0.0, 1.0);
                        a_s * cs2 + ab * cb[i] * (1.0 - a_s)
                    });
                    (c, ao)
                }
            };
            if ao <= 0.0 {
                return [0.0; 4];
            }
            let lin = rgb.map(|c| srgb_decode(c / ao) * ao);
            [lin[0], lin[1], lin[2], ao]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opaque(r: f32, g: f32, b: f32) -> [f32; 4] {
        [srgb_decode(r), srgb_decode(g), srgb_decode(b), 1.0]
    }

    fn srgb(p: [f32; 4]) -> [f32; 3] {
        [0, 1, 2].map(|i| (srgb_encode(p[i] / p[3].max(1e-6)) * 1000.0).round() / 1000.0)
    }

    #[test]
    fn names_round_trip() {
        for (i, m) in BlendMode::ALL.iter().enumerate() {
            assert_eq!(*m as usize, i);
            assert_eq!(BlendMode::from_name(m.name()), Some(*m));
        }
        assert_eq!(BlendMode::from_name("mutliply"), None);
    }

    #[test]
    fn separable_modes_match_the_w3c_values() {
        // Fond gris moyen (0,5) sous un rouge à 0,8 / vert à 0,2 / bleu à 0,6 (valeurs sRGB).
        let b = opaque(0.5, 0.5, 0.5);
        let s = opaque(0.8, 0.2, 0.6);
        let cases: [(BlendMode, [f32; 3]); 11] = [
            (BlendMode::Multiply, [0.4, 0.1, 0.3]),
            (BlendMode::Screen, [0.9, 0.6, 0.8]),
            (BlendMode::Overlay, [0.8, 0.2, 0.6]),
            (BlendMode::Darken, [0.5, 0.2, 0.5]),
            (BlendMode::Lighten, [0.8, 0.5, 0.6]),
            (BlendMode::ColorDodge, [1.0, 0.625, 1.0]),
            (BlendMode::ColorBurn, [0.375, 0.0, 0.167]),
            (BlendMode::HardLight, [0.8, 0.2, 0.6]),
            (BlendMode::SoftLight, [0.624, 0.35, 0.541]),
            (BlendMode::Difference, [0.3, 0.3, 0.1]),
            (BlendMode::Exclusion, [0.5, 0.5, 0.5]),
        ];
        for (m, want) in cases {
            let got = srgb(composite(m, b, s));
            for i in 0..3 {
                assert!(
                    (got[i] - want[i]).abs() < 0.002,
                    "{m:?} : {got:?} au lieu de {want:?}"
                );
            }
        }
    }

    #[test]
    fn component_modes_keep_the_right_part() {
        let b = opaque(0.2, 0.4, 0.8);
        let s = opaque(0.9, 0.3, 0.1);
        // La luminosité de la source garde la teinte du fond.
        let l = srgb(composite(BlendMode::Luminosity, b, s));
        let lum_s = lum([0.9, 0.3, 0.1]);
        assert!((lum(l) - lum_s).abs() < 0.005, "{l:?}");
        // La couleur de la source garde la luminosité du fond.
        let c = srgb(composite(BlendMode::Color, b, s));
        assert!((lum(c) - lum([0.2, 0.4, 0.8])).abs() < 0.005, "{c:?}");
        // Sur un fond gris, la teinte et la saturation de la source donnent du gris (saturation nulle).
        let g = opaque(0.5, 0.5, 0.5);
        let h = srgb(composite(BlendMode::Hue, g, s));
        assert!(
            (h[0] - h[1]).abs() < 0.002 && (h[1] - h[2]).abs() < 0.002,
            "{h:?}"
        );
    }

    #[test]
    fn transparent_source_leaves_the_backdrop() {
        let b = opaque(0.3, 0.6, 0.9);
        for m in BlendMode::ALL {
            let o = composite(m, b, [0.0; 4]);
            for i in 0..4 {
                assert!((o[i] - b[i]).abs() < 1e-3, "{m:?} : {o:?}");
            }
        }
    }

    #[test]
    fn transparent_backdrop_shows_the_source() {
        // Sans fond, un mode de mélange laisse la source telle quelle (sauf les découpes).
        let s = opaque(0.8, 0.2, 0.6);
        for m in BlendMode::ALL {
            let o = composite(m, [0.0; 4], s);
            let want = match m {
                BlendMode::SourceAtop | BlendMode::DestinationOut => [0.0; 4],
                _ => s,
            };
            for i in 0..4 {
                assert!((o[i] - want[i]).abs() < 1e-3, "{m:?} : {o:?}");
            }
        }
    }

    #[test]
    fn alpha_modes() {
        let b = opaque(0.0, 0.0, 1.0);
        let s = [0.5, 0.0, 0.0, 0.5]; // rouge à 50 %, prémultiplié
        assert_eq!(composite(BlendMode::DestinationOut, b, s)[3], 0.5);
        assert_eq!(composite(BlendMode::DestinationOver, b, s), b);
        let a = composite(BlendMode::SourceAtop, b, s);
        assert_eq!(a[3], 1.0);
        assert!((a[0] - 0.5).abs() < 1e-6 && (a[2] - 0.5).abs() < 1e-6);
        // Plus clair : additionne ; plus sombre : soustrait.
        let w = srgb(composite(
            BlendMode::PlusLighter,
            opaque(0.5, 0.5, 0.5),
            opaque(0.25, 0.25, 0.25),
        ));
        assert!((w[0] - 0.75).abs() < 0.002, "{w:?}");
        let d = srgb(composite(
            BlendMode::PlusDarker,
            opaque(0.5, 0.5, 0.5),
            opaque(0.75, 0.75, 0.75),
        ));
        assert!((d[0] - 0.25).abs() < 0.002, "{d:?}");
    }
}
