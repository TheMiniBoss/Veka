//! Formes analytiques. La même fonction de distance (SDF) sert au hit-test, au rendu (le shader
//! reprend ces formules) et plus tard à la réfraction du verre.

use veka_foundation::{Point, Rect, Vector};

/// Rayons des quatre coins.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CornerRadii {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl CornerRadii {
    pub const fn all(r: f32) -> Self {
        Self {
            top_left: r,
            top_right: r,
            bottom_right: r,
            bottom_left: r,
        }
    }

    /// Rayons bornés à la moitié du plus petit côté (et jamais négatifs).
    pub fn clamped(&self, rect: &Rect) -> CornerRadii {
        let m = rect.size.min_side() * 0.5;
        let c = |r: f32| r.clamp(0.0, m.max(0.0));
        CornerRadii {
            top_left: c(self.top_left),
            top_right: c(self.top_right),
            bottom_right: c(self.bottom_right),
            bottom_left: c(self.bottom_left),
        }
    }
}

/// Une forme qui remplit un rectangle donné.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Rectangle,
    RoundedRectangle(CornerRadii),
    /// Rectangle aux extrémités entièrement arrondies.
    Capsule,
    /// Cercle centré, de diamètre le plus petit côté du rectangle.
    Circle,
    /// Ellipse inscrite dans le rectangle.
    Ellipse,
}

impl Shape {
    /// Distance signée de `p` au bord de la forme placée dans `rect` : négative dedans, positive dehors.
    ///
    /// Exacte pour toutes les formes sauf l'ellipse (approximation, exacte sur le bord).
    pub fn distance(&self, rect: &Rect, p: Point) -> f32 {
        let c = rect.center();
        let q = p - c;
        let half = Vector::new(rect.width() * 0.5, rect.height() * 0.5);
        match *self {
            Shape::Rectangle => rounded_box(q, half, 0.0),
            Shape::RoundedRectangle(radii) => {
                let r = radii.clamped(rect);
                let radius = match (q.x >= 0.0, q.y >= 0.0) {
                    (false, false) => r.top_left,
                    (true, false) => r.top_right,
                    (true, true) => r.bottom_right,
                    (false, true) => r.bottom_left,
                };
                rounded_box(q, half, radius)
            }
            Shape::Capsule => rounded_box(q, half, half.x.min(half.y).max(0.0)),
            Shape::Circle => q.length() - half.x.min(half.y).max(0.0),
            Shape::Ellipse => {
                if half.x <= 0.0 || half.y <= 0.0 {
                    return q.length();
                }
                let k = Vector::new(q.x / half.x, q.y / half.y).length();
                (k - 1.0) * half.x.min(half.y)
            }
        }
    }

    /// Vrai si `p` est dans la forme (bord compris).
    pub fn contains(&self, rect: &Rect, p: Point) -> bool {
        self.distance(rect, p) <= 0.0
    }

    /// Rayons de coin équivalents (pour les renderers qui ne connaissent que le rounded rect).
    pub fn corner_radii(&self, rect: &Rect) -> CornerRadii {
        match *self {
            Shape::Rectangle | Shape::Ellipse => CornerRadii::default(),
            Shape::RoundedRectangle(r) => r.clamped(rect),
            Shape::Capsule | Shape::Circle => CornerRadii::all(rect.size.min_side() * 0.5),
        }
    }
}

/// SDF d'une boîte de demi-taille `half`, centrée, coins de rayon `r`.
fn rounded_box(q: Vector, half: Vector, r: f32) -> f32 {
    let dx = q.x.abs() - half.x + r;
    let dy = q.y.abs() - half.y + r;
    let outside = Vector::new(dx.max(0.0), dy.max(0.0)).length();
    let inside = dx.max(dy).min(0.0);
    outside + inside - r
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: Rect = Rect::new(0.0, 0.0, 100.0, 40.0);

    #[test]
    fn rectangle() {
        assert!(Shape::Rectangle.contains(&R, Point::new(0.5, 0.5)));
        assert!(!Shape::Rectangle.contains(&R, Point::new(101.0, 20.0)));
        assert!((Shape::Rectangle.distance(&R, Point::new(50.0, 20.0)) + 20.0).abs() < 1e-4);
        assert!((Shape::Rectangle.distance(&R, Point::new(110.0, 20.0)) - 10.0).abs() < 1e-4);
    }

    #[test]
    fn rounded_corner_is_not_hit() {
        let s = Shape::RoundedRectangle(CornerRadii::all(12.0));
        assert!(
            !s.contains(&R, Point::new(1.0, 1.0)),
            "coin arrondi : pas de clic"
        );
        assert!(s.contains(&R, Point::new(12.0, 1.0)));
        assert!(s.contains(&R, Point::new(50.0, 20.0)));
    }

    #[test]
    fn per_corner_radius() {
        let s = Shape::RoundedRectangle(CornerRadii {
            top_left: 0.0,
            top_right: 20.0,
            ..Default::default()
        });
        assert!(s.contains(&R, Point::new(0.5, 0.5)));
        assert!(!s.contains(&R, Point::new(99.5, 0.5)));
    }

    #[test]
    fn capsule_ends() {
        let s = Shape::Capsule;
        assert!(!s.contains(&R, Point::new(2.0, 2.0)));
        assert!(s.contains(&R, Point::new(2.0, 20.0)));
        assert!(s.contains(&R, Point::new(80.0, 39.0)));
        // Bord exact à mi-hauteur de l'extrémité.
        assert!(s.distance(&R, Point::new(0.0, 20.0)).abs() < 1e-4);
    }

    #[test]
    fn circle_uses_smallest_side() {
        let s = Shape::Circle;
        assert!(s.contains(&R, Point::new(50.0, 1.0)));
        assert!(
            !s.contains(&R, Point::new(25.0, 20.0)),
            "hors du cercle de rayon 20"
        );
        assert!((s.distance(&R, Point::new(50.0, 20.0)) + 20.0).abs() < 1e-4);
    }

    #[test]
    fn ellipse() {
        let s = Shape::Ellipse;
        assert!(s.contains(&R, Point::new(1.0, 20.0)));
        assert!(!s.contains(&R, Point::new(5.0, 5.0)));
        assert!(s.distance(&R, Point::new(100.0, 20.0)).abs() < 1e-4);
    }

    #[test]
    fn radii_are_clamped() {
        let r = CornerRadii::all(500.0).clamped(&R);
        assert_eq!(r, CornerRadii::all(20.0));
    }
}
