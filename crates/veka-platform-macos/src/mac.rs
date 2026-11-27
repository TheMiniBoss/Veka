//! Implémentation AppKit.
//!
//! Comme sur Windows, AppKit peut appeler nos méthodes pendant que l'application traite déjà un
//! événement (créer une fenêtre provoque un redimensionnement). Les événements passent donc par une
//! file, vidée seulement quand l'application est libre.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::ffi::c_void;
use std::ptr::NonNull;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSBackingStoreType, NSColor,
    NSEvent, NSEventModifierFlags, NSEventType, NSResponder, NSTrackingArea, NSTrackingAreaOptions, NSView,
    NSWindow, NSWindowButton, NSWindowDelegate, NSWindowStyleMask, NSWindowTitleVisibility,
};
use objc2_foundation::{
    NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, NSUserDefaults,
};
use raw_window_handle::{
    AppKitWindowHandle, DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawWindowHandle,
    WindowHandle,
};
use veka_platform::{
    Chrome, Instant, MouseButton, Platform, PlatformApp, PlatformContext, PlatformError, PlatformEvent,
    PlatformWindow, Point, RegionKind, Result, Size, WindowAction, WindowDesc, WindowId, WindowInfo,
    WindowRegion,
};

/// La plateforme macOS. `MacPlatform::new().run(app)` ouvre la boucle (sur le fil principal).
#[derive(Debug, Default)]
pub struct MacPlatform;

impl MacPlatform {
    pub fn new() -> Self {
        Self
    }
}

/// Ce que la vue et le délégué doivent connaître d'une fenêtre.
#[derive(Debug)]
struct WinData {
    id: WindowId,
    scale: Cell<f32>,
    size: Cell<Size>,
    regions: RefCell<Vec<WindowRegion>>,
    /// La vue dessine sa propre barre (`Chrome::Hidden` ou `Chrome::Custom`).
    custom: bool,
    /// Dernier état envoyé à l'application.
    info: Cell<WindowInfo>,
}

// ---------------------------------------------------------------------------------------------
// La vue

#[derive(Debug)]
struct ViewIvars {
    data: Rc<WinData>,
}

define_class!(
    // SAFETY : NSView n'a pas d'exigence particulière pour être sous-classée ; pas de `Drop`.
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "VekaView"]
    #[ivars = ViewIvars]
    #[derive(Debug)]
    struct VekaView;

    impl VekaView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            push(PlatformEvent::RedrawRequested { window: self.id() });
            pump();
        }

        #[unsafe(method(setFrameSize:))]
        fn set_frame_size(&self, size: NSSize) {
            // SAFETY : on appelle l'implémentation de NSView avec le même argument.
            let _: () = unsafe { msg_send![super(self), setFrameSize: size] };
            self.update_size();
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(viewDidChangeBackingProperties))]
        fn did_change_backing(&self) {
            let scale = self.window().map_or(1.0, |w| w.backingScaleFactor() as f32);
            let data = &self.ivars().data;
            if scale != data.scale.get() {
                data.scale.set(scale);
                push(PlatformEvent::ScaleChanged { window: data.id, scale });
                self.update_size();
                self.setNeedsDisplay(true);
            }
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            self.moved(event);
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            self.moved(event);
        }

        #[unsafe(method(rightMouseDragged:))]
        fn right_mouse_dragged(&self, event: &NSEvent) {
            self.moved(event);
        }

        #[unsafe(method(otherMouseDragged:))]
        fn other_mouse_dragged(&self, event: &NSEvent) {
            self.moved(event);
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            push(PlatformEvent::PointerLeft { window: self.id() });
            pump();
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            if self.drag_window(event) {
                return;
            }
            self.button(event, MouseButton::Primary, true);
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            self.button(event, MouseButton::Primary, false);
        }

        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down(&self, event: &NSEvent) {
            self.button(event, MouseButton::Secondary, true);
        }

        #[unsafe(method(rightMouseUp:))]
        fn right_mouse_up(&self, event: &NSEvent) {
            self.button(event, MouseButton::Secondary, false);
        }

        #[unsafe(method(otherMouseDown:))]
        fn other_mouse_down(&self, event: &NSEvent) {
            self.button(event, other_button(event), true);
        }

        #[unsafe(method(otherMouseUp:))]
        fn other_mouse_up(&self, event: &NSEvent) {
            self.button(event, other_button(event), false);
        }
    }
);

