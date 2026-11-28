//! Ce qui remplit une forme : couleur, jeton du thème ou dégradé.

use std::borrow::Cow;

use veka_foundation::{Color, Point, Rect};

use crate::space::{ColorSpace, from_space, lerp_in_space, to_space};

/// Un point dans le rectangle d'une forme : (0, 0) en haut à gauche, (1, 1) en bas à droite.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UnitPoint {
    pub x: f32,
    pub y: f32,
}

impl UnitPoint {
    pub const TOP_LEADING: UnitPoint = UnitPoint::new(0.0, 0.0);
    pub const TOP: UnitPoint = UnitPoint::new(0.5, 0.0);
    pub const TOP_TRAILING: UnitPoint = UnitPoint::new(1.0, 0.0);
    pub const LEADING: UnitPoint = UnitPoint::new(0.0, 0.5);
    pub const CENTER: UnitPoint = UnitPoint::new(0.5, 0.5);
    pub const TRAILING: UnitPoint = UnitPoint::new(1.0, 0.5);
    pub const BOTTOM_LEADING: UnitPoint = UnitPoint::new(0.0, 1.0);
    pub const BOTTOM: UnitPoint = UnitPoint::new(0.5, 1.0);
    pub const BOTTOM_TRAILING: UnitPoint = UnitPoint::new(1.0, 1.0);

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Le point dans `rect`, en points.
    pub fn in_rect(self, rect: &Rect) -> Point {
        Point::new(
            rect.x() + self.x * rect.width(),
            rect.y() + self.y * rect.height(),
        )
    }
}

/// Niveaux d'alpha de Colora (Alpha 13) : 100 %, 60 %, 30 %, 18 %.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Hierarchy {
    #[default]
    Primary,
    Secondary,
    Tertiary,
    Quaternary,
}

impl Hierarchy {
    pub fn alpha(self) -> f32 {
        match self {
            Hierarchy::Primary => 1.0,
            Hierarchy::Secondary => 0.6,
            Hierarchy::Tertiary => 0.3,
            Hierarchy::Quaternary => 0.18,
        }
    }
}

/// Une couleur sémantique du thème (« jeton »). Les jetons de Veka sont des constantes
/// (`Token::Accent`…) ; `Token::custom("Marque.Fond")` pour les siens.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Token(Cow<'static, str>);

macro_rules! tokens {
    ($($name:ident),* $(,)?) => {
        #[allow(non_upper_case_globals)]
        impl Token {
            $(pub const $name: Token = Token(Cow::Borrowed(stringify!($name)));)*
        }
    };
}

tokens!(
    // Couleurs de la palette système.
    Accent,
    Black,
    Blue,
    Brown,
    Clear,
    Cyan,
    Gray,
    Green,
    Indigo,
    Mint,
    Orange,
    Pink,
    Purple,
    Red,
    Teal,
    White,
    Yellow,
    // Couleurs de sens.
    Tint,
    Background,
    WindowBackground,
    Foreground,
    Separator,
    Selection,
    Placeholder,
    Link,
    Fill,
    // Couleur d'accent de l'OS (sinon `Tint`).
    SystemAccent,
    // Jetons de contrôle : renvoient à un autre jeton tant qu'aucun thème ne les fixe.
    ThemeAccent,
    ThemeBackground,
    ThemeText,
    ButtonAccent,
    ButtonText,
    ButtonBackground,
    ToggleAccent,
    ToggleBackground,
    SliderAccent,
    SliderBackground,
    SliderKnob,
    SliderText,
    ProgressAccent,
    ProgressBackground,
    GaugeAccent,
    GaugeBackground,
    MenuAccent,
    MenuBackground,
    PickerAccent,
    PickerBackground,
    TextFieldAccent,
    TextFieldBackground,
    TextFieldText,
);

impl Token {
    /// Un jeton à soi.
    pub fn custom(name: impl Into<String>) -> Self {
        Token(Cow::Owned(name.into()))
    }

    /// Un jeton par son nom (même valeur que la constante s'il s'agit d'un jeton de Veka).
    pub fn named(name: &str) -> Self {
        Token(Cow::Owned(name.to_owned()))
    }

