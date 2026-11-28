//! Le défilement (étape 10) : `ScrollView`, ses barres, `ScrollViewReader`.
//!
//! La même sensation sur Windows et sur macOS :
//! - au pavé tactile, le contenu suit le doigt, puis Veka lance son élan quand les doigts se lèvent ;
//! - à la molette, chaque cran glisse en douceur ;
//! - les bords sont élastiques : au doigt, le contenu dépasse de plus en plus difficilement, et à la fin
//!   d'un élan (ou d'une molette rapide), il rebondit, puis revient avec un ressort.

// Les deux axes se traitent par indice (0 : horizontal, 1 : vertical), en parallèle sur plusieurs tableaux.
#![allow(clippy::needless_range_loop)]

use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

use veka_colora::UnitPoint;
use veka_foundation::{Key as KeyCode, Point, Rect, ScrollPhase, Size, Vector};
use veka_graphics::{CornerRadii, DrawCommand, Shape};

use crate::element::{Action, Element, Key};
use crate::layout::Axis;
use crate::primitive::{
    EventCx, EventResult, KeyEvent, LayoutCx, PaintCx, PointerEvent, Primitive, Proposal, ScrollEvent,
    ScrollResult, TickResult,
};

/// Constante de temps de la molette : un cran fait 95 % du chemin en environ 150 ms.
const WHEEL_TAU: f32 = 0.05;
/// Constante de temps de l'élan : sa vitesse est divisée par e toutes les 0,45 s.
const MOMENTUM_TAU: f32 = 0.45;
/// Pulsation du ressort des bords (amorti critique).
const SPRING_OMEGA: f32 = 14.0;
/// Le rebond d'une molette rapide au bout, au plus.
const WHEEL_BOUNCE: f32 = 60.0;
/// Rigidité de l'élastique au doigt (celle d'iOS).
const RUBBER: f32 = 0.55;
/// Sans nouvelles d'un pavé qui ne dit pas quand le geste finit (Windows), le geste est fini.
const GESTURE_TIMEOUT: f64 = 0.12;
/// Barres : visibles une seconde après le dernier mouvement, puis elles s'effacent.
const INDICATOR_HOLD: f64 = 1.0;
const INDICATOR_FADE: f64 = 0.3;
const INDICATOR_THIN: f32 = 6.0;
const INDICATOR_WIDE: f32 = 10.0;
const INDICATOR_MARGIN: f32 = 2.0;
const INDICATOR_MIN: f32 = 24.0;
/// Une page au clavier laisse ce recouvrement.
const PAGE_OVERLAP: f32 = 40.0;
/// Marge gardée autour d'une vue qu'on fait apparaître (focus, `scroll_to`).
const REVEAL_MARGIN: f32 = 8.0;

/// Les axes qui défilent, comme `Axis.Set` de SwiftUI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScrollAxes {
    pub horizontal: bool,
    pub vertical: bool,
}

impl ScrollAxes {
    pub const VERTICAL: ScrollAxes = ScrollAxes {
        horizontal: false,
        vertical: true,
    };
    pub const HORIZONTAL: ScrollAxes = ScrollAxes {
        horizontal: true,
        vertical: false,
    };
    pub const BOTH: ScrollAxes = ScrollAxes {
        horizontal: true,
        vertical: true,
    };

    fn has(self, i: usize) -> bool {
        if i == 0 { self.horizontal } else { self.vertical }
    }
}

impl From<Axis> for ScrollAxes {
    fn from(a: Axis) -> Self {
        match a {
            Axis::Horizontal => ScrollAxes::HORIZONTAL,
            Axis::Vertical => ScrollAxes::VERTICAL,
            Axis::Depth => ScrollAxes::BOTH,
        }
    }
}

/// Les barres de défilement, comme `.scrollIndicators` de SwiftUI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum ScrollIndicators {
    /// Pendant le défilement et au survol.
    #[default]
    Automatic,
    /// Toujours, quand le contenu dépasse.
    Visible,
    Hidden,
}

/// Les bords élastiques, comme `.scrollBounceBehavior` de SwiftUI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum ScrollBounce {
    /// Toujours, même quand le contenu tient dans la vue (sur l'axe vertical).
    #[default]
    Always,
    /// Seulement quand le contenu dépasse.
    BasedOnSize,
    /// Jamais : le défilement s'arrête net aux bords.
    Never,
}

/// Le primitive de `ScrollView`.
#[derive(Clone)]
pub struct ScrollViewPrim {
    pub axes: ScrollAxes,
    /// `.on_scroll_offset_change` : appelé quand le décalage affiché change.
    pub on_offset: Option<Rc<dyn Fn(Vector)>>,
}

