//! Transitions : comment une vue entre dans l'arbre et en sort.

/// Un bord du rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Edge {
    Leading,
    Trailing,
    Top,
    Bottom,
}

impl Edge {
    pub fn opposite(self) -> Edge {
        match self {
            Edge::Leading => Edge::Trailing,
            Edge::Trailing => Edge::Leading,
            Edge::Top => Edge::Bottom,
            Edge::Bottom => Edge::Top,
        }
    }
}

/// Un effet, décrit à l'état « absent » (progression 0) ; la progression 1 est la vue normale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TransitionEffect {
    Opacity,
    /// Échelle de départ (0 = depuis rien), autour du centre.
    Scale(f32),
    /// Décalé de toute sa taille vers ce bord.
    Move(Edge),
    /// Décalé de (x, y) points.
    Offset(f32, f32),
    /// Flou de départ en points : rendu par un fondu tant que les effets n'existent pas.
    Blur(f32),
}

/// L'entrée et la sortie d'une vue. Les effets d'une liste se combinent.
#[derive(Clone, Debug, PartialEq)]
pub struct Transition {
    pub insertion: Vec<TransitionEffect>,
    pub removal: Vec<TransitionEffect>,
}

impl Default for Transition {
    /// Le fondu.
    fn default() -> Self {
        Self::opacity()
    }
}

impl Transition {
    fn both(effects: Vec<TransitionEffect>) -> Self {
        Self {
            insertion: effects.clone(),
            removal: effects,
        }
    }

    /// Aucun effet : la vue apparaît et disparaît d'un coup.
    pub fn identity() -> Self {
        Self::both(Vec::new())
    }
    pub fn opacity() -> Self {
        Self::both(vec![TransitionEffect::Opacity])
    }
    pub fn scale(from: f32) -> Self {
        Self::both(vec![TransitionEffect::Scale(from)])
    }
    /// Entre par le bord de début, sort par le bord de fin.
    pub fn slide() -> Self {
        Self::asymmetric(Self::move_edge(Edge::Leading), Self::move_edge(Edge::Trailing))
    }
    pub fn move_edge(edge: Edge) -> Self {
        Self::both(vec![TransitionEffect::Move(edge)])
    }
    /// Entre par `from`, sort par le bord opposé, avec un fondu.
    pub fn push(from: Edge) -> Self {
        Self {
            insertion: vec![TransitionEffect::Move(from), TransitionEffect::Opacity],
            removal: vec![TransitionEffect::Move(from.opposite()), TransitionEffect::Opacity],
        }
    }
    pub fn offset(x: f32, y: f32) -> Self {
        Self::both(vec![TransitionEffect::Offset(x, y)])
    }
    pub fn blur(radius: f32) -> Self {
        Self::both(vec![TransitionEffect::Blur(radius)])
    }

    /// Les deux à la fois.
    pub fn combined(mut self, other: Transition) -> Self {
        self.insertion.extend(other.insertion);
        self.removal.extend(other.removal);
        self
    }

    /// L'entrée de l'une, la sortie de l'autre.
    pub fn asymmetric(insertion: Transition, removal: Transition) -> Self {
        Self {
            insertion: insertion.insertion,
            removal: removal.removal,
        }
    }
}

/// Ce que donnent des effets à une progression : opacité, échelle (autour du centre), décalage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransitionFrame {
    pub opacity: f32,
    pub scale: f32,
    pub offset: (f32, f32),
}

impl TransitionFrame {
    pub const IDENTITY: TransitionFrame = TransitionFrame {
        opacity: 1.0,
        scale: 1.0,
        offset: (0.0, 0.0),
    };

    /// `effects` à la progression `p` (1 = vue normale), pour une vue de taille `size`.
    pub fn evaluate(effects: &[TransitionEffect], p: f32, size: (f32, f32)) -> Self {
        let mut f = Self::IDENTITY;
        let away = 1.0 - p;
        for e in effects {
            match *e {
                TransitionEffect::Opacity | TransitionEffect::Blur(_) => f.opacity *= p.clamp(0.0, 1.0),
                TransitionEffect::Scale(s) => f.scale *= s + (1.0 - s) * p,
                TransitionEffect::Move(edge) => {
                    let (dx, dy) = match edge {
                        Edge::Leading => (-size.0, 0.0),
                        Edge::Trailing => (size.0, 0.0),
                        Edge::Top => (0.0, -size.1),
                        Edge::Bottom => (0.0, size.1),
                    };
                    f.offset.0 += dx * away;
                    f.offset.1 += dy * away;
                }
                TransitionEffect::Offset(x, y) => {
                    f.offset.0 += x * away;
                    f.offset.1 += y * away;
                }
            }
        }
        f
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effets() {
        let t = Transition::push(Edge::Bottom);
        let f = TransitionFrame::evaluate(&t.insertion, 0.0, (100.0, 40.0));
        assert_eq!(f.opacity, 0.0);
        assert_eq!(f.offset, (0.0, 40.0));
        let f = TransitionFrame::evaluate(&t.removal, 0.5, (100.0, 40.0));
        assert_eq!(f.offset, (0.0, -20.0));
        assert_eq!(
            TransitionFrame::evaluate(&t.insertion, 1.0, (100.0, 40.0)),
            TransitionFrame::IDENTITY
        );
        let f = TransitionFrame::evaluate(&Transition::scale(0.5).insertion, 0.5, (1.0, 1.0));
        assert_eq!(f.scale, 0.75);
    }
}
