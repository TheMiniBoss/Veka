//! # Veka Platform
//!
//! Les traits de la couche Platform : fenêtres, entrées, horloge. Aucune implémentation ici, et
//! aucun appel au système : chaque OS a sa crate (`veka-platform-windows`, `veka-platform-macos`).
//!
//! Le principe : la plateforme possède la boucle d'événements ([`Platform::run`]) et appelle
//! l'application ([`PlatformApp::event`]) avec un contexte ([`PlatformContext`]) qui permet de
//! créer des fenêtres, lire l'heure et quitter. Toutes les tailles et positions sont en **points
//! logiques** (indépendants de l'écran) ; l'échelle (`scale`) donne le nombre de pixels par point.

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
pub use veka_foundation::{
    Color, ColorScheme, Instant, Point, Rect, RegionKind, Size, WindowAction, WindowControl, WindowInfo,
    WindowRegion,
};

/// L'apparence choisie dans les réglages du système.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Appearance {
    /// Mode clair ou sombre des applications.
    pub scheme: ColorScheme,
    /// Couleur d'accent, si le système en a une.
    pub accent: Option<Color>,
    /// « Réduire les animations » (macOS : Accessibilité > Affichage ; Windows : Accessibilité >
    /// Effets visuels > Effets d'animation désactivés).
    pub reduce_motion: bool,
}

/// Erreur d'une plateforme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformError {
    /// Cette plateforme ne sait pas encore faire ça (par exemple un `Chrome` pas encore construit).
    Unsupported(String),
    /// Échec d'un appel au système, avec son message.
    Os(String),
}

impl std::fmt::Display for PlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlatformError::Unsupported(m) => write!(f, "non pris en charge : {m}"),
            PlatformError::Os(m) => write!(f, "erreur du système : {m}"),
        }
    }
}

impl std::error::Error for PlatformError {}

pub type Result<T> = std::result::Result<T, PlatformError>;

/// Identifiant d'une fenêtre, unique pendant toute la vie de l'application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId(pub u64);

/// Décoration d'une fenêtre (voir `Docs/VEKA_ETAPE1.md` §8.4).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Chrome {
    /// Barre de titre et boutons du système.
    #[default]
    System,
    /// Barre masquée : macOS garde les « feux », Windows garde la bordure.
    Hidden,
    /// Aucune décoration du système : Veka dessine tout.
    Custom {
        /// Fond de fenêtre transparent.
        transparent: bool,
        /// Ombre portée demandée au système.
        shadow: bool,
        /// Rayon des coins demandé au système (en points).
        corner_radius: f32,
    },
}

impl Chrome {
    /// `Custom` opaque, avec ombre et coins arrondis.
    pub const fn custom() -> Self {
        Chrome::Custom {
            transparent: false,
            shadow: true,
            corner_radius: 8.0,
        }
    }

    /// `Custom` transparent : le bureau se voit là où Veka ne dessine rien.
    pub const fn transparent() -> Self {
        Chrome::Custom {
            transparent: true,
            shadow: false,
            corner_radius: 0.0,
        }
    }

    /// La fenêtre laisse voir ce qui est derrière elle.
    pub const fn is_transparent(&self) -> bool {
        matches!(
            self,
            Chrome::Custom {
                transparent: true,
                ..
            }
        )
    }
}

/// Ce qu'il faut pour créer une fenêtre.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowDesc {
    pub title: String,
    /// Taille du contenu, en points.
    pub size: Size,
    /// Taille minimale du contenu, en points.
    pub min_size: Option<Size>,
    pub resizable: bool,
    pub chrome: Chrome,
    /// Largeur des bords de redimensionnement, en points (fenêtres sans décoration du système).
    pub resize_margin: f32,
}

impl WindowDesc {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            size: Size::new(480.0, 320.0),
            min_size: None,
            resizable: true,
            chrome: Chrome::System,
            resize_margin: 6.0,
        }
    }

    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    pub fn min_size(mut self, size: Size) -> Self {
        self.min_size = Some(size);
        self
    }

    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    pub fn chrome(mut self, chrome: Chrome) -> Self {
        self.chrome = chrome;
        self
    }

    pub fn resize_margin(mut self, margin: f32) -> Self {
        self.resize_margin = margin;
        self
    }
}

/// Bouton de la souris.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Primary,
    Secondary,
    Middle,
    Other(u16),
}

