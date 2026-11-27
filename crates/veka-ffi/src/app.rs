//! L'application et ses fenêtres.

use std::ffi::{c_char, c_void};

use veka_app::App;
use veka_platform::{Size, WindowDesc};
use veka_ui::Element;
use veka_ui::schema::Params;

use crate::{VekaContext, VekaElement, build, context, guard, set_error, string, take};

/// Une application. Opaque.
pub struct VekaApp {
    app: App,
}

/// Déclare les états de la fenêtre (rappelée avant chaque `body`).
pub type VekaSetupFn = Option<unsafe extern "C" fn(cx: *mut VekaContext, user: *mut c_void)>;

/// Construit le contenu de la fenêtre. L'élément renvoyé est consommé par Veka.
pub type VekaBodyFn =
    Option<unsafe extern "C" fn(cx: *mut VekaContext, user: *mut c_void) -> *mut VekaElement>;

/// La vue d'une fenêtre C : `setup` puis `body`, rappelées quand un état change.
pub(crate) fn window_view(
    setup: VekaSetupFn,
    body: VekaBodyFn,
    user: *mut c_void,
) -> impl Fn() -> Element + 'static {
    move || {
        // SAFETY : fonctions et `user` fournis par le C, valides tant que l'application tourne.
        unsafe {
            if let Some(setup) = setup {
                setup(context(), user);
            }
            let element = match body {
                Some(body) => body(context(), user),
                None => std::ptr::null_mut(),
            };
            take(element).unwrap_or_else(|_| {
                set_error("body a renvoyé NULL (voir l'erreur précédente)");
                build("VStack", Params::new(), vec![]).expect("pile vide")
            })
        }
    }
}

/// Crée une application vide.
#[unsafe(no_mangle)]
pub extern "C" fn veka_app_new() -> *mut VekaApp {
    guard(std::ptr::null_mut(), || {
        Ok(Box::into_raw(Box::new(VekaApp { app: App::new() })))
    })
}

/// Ajoute une fenêtre de `width` × `height` points. Renvoie 0, ou -1 en cas d'erreur.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_app_add_window(
    app: *mut VekaApp,
    title: *const c_char,
    width: f32,
    height: f32,
    setup: VekaSetupFn,
    body: VekaBodyFn,
    user: *mut c_void,
) -> i32 {
    guard(-1, || {
        if app.is_null() {
            return Err("application NULL".into());
        }
        if body.is_none() {
            return Err("veka_app_add_window : body NULL".into());
        }
        // SAFETY : contrat de l'API.
        let title = unsafe { string(title, "titre") }?;
        let app = unsafe { &mut *app };
        let desc = WindowDesc::new(title).size(Size::new(width, height));
        app.app = std::mem::take(&mut app.app).window_with(desc, window_view(setup, body, user));
        Ok(0)
    })
}

/// Ouvre les fenêtres et bloque jusqu'à la fermeture de la dernière. Libère `app`.
/// Renvoie 0, ou 1 en cas d'erreur (voir `veka_last_error`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_app_run(app: *mut VekaApp) -> i32 {
    guard(1, || {
        if app.is_null() {
            return Err("application NULL".into());
        }
        // SAFETY : contrat de l'API ; l'application est consommée.
        let app = unsafe { Box::from_raw(app) };
        app.app.run().map(|()| 0).map_err(|e| e.to_string())
    })
}

/// Libère une application qui n'a pas été lancée.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_app_free(app: *mut VekaApp) {
    if !app.is_null() {
        // SAFETY : contrat de l'API.
        drop(unsafe { Box::from_raw(app) });
    }
}
