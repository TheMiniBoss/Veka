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

fn backdrop_params(b: &VekaBackdrop) -> Params {
    let shape = match b.shape {
        VekaBackdropShape::Rectangle => "rectangle",
        VekaBackdropShape::RoundedRectangle => "roundedRectangle",
        VekaBackdropShape::Capsule => "capsule",
        VekaBackdropShape::Circle => "circle",
        VekaBackdropShape::Ellipse => "ellipse",
    };
    let num = |v: f32| Value::Float(v as f64);
    Params::new()
        .with("radius", num(b.radius))
        .with("shape", Value::Enum(shape.into()))
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
