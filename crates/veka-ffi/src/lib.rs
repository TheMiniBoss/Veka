//! # Interface C de Veka
//!
//! La bibliothèque `veka.dll` / `libveka.dylib` et son en-tête `include/veka.h` (généré par
//! `cargo xtask header`). Le modèle est le même qu'en Rust : une fenêtre a une fonction `body`
//! qui construit la vue, et Veka la rappelle quand un état change.
//!
//! Règles :
//!
//! - **Propriété** : un `VekaElement*` passé à une pile, à un modifier ou renvoyé par `body` est
//!   *consommé*, même en cas d'erreur. Le C ne libère (`veka_element_free`) que ce qu'il n'a donné
//!   à personne. De même pour un `VekaPaint*` (peinture) passé à une fonction ; un `VekaTheme*`, lui,
//!   reste au C (`veka_theme_free`). Une `VekaTransition*` est consommée comme une peinture ; une
//!   `VekaTimeline*` reste au C (`veka_timeline_free`) ; un `VekaAnimation` se passe par valeur.
//! - **Erreurs** : une fonction qui échoue renvoie `NULL` (ou un code non nul) et
//!   `veka_last_error()` donne le message. Aucune panique ne traverse la frontière.
//! - **Fil** : tout se passe sur le fil qui appelle `veka_app_run` (le fil principal sur macOS).
//! - **Données** : chaque fonction passe par le schéma de Veka (`veka_element` et `veka_modifier`
//!   en sont l'accès direct) : le C n'a rien de plus ni de moins que les autres langages.

#![allow(clippy::missing_safety_doc)]

mod anima;
mod app;
mod colora;
mod effects;
mod elements;
mod keyboard;
mod state;
mod value;
mod vk;

pub use anima::*;
pub use app::*;
pub use colora::*;
pub use effects::*;
pub use elements::*;
pub use keyboard::*;
pub use state::*;
pub use value::*;
pub use vk::*;

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};

use veka_ui::Element;
use veka_ui::schema::{Params, Schema};

/// Un élément d'interface (texte, forme, pile…). Opaque.
pub struct VekaElement {
    element: Element,
}

/// Le contexte passé aux fonctions de rappel. Opaque ; à repasser tel quel à Veka.
pub struct VekaContext {
    _private: u8,
}

static CONTEXT: VekaContext = VekaContext { _private: 0 };

fn context() -> *mut VekaContext {
    &CONTEXT as *const VekaContext as *mut VekaContext
}

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
    static SCHEMA: Schema = Schema::builtin();
}

fn set_error(message: impl Into<String>) {
    let message = message.into().replace('\0', " ");
    let message = CString::new(message).unwrap_or_default();
    LAST_ERROR.with(|e| *e.borrow_mut() = Some(message));
}

/// Exécute `f` sans laisser passer de panique ; en cas d'échec, note l'erreur et renvoie `default`.
fn guard<R>(default: R, f: impl FnOnce() -> Result<R, String>) -> R {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(r)) => r,
        Ok(Err(message)) => {
            set_error(message);
            default
        }
        Err(panic) => {
            let message = panic
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "erreur interne".into());
            set_error(message);
            default
        }
    }
}

/// Lit une chaîne C (UTF-8).
unsafe fn string<'a>(s: *const c_char, what: &str) -> Result<&'a str, String> {
    if s.is_null() {
        return Err(format!("{what} : chaîne NULL"));
    }
    // SAFETY : chaîne C terminée par un zéro, valide pendant l'appel (contrat de l'API).
    unsafe { CStr::from_ptr(s) }
        .to_str()
        .map_err(|_| format!("{what} : la chaîne n'est pas en UTF-8"))
}

fn boxed(element: Element) -> *mut VekaElement {
    Box::into_raw(Box::new(VekaElement { element }))
}

/// Reprend un élément donné par le C.
unsafe fn take(element: *mut VekaElement) -> Result<Element, String> {
    if element.is_null() {
        return Err("élément NULL".into());
    }
    // SAFETY : pointeur obtenu de `boxed` et pas encore consommé (contrat de l'API).
    Ok(unsafe { Box::from_raw(element) }.element)
}

/// Reprend un tableau d'éléments : tous sont consommés, même si l'un d'eux est NULL.
unsafe fn take_all(items: *const *mut VekaElement, count: usize) -> Result<Vec<Element>, String> {
    if count == 0 {
        return Ok(Vec::new());
    }
    if items.is_null() {
        return Err("tableau d'éléments NULL".into());
    }
    // SAFETY : `count` pointeurs lisibles (contrat de l'API).
    let items = unsafe { std::slice::from_raw_parts(items, count) };
    let mut out = Vec::with_capacity(count);
    let mut missing = false;
    for &item in items {
        // SAFETY : voir `take`.
        match unsafe { take(item) } {
            Ok(e) => out.push(e),
            Err(_) => missing = true,
        }
    }
    if missing {
        return Err("un des éléments est NULL".into());
    }
    Ok(out)
}

fn build(name: &str, params: Params, children: Vec<Element>) -> Result<Element, String> {
    SCHEMA
        .with(|s| s.build(name, params, children))
        .map_err(|e| e.to_string())
}

fn build_boxed(name: &str, params: Params, children: Vec<Element>) -> Result<*mut VekaElement, String> {
    build(name, params, children).map(boxed)
}

/// Le message de la dernière erreur sur ce fil, ou NULL. Valable jusqu'à la prochaine erreur.
#[unsafe(no_mangle)]
pub extern "C" fn veka_last_error() -> *const c_char {
    LAST_ERROR.with(|e| e.borrow().as_ref().map_or(std::ptr::null(), |s| s.as_ptr()))
}

/// Efface la dernière erreur.
#[unsafe(no_mangle)]
pub extern "C" fn veka_clear_error() {
    LAST_ERROR.with(|e| *e.borrow_mut() = None);
}

/// La version de Veka (« 0.1.0 »).
#[unsafe(no_mangle)]
pub extern "C" fn veka_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

#[cfg(test)]
mod tests;
