//! L'environnement : valeurs héritées par les enfants (police, couleurs…).

use std::sync::Arc;

use veka_colora::{ColorAdjust, ColorMatrix, Hierarchy, Paint, Palette, Theme, Token};
use veka_foundation::{Color, ColorScheme, WindowInfo};
use veka_text::{TextStyle, Weight};

/// Un thème partagé par un sous-arbre. Deux `ThemeRef` sont égaux s'ils désignent le même thème.
#[derive(Clone, Debug)]
pub struct ThemeRef(pub Arc<Theme>);

impl PartialEq for ThemeRef {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

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
    pub foreground: Paint,
    /// Couleur d'accent (boutons).
    pub accent: Paint,
    /// Fond de la fenêtre.
    pub background: Paint,
    /// Thème Colora du sous-arbre (`None` : valeurs de Veka).
    pub theme: Option<ThemeRef>,
    /// Mode clair ou sombre (donné par l'hôte, sinon clair ; `.color_scheme()` le force).
    pub color_scheme: ColorScheme,
    /// Réglages de couleur accumulés (luminosité, saturation…).
    pub color_matrix: ColorMatrix,
    /// Couleur d'accent de l'OS (`Token::SystemAccent`), donnée par l'hôte.
    pub system_accent: Option<Color>,
    /// L'état de la fenêtre (agrandie, active…), donné par l'hôte.
    pub window: WindowInfo,
    /// Le réglage « Réduire les animations » du système, donné par l'hôte.
    pub reduce_motion: bool,
    /// `.ignore_reduce_motion()` : ce sous-arbre s'anime quand même.
    pub ignore_reduce_motion: bool,
    /// Le réglage « Réduire la transparence » du système, donné par l'hôte : les effets d'arrière-plan
    /// deviennent des teintes unies.
    pub reduce_transparency: bool,
    /// Qualité des effets d'arrière-plan (réglée sur l'app).
    pub backdrop_quality: BackdropQuality,
}

/// Qualité des effets qui lisent l'arrière-plan (flou, verre).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BackdropQuality {
    /// Aucun effet : une teinte unie à la place.
    Off,
    /// Un seul niveau de flou, sans mélange ni grain.
    Low,
    /// Tout.
    #[default]
    High,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            font: Font::Body,
            font_weight: None,
            foreground: Token::Foreground.into(),
            accent: Token::ButtonAccent.into(),
            background: Token::Background.into(),
            theme: None,
            color_scheme: ColorScheme::Light,
            color_matrix: ColorMatrix::IDENTITY,
            system_accent: None,
            window: WindowInfo::default(),
            reduce_motion: false,
            ignore_reduce_motion: false,
            reduce_transparency: false,
            backdrop_quality: BackdropQuality::High,
        }
    }
}

impl Environment {
    /// Vrai si les mouvements doivent être évités ici : layout et déplacements sans animation,
    /// transitions réduites à leur fondu.
    pub fn reduces_motion(&self) -> bool {
        self.reduce_motion && !self.ignore_reduce_motion
    }

    /// De quoi résoudre les jetons de ce sous-arbre.
    pub fn palette(&self) -> Palette<'_> {
        Palette {
            theme: self.theme.as_ref().map(|t| &*t.0),
            scheme: self.color_scheme,
            matrix: &self.color_matrix,
            system_accent: self.system_accent,
        }
    }

    /// La peinture sans jeton, réglages appliqués : à donner à la liste de dessin.
    pub fn resolve(&self, paint: &Paint) -> Paint {
        self.palette().resolve(paint)
    }

    /// La peinture sans jeton, **sans** les réglages de couleur (ce qu'Anima interpole : les réglages
    /// sont appliqués ensuite, au dessin).
    pub fn resolve_unadjusted(&self, paint: &Paint) -> Paint {
        Palette {
            matrix: &ColorMatrix::IDENTITY,
            ..self.palette()
        }
        .resolve(paint)
    }

    /// Couleur unie (moyenne d'un dégradé), résolue.
    pub fn color(&self, paint: &Paint) -> Color {
        self.palette().color(paint)
    }

    /// Valeur d'un jeton.
    pub fn token(&self, token: &Token) -> Paint {
        self.palette().token(token, Hierarchy::Primary)
    }

    /// Une couleur fixe, réglages de couleur appliqués.
    pub fn adjusted(&self, color: Color) -> Color {
        self.color_matrix.apply(color)
    }

    /// Mesure du thème (`CornerRadius.Card`…), ou `default`.
    pub fn metric(&self, name: &str, default: f32) -> f32 {
        self.palette().metric(name, default)
    }

    /// Ajoute un réglage de couleur : il s'applique avant ceux des parents (le plus intérieur d'abord).
    pub fn adjust(&mut self, adjust: ColorAdjust) {
        self.color_matrix = ColorMatrix::new(adjust).then(&self.color_matrix);
    }

    pub fn text_style(&self) -> TextStyle {
        TextStyle {
            family: None,
            size: self.font.size(),
            weight: self.font_weight.unwrap_or(self.font.weight()),
            color: self.color(&self.foreground),
        }
    }
}
