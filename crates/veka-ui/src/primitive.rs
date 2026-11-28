//! L'API d'extension : tout composant de Veka (piles, texte, bouton…) est un [`Primitive`], et un
//! utilisateur peut en écrire de nouveaux exactement de la même façon.

use std::any::Any;

use veka_foundation::{Point, Rect, Size};
use veka_graphics::{DrawList, Shape};
use veka_text::TextSystem;

use crate::element::Action;
use crate::env::Environment;
use crate::tree::{NodeId, Tree};

/// Taille proposée par un parent. `None` sur un axe = « prends ta taille idéale ».
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Proposal {
    pub width: Option<f32>,
    pub height: Option<f32>,
}

impl Proposal {
    pub const IDEAL: Proposal = Proposal {
        width: None,
        height: None,
    };

    pub const fn new(width: Option<f32>, height: Option<f32>) -> Self {
        Self { width, height }
    }

    pub fn exact(size: Size) -> Self {
        Self::new(Some(size.width), Some(size.height))
    }
}

/// Un bloc de base de l'interface : il se mesure, place ses enfants, se dessine et réagit au pointeur.
///
/// Toutes les méthodes ont un comportement par défaut « transparent » (prend la taille de ses enfants,
/// les place sur son propre rectangle, les dessine).
pub trait Primitive: Any {
    /// Nom dans le schéma (`"VStack"`, `"Text"`, `"padding"`…).
    fn name(&self) -> &'static str;

    /// Modifie l'environnement transmis aux enfants (police, couleur…). Renvoie `true` si modifié.
    fn update_env(&self, _env: &mut Environment) -> bool {
        false
    }

    /// Taille voulue pour cette proposition.
    fn size_that_fits(&self, cx: &mut LayoutCx<'_>, proposal: Proposal) -> Size {
        let mut size = Size::ZERO;
        for child in cx.children() {
            let s = cx.measure(child, proposal);
            size = Size::new(size.width.max(s.width), size.height.max(s.height));
        }
        size
    }

    /// Place les enfants dans `rect` (coordonnées de la fenêtre).
    fn place(&self, cx: &mut LayoutCx<'_>, rect: Rect) {
        for child in cx.children() {
            cx.place(child, rect);
        }
    }

    /// Dessine (par défaut : seulement les enfants).
    fn paint(&self, cx: &mut PaintCx<'_>, _rect: Rect) {
        cx.paint_children();
    }

    /// Forme qui reçoit le pointeur, ou `None` si ce primitive n'en reçoit pas.
    fn hit_shape(&self) -> Option<Shape> {
        None
    }

    /// Réagit au pointeur (seulement si [`hit_shape`](Self::hit_shape) n'est pas `None`).
    fn pointer(&self, _cx: &mut EventCx<'_>, _event: PointerEvent) -> EventResult {
        EventResult::default()
    }

    /// Si `false`, les enfants ne reçoivent jamais le pointeur : ce primitive le prend pour eux
    /// (un bouton de fenêtre qui contient une icône, par exemple).
    fn hit_children(&self) -> bool {
        true
    }

    /// Ce que ce primitive représente pour la fenêtre (zone de déplacement, bouton de fenêtre).
    fn window_region(&self) -> Option<veka_foundation::RegionKind> {
        None
    }

    /// Le mode clair ou sombre demandé pour toute la fenêtre (`.preferred_color_scheme`).
    fn preferred_color_scheme(&self) -> Option<veka_foundation::ColorScheme> {
        None
    }

    /// État propre au nœud (survol, cache de texte…), créé une fois à l'insertion du nœud.
    fn create_state(&self) -> Box<dyn Any> {
        Box::new(())
    }

    /// Les valeurs animables (Anima) : nombres et couleurs, comme `animatableData` de SwiftUI. Quand
    /// elles changent dans une animation, Veka les interpole et dessine
    /// [`with_animatable`](Self::with_animatable). `None` : tout change d'un coup.
    ///
    /// Seules les valeurs qui ne changent pas la taille ont leur place ici : le layout, lui, s'anime
    /// de lui-même (les rectangles des nœuds).
    fn animatable(&self, _env: &Environment) -> Option<veka_anima::AnimatableData> {
        None
    }

