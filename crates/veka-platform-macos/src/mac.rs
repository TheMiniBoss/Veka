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
use objc2::runtime::{AnyClass, AnyObject, ProtocolObject};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel,
};
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication,
    NSApplicationActivationPolicy, NSApplicationDelegate, NSApplicationTerminateReply, NSBackingStoreType,
    NSColor, NSColorSpace, NSEvent, NSEventModifierFlags, NSEventType, NSMenu, NSMenuItem, NSResponder,
    NSRunningApplication, NSSystemColorsDidChangeNotification, NSTrackingArea, NSTrackingAreaOptions, NSView,
    NSWindow, NSWindowButton, NSWindowDelegate, NSWindowStyleMask, NSWindowTitleVisibility, NSWorkspace,
    NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification,
};
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSObject, NSObjectProtocol, NSPoint, NSProcessInfo, NSRect, NSSize,
    NSString, NSUserDefaults,
};
use raw_window_handle::{
    AppKitWindowHandle, DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawWindowHandle,
    WindowHandle,
};
use veka_platform::{
    Appearance, Chrome, Color, ColorScheme, Instant, Key, Modifiers, MouseButton, Platform, PlatformApp,
    PlatformContext, PlatformError, PlatformEvent, PlatformWindow, Point, Rect, RegionKind, Result, Size,
    WindowAction, WindowDesc, WindowId, WindowInfo, WindowRegion,
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
    /// `Chrome::Custom { transparent: true }`.
    transparent: bool,
    /// Dernier état envoyé à l'application.
    info: Cell<WindowInfo>,
    /// Fenêtre transparente : les zones dessinées (points), hors desquelles les clics passent.
    hit_area: RefCell<Option<Vec<Rect>>>,
}

// ---------------------------------------------------------------------------------------------
// La vue

