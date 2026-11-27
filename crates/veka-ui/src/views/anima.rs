//! Anima dans les vues : modifiers visuels animables (`opacity`, `offset`, `scale_effect`,
//! `rotation_effect`), `.animation(spec, valeur)`, `.transition()`, et les valeurs animables des
//! peintures.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use veka_anima::{AnimatableData, AnimatableReader, Animation, Transition};
use veka_colora::{Gradient, Ink, Paint, UnitPoint};
use veka_foundation::{Color, Rect, Transform2D};
use veka_graphics::{CornerRadii, DrawCommand, Shape};

use crate::element::{Action, Element};
use crate::env::Environment;
use crate::primitive::{PaintCx, Primitive};

// --- Peintures animables -----------------------------------------------------------------------

fn push_unit(d: &mut AnimatableData, p: UnitPoint) {
    d.push(p.x).push(p.y);
}

fn read_unit(r: &mut AnimatableReader<'_>) -> UnitPoint {
    UnitPoint::new(r.f32(), r.f32())
}

fn push_gradient(d: &mut AnimatableData, g: &Gradient) {
    d.mark(g.stops.len() as u64).mark(g.space as u64);
    for s in &g.stops {
        d.push_color(s.ink.color().unwrap_or(Color::Clear))
            .push(s.location);
    }
}

fn read_gradient(g: &mut Gradient, r: &mut AnimatableReader<'_>) {
    for s in &mut g.stops {
        s.ink = Ink::Color(r.color());
        s.location = r.f32();
    }
}

/// Les valeurs animables d'une peinture **résolue** (sans jeton) : couleurs, arrêts, points.
pub(crate) fn push_paint(d: &mut AnimatableData, p: &Paint) {
    match p {
        Paint::Solid(c) => {
            d.mark(1).push_color(*c);
        }
        Paint::Linear(l) => {
            d.mark(2);
            push_gradient(d, &l.gradient);
            push_unit(d, l.start);
            push_unit(d, l.end);
        }
        Paint::Radial(g) => {
            d.mark(3);
            push_gradient(d, &g.gradient);
            push_unit(d, g.center);
            d.push(g.start_radius).push(g.end_radius);
        }
        Paint::Elliptical(g) => {
            d.mark(4);
            push_gradient(d, &g.gradient);
            push_unit(d, g.center);
            d.push(g.start_fraction).push(g.end_fraction);
        }
        Paint::Angular(g) => {
            d.mark(5);
            push_gradient(d, &g.gradient);
            push_unit(d, g.center);
            d.push(g.start_angle).push(g.end_angle);
        }
        Paint::Mesh(m) => {
            d.mark(6)
                .mark(m.width as u64)
                .mark(m.height as u64)
                .mark(m.space as u64);
            for p in &m.points {
                push_unit(d, p.position);
                d.push_color(p.ink.color().unwrap_or(Color::Clear));
            }
        }
        Paint::Token(..) => {
            d.mark(7);
        }
    }
}

/// Relit une peinture écrite par [`push_paint`] ; `template` donne sa forme (la cible).
pub(crate) fn read_paint(template: &Paint, r: &mut AnimatableReader<'_>) -> Paint {
    let mut p = template.clone();
    match &mut p {
        Paint::Solid(c) => *c = r.color(),
        Paint::Linear(l) => {
            read_gradient(&mut l.gradient, r);
            l.start = read_unit(r);
            l.end = read_unit(r);
        }
        Paint::Radial(g) => {
            read_gradient(&mut g.gradient, r);
            g.center = read_unit(r);
            g.start_radius = r.f32();
            g.end_radius = r.f32();
        }
        Paint::Elliptical(g) => {
            read_gradient(&mut g.gradient, r);
            g.center = read_unit(r);
            g.start_fraction = r.f32();
            g.end_fraction = r.f32();
        }
        Paint::Angular(g) => {
            read_gradient(&mut g.gradient, r);
            g.center = read_unit(r);
            g.start_angle = r.f32();
            g.end_angle = r.f32();
        }
        Paint::Mesh(m) => {
            for p in &mut m.points {
                p.position = read_unit(r);
                p.ink = Ink::Color(r.color());
            }
        }
        Paint::Token(..) => {}
    }
    p
}

/// Les rayons d'une forme (les autres n'ont rien d'animable).
pub(crate) fn push_shape(d: &mut AnimatableData, s: &Shape) {
    match s {
        Shape::Rectangle => d.mark(20),
        Shape::RoundedRectangle(r) => d
            .mark(21)
            .push(r.top_left)
            .push(r.top_right)
            .push(r.bottom_right)
            .push(r.bottom_left),
        Shape::Capsule => d.mark(22),
        Shape::Circle => d.mark(23),
        Shape::Ellipse => d.mark(24),
    };
}

