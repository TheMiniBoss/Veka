//! Le défilement (étape 10) : `ScrollView`, ses réglages et `scroll_to`.

use std::ffi::c_char;
use std::rc::Rc;

use veka_ui::schema::{Params, Value};

use crate::colora::VekaUnitPoint;
use crate::{VekaElement, guard, string};

/// Défile verticalement.
pub const VEKA_SCROLL_VERTICAL: u32 = 1;
/// Défile horizontalement.
pub const VEKA_SCROLL_HORIZONTAL: u32 = 2;

/// Les barres de défilement.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VekaScrollIndicators {
    /// Pendant le défilement et au survol.
    Automatic,
    /// Toujours, quand le contenu dépasse.
    Visible,
    Hidden,
}

/// Les bords élastiques.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VekaScrollBounce {
    /// Toujours (par défaut).
    Always,
    /// Seulement quand le contenu dépasse.
    BasedOnSize,
    /// Jamais : le défilement s'arrête net aux bords.
    Never,
}

/// Une vue qui défile : `axes` est `VEKA_SCROLL_VERTICAL`, `VEKA_SCROLL_HORIZONTAL` ou les deux.
/// `content` est consommé.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_scroll_view(axes: u32, content: *mut VekaElement) -> *mut VekaElement {
    let mut names = Vec::new();
    if axes & VEKA_SCROLL_HORIZONTAL != 0 {
        names.push("horizontal");
    }
    if axes & VEKA_SCROLL_VERTICAL != 0 || names.is_empty() {
        names.push("vertical");
    }
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let child = unsafe { crate::take(content) }?;
        let params = Params::new().with("axes", Value::String(Rc::from(names.join(" "))));
        crate::build_boxed("ScrollView", params, vec![child])
    })
}

/// Les barres des vues qui défilent dans le sous-arbre.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_scroll_indicators(
    element: *mut VekaElement,
    visibility: VekaScrollIndicators,
) -> *mut VekaElement {
    let v = match visibility {
        VekaScrollIndicators::Automatic => "automatic",
        VekaScrollIndicators::Visible => "visible",
        VekaScrollIndicators::Hidden => "hidden",
    };
    let params = Params::new().with("visibility", Value::Enum(Rc::from(v)));
    // SAFETY : contrat de l'API.
    unsafe { crate::elements::modify(element, "scrollIndicators", params) }
}

/// Les vues qui défilent dans le sous-arbre ne bougent plus (`true`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_scroll_disabled(element: *mut VekaElement, disabled: bool) -> *mut VekaElement {
    let params = Params::new().with("disabled", Value::Bool(disabled));
    // SAFETY : contrat de l'API.
    unsafe { crate::elements::modify(element, "scrollDisabled", params) }
}

/// Les bords élastiques des vues qui défilent dans le sous-arbre.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_scroll_bounce(
    element: *mut VekaElement,
    bounce: VekaScrollBounce,
) -> *mut VekaElement {
    let v = match bounce {
        VekaScrollBounce::Always => "always",
        VekaScrollBounce::BasedOnSize => "basedOnSize",
        VekaScrollBounce::Never => "never",
    };
    let params = Params::new().with("behavior", Value::Enum(Rc::from(v)));
    // SAFETY : contrat de l'API.
    unsafe { crate::elements::modify(element, "scrollBounceBehavior", params) }
}

/// Fait défiler jusqu'à la vue qui porte `veka_id(…, id)`, son point `anchor` aligné sur le même point
/// de la vue qui défile. Animé dans `veka_with_animation`. À appeler dans une action.
#[unsafe(no_mangle)]
pub extern "C" fn veka_scroll_to(id: i64, anchor: VekaUnitPoint) {
    veka_ui::views::ScrollViewProxy.scroll_to(id, Some(anchor.into()));
}

/// Comme `veka_scroll_to`, avec un identifiant texte (`.id("haut")` en `.vk`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_scroll_to_named(id: *const c_char, anchor: VekaUnitPoint) {
    guard((), || {
        // SAFETY : contrat de l'API.
        let id = unsafe { string(id, "veka_scroll_to_named") }?;
        veka_ui::views::ScrollViewProxy.scroll_to(id, Some(anchor.into()));
        Ok(())
    })
}