impl std::fmt::Debug for ScrollViewPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScrollViewPrim")
            .field("axes", &self.axes)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Motion {
    #[default]
    Idle,
    /// Vers `target`, en douceur (molette, clavier, `scroll_to`).
    Glide { target: f32, tau: f32 },
    /// Le doigt est posé : `raw` est la position sans l'élastique.
    Drag { raw: f32 },
    /// L'élan, après le doigt.
    Momentum,
    /// Le ressort vers `target` (un bord, ou un `scroll_to` animé).
    Spring { target: f32 },
}

/// Un axe de défilement : position, vitesse, mouvement.
#[derive(Clone, Copy, Debug, Default)]
struct AxisState {
    /// Position affichée (peut dépasser les bords pendant un rebond).
    pos: f32,
    vel: f32,
    motion: Motion,
    /// Défilement possible : `0..=max`.
    max: f32,
    /// Taille de la vue sur cet axe.
    view: f32,
    /// Taille du contenu sur cet axe.
    content: f32,
}

impl AxisState {
    fn clamp(&self, v: f32) -> f32 {
        v.clamp(0.0, self.max)
    }

    fn over(&self) -> f32 {
        if self.pos < 0.0 {
            self.pos
        } else if self.pos > self.max {
            self.pos - self.max
        } else {
            0.0
        }
    }

    fn moving(&self) -> bool {
        self.motion != Motion::Idle
    }

    /// L'élastique : `x` au-delà du bord donne ce déplacement affiché.
    fn rubber(&self, x: f32) -> f32 {
        let d = self.view.max(1.0);
        let s = x.signum();
        let x = x.abs();
        s * (1.0 - 1.0 / (x * RUBBER / d + 1.0)) * d
    }

    /// L'inverse de [`rubber`](Self::rubber).
    fn unrubber(&self, y: f32) -> f32 {
        let d = self.view.max(1.0);
        let s = y.signum();
        let y = (y.abs() / d).min(0.95);
        s * (d / RUBBER) * (1.0 / (1.0 - y) - 1.0)
    }

    /// Position affichée pour une position brute du doigt.
    fn displayed(&self, raw: f32, bounce: bool) -> f32 {
        if !bounce {
            return self.clamp(raw);
        }
        if raw < 0.0 {
            self.rubber(raw)
        } else if raw > self.max {
            self.max + self.rubber(raw - self.max)
        } else {
            raw
        }
    }

    fn raw_of(&self, pos: f32) -> f32 {
        if pos < 0.0 {
            self.unrubber(pos)
        } else if pos > self.max {
            self.max + self.unrubber(pos - self.max)
        } else {
            pos
        }
    }

    /// Avance de `dt` secondes. Renvoie vrai si la position a changé.
    fn step(&mut self, dt: f32, bounce: bool) -> bool {
        let before = self.pos;
        match self.motion {
            Motion::Idle | Motion::Drag { .. } => {}
            Motion::Glide { target, tau } => {
                let k = 1.0 - (-dt / tau).exp();
                let next = self.pos + (target - self.pos) * k;
                self.vel = (next - self.pos) / dt.max(1e-4);
                self.pos = next;
                if (target - self.pos).abs() < 0.25 {
                    self.pos = target;
                    self.vel = 0.0;
                    self.motion = Motion::Idle;
                }
                // Une molette rapide qui arrive au bout rebondit un peu.
                if bounce && (self.pos <= 0.0 && self.vel < 0.0 || self.pos >= self.max && self.vel > 0.0) {
                    let bound = if self.pos <= 0.0 { 0.0 } else { self.max };
                    let limit = WHEEL_BOUNCE * SPRING_OMEGA * std::f32::consts::E;
                    self.vel = self.vel.clamp(-limit, limit) * 0.5;
                    self.motion = Motion::Spring { target: bound };
                }
            }
            Motion::Momentum => {
                self.pos += self.vel * dt;
                self.vel *= (-dt / MOMENTUM_TAU).exp();
                if self.pos < 0.0 || self.pos > self.max {
                    if bounce {
                        let bound = if self.pos < 0.0 { 0.0 } else { self.max };
                        self.motion = Motion::Spring { target: bound };
                    } else {
                        self.pos = self.clamp(self.pos);
                        self.vel = 0.0;
                        self.motion = Motion::Idle;
                    }
                } else if self.vel.abs() < 8.0 {
                    self.vel = 0.0;
                    self.motion = Motion::Idle;
                }
            }
            Motion::Spring { target } => {
                // Amorti critique, en petits pas pour rester stable.
                let mut left = dt;
                while left > 0.0 {
                    let h = left.min(1.0 / 240.0);
                    let x = self.pos - target;
                    let a = -SPRING_OMEGA * SPRING_OMEGA * x - 2.0 * SPRING_OMEGA * self.vel;
                    self.vel += a * h;
                    self.pos += self.vel * h;
                    left -= h;
                }
                if (self.pos - target).abs() < 0.3 && self.vel.abs() < 6.0 {
                    self.pos = target;
                    self.vel = 0.0;
                    self.motion = Motion::Idle;
                }
            }
        }
        self.pos != before
    }