#[derive(Debug)]
struct ViewIvars {
    data: Rc<WinData>,
    /// Le `CADisplayLink` de la vue (macOS 14+), créé à la première image demandée. Il cadence les
    /// animations : `setNeedsDisplay` appelé pendant `drawRect:` est oublié par AppKit, donc une
    /// animation qui redemande une image depuis son rendu s'arrêterait après la première.
    link: RefCell<Option<Retained<AnyObject>>>,
    /// Une image est demandée pour la prochaine synchronisation verticale.
    wants_frame: Cell<bool>,
    /// Fenêtre transparente : minuterie qui regarde si le pointeur est sur une zone dessinée.
    poll: RefCell<Option<Retained<AnyObject>>>,
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

        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            self.key(event, true);
        }

        #[unsafe(method(keyUp:))]
        fn key_up(&self, event: &NSEvent) {
            self.key(event, false);
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            push(PlatformEvent::RedrawRequested { window: self.id() });
            pump();
        }

        #[unsafe(method(vekaDisplayLinkFired:))]
        fn display_link_fired(&self, _link: &AnyObject) {
            if self.ivars().wants_frame.replace(false) {
                push(PlatformEvent::RedrawRequested { window: self.id() });
                pump();
            }
            // Rien de redemandé pendant le rendu : on arrête le lien (0 % de CPU au repos).
            if !self.ivars().wants_frame.get() {
                self.set_link_paused(true);
            }
        }

        /// Fenêtre transparente : hors des zones dessinées, la fenêtre ignore la souris et les clics
        /// passent aux fenêtres de dessous (pas pendant un glisser).
        #[unsafe(method(vekaPollPointer:))]
        fn poll_pointer(&self, _timer: &AnyObject) {
            let Some(window) = self.window() else {
                return;
            };
            if NSEvent::pressedMouseButtons() != 0 {
                return;
            }
            let in_window = window.convertPointFromScreen(NSEvent::mouseLocation());
            let p = self.convertPoint_fromView(in_window, None);
            let p = Point::new(p.x as f32, p.y as f32);
            let ignore = match self.ivars().data.hit_area.borrow().as_ref() {
                Some(area) => !area.iter().any(|r| r.contains(p)),
                None => false,
            };
            if window.ignoresMouseEvents() != ignore {
                window.setIgnoresMouseEvents(ignore);
            }
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

        #[unsafe(method(viewDidChangeEffectiveAppearance))]
        fn did_change_appearance(&self) {
            refresh_appearance(MainThreadMarker::from(self));
            pump();
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

/// Les modificateurs d'un événement.
fn modifiers_of(event: &NSEvent) -> Modifiers {
    let f = event.modifierFlags();
    let mut m = Modifiers::NONE;
    if f.contains(NSEventModifierFlags::Shift) {
        m |= Modifiers::SHIFT;
    }
    if f.contains(NSEventModifierFlags::Control) {
        m |= Modifiers::CONTROL;
    }
    if f.contains(NSEventModifierFlags::Option) {
        m |= Modifiers::ALT;
    }
    if f.contains(NSEventModifierFlags::Command) {
        m |= Modifiers::META;
    }
    m
}

/// La touche logique d'un événement : les touches nommées par leur code (kVK_*), les autres par leur
/// caractère sans modificateur, dans la disposition active.
fn key_of(event: &NSEvent) -> Option<Key> {
    Some(match event.keyCode() {
        36 | 76 => Key::Enter,
        48 => Key::Tab,
        53 => Key::Escape,
        51 => Key::Backspace,
        117 => Key::Delete,
        114 => Key::Insert,
        123 => Key::ArrowLeft,
        124 => Key::ArrowRight,
        125 => Key::ArrowDown,
        126 => Key::ArrowUp,
        115 => Key::Home,
        119 => Key::End,
        116 => Key::PageUp,
        121 => Key::PageDown,
        122 => Key::F(1),
        120 => Key::F(2),
        99 => Key::F(3),
        118 => Key::F(4),
        96 => Key::F(5),
        97 => Key::F(6),
        98 => Key::F(7),
        100 => Key::F(8),
        101 => Key::F(9),
        109 => Key::F(10),
        103 => Key::F(11),
        111 => Key::F(12),
        105 => Key::F(13),
        107 => Key::F(14),
        113 => Key::F(15),
        106 => Key::F(16),
        64 => Key::F(17),
        79 => Key::F(18),
        80 => Key::F(19),
        90 => Key::F(20),
        _ => {
            let s = event.charactersIgnoringModifiers()?.to_string();
            let c = s.chars().next().filter(|c| !c.is_control())?;
            // Les touches de fonction d'AppKit (U+F700…) sans code connu sont ignorées.
            if ('\u{F700}'..='\u{F8FF}').contains(&c) {
                return None;
            }
            Key::Character(c.to_lowercase().next().unwrap_or(c))
        }
    })
}

fn other_button(event: &NSEvent) -> MouseButton {
    match event.buttonNumber() {
        2 => MouseButton::Middle,
        n => MouseButton::Other(n as u16),
    }
}

impl VekaView {
    fn new(mtm: MainThreadMarker, data: Rc<WinData>, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ViewIvars {
            data,
            link: RefCell::new(None),
            wants_frame: Cell::new(false),
            poll: RefCell::new(None),
        });
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

    /// Une touche : l'événement `Key`, puis, à l'appui, le texte tapé (sans ⌘ ni Ctrl, qui font des
    /// raccourcis). `characters` donne déjà le caractère composé après une touche morte.
    fn key(&self, event: &NSEvent, pressed: bool) {
        let window = self.id();
        let modifiers = modifiers_of(event);
        if let Some(key) = key_of(event) {
            push(PlatformEvent::Key {
                window,
                key,
                modifiers,
                pressed,
                repeat: pressed && event.isARepeat(),
            });
        }
        if pressed && !modifiers.contains(Modifiers::META) && !modifiers.contains(Modifiers::CONTROL) {
            let text = event.characters().map(|s| s.to_string()).unwrap_or_default();
            for ch in text.chars() {
                if !ch.is_control() && !('\u{F700}'..='\u{F8FF}').contains(&ch) {
                    push(PlatformEvent::Char { window, ch });
                }
            }
        }
        pump();
    }

    /// Demande une image à la prochaine synchronisation verticale de l'écran de la fenêtre.
    fn request_frame(&self) {
        self.ivars().wants_frame.set(true);
        if self.ivars().link.borrow().is_none() {
            // SAFETY : `displayLinkWithTarget:selector:` (NSView, macOS 14) renvoie un
            // CADisplayLink autoreleasé ; la cible (la vue) répond au sélecteur. Le lien retient la
            // vue : il est invalidé à la fermeture (`invalidate_link`).
            let has_link: bool =
                unsafe { msg_send![self, respondsToSelector: sel!(displayLinkWithTarget:selector:)] };
            let link: Option<Retained<AnyObject>> = if has_link {
                unsafe { msg_send![self, displayLinkWithTarget: self, selector: sel!(vekaDisplayLinkFired:)] }
            } else {
                None
            };
            let Some(link) = link else {
                // Sans lien (ancien macOS) : le dessin d'AppKit, en dehors de `drawRect:`.
                let view = self.retain();
                // SAFETY : la file principale existe toute la vie du processus.
                unsafe {
                    dispatch_async_f(
                        &raw const _dispatch_main_q,
                        Retained::into_raw(view).cast(),
                        needs_display_on_main,
                    )
                };
                return;
            };
            let run_loop = AnyClass::get(c"NSRunLoop").expect("NSRunLoop");
            // SAFETY : `mainRunLoop` existe toujours ; `NSRunLoopCommonModes` est une constante de
            // Foundation. Les modes communs gardent les animations pendant un redimensionnement.
            unsafe {
                let main: *mut AnyObject = msg_send![run_loop, mainRunLoop];
                let _: () = msg_send![&*link, addToRunLoop: main, forMode: NSRunLoopCommonModes];
            }
            *self.ivars().link.borrow_mut() = Some(link);
        }
        self.set_link_paused(false);
    }

    fn set_link_paused(&self, paused: bool) {
        if let Some(link) = self.ivars().link.borrow().as_ref() {
            // SAFETY : `setPaused:` de CADisplayLink.
            let _: () = unsafe { msg_send![&**link, setPaused: paused] };
        }
    }

    /// Lance la minuterie de `vekaPollPointer:` (20 fois par seconde), une seule fois.
    fn start_pointer_poll(&self) {
        if self.ivars().poll.borrow().is_some() {
            return;
        }
        let class = AnyClass::get(c"NSTimer").expect("NSTimer");
        // SAFETY : `scheduledTimerWithTimeInterval:target:selector:userInfo:repeats:` ajoute la
        // minuterie à la boucle courante (la principale) ; la vue répond au sélecteur. La minuterie
        // retient la vue : elle est invalidée à la fermeture.
        let timer: Option<Retained<AnyObject>> = unsafe {
            msg_send![class, scheduledTimerWithTimeInterval: 0.05f64, target: self,
                selector: sel!(vekaPollPointer:), userInfo: std::ptr::null::<AnyObject>(), repeats: true]
        };
        *self.ivars().poll.borrow_mut() = timer;
    }

    /// Retire le lien de la boucle (il retient la vue).
    fn invalidate_link(&self) {
        if let Some(timer) = self.ivars().poll.borrow_mut().take() {
            // SAFETY : `invalidate` de NSTimer.
            let _: () = unsafe { msg_send![&*timer, invalidate] };
        }
        if let Some(link) = self.ivars().link.borrow_mut().take() {
            // SAFETY : `invalidate` de CADisplayLink.
            let _: () = unsafe { msg_send![&*link, invalidate] };
        }
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
            // La couleur d'accent n'a pas d'avis sur la vue : on la relit quand la fenêtre revient.
            refresh_appearance(MainThreadMarker::from(self));
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

        /// ⌘Q, « Quitter » du Dock, fermeture de session : comme sur Windows, chaque fenêtre reçoit
        /// une demande de fermeture et l'application décide ; la boucle s'arrête quand la dernière
        /// est fermée. AppKit ne termine donc jamais le processus lui-même (les renderers sont
        /// détruits proprement).
        #[unsafe(method(applicationShouldTerminate:))]
        fn should_terminate(&self, _sender: &NSApplication) -> NSApplicationTerminateReply {
            request_close_all();
            NSApplicationTerminateReply::TerminateCancel
        }
    }

    impl AppDelegate {
        /// « Réduire les animations », contraste… (Réglages > Accessibilité) ou couleur d'accent
        /// changés pendant que l'application tourne.
        #[unsafe(method(vekaSystemSettingsChanged:))]
        fn system_settings_changed(&self, _notification: &NSNotification) {
            refresh_appearance(MainThreadMarker::from(self));
            pump();
        }
    }
);

impl AppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        // SAFETY : `init` de NSObject.
        let this: Retained<Self> = unsafe { msg_send![super(this), init] };
        let selector = sel!(vekaSystemSettingsChanged:);
        // SAFETY : le délégué répond au sélecteur ; il vit jusqu'à la fin de `run`, où il est retiré
        // des deux centres (`stop_observing`).
        unsafe {
            NSWorkspace::sharedWorkspace()
                .notificationCenter()
                .addObserver_selector_name_object(
                    &this,
                    selector,
                    Some(NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification),
                    None,
                );
            NSNotificationCenter::defaultCenter().addObserver_selector_name_object(
                &this,
                selector,
                Some(NSSystemColorsDidChangeNotification),
                None,
            );
        }
        this
    }

    fn stop_observing(&self) {
        // SAFETY : retire l'observateur ajouté dans `new`.
        unsafe {
            NSWorkspace::sharedWorkspace()
                .notificationCenter()
                .removeObserver(self);
            NSNotificationCenter::defaultCenter().removeObserver(self);
        }
    }
}

