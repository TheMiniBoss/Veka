//! Géométrie 2D en points logiques (`f32`).

use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

/// Un point dans un repère 2D.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// Un déplacement 2D.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vector {
    pub x: f32,
    pub y: f32,
}

/// Une taille (largeur, hauteur).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

/// Un rectangle aligné sur les axes : origine (coin haut-gauche) + taille.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

/// Marges intérieures ou extérieures.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub top: f32,
    pub leading: f32,
    pub bottom: f32,
    pub trailing: f32,
}

/// Transformation affine 2D : `x' = a·x + c·y + tx`, `y' = b·x + d·y + ty`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform2D {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub tx: f32,
    pub ty: f32,
}

impl Point {
    pub const ZERO: Point = Point { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn to_vector(self) -> Vector {
        Vector::new(self.x, self.y)
    }
}

impl Vector {
    pub const ZERO: Vector = Vector { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }
}

impl Size {
    pub const ZERO: Size = Size { width: 0.0, height: 0.0 };

    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    /// Taille dont les deux côtés sont au moins 0.
    pub fn non_negative(self) -> Self {
        Self::new(self.width.max(0.0), self.height.max(0.0))
    }

    pub fn min_side(self) -> f32 {
        self.width.min(self.height)
    }
}

impl Rect {
    pub const ZERO: Rect = Rect { origin: Point::ZERO, size: Size::ZERO };

    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { origin: Point::new(x, y), size: Size::new(width, height) }
    }

    pub const fn from_origin_size(origin: Point, size: Size) -> Self {
        Self { origin, size }
    }

    pub fn x(&self) -> f32 {
        self.origin.x
    }
    pub fn y(&self) -> f32 {
        self.origin.y
    }
    pub fn width(&self) -> f32 {
        self.size.width
    }
    pub fn height(&self) -> f32 {
        self.size.height
    }
    pub fn max_x(&self) -> f32 {
        self.origin.x + self.size.width
    }
    pub fn max_y(&self) -> f32 {
        self.origin.y + self.size.height
    }

    pub fn center(&self) -> Point {
        Point::new(self.origin.x + self.size.width * 0.5, self.origin.y + self.size.height * 0.5)
    }

    /// Vrai si `p` est dans le rectangle (bord haut-gauche inclus, bord bas-droit exclu).
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x() && p.y >= self.y() && p.x < self.max_x() && p.y < self.max_y()
    }

    /// Le rectangle réduit de `insets` (jamais de taille négative).
    pub fn inset(&self, insets: Insets) -> Rect {
        Rect::from_origin_size(
            Point::new(self.x() + insets.leading, self.y() + insets.top),
            Size::new(self.width() - insets.horizontal(), self.height() - insets.vertical()).non_negative(),
        )
    }

    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.x().min(other.x());
        let y = self.y().min(other.y());
        Rect::new(x, y, self.max_x().max(other.max_x()) - x, self.max_y().max(other.max_y()) - y)
    }

    /// Intersection, ou `None` si les rectangles ne se touchent pas.
    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let x = self.x().max(other.x());
        let y = self.y().max(other.y());
        let mx = self.max_x().min(other.max_x());
        let my = self.max_y().min(other.max_y());
        (mx > x && my > y).then(|| Rect::new(x, y, mx - x, my - y))
    }

    pub fn translate(&self, v: Vector) -> Rect {
        Rect::from_origin_size(self.origin + v, self.size)
    }

    /// Un rectangle de taille `size` centré dans celui-ci.
    pub fn centered(&self, size: Size) -> Rect {
        let c = self.center();
        Rect::new(c.x - size.width * 0.5, c.y - size.height * 0.5, size.width, size.height)
    }
}

impl Insets {
    pub const ZERO: Insets = Insets { top: 0.0, leading: 0.0, bottom: 0.0, trailing: 0.0 };

    pub const fn new(top: f32, leading: f32, bottom: f32, trailing: f32) -> Self {
        Self { top, leading, bottom, trailing }
    }

    pub const fn all(v: f32) -> Self {
        Self::new(v, v, v, v)
    }

    pub fn horizontal(&self) -> f32 {
        self.leading + self.trailing
    }

    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

impl Default for Transform2D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform2D {
    pub const IDENTITY: Transform2D = Transform2D { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: 0.0, ty: 0.0 };

    pub const fn translation(tx: f32, ty: f32) -> Self {
        Self { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx, ty }
    }

    pub const fn scale(sx: f32, sy: f32) -> Self {
        Self { a: sx, b: 0.0, c: 0.0, d: sy, tx: 0.0, ty: 0.0 }
    }

    pub fn rotation(radians: f32) -> Self {
        let (s, c) = radians.sin_cos();
        Self { a: c, b: s, c: -s, d: c, tx: 0.0, ty: 0.0 }
    }

