//! Couleurs, valeurs et paramètres : le chemin « données » vers le schéma.

use std::ffi::{c_char, c_void};
use std::rc::Rc;

use veka_foundation::Color;
use veka_ui::Action;
use veka_ui::schema::{Params, Value};

use crate::anima::{VekaAnimation, VekaTransition};
use crate::colora::{VekaPaint, VekaTheme};
use crate::{VekaContext, context, string};

/// Une couleur sRGB, composantes de 0 à 1.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VekaColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl From<VekaColor> for Color {
    fn from(c: VekaColor) -> Self {
        Color::rgba(c.r, c.g, c.b, c.a)
    }
}

/// Couleur opaque.
#[unsafe(no_mangle)]
pub extern "C" fn veka_rgb(r: f32, g: f32, b: f32) -> VekaColor {
    VekaColor { r, g, b, a: 1.0 }
}

/// Couleur avec opacité.
#[unsafe(no_mangle)]
pub extern "C" fn veka_rgba(r: f32, g: f32, b: f32, a: f32) -> VekaColor {
    VekaColor { r, g, b, a }
}

/// Couleur depuis un entier `0xRRGGBB`.
#[unsafe(no_mangle)]
pub extern "C" fn veka_hex(rgb: u32) -> VekaColor {
    let c = Color::hex(rgb);
    VekaColor {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

/// Fonction appelée par une action (clic sur un bouton, toucher…).
pub type VekaActionFn = Option<unsafe extern "C" fn(cx: *mut VekaContext, user: *mut c_void)>;

/// Une action : la fonction et son pointeur `user`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VekaAction {
    pub callback: VekaActionFn,
    pub user: *mut c_void,
}

pub(crate) fn action(callback: VekaActionFn, user: *mut c_void) -> Result<Action, String> {
    let callback = callback.ok_or("action : fonction NULL")?;
    // SAFETY : la fonction et `user` viennent du C, qui garantit leur validité tant que l'interface
    // existe (contrat de l'API).
    Ok(Action::new(move || unsafe { callback(context(), user) }))
}

/// Type d'une [`VekaValue`].
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VekaValueKind {
    Bool,
    Int,
    Number,
    String,
    Color,
    /// Une valeur d'énumération par son nom (`"title"`, `"leading"`…), dans `string`.
    Enum,
    Action,
    /// Une peinture (`VekaPaint*`, copiée : le C la garde).
    Paint,
    /// Un thème (`VekaTheme*`, partagé : le C le garde).
    Theme,
    /// Une spec d'animation, par valeur.
    Animation,
    /// Une transition (`VekaTransition*`, copiée : le C la garde).
    Transition,
}

/// Contenu d'une [`VekaValue`], selon son type.
#[repr(C)]
#[derive(Clone, Copy)]
pub union VekaValueData {
    pub boolean: bool,
    pub integer: i64,
    pub number: f64,
    pub string: *const c_char,
    pub color: VekaColor,
    pub action: VekaAction,
    pub paint: *const VekaPaint,
    pub theme: *const VekaTheme,
    pub animation: VekaAnimation,
    pub transition: *const VekaTransition,
}

/// Une valeur de paramètre. Se construit avec `veka_number`, `veka_string`…
#[repr(C)]
#[derive(Clone, Copy)]
pub struct VekaValue {
    pub kind: VekaValueKind,
    pub data: VekaValueData,
}

/// Un paramètre nommé, par exemple `{ "spacing", veka_number(16) }`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct VekaParam {
    pub name: *const c_char,
    pub value: VekaValue,
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_bool(value: bool) -> VekaValue {
    VekaValue {
        kind: VekaValueKind::Bool,
        data: VekaValueData { boolean: value },
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_int(value: i64) -> VekaValue {
    VekaValue {
        kind: VekaValueKind::Int,
        data: VekaValueData { integer: value },
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_number(value: f64) -> VekaValue {
    VekaValue {
        kind: VekaValueKind::Number,
        data: VekaValueData { number: value },
    }
}

/// Chaîne UTF-8, copiée au moment de la construction de l'élément.
#[unsafe(no_mangle)]
pub extern "C" fn veka_string(value: *const c_char) -> VekaValue {
    VekaValue {
        kind: VekaValueKind::String,
        data: VekaValueData { string: value },
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_color(value: VekaColor) -> VekaValue {
    VekaValue {
        kind: VekaValueKind::Color,
        data: VekaValueData { color: value },
    }
}

/// Valeur d'énumération par son nom : `veka_enum("title")`.
#[unsafe(no_mangle)]
pub extern "C" fn veka_enum(name: *const c_char) -> VekaValue {
    VekaValue {
        kind: VekaValueKind::Enum,
        data: VekaValueData { string: name },
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_action(callback: VekaActionFn, user: *mut c_void) -> VekaValue {
    VekaValue {
        kind: VekaValueKind::Action,
        data: VekaValueData {
            action: VekaAction { callback, user },
        },
    }
}

/// Une peinture comme valeur de paramètre (`fill`, `foregroundColor`…). Elle est copiée au moment
/// de la construction de l'élément : le C la garde et la libère.
#[unsafe(no_mangle)]
pub extern "C" fn veka_paint_value(paint: *const VekaPaint) -> VekaValue {
    VekaValue {
        kind: VekaValueKind::Paint,
        data: VekaValueData { paint },
    }
}

/// Un thème comme valeur de paramètre (modifier `theme`). Le C le garde.
#[unsafe(no_mangle)]
pub extern "C" fn veka_theme_value(theme: *const VekaTheme) -> VekaValue {
    VekaValue {
        kind: VekaValueKind::Theme,
        data: VekaValueData { theme },
    }
}

/// Une spec d'animation comme valeur de paramètre.
#[unsafe(no_mangle)]
pub extern "C" fn veka_animation_value(animation: VekaAnimation) -> VekaValue {
    VekaValue {
        kind: VekaValueKind::Animation,
        data: VekaValueData { animation },
    }
}

/// Une transition comme valeur de paramètre (copiée à l'usage : le C la garde).
#[unsafe(no_mangle)]
pub extern "C" fn veka_transition_value(transition: *const VekaTransition) -> VekaValue {
    VekaValue {
        kind: VekaValueKind::Transition,
        data: VekaValueData { transition },
    }
}

/// Convertit les paramètres C.
pub(crate) unsafe fn params(params: *const VekaParam, count: usize) -> Result<Params, String> {
    let mut out = Params::new();
    if count == 0 {
        return Ok(out);
    }
    if params.is_null() {
        return Err("tableau de paramètres NULL".into());
    }
    // SAFETY : `count` paramètres lisibles (contrat de l'API).
    for p in unsafe { std::slice::from_raw_parts(params, count) } {
        // SAFETY : chaîne C valide (contrat de l'API).
        let name = unsafe { string(p.name, "nom de paramètre") }?;
        // SAFETY : on ne lit que le champ indiqué par `kind`.
        let value = unsafe {
            match p.value.kind {
                VekaValueKind::Bool => Value::Bool(p.value.data.boolean),
                VekaValueKind::Int => Value::Int(p.value.data.integer),
                VekaValueKind::Number => Value::Float(p.value.data.number),
                VekaValueKind::String => Value::String(Rc::from(string(p.value.data.string, name)?)),
                VekaValueKind::Color => Value::Color(p.value.data.color.into()),
                VekaValueKind::Enum => Value::Enum(Rc::from(string(p.value.data.string, name)?)),
                VekaValueKind::Action => {
                    let a = p.value.data.action;
                    Value::Action(action(a.callback, a.user)?)
                }
                VekaValueKind::Paint => {
                    let p = p.value.data.paint;
                    if p.is_null() {
                        return Err(format!("{name} : peinture NULL"));
                    }
                    Value::Paint((*p).paint.clone())
                }
                VekaValueKind::Theme => {
                    let t = p.value.data.theme;
                    if t.is_null() {
                        return Err(format!("{name} : thème NULL"));
                    }
                    Value::Theme((*t).theme.clone())
                }
                VekaValueKind::Animation => Value::Animation(p.value.data.animation.into()),
                VekaValueKind::Transition => {
                    let t = p.value.data.transition;
                    if t.is_null() {
                        return Err(format!("{name} : transition NULL"));
                    }
                    Value::Transition((*t).transition.clone())
                }
            }
        };
        out = out.with(name, value);
    }
    Ok(out)
}
