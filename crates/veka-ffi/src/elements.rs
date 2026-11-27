//! Les éléments : primitives, piles et modifiers. Tous passent par le schéma.

use std::ffi::{c_char, c_void};
use std::rc::Rc;

use veka_ui::schema::{Params, Value};

use crate::value::{VekaActionFn, VekaColor, VekaParam, action, params};
use crate::{VekaElement, build_boxed, guard, string, take, take_all};

fn num(v: f32) -> Value {
    Value::Float(v as f64)
}

fn color(c: VekaColor) -> Value {
    Value::Color(c.into())
}

/// Un modifier sur `element` (consommé).
unsafe fn modify(element: *mut VekaElement, name: &str, params: Params) -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API (élément donné par Veka, non consommé).
        let child = unsafe { take(element) }?;
        build_boxed(name, params, vec![child])
    })
}

// --- Accès direct au schéma ------------------------------------------------------------------

/// Construit n'importe quel composant du schéma par son nom (`"VStack"`, `"Text"`…).
/// Les enfants sont consommés.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_element(
    name: *const c_char,
    params_ptr: *const VekaParam,
    params_count: usize,
    children: *const *mut VekaElement,
    children_count: usize,
) -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let children = unsafe { take_all(children, children_count) }?;
        let name = unsafe { string(name, "nom") }?;
        let params = unsafe { params(params_ptr, params_count) }?;
        build_boxed(name, params, children)
    })
}

/// Applique n'importe quel modifier du schéma par son nom (`"padding"`, `"fill"`…).
/// `element` est consommé.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_modifier(
    element: *mut VekaElement,
    name: *const c_char,
    params_ptr: *const VekaParam,
    params_count: usize,
) -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let child = unsafe { take(element) }?;
        let name = unsafe { string(name, "nom") }?;
        let params = unsafe { params(params_ptr, params_count) }?;
        build_boxed(name, params, vec![child])
    })
}

/// Libère un élément qui n'a été donné à personne.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_element_free(element: *mut VekaElement) {
    if !element.is_null() {
        // SAFETY : contrat de l'API.
        drop(unsafe { take(element) });
    }
}

// --- Primitives ------------------------------------------------------------------------------

/// Du texte (UTF-8, copié).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_text(content: *const c_char) -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let content = unsafe { string(content, "veka_text") }?;
        build_boxed(
            "Text",
            Params::new().with("content", Value::String(Rc::from(content))),
            vec![],
        )
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_rectangle() -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        build_boxed("Rectangle", Params::new(), vec![])
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_rounded_rect(corner_radius: f32) -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        build_boxed(
            "RoundedRectangle",
            Params::new().with("cornerRadius", num(corner_radius)),
            vec![],
        )
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_capsule() -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        build_boxed("Capsule", Params::new(), vec![])
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_circle() -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        build_boxed("Circle", Params::new(), vec![])
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_ellipse() -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        build_boxed("Ellipse", Params::new(), vec![])
    })
}

/// Espace flexible dans une pile.
#[unsafe(no_mangle)]
pub extern "C" fn veka_spacer() -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        build_boxed("Spacer", Params::new(), vec![])
    })
}

/// Bouton : `action` est appelée à chaque clic avec `user`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_button(
    title: *const c_char,
    callback: VekaActionFn,
    user: *mut c_void,
) -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let title = unsafe { string(title, "veka_button") }?;
        let params = Params::new()
            .with("title", Value::String(Rc::from(title)))
            .with("action", Value::Action(action(callback, user)?));
        build_boxed("Button", params, vec![])
    })
}

// --- Piles -----------------------------------------------------------------------------------

unsafe fn stack(
    name: &str,
    spacing: Option<f32>,
    items: *const *mut VekaElement,
    count: usize,
) -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let children = unsafe { take_all(items, count) }?;
        let mut params = Params::new();
        if let Some(s) = spacing {
            params = params.with("spacing", num(s));
        }
        build_boxed(name, params, children)
    })
}

/// Pile verticale. Les `count` éléments sont consommés.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_vstack(
    spacing: f32,
    items: *const *mut VekaElement,
    count: usize,
) -> *mut VekaElement {
    // SAFETY : contrat de l'API.
    unsafe { stack("VStack", Some(spacing), items, count) }
}

/// Pile horizontale. Les `count` éléments sont consommés.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_hstack(
    spacing: f32,
    items: *const *mut VekaElement,
    count: usize,
) -> *mut VekaElement {
    // SAFETY : contrat de l'API.
    unsafe { stack("HStack", Some(spacing), items, count) }
}

/// Pile en profondeur (le premier élément est au fond). Les `count` éléments sont consommés.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_zstack(items: *const *mut VekaElement, count: usize) -> *mut VekaElement {
    // SAFETY : contrat de l'API.
    unsafe { stack("ZStack", None, items, count) }
}

// --- Modifiers -------------------------------------------------------------------------------

/// Marge identique sur les quatre côtés.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_padding(element: *mut VekaElement, all: f32) -> *mut VekaElement {
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "padding", Params::new().with("all", num(all))) }
}

/// Taille imposée. Une valeur négative laisse cette dimension libre.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_frame(element: *mut VekaElement, width: f32, height: f32) -> *mut VekaElement {
    let mut params = Params::new();
    if width >= 0.0 {
        params = params.with("width", num(width));
    }
    if height >= 0.0 {
        params = params.with("height", num(height));
    }
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "frame", params) }
}