    /// Le mouvement est fini et la position hors des bords : le ressort la ramène.
    fn settle(&mut self, bounce: bool) {
        if self.motion == Motion::Idle && self.over() != 0.0 {
            if bounce {
                let bound = self.clamp(self.pos);
                self.motion = Motion::Spring { target: bound };
            } else {
                self.pos = self.clamp(self.pos);
            }
        }
    }

    /// Va vers `target` en douceur (molette, clavier), depuis le mouvement en cours.
    fn glide_by(&mut self, delta: f32, bounce: bool) {
        let from = match self.motion {
            Motion::Glide { target, .. } => target,
            _ => self.pos,
        };
        let wanted = from + delta;
        let target = self.clamp(wanted);
        // Déjà au bout et on pousse encore : un petit rebond.
        if bounce && target == from && wanted != from && self.over() == 0.0 {
            let push = (wanted - from).clamp(-WHEEL_BOUNCE, WHEEL_BOUNCE);
            self.vel = push * SPRING_OMEGA * 0.6;
            self.motion = Motion::Spring { target };
            return;
        }
        self.motion = Motion::Glide {
            target,
            tau: WHEEL_TAU,
        };
    }
}

/// L'état d'une `ScrollView`.
#[derive(Default)]
pub struct ScrollState {
    axes: [AxisState; 2],
    /// L'heure du dernier pas.
    last_tick: Option<f64>,
    /// Le geste du pavé tactile : a-t-il commencé par `Began` (macOS) ? Heure du dernier `Changed`.
    gesture: Option<Gesture>,
    /// Le dernier mouvement (pour les barres).
    activity: f64,
    /// Le pointeur est sur une barre.
    hover: Option<usize>,
    /// Une barre est tenue : axe et décalage du pointeur dans le pouce.
    grab: Option<(usize, f32)>,
    viewport: Rect,
    /// Le dernier décalage envoyé à `.on_scroll_offset_change`.
    reported: Option<Vector>,
    /// `scroll_to` arrivé avant le premier layout.
    pending: Option<(Vector, bool)>,
    /// Le décalage du dernier placement du contenu.
    placed: Vector,
}

#[derive(Clone, Copy, Debug)]
struct Gesture {
    /// Le système dit quand le geste finit (macOS) : l'élan part de `Ended`.
    phased: bool,
    last: f64,
}

impl ScrollState {
    /// Le décalage affiché (positif : le contenu est monté ou parti à gauche).
    pub fn offset(&self) -> Vector {
        Vector::new(self.axes[0].pos, self.axes[1].pos)
    }

    /// Le plus grand décalage, sans dépasser les bords.
    pub fn max_offset(&self) -> Vector {
        Vector::new(self.axes[0].max, self.axes[1].max)
    }

    /// Vrai pendant un mouvement (doigt, élan, molette, ressort).
    pub fn is_moving(&self) -> bool {
        self.axes.iter().any(AxisState::moving) || self.gesture.is_some()
    }

    fn component(v: Vector, i: usize) -> f32 {
        if i == 0 { v.x } else { v.y }
    }

    /// Va vers ce décalage : animé (ressort) ou tout de suite.
    pub(crate) fn go_to(&mut self, target: Vector, animated: bool) {
        if self.viewport.width() <= 0.0 && self.viewport.height() <= 0.0 {
            self.pending = Some((target, animated));
            return;
        }
        for (i, a) in self.axes.iter_mut().enumerate() {
            let t = a.clamp(Self::component(target, i));
            if animated {
                a.motion = Motion::Glide { target: t, tau: 0.09 };
            } else {
                a.pos = t;
                a.vel = 0.0;
                a.motion = Motion::Idle;
            }
        }
    }