    /// `self` puis `other` (d'abord `self`, ensuite `other`).
    pub fn then(&self, other: &Transform2D) -> Transform2D {
        Transform2D {
            a: other.a * self.a + other.c * self.b,
            b: other.b * self.a + other.d * self.b,
            c: other.a * self.c + other.c * self.d,
            d: other.b * self.c + other.d * self.d,
            tx: other.a * self.tx + other.c * self.ty + other.tx,
            ty: other.b * self.tx + other.d * self.ty + other.ty,
        }
    }

    pub fn apply(&self, p: Point) -> Point {
        Point::new(self.a * p.x + self.c * p.y + self.tx, self.b * p.x + self.d * p.y + self.ty)
    }

    /// Inverse, ou `None` si la transformation n'est pas inversible.
    pub fn inverse(&self) -> Option<Transform2D> {
        let det = self.a * self.d - self.b * self.c;
        if det.abs() < f32::EPSILON {
            return None;
        }
        let inv = 1.0 / det;
        let a = self.d * inv;
        let b = -self.b * inv;
        let c = -self.c * inv;
        let d = self.a * inv;
        Some(Transform2D { a, b, c, d, tx: -(a * self.tx + c * self.ty), ty: -(b * self.tx + d * self.ty) })
    }
}

impl Add<Vector> for Point {
    type Output = Point;
    fn add(self, v: Vector) -> Point {
        Point::new(self.x + v.x, self.y + v.y)
    }
}
impl AddAssign<Vector> for Point {
    fn add_assign(&mut self, v: Vector) {
        *self = *self + v;
    }
}
impl Sub<Vector> for Point {
    type Output = Point;
    fn sub(self, v: Vector) -> Point {
        Point::new(self.x - v.x, self.y - v.y)
    }
}
impl SubAssign<Vector> for Point {
    fn sub_assign(&mut self, v: Vector) {
        *self = *self - v;
    }
}
impl Sub for Point {
    type Output = Vector;
    fn sub(self, o: Point) -> Vector {
        Vector::new(self.x - o.x, self.y - o.y)
    }
}
impl Add for Vector {
    type Output = Vector;
    fn add(self, o: Vector) -> Vector {
        Vector::new(self.x + o.x, self.y + o.y)
    }
}
impl Sub for Vector {
    type Output = Vector;
    fn sub(self, o: Vector) -> Vector {
        Vector::new(self.x - o.x, self.y - o.y)
    }
}
impl Neg for Vector {
    type Output = Vector;
    fn neg(self) -> Vector {
        Vector::new(-self.x, -self.y)
    }
}
impl Mul<f32> for Vector {
    type Output = Vector;
    fn mul(self, s: f32) -> Vector {
        Vector::new(self.x * s, self.y * s)
    }
}
impl Mul<f32> for Size {
    type Output = Size;
    fn mul(self, s: f32) -> Size {
        Size::new(self.width * s, self.height * s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Point, b: Point) -> bool {
        (a.x - b.x).abs() < 1e-4 && (a.y - b.y).abs() < 1e-4
    }

    #[test]
    fn rect_contains_edges() {
        let r = Rect::new(10.0, 10.0, 20.0, 10.0);
        assert!(r.contains(Point::new(10.0, 10.0)));
        assert!(!r.contains(Point::new(30.0, 15.0)));
        assert!(!r.contains(Point::new(9.9, 15.0)));
    }

    #[test]
    fn rect_union_and_intersection() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, 5.0, 10.0, 10.0);
        assert_eq!(a.union(&b), Rect::new(0.0, 0.0, 15.0, 15.0));
        assert_eq!(a.intersection(&b), Some(Rect::new(5.0, 5.0, 5.0, 5.0)));
        assert_eq!(a.intersection(&Rect::new(20.0, 0.0, 1.0, 1.0)), None);
    }

    #[test]
    fn inset_never_negative() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0).inset(Insets::all(8.0));
        assert_eq!(r, Rect::new(8.0, 8.0, 0.0, 0.0));
    }

    #[test]
    fn centered() {
        let r = Rect::new(0.0, 0.0, 100.0, 50.0).centered(Size::new(20.0, 10.0));
        assert_eq!(r, Rect::new(40.0, 20.0, 20.0, 10.0));
    }

    #[test]
    fn transform_then_and_inverse() {
        let t = Transform2D::scale(2.0, 3.0).then(&Transform2D::translation(5.0, -1.0));
        let p = t.apply(Point::new(1.0, 1.0));
        assert!(close(p, Point::new(7.0, 2.0)));
        let back = t.inverse().unwrap().apply(p);
        assert!(close(back, Point::new(1.0, 1.0)));
        let r = Transform2D::rotation(std::f32::consts::FRAC_PI_2).apply(Point::new(1.0, 0.0));
        assert!(close(r, Point::new(0.0, 1.0)));
        assert!(Transform2D::scale(0.0, 1.0).inverse().is_none());
    }
}