    pub fn name(&self) -> &str {
        &self.0
    }

    pub fn secondary(self) -> Ink {
        Ink::Token(self, Hierarchy::Secondary)
    }

    pub fn tertiary(self) -> Ink {
        Ink::Token(self, Hierarchy::Tertiary)
    }

    pub fn quaternary(self) -> Ink {
        Ink::Token(self, Hierarchy::Quaternary)
    }

    pub fn hierarchy(self, h: Hierarchy) -> Ink {
        Ink::Token(self, h)
    }
}

/// Une couleur ou un jeton : la couleur d'un stop de dégradé ou d'un point de dégradé maillé.
#[derive(Clone, Debug, PartialEq)]
pub enum Ink {
    Color(Color),
    Token(Token, Hierarchy),
}

impl From<Color> for Ink {
    fn from(c: Color) -> Self {
        Ink::Color(c)
    }
}

impl From<Token> for Ink {
    fn from(t: Token) -> Self {
        Ink::Token(t, Hierarchy::Primary)
    }
}

impl Ink {
    /// La couleur, si elle n'est pas un jeton.
    pub fn color(&self) -> Option<Color> {
        match self {
            Ink::Color(c) => Some(*c),
            Ink::Token(..) => None,
        }
    }
}

/// Un arrêt de dégradé : une couleur à une position (0 à 1).
#[derive(Clone, Debug, PartialEq)]
pub struct Stop {
    pub ink: Ink,
    pub location: f32,
}

impl Stop {
    pub fn new(ink: impl Into<Ink>, location: f32) -> Self {
        Self {
            ink: ink.into(),
            location,
        }
    }
}

/// Les arrêts d'un dégradé et l'espace dans lequel il interpole.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Gradient {
    pub stops: Vec<Stop>,
    pub space: ColorSpace,
}

impl Gradient {
    /// Couleurs réparties régulièrement de 0 à 1.
    pub fn new<I: Into<Ink>>(inks: impl IntoIterator<Item = I>) -> Self {
        let inks: Vec<Ink> = inks.into_iter().map(Into::into).collect();
        let n = inks.len();
        let stops = inks
            .into_iter()
            .enumerate()
            .map(|(i, ink)| Stop {
                ink,
                location: if n > 1 { i as f32 / (n - 1) as f32 } else { 0.0 },
            })
            .collect();
        Self {
            stops,
            space: ColorSpace::default(),
        }
    }

    pub fn with_stops(stops: impl IntoIterator<Item = Stop>) -> Self {
        Self {
            stops: stops.into_iter().collect(),
            space: ColorSpace::default(),
        }
    }

    /// Couleur en `t` (0 à 1). Les stops doivent être résolus (un jeton compte comme transparent).
    pub fn sample(&self, t: f32) -> Color {
        let color = |s: &Stop| s.ink.color().unwrap_or(Color::Clear);
        let Some(first) = self.stops.first() else {
            return Color::Clear;
        };
        let last = self.stops.last().expect("au moins un stop");
        if self.stops.len() == 1 || t <= first.location {
            return color(first);
        }
        if t >= last.location {
            return color(last);
        }
        for w in self.stops.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            if t <= b.location {
                let span = b.location - a.location;
                let u = if span <= 1e-6 {
                    1.0
                } else {
                    (t - a.location) / span
                };
                let va = to_space(color(a), self.space);
                let vb = to_space(color(b), self.space);
                return from_space(lerp_in_space(va, vb, u, self.space), self.space);
            }
        }
        color(self.stops.last().expect("au moins un stop"))
    }

    /// Couleur moyenne (pour un texte ou un contour qui ne sait pas encore dessiner un dégradé).
    pub fn average(&self) -> Color {
        average((0..=16).map(|i| self.sample(i as f32 / 16.0)), self.space)
    }

    fn map(&self, f: &mut impl FnMut(&Ink) -> Ink) -> Gradient {
        Gradient {
            stops: self
                .stops
                .iter()
                .map(|s| Stop {
                    ink: f(&s.ink),
                    location: s.location,
                })
                .collect(),
            space: self.space,
        }
    }
}

