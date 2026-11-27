//! L'environnement : valeurs héritées par les enfants (police, couleurs…).

use veka_foundation::Color;
use veka_text::{TextStyle, Weight};

/// Styles de texte sémantiques.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Font {
    LargeTitle,
    Title,
    Title2,
    Title3,
    Headline,
    Body,
    Callout,
    Subheadline,
    Footnote,
    Caption,
    Caption2,
    /// Taille et graisse libres.
    System {
        size: f32,
        weight: Weight,
    },
}

impl Font {
    pub fn size(&self) -> f32 {
        match *self {
            Font::LargeTitle => 34.0,
            Font::Title => 28.0,
            Font::Title2 => 22.0,
            Font::Title3 => 20.0,
            Font::Headline | Font::Body => 17.0,
            Font::Callout => 16.0,
            Font::Subheadline => 15.0,
            Font::Footnote => 13.0,
            Font::Caption => 12.0,
            Font::Caption2 => 11.0,
            Font::System { size, .. } => size,
        }
    }

    pub fn weight(&self) -> Weight {
        match *self {
            Font::Headline => Weight::SemiBold,
            Font::System { weight, .. } => weight,
            _ => Weight::Regular,
        }
    }

    /// Nom dans le schéma (`"title"`, `"body"`…).
    pub fn from_name(name: &str) -> Option<Font> {
        Some(match name {
            "largeTitle" => Font::LargeTitle,
            "title" => Font::Title,
            "title2" => Font::Title2,
            "title3" => Font::Title3,
            "headline" => Font::Headline,
            "body" => Font::Body,
            "callout" => Font::Callout,
            "subheadline" => Font::Subheadline,
            "footnote" => Font::Footnote,
            "caption" => Font::Caption,
            "caption2" => Font::Caption2,
            _ => return None,
        })
    }
}

/// Valeurs héritées.
#[derive(Clone, Debug, PartialEq)]
pub struct Environment {
    pub font: Font,
    /// Graisse imposée (`.font_weight()`), sinon celle de la police.
    pub font_weight: Option<Weight>,
    /// Couleur du texte et des formes sans remplissage explicite.
    pub foreground: Color,
    /// Couleur d'accent (boutons).
    pub accent: Color,
    /// Fond de la fenêtre.
    pub background: Color,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            font: Font::Body,
            font_weight: None,
            foreground: Color::Black,
            accent: Color::Blue,
            background: Color::White,
        }
    }
}

impl Environment {
    pub fn text_style(&self) -> TextStyle {
        TextStyle {
            family: None,
            size: self.font.size(),
            weight: self.font_weight.unwrap_or(self.font.weight()),
            color: self.foreground,
        }
    }
}