    /// Le décalage qui montre `r` (en coordonnées du contenu, origine en haut à gauche du contenu),
    /// aligné sur `anchor`, ou le plus petit déplacement qui le rend visible.
    fn reveal(&self, r: Rect, anchor: Option<UnitPoint>) -> Vector {
        let view = self.viewport.size;
        let mut out = [0.0f32; 2];
        for (i, a) in self.axes.iter().enumerate() {
            let (start, len, vlen) = if i == 0 {
                (r.x(), r.width(), view.width)
            } else {
                (r.y(), r.height(), view.height)
            };
            out[i] = match anchor {
                Some(u) => {
                    let f = if i == 0 { u.x } else { u.y };
                    start + len * f - vlen * f
                }
                None => {
                    let (lo, hi) = (start - REVEAL_MARGIN, start + len + REVEAL_MARGIN);
                    if lo < a.pos {
                        lo
                    } else if hi > a.pos + vlen {
                        (hi - vlen).min(lo)
                    } else {
                        a.pos
                    }
                }
            };
        }
        Vector::new(out[0], out[1])
    }
}

fn bounce_on(env: &crate::env::Environment, a: &AxisState, vertical: bool) -> bool {
    match env.scroll_bounce {
        ScrollBounce::Never => false,
        ScrollBounce::BasedOnSize => a.max > 0.0,
        ScrollBounce::Always => a.max > 0.0 || vertical,
    }
}

impl ScrollViewPrim {
    /// Le rectangle de la barre de l'axe `i` (le pouce), s'il y en a une.
    fn thumb(st: &ScrollState, rect: Rect, i: usize, wide: bool) -> Option<Rect> {
        let a = &st.axes[i];
        if a.max <= 0.0 || a.content <= 0.0 {
            return None;
        }
        let thick = if wide { INDICATOR_WIDE } else { INDICATOR_THIN };
        let other = st.axes[1 - i].max > 0.0;
        let track = a.view - 2.0 * INDICATOR_MARGIN - if other { INDICATOR_WIDE } else { 0.0 };
        // Le pouce rapetisse quand le contenu dépasse le bord.
        let visible = (a.view - a.over().abs()).max(INDICATOR_MIN);
        let len = (track * visible / a.content).clamp(INDICATOR_MIN.min(track), track);
        let t = (a.pos / a.max).clamp(0.0, 1.0);
        let start = INDICATOR_MARGIN + (track - len) * t;
        Some(if i == 1 {
            Rect::new(
                rect.max_x() - INDICATOR_MARGIN - thick,
                rect.y() + start,
                thick,
                len,
            )
        } else {
            Rect::new(
                rect.x() + start,
                rect.max_y() - INDICATOR_MARGIN - thick,
                len,
                thick,
            )
        })
    }

    /// La bande où vit la barre de l'axe `i` (pour le survol et le clic).
    fn track(st: &ScrollState, rect: Rect, i: usize) -> Option<Rect> {
        let a = &st.axes[i];
        if a.max <= 0.0 {
            return None;
        }
        let band = INDICATOR_WIDE + 2.0 * INDICATOR_MARGIN;
        Some(if i == 1 {
            Rect::new(rect.max_x() - band, rect.y(), band, rect.height())
        } else {
            Rect::new(rect.x(), rect.max_y() - band, rect.width(), band)
        })
    }

    fn indicator_alpha(st: &ScrollState, env: &crate::env::Environment, now: f64) -> f32 {
        match env.scroll_indicators {
            ScrollIndicators::Hidden => 0.0,
            ScrollIndicators::Visible => 1.0,
            ScrollIndicators::Automatic => {
                if st.grab.is_some() || st.hover.is_some() || st.is_moving() {
                    return 1.0;
                }
                if st.last_tick.is_none() {
                    // Jamais défilé : pas de barre.
                    return 0.0;
                }
                let idle = now - st.activity - INDICATOR_HOLD;
                if idle <= 0.0 {
                    1.0
                } else {
                    (1.0 - idle / INDICATOR_FADE).clamp(0.0, 1.0) as f32
                }
            }
        }
    }

    fn report(&self, st: &mut ScrollState) -> Option<Action> {
        let f = self.on_offset.clone()?;
        let off = st.offset();
        if st.reported == Some(off) {
            return None;
        }
        st.reported = Some(off);
        Some(Action::new(move || f(off)))
    }
}