/// Demande la fermeture de toutes les fenêtres (voir `applicationShouldTerminate:`).
fn request_close_all() {
    let ids: Vec<WindowId> = RUNNER.with(|r| {
        r.try_borrow()
            .ok()
            .and_then(|g| g.as_ref().map(|r| r.ctx.windows.keys().copied().collect()))
            .unwrap_or_default()
    });
    for window in ids {
        push(PlatformEvent::CloseRequested { window });
    }
    pump();
}

/// Le menu d'application minimal que macOS attend : sans lui, ni ⌘Q ni ⌘H ni ⌘W.
fn install_menu(mtm: MainThreadMarker, app: &NSApplication) {
    // Le nom du paquet .app s'il y en a un, sinon celui de l'exécutable.
    let name = NSRunningApplication::currentApplication()
        .localizedName()
        .unwrap_or_else(|| NSProcessInfo::processInfo().processName())
        .to_string();
    let item = |title: &str, action: Option<objc2::runtime::Sel>, key: &str| {
        // SAFETY : initialiseur désigné ; les actions sont des méthodes standard d'AppKit, envoyées
        // à la chaîne de répondeurs.
        unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(title),
                action,
                &NSString::from_str(key),
            )
        }
    };
    let submenu = |title: &str, items: Vec<Retained<NSMenuItem>>| {
        let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(title));
        for i in &items {
            menu.addItem(i);
        }
        let top = item(title, None, "");
        top.setSubmenu(Some(&menu));
        (top, menu)
    };

    let hide_others = item("Masquer les autres", Some(sel!(hideOtherApplications:)), "h");
    hide_others.setKeyEquivalentModifierMask(NSEventModifierFlags::Command | NSEventModifierFlags::Option);
    let (app_item, _) = submenu(
        &name,
        vec![
            item(&format!("Masquer {name}"), Some(sel!(hide:)), "h"),
            hide_others,
            item("Tout afficher", Some(sel!(unhideAllApplications:)), ""),
            NSMenuItem::separatorItem(mtm),
            item(&format!("Quitter {name}"), Some(sel!(terminate:)), "q"),
        ],
    );
    let full_screen = item("Plein écran", Some(sel!(toggleFullScreen:)), "f");
    full_screen.setKeyEquivalentModifierMask(NSEventModifierFlags::Command | NSEventModifierFlags::Control);
    // `miniaturize:` et `zoom:` plutôt que `performMiniaturize:` / `performZoom:` : ces derniers
    // simulent un clic sur les boutons, masqués avec `Chrome::Custom`.
    let (window_item, window_menu) = submenu(
        "Fenêtre",
        vec![
            item("Réduire", Some(sel!(miniaturize:)), "m"),
            item("Zoom", Some(sel!(zoom:)), ""),
            full_screen,
            NSMenuItem::separatorItem(mtm),
            item("Fermer", Some(sel!(performClose:)), "w"),
        ],
    );
    let bar = NSMenu::new(mtm);
    bar.addItem(&app_item);
    bar.addItem(&window_item);
    app.setMainMenu(Some(&bar));
    app.setWindowsMenu(Some(&window_menu));
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
        self.view.request_frame();
    }

    fn redraw_requester(&self) -> Box<dyn Fn() + 'static> {
        let view = self.view.clone();
        Box::new(move || view.request_frame())
    }

    fn set_regions(&self, regions: &[WindowRegion]) {
        *self.data.regions.borrow_mut() = regions.to_vec();
    }

    fn set_hit_area(&self, area: Option<&[Rect]>) {
        if !self.data.transparent {
            return;
        }
        *self.data.hit_area.borrow_mut() = area.map(<[Rect]>::to_vec);
        if area.is_some() {
            self.view.start_pointer_poll();
        }
    }

    fn set_color_scheme(&self, scheme: Option<ColorScheme>) {
        // `nil` : la fenêtre reprend l'apparence de l'application (celle du système).
        // SAFETY : constantes d'AppKit, lues sur le fil principal.
        let name = unsafe {
            match scheme {
                Some(ColorScheme::Dark) => Some(NSAppearanceNameDarkAqua),
                Some(ColorScheme::Light) => Some(NSAppearanceNameAqua),
                None => None,
            }
        };
        let appearance = name.and_then(NSAppearance::appearanceNamed);
        self.window.setAppearance(appearance.as_deref());
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
            hit_area: RefCell::new(None),
            transparent: desc.chrome.is_transparent(),
        });
        let view = VekaView::new(mtm, data.clone(), rect);
        crate::vibrancy::set_content(&window, &view, desc.backdrop);
        let delegate = WindowDelegate::new(mtm, data.clone());
        window.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        window.center();
        // La vue reçoit les touches.
        window.makeFirstResponder(Some(&view));
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
            w.view.invalidate_link();
            w.window.setDelegate(None);
            w.window.close();
        }
    }

    fn now(&self) -> Instant {
        Instant::from_nanos(self.start.elapsed().as_nanos() as u64)
    }

    fn appearance(&self) -> Appearance {
        APPEARANCE.with(|a| a.get())
    }

    fn waker(&self) -> Option<veka_platform::Waker> {
        // SAFETY : la file principale existe toute la vie du processus ; `wake_on_main` n'utilise
        // pas le contexte.
        Some(std::sync::Arc::new(|| unsafe {
            dispatch_async_f(&raw const _dispatch_main_q, std::ptr::null_mut(), wake_on_main);
        }))
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
    static APPEARANCE: Cell<Appearance> = Cell::new(Appearance::default());
}

