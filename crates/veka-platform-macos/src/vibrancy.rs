//! Matériau du système derrière le contenu (`SystemBackdrop`) : une `NSVisualEffectView` sous la vue de
//! Veka, qui se dessine par-dessus sur un fond transparent.

use objc2::MainThreadOnly;
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSColor, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
    NSVisualEffectState, NSVisualEffectView, NSWindow,
};
use veka_platform::SystemBackdrop;

/// Pose `view` comme contenu de `window`, au-dessus du matériau demandé s'il y en a un.
pub(crate) fn set_content(window: &NSWindow, view: &NSView, backdrop: SystemBackdrop) {
    let material = match backdrop {
        SystemBackdrop::Mica => NSVisualEffectMaterial::UnderWindowBackground,
        SystemBackdrop::MicaAlt => NSVisualEffectMaterial::Sidebar,
        SystemBackdrop::Acrylic => NSVisualEffectMaterial::HUDWindow,
        _ => {
            window.setContentView(Some(view));
            return;
        }
    };
    let effect = NSVisualEffectView::initWithFrame(NSVisualEffectView::alloc(view.mtm()), view.frame());
    effect.setMaterial(material);
    effect.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    // Le matériau garde son aspect quand la fenêtre n'est pas au premier plan.
    effect.setState(NSVisualEffectState::Active);
    view.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    effect.addSubview(view);
    window.setOpaque(false);
    window.setBackgroundColor(Some(&NSColor::clearColor()));
    window.setContentView(Some(&effect));
    // La vue de Veka reçoit le clavier.
    window.makeFirstResponder(Some(view));
}
