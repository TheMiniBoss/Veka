//! Anima en C : specs d'animation, `veka_with_animation`, modifiers animables, transitions et
//! timelines.

use std::ffi::{c_char, c_void};
use std::rc::Rc;

use veka_ui::anima::{Animation, Curve, Edge, Keyframe, Repeat, Timeline, Transition, Track};
use veka_ui::schema::{Params, Value};
use veka_ui::{TimelineState, with_animation};

use crate::state::VekaState;
use crate::value::{VekaActionFn, action};
use crate::{VekaContext, VekaElement, build_boxed, context, guard, string, take};

// --- Specs -----------------------------------------------------------------------------------

/// La forme d'une animation.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VekaCurve {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    CircEaseIn,
    CircEaseOut,
    CircEaseInOut,
    /// Bézier cubique : `params` = x1, y1, x2, y2.
    Bezier,
    /// Ressort : `params` = masse, raideur, amortissement ; la durée est ignorée.
    Spring,
}

/// Une animation, passée par valeur. Se construit avec `veka_animation_linear`,
/// `veka_animation_spring`… puis `veka_animation_delay`, `veka_animation_repeat`…
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VekaAnimation {
    pub curve: VekaCurve,
    pub params: [f32; 4],
    /// Secondes (ignorée par un ressort).
    pub duration: f32,
    pub delay: f32,
    pub speed: f32,
    /// Nombre de fois en tout ; 0 ou 1 : une fois ; -1 : sans fin.
    pub repeat: i32,
    /// Une itération sur deux à l'envers.
    pub autoreverse: bool,
}

impl From<Animation> for VekaAnimation {
    fn from(a: Animation) -> Self {
        let (curve, params) = match a.curve {
            Curve::Linear => (VekaCurve::Linear, [0.0; 4]),
            Curve::EaseIn => (VekaCurve::EaseIn, [0.0; 4]),
            Curve::EaseOut => (VekaCurve::EaseOut, [0.0; 4]),
            Curve::EaseInOut => (VekaCurve::EaseInOut, [0.0; 4]),
            Curve::CircEaseIn => (VekaCurve::CircEaseIn, [0.0; 4]),
            Curve::CircEaseOut => (VekaCurve::CircEaseOut, [0.0; 4]),
            Curve::CircEaseInOut => (VekaCurve::CircEaseInOut, [0.0; 4]),
            Curve::Bezier(a, b, c, d) => (VekaCurve::Bezier, [a, b, c, d]),
            Curve::Spring {
                mass,
                stiffness,
                damping,
            } => (VekaCurve::Spring, [mass, stiffness, damping, 0.0]),
        };
        VekaAnimation {
            curve,
            params,
            duration: a.duration,
            delay: a.delay,
            speed: a.speed,
            repeat: match a.repeat {
                Repeat::Once => 1,
                Repeat::Count(n) => n.min(i32::MAX as u32) as i32,
                Repeat::Forever => -1,
            },
            autoreverse: a.autoreverse,
        }
    }
}

impl From<VekaAnimation> for Animation {
    fn from(a: VekaAnimation) -> Self {
        let p = a.params;
        let curve = match a.curve {
            VekaCurve::Linear => Curve::Linear,
            VekaCurve::EaseIn => Curve::EaseIn,
            VekaCurve::EaseOut => Curve::EaseOut,
            VekaCurve::EaseInOut => Curve::EaseInOut,
            VekaCurve::CircEaseIn => Curve::CircEaseIn,
            VekaCurve::CircEaseOut => Curve::CircEaseOut,
            VekaCurve::CircEaseInOut => Curve::CircEaseInOut,
            VekaCurve::Bezier => Curve::Bezier(p[0], p[1], p[2], p[3]),
            VekaCurve::Spring => Curve::Spring {
                mass: p[0].max(1e-3),
                stiffness: p[1].max(1e-3),
                damping: p[2].max(0.0),
            },
        };
        Animation {
            curve,
            duration: a.duration.max(0.0),
            delay: a.delay.max(0.0),
            speed: if a.speed > 0.0 { a.speed } else { 1.0 },
            repeat: match a.repeat {
                r if r < 0 => Repeat::Forever,
                0 | 1 => Repeat::Once,
                n => Repeat::Count(n as u32),
            },
            autoreverse: a.autoreverse,
        }
    }
}

