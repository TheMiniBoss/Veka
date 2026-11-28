//! Les modifiers. Chacun enveloppe l'élément dans un primitive (`padding`, `frame`…) ou modifie
//! directement le primitive quand c'est le sien (`fill` sur une forme).

use std::sync::Arc;

use veka_colora::{ColorAdjust, Paint, Theme};
use veka_foundation::{ColorScheme, Insets, Rect};
use veka_graphics::Shape;

use crate::element::{Action, Element};
use crate::env::{Environment, Font, ThemeRef};
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
    Foreground(Paint),
    Accent(Paint),
    Theme(ThemeRef),
    ColorScheme(ColorScheme),
    ColorAdjust(ColorAdjust),
    /// `.ignore_reduce_motion()`.
    IgnoreReduceMotion,
}

impl EnvModifier {
    pub fn schema_name(&self) -> &'static str {
        match self {
            EnvModifier::Font(_) => "font",
            EnvModifier::FontWeight(_) => "fontWeight",
            EnvModifier::Foreground(_) => "foregroundColor",
            EnvModifier::Accent(_) => "tint",
            EnvModifier::Theme(_) => "theme",
            EnvModifier::ColorScheme(_) => "colorScheme",
            EnvModifier::ColorAdjust(a) => match a {
                ColorAdjust::Brightness(_) => "brightness",
                ColorAdjust::Contrast(_) => "contrast",
                ColorAdjust::Saturation(_) => "saturation",
                ColorAdjust::HueRotation(_) => "hueRotation",
                ColorAdjust::Grayscale(_) => "grayscale",
            },
            EnvModifier::IgnoreReduceMotion => "ignoreReduceMotion",
        }
    }
}

impl Primitive for EnvModifier {
    fn name(&self) -> &'static str {
        self.schema_name()
    }

    fn update_env(&self, env: &mut Environment) -> bool {
        match self {
            EnvModifier::Font(f) => {
                env.font = *f;
                env.font_weight = None;
            }
            EnvModifier::FontWeight(w) => env.font_weight = Some(*w),
            EnvModifier::Foreground(p) => env.foreground = p.clone(),
            EnvModifier::Accent(p) => env.accent = p.clone(),
            EnvModifier::Theme(t) => env.theme = Some(t.clone()),
            EnvModifier::ColorScheme(s) => env.color_scheme = *s,
            EnvModifier::ColorAdjust(a) => env.adjust(*a),
            EnvModifier::IgnoreReduceMotion => env.ignore_reduce_motion = true,
        }
        true
    }

    fn animatable(&self, env: &Environment) -> Option<veka_anima::AnimatableData> {
        let mut d = veka_anima::AnimatableData::new();
        match self {
            EnvModifier::Foreground(p) | EnvModifier::Accent(p) => {
                d.mark(if matches!(self, EnvModifier::Foreground(_)) {
                    1
                } else {
                    2
                });
                super::anima::push_paint(&mut d, &env.resolve_unadjusted(p));
            }
            EnvModifier::ColorAdjust(a) => {
                let (tag, v) = adjust_parts(*a);
                d.mark(10 + tag).push(v);
            }
            _ => return None,
        }
        Some(d)
    }

    fn with_animatable(
        &self,
        env: &Environment,
        data: &veka_anima::AnimatableData,
    ) -> Option<std::rc::Rc<dyn Primitive>> {
        let mut r = data.reader();
        let m = match self {
            EnvModifier::Foreground(p) => {
                EnvModifier::Foreground(super::anima::read_paint(&env.resolve_unadjusted(p), &mut r))
            }
            EnvModifier::Accent(p) => {
                EnvModifier::Accent(super::anima::read_paint(&env.resolve_unadjusted(p), &mut r))
            }
            EnvModifier::ColorAdjust(a) => {
                let v = r.f32();
                EnvModifier::ColorAdjust(match a {
                    ColorAdjust::Brightness(_) => ColorAdjust::Brightness(v),
                    ColorAdjust::Contrast(_) => ColorAdjust::Contrast(v),
                    ColorAdjust::Saturation(_) => ColorAdjust::Saturation(v),
                    ColorAdjust::HueRotation(_) => ColorAdjust::HueRotation(v),
                    ColorAdjust::Grayscale(_) => ColorAdjust::Grayscale(v),
                })
            }
            _ => return None,
        };
        Some(std::rc::Rc::new(m))
    }
}

fn adjust_parts(a: ColorAdjust) -> (u64, f32) {
    match a {
        ColorAdjust::Brightness(v) => (0, v),
        ColorAdjust::Contrast(v) => (1, v),
        ColorAdjust::Saturation(v) => (2, v),
        ColorAdjust::HueRotation(v) => (3, v),
        ColorAdjust::Grayscale(v) => (4, v),
    }
}

/// Fond : une forme remplie derrière le contenu, à sa taille.
#[derive(Clone, Debug, PartialEq)]
pub struct Background {
    pub paint: Paint,
    pub shape: Shape,
}

