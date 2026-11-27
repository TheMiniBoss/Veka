//! Les modifiers. Chacun enveloppe l'élément dans un primitive (`padding`, `frame`…) ou modifie
//! directement le primitive quand c'est le sien (`fill` sur une forme).

use veka_foundation::{Color, Insets, Rect};
use veka_graphics::Shape;

use crate::element::{Action, Element};
use crate::env::{Environment, Font};
use crate::layout::{Alignment, Frame, Padding};
use crate::primitive::{EventCx, EventResult, PaintCx, PointerEvent, Primitive};
use crate::views::ShapePrim;
use veka_text::Weight;

/// Une longueur en points : accepte `24`, `24.0`, `24u32`…
pub trait Length {
    fn points(self) -> f32;
}

macro_rules! impl_length {
    ($($t:ty),*) => { $(impl Length for $t { fn points(self) -> f32 { self as f32 } })* };
}
impl_length!(i32, i64, u32, u64, usize, f32, f64);

/// Modifier d'environnement (police, couleur de premier plan…).
#[derive(Clone, Debug, PartialEq)]
pub enum EnvModifier {
    Font(Font),
    FontWeight(Weight),
    Foreground(Color),
    Accent(Color),
}

impl EnvModifier {
    pub fn schema_name(&self) -> &'static str {
        match self {
            EnvModifier::Font(_) => "font",
            EnvModifier::FontWeight(_) => "fontWeight",
            EnvModifier::Foreground(_) => "foregroundColor",
            EnvModifier::Accent(_) => "tint",
        }
    }
}

impl Primitive for EnvModifier {
    fn name(&self) -> &'static str {
        self.schema_name()
    }

    fn update_env(&self, env: &mut Environment) -> bool {
        match *self {
            EnvModifier::Font(f) => {
                env.font = f;
                env.font_weight = None;
            }
            EnvModifier::FontWeight(w) => env.font_weight = Some(w),
            EnvModifier::Foreground(c) => env.foreground = c,
            EnvModifier::Accent(c) => env.accent = c,
        }
        true
    }
}

/// Fond : une forme remplie derrière le contenu, à sa taille.
#[derive(Clone, Debug, PartialEq)]
pub struct Background {
    pub color: Color,
    pub shape: Shape,
}

impl Primitive for Background {
    fn name(&self) -> &'static str {
        "background"
    }

    fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        cx.list().fill(self.shape, rect, self.color);
        cx.paint_children();
    }
}

/// Action au clic sur n'importe quelle vue (`.on_tap`).
#[derive(Clone, Debug)]
pub struct TapGesture {
    pub action: Action,
}

#[derive(Default)]
struct TapState {
    tracking: bool,
}

impl Primitive for TapGesture {
    fn name(&self) -> &'static str {
        "onTapGesture"
    }

    fn create_state(&self) -> Box<dyn std::any::Any> {
        Box::<TapState>::default()
    }

    fn hit_shape(&self) -> Option<Shape> {
        Some(Shape::Rectangle)
    }

    fn pointer(&self, cx: &mut EventCx<'_>, event: PointerEvent) -> EventResult {
        let st = cx.state::<TapState>();
        match event {
            PointerEvent::Down { .. } => {
                st.tracking = true;
                EventResult {
                    handled: true,
                    ..Default::default()
                }
            }
            PointerEvent::Up { inside, .. } => {
                let fire = st.tracking && inside;
                st.tracking = false;
                EventResult {
                    handled: true,
                    action: fire.then(|| self.action.clone()),
                    ..Default::default()
                }
            }
            _ => EventResult {
                handled: true,
                ..Default::default()
            },
        }
    }
}

impl Element {
    /// Marges égales sur les quatre côtés.
    pub fn padding(self, all: impl Length) -> Element {
        self.padding_insets(Insets::all(all.points()))
    }

    /// Marges horizontales et verticales.
    pub fn padding_hv(self, horizontal: impl Length, vertical: impl Length) -> Element {
        let (h, v) = (horizontal.points(), vertical.points());
        self.padding_insets(Insets::new(v, h, v, h))
    }

    pub fn padding_insets(self, insets: Insets) -> Element {
        self.wrap(Padding { insets })
    }

    /// Taille imposée (largeur × hauteur).
    pub fn frame(self, width: impl Length, height: impl Length) -> Element {
        self.wrap(Frame {
            width: Some(width.points()),
            height: Some(height.points()),
            alignment: Alignment::Center,
        })
    }

    pub fn frame_width(self, width: impl Length) -> Element {
        self.wrap(Frame {
            width: Some(width.points()),
            height: None,
            alignment: Alignment::Center,
        })
    }

    pub fn frame_height(self, height: impl Length) -> Element {
        self.wrap(Frame {
            width: None,
            height: Some(height.points()),
            alignment: Alignment::Center,
        })
    }

    /// Remplit une forme (sur une autre vue : couleur de premier plan).
    pub fn fill(self, color: Color) -> Element {
        match self.map_primitive(|s: ShapePrim| ShapePrim {
            fill: Some(color),
            stroke: None,
            ..s
        }) {
            Ok(e) => e,
            Err(e) => e.foreground(color),
        }
    }

    /// Dessine seulement le contour d'une forme.
    pub fn stroke(self, color: Color, width: impl Length) -> Element {
        let w = width.points();
        match self.map_primitive(|s: ShapePrim| ShapePrim {
            stroke: Some((color, w)),
            ..s
        }) {
            Ok(e) => e,
            Err(e) => e,
        }
    }

    /// Couleur du texte et des formes du sous-arbre.
    pub fn foreground(self, color: Color) -> Element {
        self.wrap(EnvModifier::Foreground(color))
    }

    /// Couleur d'accent (fond des boutons…).
    pub fn tint(self, color: Color) -> Element {
        self.wrap(EnvModifier::Accent(color))
    }

    pub fn font(self, font: Font) -> Element {
        self.wrap(EnvModifier::Font(font))
    }

    pub fn font_weight(self, weight: Weight) -> Element {
        self.wrap(EnvModifier::FontWeight(weight))
    }

    /// Fond rectangulaire de la couleur donnée.
    pub fn background(self, color: Color) -> Element {
        self.wrap(Background {
            color,
            shape: Shape::Rectangle,
        })
    }

    /// Fond de la forme donnée (ex. `.background_shape(Color::Blue, Shape::Capsule)`).
    pub fn background_shape(self, color: Color, shape: Shape) -> Element {
        self.wrap(Background { color, shape })
    }

    /// Action au clic.
    pub fn on_tap(self, action: impl FnMut() + 'static) -> Element {
        self.wrap(TapGesture {
            action: Action::new(action),
        })
    }
}
