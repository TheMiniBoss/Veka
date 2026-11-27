//! Thèmes, palette de Veka et réglages de couleur.

use std::collections::BTreeMap;
use std::sync::Arc;

use veka_foundation::{Color, ColorScheme};

use crate::paint::{Hierarchy, Ink, Paint, Token};

/// Une couleur de thème : une peinture par mode (un jeton peut valoir un dégradé).
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeColor {
    pub light: Paint,
    pub dark: Paint,
}

impl ThemeColor {
    pub fn get(&self, scheme: ColorScheme) -> &Paint {
        match scheme {
            ColorScheme::Light => &self.light,
            ColorScheme::Dark => &self.dark,
        }
    }
}

/// Thème Colora : jetons → peintures par mode, et mesures de design (rayons, espacements…).
/// Un thème ne redéfinit que ce qui change : un jeton absent vient du parent, puis de Veka.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Theme {
    pub name: String,
    pub parent: Option<Arc<Theme>>,
    pub colors: BTreeMap<Token, ThemeColor>,
    pub metrics: BTreeMap<String, f32>,
}

impl Theme {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    pub fn parent(mut self, parent: impl Into<Arc<Theme>>) -> Self {
        self.parent = Some(parent.into());
        self
    }

    /// Valeur claire et valeur sombre d'un jeton.
    pub fn color(mut self, token: Token, light: impl Into<Paint>, dark: impl Into<Paint>) -> Self {
        self.colors.insert(
            token,
            ThemeColor {
                light: light.into(),
                dark: dark.into(),
            },
        );
        self
    }

    /// Même valeur dans les deux modes.
    pub fn color_all(self, token: Token, paint: impl Into<Paint>) -> Self {
        let p = paint.into();
        self.color(token, p.clone(), p)
    }

    pub fn metric(mut self, name: impl Into<String>, value: f32) -> Self {
        self.metrics.insert(name.into(), value);
        self
    }

    /// Cherche le jeton dans ce thème et ses parents (pas dans les valeurs de Veka).
    pub fn find_color(&self, token: &Token) -> Option<&ThemeColor> {
        self.colors
            .get(token)
            .or_else(|| self.parent.as_deref().and_then(|p| p.find_color(token)))
    }

    pub fn find_metric(&self, name: &str) -> Option<f32> {
        self.metrics
            .get(name)
            .copied()
            .or_else(|| self.parent.as_deref().and_then(|p| p.find_metric(name)))
    }
}

/// Les jetons de Veka : (nom, clair, alpha clair, sombre, alpha sombre). Palette système d'Apple, comme
/// Alpha 13.
const DEFAULTS: &[(&str, u32, f32, u32, f32)] = &[
    ("Accent", 0x007AFF, 1.0, 0x0A84FF, 1.0),
    ("Black", 0x000000, 1.0, 0x000000, 1.0),
    ("Blue", 0x007AFF, 1.0, 0x0A84FF, 1.0),
    ("Brown", 0xA2845E, 1.0, 0xAC8E68, 1.0),
    ("Clear", 0x000000, 0.0, 0x000000, 0.0),
    ("Cyan", 0x32ADE6, 1.0, 0x64D2FF, 1.0),
    ("Gray", 0x8E8E93, 1.0, 0x8E8E93, 1.0),
    ("Green", 0x34C759, 1.0, 0x30D158, 1.0),
    ("Indigo", 0x5856D6, 1.0, 0x5E5CE6, 1.0),
    ("Mint", 0x00C7BE, 1.0, 0x63E6E2, 1.0),
    ("Orange", 0xFF9500, 1.0, 0xFF9F0A, 1.0),
    ("Pink", 0xFF2D55, 1.0, 0xFF375F, 1.0),
    ("Purple", 0xAF52DE, 1.0, 0xBF5AF2, 1.0),
    ("Red", 0xFF3B30, 1.0, 0xFF453A, 1.0),
    ("Teal", 0x30B0C7, 1.0, 0x40C8E0, 1.0),
    ("White", 0xFFFFFF, 1.0, 0xFFFFFF, 1.0),
    ("Yellow", 0xFFCC00, 1.0, 0xFFD60A, 1.0),
    ("Tint", 0x007AFF, 1.0, 0x0A84FF, 1.0),
    ("Background", 0xFFFFFF, 1.0, 0x000000, 1.0),
    ("WindowBackground", 0xF2F2F7, 1.0, 0x1C1C1E, 1.0),
    ("Foreground", 0x000000, 1.0, 0xFFFFFF, 1.0),
    ("Separator", 0x3C3C43, 0.29, 0x545458, 0.6),
    ("Selection", 0x007AFF, 0.25, 0x0A84FF, 0.35),
    ("Placeholder", 0x3C3C43, 0.3, 0xEBEBF5, 0.3),
    ("Link", 0x007AFF, 1.0, 0x0984FF, 1.0),
    ("Fill", 0x787880, 0.2, 0x787880, 0.36),
];

