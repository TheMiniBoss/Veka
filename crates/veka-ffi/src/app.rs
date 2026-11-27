//! L'application et ses fenêtres.

use std::ffi::{c_char, c_void};

use veka_app::App;
use veka_platform::{Chrome, Size, WindowDesc};
use veka_ui::Element;
use veka_ui::schema::Params;

use crate::{VekaContext, VekaElement, build, context, guard, set_error, string, take};

/// Une application. Opaque.
pub struct VekaApp {
    pub(crate) app: App,
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

/// Décoration d'une fenêtre.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VekaChrome {
    /// Barre et boutons du système.
    System,
    /// Barre du système masquée (macOS garde ses feux).
    Hidden,
    /// Aucune décoration du système : Veka dessine tout.
    Custom,
}

/// Tous les réglages d'une fenêtre.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VekaWindowDesc {
    pub title: *const c_char,
    /// Taille du contenu, en points.
    pub width: f32,
    pub height: f32,
    /// Taille minimale (0 : aucune).
    pub min_width: f32,
    pub min_height: f32,
    pub resizable: bool,
    pub chrome: VekaChrome,
    /// `VEKA_CHROME_CUSTOM` : fond transparent.
    pub transparent: bool,
    /// `VEKA_CHROME_CUSTOM` : ombre du système.
    pub shadow: bool,
    /// `VEKA_CHROME_CUSTOM` : rayon des coins, en points.
    pub corner_radius: f32,
    /// Largeur des bords de redimensionnement, en points (0 : 6 points).
    pub resize_margin: f32,
}

/// Réglages par défaut : 480 × 320, redimensionnable, décoration du système.
#[unsafe(no_mangle)]
pub extern "C" fn veka_window_desc_default(title: *const c_char) -> VekaWindowDesc {
    VekaWindowDesc {
        title,
        width: 480.0,
        height: 320.0,
        min_width: 0.0,
        min_height: 0.0,
        resizable: true,
        chrome: VekaChrome::System,
        transparent: false,
        shadow: true,
        corner_radius: 8.0,
        resize_margin: 6.0,
    }
}

/// Ajoute une fenêtre avec tous ses réglages. Renvoie 0, ou -1 en cas d'erreur.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_app_add_window_ex(
    app: *mut VekaApp,
    desc: *const VekaWindowDesc,
    setup: VekaSetupFn,
    body: VekaBodyFn,
    user: *mut c_void,
) -> i32 {
    guard(-1, || {
        if app.is_null() || desc.is_null() {
            return Err("application ou réglages NULL".into());
        }
        if body.is_none() {
            return Err("veka_app_add_window_ex : body NULL".into());
        }
        // SAFETY : contrat de l'API.
        let (app, d) = unsafe { (&mut *app, *desc) };
        let desc = unsafe { window_desc(&d) }?;
        app.app = std::mem::take(&mut app.app).window_with(desc, window_view(setup, body, user));
        Ok(0)
    })
}

/// Les réglages Rust d'une fenêtre C.
pub(crate) unsafe fn window_desc(d: &VekaWindowDesc) -> Result<WindowDesc, String> {
    {
        // SAFETY : contrat de l'API.
        let title = unsafe { string(d.title, "titre") }?;
        let mut desc = WindowDesc::new(title)
            .size(Size::new(d.width, d.height))
            .resizable(d.resizable)
            .chrome(match d.chrome {
                VekaChrome::System => Chrome::System,
                VekaChrome::Hidden => Chrome::Hidden,
                VekaChrome::Custom => Chrome::Custom {
                    transparent: d.transparent,
                    shadow: d.shadow,
                    corner_radius: d.corner_radius,
                },
            });
        if d.min_width > 0.0 || d.min_height > 0.0 {
            desc = desc.min_size(Size::new(d.min_width, d.min_height));
        }
        if d.resize_margin > 0.0 {
            desc = desc.resize_margin(d.resize_margin);
        }
        Ok(desc)
    }
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
