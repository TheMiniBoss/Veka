//! L'animateur : fait aller une liste de valeurs de `from` à `to` selon une [`Animation`].

use crate::animation::{Animation, Repeat};
use crate::curve::ease;
use crate::spring::spring_at;

/// Les valeurs et vitesses à un instant, et si l'animation est finie.
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    pub value: Vec<f32>,
    pub velocity: Vec<f32>,
    pub done: bool,
}

/// Une animation en cours.
#[derive(Clone, Debug, PartialEq)]
pub struct Animator {
    pub spec: Animation,
    /// Secondes (l'horloge de l'appelant).
    pub start: f64,
    pub from: Vec<f32>,
    pub to: Vec<f32>,
    /// Vitesse au départ (unités par seconde), pour un ressort repris en vol.
    pub velocity: Vec<f32>,
}

impl Animator {
    /// `velocity` : vitesse de la valeur au départ (une animation interrompue), sinon nulle.
    pub fn new(
        spec: Animation,
        start: f64,
        from: Vec<f32>,
        to: Vec<f32>,
        velocity: Option<Vec<f32>>,
    ) -> Self {
        let velocity = velocity
            .filter(|v| v.len() == from.len())
            .unwrap_or_else(|| vec![0.0; from.len()]);
        Self {
            spec,
            start,
            from,
            to,
            velocity,
        }
    }

    /// Temps local en secondes (délai déduit, vitesse appliquée) ; négatif pendant le délai.
    fn local(&self, now: f64) -> f64 {
        ((now - self.start) - self.spec.delay as f64) * self.spec.speed.max(0.01) as f64
    }

    /// La valeur en `now`.
    pub fn sample(&self, now: f64) -> Sample {
        let t = self.local(now);
        if t <= 0.0 {
            return Sample {
                value: self.from.clone(),
                velocity: if self.spec.is_spring() {
                    self.velocity.clone()
                } else {
                    vec![0.0; self.from.len()]
                },
                done: false,
            };
        }
        if let Some(p) = self.spec.spring_params() {
            return self.spring(p, t);
        }
        let h = 1e-4;
        let (v1, done) = self.timed(t);
        if done {
            return Sample {
                value: self.to.clone(),
                velocity: vec![0.0; self.to.len()],
                done: true,
            };
        }
        let (v0, _) = self.timed((t - h).max(0.0));
        let speed = self.spec.speed.max(0.01) as f64;
        let velocity = v1
            .iter()
            .zip(&v0)
            .map(|(a, b)| ((a - b) as f64 / h * speed) as f32)
            .collect();
        Sample {
            value: v1,
            velocity,
            done: false,
        }
    }

    fn timed(&self, t: f64) -> (Vec<f32>, bool) {
        let d = self.spec.duration as f64;
        let total = match self.spec.repeat {
            Repeat::Once => Some(1u64),
            Repeat::Count(n) => Some(n.max(1) as u64),
            Repeat::Forever => None,
        };
        if d <= 1e-9 {
            return (self.to.clone(), total.is_some());
        }
        let iter = (t / d).floor();
        if let Some(n) = total
            && iter >= n as f64
        {
            return (self.to.clone(), true);
        }
        let mut phase = t / d - iter;
        if self.spec.autoreverse && (iter as u64) % 2 == 1 {
            phase = 1.0 - phase;
        }
        let p = ease(&self.spec.curve, phase) as f32;
        let v = self
            .from
            .iter()
            .zip(&self.to)
            .map(|(a, b)| a + (b - a) * p)
            .collect();
        (v, false)
    }

    fn spring(&self, p: crate::spring::SpringParams, t: f64) -> Sample {
        let mut value = Vec::with_capacity(self.to.len());
        let mut velocity = Vec::with_capacity(self.to.len());
        let mut done = true;
        for i in 0..self.to.len() {
            let x0 = (self.from[i] - self.to[i]) as f64;
            let v0 = self.velocity[i] as f64;
            let (x, v) = spring_at(p, x0, v0, t);
            // Au repos quand l'écart et la vitesse sont minimes devant l'amplitude du mouvement.
            let range = x0.abs().max(v0.abs() * 0.1).max(1e-3);
            if x.abs() > range * 1e-3 || v.abs() > range * 1e-2 {
                done = false;
            }
            value.push(self.to[i] + x as f32);
            velocity.push(v as f32);
        }
        if done {
            return Sample {
                value: self.to.clone(),
                velocity: vec![0.0; self.to.len()],
                done: true,
            };
        }
        Sample {
            value,
            velocity,
            done,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn courbe_a_duree() {
        let a = Animator::new(Animation::linear(1.0), 10.0, vec![0.0], vec![100.0], None);
        assert_eq!(a.sample(10.0).value, vec![0.0]);
        let s = a.sample(10.5);
        assert!((s.value[0] - 50.0).abs() < 1e-3 && !s.done);
        assert!((s.velocity[0] - 100.0).abs() < 0.5, "{:?}", s.velocity);
        assert!(a.sample(11.0).done);
        assert_eq!(a.sample(12.0).value, vec![100.0]);
    }

    #[test]
    fn delai_vitesse_repetition() {
        let spec = Animation::linear(1.0).delay(0.5).speed(2.0);
        let a = Animator::new(spec, 0.0, vec![0.0], vec![1.0], None);
        assert_eq!(a.sample(0.4).value, vec![0.0]);
        assert!((a.sample(0.75).value[0] - 0.5).abs() < 1e-4);
        assert!(a.sample(1.0).done);

        let spec = Animation::linear(1.0).repeat(3, true);
        let a = Animator::new(spec, 0.0, vec![0.0], vec![1.0], None);
        assert!((a.sample(1.25).value[0] - 0.75).abs() < 1e-4, "retour");
        assert!((a.sample(2.25).value[0] - 0.25).abs() < 1e-4);
        assert!(a.sample(3.1).done);

        let a = Animator::new(
            Animation::linear(1.0).repeat_forever(false),
            0.0,
            vec![0.0],
            vec![1.0],
            None,
        );
        assert!(!a.sample(1000.5).done);
    }

    #[test]
    fn ressort_garde_sa_vitesse() {
        let a = Animator::new(Animation::bouncy(), 0.0, vec![0.0], vec![100.0], None);
        let mid = a.sample(0.1);
        assert!(!mid.done && mid.velocity[0] > 0.0);
        // Nouvelle cible au milieu : on repart de la valeur et de la vitesse affichées.
        let b = Animator::new(
            Animation::bouncy(),
            0.1,
            mid.value.clone(),
            vec![0.0],
            Some(mid.velocity.clone()),
        );
        let s = b.sample(0.1);
        assert_eq!(s.value, mid.value);
        assert_eq!(s.velocity, mid.velocity);
        // Juste après : la valeur continue encore dans le même sens (pas d'à-coup).
        assert!(b.sample(0.11).value[0] > mid.value[0]);
        // Il finit par s'arrêter sur la cible.
        let fin = b.sample(5.0);
        assert!(fin.done && fin.value == vec![0.0]);
    }
}
