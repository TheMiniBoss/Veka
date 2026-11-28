//! Effets qui lisent l'arrière-plan : flou et verre dépoli.

use veka_ui::schema::{Params, Value};

use crate::app::VekaApp;
use crate::colora::{VekaPaint, take_paint};
use crate::{VekaElement, build_boxed, guard, take};

/// Forme découpée par un effet.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VekaBackdropShape {
    Rectangle,
    /// Coins arrondis de `corner_radius`.
    RoundedRectangle,
    Capsule,
    Circle,
    Ellipse,
}

/// Réglages d'un flou d'arrière-plan. Partir de `veka_backdrop_default` ou de
/// `veka_frosted_glass_default`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VekaBackdrop {
    /// Rayon du flou, en points.
    pub radius: f32,
    pub shape: VekaBackdropShape,
    pub corner_radius: f32,
    /// Part de la teinte, de 0 à 1.
    pub tint_amount: f32,
    /// 1 = inchangée.
    pub saturation: f32,
    /// Grain, de 0 à 1.
    pub noise: f32,
}

/// Flou d'arrière-plan : rayon 20, rectangle, teinte du fond de la fenêtre à 15 %, saturation 1,4.
#[unsafe(no_mangle)]
pub extern "C" fn veka_backdrop_default() -> VekaBackdrop {
    VekaBackdrop {
        radius: 20.0,
        shape: VekaBackdropShape::Rectangle,
        corner_radius: 0.0,
        tint_amount: 0.15,
        saturation: 1.4,
        noise: 0.0,
    }
}

/// Verre dépoli : rayon 30, coins de 16, teinte à 35 %, saturation 1,6, un peu de grain.
#[unsafe(no_mangle)]
pub extern "C" fn veka_frosted_glass_default() -> VekaBackdrop {
    VekaBackdrop {
        radius: 30.0,
        shape: VekaBackdropShape::RoundedRectangle,
        corner_radius: 16.0,
        tint_amount: 0.35,
        saturation: 1.6,
        noise: 0.035,
    }
}

fn shape_name(shape: VekaBackdropShape) -> &'static str {
    match shape {
        VekaBackdropShape::Rectangle => "rectangle",
        VekaBackdropShape::RoundedRectangle => "roundedRectangle",
        VekaBackdropShape::Capsule => "capsule",
        VekaBackdropShape::Circle => "circle",
        VekaBackdropShape::Ellipse => "ellipse",
    }
}

fn num(v: f32) -> Value {
    Value::Float(v as f64)
}

fn backdrop_params(b: &VekaBackdrop) -> Params {
    Params::new()
        .with("radius", num(b.radius))
        .with("shape", Value::Enum(shape_name(b.shape).into()))
        .with("cornerRadius", num(b.corner_radius))
        .with("tintAmount", num(b.tint_amount))
        .with("saturation", num(b.saturation))
        .with("noise", num(b.noise))
}

/// Floute ce qui est dessiné derrière `element` (consommé). `tint` (consommée) peut être NULL : teinte
/// du fond de la fenêtre.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_backdrop(
    element: *mut VekaElement,
    blur: *const VekaBackdrop,
    tint: *mut VekaPaint,
) -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API. Élément et peinture repris avant toute erreur.
        let child = unsafe { take(element) };
        let tint = (!tint.is_null()).then(|| unsafe { take_paint(tint) });
        let child = child?;
        if blur.is_null() {
            return Err("veka_backdrop : réglages NULL".into());
        }
        // SAFETY : contrat de l'API.
        let mut params = backdrop_params(unsafe { &*blur });
        if let Some(t) = tint {
            params = params.with("tint", Value::Paint(t?));
        }
        build_boxed("backdropBlur", params, vec![child])
    })
}