impl Primitive for ScrollViewPrim {
    fn name(&self) -> &'static str {
        "ScrollView"
    }

    fn create_state(&self) -> Box<dyn Any> {
        Box::new(ScrollState::default())
    }

    fn size_that_fits(&self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let Some(&child) = cx.children().first() else {
            return Size::new(p.width.unwrap_or(0.0), p.height.unwrap_or(0.0));
        };
        let inner = Proposal::new(
            if self.axes.horizontal { None } else { p.width },
            if self.axes.vertical { None } else { p.height },
        );
        let s = cx.measure(child, inner);
        Size::new(p.width.unwrap_or(s.width), p.height.unwrap_or(s.height))
    }

    fn place(&self, cx: &mut LayoutCx<'_>, rect: Rect) {
        let children = cx.children();
        let Some(&child) = children.first() else { return };
        let inner = Proposal::new(
            if self.axes.horizontal {
                None
            } else {
                Some(rect.width())
            },
            if self.axes.vertical {
                None
            } else {
                Some(rect.height())
            },
        );
        let size = cx.measure(child, inner);
        let axes = self.axes;
        let offset = cx.state_and_text(|st: &mut ScrollState, _, env| {
            st.viewport = rect;
            for i in 0..2 {
                let a = &mut st.axes[i];
                let (content, view) = if i == 0 {
                    (size.width, rect.width())
                } else {
                    (size.height, rect.height())
                };
                a.content = content;
                a.view = view;
                a.max = if axes.has(i) {
                    (content - view).max(0.0)
                } else {
                    0.0
                };
                // Le contenu a rapetissé : on reste dans les bords (sauf pendant un mouvement).
                if !a.moving() && a.over() != 0.0 {
                    a.pos = a.clamp(a.pos);
                }
                if env.scroll_disabled && !a.moving() {
                    a.pos = a.clamp(a.pos);
                }
            }
            if let Some((t, animated)) = st.pending.take() {
                st.go_to(t, animated);
            }
            st.placed = st.offset();
            st.placed
        });
        // Sur l'axe qui ne défile pas, un contenu plus étroit est centré, comme SwiftUI.
        let x = if axes.horizontal {
            rect.x() - offset.x
        } else {
            rect.x() + (rect.width() - size.width) / 2.0
        };
        let y = if axes.vertical {
            rect.y() - offset.y
        } else {
            rect.y() + (rect.height() - size.height) / 2.0
        };
        cx.place(child, Rect::new(x, y, size.width, size.height));
    }

    fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        cx.list().push(DrawCommand::PushClip {
            shape: Shape::Rectangle,
            rect,
        });
        cx.paint_children();
        cx.list().push(DrawCommand::PopClip);
        let st = cx.state::<ScrollState>();
        let env = cx.env();
        let alpha = Self::indicator_alpha(st, env, st.last_tick.unwrap_or(0.0));
        if alpha <= 0.0 {
            return;
        }
        let ink = env.text_style().color;
        let mut thumbs = Vec::new();
        for i in 0..2 {
            let wide = st.hover == Some(i) || st.grab.is_some_and(|g| g.0 == i);
            if let Some(t) = Self::thumb(st, rect, i, wide) {
                thumbs.push(t);
            }
        }
        for t in thumbs {
            cx.list().fill(
                Shape::RoundedRectangle(CornerRadii::all(t.width().min(t.height()) / 2.0)),
                t,
                ink.with_alpha(0.45 * alpha),
            );
        }
    }

    fn hit_shape(&self) -> Option<Shape> {
        Some(Shape::Rectangle)
    }

    fn clips_hits(&self) -> bool {
        true
    }

    fn hits_before_children(&self, state: &dyn Any, rect: Rect, p: Point) -> bool {
        let Some(st) = state.downcast_ref::<ScrollState>() else {
            return false;
        };
        (0..2).any(|i| Self::track(st, rect, i).is_some_and(|t| t.contains(p)))
    }

    fn pointer(&self, cx: &mut EventCx<'_>, event: PointerEvent) -> EventResult {
        let rect = cx.rect();
        let now = cx.now();
        let hidden = cx.env().scroll_indicators == ScrollIndicators::Hidden;
        let st = cx.state::<ScrollState>();
        let on_track = |st: &ScrollState, p: Point| {
            (0..2).find(|&i| Self::track(st, rect, i).is_some_and(|t| t.contains(p)))
        };
        match event {
            PointerEvent::Move { position, .. } => {
                if let Some((i, grab)) = st.grab {
                    let Some(t) = Self::thumb(st, rect, i, true) else {
                        return EventResult::default();
                    };
                    let a_len = if i == 1 { t.height() } else { t.width() };
                    let start = if i == 1 { rect.y() } else { rect.x() };
                    let p = if i == 1 { position.y } else { position.x };
                    let other = st.axes[1 - i].max > 0.0;
                    let a = &mut st.axes[i];
                    let track = a.view - 2.0 * INDICATOR_MARGIN - if other { INDICATOR_WIDE } else { 0.0 };
                    let free = (track - a_len).max(1.0);
                    let t = ((p - grab - start - INDICATOR_MARGIN) / free).clamp(0.0, 1.0);
                    a.pos = t * a.max;
                    a.vel = 0.0;
                    a.motion = Motion::Idle;
                    st.activity = now;
                    return EventResult {
                        handled: true,
                        repaint: true,
                        animate: true,
                        ..Default::default()
                    };
                }
                let hover = if hidden { None } else { on_track(st, position) };
                let changed = hover != st.hover;
                st.hover = hover;
                if hover.is_some() {
                    st.activity = now;
                }
                EventResult {
                    handled: false,
                    repaint: changed,
                    animate: changed,
                    ..Default::default()
                }
            }
            PointerEvent::Leave => {
                let changed = st.hover.take().is_some();
                EventResult {
                    repaint: changed,
                    animate: changed,
                    ..Default::default()
                }
            }
            PointerEvent::Down { position } => {
                let Some(i) = (if hidden { None } else { on_track(st, position) }) else {
                    return EventResult::default();
                };
                let Some(t) = Self::thumb(st, rect, i, true) else {
                    return EventResult::default();
                };
                let (p, t0, t1) = if i == 1 {
                    (position.y, t.y(), t.max_y())
                } else {
                    (position.x, t.x(), t.max_x())
                };
                if p >= t0 && p <= t1 {
                    st.grab = Some((i, p - t0));
                } else {
                    // Clic à côté du pouce : une page dans cette direction.
                    let a = &mut st.axes[i];
                    let page = (a.view - PAGE_OVERLAP).max(a.view * 0.5);
                    a.glide_by(if p < t0 { -page } else { page }, false);
                }
                st.activity = now;
                EventResult {
                    handled: true,
                    repaint: true,
                    animate: true,
                    ..Default::default()
                }
            }
            PointerEvent::Up { .. } => {
                st.grab = None;
                st.activity = now;
                EventResult {
                    handled: true,
                    repaint: true,
                    animate: true,
                    ..Default::default()
                }
            }
        }
    }

    fn scroll(&self, cx: &mut EventCx<'_>, event: &ScrollEvent) -> ScrollResult {
        if cx.env().scroll_disabled {
            return ScrollResult::Passed;
        }
        let now = cx.now();
        let env_bounce: [bool; 2];
        {
            let env = cx.env().clone();
            let st = cx.state::<ScrollState>();
            env_bounce = [
                bounce_on(&env, &st.axes[0], false),
                bounce_on(&env, &st.axes[1], true),
            ];
        }
        let axes = self.axes;
        let st = cx.state::<ScrollState>();
        // Un geste commence ici seulement si cette vue peut défiler dans sa direction principale.
        let main = if event.delta.y.abs() >= event.delta.x.abs() {
            1
        } else {
            0
        };
        let starts = matches!(event.phase, ScrollPhase::Began)
            || (event.delta != Vector::new(0.0, 0.0) && st.gesture.is_none());
        if starts
            && event.phase != ScrollPhase::Began
            && !(axes.has(main) && (st.axes[main].max > 0.0 || env_bounce[main]))
        {
            return ScrollResult::Passed;
        }
        if event.phase == ScrollPhase::Began && !(axes.vertical || axes.horizontal) {
            return ScrollResult::Passed;
        }
        if !st.is_moving() {
            st.last_tick = Some(now);
        }
        st.activity = now;
        match event.phase {
            ScrollPhase::Wheel => {
                st.gesture = None;
                for i in 0..2 {
                    let d = ScrollState::component(event.delta, i);
                    if axes.has(i) && d != 0.0 {
                        st.axes[i].glide_by(d, env_bounce[i]);
                    }
                }
            }
            ScrollPhase::Began => {
                st.gesture = Some(Gesture {
                    phased: true,
                    last: now,
                });
                for a in &mut st.axes {
                    let raw = a.raw_of(a.pos);
                    a.vel = 0.0;
                    a.motion = Motion::Drag { raw };
                }
            }
            ScrollPhase::Changed => {
                let g = st.gesture.get_or_insert(Gesture {
                    phased: false,
                    last: now,
                });
                let dt = (now - g.last) as f32;
                g.last = now;
                for i in 0..2 {
                    let a = &mut st.axes[i];
                    if !axes.has(i) {
                        continue;
                    }
                    let d = ScrollState::component(event.delta, i);
                    let raw = match a.motion {
                        Motion::Drag { raw } => raw,
                        _ => a.raw_of(a.pos),
                    } + d;
                    a.pos = a.displayed(raw, env_bounce[i]);
                    let raw = if env_bounce[i] { raw } else { a.pos };
                    a.motion = Motion::Drag { raw };
                    // La vitesse du doigt, lissée.
                    if dt > 0.0 && dt < 0.1 {
                        let inst = d / dt;
                        a.vel = 0.6 * inst + 0.4 * a.vel;
                    } else {
                        a.vel = 0.0;
                    }
                }
            }
            ScrollPhase::Ended => {
                let stale = st.gesture.is_some_and(|g| now - g.last > 0.06);
                st.gesture = None;
                for i in 0..2 {
                    let a = &mut st.axes[i];
                    if !matches!(a.motion, Motion::Drag { .. }) {
                        continue;
                    }
                    if stale {
                        a.vel = 0.0;
                    }
                    a.motion = if a.over() != 0.0 && env_bounce[i] {
                        Motion::Spring {
                            target: a.clamp(a.pos),
                        }
                    } else if a.vel.abs() > 30.0 {
                        Motion::Momentum
                    } else {
                        Motion::Idle
                    };
                    a.settle(env_bounce[i]);
                }
            }
        }
        ScrollResult::Consumed
    }

    fn key(&self, cx: &mut EventCx<'_>, event: &KeyEvent) -> EventResult {
        if cx.env().scroll_disabled || !event.modifiers.without_shift().is_empty() {
            return EventResult::default();
        }
        let now = cx.now();
        let st = cx.state::<ScrollState>();
        let i = if self.axes.vertical { 1 } else { 0 };
        let a = &mut st.axes[i];
        if a.max <= 0.0 {
            return EventResult::default();
        }
        let page = (a.view - PAGE_OVERLAP).max(a.view * 0.5);
        let delta = match event.key {
            KeyCode::PageDown => page,
            KeyCode::PageUp => -page,
            KeyCode::Home => -a.pos - a.max,
            KeyCode::End => a.max * 2.0,
            _ => return EventResult::default(),
        };
        let idle = !a.moving();
        a.glide_by(delta, false);
        if idle {
            st.last_tick = Some(now);
        }
        st.activity = now;
        EventResult {
            handled: true,
            repaint: true,
            animate: true,
            ..Default::default()
        }
    }

    fn tick(&self, cx: &mut EventCx<'_>) -> TickResult {
        let now = cx.now();
        let env = cx.env().clone();
        let st = cx.state::<ScrollState>();
        let dt = (now - st.last_tick.unwrap_or(now)).clamp(0.0, 0.1) as f32;
        st.last_tick = Some(now);
        // Un pavé sans phases (Windows) : le geste finit quand plus rien n'arrive. Son élan vient du
        // système, déjà reçu : seul le ressort des bords reste à faire.
        if let Some(g) = st.gesture
            && !g.phased
            && now - g.last > GESTURE_TIMEOUT
        {
            st.gesture = None;
            for (i, a) in st.axes.iter_mut().enumerate() {
                if matches!(a.motion, Motion::Drag { .. }) {
                    a.motion = Motion::Idle;
                    a.vel = 0.0;
                    a.settle(bounce_on(&env, a, i == 1));
                }
            }
        }
        let mut moved = false;
        for (i, a) in st.axes.iter_mut().enumerate() {
            let bounce = bounce_on(&env, a, i == 1);
            if dt > 0.0 {
                moved |= a.step(dt, bounce);
            }
            if !matches!(a.motion, Motion::Drag { .. }) {
                a.settle(bounce);
            }
        }
        moved |= st.offset() != st.placed;
        if moved {
            st.activity = now;
        }
        let moving = st.is_moving();
        let alpha_before = env.scroll_indicators == ScrollIndicators::Automatic;
        let fading = alpha_before
            && st.grab.is_none()
            && st.hover.is_none()
            && now - st.activity < INDICATOR_HOLD + INDICATOR_FADE + 0.05
            && st.axes.iter().any(|a| a.max > 0.0);
        let action = self.report(st);
        TickResult {
            // La position affichée vient du layout : le premier pas après un geste replace aussi.
            moved: moved || action.is_some(),
            repaint: fading || moved,
            active: moving || fading,
            action,
        }
    }
}

