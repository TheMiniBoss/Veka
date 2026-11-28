//! # Veka Platform Windows
//!
//! La plateforme Windows de Veka, écrite directement sur Win32 (crate `windows` de Microsoft) :
//!
//! - une classe de fenêtre et une `WndProc` qui traduit les messages en [`PlatformEvent`](veka_platform::PlatformEvent) ;
//! - une boucle `GetMessageW`, qui dort au repos (0 % CPU) ;
//! - DPI Per-Monitor V2 : l'échelle suit l'écran de la fenêtre (`WM_DPICHANGED`) ;
//! - souris avec `TrackMouseEvent` pour savoir quand le pointeur sort ;
//! - redessin à la demande : `InvalidateRect` puis `WM_PAINT` ;
//! - horloge `QueryPerformanceCounter`.
//!
//! Sur les autres systèmes, la crate est vide.

#[cfg(windows)]
mod text;
#[cfg(windows)]
mod win;

#[cfg(windows)]
pub use text::{clipboard_text, set_clipboard_text};
#[cfg(windows)]
pub use win::{Win32Window, WindowsPlatform};
