//! # Veka App
//!
//! L'hôte « fenêtre native » : il branche la plateforme de l'OS → une [`Surface`] par fenêtre →
//! le renderer wgpu. Il ne dessine que quand la surface le demande : rien au repos.
//!
//! ```no_run
//! use veka_app::App;
//! use veka_ui::prelude::*;
//!
//! fn bonjour() -> impl View {
//!     Text("Bonjour Veka")
//! }
//!
//! App::new().window("Bonjour", bonjour).run().unwrap();
//! ```
//!
//! Avec `VEKA_EXIT_AFTER_FIRST_FRAME=1`, l'application se ferme après sa première image (CI).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use veka_platform::{
    MouseButton, Platform, PlatformApp, PlatformContext, PlatformError, PlatformEvent, WindowId,
};
use veka_render::{FrameInfo, RenderError, Renderer};
use veka_render_wgpu::{SurfaceOptions, WindowRenderer};
use veka_ui::{Color, Element, InputEvent, PointerButton, Surface, View, component};

pub use veka_platform::{Chrome, Size, WindowDesc};

/// Les réglages d'une fenêtre : `Window::new("Titre").size(…).chrome(Chrome::custom())`.
pub type Window = WindowDesc;

/// Erreur au lancement de l'application.
#[derive(Debug)]
pub enum AppError {
    Platform(PlatformError),
    Render(RenderError),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Platform(e) => write!(f, "{e}"),
            AppError::Render(e) => write!(f, "rendu : {e}"),
        }
    }
}

impl std::error::Error for AppError {}

/// Une application Veka : une ou plusieurs fenêtres, chacune avec sa vue racine.
#[derive(Default)]
pub struct App {
    windows: Vec<(WindowDesc, Element)>,
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("windows", &self.windows.len())
            .finish()
    }
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ajoute une fenêtre de titre `title` dont le contenu est `view` (ré-appelée quand son état change).
    pub fn window<F, V>(self, title: impl Into<String>, view: F) -> Self
    where
        F: Fn() -> V + 'static,
        V: View,
    {
        self.window_with(WindowDesc::new(title), view)
    }

    /// Comme [`window`](Self::window), avec tous les réglages de la fenêtre.
    pub fn window_with<F, V>(mut self, desc: WindowDesc, view: F) -> Self
    where
        F: Fn() -> V + 'static,
        V: View,
    {
        self.windows.push((desc, component(view)));
        self
    }

    /// Ouvre les fenêtres avec la plateforme de l'OS et tourne jusqu'à la fermeture de la dernière.
    pub fn run(self) -> Result<(), AppError> {
        #[cfg(windows)]
        return self.run_on(veka_platform_windows::WindowsPlatform::new());
        #[cfg(target_os = "macos")]
        return self.run_on(veka_platform_macos::MacPlatform::new());
        #[cfg(not(any(windows, target_os = "macos")))]
        Err(AppError::Platform(PlatformError::Unsupported(
            "pas encore de plateforme Veka pour ce système".into(),
        )))
    }

    /// Comme [`run`](Self::run), avec une plateforme donnée.
    pub fn run_on(self, platform: impl Platform) -> Result<(), AppError> {
        let exit_after_first_frame =
            std::env::var_os("VEKA_EXIT_AFTER_FIRST_FRAME").is_some_and(|v| v != "0");
        let error = Rc::new(RefCell::new(None));
        let host = Host {
            pending: self.windows,
            windows: HashMap::new(),
            error: None,
            exit_after_first_frame,
        };
        platform
            .run(Box::new(Reporter {
                host,
                error: error.clone(),
            }))
            .map_err(AppError::Platform)?;
        error.borrow_mut().take().map_or(Ok(()), Err)
    }
}

/// Garde la première erreur de l'hôte pour la rendre à la fin de `run`.
struct Reporter {
    host: Host,
    error: Rc<RefCell<Option<AppError>>>,
}

impl PlatformApp for Reporter {
    fn event(&mut self, ctx: &mut dyn PlatformContext, event: PlatformEvent) {
        self.host.event(ctx, event);
        if let Some(e) = self.host.error.take() {
            self.error.borrow_mut().get_or_insert(e);
        }
    }
}

struct HostWindow {
    surface: Surface,
    renderer: WindowRenderer,
}

struct Host {
    pending: Vec<(WindowDesc, Element)>,
    windows: HashMap<WindowId, HostWindow>,
    error: Option<AppError>,
    exit_after_first_frame: bool,
}

