//! # Veka Foundation
//!
//! Types de base partagés par tous les modules de Veka : géométrie, couleurs, temps.
//!
//! Ce module ne dépend d'aucune API système : il compile partout, y compris en WebAssembly.

mod color;
mod geometry;
mod time;

pub use color::{Color, linear_to_srgb, srgb_to_linear};
pub use geometry::{Insets, Point, Rect, Size, Transform2D, Vector};
pub use time::Instant;