fn other_button(event: &NSEvent) -> MouseButton {
    match event.buttonNumber() {
        2 => MouseButton::Middle,
        n => MouseButton::Other(n as u16),
    }
}

impl VekaView {
    fn new(mtm: MainThreadMarker, data: Rc<WinData>, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ViewIvars { data });
        // SAFETY : `initWithFrame:` est l'initialiseur désigné de NSView.
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        view.setWantsLayer(true);
        // Survol et sortie, sur toute la vue quelle que soit sa taille.
        let options = NSTrackingAreaOptions::MouseEnteredAndExited
            | NSTrackingAreaOptions::MouseMoved
            | NSTrackingAreaOptions::ActiveInKeyWindow
            | NSTrackingAreaOptions::InVisibleRect;
        // SAFETY : la vue possède la zone de suivi et vit plus longtemps qu'elle.
        let area = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                NSRect::ZERO,
                options,
                Some(&view),
                None,
            )
        };
        view.addTrackingArea(&area);
        view
    }

    fn id(&self) -> WindowId {
        self.ivars().data.id
    }

    fn update_size(&self) {
        let data = &self.ivars().data;
        let frame = self.frame();
        let size = Size::new(frame.size.width as f32, frame.size.height as f32);
        data.size.set(size);
        let scale = data.scale.get();
        let (width, height) = pixels(size, scale);
        push(PlatformEvent::WindowResized {
            window: data.id,
            size,
            width,
            height,
        });
        pump();
    }

    fn position(&self, event: &NSEvent) -> Point {
        let p = self.convertPoint_fromView(event.locationInWindow(), None);
        Point::new(p.x as f32, p.y as f32)
    }

    /// Clic dans une zone de déplacement : AppKit déplace la fenêtre, le double-clic suit le réglage
    /// du système (agrandir ou réduire). Renvoie `true` si le clic est pris.
    fn drag_window(&self, event: &NSEvent) -> bool {
        let data = &self.ivars().data;
        if !data.custom {
            return false;
        }
        let p = self.position(event);
        let kind = data
            .regions
            .borrow()
            .iter()
            .rev()
            .find(|r| r.rect.contains(p))
            .map(|r| r.kind);
        if kind != Some(RegionKind::Drag) {
            return false;
        }
        let Some(window) = self.window() else {
            return false;
        };
        if event.clickCount() == 2 {
            let action = NSUserDefaults::standardUserDefaults()
                .stringForKey(&NSString::from_str("AppleActionOnDoubleClick"))
                .map(|s| s.to_string());
            match action.as_deref() {
                Some("Minimize") => window.miniaturize(None),
                Some("None") => {}
                _ => window.zoom(None),
            }
        } else {
            window.performWindowDragWithEvent(event);
        }
        true
    }

    fn moved(&self, event: &NSEvent) {
        push(PlatformEvent::PointerMoved {
            window: self.id(),
            position: self.position(event),
        });
        pump();
    }

    fn button(&self, event: &NSEvent, button: MouseButton, pressed: bool) {
        push(PlatformEvent::PointerButton {
            window: self.id(),
            position: self.position(event),
            button,
            pressed,
        });
        pump();
    }
}

fn pixels(size: Size, scale: f32) -> (u32, u32) {
    (
        (size.width * scale).round() as u32,
        (size.height * scale).round() as u32,
    )
}

// ---------------------------------------------------------------------------------------------
// Le délégué de fenêtre

#[derive(Debug)]
struct DelegateIvars {
    data: Rc<WinData>,
}

