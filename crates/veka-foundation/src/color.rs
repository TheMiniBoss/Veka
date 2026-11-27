//! Couleurs. Stockées en sRGB (comme on les écrit), converties en linéaire pour le rendu.

/// Une couleur sRGB avec alpha (non prémultiplié), composantes entre 0 et 1.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Default for Color {
    fn default() -> Self {
        Self::Black
    }
}

#[allow(non_upper_case_globals)]
impl Color {
    pub const Clear: Color = Color::rgba(0.0, 0.0, 0.0, 0.0);
    pub const Black: Color = Color::rgb(0.0, 0.0, 0.0);
    pub const White: Color = Color::rgb(1.0, 1.0, 1.0);
    pub const Gray: Color = Color::rgb(0.557, 0.557, 0.576);
    pub const Red: Color = Color::rgb(1.0, 0.231, 0.188);
    pub const Orange: Color = Color::rgb(1.0, 0.584, 0.0);
    pub const Yellow: Color = Color::rgb(1.0, 0.8, 0.0);
    pub const Green: Color = Color::rgb(0.204, 0.78, 0.349);
    pub const Teal: Color = Color::rgb(0.188, 0.69, 0.78);
    pub const Blue: Color = Color::rgb(0.0, 0.478, 1.0);
    pub const Indigo: Color = Color::rgb(0.345, 0.337, 0.839);
    pub const Purple: Color = Color::rgb(0.686, 0.322, 0.871);
    pub const Pink: Color = Color::rgb(1.0, 0.176, 0.333);

    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Depuis `0xRRGGBB`.
    pub fn hex(rgb: u32) -> Self {
        let c = |shift: u32| ((rgb >> shift) & 0xFF) as f32 / 255.0;
        Self::rgb(c(16), c(8), c(0))
    }

    pub fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    pub fn opacity(self, factor: f32) -> Self {
        Self {
            a: self.a * factor,
            ..self
        }
    }

    /// Mélange linéaire (en sRGB, suffisant pour les états survol / appui).
    pub fn mix(self, other: Color, t: f32) -> Self {
        let l = |a: f32, b: f32| a + (b - a) * t;
        Self::rgba(
            l(self.r, other.r),
            l(self.g, other.g),
            l(self.b, other.b),
            l(self.a, other.a),
        )
    }

    /// Composantes linéaires, alpha prémultiplié : ce qu'attend un renderer.
    pub fn to_linear_premultiplied(self) -> [f32; 4] {
        let a = self.a.clamp(0.0, 1.0);
        [
            srgb_to_linear(self.r) * a,
            srgb_to_linear(self.g) * a,
            srgb_to_linear(self.b) * a,
            a,
        ]
    }
}

pub fn srgb_to_linear(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub fn linear_to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex() {
        assert_eq!(Color::hex(0xFF8000), Color::rgb(1.0, 128.0 / 255.0, 0.0));
    }

    #[test]
    fn srgb_round_trip() {
        for i in 0..=20 {
            let v = i as f32 / 20.0;
            assert!((linear_to_srgb(srgb_to_linear(v)) - v).abs() < 1e-5);
        }
    }

    #[test]
    fn premultiplied() {
        let [r, _, _, a] = Color::rgba(1.0, 0.0, 0.0, 0.5).to_linear_premultiplied();
        assert_eq!(a, 0.5);
        assert!((r - 0.5).abs() < 1e-6);
    }
}
