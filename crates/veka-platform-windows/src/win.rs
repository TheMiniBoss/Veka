//! Implémentation Win32.
//!
//! Les messages arrivent dans `wndproc`, parfois de façon imbriquée (par exemple `CreateWindowExW`
//! envoie `WM_SIZE` pendant que l'application traite un événement). On les met donc dans une file,
//! et on ne la vide que si l'application n'est pas déjà en train de traiter un événement.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::ffi::c_void;
use std::num::NonZeroIsize;
use std::rc::Rc;

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawWindowHandle, Win32WindowHandle,
    WindowHandle,
};
use veka_platform::{
    Appearance, Chrome, Color, ColorScheme, Instant, MouseButton, Platform, PlatformApp, PlatformContext,
    PlatformError, PlatformEvent, PlatformWindow, Point, RegionKind, Result, Size, SystemBackdrop,
    WindowAction, WindowControl, WindowDesc, WindowId, WindowInfo, WindowRegion,
};
use windows::Win32::Foundation::{
    ERROR_CLASS_ALREADY_EXISTS, GetLastError, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM,
};
use windows::Win32::Graphics::Dwm::{
    DWMSBT_MAINWINDOW, DWMSBT_TABBEDWINDOW, DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE,
    DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DWMWCP_ROUND,
    DWMWCP_ROUNDSMALL, DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{InvalidateRect, ScreenToClient, ValidateRect};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Controls::{MARGINS, WM_MOUSELEAVE};
use windows::Win32::UI::HiDpi::{
    AdjustWindowRectExForDpi, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForSystem, GetDpiForWindow,
    GetSystemMetricsForDpi, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow,
    DispatchMessageW, GWL_EXSTYLE, GWL_STYLE, GetClientRect, GetMessageW, GetWindowLongW, HTBOTTOM,
    HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION, HTCLIENT, HTLEFT, HTMAXBUTTON, HTNOWHERE, HTRIGHT, HTTOP,
    HTTOPLEFT, HTTOPRIGHT, IDC_ARROW, IsZoomed, LoadCursorW, MINMAXINFO, MSG, NCCALCSIZE_PARAMS,
    PostMessageW, PostQuitMessage, PostThreadMessageW, RegisterClassExW, SIZE_MAXIMIZED, SIZE_MINIMIZED,
    SM_CXFRAME, SM_CXPADDEDBORDER, SM_CYFRAME, SPI_GETCLIENTAREAANIMATION, SW_MAXIMIZE, SW_MINIMIZE,
    SW_RESTORE, SW_SHOW, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SetWindowPos, SetWindowTextW, ShowWindow, SystemParametersInfoW,
    TranslateMessage, WA_INACTIVE, WINDOW_EX_STYLE, WINDOW_STYLE, WM_ACTIVATE, WM_APP, WM_CAPTURECHANGED,
    WM_CLOSE, WM_DESTROY, WM_DPICHANGED, WM_DWMCOLORIZATIONCOLORCHANGED, WM_ERASEBKGND, WM_GETMINMAXINFO,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE, WM_NCCALCSIZE, WM_NCCREATE,
    WM_NCHITTEST, WM_NCLBUTTONDBLCLK, WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_NCMOUSELEAVE, WM_NCMOUSEMOVE,
    WM_PAINT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETTINGCHANGE, WM_SIZE, WNDCLASSEXW,
    WS_EX_NOREDIRECTIONBITMAP, WS_MAXIMIZEBOX, WS_OVERLAPPEDWINDOW, WS_THICKFRAME,
};
use windows::core::{HSTRING, PCWSTR, w};

const CLASS: PCWSTR = w!("VekaWindow");
/// Message de fil (sans fenêtre) : un `Waker` a été appelé.
const WM_VEKA_WAKE: u32 = WM_APP + 1;
const BASE_DPI: f32 = 96.0;

/// La plateforme Windows. `WindowsPlatform::new().run(app)` ouvre la boucle.
#[derive(Debug, Default)]
pub struct WindowsPlatform;

impl WindowsPlatform {
    pub fn new() -> Self {
        Self
    }
}

/// Ce que la `WndProc` doit connaître d'une fenêtre. Partagé entre la fenêtre et la table des HWND.
#[derive(Debug)]
struct WinData {
    id: WindowId,
    hwnd: Cell<HWND>,
    hinstance: HINSTANCE,
    scale: Cell<f32>,
    pixels: Cell<(u32, u32)>,
    min_size: Option<Size>,
    tracking: Cell<bool>,
    buttons: Cell<u32>,
    regions: RefCell<Vec<WindowRegion>>,
    /// Sans décoration du système (`Chrome::Hidden` ou `Chrome::Custom`).
    custom: bool,
    resizable: bool,
    resize_margin: f32,
    info: Cell<WindowInfo>,
    /// Le pointeur est sur un bouton annoncé au système (zone non cliente).
    nc_tracking: Cell<bool>,
}

/// Une fenêtre Win32 ouverte par Veka.
#[derive(Debug)]
pub struct Win32Window {
    data: Rc<WinData>,
}

impl Win32Window {
    pub fn hwnd(&self) -> HWND {
        self.data.hwnd.get()
    }

    /// Les zones envoyées par [`PlatformWindow::set_regions`] (utilisées par `Chrome::Custom`).
    pub fn regions(&self) -> Vec<WindowRegion> {
        self.data.regions.borrow().clone()
    }
}

impl HasWindowHandle for Win32Window {
    fn window_handle(&self) -> std::result::Result<WindowHandle<'_>, HandleError> {
        let hwnd = NonZeroIsize::new(self.hwnd().0 as isize).ok_or(HandleError::Unavailable)?;
        let mut handle = Win32WindowHandle::new(hwnd);
        handle.hinstance = NonZeroIsize::new(self.data.hinstance.0 as isize);
        // SAFETY : la fenêtre existe tant que ce `Win32Window` existe (il est retiré avant `DestroyWindow`).
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(handle)) })
    }
}