/// Jetons de contrôle → le jeton qu'ils valent tant qu'aucun thème ne les fixe.
const CONTROLS: &[(&str, &str)] = &[
    ("ThemeAccent", "Tint"),
    ("ThemeBackground", "WindowBackground"),
    ("ThemeText", "Foreground"),
    ("ButtonAccent", "ThemeAccent"),
    ("ButtonText", "White"),
    ("ButtonBackground", "Fill"),
    ("ToggleAccent", "ThemeAccent"),
    ("ToggleBackground", "Fill"),
    ("SliderAccent", "ThemeAccent"),
    ("SliderBackground", "Fill"),
    ("SliderKnob", "White"),
    ("SliderText", "ThemeText"),
    ("ProgressAccent", "ThemeAccent"),
    ("ProgressBackground", "Fill"),
    ("GaugeAccent", "ThemeAccent"),
    ("GaugeBackground", "Fill"),
    ("MenuAccent", "ThemeAccent"),
    ("MenuBackground", "WindowBackground"),
    ("PickerAccent", "ThemeAccent"),
    ("PickerBackground", "Fill"),
];

/// Les 26 jetons de couleur de Veka (sans les jetons de contrôle).
pub fn default_tokens() -> impl Iterator<Item = Token> {
    DEFAULTS.iter().map(|d| Token::named(d.0))
}

/// Les jetons de contrôle.
pub fn control_tokens() -> impl Iterator<Item = Token> {
    CONTROLS.iter().map(|c| Token::named(c.0))
}

/// Le jeton qu'un jeton de contrôle vaut quand aucun thème ne le fixe.
pub fn control_target(token: &Token) -> Option<Token> {
    CONTROLS
        .iter()
        .find(|c| c.0 == token.name())
        .map(|c| Token::named(c.1))
}

/// Valeur de Veka d'un jeton de la palette (pas des jetons de contrôle).
pub fn default_color(token: &Token, scheme: ColorScheme) -> Option<Color> {
    DEFAULTS.iter().find(|d| d.0 == token.name()).map(|d| {
        let (hex, a) = match scheme {
            ColorScheme::Light => (d.1, d.2),
            ColorScheme::Dark => (d.3, d.4),
        };
        Color::hex(hex).with_alpha(a)
    })
}

/// Couleur d'un jeton inconnu : bien visible.
pub const UNKNOWN: Color = Color::rgb(1.0, 0.0, 1.0);

/// Réglage de couleur d'un sous-arbre (comme SwiftUI).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorAdjust {
    /// Ajoute la valeur à chaque composante (-1 à 1).
    Brightness(f32),
    /// Écarte du gris moyen (1 = inchangé).
    Contrast(f32),
    /// 0 = gris, 1 = inchangé, plus de 1 = plus saturé.
    Saturation(f32),
    /// Rotation de teinte, en degrés.
    HueRotation(f32),
    /// 0 = inchangé, 1 = entièrement gris.
    Grayscale(f32),
}

/// Matrice 4 × 5 appliquée au sRGB + alpha (comme `feColorMatrix`) : sortie = M · (r, g, b, a, 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorMatrix(pub [[f32; 5]; 4]);

impl Default for ColorMatrix {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl ColorMatrix {
    pub const IDENTITY: ColorMatrix = ColorMatrix([
        [1.0, 0.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0, 0.0],
    ]);

    pub fn is_identity(&self) -> bool {
        *self == Self::IDENTITY
    }