impl From<VekaCurve> for Curve {
    /// Pour les images clés (Bézier et ressort n'y ont pas de sens : linéaire).
    fn from(c: VekaCurve) -> Self {
        match c {
            VekaCurve::EaseIn => Curve::EaseIn,
            VekaCurve::EaseOut => Curve::EaseOut,
            VekaCurve::EaseInOut => Curve::EaseInOut,
            VekaCurve::CircEaseIn => Curve::CircEaseIn,
            VekaCurve::CircEaseOut => Curve::CircEaseOut,
            VekaCurve::CircEaseInOut => Curve::CircEaseInOut,
            _ => Curve::Linear,
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_animation_linear(duration: f32) -> VekaAnimation {
    Animation::linear(duration).into()
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_animation_ease_in(duration: f32) -> VekaAnimation {
    Animation::ease_in(duration).into()
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_animation_ease_out(duration: f32) -> VekaAnimation {
    Animation::ease_out(duration).into()
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_animation_ease_in_out(duration: f32) -> VekaAnimation {
    Animation::ease_in_out(duration).into()
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_animation_circ_ease_in(duration: f32) -> VekaAnimation {
    Animation::circ_ease_in(duration).into()
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_animation_circ_ease_out(duration: f32) -> VekaAnimation {
    Animation::circ_ease_out(duration).into()
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_animation_circ_ease_in_out(duration: f32) -> VekaAnimation {
    Animation::circ_ease_in_out(duration).into()
}

/// Bézier cubique, comme `cubic-bezier(x1, y1, x2, y2)` en CSS.
#[unsafe(no_mangle)]
pub extern "C" fn veka_animation_timing_curve(x1: f32, y1: f32, x2: f32, y2: f32, duration: f32) -> VekaAnimation {
    Animation::timing_curve(x1, y1, x2, y2, duration).into()
}

/// Ressort : `response` = période (s), `damping_fraction` = 1 sans rebond.
#[unsafe(no_mangle)]
pub extern "C" fn veka_animation_spring(response: f32, damping_fraction: f32) -> VekaAnimation {
    Animation::spring(response, damping_fraction).into()
}

/// Ressort aux paramètres physiques.
#[unsafe(no_mangle)]
pub extern "C" fn veka_animation_interpolating_spring(mass: f32, stiffness: f32, damping: f32) -> VekaAnimation {
    Animation::interpolating_spring(mass, stiffness, damping).into()
}

/// Le ressort sans rebond (l'animation par défaut).
#[unsafe(no_mangle)]
pub extern "C" fn veka_animation_smooth() -> VekaAnimation {
    Animation::smooth().into()
}

/// Un ressort au léger rebond.
#[unsafe(no_mangle)]
pub extern "C" fn veka_animation_snappy() -> VekaAnimation {
    Animation::snappy().into()
}

/// Un ressort au rebond visible.
#[unsafe(no_mangle)]
pub extern "C" fn veka_animation_bouncy() -> VekaAnimation {
    Animation::bouncy().into()
}

/// Commence après `seconds`.
#[unsafe(no_mangle)]
pub extern "C" fn veka_animation_delay(animation: VekaAnimation, seconds: f32) -> VekaAnimation {
    Animation::from(animation).delay(seconds).into()
}

/// Multiplie la vitesse.
#[unsafe(no_mangle)]
pub extern "C" fn veka_animation_speed(animation: VekaAnimation, speed: f32) -> VekaAnimation {
    Animation::from(animation).speed(speed).into()
}

/// Joue `count` fois en tout.
#[unsafe(no_mangle)]
pub extern "C" fn veka_animation_repeat(animation: VekaAnimation, count: u32, autoreverse: bool) -> VekaAnimation {
    Animation::from(animation).repeat(count, autoreverse).into()
}

/// Joue sans fin.
#[unsafe(no_mangle)]
pub extern "C" fn veka_animation_repeat_forever(animation: VekaAnimation, autoreverse: bool) -> VekaAnimation {
    Animation::from(animation).repeat_forever(autoreverse).into()
}

/// Fonction appelée par `veka_with_animation`.
pub type VekaWithFn = Option<unsafe extern "C" fn(cx: *mut VekaContext, user: *mut c_void)>;

/// Appelle `f` : ce que changent les états modifiés dedans s'anime avec `animation` (layout,
/// couleurs, opacité, entrées et sorties). Comme `withAnimation` de SwiftUI.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_with_animation(
    _cx: *mut VekaContext,
    animation: VekaAnimation,
    f: VekaWithFn,
    user: *mut c_void,
) {
    guard((), || {
        let f = f.ok_or("veka_with_animation : fonction NULL")?;
        // SAFETY : fonction et `user` valides pendant l'appel (contrat de l'API).
        with_animation(animation.into(), || unsafe { f(context(), user) });
        Ok(())
    })
}

// --- Transitions -----------------------------------------------------------------------------

/// Une transition d'entrée et de sortie. Opaque, consommée quand elle est passée à une fonction.
pub struct VekaTransition {
    pub(crate) transition: Transition,
}

/// Un bord.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VekaEdge {
    Leading,
    Trailing,
    Top,
    Bottom,
}

impl From<VekaEdge> for Edge {
    fn from(e: VekaEdge) -> Self {
        match e {
            VekaEdge::Leading => Edge::Leading,
            VekaEdge::Trailing => Edge::Trailing,
            VekaEdge::Top => Edge::Top,
            VekaEdge::Bottom => Edge::Bottom,
        }
    }
}

fn transition(t: Transition) -> *mut VekaTransition {
    Box::into_raw(Box::new(VekaTransition { transition: t }))
}

pub(crate) unsafe fn take_transition(t: *mut VekaTransition) -> Result<Transition, String> {
    if t.is_null() {
        return Err("transition NULL".into());
    }
    // SAFETY : pointeur obtenu de `transition` et pas encore consommé (contrat de l'API).
    Ok(unsafe { Box::from_raw(t) }.transition)
}

/// Aucun effet.
#[unsafe(no_mangle)]
pub extern "C" fn veka_transition_identity() -> *mut VekaTransition {
    transition(Transition::identity())
}

/// Le fondu (la transition par défaut).
#[unsafe(no_mangle)]
pub extern "C" fn veka_transition_opacity() -> *mut VekaTransition {
    transition(Transition::opacity())
}

/// Grandit depuis l'échelle `from` (0 : depuis rien).
#[unsafe(no_mangle)]
pub extern "C" fn veka_transition_scale(from: f32) -> *mut VekaTransition {
    transition(Transition::scale(from))
}

/// Entre par le bord de début, sort par celui de fin.
#[unsafe(no_mangle)]
pub extern "C" fn veka_transition_slide() -> *mut VekaTransition {
    transition(Transition::slide())
}

/// Entre et sort par `edge`.
#[unsafe(no_mangle)]
pub extern "C" fn veka_transition_move(edge: VekaEdge) -> *mut VekaTransition {
    transition(Transition::move_edge(edge.into()))
}

/// Entre par `from`, sort par le bord opposé, en fondu.
#[unsafe(no_mangle)]
pub extern "C" fn veka_transition_push(from: VekaEdge) -> *mut VekaTransition {
    transition(Transition::push(from.into()))
}

/// Décalée de (x, y) points quand elle est absente.
#[unsafe(no_mangle)]
pub extern "C" fn veka_transition_offset(x: f32, y: f32) -> *mut VekaTransition {
    transition(Transition::offset(x, y))
}

/// Floue quand elle est absente (rendu par un fondu pour l'instant).
#[unsafe(no_mangle)]
pub extern "C" fn veka_transition_blur(radius: f32) -> *mut VekaTransition {
    transition(Transition::blur(radius))
}

/// Les deux à la fois (les deux sont consommées).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_transition_combined(
    a: *mut VekaTransition,
    b: *mut VekaTransition,
) -> *mut VekaTransition {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API ; les deux sont reprises avant toute erreur.
        let (a, b) = unsafe { (take_transition(a), take_transition(b)) };
        Ok(transition(a?.combined(b?)))
    })
}

/// Une transition pour l'entrée, une autre pour la sortie (les deux sont consommées).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_transition_asymmetric(
    insertion: *mut VekaTransition,
    removal: *mut VekaTransition,
) -> *mut VekaTransition {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API ; les deux sont reprises avant toute erreur.
        let (i, r) = unsafe { (take_transition(insertion), take_transition(removal)) };
        Ok(transition(Transition::asymmetric(i?, r?)))
    })
}

/// Libère une transition donnée à personne.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_transition_free(t: *mut VekaTransition) {
    if !t.is_null() {
        // SAFETY : contrat de l'API.
        drop(unsafe { Box::from_raw(t) });
    }
}

