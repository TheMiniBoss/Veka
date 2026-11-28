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
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum BackdropEffect {
    /// Aucun effet : sert à tester le découpage en segments (étape 1).
    None,
    /// Flou de l'arrière-plan, teinté, découpé à la forme (`backdrop_blur`, `frosted_glass`).
    Blur(BlurEffect),
    /// Verre liquide : réfraction au bord, couleurs séparées, reflet (`glass`, `glass_group`).
    /// En boîte : ses formes alourdiraient toutes les commandes de la liste.
    Glass(Box<GlassEffect>),
    /// Forme d'un verre qui appartient à un `glass_group` : le groupe la reprend et retire la commande
    /// (un renderer ne la voit jamais).
    GlassMember,
}

/// Une forme d'un verre (dans le repère de la commande).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlassShape {
    pub shape: Shape,
    pub rect: Rect,
}

/// Formes d'un groupe de verre au plus.
pub const MAX_GLASS_SHAPES: usize = 8;

/// Réglages d'un verre liquide. Les longueurs sont en points logiques.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlassEffect {
    /// Léger flou de ce qu'on voit à travers.
    pub blur: f32,
    /// Déplacement du fond tout au bord.
    pub refraction: f32,
    /// Largeur du bord courbe (réfraction et reflet).
    pub bezel: f32,
    /// Séparation des couleurs de la réfraction, de 0 à 1.
    pub aberration: f32,
    /// Reflet spéculaire du bord, de 0 à 1.
    pub highlight: f32,
    /// Anneau sombre juste à l'intérieur du bord (Liquid Glass 27), de 0 à 1.
    pub rim_dark: f32,
    /// Lumière propre au verre, du haut vers le bas (27), de 0 à 1.
    pub inner_light: f32,
    pub tint: veka_foundation::Color,
    /// Part de la teinte, de 0 à 1.
    pub tint_amount: f32,
    pub saturation: f32,
    /// Opacité de l'ombre portée (0 : aucune).
    pub shadow: f32,
    /// Rayon de l'ombre.
    pub shadow_radius: f32,
    /// Le reflet suit le pointeur ([`DrawList::pointer`]) ; sinon lumière fixe en haut à gauche.
    pub interactive: bool,
    /// Qualité réduite : un seul niveau de flou, sans séparation des couleurs.
    pub low_quality: bool,
    /// Fusion des formes d'un groupe (minimum lisse), en points.
    pub smoothness: f32,
    /// Les formes d'un groupe ; `count == 0` : la forme de la commande.
    pub shapes: [GlassShape; MAX_GLASS_SHAPES],
    pub count: u8,
}

impl Default for GlassEffect {
    fn default() -> Self {
        Self {
            blur: 3.0,
            refraction: 24.0,
            bezel: 20.0,
            aberration: 0.35,
            highlight: 0.75,
            rim_dark: 0.35,
            inner_light: 0.5,
            tint: veka_foundation::Color::White,
            tint_amount: 0.08,
            saturation: 1.25,
            shadow: 0.18,
            shadow_radius: 14.0,
            interactive: false,
            low_quality: false,
            smoothness: 0.0,
            shapes: [GlassShape {
                shape: Shape::Rectangle,
                rect: Rect::ZERO,
            }; MAX_GLASS_SHAPES],
            count: 0,
        }
    }
}

impl GlassEffect {
    /// Les formes du verre : celles du groupe, sinon `(shape, rect)` de la commande.
    pub fn shapes_or(&self, shape: Shape, rect: Rect) -> Vec<GlassShape> {
        if self.count == 0 {
            vec![GlassShape { shape, rect }]
        } else {
            self.shapes[..(self.count as usize).min(MAX_GLASS_SHAPES)].to_vec()
        }
    }
}

