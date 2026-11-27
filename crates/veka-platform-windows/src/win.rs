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
    Chrome, Instant, MouseButton, Platform, PlatformApp, PlatformContext, PlatformError, PlatformEvent,
    PlatformWindow, Point, Result, Size, WindowDesc, WindowId, WindowRegion,
};
use windows::Win32::Foundation::{
    ERROR_CLASS_ALREADY_EXISTS, GetLastError, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM,
};
use windows::Win32::Graphics::Gdi::{InvalidateRect, ValidateRect};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::UI::HiDpi::{
    AdjustWindowRectExForDpi, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForSystem, GetDpiForWindow,
    SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow,
    DispatchMessageW, GWL_EXSTYLE, GWL_STYLE, GetClientRect, GetMessageW, GetWindowLongW, IDC_ARROW,
    LoadCursorW, MINMAXINFO, MSG, PostQuitMessage, RegisterClassExW, SW_SHOW, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOZORDER, SetWindowPos, SetWindowTextW, ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_CAPTURECHANGED, WM_CLOSE, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND, WM_GETMINMAXINFO, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE, WM_NCCREATE, WM_PAINT, WM_RBUTTONDOWN,
    WM_RBUTTONUP, WM_SIZE, WNDCLASSEXW, WS_MAXIMIZEBOX, WS_OVERLAPPEDWINDOW, WS_THICKFRAME,
};
use windows::core::{HSTRING, PCWSTR, w};

const CLASS: PCWSTR = w!("VekaWindow");
const BASE_DPI: f32 = 96.0;
/// `WM_MOUSELEAVE` (défini dans `Win32_UI_Controls`, une grosse feature pour une constante).
const WM_MOUSELEAVE: u32 = 0x02A3;

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
}

impl PlatformContext for Ctx {
    fn create_window(&mut self, desc: WindowDesc) -> Result<WindowId> {
        if desc.chrome != Chrome::System {
            return Err(PlatformError::Unsupported(
                "Chrome::Hidden et Chrome::Custom arrivent juste après l'étape 1".into(),
            ));
        }
        let id = WindowId(self.next_id);
        self.next_id += 1;
        let style = if desc.resizable {
            WS_OVERLAPPEDWINDOW
        } else {
            WS_OVERLAPPEDWINDOW & !(WS_THICKFRAME | WS_MAXIMIZEBOX)
        };
        let ex_style = WINDOW_EX_STYLE(0);
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
        });
        let (w, h) = outer_size(desc.size, style, ex_style, dpi);
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
                let (w, h) = outer_size(desc.size, style, ex_style, real);
                let _ = SetWindowPos(hwnd, None, 0, 0, w, h, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
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
}

/// Taille extérieure (bordures comprises) pour un contenu de `size` points.
fn outer_size(size: Size, style: WINDOW_STYLE, ex_style: WINDOW_EX_STYLE, dpi: u32) -> (i32, i32) {
    let scale = dpi as f32 / BASE_DPI;
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: (size.width * scale).round() as i32,
        bottom: (size.height * scale).round() as i32,
    };
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
            };
            RUNNER.with(|r| *r.borrow_mut() = Some(Runner { app, ctx }));
            push(PlatformEvent::Resumed);
            pump();

            let mut msg = MSG::default();
            loop {
                let got = GetMessageW(&mut msg, None, 0, 0);
                if got.0 == 0 || got.0 == -1 {
                    break;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
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
            WM_PAINT => {
                let _ = ValidateRect(Some(hwnd), None);
                push(PlatformEvent::RedrawRequested { window });
            }
            WM_SIZE => {
                let width = (lparam.0 & 0xffff) as u32;
                let height = ((lparam.0 >> 16) & 0xffff) as u32;
                data.pixels.set((width, height));
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
                    let (w, h) = outer_size(min, style, ex_style, (scale * BASE_DPI).round() as u32);
                    let info = &mut *(lparam.0 as *mut MINMAXINFO);
                    info.ptMinTrackSize.x = w;
                    info.ptMinTrackSize.y = h;
                }
                return LRESULT(0);
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
