//! Les vues de base, avec les noms de SwiftUI : `Text`, `Rectangle`, `Capsule`, `Button`…

#![allow(non_snake_case)]

mod anima;
mod button;
mod modifiers;
mod shape;
mod text;

pub use anima::{
    AnimationEndPrim, AnimationPrim, MatchedGeometryPrim, OffsetPrim, OpacityPrim, RotationPrim,
    ScaleEffectPrim, TransitionPrim, value_key,
};
pub use button::{ButtonPrim, ButtonState};
pub use modifiers::{Background, EnvModifier, Length, TapGesture};
pub use shape::ShapePrim;
pub use text::{TextPrim, TextState};

use veka_graphics::{CornerRadii, Shape};

use crate::element::{Action, Element, View};
use crate::layout::{Alignment, Axis, DEFAULT_SPACER_MIN, DEFAULT_SPACING, Spacer as SpacerPrim, Stack};

/// Du texte.
pub fn Text(content: impl Into<String>) -> Element {
    Element::new(
        TextPrim {
            content: content.into(),
        },
        Vec::new(),
    )
}

/// Un rectangle qui remplit la place proposée.
pub fn Rectangle() -> Element {
    ShapePrim::element(Shape::Rectangle)
}

/// Un rectangle aux coins arrondis de rayon `radius`.
pub fn RoundedRectangle(radius: impl Length) -> Element {
    ShapePrim::element(Shape::RoundedRectangle(CornerRadii::all(radius.points())))
}

/// Un rectangle aux coins arrondis, un rayon par coin.
pub fn UnevenRoundedRectangle(radii: CornerRadii) -> Element {
    ShapePrim::element(Shape::RoundedRectangle(radii))
}

/// Un rectangle aux extrémités entièrement arrondies.
pub fn Capsule() -> Element {
    ShapePrim::element(Shape::Capsule)
}

/// Un cercle centré (diamètre = plus petit côté).
pub fn Circle() -> Element {
    ShapePrim::element(Shape::Circle)
}

/// Une ellipse qui remplit la place proposée.
pub fn Ellipse() -> Element {
    ShapePrim::element(Shape::Ellipse)
}

/// Un espace flexible qui pousse ses voisins dans une pile.
pub fn Spacer() -> Element {
    Element::new(
        SpacerPrim {
            min_length: DEFAULT_SPACER_MIN,
        },
        Vec::new(),
    )
}

/// Un bouton : un libellé et une action lancée au clic.
pub fn Button(title: impl Into<String>, action: impl FnMut() + 'static) -> Element {
    ButtonWith(Text(title), action)
}

/// Un bouton dont le libellé est une vue quelconque.
pub fn ButtonWith(label: impl View, action: impl FnMut() + 'static) -> Element {
    Element::new(
        ButtonPrim {
            action: Action::new(action),
        },
        vec![label.into_element()],
    )
}

/// Une pile (utilisée par les macros `VStack!`, `HStack!`, `ZStack!`).
pub fn stack(axis: Axis, spacing: Option<f32>, children: Vec<Element>) -> Element {
    Element::new(
        Stack {
            axis,
            spacing: spacing.unwrap_or(DEFAULT_SPACING),
            alignment: Alignment::Center,
        },
        children,
    )
}