pub(crate) fn read_shape(template: Shape, r: &mut AnimatableReader<'_>) -> Shape {
    match template {
        Shape::RoundedRectangle(_) => Shape::RoundedRectangle(CornerRadii {
            top_left: r.f32().max(0.0),
            top_right: r.f32().max(0.0),
            bottom_right: r.f32().max(0.0),
            bottom_left: r.f32().max(0.0),
        }),
        s => s,
    }
}

// --- Modifiers visuels -------------------------------------------------------------------------

fn paint_with(cx: &mut PaintCx<'_>, before: DrawCommand, after: DrawCommand) {
    cx.list().push(before);
    cx.paint_children();
    cx.list().push(after);
}

fn around_center(rect: Rect, m: Transform2D) -> Transform2D {
    let c = rect.center();
    Transform2D::translation(-c.x, -c.y)
        .then(&m)
        .then(&Transform2D::translation(c.x, c.y))
}

macro_rules! visual {
    ($t:ident, $name:literal, [$($f:ident),*]) => {
        impl $t {
            fn data(&self) -> AnimatableData {
                let mut d = AnimatableData::new();
                d.mark(value_key(&$name));
                $(d.push(self.$f);)*
                d
            }
        }
    };
}

/// Opacité du contenu (0 à 1). Ne refait pas le layout.
#[derive(Clone, Debug, PartialEq)]
pub struct OpacityPrim {
    pub opacity: f32,
}
visual!(OpacityPrim, "opacity", [opacity]);

impl Primitive for OpacityPrim {
    fn name(&self) -> &'static str {
        "opacity"
    }

    fn paint(&self, cx: &mut PaintCx<'_>, _rect: Rect) {
        let o = self.opacity.clamp(0.0, 1.0);
        if o >= 1.0 {
            cx.paint_children();
        } else if o > 0.0 {
            paint_with(cx, DrawCommand::PushLayer { opacity: o }, DrawCommand::PopLayer);
        }
    }

    fn animatable(&self, _env: &Environment) -> Option<AnimatableData> {
        Some(self.data())
    }

    fn with_animatable(&self, _env: &Environment, data: &AnimatableData) -> Option<Rc<dyn Primitive>> {
        let mut r = data.reader();
        Some(Rc::new(OpacityPrim { opacity: r.f32() }))
    }
}

/// Décalage du contenu, en points (sa place dans le layout ne bouge pas).
#[derive(Clone, Debug, PartialEq)]
pub struct OffsetPrim {
    pub x: f32,
    pub y: f32,
}
visual!(OffsetPrim, "offset", [x, y]);

impl Primitive for OffsetPrim {
    fn name(&self) -> &'static str {
        "offset"
    }

    fn paint(&self, cx: &mut PaintCx<'_>, _rect: Rect) {
        if self.x == 0.0 && self.y == 0.0 {
            return cx.paint_children();
        }
        let t = Transform2D::translation(self.x, self.y);
        paint_with(cx, DrawCommand::PushTransform(t), DrawCommand::PopTransform);
    }

    fn animatable(&self, _env: &Environment) -> Option<AnimatableData> {
        Some(self.data())
    }

    fn with_animatable(&self, _env: &Environment, data: &AnimatableData) -> Option<Rc<dyn Primitive>> {
        let mut r = data.reader();
        Some(Rc::new(OffsetPrim {
            x: r.f32(),
            y: r.f32(),
        }))
    }
}

/// Échelle du contenu autour de son centre.
#[derive(Clone, Debug, PartialEq)]
pub struct ScaleEffectPrim {
    pub x: f32,
    pub y: f32,
}
visual!(ScaleEffectPrim, "scaleEffect", [x, y]);

impl Primitive for ScaleEffectPrim {
    fn name(&self) -> &'static str {
        "scaleEffect"
    }

    fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        if self.x == 1.0 && self.y == 1.0 {
            return cx.paint_children();
        }
        let t = around_center(rect, Transform2D::scale(self.x, self.y));
        paint_with(cx, DrawCommand::PushTransform(t), DrawCommand::PopTransform);
    }

    fn animatable(&self, _env: &Environment) -> Option<AnimatableData> {
        Some(self.data())
    }

    fn with_animatable(&self, _env: &Environment, data: &AnimatableData) -> Option<Rc<dyn Primitive>> {
        let mut r = data.reader();
        Some(Rc::new(ScaleEffectPrim {
            x: r.f32(),
            y: r.f32(),
        }))
    }
}

/// Rotation du contenu autour de son centre, en degrés (sens horaire).
#[derive(Clone, Debug, PartialEq)]
pub struct RotationPrim {
    pub degrees: f32,
}
visual!(RotationPrim, "rotationEffect", [degrees]);

