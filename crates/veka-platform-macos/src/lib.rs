//! # Veka Platform macOS
//!
//! La plateforme macOS de Veka, écrite directement sur AppKit (crates `objc2`) :
//!
//! - `NSApplication` et son délégué, qui envoie `Resumed` au lancement ;
//! - une `NSWindow` par fenêtre, avec une `NSView` à nous (repère retourné : origine en haut à gauche) ;
//! - souris avec une `NSTrackingArea` (survol et sortie) ;
//! - échelle Retina par `backingScaleFactor`, suivie par `viewDidChangeBackingProperties` ;
//! - redessin à la demande : `setNeedsDisplay` puis `drawRect:`.
//!
//! wgpu crée sa couche `CAMetalLayer` à partir de la vue. Sur les autres systèmes, la crate est vide.

#[cfg(target_os = "macos")]
mod mac;

#[cfg(target_os = "macos")]
pub use mac::{MacPlatform, MacWindow};