/// Flou d'arrière-plan de rayon `radius` (rectangle, réglages par défaut).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_backdrop_blur(element: *mut VekaElement, radius: f32) -> *mut VekaElement {
    let b = VekaBackdrop {
        radius,
        ..veka_backdrop_default()
    };
    // SAFETY : contrat de l'API.
    unsafe { veka_backdrop(element, &b, std::ptr::null_mut()) }
}

/// Verre dépoli avec ses réglages par défaut.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_frosted_glass(element: *mut VekaElement) -> *mut VekaElement {
    let b = veka_frosted_glass_default();
    // SAFETY : contrat de l'API.
    unsafe { veka_backdrop(element, &b, std::ptr::null_mut()) }
}

/// Réglages d'un verre liquide (façon Liquid Glass 27). Partir de `veka_glass_regular` ou de
/// `veka_glass_clear`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VekaGlass {
    /// De 0 (teinté) à 1 (clair).
    pub clarity: f32,
    /// Non nul : le reflet suit le pointeur.
    pub interactive: i32,
    pub shape: VekaBackdropShape,
    pub corner_radius: f32,
    /// Déplacement du fond au bord, en points.
    pub refraction: f32,
    /// Largeur du bord courbe, en points.
    pub bezel: f32,
    /// Séparation des couleurs au bord, de 0 à 1.
    pub aberration: f32,
    /// Reflet du bord, de 0 à 1.
    pub highlight: f32,
    /// Ombre portée, de 0 à 1.
    pub shadow: f32,
}

/// Le réglage « Par défaut » : clarté 0,4, capsule, réfraction 22, bord 18.
#[unsafe(no_mangle)]
pub extern "C" fn veka_glass_regular() -> VekaGlass {
    let g = veka_ui::views::Glass::regular();
    VekaGlass {
        clarity: g.clarity,
        interactive: 0,
        shape: VekaBackdropShape::Capsule,
        corner_radius: 0.0,
        refraction: g.refraction,
        bezel: g.bezel,
        aberration: g.aberration,
        highlight: g.highlight,
        shadow: g.shadow,
    }
}

/// Le réglage « Clair » : comme `veka_glass_regular`, clarté 0,9.
#[unsafe(no_mangle)]
pub extern "C" fn veka_glass_clear() -> VekaGlass {
    VekaGlass {
        clarity: veka_ui::views::Glass::clear().clarity,
        ..veka_glass_regular()
    }
}

fn glass_params(g: &VekaGlass) -> Params {
    Params::new()
        .with("clarity", num(g.clarity))
        .with("interactive", Value::Bool(g.interactive != 0))
        .with("shape", Value::Enum(shape_name(g.shape).into()))
        .with("cornerRadius", num(g.corner_radius))
        .with("refraction", num(g.refraction))
        .with("bezel", num(g.bezel))
        .with("aberration", num(g.aberration))
        .with("highlight", num(g.highlight))
        .with("shadow", num(g.shadow))
}

/// Construit `name` (verre ou groupe) autour de `element` (consommé), avec `tint` (consommée, NULL :
/// teinte du fond de la fenêtre).
fn glass_like(
    name: &str,
    element: *mut VekaElement,
    glass: *const VekaGlass,
    tint: *mut VekaPaint,
    extra: Option<(&str, f32)>,
) -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API. Élément et peinture repris avant toute erreur.
        let child = unsafe { take(element) };
        let tint = (!tint.is_null()).then(|| unsafe { take_paint(tint) });
        let child = child?;
        let mut params = if glass.is_null() {
            Params::new()
        } else {
            // SAFETY : contrat de l'API.
            glass_params(unsafe { &*glass })
        };
        if let Some(t) = tint {
            params = params.with("tint", Value::Paint(t?));
        }
        if let Some((k, v)) = extra {
            params = params.with(k, num(v));
        }
        build_boxed(name, params, vec![child])
    })
}

