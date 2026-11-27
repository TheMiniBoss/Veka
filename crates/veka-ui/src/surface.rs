//! `Surface` : une interface Veka sans fenêtre. L'hôte (app native, moteur de jeu…) lui donne une
//! taille et des événements, et lui demande une liste de dessin quand elle en a besoin.

use std::rc::Rc;

use veka_foundation::{Point, Rect, RegionKind, Size, WindowAction, WindowInfo, WindowRegion};
use veka_graphics::{DrawList, Shape};
use veka_text::TextSystem;

use crate::element::{Element, View};
use crate::env::Environment;
use crate::primitive::{EventCx, EventResult, LayoutCx, PaintCx, PointerEvent, Primitive, Proposal};
use crate::state;
use crate::tree::{self, NodeId, Tree};

/// Bouton de souris.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
}

/// Événement d'entrée envoyé par l'hôte. Positions en points logiques, depuis le coin haut-gauche.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InputEvent {
    PointerMoved {
        position: Point,
    },
    PointerDown {
        position: Point,
        button: PointerButton,
    },
    PointerUp {
        position: Point,
        button: PointerButton,
    },
    /// Le pointeur a quitté la fenêtre.
    PointerLeft,
}

/// Racine : remplit la fenêtre, peint le fond, centre le contenu.
struct Root;

impl Primitive for Root {
    fn name(&self) -> &'static str {
        "root"
    }

    fn size_that_fits(&self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        Size::new(p.width.unwrap_or(0.0), p.height.unwrap_or(0.0))
    }

    fn place(&self, cx: &mut LayoutCx<'_>, rect: Rect) {
        for c in cx.children() {
            let s = cx.measure(c, Proposal::exact(rect.size));
            cx.place(
                c,
                rect.centered(Size::new(s.width.min(rect.width()), s.height.min(rect.height()))),
            );
        }
    }

    fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        let bg = cx.env().background;
        cx.list().fill(Shape::Rectangle, rect, bg);
        cx.paint_children();
    }
}

/// Une interface Veka : arbre de vues + layout + dessin + événements.
pub struct Surface {
    id: u32,
    tree: Tree,
    root: NodeId,
    text: TextSystem,
    env: Rc<Environment>,
    size: Size,
    scale: f32,
    list: DrawList,
    needs_layout: bool,
    needs_paint: bool,
    hovered: Option<NodeId>,
    captured: Option<NodeId>,
    regions: Vec<WindowRegion>,
    regions_changed: bool,
}

impl std::fmt::Debug for Surface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Surface")
            .field("id", &self.id)
            .field("size", &self.size)
            .finish()
    }
}

impl Surface {
    /// Avec les polices par défaut.
    pub fn new(root: impl View) -> Self {
        Self::with_text_system(root, TextSystem::new())
    }

    pub fn with_text_system(root: impl View, text: TextSystem) -> Self {
        let id = state::new_surface_id();
        let mut tree = Tree::new(id);
        let root = tree.insert(Element::new(Root, vec![root.into_element()]), None);
        Self {
            id,
            tree,
            root,
            text,
            env: Rc::new(Environment::default()),
            size: Size::ZERO,
            scale: 1.0,
            list: DrawList::new(),
            needs_layout: true,
            needs_paint: true,
            hovered: None,
            captured: None,
            regions: Vec::new(),
            regions_changed: false,
        }
    }

    /// Appelé quand un état change hors d'un événement (pour que l'hôte demande une image).
    pub fn set_waker(&mut self, waker: impl Fn() + 'static) {
        state::set_waker(self.id, Some(Rc::new(waker)));
    }

    pub fn environment(&self) -> &Environment {
        &self.env
    }

    pub fn set_environment(&mut self, env: Environment) {
        let window = self.env.window;
        self.env = Rc::new(Environment { window, ..env });
        self.needs_layout = true;
    }

    /// L'état de la fenêtre (agrandie, active…), lu par les vues dans `Environment::window`.
    pub fn set_window_info(&mut self, info: WindowInfo) {
        if self.env.window != info {
            self.env = Rc::new(Environment {
                window: info,
                ..(*self.env).clone()
            });
            self.needs_layout = true;
        }
    }

    pub fn window_info(&self) -> WindowInfo {
        self.env.window
    }

    /// Les zones de la fenêtre (déplacement, boutons) issues du dernier layout, en points.
    pub fn regions(&self) -> &[WindowRegion] {
        &self.regions
    }

    /// Les zones, seulement si elles ont changé depuis le dernier appel.
    pub fn take_regions_if_changed(&mut self) -> Option<Vec<WindowRegion>> {
        std::mem::take(&mut self.regions_changed).then(|| self.regions.clone())
    }

