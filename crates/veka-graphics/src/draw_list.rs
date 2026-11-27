//! La liste de dessin : ce que Veka demande de dessiner, dans l'ordre de peinture.

use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use veka_foundation::{Point, Rect, Transform2D};

use crate::{Paint, Shape};

/// Identifiant d'une police dans [`DrawList::fonts`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FontId(pub u32);

/// Une police référencée par la liste : ses octets (OpenType) et l'index de la face.
#[derive(Clone)]
pub struct FontRef {
    /// Identifiant stable des données (même police = même id), pour les caches des renderers.
    pub key: u64,
    pub data: Arc<dyn AsRef<[u8]> + Send + Sync>,
    pub index: u32,
}

impl fmt::Debug for FontRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FontRef")
            .field("key", &self.key)
            .field("index", &self.index)
            .finish()
    }
}

/// Un glyphe placé : position de sa ligne de base, en points logiques.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    pub id: u32,
    pub x: f32,
    pub y: f32,
}

/// Une suite de glyphes d'une même police et d'une même taille.
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphRun {
    pub font: FontId,
    /// Taille en points logiques.
    pub size: f32,
    /// Plage dans [`DrawList::glyphs`].
    pub glyphs: Range<u32>,
    pub paint: Paint,
}

/// Effet qui lit ce qui est déjà dessiné derrière lui (flou, verre).
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum BackdropEffect {
    /// Aucun effet : sert à tester le découpage en segments (étape 1).
    None,
}

/// Une commande de dessin.
#[derive(Clone, Debug, PartialEq)]
pub enum DrawCommand {
    FillShape {
        shape: Shape,
        rect: Rect,
        paint: Paint,
    },
    StrokeShape {
        shape: Shape,
        rect: Rect,
        paint: Paint,
        width: f32,
    },
    Glyphs(GlyphRun),
    PushClip {
        shape: Shape,
        rect: Rect,
    },
    PopClip,
    PushTransform(Transform2D),
    PopTransform,
    PushLayer {
        opacity: f32,
    },
    PopLayer,
    /// Lit ce qui est déjà dessiné derrière `rect` (Liquid Glass) : coupe la liste en segments.
    Backdrop {
        shape: Shape,
        rect: Rect,
        effect: BackdropEffect,
    },
}

/// Une partie de la liste entre deux lectures de l'arrière-plan.
///
/// Le renderer dessine `commands`, puis rend le résultat lisible pour `backdrops` : tous les effets
/// d'un segment partagent **une** capture.
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub commands: Range<usize>,
    /// Index des commandes `Backdrop` qui terminent ce segment.
    pub backdrops: Range<usize>,
}

/// Tout ce qu'il faut pour dessiner une image.
#[derive(Clone, Debug, Default)]
pub struct DrawList {
    pub commands: Vec<DrawCommand>,
    pub glyphs: Vec<Glyph>,
    pub fonts: Vec<FontRef>,
}

impl DrawList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.commands.clear();
        self.glyphs.clear();
        self.fonts.clear();
    }

    pub fn push(&mut self, command: DrawCommand) {
        self.commands.push(command);
    }

    pub fn fill(&mut self, shape: Shape, rect: Rect, paint: impl Into<Paint>) {
        self.push(DrawCommand::FillShape {
            shape,
            rect,
            paint: paint.into(),
        });
    }

    pub fn stroke(&mut self, shape: Shape, rect: Rect, paint: impl Into<Paint>, width: f32) {
        self.push(DrawCommand::StrokeShape {
            shape,
            rect,
            paint: paint.into(),
            width,
        });
    }

    /// Ajoute une police (ou retrouve celle déjà ajoutée avec la même clé).
    pub fn font(&mut self, font: FontRef) -> FontId {
        if let Some(i) = self
            .fonts
            .iter()
            .position(|f| f.key == font.key && f.index == font.index)
        {
            return FontId(i as u32);
        }
        self.fonts.push(font);
        FontId(self.fonts.len() as u32 - 1)
    }

    /// Ajoute une suite de glyphes décalée de `origin`.
    pub fn glyphs(
        &mut self,
        font: FontId,
        size: f32,
        origin: Point,
        glyphs: impl IntoIterator<Item = Glyph>,
        paint: impl Into<Paint>,
    ) {
        let start = self.glyphs.len() as u32;
        self.glyphs.extend(glyphs.into_iter().map(|g| Glyph {
            id: g.id,
            x: g.x + origin.x,
            y: g.y + origin.y,
        }));
        let end = self.glyphs.len() as u32;
        if end > start {
            self.push(DrawCommand::Glyphs(GlyphRun {
                font,
                size,
                glyphs: start..end,
                paint: paint.into(),
            }));
        }
    }

    /// Découpe la liste aux commandes `Backdrop` (§7.3). Des `Backdrop` consécutifs (sans dessin entre
    /// eux) partagent le même segment, donc la même capture.
    pub fn segments(&self) -> Vec<Segment> {
        let mut out = Vec::new();
        let mut start = 0;
        let mut i = 0;
        let n = self.commands.len();
        while i < n {
            if matches!(self.commands[i], DrawCommand::Backdrop { .. }) {
                let b = i;
                while i < n && matches!(self.commands[i], DrawCommand::Backdrop { .. }) {
                    i += 1;
                }
                out.push(Segment {
                    commands: start..b,
                    backdrops: b..i,
                });
                start = i;
            } else {
                i += 1;
            }
        }
        if start < n || out.is_empty() {
            out.push(Segment {
                commands: start..n,
                backdrops: n..n,
            });
        }
        out
    }
}