/// Une vue qui défile, comme `ScrollView` de SwiftUI : `ScrollView![contenu]` défile verticalement.
pub fn scroll_view(axes: impl Into<ScrollAxes>, content: Element) -> Element {
    Element::new(
        ScrollViewPrim {
            axes: axes.into(),
            on_offset: None,
        },
        vec![content],
    )
}

impl From<ScrollIndicators> for crate::schema::Value {
    fn from(v: ScrollIndicators) -> Self {
        crate::schema::Value::Enum(Rc::from(match v {
            ScrollIndicators::Automatic => "automatic",
            ScrollIndicators::Visible => "visible",
            ScrollIndicators::Hidden => "hidden",
        }))
    }
}

/// Le primitive des réglages du défilement (`.scroll_indicators`, `.scroll_disabled`, `.scroll_bounce`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrollEnv {
    Indicators(ScrollIndicators),
    Disabled(bool),
    Bounce(ScrollBounce),
}

impl Primitive for ScrollEnv {
    fn name(&self) -> &'static str {
        match self {
            ScrollEnv::Indicators(_) => "scrollIndicators",
            ScrollEnv::Disabled(_) => "scrollDisabled",
            ScrollEnv::Bounce(_) => "scrollBounceBehavior",
        }
    }

    fn update_env(&self, env: &mut crate::env::Environment) -> bool {
        match *self {
            ScrollEnv::Indicators(v) => env.scroll_indicators = v,
            ScrollEnv::Disabled(v) => env.scroll_disabled = v,
            ScrollEnv::Bounce(v) => env.scroll_bounce = v,
        }
        true
    }
}

