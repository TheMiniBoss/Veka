//! Colora en C : peintures (couleur, jeton, dégradés), thèmes, mode clair / sombre et réglages.
//!
//! Propriété : un `VekaPaint*` passé à une fonction est **consommé**, comme un élément (même en cas
//! d'erreur) ; `veka_paint_copy` pour en garder un. Un `VekaTheme*`, lui, reste au C : il le crée
//! une fois, le prête à `veka_theme` autant de fois qu'il veut et le libère avec `veka_theme_free`.

use std::ffi::c_char;
use std::rc::Rc;
use std::sync::Arc;

use veka_colora::{
    AngularGradient, ColorSpace, EllipticalGradient, Gradient, Hierarchy, Ink, LinearGradient, MeshGradient,
    MeshPoint, Paint, RadialGradient, Stop, Theme, Token, UnitPoint,
};
use veka_ui::schema::{Params, Value};

use crate::value::VekaColor;
use crate::{VekaElement, build_boxed, guard, string, take};

/// Une peinture : couleur, jeton du thème ou dégradé. Opaque.
pub struct VekaPaint {
    pub(crate) paint: Paint,
}

/// Un thème Colora. Opaque.
pub struct VekaTheme {
    pub(crate) theme: Arc<Theme>,
}

/// Un point dans le rectangle d'une forme : (0, 0) en haut à gauche, (1, 1) en bas à droite.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VekaUnitPoint {
    pub x: f32,
    pub y: f32,
}

impl From<VekaUnitPoint> for UnitPoint {
    fn from(p: VekaUnitPoint) -> Self {
        UnitPoint::new(p.x, p.y)
    }
}

pub const VEKA_UNIT_TOP_LEADING: VekaUnitPoint = VekaUnitPoint { x: 0.0, y: 0.0 };
pub const VEKA_UNIT_TOP: VekaUnitPoint = VekaUnitPoint { x: 0.5, y: 0.0 };
pub const VEKA_UNIT_TOP_TRAILING: VekaUnitPoint = VekaUnitPoint { x: 1.0, y: 0.0 };
pub const VEKA_UNIT_LEADING: VekaUnitPoint = VekaUnitPoint { x: 0.0, y: 0.5 };
pub const VEKA_UNIT_CENTER: VekaUnitPoint = VekaUnitPoint { x: 0.5, y: 0.5 };
pub const VEKA_UNIT_TRAILING: VekaUnitPoint = VekaUnitPoint { x: 1.0, y: 0.5 };
pub const VEKA_UNIT_BOTTOM_LEADING: VekaUnitPoint = VekaUnitPoint { x: 0.0, y: 1.0 };
pub const VEKA_UNIT_BOTTOM: VekaUnitPoint = VekaUnitPoint { x: 0.5, y: 1.0 };
pub const VEKA_UNIT_BOTTOM_TRAILING: VekaUnitPoint = VekaUnitPoint { x: 1.0, y: 1.0 };

/// Un point quelconque (`VEKA_UNIT_CENTER`… pour les points usuels).
#[unsafe(no_mangle)]
pub extern "C" fn veka_unit_point(x: f32, y: f32) -> VekaUnitPoint {
    VekaUnitPoint { x, y }
}

/// Niveau d'un jeton : 100 %, 60 %, 30 % ou 18 % d'opacité.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VekaHierarchy {
    Primary,
    Secondary,
    Tertiary,
    Quaternary,
}

impl From<VekaHierarchy> for Hierarchy {
    fn from(h: VekaHierarchy) -> Self {
        match h {
            VekaHierarchy::Primary => Hierarchy::Primary,
            VekaHierarchy::Secondary => Hierarchy::Secondary,
            VekaHierarchy::Tertiary => Hierarchy::Tertiary,
            VekaHierarchy::Quaternary => Hierarchy::Quaternary,
        }
    }
}

/// Espace dans lequel un dégradé interpole (Oklab par défaut).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VekaColorSpace {
    Oklab,
    OkLch,
    Linear,
    Srgb,
}