define_class!(
    // SAFETY : NSObject n'a pas d'exigence pour être sous-classé ; pas de `Drop`.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "VekaWindowDelegate"]
    #[ivars = DelegateIvars]
    #[derive(Debug)]
    struct WindowDelegate;

    unsafe impl NSObjectProtocol for WindowDelegate {}

    unsafe impl NSWindowDelegate for WindowDelegate {
        #[unsafe(method(windowShouldClose:))]
        fn window_should_close(&self, _sender: &NSWindow) -> bool {
            // C'est l'application qui décide de fermer.
            push(PlatformEvent::CloseRequested {
                window: self.ivars().data.id,
            });
            pump();
            false
        }

        #[unsafe(method(windowWillClose:))]
        fn window_will_close(&self, _notification: &NSNotification) {
            push(PlatformEvent::WindowClosed {
                window: self.ivars().data.id,
            });
            pump();
        }

        #[unsafe(method(windowDidResize:))]
        fn did_resize(&self, notification: &NSNotification) {
            self.state_changed(notification);
        }

        #[unsafe(method(windowDidBecomeKey:))]
        fn did_become_key(&self, notification: &NSNotification) {
            self.state_changed(notification);
        }

        #[unsafe(method(windowDidResignKey:))]
        fn did_resign_key(&self, notification: &NSNotification) {
            self.state_changed(notification);
        }

        #[unsafe(method(windowDidEnterFullScreen:))]
        fn did_enter_full_screen(&self, notification: &NSNotification) {
            self.state_changed(notification);
        }

        #[unsafe(method(windowDidExitFullScreen:))]
        fn did_exit_full_screen(&self, notification: &NSNotification) {
            self.state_changed(notification);
        }
    }
);

fn window_info(window: &NSWindow) -> WindowInfo {
    WindowInfo {
        maximized: window.isZoomed(),
        fullscreen: window.styleMask().contains(NSWindowStyleMask::FullScreen),
        active: window.isKeyWindow(),
    }
}

impl WindowDelegate {
    fn new(mtm: MainThreadMarker, data: Rc<WinData>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(DelegateIvars { data });
        // SAFETY : `init` de NSObject.
        unsafe { msg_send![super(this), init] }
    }

    /// Envoie le nouvel état de la fenêtre s'il a changé.
    fn state_changed(&self, notification: &NSNotification) {
        let Some(object) = notification.object() else {
            return;
        };
        let Ok(window) = object.downcast::<NSWindow>() else {
            return;
        };
        let info = window_info(&window);
        let data = &self.ivars().data;
        if info != data.info.replace(info) {
            push(PlatformEvent::WindowStateChanged {
                window: data.id,
                info,
            });
            pump();
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Le délégué d'application

define_class!(
    // SAFETY : NSObject n'a pas d'exigence pour être sous-classé ; pas de `Drop`.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "VekaAppDelegate"]
    struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, _notification: &NSNotification) {
            let mtm = MainThreadMarker::from(self);
            NSApplication::sharedApplication(mtm).activate();
            push(PlatformEvent::Resumed);
            pump();
        }
    }
);

impl AppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        // SAFETY : `init` de NSObject.
        unsafe { msg_send![super(this), init] }
    }
}

// ---------------------------------------------------------------------------------------------
// Fenêtre et contexte

/// Une fenêtre AppKit ouverte par Veka.
#[derive(Debug)]
pub struct MacWindow {
    data: Rc<WinData>,
    window: Retained<NSWindow>,
    view: Retained<VekaView>,
    _delegate: Retained<WindowDelegate>,
}

impl MacWindow {
    pub fn ns_window(&self) -> &NSWindow {
        &self.window
    }

    pub fn ns_view(&self) -> &NSView {
        &self.view
    }

    /// Les zones envoyées par [`PlatformWindow::set_regions`] (utilisées par `Chrome::Custom`).
    pub fn regions(&self) -> Vec<WindowRegion> {
        self.data.regions.borrow().clone()
    }
}

