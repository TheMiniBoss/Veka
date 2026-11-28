//! # Veka Foundation
//!
//! Types de base partagés par tous les modules de Veka : géométrie, couleurs, temps, notions de fenêtre.
//!
//! Ce module ne dépend d'aucune API système : il compile partout, y compris en WebAssembly.

mod color;
mod geometry;
mod key;
mod time;
mod window;

pub use color::{Color, ColorScheme, linear_to_srgb, srgb_to_linear};
pub use geometry::{Insets, Point, Rect, Size, Transform2D, Vector};
pub use key::{Key, Modifiers, ScrollPhase};
pub use time::Instant;
pub use window::{RegionKind, WindowAction, WindowControl, WindowInfo, WindowRegion};