fn average(colors: impl Iterator<Item = Color>, space: ColorSpace) -> Color {
    // Moyenne de teintes : pas de sens simple, on moyenne en Oklab.
    let space = if space == ColorSpace::OkLch {
        ColorSpace::Oklab
    } else {
        space
    };
    let mut sum = [0.0f32; 4];
    let mut n = 0.0;
    for c in colors {
        let v = to_space(c, space);
        for (s, x) in sum.iter_mut().zip(v) {
            *s += x;
        }
        n += 1.0;
    }
    if n == 0.0 {
        return Color::Clear;
    }
    from_space(sum.map(|s| s / n), space)
}

macro_rules! gradient_builders {
    ($t:ty) => {
        impl $t {
            /// Espace d'interpolation (Oklab par défaut).
            pub fn space(mut self, space: ColorSpace) -> Self {
                self.gradient.space = space;
                self
            }
        }
    };
}

/// Dégradé linéaire de `start` à `end`.
#[derive(Clone, Debug, PartialEq)]
pub struct LinearGradient {
    pub gradient: Gradient,
    pub start: UnitPoint,
    pub end: UnitPoint,
}

impl LinearGradient {
    /// De gauche à droite, couleurs réparties régulièrement.
    pub fn new<I: Into<Ink>>(inks: impl IntoIterator<Item = I>) -> Self {
        Self::with(Gradient::new(inks))
    }

    pub fn with(gradient: Gradient) -> Self {
        Self {
            gradient,
            start: UnitPoint::LEADING,
            end: UnitPoint::TRAILING,
        }
    }

    pub fn from(mut self, start: UnitPoint) -> Self {
        self.start = start;
        self
    }

    pub fn to(mut self, end: UnitPoint) -> Self {
        self.end = end;
        self
    }
}

/// Dégradé en cercles autour de `center`, rayons en points.
#[derive(Clone, Debug, PartialEq)]
pub struct RadialGradient {
    pub gradient: Gradient,
    pub center: UnitPoint,
    pub start_radius: f32,
    pub end_radius: f32,
}

impl RadialGradient {
    pub fn new<I: Into<Ink>>(inks: impl IntoIterator<Item = I>, end_radius: f32) -> Self {
        Self::with(Gradient::new(inks), end_radius)
    }

    pub fn with(gradient: Gradient, end_radius: f32) -> Self {
        Self {
            gradient,
            center: UnitPoint::CENTER,
            start_radius: 0.0,
            end_radius,
        }
    }

    pub fn center(mut self, center: UnitPoint) -> Self {
        self.center = center;
        self
    }

    pub fn start_radius(mut self, r: f32) -> Self {
        self.start_radius = r;
        self
    }
}

/// Dégradé en ellipses ajustées à la forme ; rayons en fraction de la demi-taille (1 = le bord).
#[derive(Clone, Debug, PartialEq)]
pub struct EllipticalGradient {
    pub gradient: Gradient,
    pub center: UnitPoint,
    pub start_fraction: f32,
    pub end_fraction: f32,
}

impl EllipticalGradient {
    pub fn new<I: Into<Ink>>(inks: impl IntoIterator<Item = I>) -> Self {
        Self::with(Gradient::new(inks))
    }

    pub fn with(gradient: Gradient) -> Self {
        Self {
            gradient,
            center: UnitPoint::CENTER,
            start_fraction: 0.0,
            end_fraction: 1.0,
        }
    }

    pub fn center(mut self, center: UnitPoint) -> Self {
        self.center = center;
        self
    }

    pub fn fractions(mut self, start: f32, end: f32) -> Self {
        self.start_fraction = start;
        self.end_fraction = end;
        self
    }
}

/// Balayage autour de `center`, de `start_angle` à `end_angle` (degrés, sens horaire depuis +X).
#[derive(Clone, Debug, PartialEq)]
pub struct AngularGradient {
    pub gradient: Gradient,
    pub center: UnitPoint,
    pub start_angle: f32,
    pub end_angle: f32,
}

impl AngularGradient {
    /// Un tour complet.
    pub fn new<I: Into<Ink>>(inks: impl IntoIterator<Item = I>) -> Self {
        Self::with(Gradient::new(inks))
    }

