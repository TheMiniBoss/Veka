//! Le texte sur Windows (étape 9) : le presse-papiers (`CF_UNICODETEXT`) et l'IME (IMM32).

use veka_platform::{Ime, Rect};
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND, POINT, RECT};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::UI::Input::Ime::{
    CANDIDATEFORM, CFS_EXCLUDE, CFS_POINT, COMPOSITIONFORM, GCS_COMPSTR, GCS_CURSORPOS, GCS_RESULTSTR, HIMC,
    IACE_DEFAULT, IME_COMPOSITION_STRING, ImmAssociateContextEx, ImmGetCompositionStringW, ImmGetContext,
    ImmReleaseContext, ImmSetCandidateWindow, ImmSetCompositionWindow,
};

/// `CF_UNICODETEXT` (dans `Win32_System_Ole`, qu'on n'importe pas pour une constante).
const CF_UNICODETEXT: u32 = 13;

/// Ouvre le presse-papiers, le temps de `f`. Une fenêtre doit le posséder pour pouvoir y écrire.
fn with_clipboard<T>(f: impl FnOnce() -> Option<T>) -> Option<T> {
    let owner = super::win::any_hwnd();
    // SAFETY : `owner` est une fenêtre de ce fil, ou aucune ; le presse-papiers est refermé ensuite.
    unsafe {
        // Une autre app peut le tenir un instant : on réessaie un peu.
        let mut open = false;
        for _ in 0..5 {
            if OpenClipboard(owner).is_ok() {
                open = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        if !open {
            return None;
        }
        let r = f();
        let _ = CloseClipboard();
        r
    }
}

/// Le texte du presse-papiers du système.
pub fn clipboard_text() -> Option<String> {
    with_clipboard(|| {
        // SAFETY : presse-papiers ouvert ; la mémoire est verrouillée pendant la lecture et le texte
        // s'arrête au premier zéro, dans la taille du bloc.
        unsafe {
            IsClipboardFormatAvailable(CF_UNICODETEXT).ok()?;
            let handle = GetClipboardData(CF_UNICODETEXT).ok()?;
            let mem = HGLOBAL(handle.0);
            let size = windows::Win32::System::Memory::GlobalSize(mem) / 2;
            let ptr = GlobalLock(mem) as *const u16;
            if ptr.is_null() {
                return None;
            }
            let units = std::slice::from_raw_parts(ptr, size);
            let len = units.iter().position(|&u| u == 0).unwrap_or(units.len());
            let text = String::from_utf16_lossy(&units[..len]);
            let _ = GlobalUnlock(mem);
            Some(text)
        }
    })
}

/// Met du texte dans le presse-papiers du système. Les fins de ligne deviennent `\r\n`.
pub fn set_clipboard_text(text: &str) {
    let units: Vec<u16> = text
        .replace("\r\n", "\n")
        .replace('\n', "\r\n")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    with_clipboard(|| {
        // SAFETY : presse-papiers ouvert ; le bloc alloué a la taille du texte, et il appartient au
        // système après `SetClipboardData` (on ne le libère qu'en cas d'échec).
        unsafe {
            EmptyClipboard().ok()?;
            let mem = GlobalAlloc(GMEM_MOVEABLE, units.len() * 2).ok()?;
            let ptr = GlobalLock(mem) as *mut u16;
            if ptr.is_null() {
                let _ = GlobalFree(Some(mem));
                return None;
            }
            std::ptr::copy_nonoverlapping(units.as_ptr(), ptr, units.len());
            let _ = GlobalUnlock(mem);
            if SetClipboardData(CF_UNICODETEXT, Some(HANDLE(mem.0))).is_err() {
                let _ = GlobalFree(Some(mem));
            }
            Some(())
        }
    });
}

/// Branche l'IME (`Some`) ou le coupe (`None`) pour la fenêtre, et place sa fenêtre de candidats sous la
/// zone (en points).
///
/// # Safety
/// `hwnd` doit être une fenêtre valide de ce fil.
pub unsafe fn set_ime_area(hwnd: HWND, area: Option<Rect>, scale: f32) {
    // SAFETY : voir la fonction.
    unsafe {
        let Some(area) = area else {
            // Pas de champ : les touches arrivent telles quelles, sans composition.
            let _ = ImmAssociateContextEx(hwnd, HIMC::default(), 0);
            return;
        };
        let _ = ImmAssociateContextEx(hwnd, HIMC::default(), IACE_DEFAULT);
        place_ime(hwnd, area, scale);
    }
}

/// Place les fenêtres de l'IME sous la zone.
///
/// # Safety
/// `hwnd` doit être une fenêtre valide de ce fil.
pub unsafe fn place_ime(hwnd: HWND, area: Rect, scale: f32) {
    let rect = RECT {
        left: (area.x() * scale).floor() as i32,
        top: (area.y() * scale).floor() as i32,
        right: (area.max_x() * scale).ceil() as i32,
        bottom: (area.max_y() * scale).ceil() as i32,
    };
    // SAFETY : voir la fonction ; le contexte est rendu après usage.
    unsafe {
        let himc = ImmGetContext(hwnd);
        if himc.is_invalid() {
            return;
        }
        let comp = COMPOSITIONFORM {
            dwStyle: CFS_POINT,
            ptCurrentPos: POINT {
                x: rect.left,
                y: rect.top,
            },
            rcArea: rect,
        };
        let _ = ImmSetCompositionWindow(himc, &comp);
        let cand = CANDIDATEFORM {
            dwIndex: 0,
            dwStyle: CFS_EXCLUDE,
            ptCurrentPos: POINT {
                x: rect.left,
                y: rect.bottom,
            },
            rcArea: rect,
        };
        let _ = ImmSetCandidateWindow(himc, &cand);
        let _ = ImmReleaseContext(hwnd, himc);
    }
}

/// Lit `WM_IME_COMPOSITION` : le texte choisi d'abord, puis le texte en cours.
///
/// # Safety
/// `hwnd` doit être une fenêtre valide de ce fil.
pub unsafe fn composition(hwnd: HWND, flags: u32) -> Vec<Ime> {
    let mut out = Vec::new();
    // SAFETY : voir la fonction ; les tampons ont la taille annoncée par IMM32.
    unsafe {
        let himc = ImmGetContext(hwnd);
        if himc.is_invalid() {
            return out;
        }
        if flags & GCS_RESULTSTR.0 != 0
            && let Some(text) = read(himc, GCS_RESULTSTR)
        {
            out.push(Ime::Commit(String::from_utf16_lossy(&text)));
        }
        if flags & GCS_COMPSTR.0 != 0 {
            let text = read(himc, GCS_COMPSTR).unwrap_or_default();
            let cursor = if flags & GCS_CURSORPOS.0 != 0 {
                let pos = ImmGetCompositionStringW(himc, GCS_CURSORPOS, None, 0);
                (pos >= 0).then(|| {
                    let b = byte_offset(&text, pos as usize);
                    (b, b)
                })
            } else {
                None
            };
            out.push(Ime::Preedit {
                text: String::from_utf16_lossy(&text),
                cursor,
            });
        }
        let _ = ImmReleaseContext(hwnd, himc);
    }
    out
}

/// Une chaîne de la composition, en UTF-16.
unsafe fn read(himc: HIMC, kind: IME_COMPOSITION_STRING) -> Option<Vec<u16>> {
    // SAFETY : la taille est demandée d'abord, puis le tampon a exactement cette taille.
    unsafe {
        let bytes = ImmGetCompositionStringW(himc, kind, None, 0);
        if bytes < 0 {
            return None;
        }
        let mut buf = vec![0u16; bytes as usize / 2];
        if !buf.is_empty() {
            ImmGetCompositionStringW(himc, kind, Some(buf.as_mut_ptr().cast()), bytes as u32);
        }
        Some(buf)
    }
}

/// La position en octets UTF-8 qui correspond à `units` unités UTF-16 du texte.
fn byte_offset(text: &[u16], units: usize) -> usize {
    char::decode_utf16(text[..units.min(text.len())].iter().copied())
        .map(|c| c.map_or(3, char::len_utf8))
        .sum()
}