impl From<VekaColorSpace> for ColorSpace {
    fn from(s: VekaColorSpace) -> Self {
        match s {
            VekaColorSpace::Oklab => ColorSpace::Oklab,
            VekaColorSpace::OkLch => ColorSpace::OkLch,
            VekaColorSpace::Linear => ColorSpace::Linear,
            VekaColorSpace::Srgb => ColorSpace::Srgb,
        }
    }
}

/// Mode clair ou sombre.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VekaColorScheme {
    Light,
    Dark,
}

/// Un arrêt de dégradé : une couleur, ou un jeton si `token` n'est pas NULL, à la position
/// `location` (0 à 1).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VekaStop {
    pub color: VekaColor,
    pub token: *const c_char,
    pub hierarchy: VekaHierarchy,
    pub location: f32,
}

fn boxed(paint: Paint) -> *mut VekaPaint {
    Box::into_raw(Box::new(VekaPaint { paint }))
}

/// Reprend une peinture donnée par le C.
pub(crate) unsafe fn take_paint(paint: *mut VekaPaint) -> Result<Paint, String> {
    if paint.is_null() {
        return Err("peinture NULL".into());
    }
    // SAFETY : pointeur obtenu de `boxed` et pas encore consommé (contrat de l'API).
    Ok(unsafe { Box::from_raw(paint) }.paint)
}

unsafe fn colors<'a>(colors: *const VekaColor, count: usize) -> Result<&'a [VekaColor], String> {
    if count == 0 {
        return Ok(&[]);
    }
    if colors.is_null() {
        return Err("tableau de couleurs NULL".into());
    }
    // SAFETY : `count` couleurs lisibles (contrat de l'API).
    Ok(unsafe { std::slice::from_raw_parts(colors, count) })
}

unsafe fn gradient(c: *const VekaColor, count: usize) -> Result<Gradient, String> {
    // SAFETY : contrat de l'API.
    let c = unsafe { colors(c, count) }?;
    Ok(Gradient::new(c.iter().map(|&c| veka_foundation::Color::from(c))))
}

// --- Peintures -------------------------------------------------------------------------------

/// Une couleur unie.
#[unsafe(no_mangle)]
pub extern "C" fn veka_solid(color: VekaColor) -> *mut VekaPaint {
    boxed(Paint::Solid(color.into()))
}

/// Un jeton du thème par son nom (`"Accent"`, `"Foreground"`, `"Marque.Fond"`…). Un jeton inconnu
/// s'affiche en magenta.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_token(name: *const c_char, hierarchy: VekaHierarchy) -> *mut VekaPaint {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let name = unsafe { string(name, "jeton") }?;
        Ok(boxed(Paint::Token(Token::named(name), hierarchy.into())))
    })
}

/// Dégradé linéaire de `from` à `to`, couleurs réparties régulièrement.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_linear_gradient(
    colors: *const VekaColor,
    count: usize,
    from: VekaUnitPoint,
    to: VekaUnitPoint,
) -> *mut VekaPaint {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let g = unsafe { gradient(colors, count) }?;
        Ok(boxed(
            LinearGradient::with(g).from(from.into()).to(to.into()).into(),
        ))
    })
}

/// Dégradé radial autour de `center`, rayons en points.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_radial_gradient(
    colors: *const VekaColor,
    count: usize,
    center: VekaUnitPoint,
    start_radius: f32,
    end_radius: f32,
) -> *mut VekaPaint {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let g = unsafe { gradient(colors, count) }?;
        Ok(boxed(
            RadialGradient::with(g, end_radius)
                .center(center.into())
                .start_radius(start_radius)
                .into(),
        ))
    })
}

/// Dégradé elliptique : rayons en fraction de la demi-taille de la forme (0 à 1).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_elliptical_gradient(
    colors: *const VekaColor,
    count: usize,
    center: VekaUnitPoint,
    start_fraction: f32,
    end_fraction: f32,
) -> *mut VekaPaint {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let g = unsafe { gradient(colors, count) }?;
        Ok(boxed(
            EllipticalGradient::with(g)
                .center(center.into())
                .fractions(start_fraction, end_fraction)
                .into(),
        ))
    })
}