/// Remplit une forme.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_fill(element: *mut VekaElement, c: VekaColor) -> *mut VekaElement {
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "fill", Params::new().with("color", color(c))) }
}

/// Trace le contour d'une forme.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_stroke(
    element: *mut VekaElement,
    c: VekaColor,
    line_width: f32,
) -> *mut VekaElement {
    let params = Params::new()
        .with("color", color(c))
        .with("lineWidth", num(line_width));
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "stroke", params) }
}

/// Couleur du texte et des formes non remplies du contenu.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_foreground(element: *mut VekaElement, c: VekaColor) -> *mut VekaElement {
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "foregroundColor", Params::new().with("color", color(c))) }
}

/// Couleur d'accent (boutons…).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_tint(element: *mut VekaElement, c: VekaColor) -> *mut VekaElement {
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "tint", Params::new().with("color", color(c))) }
}

/// Fond coloré derrière le contenu.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_background(element: *mut VekaElement, c: VekaColor) -> *mut VekaElement {
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "background", Params::new().with("color", color(c))) }
}

/// Style de texte.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VekaFont {
    LargeTitle,
    Title,
    Title2,
    Title3,
    Headline,
    Body,
    Callout,
    Subheadline,
    Footnote,
    Caption,
    Caption2,
}

impl VekaFont {
    fn name(self) -> &'static str {
        match self {
            VekaFont::LargeTitle => "largeTitle",
            VekaFont::Title => "title",
            VekaFont::Title2 => "title2",
            VekaFont::Title3 => "title3",
            VekaFont::Headline => "headline",
            VekaFont::Body => "body",
            VekaFont::Callout => "callout",
            VekaFont::Subheadline => "subheadline",
            VekaFont::Footnote => "footnote",
            VekaFont::Caption => "caption",
            VekaFont::Caption2 => "caption2",
        }
    }
}

/// Graisse du texte.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VekaWeight {
    Thin,
    UltraLight,
    Light,
    Regular,
    Medium,
    Semibold,
    Bold,
    Heavy,
    Black,
}

impl VekaWeight {
    fn name(self) -> &'static str {
        match self {
            VekaWeight::Thin => "thin",
            VekaWeight::UltraLight => "ultraLight",
            VekaWeight::Light => "light",
            VekaWeight::Regular => "regular",
            VekaWeight::Medium => "medium",
            VekaWeight::Semibold => "semibold",
            VekaWeight::Bold => "bold",
            VekaWeight::Heavy => "heavy",
            VekaWeight::Black => "black",
        }
    }
}

/// Style du texte du contenu.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_font(element: *mut VekaElement, font: VekaFont) -> *mut VekaElement {
    let params = Params::new().with("style", Value::Enum(Rc::from(font.name())));
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "font", params) }
}

/// Graisse du texte du contenu.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_font_weight(element: *mut VekaElement, weight: VekaWeight) -> *mut VekaElement {
    let params = Params::new().with("weight", Value::Enum(Rc::from(weight.name())));
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "fontWeight", params) }
}

/// Appelle `callback` quand on touche (clique) le contenu.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_on_tap(
    element: *mut VekaElement,
    callback: VekaActionFn,
    user: *mut c_void,
) -> *mut VekaElement {
    let params = match action(callback, user) {
        Ok(a) => Params::new().with("action", Value::Action(a)),
        Err(e) => {
            // SAFETY : l'élément est consommé même en cas d'erreur.
            unsafe { crate::veka_element_free(element) };
            crate::set_error(e);
            return std::ptr::null_mut();
        }
    };
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "onTapGesture", params) }
}

// --- Fenêtres dessinées par Veka -------------------------------------------------------------

/// Un bouton de fenêtre.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VekaWindowControl {
    Close,
    Minimize,
    Maximize,
    Fullscreen,
}

impl VekaWindowControl {
    fn name(self) -> &'static str {
        match self {
            VekaWindowControl::Close => "close",
            VekaWindowControl::Minimize => "minimize",
            VekaWindowControl::Maximize => "maximize",
            VekaWindowControl::Fullscreen => "fullscreen",
        }
    }
}

/// La vue déplace la fenêtre quand on la tire (double-clic : agrandir).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_window_drag_area(element: *mut VekaElement) -> *mut VekaElement {
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "windowDragArea", Params::new()) }
}

/// La vue devient le bouton `control` de la fenêtre.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_window_control(
    element: *mut VekaElement,
    control: VekaWindowControl,
) -> *mut VekaElement {
    let params = Params::new().with("control", Value::Enum(Rc::from(control.name())));
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "windowControl", params) }
}

/// Un bouton de fenêtre dessiné par Veka, au style du système.
#[unsafe(no_mangle)]
pub extern "C" fn veka_window_button(control: VekaWindowControl) -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        build_boxed(
            "WindowButton",
            Params::new().with("control", Value::Enum(Rc::from(control.name()))),
            vec![],
        )
    })
}

/// Une barre de titre prête à l'emploi (titre et boutons placés comme sur le système).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_title_bar(title: *const c_char) -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let title = unsafe { string(title, "veka_title_bar") }?;
        build_boxed(
            "TitleBar",
            Params::new().with("title", Value::String(Rc::from(title))),
            vec![],
        )
    })
}
