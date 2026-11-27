//! # Veka Graphics
//!
//! Formes analytiques (SDF), peintures et **liste de dessin**.
//!
//! La liste de dessin est ce que le cœur de Veka produit à chaque image. Elle ne contient que des
//! données : n'importe quel renderer (wgpu, Unreal, Unity…) peut la dessiner.

mod draw_list;
mod paint;
mod shape;

pub use draw_list::{
    BackdropEffect, DrawCommand, DrawList, FontId, FontRef, Glyph, GlyphRun, Segment,
};
pub use paint::Paint;
pub use shape::{CornerRadii, Shape};
pub use veka_foundation as foundation;
