//! Timelines : des pistes à images clés et des événements nommés, pour les animations écrites à
//! l'avance. Une vue lit la valeur de ses pistes pendant la lecture (voir `TimelineState` dans
//! veka-ui), ce qui permet de piloter n'importe quelle propriété ou de changer un état à un instant
//! donné.

use crate::animation::Curve;
use crate::curve::ease;

/// Une image clé : une valeur à un instant (secondes) ; `curve` adoucit le segment qui part d'ici.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyframe {
    pub time: f32,
    pub value: f32,
    pub curve: Curve,
}

impl Keyframe {
    pub fn new(time: f32, value: f32) -> Self {
        Self {
            time,
            value,
            curve: Curve::EaseInOut,
        }
    }

    pub fn curve(mut self, curve: Curve) -> Self {
        self.curve = curve;
        self
    }
}

/// Une piste nommée, ses images clés triées par temps.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub name: String,
    pub keys: Vec<Keyframe>,
}

impl Track {
    /// La valeur en `t` (avant la première clé : sa valeur ; après la dernière : la sienne).
    pub fn evaluate(&self, t: f32) -> f32 {
        let Some(first) = self.keys.first() else {
            return 0.0;
        };
        if t <= first.time {
            return first.value;
        }
        for w in self.keys.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            if t <= b.time {
                let span = b.time - a.time;
                if span <= 1e-6 {
                    return b.value;
                }
                let p = ease(&a.curve, ((t - a.time) / span) as f64) as f32;
                return a.value + (b.value - a.value) * p;
            }
        }
        self.keys.last().map_or(0.0, |k| k.value)
    }
}

/// Un événement nommé à un instant.
#[derive(Clone, Debug, PartialEq)]
pub struct TimelineEvent {
    pub time: f32,
    pub name: String,
}

/// Des pistes et des événements.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Timeline {
    pub tracks: Vec<Track>,
    pub events: Vec<TimelineEvent>,
    /// Secondes ; 0 = jusqu'à la dernière clé ou au dernier événement.
    pub length: f32,
    pub looping: bool,
    pub speed: f32,
}

impl Timeline {
    pub fn new() -> Self {
        Self {
            speed: 1.0,
            ..Default::default()
        }
    }

    /// Ajoute une piste (les clés sont triées).
    pub fn track(mut self, name: impl Into<String>, keys: impl IntoIterator<Item = Keyframe>) -> Self {
        let mut keys: Vec<Keyframe> = keys.into_iter().collect();
        keys.sort_by(|a, b| a.time.total_cmp(&b.time));
        self.tracks.push(Track {
            name: name.into(),
            keys,
        });
        self
    }

    pub fn event(mut self, time: f32, name: impl Into<String>) -> Self {
        self.events.push(TimelineEvent {
            time,
            name: name.into(),
        });
        self.events.sort_by(|a, b| a.time.total_cmp(&b.time));
        self
    }

    pub fn length(mut self, seconds: f32) -> Self {
        self.length = seconds.max(0.0);
        self
    }

    pub fn looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    pub fn speed(mut self, speed: f32) -> Self {
        self.speed = speed.max(0.01);
        self
    }

    /// La durée : `length`, sinon la dernière clé ou le dernier événement.
    pub fn duration(&self) -> f32 {
        if self.length > 0.0 {
            return self.length;
        }
        let keys = self.tracks.iter().filter_map(|t| t.keys.last()).map(|k| k.time);
        let events = self.events.iter().map(|e| e.time);
        keys.chain(events).fold(0.0, f32::max)
    }

    /// La valeur d'une piste en `t` (0 si elle n'existe pas).
    pub fn value(&self, track: &str, t: f32) -> f32 {
        self.tracks
            .iter()
            .find(|tr| tr.name == track)
            .map_or(0.0, |tr| tr.evaluate(t))
    }

    /// Les événements franchis en allant de `from` à `to` (dans un sens ou dans l'autre ; `from`
    /// exclu, `to` inclus).
    pub fn crossed(&self, from: f32, to: f32) -> Vec<&str> {
        let (lo, hi) = if from <= to { (from, to) } else { (to, from) };
        let mut out: Vec<&TimelineEvent> = self
            .events
            .iter()
            .filter(|e| {
                if from <= to {
                    e.time > lo && e.time <= hi
                } else {
                    e.time >= lo && e.time < hi
                }
            })
            .collect();
        if from > to {
            out.reverse();
        }
        out.into_iter().map(|e| e.name.as_str()).collect()
    }
}