    pub fn with(gradient: Gradient) -> Self {
        Self {
            gradient,
            center: UnitPoint::CENTER,
            start_angle: 0.0,
            end_angle: 360.0,
        }
    }

    pub fn center(mut self, center: UnitPoint) -> Self {
        self.center = center;
        self
    }

    pub fn angles(mut self, start: f32, end: f32) -> Self {
        self.start_angle = start;
        self.end_angle = end;
        self
    }
}

gradient_builders!(LinearGradient);
gradient_builders!(RadialGradient);
gradient_builders!(EllipticalGradient);
gradient_builders!(AngularGradient);

/// Un point d'un dégradé maillé.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshPoint {
    pub position: UnitPoint,
    pub ink: Ink,
}

/// Dégradé maillé : `width × height` points, ligne par ligne. Couleurs interpolées dans `space`,
/// lissage bicubique (Catmull-Rom) des positions et des couleurs si `smooth`, sinon bilinéaire.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshGradient {
    pub width: usize,
    pub height: usize,
    pub points: Vec<MeshPoint>,
    pub smooth: bool,
    pub space: ColorSpace,
}

impl MeshGradient {
    /// Grille régulière, couleurs ligne par ligne (les manquantes sont blanches).
    pub fn grid<I: Into<Ink>>(width: usize, height: usize, inks: impl IntoIterator<Item = I>) -> Self {
        let (width, height) = (width.max(2), height.max(2));
        let mut inks = inks.into_iter().map(Into::into);
        let mut points = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                points.push(MeshPoint {
                    position: UnitPoint::new(x as f32 / (width - 1) as f32, y as f32 / (height - 1) as f32),
                    ink: inks.next().unwrap_or(Ink::Color(Color::White)),
                });
            }
        }
        Self {
            width,
            height,
            points,
            smooth: true,
            space: ColorSpace::default(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.width >= 2 && self.height >= 2 && self.points.len() == self.width * self.height
    }

    pub fn at(&self, x: usize, y: usize) -> &MeshPoint {
        &self.points[y * self.width + x]
    }

    pub fn smooth(mut self, smooth: bool) -> Self {
        self.smooth = smooth;
        self
    }

    pub fn space(mut self, space: ColorSpace) -> Self {
        self.space = space;
        self
    }
}

/// Ce qui remplit une forme, un trait ou du texte.
#[derive(Clone, Debug, PartialEq)]
pub enum Paint {
    Solid(Color),
    /// Couleur sémantique : remplacée par sa valeur (thème, mode clair / sombre) avant le rendu.
    Token(Token, Hierarchy),
    Linear(LinearGradient),
    Radial(RadialGradient),
    Elliptical(EllipticalGradient),
    Angular(AngularGradient),
    Mesh(MeshGradient),
}

impl Default for Paint {
    fn default() -> Self {
        Paint::Solid(Color::Black)
    }
}

impl From<Color> for Paint {
    fn from(c: Color) -> Self {
        Paint::Solid(c)
    }
}

impl From<Token> for Paint {
    fn from(t: Token) -> Self {
        Paint::Token(t, Hierarchy::Primary)
    }
}

impl From<Ink> for Paint {
    fn from(i: Ink) -> Self {
        match i {
            Ink::Color(c) => Paint::Solid(c),
            Ink::Token(t, h) => Paint::Token(t, h),
        }
    }
}

impl From<LinearGradient> for Paint {
    fn from(g: LinearGradient) -> Self {
        Paint::Linear(g)
    }
}
impl From<RadialGradient> for Paint {
    fn from(g: RadialGradient) -> Self {
        Paint::Radial(g)
    }
}
impl From<EllipticalGradient> for Paint {
    fn from(g: EllipticalGradient) -> Self {
        Paint::Elliptical(g)
    }
}
impl From<AngularGradient> for Paint {
    fn from(g: AngularGradient) -> Self {
        Paint::Angular(g)
    }
}
impl From<MeshGradient> for Paint {
    fn from(g: MeshGradient) -> Self {
        Paint::Mesh(g)
    }
}

impl Paint {
    /// La couleur unie, si c'en est une.
    pub fn solid(&self) -> Option<Color> {
        match self {
            Paint::Solid(c) => Some(*c),
            _ => None,
        }
    }

