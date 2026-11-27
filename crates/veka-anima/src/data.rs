//! Les valeurs animables d'un primitive : une liste de nombres (les couleurs y sont en Oklab).

use veka_colora::{ColorSpace, from_space, to_space};
use veka_foundation::{Color, Point, Size};

/// Une liste de nombres que l'on interpole composante par composante, et sa « forme » : deux listes ne
/// s'animent l'une vers l'autre que si elles ont la même longueur et la même signature (sinon :
/// changement immédiat). La signature distingue par exemple une couleur unie d'un dégradé.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimatableData {
    pub values: Vec<f32>,
    pub signature: u64,
}

impl AnimatableData {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_values(values: Vec<f32>, signature: u64) -> Self {
        Self { values, signature }
    }

    /// Note une sorte de contenu (un dégradé, une forme…) dans la signature.
    pub fn mark(&mut self, tag: u64) -> &mut Self {
        // FNV-1a sur les 8 octets.
        for b in tag.to_le_bytes() {
            self.signature = (self.signature ^ b as u64).wrapping_mul(0x100_0000_01b3);
        }
        self.signature = self.signature.wrapping_add(0xcbf2_9ce4_8422_2325);
        self
    }

    /// Même forme : les deux peuvent s'animer l'une vers l'autre.
    pub fn compatible(&self, other: &AnimatableData) -> bool {
        self.signature == other.signature && self.values.len() == other.values.len()
    }

    pub fn push(&mut self, v: f32) -> &mut Self {
        self.values.push(v);
        self
    }

    pub fn push_point(&mut self, p: Point) -> &mut Self {
        self.values.extend([p.x, p.y]);
        self
    }

    pub fn push_size(&mut self, s: Size) -> &mut Self {
        self.values.extend([s.width, s.height]);
        self
    }

    /// Une couleur, en Oklab prémultiplié (4 nombres) : l'interpolation y est propre.
    pub fn push_color(&mut self, c: Color) -> &mut Self {
        self.values.extend(to_space(c, ColorSpace::Oklab));
        self
    }

    pub fn reader(&self) -> AnimatableReader<'_> {
        AnimatableReader {
            data: &self.values,
            i: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// Relit une [`AnimatableData`] dans l'ordre où elle a été écrite.
#[derive(Debug)]
pub struct AnimatableReader<'a> {
    data: &'a [f32],
    i: usize,
}

impl AnimatableReader<'_> {
    pub fn f32(&mut self) -> f32 {
        let v = self.data.get(self.i).copied().unwrap_or(0.0);
        self.i += 1;
        v
    }

    pub fn point(&mut self) -> Point {
        Point::new(self.f32(), self.f32())
    }

    pub fn size(&mut self) -> Size {
        Size::new(self.f32(), self.f32())
    }

    pub fn color(&mut self) -> Color {
        let v = [self.f32(), self.f32(), self.f32(), self.f32()];
        from_space(v, ColorSpace::Oklab)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aller_retour() {
        let mut d = AnimatableData::new();
        d.push(2.5)
            .push_color(Color::hex(0xFF6A00))
            .push_point(Point::new(1.0, 2.0));
        assert_eq!(d.len(), 7);
        let mut e = d.clone();
        e.mark(1);
        assert!(!d.compatible(&e) && d.compatible(&d.clone()));
        let mut r = d.reader();
        assert_eq!(r.f32(), 2.5);
        let c = r.color();
        let o = Color::hex(0xFF6A00);
        assert!((c.r - o.r).abs() < 1e-3 && (c.g - o.g).abs() < 1e-3 && (c.b - o.b).abs() < 1e-3);
        assert_eq!(r.point(), Point::new(1.0, 2.0));
    }
}