/// La lecture d'une timeline : temps, sens, en cours ou non. Avance avec [`advance`](Self::advance).
#[derive(Clone, Debug, PartialEq)]
pub struct Playback {
    pub timeline: Timeline,
    pub time: f32,
    pub playing: bool,
    /// 1 en avant, -1 à l'envers.
    pub direction: f32,
}

impl Playback {
    pub fn new(timeline: Timeline) -> Self {
        Self {
            timeline,
            time: 0.0,
            playing: false,
            direction: 1.0,
        }
    }

    pub fn value(&self, track: &str) -> f32 {
        self.timeline.value(track, self.time)
    }

    /// Avance de `dt` secondes ; renvoie les événements franchis. S'arrête au bout (sauf en boucle).
    pub fn advance(&mut self, dt: f32) -> Vec<String> {
        if !self.playing || dt <= 0.0 {
            return Vec::new();
        }
        let len = self.timeline.duration();
        let mut events = Vec::new();
        let mut from = self.time;
        let mut to = self.time + dt * self.timeline.speed * self.direction;
        loop {
            let end = if self.direction > 0.0 { len } else { 0.0 };
            let past = if self.direction > 0.0 { to >= end } else { to <= end };
            if !past || len <= 0.0 {
                events.extend(self.timeline.crossed(from, to).into_iter().map(String::from));
                self.time = to.clamp(0.0, len.max(0.0));
                break;
            }
            events.extend(self.timeline.crossed(from, end).into_iter().map(String::from));
            if !self.timeline.looping {
                self.time = end;
                self.playing = false;
                break;
            }
            // Boucle : on repart du début (ou de la fin, à l'envers).
            let rest = to - end;
            from = if self.direction > 0.0 { 0.0 } else { len };
            to = from + rest;
            // Un événement à 0 (ou à la fin) est rejoué à chaque tour.
            if let Some(e) = self.timeline.events.iter().find(|e| e.time == from) {
                events.push(e.name.clone());
            }
        }
        events
    }

    pub fn play(&mut self) {
        let len = self.timeline.duration();
        // Relancée au bout : repart du début.
        if self.direction > 0.0 && self.time >= len {
            self.time = 0.0;
        } else if self.direction < 0.0 && self.time <= 0.0 {
            self.time = len;
        }
        self.playing = true;
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    /// Met en pause et revient au début.
    pub fn stop(&mut self) {
        self.playing = false;
        self.time = 0.0;
        self.direction = 1.0;
    }

    pub fn seek(&mut self, time: f32) {
        self.time = time.clamp(0.0, self.timeline.duration());
    }

    /// Repart dans l'autre sens depuis l'instant présent.
    pub fn reverse(&mut self) {
        self.direction = -self.direction;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tl() -> Timeline {
        Timeline::new()
            .track(
                "x",
                [Keyframe::new(0.0, 0.0).curve(Curve::Linear), Keyframe::new(1.0, 100.0), Keyframe::new(2.0, 50.0)],
            )
            .event(0.5, "milieu")
            .event(2.0, "fin")
    }

    #[test]
    fn pistes() {
        let t = tl();
        assert_eq!(t.duration(), 2.0);
        assert_eq!(t.value("x", -1.0), 0.0);
        assert_eq!(t.value("x", 0.25), 25.0);
        assert_eq!(t.value("x", 1.5), 75.0, "ease in-out à mi-segment");
        assert_eq!(t.value("x", 3.0), 50.0);
        assert_eq!(t.value("y", 1.0), 0.0);
    }

    #[test]
    fn lecture_evenements_boucle_retour() {
        let mut p = Playback::new(tl());
        assert!(p.advance(1.0).is_empty(), "pas en lecture");
        p.play();
        assert_eq!(p.advance(0.6), ["milieu"]);
        assert_eq!(p.advance(2.0), ["fin"]);
        assert!(!p.playing && p.time == 2.0);
        p.reverse();
        p.play();
        assert_eq!(p.advance(1.6), ["milieu"]);
        assert!((p.time - 0.4).abs() < 1e-5);

        let mut p = Playback::new(tl().looping(true));
        p.play();
        let ev = p.advance(4.6);
        assert_eq!(ev, ["milieu", "fin", "milieu", "fin", "milieu"]);
        assert!(p.playing && (p.time - 0.6).abs() < 1e-4);
    }
}
