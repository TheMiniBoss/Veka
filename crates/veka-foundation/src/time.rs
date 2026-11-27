//! Temps. Le cœur ne lit jamais l'horloge lui-même : c'est la plateforme qui fournit `Instant`.

use std::time::Duration;

/// Un instant, en nanosecondes depuis une origine choisie par la plateforme.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant(pub u64);

impl Instant {
    pub const fn from_nanos(nanos: u64) -> Self {
        Self(nanos)
    }

    pub fn duration_since(self, earlier: Instant) -> Duration {
        Duration::from_nanos(self.0.saturating_sub(earlier.0))
    }

    pub fn add(self, d: Duration) -> Instant {
        Instant(self.0.saturating_add(d.as_nanos() as u64))
    }
}