impl Element {
    /// Les barres des vues qui défilent dans le sous-arbre, comme `.scrollIndicators` de SwiftUI.
    pub fn scroll_indicators(self, v: ScrollIndicators) -> Element {
        self.wrap(ScrollEnv::Indicators(v))
    }

    /// Les vues qui défilent dans le sous-arbre ne bougent plus (`true`), comme `.scrollDisabled`.
    pub fn scroll_disabled(self, disabled: bool) -> Element {
        self.wrap(ScrollEnv::Disabled(disabled))
    }

    /// Les bords élastiques des vues qui défilent dans le sous-arbre, comme `.scrollBounceBehavior`.
    pub fn scroll_bounce(self, v: ScrollBounce) -> Element {
        self.wrap(ScrollEnv::Bounce(v))
    }

    /// Appelé quand le décalage d'une `ScrollView` change (à appliquer à la `ScrollView` elle-même),
    /// comme `.onScrollGeometryChange` de SwiftUI. Le décalage est positif quand le contenu est monté.
    pub fn on_scroll_offset_change(self, f: impl Fn(Vector) + 'static) -> Element {
        let f: Rc<dyn Fn(Vector)> = Rc::new(f);
        match self.map_primitive::<ScrollViewPrim>(|mut p| {
            p.on_offset = Some(f.clone());
            p
        }) {
            Ok(e) => e,
            // Pas une `ScrollView` : sans effet.
            Err(e) => e,
        }
    }
}