impl HasWindowHandle for MacWindow {
    fn window_handle(&self) -> std::result::Result<WindowHandle<'_>, HandleError> {
        let view = NonNull::from(&*self.view).cast::<c_void>();
        // SAFETY : la vue est retenue par ce `MacWindow`.
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::AppKit(AppKitWindowHandle::new(view))) })
    }
}

impl HasDisplayHandle for MacWindow {
    fn display_handle(&self) -> std::result::Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::appkit())
    }
}

impl PlatformWindow for MacWindow {
    fn id(&self) -> WindowId {
        self.data.id
    }

    fn size(&self) -> Size {
        self.data.size.get()
    }

    fn pixel_size(&self) -> (u32, u32) {
        pixels(self.data.size.get(), self.data.scale.get())
    }

    fn scale(&self) -> f32 {
        self.data.scale.get()
    }

    fn set_title(&self, title: &str) {
        self.window.setTitle(&NSString::from_str(title));
    }

    fn request_redraw(&self) {
        self.view.setNeedsDisplay(true);
    }

    fn redraw_requester(&self) -> Box<dyn Fn() + 'static> {
        let view = self.view.clone();
        Box::new(move || view.setNeedsDisplay(true))
    }

    fn set_regions(&self, regions: &[WindowRegion]) {
        *self.data.regions.borrow_mut() = regions.to_vec();
    }

    fn info(&self) -> WindowInfo {
        window_info(&self.window)
    }

    fn perform(&self, action: WindowAction) {
        match action {
            WindowAction::Minimize => self.window.miniaturize(None),
            WindowAction::ToggleMaximize => self.window.zoom(None),
            WindowAction::Close => self.window.performClose(None),
            WindowAction::ToggleFullscreen => self.window.toggleFullScreen(None),
        }
    }
}

/// Barre transparente pour `Hidden`, feux masqués en plus pour `Custom`.
fn decorate(window: &NSWindow, chrome: Chrome) {
    let Chrome::Custom {
        transparent, shadow, ..
    } = chrome
    else {
        if chrome == Chrome::Hidden {
            window.setTitlebarAppearsTransparent(true);
            window.setTitleVisibility(NSWindowTitleVisibility::Hidden);
        }
        return;
    };
    window.setTitlebarAppearsTransparent(true);
    window.setTitleVisibility(NSWindowTitleVisibility::Hidden);
    for b in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ] {
        if let Some(button) = window.standardWindowButton(b) {
            button.setHidden(true);
        }
    }
    window.setHasShadow(shadow);
    if transparent {
        window.setOpaque(false);
        window.setBackgroundColor(Some(&NSColor::clearColor()));
    }
}

struct Ctx {
    mtm: MainThreadMarker,
    windows: HashMap<WindowId, MacWindow>,
    next_id: u64,
    start: std::time::Instant,
}

impl PlatformContext for Ctx {
    fn create_window(&mut self, desc: WindowDesc) -> Result<WindowId> {
        let mtm = self.mtm;
        let id = WindowId(self.next_id);
        self.next_id += 1;

        let mut style =
            NSWindowStyleMask::Titled | NSWindowStyleMask::Closable | NSWindowStyleMask::Miniaturizable;
        if desc.resizable {
            style |= NSWindowStyleMask::Resizable;
        }
        // La fenêtre reste « titrée » pour garder bords, ombre, coins et plein écran du système ;
        // le contenu passe simplement sous la barre.
        let custom = desc.chrome != Chrome::System;
        if custom {
            style |= NSWindowStyleMask::FullSizeContentView;
        }
        let rect = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(desc.size.width as f64, desc.size.height as f64),
        );
        // SAFETY : initialiseur désigné de NSWindow, sur le fil principal.
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect,
                style,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        // SAFETY : la fenêtre est retenue par `MacWindow` ; AppKit ne doit pas la libérer à la fermeture.
        unsafe { window.setReleasedWhenClosed(false) };
        window.setTitle(&NSString::from_str(&desc.title));
        window.setAcceptsMouseMovedEvents(true);
        if let Some(min) = desc.min_size {
            window.setContentMinSize(NSSize::new(min.width as f64, min.height as f64));
        }
        decorate(&window, desc.chrome);