/// Mode clair / sombre de l'application (celui des Réglages, sauf si l'app le force) et couleur
/// d'accent du système.
fn read_appearance(mtm: MainThreadMarker) -> Appearance {
    let name = NSApplication::sharedApplication(mtm).effectiveAppearance().name();
    // `DarkAqua`, `VibrantDark`, `AccessibilityHighContrastDarkAqua`…
    let scheme = if name.to_string().contains("Dark") {
        ColorScheme::Dark
    } else {
        ColorScheme::Light
    };
    let accent = NSColor::controlAccentColor()
        .colorUsingColorSpace(&NSColorSpace::sRGBColorSpace())
        .map(|c| {
            Color::rgb(
                c.redComponent() as f32,
                c.greenComponent() as f32,
                c.blueComponent() as f32,
            )
        });
    let workspace = NSWorkspace::sharedWorkspace();
    let reduce_motion = workspace.accessibilityDisplayShouldReduceMotion();
    let reduce_transparency = workspace.accessibilityDisplayShouldReduceTransparency();
    Appearance {
        scheme,
        accent,
        reduce_motion,
        reduce_transparency,
    }
}

/// Relit l'apparence ; prévient l'application si elle a changé.
fn refresh_appearance(mtm: MainThreadMarker) {
    let now = read_appearance(mtm);
    if APPEARANCE.with(|a| a.replace(now)) != now {
        push(PlatformEvent::AppearanceChanged { appearance: now });
    }
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

/// libdispatch (dans libSystem) : la file principale tourne dans la boucle d'AppKit.
#[repr(C)]
struct DispatchQueue {
    _private: [u8; 0],
}

unsafe extern "C" {
    static _dispatch_main_q: DispatchQueue;
    fn dispatch_async_f(
        queue: *const DispatchQueue,
        context: *mut std::ffi::c_void,
        work: extern "C" fn(*mut std::ffi::c_void),
    );
}

unsafe extern "C" {
    static NSRunLoopCommonModes: &'static NSString;
}

extern "C" fn needs_display_on_main(view: *mut std::ffi::c_void) {
    // SAFETY : `view` vient de `Retained::into_raw` dans `request_frame`.
    let view = unsafe { Retained::from_raw(view.cast::<VekaView>()) }.expect("vue");
    view.setNeedsDisplay(true);
}

extern "C" fn wake_on_main(_: *mut std::ffi::c_void) {
    push(PlatformEvent::Wake);
    pump();
}

impl Platform for MacPlatform {
    fn run(self, app: Box<dyn PlatformApp>) -> Result<()> {
        let mtm = MainThreadMarker::new()
            .ok_or_else(|| PlatformError::Os("AppKit doit tourner sur le fil principal".into()))?;
        let ns_app = NSApplication::sharedApplication(mtm);
        ns_app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        let delegate = AppDelegate::new(mtm);
        ns_app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        install_menu(mtm, &ns_app);

        let ctx = Ctx {
            mtm,
            windows: HashMap::new(),
            next_id: 1,
            start: std::time::Instant::now(),
        };
        APPEARANCE.with(|a| a.set(read_appearance(mtm)));
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
        delegate.stop_observing();
        ns_app.setDelegate(None);
        Ok(())
    }
}