impl Primitive for Background {
    fn name(&self) -> &'static str {
        "background"
    }

    fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        let paint = cx.env().resolve(&self.paint);
        cx.list().fill(self.shape, rect, paint);
        cx.paint_children();
    }

    fn animatable(&self, env: &Environment) -> Option<veka_anima::AnimatableData> {
        let mut d = veka_anima::AnimatableData::new();
        super::anima::push_shape(&mut d, &self.shape);
        super::anima::push_paint(&mut d, &env.resolve_unadjusted(&self.paint));
        Some(d)
    }

    fn with_animatable(
        &self,
        env: &Environment,
        data: &veka_anima::AnimatableData,
    ) -> Option<std::rc::Rc<dyn Primitive>> {
        let mut r = data.reader();
        let shape = super::anima::read_shape(self.shape, &mut r);
        let paint = super::anima::read_paint(&env.resolve_unadjusted(&self.paint), &mut r);
        Some(std::rc::Rc::new(Background { paint, shape }))
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

    fn primary_action(&self) -> Option<Action> {
        Some(self.action.clone())
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

    /// Remplit une forme : couleur, jeton ou dégradé (sur une autre vue : premier plan).
    pub fn fill(self, paint: impl Into<Paint>) -> Element {
        let paint = paint.into();
        let p = paint.clone();
        match self.map_primitive(move |s: ShapePrim| ShapePrim {
            fill: Some(p),
            stroke: None,
            ..s
        }) {
            Ok(e) => e,
            Err(e) => e.foreground(paint),
        }
    }

    /// Dessine seulement le contour d'une forme.
    pub fn stroke(self, paint: impl Into<Paint>, width: impl Length) -> Element {
        let (paint, w) = (paint.into(), width.points());
        match self.map_primitive(move |s: ShapePrim| ShapePrim {
            stroke: Some((paint, w)),
            ..s
        }) {
            Ok(e) => e,
            Err(e) => e,
        }
    }

    /// Couleur du texte et des formes du sous-arbre.
    pub fn foreground(self, paint: impl Into<Paint>) -> Element {
        self.wrap(EnvModifier::Foreground(paint.into()))
    }

    /// Couleur d'accent (fond des boutons…).
    pub fn tint(self, paint: impl Into<Paint>) -> Element {
        self.wrap(EnvModifier::Accent(paint.into()))
    }

    pub fn font(self, font: Font) -> Element {
        self.wrap(EnvModifier::Font(font))
    }

    pub fn font_weight(self, weight: Weight) -> Element {
        self.wrap(EnvModifier::FontWeight(weight))
    }

    /// Fond rectangulaire.
    pub fn background(self, paint: impl Into<Paint>) -> Element {
        self.wrap(Background {
            paint: paint.into(),
            shape: Shape::Rectangle,
        })
    }

    /// Fond de la forme donnée (ex. `.background_shape(Color::Blue, Shape::Capsule)`).
    pub fn background_shape(self, paint: impl Into<Paint>, shape: Shape) -> Element {
        self.wrap(Background {
            paint: paint.into(),
            shape,
        })
    }

    /// Thème Colora du sous-arbre.
    pub fn theme(self, theme: impl Into<Arc<Theme>>) -> Element {
        self.wrap(EnvModifier::Theme(ThemeRef(theme.into())))
    }

    /// Force le mode clair ou sombre du sous-arbre.
    pub fn color_scheme(self, scheme: ColorScheme) -> Element {
        self.wrap(EnvModifier::ColorScheme(scheme))
    }

    /// Le mode clair ou sombre de **toute la fenêtre**, barre de titre du système comprise (comme
    /// `preferredColorScheme` en SwiftUI). `None` : celui du système. La première vue qui le demande
    /// (en partant de la racine) l'emporte.
    pub fn preferred_color_scheme(self, scheme: Option<ColorScheme>) -> Element {
        self.wrap(PreferredColorScheme { scheme })
    }

    /// Ajoute `amount` (-1 à 1) à chaque composante.
    pub fn brightness(self, amount: f32) -> Element {
        self.wrap(EnvModifier::ColorAdjust(ColorAdjust::Brightness(amount)))
    }

    /// 1 = inchangé.
    pub fn contrast(self, amount: f32) -> Element {
        self.wrap(EnvModifier::ColorAdjust(ColorAdjust::Contrast(amount)))
    }

    /// 0 = gris, 1 = inchangé.
    pub fn saturation(self, amount: f32) -> Element {
        self.wrap(EnvModifier::ColorAdjust(ColorAdjust::Saturation(amount)))
    }

    /// Rotation de teinte, en degrés.
    pub fn hue_rotation(self, degrees: f32) -> Element {
        self.wrap(EnvModifier::ColorAdjust(ColorAdjust::HueRotation(degrees)))
    }

    /// 0 = inchangé, 1 = gris.
    pub fn grayscale(self, amount: f32) -> Element {
        self.wrap(EnvModifier::ColorAdjust(ColorAdjust::Grayscale(amount)))
    }

    /// Action au clic.
    pub fn on_tap(self, action: impl FnMut() + 'static) -> Element {
        self.wrap(TapGesture {
            action: Action::new(action),
        })
    }
}

/// Mode clair ou sombre demandé pour toute la fenêtre (`.preferred_color_scheme`).
#[derive(Clone, Debug, PartialEq)]
pub struct PreferredColorScheme {
    pub scheme: Option<ColorScheme>,
}

impl crate::primitive::Primitive for PreferredColorScheme {
    fn name(&self) -> &'static str {
        "preferredColorScheme"
    }

    fn preferred_color_scheme(&self) -> Option<ColorScheme> {
        self.scheme
    }
}