        let data = Rc::new(WinData {
            id,
            scale: Cell::new(window.backingScaleFactor() as f32),
            size: Cell::new(desc.size),
            regions: RefCell::new(Vec::new()),
            custom,
            info: Cell::new(window_info(&window)),
        });
        let view = VekaView::new(mtm, data.clone(), rect);
        window.setContentView(Some(&view));
        let delegate = WindowDelegate::new(mtm, data.clone());
        window.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        window.center();
        window.makeKeyAndOrderFront(None);

        self.windows.insert(
            id,
            MacWindow {
                data,
                window,
                view,
                _delegate: delegate,
            },
        );
        Ok(id)
    }

    fn window(&self, id: WindowId) -> &dyn PlatformWindow {
        self.windows.get(&id).expect("fenêtre inconnue")
    }

    fn close_window(&mut self, id: WindowId) {
        if let Some(w) = self.windows.remove(&id) {
            w.window.setDelegate(None);
            w.window.close();
        }
    }

    fn now(&self) -> Instant {
        Instant::from_nanos(self.start.elapsed().as_nanos() as u64)
    }

    fn exit(&mut self) {
        let app = NSApplication::sharedApplication(self.mtm);
        app.stop(None);
        // `stop` n'agit qu'après le prochain événement : on en poste un vide.
        let event = NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
            NSEventType::ApplicationDefined,
            NSPoint::new(0.0, 0.0),
            NSEventModifierFlags::empty(),
            0.0,
            0,
            None,
            0,
            0,
            0,
        );
        if let Some(event) = event {
            app.postEvent_atStart(&event, false);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Boucle

struct Runner {
    app: Box<dyn PlatformApp>,
    ctx: Ctx,
}

thread_local! {
    static RUNNER: RefCell<Option<Runner>> = const { RefCell::new(None) };
    static QUEUE: RefCell<VecDeque<PlatformEvent>> = const { RefCell::new(VecDeque::new()) };
}

fn push(event: PlatformEvent) {
    QUEUE.with(|q| q.borrow_mut().push_back(event));
}

/// Donne les événements en attente à l'application, sauf si elle est déjà en train d'en traiter un.
fn pump() {
    RUNNER.with(|runner| {
        let Ok(mut guard) = runner.try_borrow_mut() else {
            return;
        };
        let Some(runner) = guard.as_mut() else { return };
        while let Some(event) = QUEUE.with(|q| q.borrow_mut().pop_front()) {
            runner.app.event(&mut runner.ctx, event);
        }
    });
}

impl Platform for MacPlatform {
    fn run(self, app: Box<dyn PlatformApp>) -> Result<()> {
        let mtm = MainThreadMarker::new()
            .ok_or_else(|| PlatformError::Os("AppKit doit tourner sur le fil principal".into()))?;
        let ns_app = NSApplication::sharedApplication(mtm);
        ns_app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        let delegate = AppDelegate::new(mtm);
        ns_app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

        let ctx = Ctx {
            mtm,
            windows: HashMap::new(),
            next_id: 1,
            start: std::time::Instant::now(),
        };
        RUNNER.with(|r| *r.borrow_mut() = Some(Runner { app, ctx }));
        ns_app.run();

        // Fin : l'application d'abord (ses renderers utilisent les vues), puis les fenêtres.
        let runner = RUNNER.with(|r| r.borrow_mut().take());
        if let Some(Runner { app, mut ctx }) = runner {
            drop(app);
            let ids: Vec<WindowId> = ctx.windows.keys().copied().collect();
            for id in ids {
                ctx.close_window(id);
            }
        }
        QUEUE.with(|q| q.borrow_mut().clear());
        ns_app.setDelegate(None);
        Ok(())
    }
}