/// Réglages d'un flou d'arrière-plan. Les longueurs sont en points logiques.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlurEffect {
    /// Rayon du flou.
    pub radius: f32,
    /// Couleur mélangée au flou (déjà résolue par Colora).
    pub tint: veka_foundation::Color,
    /// Part de la teinte, de 0 à 1.
    pub tint_amount: f32,
    /// 1 = inchangée ; au-dessus, couleurs plus vives.
    pub saturation: f32,
    /// Grain (verre dépoli), de 0 à 1.
    pub noise: f32,
    /// Qualité réduite : un seul niveau de flou, sans mélange ni grain.
    pub low_quality: bool,
    /// Ondulation du fond (`liquid_distortion`) : déplacement en points (0 : aucune).
    pub distortion: f32,
    /// Taille des vagues, en points.
    pub wave: f32,
    /// Temps de l'ondulation, en secondes.
    pub time: f32,
}

impl Default for BlurEffect {
    fn default() -> Self {
        Self {
            radius: 20.0,
            tint: veka_foundation::Color::White,
            tint_amount: 0.15,
            saturation: 1.4,
            noise: 0.0,
            low_quality: false,
            distortion: 0.0,
            wave: 120.0,
            time: 0.0,
        }
    }
}

impl BackdropEffect {
    /// Jusqu'où l'effet lit autour de sa forme, en points (le flou déborde).
    pub fn reach(&self) -> f32 {
        match self {
            BackdropEffect::None | BackdropEffect::GlassMember => 0.0,
            BackdropEffect::Blur(b) => b.radius.max(0.0) * 2.0 + b.distortion.max(0.0) * 1.5,
            BackdropEffect::Glass(g) => {
                g.blur.max(0.0) * 2.0
                    + g.refraction.abs() * (1.0 + g.aberration.abs())
                    + g.shadow_radius.max(0.0) * 1.5
            }
        }
    }
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
    /// Début d'un **calque** (étape 8) : le sous-arbre est dessiné à part, puis composé d'un bloc sur ce
    /// qui est derrière avec `blend` et `opacity`. Sert à `blend_mode` et à `compositing_group`.
    PushGroup {
        blend: crate::BlendMode,
        opacity: f32,
    },
    PopGroup,
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
    /// Position du pointeur (points), pour les verres `interactive`.
    pub pointer: Option<Point>,
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