    pub fn new(adjust: ColorAdjust) -> Self {
        let mut m = Self::IDENTITY.0;
        let rgb = |m: &mut [[f32; 5]; 4], k: [[f32; 3]; 3]| {
            for r in 0..3 {
                m[r][..3].copy_from_slice(&k[r]);
            }
        };
        match adjust {
            ColorAdjust::Brightness(v) => {
                for row in m.iter_mut().take(3) {
                    row[4] = v;
                }
            }
            ColorAdjust::Contrast(c) => {
                for (i, row) in m.iter_mut().enumerate().take(3) {
                    row[i] = c;
                    row[4] = 0.5 * (1.0 - c);
                }
            }
            ColorAdjust::Saturation(s) => rgb(&mut m, saturation(s)),
            ColorAdjust::Grayscale(g) => rgb(&mut m, saturation(1.0 - g.clamp(0.0, 1.0))),
            ColorAdjust::HueRotation(deg) => {
                let (sn, cs) = deg.to_radians().sin_cos();
                rgb(
                    &mut m,
                    [
                        [
                            0.213 + cs * 0.787 - sn * 0.213,
                            0.715 - cs * 0.715 - sn * 0.715,
                            0.072 - cs * 0.072 + sn * 0.928,
                        ],
                        [
                            0.213 - cs * 0.213 + sn * 0.143,
                            0.715 + cs * 0.285 + sn * 0.140,
                            0.072 - cs * 0.072 - sn * 0.283,
                        ],
                        [
                            0.213 - cs * 0.213 - sn * 0.787,
                            0.715 - cs * 0.715 + sn * 0.715,
                            0.072 + cs * 0.928 + sn * 0.072,
                        ],
                    ],
                );
            }
        }
        ColorMatrix(m)
    }

    /// Applique `self` puis `outer`.
    pub fn then(&self, outer: &ColorMatrix) -> ColorMatrix {
        let (a, b) = (&self.0, &outer.0);
        let mut out = [[0.0; 5]; 4];
        for r in 0..4 {
            for c in 0..5 {
                let mut v = if c == 4 { b[r][4] } else { 0.0 };
                for k in 0..4 {
                    v += b[r][k] * a[k][c];
                }
                out[r][c] = v;
            }
        }
        ColorMatrix(out)
    }

    pub fn apply(&self, c: Color) -> Color {
        if self.is_identity() {
            return c;
        }
        let v = [c.r, c.g, c.b, c.a];
        let m = &self.0;
        let o = |r: usize| {
            (m[r][0] * v[0] + m[r][1] * v[1] + m[r][2] * v[2] + m[r][3] * v[3] + m[r][4]).clamp(0.0, 1.0)
        };
        Color::rgba(o(0), o(1), o(2), o(3))
    }
}

fn saturation(s: f32) -> [[f32; 3]; 3] {
    [
        [0.213 + 0.787 * s, 0.715 - 0.715 * s, 0.072 - 0.072 * s],
        [0.213 - 0.213 * s, 0.715 + 0.285 * s, 0.072 - 0.072 * s],
        [0.213 - 0.213 * s, 0.715 - 0.715 * s, 0.072 + 0.928 * s],
    ]
}

/// Ce qu'il faut pour résoudre les jetons : thème, mode, réglages, accent de l'OS.
#[derive(Clone, Copy, Debug)]
pub struct Palette<'a> {
    pub theme: Option<&'a Theme>,
    pub scheme: ColorScheme,
    pub matrix: &'a ColorMatrix,
    /// Couleur d'accent de l'OS, pour `Token::SystemAccent`.
    pub system_accent: Option<Color>,
}

/// Profondeur maximale des renvois entre jetons (protège des cycles).
const MAX_DEPTH: u32 = 16;