/// Pose un verre liquide sous `element` (consommé). `glass` NULL : `veka_glass_regular`. `tint`
/// (consommée) peut être NULL. Dans un `veka_glass_group`, l'élément donne seulement sa forme au groupe.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_glass(
    element: *mut VekaElement,
    glass: *const VekaGlass,
    tint: *mut VekaPaint,
) -> *mut VekaElement {
    glass_like("glass", element, glass, tint, None)
}

/// Un seul verre pour tous les `veka_glass` de `element` (consommé) : leurs formes fusionnent comme des
/// gouttes à moins de `smoothness` points. `glass` (NULL : par défaut) et `tint` règlent ce verre.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_glass_group(
    element: *mut VekaElement,
    smoothness: f32,
    glass: *const VekaGlass,
    tint: *mut VekaPaint,
) -> *mut VekaElement {
    glass_like(
        "glassGroup",
        element,
        glass,
        tint,
        Some(("smoothness", smoothness)),
    )
}

/// Le fond ondule dans la forme d'`element` (consommé) : `strength` points de déplacement, vagues de
/// `scale` points, `speed` 1 = normal, 0 = figé.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_liquid_distortion(
    element: *mut VekaElement,
    strength: f32,
    scale: f32,
    speed: f32,
    shape: VekaBackdropShape,
    corner_radius: f32,
) -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let child = unsafe { take(element) }?;
        let params = Params::new()
            .with("strength", num(strength))
            .with("scale", num(scale))
            .with("speed", num(speed))
            .with("shape", Value::Enum(shape_name(shape).into()))
            .with("cornerRadius", num(corner_radius));
        build_boxed("liquidDistortion", params, vec![child])
    })
}

/// Qualité des effets d'arrière-plan.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VekaBackdropQuality {
    /// Aucun effet : une teinte unie à la place.
    Off,
    /// Un seul niveau de flou, sans grain.
    Low,
    High,
}

/// Règle la qualité des effets d'arrière-plan de toutes les fenêtres (par défaut : `HIGH`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_app_set_backdrop_quality(
    app: *mut VekaApp,
    quality: VekaBackdropQuality,
) -> i32 {
    guard(-1, || {
        if app.is_null() {
            return Err("application NULL".into());
        }
        // SAFETY : contrat de l'API.
        let app = unsafe { &mut *app };
        let q = match quality {
            VekaBackdropQuality::Off => veka_ui::BackdropQuality::Off,
            VekaBackdropQuality::Low => veka_ui::BackdropQuality::Low,
            VekaBackdropQuality::High => veka_ui::BackdropQuality::High,
        };
        app.app = std::mem::take(&mut app.app).backdrop_quality(q);
        Ok(0)
    })
}

/// Comment une vue se mélange avec ce qui est dessiné derrière elle (mêmes noms qu'en SwiftUI).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VekaBlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    SoftLight,
    HardLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
    /// Le groupe ne se dessine que là où le fond est déjà dessiné.
    SourceAtop,
    /// Le groupe passe derrière ce qui est déjà dessiné.
    DestinationOver,
    /// Le groupe efface le fond (perce un trou à sa forme).
    DestinationOut,
    PlusDarker,
    PlusLighter,
}

/// Mélange `element` avec ce qui est dessiné derrière lui, d'un bloc (le sous-arbre est d'abord
/// dessiné à part). Reprend `element`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_blend_mode(element: *mut VekaElement, mode: VekaBlendMode) -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let child = unsafe { take(element) }?;
        let name = veka_ui::BlendMode::NAMES[mode as usize];
        build_boxed(
            "blendMode",
            Params::new().with_static("mode", Value::Enum(name.into())),
            vec![child],
        )
    })
}

/// Dessine `element` à part avant de le composer : une opacité appliquée ensuite le rend transparent
/// d'un bloc. Reprend `element`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_compositing_group(element: *mut VekaElement) -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let child = unsafe { take(element) }?;
        build_boxed("compositingGroup", Params::new(), vec![child])
    })
}
