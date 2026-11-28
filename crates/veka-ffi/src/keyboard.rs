//! Le clavier, le focus et les champs de texte (étape 9).
//!
//! Les touches se nomment comme en `.vk` : `"return"`, `"escape"`, `"tab"`, `"upArrow"`, `"f1"`… ou un
//! caractère (`"r"`). Les modificateurs sont des bits `VEKA_MODIFIER_*` ; `VEKA_MODIFIER_COMMAND` vaut ⌘ sur
//! le Mac et Ctrl ailleurs.

use std::cell::RefCell;
use std::ffi::{CString, c_char, c_void};
use std::rc::Rc;

use veka_ui::State;
use veka_ui::schema::{Binding, Params, Value};

use crate::value::{VekaActionFn, action};
use crate::{VekaContext, VekaElement, VekaState, build_boxed, guard, string};

/// Maj.
pub const VEKA_MODIFIER_SHIFT: u32 = 1;
/// Ctrl, sur toutes les plateformes.
pub const VEKA_MODIFIER_CONTROL: u32 = 2;
/// Alt (⌥ sur le Mac).
pub const VEKA_MODIFIER_OPTION: u32 = 4;
/// ⌘ sur le Mac, Ctrl ailleurs : un seul raccourci pour les deux.
pub const VEKA_MODIFIER_COMMAND: u32 = 16;

fn handle(s: *mut VekaState) -> Result<u64, String> {
    if s.is_null() {
        return Err("état NULL".into());
    }
    Ok(s as usize as u64)
}

thread_local! {
    /// Le texte rendu par `veka_state_get_string`, gardé jusqu'au prochain appel.
    static LAST_STRING: RefCell<CString> = RefCell::new(CString::default());
}

/// Crée un état texte (la première fois) ou retrouve le même (les fois suivantes).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_state_new_string(
    _cx: *mut VekaContext,
    initial: *const c_char,
) -> *mut VekaState {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let initial = unsafe { string(initial, "veka_state_new_string") }?.to_owned();
        Ok(State::new(initial).to_bits() as usize as *mut VekaState)
    })
}

/// Le texte de l'état, en UTF-8. Valable jusqu'au prochain appel de `veka_state_get_string`.
#[unsafe(no_mangle)]
pub extern "C" fn veka_state_get_string(_cx: *mut VekaContext, s: *mut VekaState) -> *const c_char {
    guard(std::ptr::null(), || {
        let text = State::<String>::from_bits(handle(s)?).get();
        let text = CString::new(text.replace('\0', " ")).unwrap_or_default();
        Ok(LAST_STRING.with(|l| {
            *l.borrow_mut() = text;
            l.borrow().as_ptr()
        }))
    })
}

/// Change le texte : la vue sera reconstruite avant la prochaine image.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_state_set_string(
    _cx: *mut VekaContext,
    s: *mut VekaState,
    value: *const c_char,
) {
    guard((), || {
        let state = State::<String>::from_bits(handle(s)?);
        // SAFETY : contrat de l'API.
        let value = unsafe { string(value, "veka_state_set_string") }?;
        state.set(value.to_owned());
        Ok(())
    })
}

/// Relie un état texte (`veka_state_new_string`) au schéma.
fn string_binding(s: *mut VekaState) -> Result<Binding, String> {
    let state = State::<String>::from_bits(handle(s)?);
    Ok(Binding {
        get: Rc::new(move || {
            Value::String(Rc::from(if state.is_alive() {
                state.get()
            } else {
                String::new()
            }))
        }),
        set: Rc::new(move |v| {
            if let Value::String(t) = v {
                state.set(t.to_string());
            }
        }),
    })
}

/// Relie un état entier (`veka_state_new_int`) au schéma.
fn int_binding(s: *mut VekaState) -> Result<Binding, String> {
    let state = State::<i64>::from_bits(handle(s)?);
    Ok(Binding {
        get: Rc::new(move || Value::Int(if state.is_alive() { state.get() } else { 0 })),
        set: Rc::new(move |v| {
            if let Value::Int(n) = v {
                state.set(n);
            }
        }),
    })
}

unsafe fn field(name: &str, placeholder: *const c_char, text: *mut VekaState) -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        let mut params = Params::new().with("text", Value::Binding(string_binding(text)?));
        if !placeholder.is_null() {
            // SAFETY : contrat de l'API.
            let p = unsafe { string(placeholder, name) }?;
            params = params.with("placeholder", Value::String(Rc::from(p)));
        }
        build_boxed(name, params, vec![])
    })
}