/// Événement envoyé par la plateforme à l'application.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlatformEvent {
    /// La boucle démarre : c'est le moment de créer les fenêtres.
    Resumed,
    /// Nouvelle taille du contenu. `size` en points ; `width` et `height` en pixels.
    WindowResized {
        window: WindowId,
        size: Size,
        width: u32,
        height: u32,
    },
    /// Nouvelle échelle (la fenêtre a changé d'écran, ou l'utilisateur a changé le zoom).
    ScaleChanged {
        window: WindowId,
        scale: f32,
    },
    /// L'utilisateur veut fermer la fenêtre. Rien ne se passe tant que l'app ne la ferme pas.
    CloseRequested {
        window: WindowId,
    },
    /// Agrandie, plein écran, active : voir [`WindowInfo`].
    WindowStateChanged {
        window: WindowId,
        info: WindowInfo,
    },
    /// La fenêtre a été détruite.
    WindowClosed {
        window: WindowId,
    },
    /// Il faut dessiner la fenêtre.
    RedrawRequested {
        window: WindowId,
    },
    PointerMoved {
        window: WindowId,
        position: Point,
    },
    PointerButton {
        window: WindowId,
        position: Point,
        button: MouseButton,
        pressed: bool,
    },
    /// Le pointeur a quitté la fenêtre.
    PointerLeft {
        window: WindowId,
    },
    /// L'utilisateur a changé le mode clair / sombre ou la couleur d'accent du système.
    AppearanceChanged {
        appearance: Appearance,
    },
    /// Un autre fil a appelé le [`Waker`] de la boucle (fichier relu, travail fini…).
    Wake,
}

/// Réveille la boucle depuis n'importe quel fil : elle reçoit un [`PlatformEvent::Wake`].
/// Plusieurs appels rapprochés peuvent ne donner qu'un événement.
pub type Waker = std::sync::Arc<dyn Fn() + Send + Sync>;

/// Une fenêtre ouverte. Ses poignées `raw-window-handle` servent au renderer.
pub trait PlatformWindow: HasWindowHandle + HasDisplayHandle {
    fn id(&self) -> WindowId;
    /// Taille du contenu, en points.
    fn size(&self) -> Size;
    /// Taille du contenu, en pixels.
    fn pixel_size(&self) -> (u32, u32);
    /// Pixels par point (1.0 à 100 %, 1.5 à 150 %, 2.0 sur un écran Retina).
    fn scale(&self) -> f32;
    fn set_title(&self, title: &str);
    /// Demande un `RedrawRequested` (regroupé avec les autres demandes).
    fn request_redraw(&self);
    /// Une fonction qui demande un redessin, à garder ailleurs (par exemple dans une `Surface`).
    /// Elle doit être appelée sur le fil de la boucle.
    fn redraw_requester(&self) -> Box<dyn Fn() + 'static>;
    /// Les zones de la fenêtre pour le système (déplacement, boutons). Voir [`WindowRegion`].
    fn set_regions(&self, regions: &[WindowRegion]);
    /// L'état actuel (agrandie, plein écran, active).
    fn info(&self) -> WindowInfo;
    /// Agit sur la fenêtre. Une action que la plateforme ne sait pas encore faire est ignorée.
    fn perform(&self, action: WindowAction);
}

/// Ce que l'application peut demander à la plateforme pendant un événement.
pub trait PlatformContext {
    fn create_window(&mut self, desc: WindowDesc) -> Result<WindowId>;
    /// La fenêtre `id`. Panique si elle n'existe pas (ou plus).
    fn window(&self, id: WindowId) -> &dyn PlatformWindow;
    fn close_window(&mut self, id: WindowId);
    /// Horloge monotone de la plateforme.
    fn now(&self) -> Instant;
    /// Quitte la boucle après l'événement en cours.
    fn exit(&mut self);
    /// Mode clair / sombre et couleur d'accent du système.
    fn appearance(&self) -> Appearance {
        Appearance::default()
    }
    /// De quoi réveiller la boucle depuis un autre fil, si la plateforme le sait.
    fn waker(&self) -> Option<Waker> {
        None
    }
}

/// L'application, vue par la plateforme.
pub trait PlatformApp {
    fn event(&mut self, ctx: &mut dyn PlatformContext, event: PlatformEvent);
}

/// Une plateforme : elle possède la boucle d'événements.
pub trait Platform {
    /// Lance la boucle et ne rend la main qu'à la fin de l'application.
    fn run(self, app: Box<dyn PlatformApp>) -> Result<()>;
}
