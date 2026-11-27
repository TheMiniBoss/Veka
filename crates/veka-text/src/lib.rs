//! # Veka Text
//!
//! Polices et mise en page du texte (avec [Parley]). Le résultat, [`TextLayout`], donne la taille du
//! texte et ses glyphes placés ; il s'ajoute à une [`DrawList`] sans rien rastériser : chaque renderer
//! dessine les glyphes à sa façon.
//!
//! Aucune police du système n'est lue : les polices sont fournies en octets (par défaut, LT Flode Neue
//! est embarquée). Les polices du système passeront plus tard par la couche Platform.
//!
//! [Parley]: https://github.com/linebender/parley

use std::sync::Arc;

use parley::fontique::{Blob, Collection, CollectionOptions, SourceCache};
use parley::{FontContext, FontFamily, FontWeight, LayoutContext, PositionedLayoutItem, StyleProperty};
use veka_foundation::{Color, Point, Size};
use veka_graphics::{DrawList, FontRef, Glyph};

/// Nom de la famille embarquée par défaut.
pub const DEFAULT_FAMILY: &str = "LT Flode Neue";

/// Graisse, de 1 à 1000 (400 = normal, 700 = gras).
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Weight(pub f32);

#[allow(non_upper_case_globals)]
impl Weight {
    pub const Thin: Weight = Weight(100.0);
    pub const ExtraLight: Weight = Weight(200.0);
    pub const Light: Weight = Weight(300.0);
    pub const Regular: Weight = Weight(400.0);
    pub const Medium: Weight = Weight(500.0);
    pub const SemiBold: Weight = Weight(600.0);
    pub const Bold: Weight = Weight(700.0);
    pub const ExtraBold: Weight = Weight(800.0);
    pub const Black: Weight = Weight(900.0);
}

impl Default for Weight {
    fn default() -> Self {
        Weight::Regular
    }
}

/// Style d'un texte.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    /// Famille ; `None` = famille par défaut du [`TextSystem`].
    pub family: Option<Arc<str>>,
    /// Taille en points logiques.
    pub size: f32,
    pub weight: Weight,
    pub color: Color,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self { family: None, size: 17.0, weight: Weight::Regular, color: Color::Black }
    }
}

/// Une suite de glyphes d'une police, placés par rapport au coin haut-gauche du texte.
#[derive(Clone, Debug)]
pub struct TextRun {
    pub font: FontRef,
    pub size: f32,
    pub color: Color,
    pub glyphs: Vec<Glyph>,
}

/// Un texte mis en page.
#[derive(Clone, Debug, Default)]
pub struct TextLayout {
    /// Taille occupée (largeur de la ligne la plus longue × hauteur des lignes).
    pub size: Size,
    /// Ligne de base de la première ligne, depuis le haut.
    pub first_baseline: f32,
    pub line_count: usize,
    pub runs: Vec<TextRun>,
}

impl TextLayout {
    /// Ajoute les glyphes à la liste, le coin haut-gauche du texte en `origin`.
    pub fn draw(&self, list: &mut DrawList, origin: Point) {
        for run in &self.runs {
            let font = list.font(run.font.clone());
            list.glyphs(font, run.size, origin, run.glyphs.iter().copied(), run.color);
        }
    }
}

/// Polices chargées + contextes de mise en page. Un par thread d'interface.
pub struct TextSystem {
    fonts: FontContext,
    layout: LayoutContext<Color>,
    default_family: Arc<str>,
}

impl std::fmt::Debug for TextSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextSystem").field("default_family", &self.default_family).finish()
    }
}

impl Default for TextSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl TextSystem {
    /// Avec les polices embarquées si la fonctionnalité `default-fonts` est active, sinon vide.
    pub fn new() -> Self {
        let mut system = Self::empty();
        #[cfg(feature = "default-fonts")]
        for data in default_fonts::ALL {
            system.register_font(data);
        }
        system
    }

    /// Sans aucune police.
    pub fn empty() -> Self {
        let collection = Collection::new(CollectionOptions { shared: false, system_fonts: false });
        Self {
            fonts: FontContext { collection, source_cache: SourceCache::default() },
            layout: LayoutContext::new(),
            default_family: DEFAULT_FAMILY.into(),
        }
    }

    /// Ajoute une police (OpenType / TrueType, collections comprises) ; renvoie les familles trouvées.
    pub fn register_font(&mut self, data: &'static [u8]) -> Vec<String> {
        self.register_font_data(Arc::new(data))
    }

    /// Comme [`register_font`](Self::register_font), pour des octets chargés à l'exécution.
    pub fn register_font_data(&mut self, data: Arc<dyn AsRef<[u8]> + Send + Sync>) -> Vec<String> {
        let families = self.fonts.collection.register_fonts(Blob::new(data), None);
        families
            .iter()
            .filter_map(|(id, _)| self.fonts.collection.family_name(*id).map(str::to_owned))
            .collect()
    }

    pub fn set_default_family(&mut self, family: impl Into<Arc<str>>) {
        self.default_family = family.into();
    }

