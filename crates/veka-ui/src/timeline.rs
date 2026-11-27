//! Les timelines dans une vue : [`TimelineState`] garde une [`Playback`] comme un état ; pendant la
//! lecture, la surface la fait avancer à chaque image et ré-évalue la vue, qui lit la valeur de ses
//! pistes avec [`TimelineState::value`].
//!
//! ```
//! # use veka_ui::prelude::*;
//! fn pulsation() -> impl View {
//!     let tl = TimelineState::playing(
//!         Timeline::new()
//!             .track("échelle", [Keyframe::new(0.0, 1.0), Keyframe::new(0.4, 1.3), Keyframe::new(0.8, 1.0)])
//!             .looping(true),
//!     );
//!     Circle().frame(20, 20).scale_effect(tl.value("échelle"))
//! }
//! ```

use std::cell::RefCell;
use std::rc::Rc;

use veka_anima::{Playback, Timeline};

use crate::state::State;

type Handler = Rc<RefCell<dyn FnMut(&str)>>;

#[doc(hidden)]
pub struct TimelineSlot {
    playback: Playback,
    on_event: Option<Handler>,
}

struct Entry {
    surface: u32,
    state: State<TimelineSlot>,
    /// L'heure à laquelle la timeline a avancé pour la dernière fois (aucune : elle vient d'être lancée).
    last: Option<f64>,
}

thread_local! {
    static PLAYING: RefCell<Vec<Entry>> = const { RefCell::new(Vec::new()) };
}

/// Une timeline lue par une vue. Une poignée `Copy`, comme [`State`] : à créer pendant l'évaluation
/// de la vue, toujours dans le même ordre.
#[derive(Clone, Copy, Debug)]
pub struct TimelineState {
    state: State<TimelineSlot>,
}

impl std::fmt::Debug for TimelineSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.playback.fmt(f)
    }
}

impl TimelineState {
    /// Crée la timeline, en pause au début (la première fois ; ensuite `timeline` est ignorée).
    pub fn new(timeline: Timeline) -> Self {
        Self::create(timeline, false)
    }

    /// Comme [`new`](Self::new), lancée dès sa création.
    pub fn playing(timeline: Timeline) -> Self {
        Self::create(timeline, true)
    }

    fn create(timeline: Timeline, autoplay: bool) -> Self {
        let mut created = false;
        let state = State::new_with(|| {
            created = true;
            let mut playback = Playback::new(timeline);
            if autoplay {
                playback.play();
            }
            TimelineSlot {
                playback,
                on_event: None,
            }
        });
        let t = Self { state };
        if created && autoplay {
            t.register();
        }
        t
    }

    /// La valeur d'une piste à l'instant présent (0 si elle n'existe pas).
    pub fn value(&self, track: &str) -> f32 {
        self.state.with(|s| s.playback.value(track))
    }

    /// L'instant présent, en secondes.
    pub fn time(&self) -> f32 {
        self.state.with(|s| s.playback.time)
    }

    pub fn duration(&self) -> f32 {
        self.state.with(|s| s.playback.timeline.duration())
    }

    pub fn is_playing(&self) -> bool {
        self.state.with(|s| s.playback.playing)
    }

    /// Vrai si la lecture va vers le début.
    pub fn is_reversed(&self) -> bool {
        self.state.with(|s| s.playback.direction < 0.0)
    }

    /// Lance (ou reprend) la lecture ; arrivée au bout, elle repart du début.
    pub fn play(&self) {
        self.state.update(|s| s.playback.play());
        self.register();
    }

    pub fn pause(&self) {
        self.state.update(|s| s.playback.pause());
    }

    /// Met en pause et revient au début.
    pub fn stop(&self) {
        self.state.update(|s| s.playback.stop());
    }

    /// Va à l'instant `time` (secondes).
    pub fn seek(&self, time: f32) {
        self.state.update(|s| s.playback.seek(time));
    }

    /// Change de sens depuis l'instant présent.
    pub fn reverse(&self) {
        self.state.update(|s| s.playback.reverse());
    }

    /// Appelé pour chaque événement franchi pendant la lecture, avec son nom. Peut être appelé à
    /// chaque évaluation de la vue (le dernier rappel donné est gardé).
    pub fn on_event(&self, f: impl FnMut(&str) + 'static) {
        let h: Handler = Rc::new(RefCell::new(f));
        self.state.update_silent(|s| s.on_event = Some(h));
    }

    fn register(&self) {
        let Some(surface) = self.state.surface() else {
            return;
        };
        PLAYING.with(|p| {
            let mut p = p.borrow_mut();
            let bits = self.state.to_bits();
            if !p.iter().any(|e| e.state.to_bits() == bits) {
                p.push(Entry {
                    surface,
                    state: self.state,
                    last: None,
                });
            }
        });
    }

    /// La poignée sous forme d'entier, pour l'interface C.
    #[doc(hidden)]
    pub fn to_bits(self) -> u64 {
        self.state.to_bits()
    }

    #[doc(hidden)]
    pub fn from_bits(bits: u64) -> Self {
        Self {
            state: State::from_bits(bits),
        }
    }
}

/// Fait avancer les timelines de `surface` jusqu'à `now` et lance leurs rappels d'événements.
pub(crate) fn advance(surface: u32, now: f64) {
    let entries: Vec<Entry> = PLAYING.with(|p| {
        let mut p = p.borrow_mut();
        let (mine, others): (Vec<Entry>, Vec<Entry>) = p.drain(..).partition(|e| e.surface == surface);
        *p = others;
        mine
    });
    let mut kept = Vec::new();
    for mut e in entries {
        if !e.state.is_alive() {
            continue;
        }
        let dt = e.last.map_or(0.0, |l| now - l);
        e.last = Some(now);
        let playing = if dt > 0.0 {
            let (events, playing, handler) = e.state.update(|s| {
                let ev = s.playback.advance(dt as f32);
                (ev, s.playback.playing, s.on_event.clone())
            });
            if let Some(h) = handler {
                for name in &events {
                    (h.borrow_mut())(name);
                }
            }
            // Un rappel a pu mettre la timeline en pause.
            playing && e.state.try_with(|s| s.playback.playing).unwrap_or(false)
        } else {
            e.state.try_with(|s| s.playback.playing).unwrap_or(false)
        };
        if playing {
            kept.push(e);
        }
    }
    PLAYING.with(|p| {
        let mut p = p.borrow_mut();
        for e in kept {
            let bits = e.state.to_bits();
            if !p.iter().any(|x| x.state.to_bits() == bits) {
                p.push(e);
            }
        }
    });
}

/// Vrai si une timeline de `surface` est en lecture.
pub(crate) fn has_playing(surface: u32) -> bool {
    PLAYING.with(|p| {
        p.borrow()
            .iter()
            .any(|e| e.surface == surface && e.state.is_alive())
    })
}

pub(crate) fn forget(surface: u32) {
    PLAYING.with(|p| p.borrow_mut().retain(|e| e.surface != surface));
}