    /// Lit ce qui est déjà dessiné derrière `rect` et y applique `effect`, découpé à `shape`.
    pub fn backdrop(&mut self, shape: Shape, rect: Rect, effect: BackdropEffect) {
        self.push(DrawCommand::Backdrop { shape, rect, effect });
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

    /// Les zones où quelque chose est dessiné, en points : au plus `max` rectangles qui couvrent tout le
    /// dessin (en le débordant un peu). Une fenêtre transparente laisse passer les clics ailleurs.
    pub fn painted_bounds(&self, max: usize) -> Vec<Rect> {
        let max = max.max(1);
        let mut out: Vec<Rect> = Vec::new();
        let mut transforms = vec![veka_foundation::Transform2D::IDENTITY];
        let mut clips: Vec<Option<Rect>> = vec![None];
        let bounds = |t: &veka_foundation::Transform2D, r: &Rect, m: f32| {
            let (x0, y0, x1, y1) = (r.x() - m, r.y() - m, r.max_x() + m, r.max_y() + m);
            let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
            for (x, y) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
                let p = t.apply(Point::new(x, y));
                b = [b[0].min(p.x), b[1].min(p.y), b[2].max(p.x), b[3].max(p.y)];
            }
            Rect::new(b[0], b[1], b[2] - b[0], b[3] - b[1])
        };
        for c in &self.commands {
            let t = *transforms.last().expect("pile");
            let clip = *clips.last().expect("pile");
            let r = match c {
                // Une couleur entièrement transparente ne dessine rien (fond d'une fenêtre transparente).
                DrawCommand::FillShape {
                    paint: Paint::Solid(c),
                    ..
                } if c.a <= 0.0 => None,
                DrawCommand::FillShape { rect, .. } => Some(bounds(&t, rect, 1.0)),
                DrawCommand::StrokeShape { rect, width, .. } => Some(bounds(&t, rect, 1.0 + width * 0.5)),
                DrawCommand::Glyphs(run) => {
                    let gs = &self.glyphs[run.glyphs.start as usize..run.glyphs.end as usize];
                    gs.iter()
                        .map(|g| Rect::new(g.x, g.y - run.size, run.size, run.size * 1.4))
                        .reduce(|a, b| a.union(&b))
                        .map(|r| bounds(&t, &r, 1.0))
                }
                DrawCommand::Backdrop { rect, effect, .. } => Some(bounds(&t, rect, 1.0 + effect.reach())),
                DrawCommand::PushClip { rect, .. } => {
                    let r = bounds(&t, rect, 0.0);
                    clips.push(Some(match clip {
                        Some(c) => c.intersection(&r).unwrap_or(Rect::ZERO),
                        None => r,
                    }));
                    None
                }
                DrawCommand::PopClip => {
                    if clips.len() > 1 {
                        clips.pop();
                    }
                    None
                }
                DrawCommand::PushTransform(m) => {
                    transforms.push(m.then(&t));
                    None
                }
                DrawCommand::PopTransform => {
                    if transforms.len() > 1 {
                        transforms.pop();
                    }
                    None
                }
                DrawCommand::PushLayer { .. }
                | DrawCommand::PopLayer
                | DrawCommand::PushGroup { .. }
                | DrawCommand::PopGroup => None,
            };
            let Some(r) = r else { continue };
            let Some(r) = (match clip {
                Some(c) => c.intersection(&r),
                None => Some(r),
            }) else {
                continue;
            };
            if r.width() <= 0.0 || r.height() <= 0.0 {
                continue;
            }
            add_rect(&mut out, r, max);
        }
        out
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

/// Ajoute `r` aux zones : fondu dans une zone qu'il touche, et au-delà de `max` zones, les deux dont
/// l'union grandit le moins sont fondues.
fn add_rect(out: &mut Vec<Rect>, r: Rect, max: usize) {
    let near = |a: &Rect, b: &Rect| {
        a.x() <= b.max_x() + 4.0
            && b.x() <= a.max_x() + 4.0
            && a.y() <= b.max_y() + 4.0
            && b.y() <= a.max_y() + 4.0
    };
    let mut r = r;
    // Une zone agrandie peut en toucher d'autres : on recommence jusqu'à ce qu'elle soit seule.
    while let Some(i) = out.iter().position(|o| near(o, &r)) {
        r = r.union(&out.swap_remove(i));
    }
    out.push(r);
    while out.len() > max {
        let area = |r: &Rect| r.width() * r.height();
        let mut best = (0, 1, f32::MAX);
        for i in 0..out.len() {
            for j in i + 1..out.len() {
                let cost = area(&out[i].union(&out[j])) - area(&out[i]) - area(&out[j]);
                if cost < best.2 {
                    best = (i, j, cost);
                }
            }
        }
        let b = out.swap_remove(best.1);
        out[best.0] = out[best.0].union(&b);
    }
}

/// Sortie texte stable (arrondie au centième) : sert aux tests par instantanés.
impl fmt::Display for DrawList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = |v: f32| (v * 100.0).round() / 100.0;
        let rect = |x: &Rect| format!("[{} {} {}×{}]", r(x.x()), r(x.y()), r(x.width()), r(x.height()));
        let color = |c: &veka_foundation::Color| format!("rgba({} {} {} {})", r(c.r), r(c.g), r(c.b), r(c.a));
        let stops = |g: &veka_colora::Gradient| {
            g.stops
                .iter()
                .map(|s| match &s.ink {
                    veka_colora::Ink::Color(c) => format!("{}@{}", color(c), r(s.location)),
                    veka_colora::Ink::Token(t, _) => format!("{}@{}", t.name(), r(s.location)),
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        let paint = |p: &Paint| match p {
            Paint::Solid(c) => color(c),
            Paint::Token(t, h) => format!("token({} {h:?})", t.name()),
            Paint::Linear(g) => format!(
                "linear({} {} → {} {}; {})",
                r(g.start.x),
                r(g.start.y),
                r(g.end.x),
                r(g.end.y),
                stops(&g.gradient)
            ),
            Paint::Radial(g) => format!(
                "radial({} {} r {}→{}; {})",
                r(g.center.x),
                r(g.center.y),
                r(g.start_radius),
                r(g.end_radius),
                stops(&g.gradient)
            ),
            Paint::Elliptical(g) => format!(
                "elliptical({} {} f {}→{}; {})",
                r(g.center.x),
                r(g.center.y),
                r(g.start_fraction),
                r(g.end_fraction),
                stops(&g.gradient)
            ),
            Paint::Angular(g) => format!(
                "angular({} {} {}°→{}°; {})",
                r(g.center.x),
                r(g.center.y),
                r(g.start_angle),
                r(g.end_angle),
                stops(&g.gradient)
            ),
            Paint::Mesh(m) => format!("mesh({}×{})", m.width, m.height),
        };
        let mut depth = 0usize;
        for c in &self.commands {
            if matches!(
                c,
                DrawCommand::PopClip
                    | DrawCommand::PopTransform
                    | DrawCommand::PopLayer
                    | DrawCommand::PopGroup
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
                DrawCommand::PushGroup { blend, opacity } => {
                    writeln!(f, "group {} opacity={}", blend.name(), r(*opacity))?
                }
                DrawCommand::PopClip
                | DrawCommand::PopTransform
                | DrawCommand::PopLayer
                | DrawCommand::PopGroup => writeln!(f, "end")?,
                DrawCommand::Backdrop {
                    shape,
                    rect: x,
                    effect,
                } => writeln!(f, "backdrop {shape:?} {} {effect:?}", rect(x))?,
            }
            if matches!(
                c,
                DrawCommand::PushClip { .. }
                    | DrawCommand::PushTransform(_)
                    | DrawCommand::PushLayer { .. }
                    | DrawCommand::PushGroup { .. }
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
    fn painted_bounds_cover_the_drawing() {
        let mut l = DrawList::new();
        l.fill(Shape::Rectangle, Rect::new(10.0, 10.0, 100.0, 50.0), Color::Red);
        l.fill(Shape::Circle, Rect::new(20.0, 20.0, 10.0, 10.0), Color::Blue); // dans le premier
        l.fill(Shape::Rectangle, Rect::new(300.0, 10.0, 20.0, 20.0), Color::Red); // à part
        l.push(DrawCommand::PushTransform(
            veka_foundation::Transform2D::translation(0.0, 200.0),
        ));
        l.fill(Shape::Rectangle, Rect::new(10.0, 0.0, 10.0, 10.0), Color::Red);
        l.push(DrawCommand::PopTransform);
        l.push(DrawCommand::PushClip {
            shape: Shape::Rectangle,
            rect: Rect::new(500.0, 0.0, 10.0, 10.0),
        });
        l.fill(Shape::Rectangle, Rect::new(0.0, 0.0, 1000.0, 1000.0), Color::Red); // coupé
        l.push(DrawCommand::PopClip);
        let b = l.painted_bounds(8);
        assert_eq!(b.len(), 4, "{b:?}");
        assert!(b.contains(&Rect::new(9.0, 9.0, 102.0, 52.0)), "{b:?}");
        assert!(b.contains(&Rect::new(9.0, 199.0, 12.0, 12.0)), "{b:?}");
        assert!(b.contains(&Rect::new(500.0, 0.0, 10.0, 10.0)), "{b:?}");
        // Au plus `max` zones, qui couvrent toujours tout.
        let b = l.painted_bounds(2);
        assert_eq!(b.len(), 2);
        assert!(b.iter().any(|r| r.contains(Point::new(505.0, 5.0))));
        assert!(b.iter().any(|r| r.contains(Point::new(15.0, 205.0))));
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