// --- Modifiers -------------------------------------------------------------------------------

unsafe fn modify(element: *mut VekaElement, name: &str, params: Params) -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let child = unsafe { take(element) }?;
        build_boxed(name, params, vec![child])
    })
}

fn num(v: f32) -> Value {
    Value::Float(v as f64)
}

/// Opacité (0 à 1), animable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_opacity(element: *mut VekaElement, opacity: f32) -> *mut VekaElement {
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "opacity", Params::new().with("amount", num(opacity))) }
}

/// Décale le dessin sans changer le layout, animable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_offset(element: *mut VekaElement, x: f32, y: f32) -> *mut VekaElement {
    let params = Params::new().with("x", num(x)).with("y", num(y));
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "offset", params) }
}

/// Agrandit le dessin autour de son centre, animable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_scale_effect(element: *mut VekaElement, scale: f32) -> *mut VekaElement {
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "scaleEffect", Params::new().with("scale", num(scale))) }
}

/// Comme `veka_scale_effect`, un facteur par axe.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_scale_effect_xy(element: *mut VekaElement, x: f32, y: f32) -> *mut VekaElement {
    let params = Params::new().with("x", num(x)).with("y", num(y));
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "scaleEffect", params) }
}

/// Tourne le dessin autour de son centre (degrés, sens horaire), animable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_rotation_effect(element: *mut VekaElement, degrees: f32) -> *mut VekaElement {
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "rotationEffect", Params::new().with("degrees", num(degrees))) }
}