    /// Met en page `text`. Avec `max_width`, le texte revient à la ligne pour tenir dans la largeur.
    pub fn layout(&mut self, text: &str, style: &TextStyle, max_width: Option<f32>) -> TextLayout {
        let family = style.family.clone().unwrap_or_else(|| self.default_family.clone());
        let mut builder = self.layout.ranged_builder(&mut self.fonts, text, 1.0, false);
        builder.push_default(StyleProperty::FontFamily(FontFamily::named(&family)));
        builder.push_default(StyleProperty::FontSize(style.size));
        builder.push_default(StyleProperty::FontWeight(FontWeight::new(style.weight.0)));
        builder.push_default(StyleProperty::Brush(style.color));
        let mut layout: parley::Layout<Color> = builder.build(text);
        layout.break_all_lines(max_width);

        let mut out = TextLayout {
            size: Size::new(layout.width(), layout.height()),
            first_baseline: layout.first_baseline().unwrap_or(0.0),
            line_count: layout.len(),
            runs: Vec::new(),
        };
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(run) = item else { continue };
                let font = run.run().font();
                let (data, key) = font.data.clone().into_raw_parts();
                out.runs.push(TextRun {
                    font: FontRef { key, data, index: font.index },
                    size: run.run().font_size(),
                    color: run.style().brush,
                    glyphs: run.positioned_glyphs().map(|g| Glyph { id: g.id, x: g.x, y: g.y }).collect(),
                });
            }
        }
        out
    }
}

#[cfg(feature = "default-fonts")]
mod default_fonts {
    macro_rules! font {
        ($name:literal) => {
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fonts/LTFlodeNeue/LTFlodeNeue-", $name, ".otf"))
                as &'static [u8]
        };
    }

    /// Les graisses droites de LT Flode Neue (licence SIL Open Font License).
    pub const ALL: &[&[u8]] = &[
        font!("Hairline"),
        font!("Thin"),
        font!("ExtraLight"),
        font!("Light"),
        font!("Regular"),
        font!("News"),
        font!("Medium"),
        font!("SemiBold"),
        font!("Bold"),
        font!("ExtraBold"),
        font!("Black"),
        font!("ExtraBlack"),
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_family_is_registered() {
        let mut s = TextSystem::empty();
        let fams = s.register_font(default_fonts::ALL[4]);
        assert_eq!(fams, vec![DEFAULT_FAMILY.to_string()]);
    }

    #[test]
    fn measures_a_line() {
        let mut s = TextSystem::new();
        let l = s.layout("Bonjour Veka", &TextStyle { size: 28.0, ..Default::default() }, None);
        assert_eq!(l.line_count, 1);
        assert!(l.size.width > 100.0 && l.size.width < 250.0, "largeur {}", l.size.width);
        assert!(l.size.height > 28.0 && l.size.height < 45.0, "hauteur {}", l.size.height);
        assert!(l.first_baseline > 0.0 && l.first_baseline < l.size.height);
        let n: usize = l.runs.iter().map(|r| r.glyphs.len()).sum();
        assert_eq!(n, 12);
    }

    #[test]
    fn is_deterministic() {
        let mut a = TextSystem::new();
        let mut b = TextSystem::new();
        let st = TextStyle::default();
        assert_eq!(a.layout("Clics : 42", &st, None).size, b.layout("Clics : 42", &st, None).size);
    }

    #[test]
    fn bold_is_wider() {
        let mut s = TextSystem::new();
        let regular = s.layout("Veka", &TextStyle::default(), None).size.width;
        let bold = s.layout("Veka", &TextStyle { weight: Weight::Black, ..Default::default() }, None).size.width;
        assert!(bold > regular);
    }

    #[test]
    fn wraps_to_max_width() {
        let mut s = TextSystem::new();
        let st = TextStyle::default();
        let one = s.layout("Bonjour tout le monde", &st, None);
        let wrapped = s.layout("Bonjour tout le monde", &st, Some(one.size.width * 0.6));
        assert!(wrapped.line_count >= 2);
        assert!(wrapped.size.width <= one.size.width * 0.6 + 0.5);
        assert!(wrapped.size.height > one.size.height);
    }

    #[test]
    fn accents_have_glyphs() {
        let mut s = TextSystem::new();
        let l = s.layout("éàçôü", &TextStyle::default(), None);
        assert!(l.runs.iter().flat_map(|r| &r.glyphs).all(|g| g.id != 0), "pas de glyphe manquant");
    }

    #[test]
    fn draws_into_list() {
        let mut s = TextSystem::new();
        let l = s.layout("Hi", &TextStyle::default(), None);
        let mut list = DrawList::new();
        l.draw(&mut list, Point::new(10.0, 20.0));
        assert_eq!(list.fonts.len(), 1);
        assert_eq!(list.glyphs.len(), 2);
        assert!(list.glyphs[0].x >= 10.0 && list.glyphs[0].y > 20.0);
    }
}
