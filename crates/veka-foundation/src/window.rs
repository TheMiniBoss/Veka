//! Les notions de fenêtre partagées par l'interface et les plateformes (données seulement).

use crate::Rect;

/// Un bouton de fenêtre qu'une vue Veka peut jouer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowControl {
    Close,
    Minimize,
    Maximize,
    Fullscreen,
}

impl WindowControl {
    /// L'action déclenchée par ce bouton.
    pub fn action(self) -> WindowAction {
        match self {
            WindowControl::Close => WindowAction::Close,
            WindowControl::Minimize => WindowAction::Minimize,
            WindowControl::Maximize => WindowAction::ToggleMaximize,
            WindowControl::Fullscreen => WindowAction::ToggleFullscreen,
        }
    }

    /// Nom dans le schéma (`"close"`, `"minimize"`…).
    pub fn name(self) -> &'static str {
        match self {
            WindowControl::Close => "close",
            WindowControl::Minimize => "minimize",
            WindowControl::Maximize => "maximize",
            WindowControl::Fullscreen => "fullscreen",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "close" => WindowControl::Close,
            "minimize" => WindowControl::Minimize,
            "maximize" => WindowControl::Maximize,
            "fullscreen" => WindowControl::Fullscreen,
            _ => return None,
        })
    }
}

/// Une action sur la fenêtre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowAction {
    Minimize,
    /// Agrandit, ou restaure si la fenêtre est déjà agrandie.
    ToggleMaximize,
    /// Demande la fermeture (comme le bouton du système : l'application peut refuser).
    Close,
    ToggleFullscreen,
}

/// Ce que représente une zone de la fenêtre pour le système.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegionKind {
    /// La zone déplace la fenêtre (double-clic = agrandir).
    Drag,
    /// La zone est un bouton de fenêtre (`Maximize` garde les Snap Layouts de Windows 11).
    Control(WindowControl),
    /// Une vue interactive posée sur une zone de déplacement : elle garde le pointeur.
    Client,
}

/// Une zone de la fenêtre, en points. Les dernières de la liste sont au-dessus.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowRegion {
    pub rect: Rect,
    pub kind: RegionKind,
}

/// L'état d'une fenêtre, tel que les vues peuvent le lire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowInfo {
    pub maximized: bool,
    pub fullscreen: bool,
    /// La fenêtre est au premier plan (elle reçoit le clavier).
    pub active: bool,
}

impl Default for WindowInfo {
    fn default() -> Self {
        Self {
            maximized: false,
            fullscreen: false,
            active: true,
        }
    }
}