/// Anime le contenu avec `*animation` chaque fois que `value` change. `animation` NULL : ne pas
/// animer, même dans un `veka_with_animation`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_animation(
    element: *mut VekaElement,
    animation: *const VekaAnimation,
    value: i64,
) -> *mut VekaElement {
    let mut params = Params::new().with("value", Value::Int(value));
    if !animation.is_null() {
        // SAFETY : pointeur lisible (contrat de l'API).
        params = params.with("spec", Value::Animation(unsafe { *animation }.into()));
    }
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "animation", params) }
}

/// Comment le contenu entre et sort pendant une animation. La transition est consommée.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_transition(
    element: *mut VekaElement,
    transition: *mut VekaTransition,
) -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API ; les deux sont repris avant toute erreur.
        let (child, t) = unsafe { (take(element), take_transition(transition)) };
        let params = Params::new().with("transition", Value::Transition(t?));
        build_boxed("transition", params, vec![child?])
    })
}

/// Relie le contenu aux vues de même `id` : quand l'une disparaît et que l'autre apparaît dans la
/// même animation, la nouvelle part de la place de l'ancienne.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_matched_geometry_effect(
    element: *mut VekaElement,
    id: *const c_char,
) -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let child = unsafe { take(element) }?;
        let id = unsafe { string(id, "matched_geometry_effect") }?;
        let params = Params::new().with("id", Value::String(Rc::from(id)));
        build_boxed("matchedGeometryEffect", params, vec![child])
    })
}

/// L'identité de l'élément parmi ses frères (comme `.id()` en Rust) : une ligne garde son état et
/// ses animations quand la liste change, et entre ou sort seule avec sa transition.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_id(element: *mut VekaElement, id: i64) -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let e = unsafe { take(element) }?;
        Ok(crate::boxed(e.id(id)))
    })
}

/// Anime le contenu même si le système demande de réduire les animations.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_ignore_reduce_motion(element: *mut VekaElement) -> *mut VekaElement {
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "ignoreReduceMotion", Params::new()) }
}

