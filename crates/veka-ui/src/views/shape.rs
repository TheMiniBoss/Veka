use veka_colora::Paint;
use veka_foundation::{Rect, Size};
use veka_graphics::Shape;

use crate::element::Element;
use crate::primitive::{LayoutCx, PaintCx, Primitive, Proposal};

/// Le primitive des formes : remplissage (par défaut la couleur de premier plan) et trait optionnel.
#[derive(Clone, Debug, PartialEq)]
pub struct ShapePrim {
    pub shape: Shape,
    /// `None` : couleur de premier plan de l'environnement.
    pub fill: Option<Paint>,
    /// Trait seul (sans remplissage) : peinture et épaisseur.
    pub stroke: Option<(Paint, f32)>,
}

impl ShapePrim {
    pub fn element(shape: Shape) -> Element {
        Element::new(
            ShapePrim {
                shape,
                fill: None,
                stroke: None,
            },
            Vec::new(),
        )
    }

    pub fn name_of(shape: &Shape) -> &'static str {
        match shape {
            Shape::Rectangle => "Rectangle",
            Shape::RoundedRectangle(_) => "RoundedRectangle",
            Shape::Capsule => "Capsule",
            Shape::Circle => "Circle",
            Shape::Ellipse => "Ellipse",
        }
    }
}

/// Taille idéale d'une forme quand rien n'est proposé (comme SwiftUI).
const IDEAL: f32 = 10.0;

impl Primitive for ShapePrim {
    fn name(&self) -> &'static str {
        Self::name_of(&self.shape)
    }

    fn size_that_fits(&self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        // Une forme prend toute la place proposée.
        Size::new(p.width.unwrap_or(IDEAL), p.height.unwrap_or(IDEAL))
    }

    fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        match &self.stroke {
            Some((paint, width)) => {
                let p = cx.env().resolve(paint);
                cx.list().stroke(self.shape, rect, p, *width);
            }
            None => {
                let env = cx.env();
                let p = env.resolve(self.fill.as_ref().unwrap_or(&env.foreground));
                cx.list().fill(self.shape, rect, p);
            }
        }
    }
}
