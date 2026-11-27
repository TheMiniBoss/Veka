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
//! Une fenêtre peut aussi venir d'un fichier `.vk` : [`App::window_vk`]. En debug, le fichier est
//! relu dès qu'il change (voir [`App::hot_reload`]) ; une erreur s'affiche en bandeau, l'ancienne
//! version reste à l'écran.
//!
//! Avec `VEKA_EXIT_AFTER_FIRST_FRAME=1`, l'application se ferme après sa première image (CI).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

use veka_lang::{Diagnostic, Program};

use veka_platform::{
    MouseButton, Platform, PlatformApp, PlatformContext, PlatformError, PlatformEvent, WindowId,
};
use veka_render::{FrameInfo, RenderError, Renderer};
use veka_render_wgpu::{SurfaceOptions, WindowRenderer};
use veka_ui::{
    Axis, Color, Element, Font, InputEvent, PointerButton, State, Surface, View, component, views,
};

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
pub struct App {
    windows: Vec<(WindowDesc, Element)>,
    files: Vec<VkFile>,
    hot_reload: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            windows: Vec::new(),
            files: Vec::new(),
            hot_reload: cfg!(debug_assertions),
        }
    }
}

/// Un fichier `.vk` affiché : son programme, et de quoi redessiner son bandeau d'erreurs.
struct VkFile {
    path: PathBuf,
    program: Program,
    banner: Banner,
}

/// Les erreurs à afficher par-dessus une vue `.vk`.
#[derive(Clone, Default)]
struct Banner {
    errors: Rc<RefCell<Vec<String>>>,
    /// L'état du bandeau, connu après sa première évaluation.
    state: Rc<Cell<Option<State<u32>>>>,
}

impl Banner {
    fn set(&self, errors: Vec<String>) {
        if *self.errors.borrow() == errors {
            return;
        }
        for e in &errors {
            eprintln!("Veka .vk : {e}");
        }
        *self.errors.borrow_mut() = errors;
        if let Some(s) = self.state.get()
            && s.is_alive()
        {
            s.update(|g| *g += 1);
        }
    }

    /// La vue, avec le bandeau en haut quand il y a des erreurs. L'arbre garde la même forme
    /// avec ou sans bandeau : la vue (et ses états) n'est pas recréée.
    fn wrap(&self, content: impl Fn() -> Element + 'static) -> Element {
        let banner = self.clone();
        component(move || {
            let generation = State::new(0u32);
            banner.state.set(Some(generation));
            let _ = generation.get();
            let errors = banner.errors.borrow();
            let overlay = if errors.is_empty() {
                Vec::new()
            } else {
                let lines: Vec<Element> = errors
                    .iter()
                    .take(5)
                    .map(|e| {
                        views::Text(e.clone())
                            .font(Font::Footnote)
                            .foreground(Color::White)
                    })
                    .collect();
                // Une bande sur toute la largeur : le texte à gauche, un `Spacer` à droite.
                let texte = views::stack(Axis::Vertical, Some(4.0), lines);
                let bandeau = views::stack(Axis::Horizontal, Some(0.0), vec![texte, views::Spacer()])
                    .padding(12)
                    .background(Color::Red);
                vec![bandeau, views::Spacer()]
            };
            views::stack(
                Axis::Depth,
                None,
                vec![content(), views::stack(Axis::Vertical, Some(0.0), overlay)],
            )
        })
    }
}

fn messages(errors: &[Diagnostic]) -> Vec<String> {
    errors
        .iter()
        .filter(|d| d.is_error())
        .map(ToString::to_string)
        .collect()
}