    /// Plus aucun jeton.
    pub fn is_resolved(&self) -> bool {
        let mut ok = true;
        self.map_inks(&mut |i| {
            ok &= matches!(i, Ink::Color(_));
            i.clone()
        });
        ok && !matches!(self, Paint::Token(..))
    }

    /// Couleur moyenne (texte en dégradé, mélanges). Une peinture non résolue donne du transparent.
    pub fn average(&self) -> Color {
        match self {
            Paint::Solid(c) => *c,
            Paint::Token(..) => Color::Clear,
            Paint::Linear(g) => g.gradient.average(),
            Paint::Radial(g) => g.gradient.average(),
            Paint::Elliptical(g) => g.gradient.average(),
            Paint::Angular(g) => g.gradient.average(),
            Paint::Mesh(m) => average(
                m.points.iter().map(|p| p.ink.color().unwrap_or(Color::Clear)),
                m.space,
            ),
        }
    }

    /// La même peinture, chaque couleur ou jeton remplacé par `f` (pour un `Paint::Token` : appelé avec
    /// le jeton).
    pub fn map_inks(&self, f: &mut impl FnMut(&Ink) -> Ink) -> Paint {
        match self {
            Paint::Solid(c) => f(&Ink::Color(*c)).into(),
            Paint::Token(t, h) => f(&Ink::Token(t.clone(), *h)).into(),
            Paint::Linear(g) => Paint::Linear(LinearGradient {
                gradient: g.gradient.map(f),
                ..g.clone()
            }),
            Paint::Radial(g) => Paint::Radial(RadialGradient {
                gradient: g.gradient.map(f),
                ..g.clone()
            }),
            Paint::Elliptical(g) => Paint::Elliptical(EllipticalGradient {
                gradient: g.gradient.map(f),
                ..g.clone()
            }),
            Paint::Angular(g) => Paint::Angular(AngularGradient {
                gradient: g.gradient.map(f),
                ..g.clone()
            }),
            Paint::Mesh(m) => Paint::Mesh(MeshGradient {
                points: m
                    .points
                    .iter()
                    .map(|p| MeshPoint {
                        position: p.position,
                        ink: f(&p.ink),
                    })
                    .collect(),
                ..m.clone()
            }),
        }
    }

    /// Chaque couleur transformée par `f` (les jetons sont laissés tels quels).
    pub fn map_colors(&self, f: impl Fn(Color) -> Color) -> Paint {
        self.map_inks(&mut |i| match i {
            Ink::Color(c) => Ink::Color(f(*c)),
            other => other.clone(),
        })
    }

    /// Multiplie l'opacité.
    pub fn opacity(&self, factor: f32) -> Paint {
        self.map_colors(|c| c.opacity(factor))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stops_reguliers() {
        let g = Gradient::new([Color::Red, Color::Green, Color::Blue]);
        let l: Vec<f32> = g.stops.iter().map(|s| s.location).collect();
        assert_eq!(l, [0.0, 0.5, 1.0]);
    }

    #[test]
    fn echantillon_aux_stops() {
        let g = Gradient::new([Color::Red, Color::Blue]);
        assert_eq!(g.sample(-1.0), Color::Red);
        assert_eq!(g.sample(2.0), Color::Blue);
        let m = g.sample(0.5);
        assert!(m.r > 0.2 && m.b > 0.2);
    }

    #[test]
    fn jetons_et_resolution() {
        let p: Paint = LinearGradient::new([Ink::from(Token::Accent), Color::White.into()]).into();
        assert!(!p.is_resolved());
        let r = p.map_inks(&mut |i| match i {
            Ink::Token(..) => Ink::Color(Color::Blue),
            o => o.clone(),
        });
        assert!(r.is_resolved());
        assert_eq!(Token::named("Accent"), Token::Accent);
    }

    #[test]
    fn maillage() {
        let m = MeshGradient::grid(3, 2, [Color::Red]);
        assert!(m.is_valid());
        assert_eq!(m.at(2, 1).position, UnitPoint::new(1.0, 1.0));
        assert_eq!(m.at(1, 0).ink, Ink::Color(Color::White));
    }
}