/// Appelle `callback` quand les animations du contenu sont finies.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_on_animation_end(
    element: *mut VekaElement,
    callback: VekaActionFn,
    user: *mut c_void,
) -> *mut VekaElement {
    let params = match action(callback, user) {
        Ok(a) => Params::new().with("action", Value::Action(a)),
        Err(e) => {
            // SAFETY : l'élément est consommé même en cas d'erreur.
            unsafe { crate::veka_element_free(element) };
            crate::set_error(e);
            return std::ptr::null_mut();
        }
    };
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "onAnimationEnd", params) }
}

// --- Timelines -------------------------------------------------------------------------------

/// La description d'une timeline : pistes à images clés et événements. Gardée par le C
/// (`veka_timeline_free`) ; `veka_state_new_timeline` en fait une copie.
pub struct VekaTimeline {
    timeline: Timeline,
}

unsafe fn timeline_mut<'a>(t: *mut VekaTimeline) -> Result<&'a mut Timeline, String> {
    if t.is_null() {
        return Err("timeline NULL".into());
    }
    // SAFETY : pointeur obtenu de `veka_timeline_new` (contrat de l'API).
    Ok(unsafe { &mut (*t).timeline })
}

/// Une timeline vide (vitesse 1, sans boucle).
#[unsafe(no_mangle)]
pub extern "C" fn veka_timeline_new() -> *mut VekaTimeline {
    Box::into_raw(Box::new(VekaTimeline {
        timeline: Timeline::new(),
    }))
}

/// Ajoute une image clé à la piste `track` (créée au besoin) : `value` à `time` secondes, `curve`
/// adoucissant le segment qui part de cette clé.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_timeline_key(
    t: *mut VekaTimeline,
    track: *const c_char,
    time: f32,
    value: f32,
    curve: VekaCurve,
) {
    guard((), || {
        // SAFETY : contrat de l'API.
        let tl = unsafe { timeline_mut(t) }?;
        let name = unsafe { string(track, "piste") }?;
        let key = Keyframe::new(time, value).curve(curve.into());
        let i = match tl.tracks.iter().position(|tr| tr.name == name) {
            Some(i) => i,
            None => {
                tl.tracks.push(Track {
                    name: name.to_owned(),
                    keys: Vec::new(),
                });
                tl.tracks.len() - 1
            }
        };
        let keys = &mut tl.tracks[i].keys;
        keys.push(key);
        keys.sort_by(|a, b| a.time.total_cmp(&b.time));
        Ok(())
    })
}

/// Un événement nommé à `time` secondes (voir `veka_timeline_on_event`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_timeline_event(t: *mut VekaTimeline, time: f32, name: *const c_char) {
    guard((), || {
        // SAFETY : contrat de l'API.
        let tl = unsafe { timeline_mut(t) }?;
        let name = unsafe { string(name, "événement") }?;
        *tl = std::mem::take(tl).event(time, name);
        Ok(())
    })
}

/// La durée en secondes (0 : jusqu'à la dernière clé ou au dernier événement).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_timeline_length(t: *mut VekaTimeline, seconds: f32) {
    guard((), || {
        // SAFETY : contrat de l'API.
        let tl = unsafe { timeline_mut(t) }?;
        tl.length = seconds.max(0.0);
        Ok(())
    })
}

/// Recommence au début une fois au bout.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_timeline_looping(t: *mut VekaTimeline, looping: bool) {
    guard((), || {
        // SAFETY : contrat de l'API.
        unsafe { timeline_mut(t) }?.looping = looping;
        Ok(())
    })
}