    /// Les actions sur la fenêtre demandées par les vues (boutons de fenêtre…), à exécuter par l'hôte.
    pub fn take_window_actions(&mut self) -> Vec<WindowAction> {
        crate::window::take_actions()
    }

    /// Taille en points logiques et échelle d'affichage (2.0 sur un écran Retina, 1.5 à 150 %…).
    pub fn resize(&mut self, size: Size, scale: f32) {
        if size != self.size || scale != self.scale {
            self.size = size;
            self.scale = scale;
            self.needs_layout = true;
        }
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Vrai si une nouvelle image est nécessaire. Faux au repos : l'hôte ne dessine rien.
    pub fn needs_frame(&self) -> bool {
        self.needs_layout || self.needs_paint || state::has_dirty(self.id)
    }

    /// Nombre total de nœuds créés depuis le début (sert aux tests : un nœud réutilisé n'est pas recréé).
    pub fn nodes_created(&self) -> usize {
        self.tree.created
    }

    /// Ré-évalue les vues dont l'état a changé, refait le layout si besoin et produit la liste de dessin.
    pub fn frame(&mut self) -> &DrawList {
        let mut dirty = state::take_dirty(self.id);
        while !dirty.is_empty() {
            // Les plus hauts d'abord : ré-évaluer un parent ré-évalue ses enfants.
            dirty.sort_by_key(|&n| self.depth(n));
            for n in dirty {
                if self.tree.nodes.contains_key(n) {
                    self.tree.rebuild(n);
                }
            }
            self.needs_layout = true;
            // Une vue a pu changer un état pendant son évaluation.
            dirty = state::take_dirty(self.id);
        }
        if self.needs_layout {
            self.tree.resolve_env(self.root, &self.env);
            let rect = Rect::from_origin_size(Point::ZERO, self.size);
            tree::measure(
                &mut self.tree,
                &mut self.text,
                self.root,
                Proposal::exact(self.size),
            );
            tree::place(&mut self.tree, &mut self.text, self.root, rect);
            self.needs_layout = false;
            self.needs_paint = true;
            self.revalidate_pointer();
            self.update_regions();
        }
        if self.needs_paint {
            self.list.clear();
            tree::paint(&self.tree, &mut self.list, self.root);
            self.needs_paint = false;
        }
        &self.list
    }

    /// La dernière liste produite par [`frame`](Self::frame).
    pub fn draw_list(&self) -> &DrawList {
        &self.list
    }

    /// Traite un événement. Renvoie `true` si une nouvelle image est nécessaire.
    pub fn handle(&mut self, event: InputEvent) -> bool {
        if self.needs_layout || state::has_dirty(self.id) {
            // Le hit-test doit voir l'interface telle qu'elle est affichée.
            self.frame();
        }
        let mut actions = Vec::new();
        match event {
            InputEvent::PointerMoved { position } => {
                if let Some(c) = self.captured.filter(|c| self.tree.nodes.contains_key(*c)) {
                    let inside = self.inside(c, position);
                    self.send(c, PointerEvent::Move { position, inside }, &mut actions);
                } else {
                    let hit = self.hit_test(position);
                    if hit != self.hovered {
                        if let Some(old) = self.hovered.filter(|o| self.tree.nodes.contains_key(*o)) {
                            self.send(old, PointerEvent::Leave, &mut actions);
                        }
                        self.hovered = hit;
                    }
                    if let Some(h) = hit {
                        self.send(
                            h,
                            PointerEvent::Move {
                                position,
                                inside: true,
                            },
                            &mut actions,
                        );
                    }
                }
            }
            InputEvent::PointerDown {
                position,
                button: PointerButton::Primary,
            } => {
                if let Some(h) = self.hit_test(position)
                    && self.send(h, PointerEvent::Down { position }, &mut actions)
                {
                    self.captured = Some(h);
                }
            }
            InputEvent::PointerUp {
                position,
                button: PointerButton::Primary,
            } => {
                if let Some(c) = self.captured.take().filter(|c| self.tree.nodes.contains_key(*c)) {
                    let inside = self.inside(c, position);
                    self.send(c, PointerEvent::Up { position, inside }, &mut actions);
                    if !inside {
                        // Relâché ailleurs : ce qui est sous le pointeur devient survolé.
                        self.hovered = self.hit_test(position);
                        if let Some(h) = self.hovered {
                            self.send(
                                h,
                                PointerEvent::Move {
                                    position,
                                    inside: true,
                                },
                                &mut actions,
                            );
                        }
                    }
                }
            }
            InputEvent::PointerLeft => {
                if let Some(old) = self.hovered.take().filter(|o| self.tree.nodes.contains_key(*o)) {
                    self.send(old, PointerEvent::Leave, &mut actions);
                }
            }
            _ => {}
        }
        // Les actions tournent quand plus rien n'est emprunté : elles peuvent modifier l'état.
        for a in actions {
            a.call();
        }
        self.needs_frame()
    }

    /// Rectangles des nœuds dont le primitive s'appelle `name`, dans l'ordre de l'arbre
    /// (sert aux tests et, plus tard, à l'accessibilité).
    pub fn rects_of(&self, name: &str) -> Vec<Rect> {
        let mut out = Vec::new();
        self.walk(self.root, &mut |n| {
            if n.prim.name() == name {
                out.push(n.rect);
            }
        });
        out
    }

    /// Les textes affichés, dans l'ordre de l'arbre.
    pub fn texts(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.walk(self.root, &mut |n| {
            if let Some(t) = (n.prim.as_ref() as &dyn std::any::Any).downcast_ref::<crate::views::TextPrim>()
            {
                out.push(t.content.clone());
            }
        });
        out
    }

    fn walk(&self, id: NodeId, f: &mut dyn FnMut(&tree::Node)) {
        let node = &self.tree.nodes[id];
        f(node);
        for &c in &node.children {
            self.walk(c, f);
        }
    }

    fn depth(&self, mut n: NodeId) -> usize {
        let mut d = 0;
        while let Some(p) = self.tree.nodes.get(n).and_then(|x| x.parent) {
            d += 1;
            n = p;
        }
        d
    }

    fn inside(&self, id: NodeId, p: Point) -> bool {
        let node = &self.tree.nodes[id];
        node.prim.hit_shape().is_some_and(|s| s.contains(&node.rect, p))
    }

    /// Le nœud le plus haut (dernier dessiné) qui reçoit le pointeur en `p`.
    pub(crate) fn hit_test(&self, p: Point) -> Option<NodeId> {
        fn walk(tree: &Tree, id: NodeId, p: Point) -> Option<NodeId> {
            let node = &tree.nodes[id];
            if node.prim.hit_children() {
                for &c in node.children.iter().rev() {
                    if let Some(h) = walk(tree, c, p) {
                        return Some(h);
                    }
                }
            }
            node.prim
                .hit_shape()
                .filter(|s| s.contains(&node.rect, p))
                .map(|_| id)
        }
        walk(&self.tree, self.root, p)
    }

    fn send(&mut self, id: NodeId, event: PointerEvent, actions: &mut Vec<crate::Action>) -> bool {
        let node = &mut self.tree.nodes[id];
        let prim = node.prim.clone();
        let r: EventResult = prim.pointer(
            &mut EventCx {
                state: node.state.as_mut(),
                rect: node.rect,
            },
            event,
        );
        if r.repaint {
            self.needs_paint = true;
        }
        actions.extend(r.action);
        r.handled
    }

    fn update_regions(&mut self) {
        fn collect(tree: &Tree, id: NodeId, in_drag: bool, out: &mut Vec<WindowRegion>) {
            let node = &tree.nodes[id];
            let rect = node.rect;
            match node.prim.window_region() {
                Some(RegionKind::Control(c)) => {
                    out.push(WindowRegion {
                        rect,
                        kind: RegionKind::Control(c),
                    });
                    return;
                }
                Some(RegionKind::Drag) => {
                    out.push(WindowRegion {
                        rect,
                        kind: RegionKind::Drag,
                    });
                    for &child in &node.children {
                        collect(tree, child, true, out);
                    }
                    return;
                }
                _ => {}
            }
            if in_drag && node.prim.hit_shape().is_some() {
                // Une vue interactive dans une zone de déplacement garde le pointeur.
                out.push(WindowRegion {
                    rect,
                    kind: RegionKind::Client,
                });
                if !node.prim.hit_children() {
                    return;
                }
            }
            for &child in &node.children {
                collect(tree, child, in_drag, out);
            }
        }
        let mut regions = Vec::new();
        collect(&self.tree, self.root, false, &mut regions);
        if regions != self.regions {
            self.regions = regions;
            self.regions_changed = true;
        }
    }

    /// Après un nouveau layout, le nœud survolé peut avoir disparu.
    fn revalidate_pointer(&mut self) {
        if self.hovered.is_some_and(|h| !self.tree.nodes.contains_key(h)) {
            self.hovered = None;
        }
        if self.captured.is_some_and(|h| !self.tree.nodes.contains_key(h)) {
            self.captured = None;
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        let root = self.root;
        self.tree.remove(root);
        state::set_waker(self.id, None);
        let _ = state::take_dirty(self.id);
    }
}
