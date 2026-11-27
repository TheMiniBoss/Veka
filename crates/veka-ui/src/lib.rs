//! # Veka UI
//!
//! Vues déclaratives, état, layout, contrôles. Une vue est une fonction qui décrit l'interface ;
//! Veka la ré-évalue quand son état change et ne met à jour que ce qui a changé.
//!
//! ```
//! use veka_ui::prelude::*;
//!
//! fn compteur() -> impl View {
//!     let mut clics = State::new(0);
//!     VStack![spacing: 16;
//!         Text("Bonjour Veka").font(Font::Title),
//!         Button(format!("Clics : {clics}"), move || clics += 1),
//!     ]
//!     .padding(24)
//! }
//!
//! let mut surface = Surface::new(component(compteur));
//! surface.resize(Size::new(480.0, 320.0), 1.0);
//! let list = surface.frame();
//! assert!(!list.commands.is_empty());
//! ```
//!
//! Ce module ne dépend d'aucune API système : la fenêtre, la souris et le dessin passent par
//! [`Surface`], que l'hôte (app native, moteur de jeu…) alimente.

mod element;
mod env;
mod layout;
pub mod macros;
mod primitive;
pub mod schema;
mod state;
mod surface;
mod tree;
pub mod views;

pub use element::{Action, Element, Key, View, component};
pub use env::{Environment, Font};
pub use layout::{Alignment, Axis};
pub use primitive::{EventCx, EventResult, LayoutCx, PaintCx, PointerEvent, Primitive, Proposal};
pub use state::State;
pub use surface::{InputEvent, PointerButton, Surface};
pub use veka_foundation::{Color, Insets, Point, Rect, Size};
pub use veka_graphics::{CornerRadii, DrawList, Shape};
pub use veka_text::Weight;

/// Tout ce qu'il faut pour écrire des vues : `use veka_ui::prelude::*;`.
pub mod prelude {
    pub use crate::views::*;
    pub use crate::{
        Alignment, Axis, Color, Element, Environment, Font, Insets, Key, Point, Rect, Shape, Size, State,
        Surface, View, Weight, component,
    };
    pub use crate::{HStack, VStack, ZStack};
}