fn modified(path: &PathBuf) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
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

    /// Ajoute une fenêtre qui affiche la vue `view` du fichier `.vk` `path` (le titre est le nom
    /// de la vue). Le fichier est lu tout de suite ; un fichier illisible ou faux s'affiche en erreur.
    pub fn window_vk(self, path: impl Into<PathBuf>, view: &str) -> Self {
        self.window_vk_with(WindowDesc::new(view), path, view)
    }

    /// Comme [`window_vk`](Self::window_vk), avec tous les réglages de la fenêtre.
    pub fn window_vk_with(mut self, desc: WindowDesc, path: impl Into<PathBuf>, view: &str) -> Self {
        let path = path.into();
        let (program, errors) = match std::fs::read_to_string(&path) {
            Ok(source) => {
                let p = Program::new(&source);
                let e = messages(&p.errors());
                (p, e)
            }
            Err(e) => (Program::new(""), vec![format!("{} : {e}", path.display())]),
        };
        let banner = Banner::default();
        banner.set(errors);
        let root = {
            let (program, view) = (program.clone(), view.to_string());
            banner.wrap(move || program.view(&view))
        };
        self.windows.push((desc, root));
        self.files.push(VkFile {
            path,
            program,
            banner,
        });
        self
    }

    /// Relit les fichiers `.vk` quand ils changent. Par défaut : oui en debug, non en release.
    pub fn hot_reload(mut self, on: bool) -> Self {
        self.hot_reload = on;
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
            watch: Watch {
                files: self.files,
                changed: Vec::new(),
                on: self.hot_reload,
            },
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
    watch: Watch,
}

/// Le rechargement à chaud : un fil surveille la date des fichiers et réveille la boucle.
struct Watch {
    files: Vec<VkFile>,
    /// Un drapeau par fichier, levé par le fil de surveillance.
    changed: Vec<Arc<AtomicBool>>,
    on: bool,
}

impl Watch {
    fn start(&mut self, ctx: &dyn PlatformContext) {
        if !self.on || self.files.is_empty() {
            return;
        }
        let Some(waker) = ctx.waker() else { return };
        self.changed = self
            .files
            .iter()
            .map(|_| Arc::new(AtomicBool::new(false)))
            .collect();
        let watched: Vec<(PathBuf, Arc<AtomicBool>)> = self
            .files
            .iter()
            .zip(&self.changed)
            .map(|(f, c)| (f.path.clone(), c.clone()))
            .collect();
        let spawned = std::thread::Builder::new()
            .name("veka-vk-watch".into())
            .spawn(move || {
                let mut dates: Vec<Option<SystemTime>> = watched.iter().map(|(p, _)| modified(p)).collect();
                loop {
                    std::thread::sleep(Duration::from_millis(250));
                    let mut wake = false;
                    for ((path, flag), date) in watched.iter().zip(dates.iter_mut()) {
                        let now = modified(path);
                        if now != *date {
                            *date = now;
                            flag.store(true, Ordering::Release);
                        }
                        // Toujours en attente (réveil perdu) : on redemande.
                        wake |= flag.load(Ordering::Acquire);
                    }
                    if wake {
                        waker();
                    }
                }
            });
        if spawned.is_err() {
            eprintln!("Veka .vk : rechargement à chaud indisponible (fil non créé)");
        }
    }

    /// Relit les fichiers signalés.
    fn reload(&self) {
        for (file, flag) in self.files.iter().zip(&self.changed) {
            if !flag.swap(false, Ordering::AcqRel) {
                continue;
            }
            match std::fs::read_to_string(&file.path) {
                // Un éditeur peut vider le fichier avant de l'écrire : on attend la suite.
                Ok(source) if source.trim().is_empty() => {}
                Ok(source) => match file.program.reload(&source) {
                    Ok(()) => file.banner.set(Vec::new()),
                    Err(errors) => file.banner.set(messages(&errors)),
                },
                Err(e) => file.banner.set(vec![format!("{} : {e}", file.path.display())]),
            }
        }
    }

    /// Une erreur dans une action `.vk` (après un clic…).
    fn runtime_errors(&self) {
        if let Some(d) = Program::take_runtime_error() {
            for f in &self.files {
                f.banner.set(vec![d.to_string()]);
            }
        }
    }
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
        surface.set_reduce_motion(appearance.reduce_motion);
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
        self.watch.runtime_errors();
        let w = self.windows.get_mut(&id).expect("fenêtre ouverte");
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
                self.watch.start(ctx);
            }
            PlatformEvent::Wake => {
                self.watch.reload();
                for (&id, w) in self.windows.iter() {
                    if w.surface.needs_frame() {
                        ctx.window(id).request_redraw();
                    }
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
                    w.surface.set_reduce_motion(appearance.reduce_motion);
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
