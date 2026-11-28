//! `Surface` : une interface Veka sans fenêtre. L'hôte (app native, moteur de jeu…) lui donne une
//! taille et des événements, et lui demande une liste de dessin quand elle en a besoin.

use std::rc::Rc;

use veka_foundation::{
    Color, ColorScheme, Point, Rect, RegionKind, Size, WindowAction, WindowInfo, WindowRegion,
};
use veka_graphics::{DrawCommand, DrawList, Shape};
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
        let bg = cx.env().resolve(&cx.env().background);
        cx.list().fill(Shape::Rectangle, rect, bg);
        // Comme SwiftUI : le contenu est dessiné au-dessus du fond de la fenêtre, qui n'en fait pas
        // partie. Un mode de fusion se mélange avec tout ce que l'interface a dessiné derrière la vue,
        // mais pas avec le fond de la fenêtre : s'il y en a un, le contenu passe dans un calque
        // transparent, posé ensuite normalement sur le fond.
        let start = cx.list().commands.len();
        cx.paint_children();
        let list = cx.list();
        let blends = list.commands[start..].iter().any(|c| {
            matches!(c, DrawCommand::PushGroup { blend, .. } if *blend != veka_graphics::BlendMode::Normal)
        });
        if blends {
            list.commands.insert(
                start,
                DrawCommand::PushGroup {
                    blend: veka_graphics::BlendMode::Normal,
                    opacity: 1.0,
                },
            );
            list.push(DrawCommand::PopGroup);
        }
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
    /// L'heure de la dernière image, en secondes (donnée par l'hôte, voir `frame_at`).
    now: f64,
    /// Les vues `.on_animation_end` dont le sous-arbre s'animait à l'image précédente.
    animating_watchers: Vec<NodeId>,
    /// Vrai une fois que l'hôte a donné l'heure (`frame_at`) : avant, les timelines n'avancent pas.
    clock: bool,
    /// Le pointeur, s'il est dans la fenêtre (lumière des verres `interactive`).
    pointer: Option<Point>,
    /// Le mode du système, et celui que les vues demandent pour la fenêtre (`.preferred_color_scheme`).
    system_scheme: ColorScheme,
    preferred_scheme: Option<ColorScheme>,
    preferred_changed: bool,
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
            now: 0.0,
            animating_watchers: Vec::new(),
            clock: false,
            pointer: None,
            system_scheme: ColorScheme::default(),
            preferred_scheme: None,
            preferred_changed: false,
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
        let reduce_motion = self.env.reduce_motion;
        let reduce_transparency = self.env.reduce_transparency;
        let backdrop_quality = self.env.backdrop_quality;
        self.env = Rc::new(Environment {
            window,
            reduce_motion,
            reduce_transparency,
            backdrop_quality,
            ..env
        });
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

    /// Le mode clair ou sombre du système et sa couleur d'accent, donnés par l'hôte.
    pub fn set_system_appearance(&mut self, scheme: ColorScheme, accent: Option<Color>) {
        self.system_scheme = scheme;
        let scheme = self.preferred_scheme.unwrap_or(scheme);
        if self.env.color_scheme != scheme || self.env.system_accent != accent {
            self.env = Rc::new(Environment {
                color_scheme: scheme,
                system_accent: accent,
                ..(*self.env).clone()
            });
            self.needs_layout = true;
        }
    }

    /// Le réglage « Réduire les animations » du système, donné par l'hôte.
    pub fn set_reduce_motion(&mut self, reduce: bool) {
        if self.env.reduce_motion != reduce {
            self.env = Rc::new(Environment {
                reduce_motion: reduce,
                ..(*self.env).clone()
            });
            self.needs_layout = true;
        }
    }

    /// Le réglage « Réduire la transparence » du système, donné par l'hôte.
    pub fn set_reduce_transparency(&mut self, reduce: bool) {
        if self.env.reduce_transparency != reduce {
            self.env = Rc::new(Environment {
                reduce_transparency: reduce,
                ..(*self.env).clone()
            });
            self.needs_layout = true;
        }
    }

    /// La qualité des effets d'arrière-plan.
    pub fn set_backdrop_quality(&mut self, quality: crate::env::BackdropQuality) {
        if self.env.backdrop_quality != quality {
            self.env = Rc::new(Environment {
                backdrop_quality: quality,
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
    /// Le mode demandé pour la fenêtre par `.preferred_color_scheme`, s'il a changé depuis le dernier
    /// appel (`Some(None)` : de nouveau celui du système). L'hôte l'applique à la barre de titre.
    pub fn take_preferred_color_scheme_if_changed(&mut self) -> Option<Option<ColorScheme>> {
        std::mem::take(&mut self.preferred_changed).then_some(self.preferred_scheme)
    }

    /// Le mode demandé pour la fenêtre par `.preferred_color_scheme` (`None` : celui du système).
    pub fn preferred_color_scheme(&self) -> Option<ColorScheme> {
        self.preferred_scheme
    }

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
        self.needs_layout || self.needs_paint || state::has_dirty(self.id) || self.is_animating()
    }

    /// Vrai tant qu'une animation tourne : l'hôte doit alors redessiner à chaque image.
    pub fn is_animating(&self) -> bool {
        !self.tree.anims.is_empty() || crate::timeline::has_playing(self.id)
    }

    /// Comme [`frame`](Self::frame), à l'heure `now` (celle de la plateforme) : les animations
    /// avancent jusque-là.
    pub fn frame_at(&mut self, now: veka_foundation::Instant) -> &DrawList {
        self.now = veka_anima::seconds(now).max(self.now);
        self.clock = true;
        self.frame()
    }

    /// Nombre total de nœuds créés depuis le début (sert aux tests : un nœud réutilisé n'est pas recréé).
    pub fn nodes_created(&self) -> usize {
        self.tree.created
    }

    /// Ré-évalue les vues dont l'état a changé, refait le layout si besoin et produit la liste de dessin.
    /// Les animations restent à l'heure de la dernière image (voir [`frame_at`](Self::frame_at)).
    pub fn frame(&mut self) -> &DrawList {
        let now = self.now;
        if self.clock {
            crate::timeline::advance(self.id, now);
        }
        let ticked = crate::anim::tick(&mut self.tree, now);
        self.needs_paint |= ticked.paint;
        self.needs_layout |= ticked.layout;
        let mut spec = None;
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
            spec = spec.or(state::take_transaction(self.id));
            // Une vue a pu changer un état pendant son évaluation.
            dirty = state::take_dirty(self.id);
        }
        // Deux tours au plus : un `.preferred_color_scheme` qui change refait l'environnement.
        for _ in 0..2 {
            if !self.needs_layout {
                break;
            }
            self.tree.resolve_env(self.root, &self.env);
            let rect = Rect::from_origin_size(Point::ZERO, self.size);
            tree::measure(
                &mut self.tree,
                &mut self.text,
                self.root,
                Proposal::exact(self.size),
            );
            tree::place(&mut self.tree, &mut self.text, self.root, rect);
            crate::anim::after_layout(&mut self.tree, self.root, spec, now);
            self.needs_layout = false;
            self.needs_paint = true;
            self.revalidate_pointer();
            self.update_regions();
            self.update_preferred_scheme();
        }
        if self.needs_paint {
            self.list.clear();
            tree::paint(&self.tree, &mut self.list, self.root);
            self.needs_paint = false;
        }
        self.list.pointer = self.pointer;
        for a in self.finished_animations() {
            a.call();
        }
        &self.list
    }

    /// Les rappels `.on_animation_end` dont le sous-arbre vient d'arrêter de s'animer.
    fn finished_animations(&mut self) -> Vec<crate::Action> {
        if self.tree.anims.is_empty() && self.animating_watchers.is_empty() {
            return Vec::new();
        }
        fn animating(tree: &Tree, id: NodeId) -> bool {
            tree.anims.contains_key(&id) || tree.nodes[id].children.iter().any(|&c| animating(tree, c))
        }
        fn visit(
            tree: &Tree,
            id: NodeId,
            now: &mut Vec<NodeId>,
            before: &[NodeId],
            out: &mut Vec<crate::Action>,
        ) {
            let node = &tree.nodes[id];
            if let Some(w) =
                (node.prim.as_ref() as &dyn std::any::Any).downcast_ref::<crate::views::AnimationEndPrim>()
            {
                if animating(tree, id) {
                    now.push(id);
                } else if before.contains(&id) {
                    out.push(w.action.clone());
                }
            }
            for &c in &node.children {
                visit(tree, c, now, before, out);
            }
        }
        let before = std::mem::take(&mut self.animating_watchers);
        let mut out = Vec::new();
        visit(
            &self.tree,
            self.root,
            &mut self.animating_watchers,
            &before,
            &mut out,
        );
        out
    }

    /// La dernière liste produite par [`frame`](Self::frame).
    pub fn draw_list(&self) -> &DrawList {
        &self.list
    }

    /// Le pointeur a bougé : une image de plus seulement s'il y a un verre qui suit la lumière.
    fn set_pointer(&mut self, p: Option<Point>) {
        if self.pointer == p {
            return;
        }
        self.pointer = p;
        let interactive = self.list.commands.iter().any(|c| {
            matches!(
                c,
                veka_graphics::DrawCommand::Backdrop {
                    effect: veka_graphics::BackdropEffect::Glass(g),
                    ..
                } if g.interactive
            )
        });
        if interactive {
            self.list.pointer = p;
            self.needs_paint = true;
        }
    }

    /// Traite un événement. Renvoie `true` si une nouvelle image est nécessaire.
    pub fn handle(&mut self, event: InputEvent) -> bool {
        if self.needs_layout || state::has_dirty(self.id) {
            // Le hit-test doit voir l'interface telle qu'elle est affichée.
            self.frame();
        }
        let mut actions = Vec::new();
        match event {
            InputEvent::PointerMoved { position } | InputEvent::PointerDown { position, .. } => {
                self.set_pointer(Some(position));
            }
            InputEvent::PointerLeft => self.set_pointer(None),
            _ => {}
        }
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

    /// Parcourt l'arbre affiché, sans les nœuds qui sortent.
    fn walk(&self, id: NodeId, f: &mut dyn FnMut(&tree::Node)) {
        let node = &self.tree.nodes[id];
        f(node);
        for c in self.tree.live_children_iter(id) {
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
                for c in tree.live_children(id).into_iter().rev() {
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

    fn update_preferred_scheme(&mut self) {
        fn find(tree: &Tree, id: NodeId) -> Option<ColorScheme> {
            let node = &tree.nodes[id];
            if let Some(s) = node.prim.preferred_color_scheme() {
                return Some(s);
            }
            tree.live_children_iter(id).find_map(|c| find(tree, c))
        }
        let found = find(&self.tree, self.root);
        if found == self.preferred_scheme {
            return;
        }
        self.preferred_scheme = found;
        self.preferred_changed = true;
        let scheme = found.unwrap_or(self.system_scheme);
        if self.env.color_scheme != scheme {
            self.env = Rc::new(Environment {
                color_scheme: scheme,
                ..(*self.env).clone()
            });
            self.needs_layout = true;
        }
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
                    for child in tree.live_children_iter(id) {
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
            for child in tree.live_children_iter(id) {
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
        let alive = |tree: &Tree, h: NodeId| tree.nodes.get(h).is_some_and(|n| !n.leaving);
        if self.hovered.is_some_and(|h| !alive(&self.tree, h)) {
            self.hovered = None;
        }
        if self.captured.is_some_and(|h| !alive(&self.tree, h)) {
            self.captured = None;
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        let root = self.root;
        self.tree.remove(root);
        state::set_waker(self.id, None);
        crate::timeline::forget(self.id);
        let _ = state::take_dirty(self.id);
    }
}