/// Multiplie la vitesse de lecture.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_timeline_speed(t: *mut VekaTimeline, speed: f32) {
    guard((), || {
        // SAFETY : contrat de l'API.
        unsafe { timeline_mut(t) }?.speed = speed.max(0.01);
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_timeline_free(t: *mut VekaTimeline) {
    if !t.is_null() {
        // SAFETY : contrat de l'API.
        drop(unsafe { Box::from_raw(t) });
    }
}

fn timeline_handle(s: *mut VekaState) -> Result<TimelineState, String> {
    if s.is_null() {
        return Err("timeline NULL".into());
    }
    Ok(TimelineState::from_bits(s as usize as u64))
}

/// Un état timeline (créé la première fois avec une copie de `*timeline`, retrouvé ensuite), comme
/// les autres `veka_state_new_*`. `autoplay` : lancé dès sa création.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_state_new_timeline(
    _cx: *mut VekaContext,
    timeline: *const VekaTimeline,
    autoplay: bool,
) -> *mut VekaState {
    guard(std::ptr::null_mut(), || {
        if timeline.is_null() {
            return Err("timeline NULL".into());
        }
        // SAFETY : pointeur lisible (contrat de l'API).
        let tl = unsafe { (*timeline).timeline.clone() };
        let state = if autoplay {
            TimelineState::playing(tl)
        } else {
            TimelineState::new(tl)
        };
        Ok(state.to_bits() as usize as *mut VekaState)
    })
}

/// Lance (ou reprend) la lecture ; arrivée au bout, elle repart du début.
#[unsafe(no_mangle)]
pub extern "C" fn veka_timeline_play(_cx: *mut VekaContext, s: *mut VekaState) {
    guard((), || {
        timeline_handle(s)?.play();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_timeline_pause(_cx: *mut VekaContext, s: *mut VekaState) {
    guard((), || {
        timeline_handle(s)?.pause();
        Ok(())
    })
}

/// Met en pause et revient au début.
#[unsafe(no_mangle)]
pub extern "C" fn veka_timeline_stop(_cx: *mut VekaContext, s: *mut VekaState) {
    guard((), || {
        timeline_handle(s)?.stop();
        Ok(())
    })
}

/// Va à `time` secondes.
#[unsafe(no_mangle)]
pub extern "C" fn veka_timeline_seek(_cx: *mut VekaContext, s: *mut VekaState, time: f32) {
    guard((), || {
        timeline_handle(s)?.seek(time);
        Ok(())
    })
}

/// Change de sens depuis l'instant présent.
#[unsafe(no_mangle)]
pub extern "C" fn veka_timeline_reverse(_cx: *mut VekaContext, s: *mut VekaState) {
    guard((), || {
        timeline_handle(s)?.reverse();
        Ok(())
    })
}

/// La valeur de la piste `track` à l'instant présent (0 si elle n'existe pas).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_timeline_value(
    _cx: *mut VekaContext,
    s: *mut VekaState,
    track: *const c_char,
) -> f32 {
    guard(0.0, || {
        // SAFETY : contrat de l'API.
        let track = unsafe { string(track, "piste") }?;
        Ok(timeline_handle(s)?.value(track))
    })
}

/// L'instant présent, en secondes.
#[unsafe(no_mangle)]
pub extern "C" fn veka_timeline_time(_cx: *mut VekaContext, s: *mut VekaState) -> f32 {
    guard(0.0, || Ok(timeline_handle(s)?.time()))
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_timeline_is_playing(_cx: *mut VekaContext, s: *mut VekaState) -> bool {
    guard(false, || Ok(timeline_handle(s)?.is_playing()))
}

/// Fonction appelée pour chaque événement franchi, avec son nom.
pub type VekaEventFn = Option<unsafe extern "C" fn(cx: *mut VekaContext, name: *const c_char, user: *mut c_void)>;

/// Appelle `callback` pour chaque événement franchi pendant la lecture. Peut être rappelée à chaque
/// `body` (la dernière fonction donnée est gardée).
#[unsafe(no_mangle)]
pub extern "C" fn veka_timeline_on_event(
    _cx: *mut VekaContext,
    s: *mut VekaState,
    callback: VekaEventFn,
    user: *mut c_void,
) {
    guard((), || {
        let cb = callback.ok_or("veka_timeline_on_event : fonction NULL")?;
        let user = user as usize;
        timeline_handle(s)?.on_event(move |name| {
            let c = std::ffi::CString::new(name.replace('\0', " ")).unwrap_or_default();
            // SAFETY : fonction et `user` valides tant que l'interface existe (contrat de l'API).
            unsafe { cb(context(), c.as_ptr(), user as *mut c_void) };
        });
        Ok(())
    })
}