impl HasDisplayHandle for Win32Window {
    fn display_handle(&self) -> std::result::Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::windows())
    }
}

impl PlatformWindow for Win32Window {
    fn id(&self) -> WindowId {
        self.data.id
    }

    fn size(&self) -> Size {
        let (w, h) = self.data.pixels.get();
        let s = self.data.scale.get();
        Size::new(w as f32 / s, h as f32 / s)
    }

    fn pixel_size(&self) -> (u32, u32) {
        self.data.pixels.get()
    }

    fn scale(&self) -> f32 {
        self.data.scale.get()
    }

    fn set_title(&self, title: &str) {
        // SAFETY : HWND valide (voir `window_handle`).
        unsafe {
            let _ = SetWindowTextW(self.hwnd(), &HSTRING::from(title));
        }
    }

    fn request_redraw(&self) {
        invalidate(self.hwnd().0 as isize);
    }

    fn redraw_requester(&self) -> Box<dyn Fn() + 'static> {
        let hwnd = self.hwnd().0 as isize;
        Box::new(move || invalidate(hwnd))
    }

    fn set_regions(&self, regions: &[WindowRegion]) {
        *self.data.regions.borrow_mut() = regions.to_vec();
    }

    fn info(&self) -> WindowInfo {
        self.data.info.get()
    }

    fn perform(&self, action: WindowAction) {
        let hwnd = self.hwnd();
        // SAFETY : HWND valide ; ces appels envoient des messages traités par `wndproc`.
        unsafe {
            match action {
                WindowAction::Minimize => {
                    let _ = ShowWindow(hwnd, SW_MINIMIZE);
                }
                WindowAction::ToggleMaximize => {
                    let cmd = if IsZoomed(hwnd).as_bool() {
                        SW_RESTORE
                    } else {
                        SW_MAXIMIZE
                    };
                    let _ = ShowWindow(hwnd, cmd);
                }
                WindowAction::Close => {
                    let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                }
                // Plein écran : pas encore sur Windows.
                WindowAction::ToggleFullscreen => {}
            }
        }
    }
}

