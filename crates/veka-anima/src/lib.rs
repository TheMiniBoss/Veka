//! # Veka Anima
//!
//! Tout ce qui bouge : courbes (*ease*, Bézier cubique, ressorts analytiques), la spec [`Animation`],
//! l'[`Animator`] qui fait avancer une liste de valeurs et les [`Transition`] d'entrée et de sortie.
//! Le temps est toujours donné par l'appelant : aucun appel système.

mod animation;
mod animator;
mod curve;
mod data;
mod spring;
mod transition;

pub use animation::{Animation, Curve, Repeat};
pub use animator::{Animator, Sample};
pub use curve::{cubic_bezier, ease};
pub use data::{AnimatableData, AnimatableReader};
pub use spring::{SpringParams, spring_at};
pub use transition::{Edge, Transition, TransitionEffect, TransitionFrame};

/// Les secondes écoulées depuis `start` (le temps des animations).
pub fn seconds(now: veka_foundation::Instant) -> f64 {
    now.0 as f64 * 1e-9
}