impl Host {
    fn open(
        &mut self,
        ctx: &mut dyn PlatformContext,
        desc: WindowDesc,
        root: Element,
    ) -> Result<(), AppError> {
        let transparent = desc.chrome.is_transparent();
        let id = ctx.create_window(desc).map_err(AppError::Platform)?;
        let window = ctx.window(id);
        let mut surface = Surface::new(root);
        if transparent {
            let mut env = surface.environment().clone();
            env.background = Color::Clear.into();
            surface.set_environment(env);
        }
        surface.resize(window.size(), window.scale());
        surface.set_waker(window.redraw_requester());
        surface.set_window_info(window.info());
        let appearance = ctx.appearance();
        surface.set_system_appearance(appearance.scheme, appearance.accent);
        let (width, height) = window.pixel_size();
        let frame = FrameInfo {
            width,
            height,
            scale: window.scale(),
        };
        // SAFETY : le renderer est retiré (et détruit) avant la fermeture de sa fenêtre.
        let renderer = unsafe { WindowRenderer::with_options(window, frame, SurfaceOptions { transparent }) };
        let renderer = match renderer {
            Ok(r) => r,
            Err(e) => {
                ctx.close_window(id);
                return Err(AppError::Render(e));
            }
        };
        window.request_redraw();
        self.windows.insert(id, HostWindow { surface, renderer });
        Ok(())
    }

    fn close(&mut self, ctx: &mut dyn PlatformContext, id: WindowId) {
        // Le renderer d'abord, la fenêtre ensuite.
        self.windows.remove(&id);
        ctx.close_window(id);
        if self.windows.is_empty() {
            ctx.exit();
        }
    }

    fn input(&mut self, ctx: &mut dyn PlatformContext, id: WindowId, event: InputEvent) {
        let Some(w) = self.windows.get_mut(&id) else {
            return;
        };
        w.surface.handle(event);
        let actions = w.surface.take_window_actions();
        if w.surface.needs_frame() {
            ctx.window(id).request_redraw();
        }
        // Après le traitement : une action peut fermer la fenêtre.
        for action in actions {
            ctx.window(id).perform(action);
        }
    }

    fn event(&mut self, ctx: &mut dyn PlatformContext, event: PlatformEvent) {
        match event {
            PlatformEvent::Resumed => {
                for (desc, root) in std::mem::take(&mut self.pending) {
                    if let Err(e) = self.open(ctx, desc, root) {
                        self.error = Some(e);
                        ctx.exit();
                        return;
                    }
                }
                if self.windows.is_empty() {
                    ctx.exit();
                }
            }
            PlatformEvent::WindowResized {
                window,
                size,
                width,
                height,
            } => {
                if let Some(w) = self.windows.get_mut(&window) {
                    let scale = ctx.window(window).scale();
                    w.surface.resize(size, scale);
                    w.renderer.resize(FrameInfo { width, height, scale });
                    ctx.window(window).request_redraw();
                }
            }
            PlatformEvent::ScaleChanged { window, scale } => {
                if let Some(w) = self.windows.get_mut(&window) {
                    let size = w.surface.size();
                    w.surface.resize(size, scale);
                    ctx.window(window).request_redraw();
                }
            }
            PlatformEvent::RedrawRequested { window } => {
                let Some(w) = self.windows.get_mut(&window) else {
                    return;
                };
                // L'heure de la plateforme : les animations avancent jusque-là. Tant qu'une animation
                // tourne, `needs_frame` reste vrai et on redessine à chaque synchronisation verticale.
                let list = w.surface.frame_at(ctx.now());
                let result = w.renderer.render(list);
                if let Some(regions) = w.surface.take_regions_if_changed() {
                    ctx.window(window).set_regions(&regions);
                }
                match result {
                    Ok(()) => {
                        if self.exit_after_first_frame {
                            ctx.exit();
                        }
                    }
                    // Surface perdue (changement d'écran, veille…) : elle est reconfigurée, on réessaie.
                    Err(RenderError::Lost) => ctx.window(window).request_redraw(),
                    Err(e) => {
                        self.error = Some(AppError::Render(e));
                        ctx.exit();
                    }
                }
                if w.surface.needs_frame() {
                    ctx.window(window).request_redraw();
                }
            }
            PlatformEvent::WindowStateChanged { window, info } => {
                if let Some(w) = self.windows.get_mut(&window) {
                    w.surface.set_window_info(info);
                    if w.surface.needs_frame() {
                        ctx.window(window).request_redraw();
                    }
                }
            }
            PlatformEvent::AppearanceChanged { appearance } => {
                for (&id, w) in self.windows.iter_mut() {
                    w.surface
                        .set_system_appearance(appearance.scheme, appearance.accent);
                    if w.surface.needs_frame() {
                        ctx.window(id).request_redraw();
                    }
                }
            }
            PlatformEvent::CloseRequested { window } => self.close(ctx, window),
            PlatformEvent::WindowClosed { window } => {
                self.windows.remove(&window);
            }
            PlatformEvent::PointerMoved { window, position } => {
                self.input(ctx, window, InputEvent::PointerMoved { position })
            }
            PlatformEvent::PointerButton {
                window,
                position,
                button,
                pressed,
            } => {
                let button = match button {
                    MouseButton::Primary => PointerButton::Primary,
                    MouseButton::Secondary => PointerButton::Secondary,
                    MouseButton::Middle => PointerButton::Middle,
                    MouseButton::Other(_) => return,
                };
                let event = if pressed {
                    InputEvent::PointerDown { position, button }
                } else {
                    InputEvent::PointerUp { position, button }
                };
                self.input(ctx, window, event);
            }
            PlatformEvent::PointerLeft { window } => self.input(ctx, window, InputEvent::PointerLeft),
        }
    }
}