/// Demande un `WM_PAINT`. Sans effet si la fenêtre n'existe plus (le système vérifie le HWND).
fn invalidate(hwnd: isize) {
    // SAFETY : `InvalidateRect` accepte un HWND périmé et renvoie simplement un échec.
    unsafe {
        let _ = InvalidateRect(Some(HWND(hwnd as *mut c_void)), None, false);
    }
}

struct Ctx {
    hinstance: HINSTANCE,
    windows: HashMap<WindowId, Win32Window>,
    next_id: u64,
    frequency: i64,
    thread: u32,
}

impl PlatformContext for Ctx {
    fn create_window(&mut self, desc: WindowDesc) -> Result<WindowId> {
        let custom = desc.chrome != Chrome::System;
        let id = WindowId(self.next_id);
        self.next_id += 1;
        let style = if desc.resizable {
            WS_OVERLAPPEDWINDOW
        } else {
            WS_OVERLAPPEDWINDOW & !(WS_THICKFRAME | WS_MAXIMIZEBOX)
        };
        // Fenêtre transparente : pas de surface GDI, le contenu vient de DirectComposition (wgpu).
        let ex_style = if desc.is_transparent() {
            WS_EX_NOREDIRECTIONBITMAP
        } else {
            WINDOW_EX_STYLE(0)
        };
        // SAFETY : appel sans pointeur.
        let dpi = unsafe { GetDpiForSystem() };
        let data = Rc::new(WinData {
            id,
            hwnd: Cell::new(HWND::default()),
            hinstance: self.hinstance,
            scale: Cell::new(dpi as f32 / BASE_DPI),
            pixels: Cell::new((0, 0)),
            min_size: desc.min_size,
            tracking: Cell::new(false),
            buttons: Cell::new(0),
            regions: RefCell::new(Vec::new()),
            custom,
            resizable: desc.resizable,
            resize_margin: desc.resize_margin,
            info: Cell::new(WindowInfo::default()),
            nc_tracking: Cell::new(false),
        });
        let (w, h) = outer_size(desc.size, style, ex_style, dpi, custom);
        // SAFETY : `data` vit au moins jusqu'à la fin de l'appel ; `WM_NCCREATE` en prend une référence.
        let hwnd = unsafe {
            CreateWindowExW(
                ex_style,
                CLASS,
                &HSTRING::from(desc.title.as_str()),
                style,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                w,
                h,
                None,
                None,
                Some(self.hinstance),
                Some(Rc::as_ptr(&data) as *const c_void),
            )
        }
        .map_err(|e| PlatformError::Os(e.message()))?;

        // La fenêtre peut s'ouvrir sur un écran dont l'échelle n'est pas celle du système.
        // SAFETY : HWND tout juste créé.
        unsafe {
            let real = GetDpiForWindow(hwnd);
            if real != 0 && real != dpi {
                data.scale.set(real as f32 / BASE_DPI);
                let (w, h) = outer_size(desc.size, style, ex_style, real, custom);
                let _ = SetWindowPos(hwnd, None, 0, 0, w, h, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
            }
            apply_scheme(hwnd, APPEARANCE.with(|a| a.get()).scheme);
            if custom {
                decorate(hwnd, desc.chrome);
            }
            if desc.backdrop != SystemBackdrop::None {
                system_backdrop(hwnd, desc.backdrop);
            }
            let mut rect = RECT::default();
            if GetClientRect(hwnd, &mut rect).is_ok() {
                data.pixels
                    .set(((rect.right - rect.left) as u32, (rect.bottom - rect.top) as u32));
            }
            let _ = ShowWindow(hwnd, SW_SHOW);
        }
        self.windows.insert(id, Win32Window { data });
        Ok(id)
    }

    fn window(&self, id: WindowId) -> &dyn PlatformWindow {
        self.windows.get(&id).expect("fenêtre inconnue")
    }

    fn close_window(&mut self, id: WindowId) {
        if let Some(window) = self.windows.remove(&id) {
            // SAFETY : HWND encore valide, plus aucune référence côté application.
            unsafe {
                let _ = DestroyWindow(window.hwnd());
            }
        }
    }

    fn now(&self) -> Instant {
        let mut counter = 0i64;
        // SAFETY : pointeur vers une variable locale.
        unsafe {
            let _ = QueryPerformanceCounter(&mut counter);
        }
        Instant::from_nanos((counter as i128 * 1_000_000_000 / self.frequency as i128) as u64)
    }

    fn exit(&mut self) {
        // SAFETY : appel sans pointeur.
        unsafe { PostQuitMessage(0) };
    }

    fn appearance(&self) -> Appearance {
        APPEARANCE.with(|a| a.get())
    }

    fn waker(&self) -> Option<veka_platform::Waker> {
        let thread = self.thread;
        // SAFETY : message sans pointeur ; un fil terminé fait simplement échouer l'appel.
        Some(std::sync::Arc::new(move || unsafe {
            let _ = PostThreadMessageW(thread, WM_VEKA_WAKE, WPARAM(0), LPARAM(0));
        }))
    }
}

/// Ombre et coins demandés à DWM pour une fenêtre sans décoration, puis recalcul du cadre.
///
/// # Safety
///
/// `hwnd` doit être une fenêtre valide.
unsafe fn decorate(hwnd: HWND, chrome: Chrome) {
    let (shadow, radius) = match chrome {
        Chrome::Custom {
            shadow,
            corner_radius,
            ..
        } => (shadow, corner_radius),
        _ => (true, 8.0),
    };
    // Une marge d'un pixel suffit pour que DWM dessine l'ombre ; ce pixel est recouvert par Veka.
    let margins = MARGINS {
        cxLeftWidth: 0,
        cxRightWidth: 0,
        cyTopHeight: i32::from(shadow),
        cyBottomHeight: 0,
    };
    // Windows 11 ne propose que deux rayons : petit (≈ 4 px) ou normal (≈ 8 px).
    let corners = if radius <= 0.0 {
        DWMWCP_DONOTROUND
    } else if radius < 6.0 {
        DWMWCP_ROUNDSMALL
    } else {
        DWMWCP_ROUND
    };
    // SAFETY : HWND valide (contrat) et pointeurs vers des variables locales.
    unsafe {
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
        // Échoue sans conséquence sur Windows 10 (coins droits).
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corners as *const _ as *const c_void,
            std::mem::size_of_val(&corners) as u32,
        );
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

/// Matériau de DWM sous tout le contenu (Windows 11 22H2 et plus ; sans effet avant).
///
/// # Safety
/// `hwnd` doit être une fenêtre valide.
unsafe fn system_backdrop(hwnd: HWND, backdrop: SystemBackdrop) {
    let kind = match backdrop {
        SystemBackdrop::Mica => DWMSBT_MAINWINDOW,
        SystemBackdrop::MicaAlt => DWMSBT_TABBEDWINDOW,
        SystemBackdrop::Acrylic => DWMSBT_TRANSIENTWINDOW,
        _ => return,
    };
    // Le cadre étendu à tout le contenu : DWM y dessine le matériau, que Veka laisse voir là où il
    // dessine du transparent.
    let margins = MARGINS {
        cxLeftWidth: -1,
        cxRightWidth: -1,
        cyTopHeight: -1,
        cyBottomHeight: -1,
    };
    // SAFETY : HWND valide (contrat) et pointeurs vers des variables locales.
    unsafe {
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            &kind as *const _ as *const c_void,
            std::mem::size_of_val(&kind) as u32,
        );
    }
}

/// Taille extérieure (bordures comprises) pour un contenu de `size` points. Sans décoration du
/// système (`custom`), la fenêtre n'est que son contenu.
fn outer_size(
    size: Size,
    style: WINDOW_STYLE,
    ex_style: WINDOW_EX_STYLE,
    dpi: u32,
    custom: bool,
) -> (i32, i32) {
    let scale = dpi as f32 / BASE_DPI;
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: (size.width * scale).round() as i32,
        bottom: (size.height * scale).round() as i32,
    };
    if custom {
        return (rect.right, rect.bottom);
    }
    // SAFETY : pointeur vers une variable locale.
    unsafe {
        let _ = AdjustWindowRectExForDpi(&mut rect, style, false, ex_style, dpi);
    }
    (rect.right - rect.left, rect.bottom - rect.top)
}