/// Dégradé angulaire autour de `center` : angles en degrés, sens horaire depuis la droite.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_angular_gradient(
    colors: *const VekaColor,
    count: usize,
    center: VekaUnitPoint,
    start_angle: f32,
    end_angle: f32,
) -> *mut VekaPaint {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let g = unsafe { gradient(colors, count) }?;
        Ok(boxed(
            AngularGradient::with(g)
                .center(center.into())
                .angles(start_angle, end_angle)
                .into(),
        ))
    })
}

/// Dégradé maillé de `width` × `height` points (au moins 2 × 2), couleurs ligne par ligne.
/// `positions` peut être NULL : grille régulière.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_mesh_gradient(
    width: usize,
    height: usize,
    positions: *const VekaUnitPoint,
    colors: *const VekaColor,
) -> *mut VekaPaint {
    guard(std::ptr::null_mut(), || {
        if width < 2 || height < 2 {
            return Err("dégradé maillé : au moins 2 × 2 points".into());
        }
        let n = width * height;
        // SAFETY : contrat de l'API.
        let c = unsafe { self::colors(colors, n) }?;
        let mut mesh = MeshGradient::grid(width, height, c.iter().map(|&c| veka_foundation::Color::from(c)));
        if !positions.is_null() {
            // SAFETY : `width × height` points lisibles (contrat de l'API).
            let pos = unsafe { std::slice::from_raw_parts(positions, n) };
            for (point, &p) in mesh.points.iter_mut().zip(pos) {
                *point = MeshPoint {
                    position: p.into(),
                    ink: point.ink.clone(),
                };
            }
        }
        Ok(boxed(mesh.into()))
    })
}

/// Remplace les arrêts d'un dégradé (pour y mettre des jetons ou des positions libres). Pour un
/// dégradé maillé, les couleurs des points dans l'ordre (les positions des arrêts sont ignorées).
/// `paint` est consommé ; renvoie la peinture modifiée, ou NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_paint_stops(
    paint: *mut VekaPaint,
    stops: *const VekaStop,
    count: usize,
) -> *mut VekaPaint {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let mut paint = unsafe { take_paint(paint) }?;
        if count > 0 && stops.is_null() {
            return Err("tableau d'arrêts NULL".into());
        }
        let raw = if count == 0 {
            &[][..]
        } else {
            // SAFETY : `count` arrêts lisibles (contrat de l'API).
            unsafe { std::slice::from_raw_parts(stops, count) }
        };
        let mut list = Vec::with_capacity(count);
        for s in raw {
            let ink = if s.token.is_null() {
                Ink::Color(s.color.into())
            } else {
                // SAFETY : contrat de l'API.
                let name = unsafe { string(s.token, "jeton") }?;
                Ink::Token(Token::named(name), s.hierarchy.into())
            };
            list.push(Stop::new(ink, s.location));
        }
        let gradient = match &mut paint {
            Paint::Linear(g) => &mut g.gradient,
            Paint::Radial(g) => &mut g.gradient,
            Paint::Elliptical(g) => &mut g.gradient,
            Paint::Angular(g) => &mut g.gradient,
            Paint::Mesh(m) => {
                for (point, stop) in m.points.iter_mut().zip(list) {
                    point.ink = stop.ink;
                }
                return Ok(boxed(paint));
            }
            Paint::Solid(_) | Paint::Token(..) => return Err("veka_paint_stops : pas un dégradé".into()),
        };
        gradient.stops = list;
        Ok(boxed(paint))
    })
}

/// Choisit l'espace d'interpolation d'un dégradé. `paint` est consommé ; renvoie la peinture
/// modifiée (inchangée si ce n'est pas un dégradé).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_paint_space(paint: *mut VekaPaint, space: VekaColorSpace) -> *mut VekaPaint {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let mut paint = unsafe { take_paint(paint) }?;
        let space = space.into();
        match &mut paint {
            Paint::Linear(g) => g.gradient.space = space,
            Paint::Radial(g) => g.gradient.space = space,
            Paint::Elliptical(g) => g.gradient.space = space,
            Paint::Angular(g) => g.gradient.space = space,
            Paint::Mesh(m) => m.space = space,
            Paint::Solid(_) | Paint::Token(..) => {}
        }
        Ok(boxed(paint))
    })
}

