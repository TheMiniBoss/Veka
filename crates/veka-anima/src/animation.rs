//! `Animation` : comment une valeur va de l'ancienne à la nouvelle.

use crate::spring::SpringParams;

/// La forme du mouvement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Curve {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    CircEaseIn,
    CircEaseOut,
    CircEaseInOut,
    /// Bézier cubique (x1, y1, x2, y2), comme `cubic-bezier` en CSS.
    Bezier(f32, f32, f32, f32),
    /// Ressort physique : la durée est ignorée, il s'arrête de lui-même.
    Spring {
        mass: f32,
        stiffness: f32,
        damping: f32,
    },
}

/// Combien de fois l'animation se joue.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Repeat {
    #[default]
    Once,
    /// En tout (2 = deux fois).
    Count(u32),
    Forever,
}

/// Une animation : courbe, durée, délai, vitesse, répétition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Animation {
    pub curve: Curve,
    /// Secondes (ignorée par un ressort).
    pub duration: f32,
    pub delay: f32,
    /// Multiplie le temps.
    pub speed: f32,
    pub repeat: Repeat,
    /// Une itération sur deux se joue à l'envers.
    pub autoreverse: bool,
}

impl Default for Animation {
    /// Le ressort `smooth`, comme SwiftUI.
    fn default() -> Self {
        Self::smooth()
    }
}

impl Animation {
    fn timed(curve: Curve, duration: f32) -> Self {
        Self {
            curve,
            duration: duration.max(0.0),
            delay: 0.0,
            speed: 1.0,
            repeat: Repeat::Once,
            autoreverse: false,
        }
    }

    pub fn linear(duration: f32) -> Self {
        Self::timed(Curve::Linear, duration)
    }
    pub fn ease_in(duration: f32) -> Self {
        Self::timed(Curve::EaseIn, duration)
    }
    pub fn ease_out(duration: f32) -> Self {
        Self::timed(Curve::EaseOut, duration)
    }
    pub fn ease_in_out(duration: f32) -> Self {
        Self::timed(Curve::EaseInOut, duration)
    }
    pub fn circ_ease_in(duration: f32) -> Self {
        Self::timed(Curve::CircEaseIn, duration)
    }
    pub fn circ_ease_out(duration: f32) -> Self {
        Self::timed(Curve::CircEaseOut, duration)
    }
    pub fn circ_ease_in_out(duration: f32) -> Self {
        Self::timed(Curve::CircEaseInOut, duration)
    }

    /// Bézier cubique, comme `cubic-bezier(x1, y1, x2, y2)` en CSS.
    pub fn timing_curve(x1: f32, y1: f32, x2: f32, y2: f32, duration: f32) -> Self {
        Self::timed(Curve::Bezier(x1, y1, x2, y2), duration)
    }

    /// Ressort façon SwiftUI : `response` = période (s), `damping_fraction` = 1 sans rebond.
    pub fn spring(response: f32, damping_fraction: f32) -> Self {
        let p = SpringParams::from_response(response as f64, damping_fraction as f64);
        Self::interpolating_spring(p.mass as f32, p.stiffness as f32, p.damping as f32)
    }

    /// Ressort aux paramètres physiques.
    pub fn interpolating_spring(mass: f32, stiffness: f32, damping: f32) -> Self {
        Self::timed(
            Curve::Spring {
                mass,
                stiffness,
                damping,
            },
            0.0,
        )
    }

    /// Sans rebond.
    pub fn smooth() -> Self {
        Self::spring(0.5, 1.0)
    }
    /// Un léger rebond.
    pub fn snappy() -> Self {
        Self::spring(0.5, 0.85)
    }
    /// Un rebond visible.
    pub fn bouncy() -> Self {
        Self::spring(0.5, 0.7)
    }

    pub fn delay(mut self, seconds: f32) -> Self {
        self.delay = seconds.max(0.0);
        self
    }

    pub fn speed(mut self, speed: f32) -> Self {
        self.speed = speed.max(0.01);
        self
    }

    /// Joue `count` fois en tout.
    pub fn repeat(mut self, count: u32, autoreverse: bool) -> Self {
        self.repeat = Repeat::Count(count.max(1));
        self.autoreverse = autoreverse;
        self
    }

    pub fn repeat_forever(mut self, autoreverse: bool) -> Self {
        self.repeat = Repeat::Forever;
        self.autoreverse = autoreverse;
        self
    }

    pub fn is_spring(&self) -> bool {
        matches!(self.curve, Curve::Spring { .. })
    }

    pub(crate) fn spring_params(&self) -> Option<SpringParams> {
        match self.curve {
            Curve::Spring {
                mass,
                stiffness,
                damping,
            } => Some(SpringParams {
                mass: mass as f64,
                stiffness: stiffness as f64,
                damping: damping as f64,
            }),
            _ => None,
        }
    }
}