struct Runner {
    app: Box<dyn PlatformApp>,
    ctx: Ctx,
}

thread_local! {
    static RUNNER: RefCell<Option<Runner>> = const { RefCell::new(None) };
    static QUEUE: RefCell<VecDeque<PlatformEvent>> = const { RefCell::new(VecDeque::new()) };
    static BY_HWND: RefCell<HashMap<isize, Rc<WinData>>> = RefCell::new(HashMap::new());
    static APPEARANCE: Cell<Appearance> = Cell::new(Appearance::default());
}

/// Lit un DWORD de `HKEY_CURRENT_USER`.
fn read_dword(key: PCWSTR, value: PCWSTR) -> Option<u32> {
    let mut data = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY : tampon local de la bonne taille.
    let err = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key,
            value,
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut u32 as *mut c_void),
            Some(&mut size),
        )
    };
    err.is_ok().then_some(data)
}

/// Mode clair / sombre (Paramètres > Personnalisation > Couleurs) et couleur d'accent.
fn read_appearance() -> Appearance {
    let light = read_dword(
        w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
        w!("AppsUseLightTheme"),
    );
    let scheme = match light {
        Some(0) => ColorScheme::Dark,
        _ => ColorScheme::Light,
    };
    // 0xAABBGGRR.
    let accent = read_dword(w!("Software\\Microsoft\\Windows\\DWM"), w!("AccentColor")).map(|v| {
        Color::rgb(
            (v & 0xFF) as f32 / 255.0,
            ((v >> 8) & 0xFF) as f32 / 255.0,
            ((v >> 16) & 0xFF) as f32 / 255.0,
        )
    });
    // Effets d'animation : désactivés = réduire les animations.
    let mut animations = windows::core::BOOL(1);
    // SAFETY : `animations` est un BOOL valide pendant l'appel.
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some(&mut animations as *mut _ as *mut c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    let reduce_motion = ok.is_ok() && !animations.as_bool();
    // Effets de transparence : 0 = désactivés.
    let reduce_transparency = read_dword(
        w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
        w!("EnableTransparency"),
    ) == Some(0);
    Appearance {
        scheme,
        accent,
        reduce_motion,
        reduce_transparency,
    }
}

/// Barre de titre du système sombre ou claire, comme les applications.
///
/// # Safety
///
/// `hwnd` doit être une fenêtre valide.
unsafe fn apply_scheme(hwnd: HWND, scheme: ColorScheme) {
    let dark: i32 = (scheme == ColorScheme::Dark) as i32;
    // SAFETY : garanti par l'appelant ; échoue sans dommage avant Windows 10 20H1.
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark as *const i32 as *const c_void,
            std::mem::size_of::<i32>() as u32,
        );
    }
}

