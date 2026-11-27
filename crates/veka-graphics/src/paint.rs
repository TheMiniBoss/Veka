//! Peintures. Colora ajoutera les dégradés (linéaire, radial, angulaire, maillé).

use veka_foundation::Color;

/// Ce qui remplit une forme, un trait ou du texte.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Paint {
    Solid(Color),
}

impl From<Color> for Paint {
    fn from(c: Color) -> Self {
        Paint::Solid(c)
    }
}