/// Un champ de texte d'une ligne, relié à un état texte. `placeholder` peut être NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_text_field(
    placeholder: *const c_char,
    text: *mut VekaState,
) -> *mut VekaElement {
    // SAFETY : contrat de l'API.
    unsafe { field("TextField", placeholder, text) }
}

/// Un champ de mot de passe : des points à la place des caractères, sans copier.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_secure_field(
    placeholder: *const c_char,
    text: *mut VekaState,
) -> *mut VekaElement {
    // SAFETY : contrat de l'API.
    unsafe { field("SecureField", placeholder, text) }
}

/// Applique un modifier ; l'élément est consommé même si `params` a échoué.
unsafe fn modify(element: *mut VekaElement, name: &str, params: Result<Params, String>) -> *mut VekaElement {
    match params {
        // SAFETY : contrat de l'API.
        Ok(p) => unsafe { crate::elements::modify(element, name, p) },
        Err(e) => {
            // SAFETY : l'élément est consommé même en cas d'erreur.
            unsafe { crate::veka_element_free(element) };
            crate::set_error(e);
            std::ptr::null_mut()
        }
    }
}

/// Action à l'appui sur Entrée dans un champ du sous-arbre.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_on_submit(
    element: *mut VekaElement,
    callback: VekaActionFn,
    user: *mut c_void,
) -> *mut VekaElement {
    let params = action(callback, user).map(|a| Params::new().with("action", Value::Action(a)));
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "onSubmit", params) }
}

/// Champs du sous-arbre sans cadre (`true`), ou avec le cadre arrondi (`false`, par défaut).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_text_field_plain(element: *mut VekaElement, plain: bool) -> *mut VekaElement {
    let style = if plain { "plain" } else { "roundedBorder" };
    let params = Params::new().with("style", Value::Enum(Rc::from(style)));
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "textFieldStyle", Ok(params)) }
}

/// La vue prend le focus du clavier (clic, Tab).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_focusable(element: *mut VekaElement) -> *mut VekaElement {
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "focusable", Ok(Params::new())) }
}

/// Relie le focus à un état entier : l'état vaut `value` quand ce sous-arbre a le focus, et 0 quand
/// plus rien ne l'a. Changer l'état depuis le code donne le focus (comme `@FocusState`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_focused(
    element: *mut VekaElement,
    focus: *mut VekaState,
    value: i64,
) -> *mut VekaElement {
    let params = int_binding(focus).map(|b| {
        Params::new()
            .with("binding", Value::Binding(b))
            .with("equals", Value::Int(value))
    });
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "focused", params) }
}

fn key_params(key: *const c_char) -> Result<Params, String> {
    // SAFETY : contrat de l'API.
    let key = unsafe { string(key, "touche") }?;
    if veka_ui::prelude::KeyEquivalent::from_name(key.trim_start_matches('.')).is_none() {
        return Err(format!("touche inconnue : « {key} »"));
    }
    Ok(Params::new().with("key", Value::String(Rc::from(key))))
}

/// Action à l'appui d'une touche (`"return"`, `"escape"`, `"r"`…), quand ce sous-arbre a le focus ou
/// que rien ne l'a. La touche est prise : elle ne remonte pas plus haut.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_on_key_press(
    element: *mut VekaElement,
    key: *const c_char,
    callback: VekaActionFn,
    user: *mut c_void,
) -> *mut VekaElement {
    let params = key_params(key).and_then(|p| Ok(p.with("action", Value::Action(action(callback, user)?))));
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "onKeyPress", params) }
}

/// Déclenche le bouton du sous-arbre depuis n'importe où dans la fenêtre : `"escape"` et 0 pour
/// annuler, `"s"` et `VEKA_MODIFIER_COMMAND` pour ⌘S / Ctrl+S…
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_keyboard_shortcut(
    element: *mut VekaElement,
    key: *const c_char,
    modifiers: u32,
) -> *mut VekaElement {
    let mut names = Vec::new();
    for (bit, name) in [
        (VEKA_MODIFIER_COMMAND, "command"),
        (VEKA_MODIFIER_SHIFT, "shift"),
        (VEKA_MODIFIER_OPTION, "option"),
        (VEKA_MODIFIER_CONTROL, "control"),
    ] {
        if modifiers & bit != 0 {
            names.push(name);
        }
    }
    let params = key_params(key).map(|p| p.with("modifiers", Value::String(Rc::from(names.join(" ")))));
    // SAFETY : contrat de l'API.
    unsafe { modify(element, "keyboardShortcut", params) }
}