// --- ScrollViewReader -------------------------------------------------------------------------

/// Une demande de `scroll_to`, traitée à la prochaine image.
#[derive(Clone, Debug)]
pub(crate) struct ScrollRequest {
    pub key: Key,
    pub anchor: Option<UnitPoint>,
    pub animated: bool,
}

thread_local! {
    static REQUESTS: RefCell<Vec<ScrollRequest>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn take_scroll_requests() -> Vec<ScrollRequest> {
    REQUESTS.with(|r| std::mem::take(&mut *r.borrow_mut()))
}

pub(crate) fn has_scroll_requests() -> bool {
    REQUESTS.with(|r| !r.borrow().is_empty())
}

/// De quoi faire défiler jusqu'à une vue, comme `ScrollViewProxy` de SwiftUI.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScrollViewProxy;

impl ScrollViewProxy {
    /// Fait défiler les `ScrollView` qui la contiennent jusqu'à la vue qui porte `.id(id)`, alignée sur
    /// `anchor` (`None` : juste assez pour la voir). Animé dans un `with_animation`.
    pub fn scroll_to(&self, id: impl Into<Key>, anchor: Option<UnitPoint>) {
        let animated = crate::state::current_transaction().is_some();
        REQUESTS.with(|r| {
            r.borrow_mut().push(ScrollRequest {
                key: id.into(),
                anchor,
                animated,
            })
        });
        crate::state::wake_all();
    }
}

/// Donne un [`ScrollViewProxy`] au contenu, comme `ScrollViewReader` de SwiftUI.
#[allow(non_snake_case)]
pub fn ScrollViewReader(content: impl FnOnce(ScrollViewProxy) -> Element) -> Element {
    content(ScrollViewProxy)
}

/// Le décalage qui montre `target` (un rectangle affiché) dans la `ScrollView` du nœud `sv`.
pub(crate) fn reveal_offset(st: &ScrollState, target: Rect, anchor: Option<UnitPoint>) -> Vector {
    let o = st.offset();
    // En coordonnées du contenu.
    let r = Rect::new(
        target.x() - st.viewport.x() + o.x,
        target.y() - st.viewport.y() + o.y,
        target.width(),
        target.height(),
    );
    st.reveal(r, anchor)
}