/// Une copie de la peinture (qui, elle, n'est pas consommée).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_paint_copy(paint: *const VekaPaint) -> *mut VekaPaint {
    guard(std::ptr::null_mut(), || {
        if paint.is_null() {
            return Err("peinture NULL".into());
        }
        // SAFETY : peinture valide (contrat de l'API).
        Ok(boxed(unsafe { &*paint }.paint.clone()))
    })
}

/// Libère une peinture qui n'a été donnée à personne.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_paint_free(paint: *mut VekaPaint) {
    if !paint.is_null() {
        // SAFETY : contrat de l'API.
        drop(unsafe { take_paint(paint) });
    }
}

// --- Modifiers -------------------------------------------------------------------------------

/// Un modifier à une peinture : l'élément et la peinture sont consommés.
unsafe fn modify_paint(
    element: *mut VekaElement,
    paint: *mut VekaPaint,
    name: &str,
    extra: Params,
) -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API. Les deux sont repris avant toute erreur.
        let child = unsafe { take(element) };
        let paint = unsafe { take_paint(paint) };
        let (child, paint) = (child?, paint?);
        let mut params = Params::new().with("paint", Value::Paint(paint));
        params.0.extend(extra.0);
        build_boxed(name, params, vec![child])
    })
}

unsafe fn modify(element: *mut VekaElement, name: &str, params: Params) -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let child = unsafe { take(element) }?;
        build_boxed(name, params, vec![child])
    })
}

/// Remplit une forme avec une peinture (consommée).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_fill_paint(
    element: *mut VekaElement,
    paint: *mut VekaPaint,
) -> *mut VekaElement {
    // SAFETY : contrat de l'API.
    unsafe { modify_paint(element, paint, "fill", Params::new()) }
}

/// Contour d'une forme avec une peinture (consommée).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_stroke_paint(
    element: *mut VekaElement,
    paint: *mut VekaPaint,
    line_width: f32,
) -> *mut VekaElement {
    let extra = Params::new().with("lineWidth", Value::Float(line_width as f64));
    // SAFETY : contrat de l'API.
    unsafe { modify_paint(element, paint, "stroke", extra) }
}

/// Couleur du texte et des formes non remplies du contenu (peinture consommée).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_foreground_paint(
    element: *mut VekaElement,
    paint: *mut VekaPaint,
) -> *mut VekaElement {
    // SAFETY : contrat de l'API.
    unsafe { modify_paint(element, paint, "foregroundColor", Params::new()) }
}

/// Couleur d'accent du contenu (peinture consommée).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_tint_paint(
    element: *mut VekaElement,
    paint: *mut VekaPaint,
) -> *mut VekaElement {
    // SAFETY : contrat de l'API.
    unsafe { modify_paint(element, paint, "tint", Params::new()) }
}

/// Fond derrière le contenu (peinture consommée).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_background_paint(
    element: *mut VekaElement,
    paint: *mut VekaPaint,
) -> *mut VekaElement {
    // SAFETY : contrat de l'API.
    unsafe { modify_paint(element, paint, "background", Params::new()) }
}

/// Applique un thème au contenu. Le thème n'est pas consommé.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_theme(element: *mut VekaElement, theme: *const VekaTheme) -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let child = unsafe { take(element) }?;
        if theme.is_null() {
            return Err("thème NULL".into());
        }
        // SAFETY : thème valide (contrat de l'API).
        let theme = unsafe { &*theme }.theme.clone();
        build_boxed(
            "theme",
            Params::new().with("theme", Value::Theme(theme)),
            vec![child],
        )
    })
}

/// Force le mode clair ou sombre du contenu (sinon : celui du système).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_color_scheme(
    element: *mut VekaElement,
    scheme: VekaColorScheme,
) -> *mut VekaElement {
    let name = match scheme {
        VekaColorScheme::Light => "light",
        VekaColorScheme::Dark => "dark",
    };
    let params = Params::new().with("scheme", Value::Enum(Rc::from(name)));
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "colorScheme", params) }
}

/// Un réglage de couleur : un seul paramètre `amount`.
unsafe fn adjust(element: *mut VekaElement, name: &str, amount: f32) -> *mut VekaElement {
    let params = Params::new().with("amount", Value::Float(amount as f64));
    // SAFETY : contrat de l'API.
    unsafe { modify(element, name, params) }
}