/// Sortie texte stable (arrondie au centième) : sert aux tests par instantanés.
impl fmt::Display for DrawList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = |v: f32| (v * 100.0).round() / 100.0;
        let rect = |x: &Rect| format!("[{} {} {}×{}]", r(x.x()), r(x.y()), r(x.width()), r(x.height()));
        let paint = |p: &Paint| match p {
            Paint::Solid(c) => format!("rgba({} {} {} {})", r(c.r), r(c.g), r(c.b), r(c.a)),
        };
        let mut depth = 0usize;
        for c in &self.commands {
            if matches!(
                c,
                DrawCommand::PopClip | DrawCommand::PopTransform | DrawCommand::PopLayer
            ) {
                depth = depth.saturating_sub(1);
            }
            write!(f, "{}", "  ".repeat(depth))?;
            match c {
                DrawCommand::FillShape {
                    shape,
                    rect: x,
                    paint: p,
                } => writeln!(f, "fill {shape:?} {} {}", rect(x), paint(p))?,
                DrawCommand::StrokeShape {
                    shape,
                    rect: x,
                    paint: p,
                    width,
                } => writeln!(f, "stroke {shape:?} {} {} w={}", rect(x), paint(p), r(*width))?,
                DrawCommand::Glyphs(run) => {
                    let gs = &self.glyphs[run.glyphs.start as usize..run.glyphs.end as usize];
                    let first = gs.first().map(|g| (r(g.x), r(g.y))).unwrap_or_default();
                    let last = gs.last().map(|g| (r(g.x), r(g.y))).unwrap_or_default();
                    writeln!(
                        f,
                        "glyphs font={} size={} n={} first={first:?} last={last:?} {}",
                        run.font.0,
                        r(run.size),
                        gs.len(),
                        paint(&run.paint)
                    )?
                }
                DrawCommand::PushClip { shape, rect: x } => writeln!(f, "clip {shape:?} {}", rect(x))?,
                DrawCommand::PushTransform(t) => writeln!(f, "transform {t:?}")?,
                DrawCommand::PushLayer { opacity } => writeln!(f, "layer opacity={}", r(*opacity))?,
                DrawCommand::PopClip | DrawCommand::PopTransform | DrawCommand::PopLayer => {
                    writeln!(f, "end")?
                }
                DrawCommand::Backdrop {
                    shape,
                    rect: x,
                    effect,
                } => writeln!(f, "backdrop {shape:?} {} {effect:?}", rect(x))?,
            }
            if matches!(
                c,
                DrawCommand::PushClip { .. } | DrawCommand::PushTransform(_) | DrawCommand::PushLayer { .. }
            ) {
                depth += 1;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use veka_foundation::Color;

    fn backdrop() -> DrawCommand {
        DrawCommand::Backdrop {
            shape: Shape::Capsule,
            rect: Rect::ZERO,
            effect: BackdropEffect::None,
        }
    }

    #[test]
    fn one_segment_without_backdrop() {
        let mut l = DrawList::new();
        l.fill(Shape::Rectangle, Rect::new(0.0, 0.0, 1.0, 1.0), Color::Red);
        assert_eq!(
            l.segments(),
            vec![Segment {
                commands: 0..1,
                backdrops: 1..1
            }]
        );
    }

    #[test]
    fn backdrops_split_and_share_a_capture() {
        let mut l = DrawList::new();
        l.fill(Shape::Rectangle, Rect::ZERO, Color::Red); // 0
        l.push(backdrop()); // 1
        l.push(backdrop()); // 2 : même capture que 1
        l.fill(Shape::Circle, Rect::ZERO, Color::Blue); // 3
        l.push(backdrop()); // 4 : nouvelle capture (du contenu a été dessiné depuis)
        assert_eq!(
            l.segments(),
            vec![
                Segment {
                    commands: 0..1,
                    backdrops: 1..3
                },
                Segment {
                    commands: 3..4,
                    backdrops: 4..5
                },
            ]
        );
    }

    #[test]
    fn fonts_are_deduplicated() {
        let mut l = DrawList::new();
        let data: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(vec![0u8; 4]);
        let a = l.font(FontRef {
            key: 7,
            data: data.clone(),
            index: 0,
        });
        let b = l.font(FontRef {
            key: 7,
            data,
            index: 0,
        });
        assert_eq!(a, b);
        assert_eq!(l.fonts.len(), 1);
    }

    #[test]
    fn display_is_readable() {
        let mut l = DrawList::new();
        l.push(DrawCommand::PushLayer { opacity: 0.5 });
        l.fill(Shape::Capsule, Rect::new(1.0, 2.0, 3.0, 4.0), Color::White);
        l.push(DrawCommand::PopLayer);
        assert_eq!(
            l.to_string(),
            "layer opacity=0.5\n  fill Capsule [1 2 3×4] rgba(1 1 1 1)\nend\n"
        );
    }
}