    /// Le même primitive, avec des valeurs intermédiaires (lues dans l'ordre d'écriture).
    fn with_animatable(
        &self,
        _env: &Environment,
        _data: &veka_anima::AnimatableData,
    ) -> Option<std::rc::Rc<dyn Primitive>> {
        None
    }
}

/// Contexte de mesure et de placement.
pub struct LayoutCx<'a> {
    pub(crate) tree: &'a mut Tree,
    pub(crate) text: &'a mut TextSystem,
    pub(crate) node: NodeId,
}

impl LayoutCx<'_> {
    /// Les enfants à mesurer et placer (sans ceux qui sont en train de sortir). Sur la pile jusqu'à 4 :
    /// mesurer et placer ne doit rien allouer pour les modifiers (un seul enfant).
    pub fn children(&self) -> smallvec::SmallVec<[NodeId; 4]> {
        self.tree.live_children_iter(self.node).collect()
    }

    pub fn env(&self) -> &Environment {
        &self.tree.nodes[self.node].env
    }

    /// Nom du primitive d'un enfant (sert par exemple à reconnaître les `Spacer`).
    pub fn child_name(&self, child: NodeId) -> &'static str {
        self.tree.nodes[child].prim.name()
    }

    pub fn measure(&mut self, child: NodeId, proposal: Proposal) -> Size {
        crate::tree::measure(self.tree, self.text, child, proposal)
    }

    pub fn place(&mut self, child: NodeId, rect: Rect) {
        crate::tree::place(self.tree, self.text, child, rect);
    }

    /// L'état du nœud et le système de texte, ensemble.
    pub fn state_and_text<S: 'static, R>(
        &mut self,
        f: impl FnOnce(&mut S, &mut TextSystem, &Environment) -> R,
    ) -> R {
        let node = &mut self.tree.nodes[self.node];
        let state = node.state.downcast_mut::<S>().expect("type d'état du primitive");
        f(state, self.text, &node.env)
    }
}

/// Contexte de dessin.
pub struct PaintCx<'a> {
    pub(crate) tree: &'a Tree,
    pub(crate) list: &'a mut DrawList,
    pub(crate) node: NodeId,
}

impl PaintCx<'_> {
    pub fn list(&mut self) -> &mut DrawList {
        self.list
    }

    pub fn env(&self) -> &Environment {
        &self.tree.nodes[self.node].env
    }

    pub fn state<S: 'static>(&self) -> &S {
        self.tree.nodes[self.node]
            .state
            .downcast_ref::<S>()
            .expect("type d'état du primitive")
    }

    /// Dessine les enfants. Ceux qui entrent ou sortent (transition en cours) passent au-dessus de
    /// leurs frères : une ligne qui glisse ne disparaît pas sous ses voisines.
    pub fn paint_children(&mut self) {
        let children = &self.tree.nodes[self.node].children;
        let moving = |c: &NodeId| self.tree.anims.get(c).is_some_and(|a| a.in_transition());
        for child in children.iter().filter(|c| !moving(c)) {
            crate::tree::paint(self.tree, self.list, *child);
        }
        for child in children.iter().filter(|c| moving(c)) {
            crate::tree::paint(self.tree, self.list, *child);
        }
    }
}

/// Événement de pointeur reçu par un primitive.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointerEvent {
    /// Le pointeur bouge ; `inside` : il est dans la forme.
    Move {
        position: Point,
        inside: bool,
    },
    Down {
        position: Point,
    },
    /// Relâché ; `inside` : dans la forme du primitive qui a reçu l'appui.
    Up {
        position: Point,
        inside: bool,
    },
    /// Le pointeur a quitté la forme (ou la fenêtre).
    Leave,
}

/// Contexte d'un événement.
pub struct EventCx<'a> {
    pub(crate) state: &'a mut dyn Any,
    pub(crate) rect: Rect,
}

impl EventCx<'_> {
    pub fn state<S: 'static>(&mut self) -> &mut S {
        self.state.downcast_mut::<S>().expect("type d'état du primitive")
    }

    pub fn rect(&self) -> Rect {
        self.rect
    }
}

/// Résultat d'un événement.
#[derive(Default)]
pub struct EventResult {
    /// L'événement a été pris (l'appui capture alors le pointeur).
    pub handled: bool,
    /// Le primitive doit être redessiné (survol, appui…).
    pub repaint: bool,
    /// Action à lancer une fois l'événement traité.
    pub action: Option<Action>,
}