impl Primitive for RotationPrim {
    fn name(&self) -> &'static str {
        "rotationEffect"
    }

    fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        if self.degrees == 0.0 {
            return cx.paint_children();
        }
        let t = around_center(rect, Transform2D::rotation(self.degrees.to_radians()));
        paint_with(cx, DrawCommand::PushTransform(t), DrawCommand::PopTransform);
    }

    fn animatable(&self, _env: &Environment) -> Option<AnimatableData> {
        Some(self.data())
    }

    fn with_animatable(&self, _env: &Environment, data: &AnimatableData) -> Option<Rc<dyn Primitive>> {
        let mut r = data.reader();
        Some(Rc::new(RotationPrim { degrees: r.f32() }))
    }
}

// --- Animation et transition -------------------------------------------------------------------

/// `.animation(spec, valeur)` : quand `value` change, le sous-arbre s'anime avec `spec`.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimationPrim {
    pub spec: Option<Animation>,
    /// Empreinte de la valeur surveillée.
    pub value: u64,
}

impl Primitive for AnimationPrim {
    fn name(&self) -> &'static str {
        "animation"
    }
}

/// `.transition(t)` : comment la vue entre et sort quand c'est animé.
#[derive(Clone, Debug, PartialEq)]
pub struct TransitionPrim {
    pub transition: Transition,
}

impl Primitive for TransitionPrim {
    fn name(&self) -> &'static str {
        "transition"
    }
}

/// `.matched_geometry_effect(id)` : quand une vue portant `key` apparaît pendant qu'une autre portant
/// la même clé disparaît, la nouvelle part de la place de l'ancienne.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchedGeometryPrim {
    pub key: u64,
}

impl Primitive for MatchedGeometryPrim {
    fn name(&self) -> &'static str {
        "matchedGeometryEffect"
    }
}

/// `.on_animation_end(f)` : `f` est appelé quand les animations de ce sous-arbre sont finies.
#[derive(Debug)]
pub struct AnimationEndPrim {
    pub action: Action,
}

impl Primitive for AnimationEndPrim {
    fn name(&self) -> &'static str {
        "onAnimationEnd"
    }
}

/// L'empreinte d'une valeur (pour `.animation`).
pub fn value_key(value: &impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    value.hash(&mut h);
    h.finish()
}

impl Element {
    /// Opacité (0 à 1), animable.
    pub fn opacity(self, opacity: f32) -> Element {
        self.wrap(OpacityPrim { opacity })
    }

    /// Décale le dessin de (x, y) points, sans changer le layout. Animable.
    pub fn offset(self, x: f32, y: f32) -> Element {
        self.wrap(OffsetPrim { x, y })
    }

    /// Agrandit ou réduit le dessin autour de son centre, sans changer le layout. Animable.
    pub fn scale_effect(self, scale: f32) -> Element {
        self.scale_effect_xy(scale, scale)
    }

    pub fn scale_effect_xy(self, x: f32, y: f32) -> Element {
        self.wrap(ScaleEffectPrim { x, y })
    }

    /// Tourne le dessin autour de son centre (degrés, sens horaire), sans changer le layout. Animable.
    pub fn rotation_effect(self, degrees: f32) -> Element {
        self.wrap(RotationPrim { degrees })
    }

    /// Anime ce sous-arbre avec `spec` chaque fois que `value` change, même hors de
    /// `with_animation`. `None` : ne pas animer (même dans un `with_animation`).
    pub fn animation(self, spec: impl Into<Option<Animation>>, value: impl Hash) -> Element {
        self.wrap(AnimationPrim {
            spec: spec.into(),
            value: value_key(&value),
        })
    }

    /// Comment la vue entre dans l'arbre et en sort pendant une animation (fondu par défaut).
    pub fn transition(self, transition: Transition) -> Element {
        self.wrap(TransitionPrim { transition })
    }

    /// Anime ce sous-arbre normalement même si le système demande de réduire les animations (à
    /// réserver aux mouvements qui portent du sens, comme une jauge).
    pub fn ignore_reduce_motion(self) -> Element {
        self.wrap(super::EnvModifier::IgnoreReduceMotion)
    }

    /// Relie cette vue aux autres vues de même `id` : quand l'une disparaît et qu'une autre apparaît
    /// dans la même animation, la nouvelle part de la place et de la taille de l'ancienne (qui est
    /// retirée tout de suite). Comme `matchedGeometryEffect` de SwiftUI, sans espace de noms.
    pub fn matched_geometry_effect(self, id: impl Hash) -> Element {
        self.wrap(MatchedGeometryPrim { key: value_key(&id) })
    }

    /// Appelle `f` quand les animations de ce sous-arbre (layout, valeurs, entrées et sorties) se
    /// terminent.
    pub fn on_animation_end(self, f: impl FnMut() + 'static) -> Element {
        self.wrap(AnimationEndPrim {
            action: Action::new(f),
        })
    }
}
