//! # Veka Render
//!
//! Le contrat d'un renderer : dessiner une [`DrawList`]. Le premier renderer est `veka-render-wgpu` ;
//! Unreal et Unity auront le leur, qui lira la même liste.

use std::fmt;

pub use veka_graphics::DrawList;

/// Taille de la cible en pixels physiques et échelle d'affichage (points logiques → pixels).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameInfo {
    pub width: u32,
    pub height: u32,
    pub scale: f32,
}

/// Erreur de rendu.
#[derive(Debug)]
pub enum RenderError {
    /// La cible a été perdue (fenêtre fermée, surface à reconfigurer) : l'image est sautée.
    Lost,
    /// Pas de GPU utilisable.
    NoAdapter(String),
    Other(String),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::Lost => f.write_str("cible de rendu perdue"),
            RenderError::NoAdapter(e) => write!(f, "aucun GPU utilisable : {e}"),
            RenderError::Other(e) => f.write_str(e),
        }
    }
}

impl std::error::Error for RenderError {}

/// Un renderer : dessine une liste de dessin dans sa cible.
pub trait Renderer {
    /// Adapte la cible à une nouvelle taille.
    fn resize(&mut self, frame: FrameInfo);

    /// Dessine `list` et présente l'image.
    fn render(&mut self, list: &DrawList) -> Result<(), RenderError>;
}
