//! # Veka Colora
//!
//! Tout ce qui colore une vue : espaces de couleur (Oklab), dégradés, jetons du thème, thèmes et
//! réglages de couleur. Sans aucun appel système.

mod paint;
mod sample;
mod space;
mod theme;

pub use paint::{
    AngularGradient, EllipticalGradient, Gradient, Hierarchy, Ink, LinearGradient, MeshGradient, MeshPoint,
    Paint, RadialGradient, Stop, Token, UnitPoint,
};
pub use sample::{mesh_image, mesh_key, ramp, ramp_key};
pub use space::{
    ColorSpace, OkLch, Oklab, from_space, lerp_in_space, linear_to_oklab, mix, oklab_to_linear, to_space,
};
pub use theme::{
    ColorAdjust, ColorMatrix, Palette, Theme, ThemeColor, UNKNOWN, control_target, control_tokens,
    default_color, default_tokens,
};
pub use veka_foundation::{Color, ColorScheme};