/// Ajoute `amount` (-1 à 1) à chaque composante des couleurs du contenu.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_brightness(element: *mut VekaElement, amount: f32) -> *mut VekaElement {
    // SAFETY : contrat de l'API.
    unsafe { adjust(element, "brightness", amount) }
}

/// Contraste du contenu (1 = inchangé).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_contrast(element: *mut VekaElement, amount: f32) -> *mut VekaElement {
    // SAFETY : contrat de l'API.
    unsafe { adjust(element, "contrast", amount) }
}

/// Saturation du contenu (0 = gris, 1 = inchangé).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_saturation(element: *mut VekaElement, amount: f32) -> *mut VekaElement {
    // SAFETY : contrat de l'API.
    unsafe { adjust(element, "saturation", amount) }
}

/// Rotation de teinte du contenu, en degrés.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_hue_rotation(element: *mut VekaElement, amount: f32) -> *mut VekaElement {
    // SAFETY : contrat de l'API.
    unsafe { adjust(element, "hueRotation", amount) }
}

/// Niveaux de gris (0 = inchangé, 1 = entièrement gris).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_grayscale(element: *mut VekaElement, amount: f32) -> *mut VekaElement {
    // SAFETY : contrat de l'API.
    unsafe { adjust(element, "grayscale", amount) }
}

// --- Thèmes ----------------------------------------------------------------------------------

/// Un thème vide : tout jeton absent vient de `parent` (qui peut être NULL), puis de Veka.
/// Le parent est copié : le modifier ensuite ne change pas ce thème.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_theme_new(name: *const c_char, parent: *const VekaTheme) -> *mut VekaTheme {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let name = unsafe { string(name, "nom du thème") }?;
        let mut theme = Theme::new(name);
        if !parent.is_null() {
            // SAFETY : thème valide (contrat de l'API).
            theme = theme.parent(unsafe { &*parent }.theme.clone());
        }
        Ok(Box::into_raw(Box::new(VekaTheme {
            theme: Arc::new(theme),
        })))
    })
}

/// Valeur d'un jeton en mode clair et en mode sombre (`dark` NULL : la même). Les peintures sont
/// consommées. Renvoie 0, ou -1 en cas d'erreur.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_theme_set_color(
    theme: *mut VekaTheme,
    token: *const c_char,
    light: *mut VekaPaint,
    dark: *mut VekaPaint,
) -> i32 {
    guard(-1, || {
        // SAFETY : contrat de l'API. Les peintures sont reprises avant toute erreur.
        let light = unsafe { take_paint(light) };
        let dark = if dark.is_null() {
            None
        } else {
            Some(unsafe { take_paint(dark) })
        };
        let light = light?;
        let dark = match dark {
            Some(d) => d?,
            None => light.clone(),
        };
        let token = unsafe { string(token, "jeton") }?;
        if theme.is_null() {
            return Err("thème NULL".into());
        }
        // SAFETY : thème valide et exclusif pendant l'appel (contrat de l'API).
        let t = Arc::make_mut(&mut unsafe { &mut *theme }.theme);
        *t = std::mem::take(t).color(Token::named(token), light, dark);
        Ok(0)
    })
}

/// Une mesure de design du thème (`"CornerRadius.Card"`…). Renvoie 0, ou -1 en cas d'erreur.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_theme_set_metric(
    theme: *mut VekaTheme,
    name: *const c_char,
    value: f32,
) -> i32 {
    guard(-1, || {
        // SAFETY : contrat de l'API.
        let name = unsafe { string(name, "mesure") }?;
        if theme.is_null() {
            return Err("thème NULL".into());
        }
        // SAFETY : thème valide et exclusif pendant l'appel (contrat de l'API).
        let t = Arc::make_mut(&mut unsafe { &mut *theme }.theme);
        *t = std::mem::take(t).metric(name, value);
        Ok(0)
    })
}

/// Libère un thème. Les éléments qui l'utilisent gardent leur copie.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_theme_free(theme: *mut VekaTheme) {
    if !theme.is_null() {
        // SAFETY : pointeur obtenu de `veka_theme_new` (contrat de l'API).
        drop(unsafe { Box::from_raw(theme) });
    }
}
