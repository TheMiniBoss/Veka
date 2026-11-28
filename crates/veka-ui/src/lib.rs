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

mod anim;
mod element;
mod env;
mod layout;
pub mod macros;
mod primitive;
pub mod schema;
mod state;
mod surface;
mod timeline;
mod tree;
pub mod views;
pub mod window;

pub use element::{Action, Element, Key, View, component, component_tagged};
pub use env::{BackdropQuality, Environment, Font, ThemeRef};
pub use layout::{Alignment, Axis};
pub use primitive::{EventCx, EventResult, KeyEvent, LayoutCx, PaintCx, PointerEvent, Primitive, Proposal};
pub use state::{State, with_animation};
pub use surface::{InputEvent, PointerButton, Surface};
pub use timeline::TimelineState;
pub use veka_anima as anima;
pub use veka_anima::{Animation, Curve, Edge, Keyframe, Repeat, Timeline, Transition, TransitionEffect};
pub use veka_colora as colora;
pub use veka_colora::{
    AngularGradient, ColorAdjust, ColorScheme, ColorSpace, EllipticalGradient, Gradient, Hierarchy, Ink,
    LinearGradient, MeshGradient, MeshPoint, Paint, RadialGradient, Stop, Theme, Token, UnitPoint,
};
pub use veka_foundation::{
    Color, Insets, Point, Rect, RegionKind, Size, WindowAction, WindowControl, WindowInfo, WindowRegion,
};
pub use veka_graphics::{BlendMode, CornerRadii, DrawList, Shape};
pub use veka_text::Weight;

/// Tout ce qu'il faut pour écrire des vues : `use veka_ui::prelude::*;`.
pub mod prelude {
    pub use crate::views::*;
    pub use crate::window::{
        TitleBar, TitleBarStyle, TitleBarWith, WindowActionExt, WindowButton, WindowButtonWith,
        perform_window_action,
    };
    pub use crate::{
        Alignment, AngularGradient, Axis, BackdropQuality, BlendMode, Color, ColorScheme, ColorSpace,
        CornerRadii, Element, EllipticalGradient, Environment, Font, Gradient, Hierarchy, Ink, Insets, Key,
        LinearGradient, MeshGradient, Paint, Point, RadialGradient, Rect, Shape, Size, State, Stop, Surface,
        Theme, Token, UnitPoint, View, Weight, WindowAction, WindowControl, WindowInfo, component,
    };
    pub use crate::{Animation, Curve, Edge, Keyframe, Timeline, TimelineState, Transition, with_animation};
    pub use crate::{HStack, VStack, ZStack};
    pub use veka_foundation::{Key as KeyEquivalent, Modifiers};
}