impl Palette<'_> {
    /// La peinture sans jeton, réglages de couleur appliqués : ce que reçoit le renderer.
    pub fn resolve(&self, paint: &Paint) -> Paint {
        let p = self.resolve_raw(paint, 0);
        if self.matrix.is_identity() {
            p
        } else {
            p.map_colors(|c| self.matrix.apply(c))
        }
    }

    /// Couleur unie d'une peinture (moyenne pour un dégradé), résolue.
    pub fn color(&self, paint: &Paint) -> Color {
        self.resolve(paint).average()
    }

    /// Valeur d'un jeton, résolue.
    pub fn token(&self, token: &Token, hierarchy: Hierarchy) -> Paint {
        self.resolve(&Paint::Token(token.clone(), hierarchy))
    }

    /// Mesure du thème, ou `default`.
    pub fn metric(&self, name: &str, default: f32) -> f32 {
        self.theme.and_then(|t| t.find_metric(name)).unwrap_or(default)
    }

    fn resolve_raw(&self, paint: &Paint, depth: u32) -> Paint {
        match paint {
            Paint::Solid(_) => paint.clone(),
            Paint::Token(t, h) => self.token_raw(t, depth).opacity(h.alpha()),
            _ => paint.map_inks(&mut |i| match i {
                Ink::Color(c) => Ink::Color(*c),
                // Un jeton dans un dégradé : sa couleur (moyenne s'il vaut lui-même un dégradé).
                Ink::Token(t, h) => Ink::Color(self.token_raw(t, depth).average().opacity(h.alpha())),
            }),
        }
    }

    fn token_raw(&self, token: &Token, depth: u32) -> Paint {
        if depth >= MAX_DEPTH {
            return Paint::Solid(UNKNOWN);
        }
        if let Some(c) = self.theme.and_then(|t| t.find_color(token)) {
            return self.resolve_raw(c.get(self.scheme), depth + 1);
        }
        if *token == Token::SystemAccent {
            return match self.system_accent {
                Some(c) => Paint::Solid(c),
                None => self.token_raw(&Token::Tint, depth + 1),
            };
        }
        if let Some(target) = control_target(token) {
            return self.token_raw(&target, depth + 1);
        }
        Paint::Solid(default_color(token, self.scheme).unwrap_or(UNKNOWN))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::LinearGradient;

    fn palette<'a>(theme: Option<&'a Theme>, scheme: ColorScheme, matrix: &'a ColorMatrix) -> Palette<'a> {
        Palette {
            theme,
            scheme,
            matrix,
            system_accent: None,
        }
    }

    #[test]
    fn valeurs_de_veka() {
        let id = ColorMatrix::IDENTITY;
        let light = palette(None, ColorScheme::Light, &id);
        let dark = palette(None, ColorScheme::Dark, &id);
        assert_eq!(light.color(&Token::Foreground.into()), Color::Black);
        assert_eq!(dark.color(&Token::Foreground.into()), Color::White);
        assert_eq!(light.color(&Token::Tint.into()), Color::hex(0x007AFF));
        assert_eq!(default_tokens().count(), 26);
    }

    #[test]
    fn jetons_de_controle_et_hierarchie() {
        let id = ColorMatrix::IDENTITY;
        let p = palette(None, ColorScheme::Light, &id);
        assert_eq!(p.color(&Token::ToggleAccent.into()), Color::hex(0x007AFF));
        let theme = Theme::new("t").color_all(Token::Tint, Color::Red);
        let p = palette(Some(&theme), ColorScheme::Light, &id);
        assert_eq!(p.color(&Token::ButtonAccent.into()), Color::Red);
        assert_eq!(
            p.color(&Token::Foreground.secondary().into()),
            Color::Black.with_alpha(0.6)
        );
    }

    #[test]
    fn parent_et_inconnu() {
        let id = ColorMatrix::IDENTITY;
        let base = Theme::new("base").color(Token::custom("Marque"), Color::Green, Color::Blue);
        let enfant = Theme::new("enfant").parent(base);
        let p = palette(Some(&enfant), ColorScheme::Dark, &id);
        assert_eq!(p.color(&Token::custom("Marque").into()), Color::Blue);
        assert_eq!(p.color(&Token::custom("Absent").into()), UNKNOWN);
    }

    #[test]
    fn cycle_protege() {
        let id = ColorMatrix::IDENTITY;
        let t = Theme::new("t")
            .color_all(Token::custom("A"), Token::custom("B"))
            .color_all(Token::custom("B"), Token::custom("A"));
        let p = palette(Some(&t), ColorScheme::Light, &id);
        assert_eq!(p.color(&Token::custom("A").into()), UNKNOWN);
    }

    #[test]
    fn jeton_dans_un_degrade() {
        let id = ColorMatrix::IDENTITY;
        let p = palette(None, ColorScheme::Light, &id);
        let g: Paint = LinearGradient::new([Ink::from(Token::Red), Ink::from(Token::Blue)]).into();
        let r = p.resolve(&g);
        assert!(r.is_resolved());
    }

    #[test]
    fn matrices() {
        let gris = ColorMatrix::new(ColorAdjust::Saturation(0.0)).apply(Color::Red);
        assert!((gris.r - gris.g).abs() < 1e-3 && (gris.g - gris.b).abs() < 1e-3);
        let c = Color::rgb(0.2, 0.5, 0.7);
        let tour = ColorMatrix::new(ColorAdjust::HueRotation(360.0)).apply(c);
        assert!((tour.r - c.r).abs() < 1e-3 && (tour.b - c.b).abs() < 1e-3);
        let b = ColorMatrix::new(ColorAdjust::Brightness(0.1));
        let deux = b.then(&b).apply(Color::Black);
        assert!((deux.r - 0.2).abs() < 1e-5);
        let id = ColorMatrix::IDENTITY;
        let m = ColorMatrix::new(ColorAdjust::Grayscale(1.0));
        let p = palette(None, ColorScheme::Light, &m);
        let r = p.color(&Token::Red.into());
        assert!((r.r - r.g).abs() < 1e-3);
        assert!(id.is_identity());
    }
}