/// Relit l'apparence du système ; prévient l'application et les fenêtres si elle a changé.
fn refresh_appearance() {
    let now = read_appearance();
    if APPEARANCE.with(|a| a.replace(now)) != now {
        BY_HWND.with(|m| {
            for data in m.borrow().values() {
                // SAFETY : les HWND de la table sont vivants (retirés à WM_DESTROY).
                unsafe { apply_scheme(data.hwnd.get(), now.scheme) };
            }
        });
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

impl Platform for WindowsPlatform {
    fn run(self, app: Box<dyn PlatformApp>) -> Result<()> {
        let os = |e: windows::core::Error| PlatformError::Os(e.message());
        // SAFETY : appels Win32 avec des structures locales correctement remplies.
        unsafe {
            // Peut échouer si le manifeste de l'exécutable a déjà choisi : on garde son choix.
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            let hinstance: HINSTANCE = GetModuleHandleW(None).map_err(os)?.into();
            let class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(wndproc),
                hInstance: hinstance,
                hCursor: LoadCursorW(None, IDC_ARROW).map_err(os)?,
                lpszClassName: CLASS,
                ..Default::default()
            };
            if RegisterClassExW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
                return Err(os(windows::core::Error::from_thread()));
            }
            let mut frequency = 0i64;
            QueryPerformanceFrequency(&mut frequency).map_err(os)?;

            let ctx = Ctx {
                hinstance,
                windows: HashMap::new(),
                next_id: 1,
                frequency: frequency.max(1),
                thread: GetCurrentThreadId(),
            };
            APPEARANCE.with(|a| a.set(read_appearance()));
            RUNNER.with(|r| *r.borrow_mut() = Some(Runner { app, ctx }));
            push(PlatformEvent::Resumed);
            pump();

            let mut msg = MSG::default();
            loop {
                let got = GetMessageW(&mut msg, None, 0, 0);
                if got.0 == 0 || got.0 == -1 {
                    break;
                }
                if msg.hwnd.is_invalid() && msg.message == WM_VEKA_WAKE {
                    push(PlatformEvent::Wake);
                } else {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                pump();
            }

            // Fin : on ferme ce qui reste ouvert.
            let runner = RUNNER.with(|r| r.borrow_mut().take());
            if let Some(Runner { app, mut ctx }) = runner {
                // L'application d'abord : ses renderers utilisent encore les fenêtres.
                drop(app);
                let ids: Vec<WindowId> = ctx.windows.keys().copied().collect();
                for id in ids {
                    ctx.close_window(id);
                }
            }
            QUEUE.with(|q| q.borrow_mut().clear());
        }
        Ok(())
    }
}

fn point(lparam: LPARAM, scale: f32) -> Point {
    let x = (lparam.0 & 0xffff) as u16 as i16 as f32;
    let y = ((lparam.0 >> 16) & 0xffff) as u16 as i16 as f32;
    Point::new(x / scale, y / scale)
}

fn button_of(msg: u32) -> Option<(MouseButton, bool)> {
    Some(match msg {
        WM_LBUTTONDOWN => (MouseButton::Primary, true),
        WM_LBUTTONUP => (MouseButton::Primary, false),
        WM_RBUTTONDOWN => (MouseButton::Secondary, true),
        WM_RBUTTONUP => (MouseButton::Secondary, false),
        WM_MBUTTONDOWN => (MouseButton::Middle, true),
        WM_MBUTTONUP => (MouseButton::Middle, false),
        _ => return None,
    })
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY (toute la fonction) : le système appelle avec un HWND valide et des paramètres
    // conformes à la documentation de chaque message.
    unsafe {
        if msg == WM_NCCREATE {
            let create = &*(lparam.0 as *const CREATESTRUCTW);
            let ptr = create.lpCreateParams as *const WinData;
            if !ptr.is_null() {
                Rc::increment_strong_count(ptr);
                let data = Rc::from_raw(ptr);
                data.hwnd.set(hwnd);
                BY_HWND.with(|m| m.borrow_mut().insert(hwnd.0 as isize, data));
            }
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }

        let Some(data) = BY_HWND.with(|m| m.borrow().get(&(hwnd.0 as isize)).cloned()) else {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        };
        let window = data.id;
        let scale = data.scale.get();

        match msg {
            WM_ERASEBKGND => return LRESULT(1),
            WM_SETTINGCHANGE | WM_DWMCOLORIZATIONCOLORCHANGED => {
                refresh_appearance();
                pump();
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            WM_PAINT => {
                let _ = ValidateRect(Some(hwnd), None);
                push(PlatformEvent::RedrawRequested { window });
            }
            WM_SIZE => {
                let width = (lparam.0 & 0xffff) as u32;
                let height = ((lparam.0 >> 16) & 0xffff) as u32;
                data.pixels.set((width, height));
                if wparam.0 as u32 != SIZE_MINIMIZED {
                    let maximized = wparam.0 as u32 == SIZE_MAXIMIZED;
                    set_info(
                        &data,
                        WindowInfo {
                            maximized,
                            ..data.info.get()
                        },
                    );
                }
                let size = Size::new(width as f32 / scale, height as f32 / scale);
                push(PlatformEvent::WindowResized {
                    window,
                    size,
                    width,
                    height,
                });
            }
            WM_DPICHANGED => {
                let dpi = (wparam.0 & 0xffff) as u32;
                let scale = dpi as f32 / BASE_DPI;
                data.scale.set(scale);
                push(PlatformEvent::ScaleChanged { window, scale });
                let r = &*(lparam.0 as *const RECT);
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            WM_GETMINMAXINFO => {
                if let Some(min) = data.min_size {
                    let style = WINDOW_STYLE(GetWindowLongW(hwnd, GWL_STYLE) as u32);
                    let ex_style = WINDOW_EX_STYLE(GetWindowLongW(hwnd, GWL_EXSTYLE) as u32);
                    let dpi = (scale * BASE_DPI).round() as u32;
                    let (w, h) = outer_size(min, style, ex_style, dpi, data.custom);
                    let info = &mut *(lparam.0 as *mut MINMAXINFO);
                    info.ptMinTrackSize.x = w;
                    info.ptMinTrackSize.y = h;
                }
                return LRESULT(0);
            }
            WM_ACTIVATE => {
                let active = (wparam.0 & 0xffff) as u32 != WA_INACTIVE;
                set_info(
                    &data,
                    WindowInfo {
                        active,
                        ..data.info.get()
                    },
                );
                pump();
                // Le système donne le focus clavier à la fenêtre.
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            WM_NCCALCSIZE if data.custom && wparam.0 != 0 => {
                // Pas de cadre : le contenu occupe toute la fenêtre. Agrandie, Windows place la fenêtre
                // un cadre au-delà de l'écran : on le retire pour ne rien perdre.
                if IsZoomed(hwnd).as_bool() {
                    let dpi = GetDpiForWindow(hwnd);
                    let pad = GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
                    let fx = GetSystemMetricsForDpi(SM_CXFRAME, dpi) + pad;
                    let fy = GetSystemMetricsForDpi(SM_CYFRAME, dpi) + pad;
                    let params = &mut *(lparam.0 as *mut NCCALCSIZE_PARAMS);
                    let r = &mut params.rgrc[0];
                    r.left += fx;
                    r.right -= fx;
                    r.top += fy;
                    r.bottom -= fy;
                }
                return LRESULT(0);
            }
            WM_NCHITTEST if data.custom => {
                return LRESULT(hit_test(hwnd, &data, lparam) as isize);
            }
            WM_NCMOUSEMOVE if data.custom => {
                let position = client_point(hwnd, lparam, scale);
                if wparam.0 as u32 == HTMAXBUTTON && !data.nc_tracking.get() {
                    let mut track = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE | TME_NONCLIENT,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    if TrackMouseEvent(&mut track).is_ok() {
                        data.nc_tracking.set(true);
                    }
                }
                push(PlatformEvent::PointerMoved { window, position });
                pump();
                if wparam.0 as u32 == HTMAXBUTTON {
                    return LRESULT(0);
                }
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            WM_NCMOUSELEAVE if data.custom => {
                data.nc_tracking.set(false);
                push(PlatformEvent::PointerLeft { window });
            }
            // Le bouton agrandir est annoncé au système (Snap Layouts) : ses clics arrivent ici.
            WM_NCLBUTTONDOWN | WM_NCLBUTTONUP | WM_NCLBUTTONDBLCLK
                if data.custom && wparam.0 as u32 == HTMAXBUTTON =>
            {
                let position = client_point(hwnd, lparam, scale);
                let pressed = msg != WM_NCLBUTTONUP;
                push(PlatformEvent::PointerButton {
                    window,
                    position,
                    button: MouseButton::Primary,
                    pressed,
                });
            }
            WM_MOUSEMOVE => {
                if !data.tracking.get() {
                    let mut track = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    if TrackMouseEvent(&mut track).is_ok() {
                        data.tracking.set(true);
                    }
                }
                push(PlatformEvent::PointerMoved {
                    window,
                    position: point(lparam, scale),
                });
            }
            WM_MOUSELEAVE => {
                data.tracking.set(false);
                push(PlatformEvent::PointerLeft { window });
            }
            WM_CAPTURECHANGED => data.buttons.set(0),
            WM_CLOSE => push(PlatformEvent::CloseRequested { window }),
            WM_DESTROY => {
                BY_HWND.with(|m| m.borrow_mut().remove(&(hwnd.0 as isize)));
                push(PlatformEvent::WindowClosed { window });
            }
            _ => {
                let Some((button, pressed)) = button_of(msg) else {
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                };
                // On garde la souris pendant un appui, pour recevoir le relâchement hors de la fenêtre.
                if pressed {
                    if data.buttons.get() == 0 {
                        SetCapture(hwnd);
                    }
                    data.buttons.set(data.buttons.get() + 1);
                } else {
                    let left = data.buttons.get().saturating_sub(1);
                    data.buttons.set(left);
                    if left == 0 {
                        let _ = ReleaseCapture();
                    }
                }
                push(PlatformEvent::PointerButton {
                    window,
                    position: point(lparam, scale),
                    button,
                    pressed,
                });
            }
        }
        pump();
        LRESULT(0)
    }
}

/// Met à jour l'état de la fenêtre et prévient l'application s'il a changé.
fn set_info(data: &WinData, info: WindowInfo) {
    if data.info.get() != info {
        data.info.set(info);
        push(PlatformEvent::WindowStateChanged {
            window: data.id,
            info,
        });
    }
}

/// Position du pointeur (coordonnées d'écran dans `lparam`) en points dans la fenêtre.
///
/// # Safety
///
/// `hwnd` doit être une fenêtre valide.
unsafe fn client_point(hwnd: HWND, lparam: LPARAM, scale: f32) -> Point {
    let mut p = POINT {
        x: (lparam.0 & 0xffff) as u16 as i16 as i32,
        y: ((lparam.0 >> 16) & 0xffff) as u16 as i16 as i32,
    };
    // SAFETY : HWND valide (contrat), pointeur local.
    unsafe {
        let _ = ScreenToClient(hwnd, &mut p);
    }
    Point::new(p.x as f32 / scale, p.y as f32 / scale)
}

/// Ce que le système doit faire au point donné : bords, déplacement, bouton agrandir ou contenu.
///
/// # Safety
///
/// `hwnd` doit être une fenêtre valide.
unsafe fn hit_test(hwnd: HWND, data: &WinData, lparam: LPARAM) -> u32 {
    let scale = data.scale.get();
    // SAFETY : contrat de la fonction.
    let p = unsafe { client_point(hwnd, lparam, scale) };
    let (w, h) = data.pixels.get();
    let (w, h) = (w as f32 / scale, h as f32 / scale);
    // SAFETY : HWND valide.
    let zoomed = unsafe { IsZoomed(hwnd).as_bool() };
    if data.resizable && !zoomed {
        let m = data.resize_margin;
        let (left, right) = (p.x < m, p.x >= w - m);
        let (top, bottom) = (p.y < m, p.y >= h - m);
        let ht = match (top, bottom, left, right) {
            (true, _, true, _) => HTTOPLEFT,
            (true, _, _, true) => HTTOPRIGHT,
            (_, true, true, _) => HTBOTTOMLEFT,
            (_, true, _, true) => HTBOTTOMRIGHT,
            (true, ..) => HTTOP,
            (_, true, ..) => HTBOTTOM,
            (_, _, true, _) => HTLEFT,
            (_, _, _, true) => HTRIGHT,
            _ => HTNOWHERE,
        };
        if ht != HTNOWHERE {
            return ht;
        }
    }
    let regions = data.regions.borrow();
    for region in regions.iter().rev() {
        if region.rect.contains(p) {
            return match region.kind {
                RegionKind::Drag => HTCAPTION,
                RegionKind::Control(WindowControl::Maximize) if data.resizable => HTMAXBUTTON,
                _ => HTCLIENT,
            };
        }
    }
    HTCLIENT
}
