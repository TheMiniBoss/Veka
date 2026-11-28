//! L'arbre retenu : un nœud par primitive ou vue composée. Il garde l'état, l'environnement résolu,
//! le layout calculé, et n'est mis à jour que là où les `Element` ont changé.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::rc::Rc;

use slotmap::{SlotMap, new_key_type};
use veka_foundation::{Rect, Size};
use veka_graphics::DrawList;
use veka_text::TextSystem;

use crate::anim::NodeAnims;
use crate::element::{Element, ElementKind, Key};
use crate::env::Environment;
use crate::primitive::{LayoutCx, PaintCx, Primitive, Proposal};
use crate::state::{self, Owner, StateKey};

new_key_type! {
    /// Identifiant d'un nœud de l'arbre.
    pub struct NodeId;
}

pub(crate) struct ComponentData {
    pub render: Rc<dyn Fn() -> Element>,
    pub hooks: Vec<StateKey>,
}

pub(crate) struct Node {
    pub prim: Rc<dyn Primitive>,
    /// Le type du primitive, ou de la vue composée et son `tag`.
    pub type_id: (TypeId, u64),
    pub key: Option<Key>,
    pub component: Option<ComponentData>,
    pub children: Vec<NodeId>,
    pub parent: Option<NodeId>,
    pub state: Box<dyn Any>,
    pub env: Rc<Environment>,
    /// Le rectangle affiché (celui qu'on dessine et qui reçoit le pointeur). Pendant une animation
    /// de layout, il va de l'ancien rectangle vers `layout_rect`.
    pub rect: Rect,
    /// Le rectangle calculé par le dernier layout.
    pub layout_rect: Rect,
    /// `layout_rect` au dernier passage d'Anima (`None` : nœud tout juste inséré).
    pub last_layout: Option<Rect>,
    /// Le primitive affiché pendant une animation de valeurs (sinon : `prim`).
    pub shown: Option<Rc<dyn Primitive>>,
    /// Le primitive d'avant, s'il a été remplacé depuis le dernier passage d'Anima.
    pub prev: Option<Rc<dyn Primitive>>,
    /// Le nœud sort de l'arbre : il n'a plus de place ni de pointeur, il est encore dessiné le temps
    /// de sa transition.
    pub leaving: bool,
    /// Mesures de l'image en cours.
    pub cache: Vec<(Proposal, Size)>,
}

impl Node {
    /// Le primitive à dessiner.
    pub fn shown(&self) -> &Rc<dyn Primitive> {
        self.shown.as_ref().unwrap_or(&self.prim)
    }
}

/// Le primitive d'une vue composée : transparent, il contient le résultat de la vue.
struct ComponentPrim;

impl Primitive for ComponentPrim {
    fn name(&self) -> &'static str {
        "component"
    }
}

pub(crate) struct Tree {
    pub nodes: SlotMap<NodeId, Node>,
    pub surface: u32,
    /// Nombre de nœuds créés depuis le début (pour les tests de réutilisation).
    pub created: usize,
    /// Les animations en cours, par nœud.
    pub anims: HashMap<NodeId, NodeAnims>,
    /// La vue qui a le focus du clavier.
    pub focused: Option<NodeId>,
}

impl Tree {
    pub fn new(surface: u32) -> Self {
        Self {
            nodes: SlotMap::with_key(),
            surface,
            created: 0,
            anims: HashMap::new(),
            focused: None,
        }
    }

    /// Crée le nœud d'un élément et tout son sous-arbre.
    pub fn insert(&mut self, element: Element, parent: Option<NodeId>) -> NodeId {
        let key = element.key;
        let (prim, type_id, component, children): (Rc<dyn Primitive>, (TypeId, u64), _, Vec<Element>) =
            match element.kind {
                ElementKind::Primitive { prim, children } => {
                    let t = (prim.as_ref() as &dyn Any).type_id();
                    (prim, (t, 0), None, children)
                }
                ElementKind::Component {
                    render, type_id, tag, ..
                } => (
                    Rc::new(ComponentPrim),
                    (type_id, tag),
                    Some(ComponentData {
                        render,
                        hooks: Vec::new(),
                    }),
                    Vec::new(),
                ),
            };
        let state = prim.create_state();
        let id = self.nodes.insert(Node {
            prim,
            type_id,
            key,
            component,
            children: Vec::new(),
            parent,
            state,
            env: Rc::new(Environment::default()),
            rect: Rect::ZERO,
            layout_rect: Rect::ZERO,
            last_layout: None,
            shown: None,
            prev: None,
            leaving: false,
            cache: Vec::new(),
        });
        self.created += 1;
        if self.nodes[id].component.is_some() {
            self.rebuild(id);
        } else {
            let kids = children.into_iter().map(|c| self.insert(c, Some(id))).collect();
            self.nodes[id].children = kids;
        }
        id
    }

    /// Supprime un nœud, son sous-arbre et leurs états.
    pub fn remove(&mut self, id: NodeId) {
        self.anims.remove(&id);
        if let Some(node) = self.nodes.remove(id) {
            if let Some(c) = &node.component {
                state::drop_states(&c.hooks);
            }
            for child in node.children {
                self.remove(child);
            }
        }
    }

    /// Retire un nœud de la liste de son parent, puis le supprime.
    pub fn detach(&mut self, id: NodeId) {
        if let Some(p) = self.nodes.get(id).and_then(|n| n.parent)
            && let Some(parent) = self.nodes.get_mut(p)
        {
            parent.children.retain(|&c| c != id);
        }
        self.remove(id);
    }

    /// Les enfants qui ne sont pas en train de sortir.
    pub fn live_children(&self, id: NodeId) -> Vec<NodeId> {
        self.live_children_iter(id).collect()
    }

    /// Comme [`live_children`](Self::live_children), sans allouer.
    pub fn live_children_iter(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.nodes[id]
            .children
            .iter()
            .copied()
            .filter(|&c| !self.nodes[c].leaving)
    }

    /// Ré-évalue une vue composée et met à jour son contenu.
    pub fn rebuild(&mut self, id: NodeId) {
        let Some(comp) = self.nodes.get_mut(id).and_then(|n| n.component.as_mut()) else {
            return;
        };
        let render = comp.render.clone();
        let hooks = std::mem::take(&mut comp.hooks);
        state::begin_frame(
            Owner {
                surface: self.surface,
                node: id,
            },
            hooks,
        );
        // Aucun emprunt de l'arbre pendant le code de l'utilisateur.
        let element = render();
        let hooks = state::end_frame();
        if let Some(comp) = self.nodes[id].component.as_mut() {
            comp.hooks = hooks;
        }
        self.reconcile_children(id, vec![element]);
    }

    /// Met à jour `id` avec un nouvel élément du même type.
    fn update(&mut self, id: NodeId, element: Element) {
        match element.kind {
            ElementKind::Primitive { prim, children } => {
                let node = &mut self.nodes[id];
                let old = std::mem::replace(&mut node.prim, prim);
                // Le premier remplacement depuis le dernier passage d'Anima garde l'ancien.
                if node.prev.is_none() {
                    node.prev = Some(old);
                }
                self.reconcile_children(id, children);
            }
            ElementKind::Component { render, .. } => {
                if let Some(c) = self.nodes[id].component.as_mut() {
                    c.render = render;
                }
                // Le parent a été ré-évalué : la vue l'est aussi (ses paramètres ont pu changer).
                self.rebuild(id);
            }
        }
    }

    /// Réconcilie les enfants : même type + même position (ou même `.id()`) = nœud réutilisé.
    pub fn reconcile_children(&mut self, id: NodeId, elements: Vec<Element>) {
        let all = std::mem::take(&mut self.nodes[id].children);
        // Ceux qui sortent déjà ne comptent plus : ils gardent seulement leur place à l'écran.
        let mut leaving: smallvec::SmallVec<[(usize, NodeId); 4]> = all
            .iter()
            .enumerate()
            .filter(|&(_, &o)| self.nodes[o].leaving)
            .map(|(i, &o)| (i, o))
            .collect();
        let mut old: smallvec::SmallVec<[(usize, NodeId); 8]> = all
            .iter()
            .enumerate()
            .filter(|&(_, &o)| !self.nodes[o].leaving)
            .map(|(i, &o)| (i, o))
            .collect();
        let mut new = Vec::with_capacity(elements.len() + leaving.len());
        for (i, element) in elements.into_iter().enumerate() {
            let type_id = element_type(&element);
            let found = match &element.key {
                Some(k) => old
                    .iter()
                    .position(|&(_, o)| o != NodeId::default() && self.nodes[o].key.as_ref() == Some(k)),
                None => old
                    .get(i)
                    .and_then(|&(_, o)| self.nodes.get(o))
                    .and_then(|n| n.key.is_none().then_some(i)),
            };
            let reuse = found.filter(|&j| self.nodes.get(old[j].1).is_some_and(|n| n.type_id == type_id));
            match reuse {
                Some(j) => {
                    let node = old[j].1;
                    // Garde la place (positions des suivants) : remplacé par une valeur invalide.
                    old[j].1 = NodeId::default();
                    self.update(node, element);
                    new.push(node);
                }
                None => new.push(self.insert(element, Some(id))),
            }
        }
        // Les autres sortent : Anima décide ensuite s'ils disparaissent tout de suite ou avec une
        // transition.
        for (i, o) in old {
            if o != NodeId::default() {
                self.nodes[o].leaving = true;
                leaving.push((i, o));
            }
        }
        leaving.sort_by_key(|&(i, _)| i);
        for (i, o) in leaving {
            new.insert(i.min(new.len()), o);
        }
        self.nodes[id].children = new;
    }

    /// Calcule l'environnement de chaque nœud.
    pub fn resolve_env(&mut self, id: NodeId, parent: &Rc<Environment>) {
        let node = &mut self.nodes[id];
        let mut env = (**parent).clone();
        node.env = if node.shown().update_env(&mut env) {
            Rc::new(env)
        } else {
            parent.clone()
        };
        node.cache.clear();
        let env = node.env.clone();
        // Par indice : sans copier la liste des enfants.
        let mut i = 0;
        while let Some(&child) = self.nodes[id].children.get(i) {
            self.resolve_env(child, &env);
            i += 1;
        }
    }
}

fn element_type(e: &Element) -> (TypeId, u64) {
    match &e.kind {
        ElementKind::Primitive { prim, .. } => ((prim.as_ref() as &dyn Any).type_id(), 0),
        ElementKind::Component { type_id, tag, .. } => (*type_id, *tag),
    }
}

pub(crate) fn measure(tree: &mut Tree, text: &mut TextSystem, id: NodeId, proposal: Proposal) -> Size {
    if let Some(&(_, s)) = tree.nodes[id].cache.iter().find(|(p, _)| *p == proposal) {
        return s;
    }
    let prim = tree.nodes[id].prim.clone();
    let size = prim.size_that_fits(&mut LayoutCx { tree, text, node: id }, proposal);
    tree.nodes[id].cache.push((proposal, size));
    size
}

pub(crate) fn place(tree: &mut Tree, text: &mut TextSystem, id: NodeId, rect: Rect) {
    // Le rectangle affiché est mis à jour par Anima, juste après le layout.
    tree.nodes[id].layout_rect = rect;
    let prim = tree.nodes[id].prim.clone();
    prim.place(&mut LayoutCx { tree, text, node: id }, rect);
}

pub(crate) fn paint(tree: &Tree, list: &mut DrawList, id: NodeId) {
    let node = &tree.nodes[id];
    let anims = tree.anims.get(&id);
    let frame = anims.and_then(|a| a.transition_frame(node.rect));
    let pops = match frame {
        Some(f) => crate::anim::push_frame(list, &f, node.rect),
        None => 0,
    };
    let geometry = anims.and_then(|a| a.geometry_transform(node.rect));
    if let Some(t) = geometry {
        list.push(veka_graphics::DrawCommand::PushTransform(t));
    }
    node.shown()
        .paint(&mut PaintCx { tree, list, node: id }, node.rect);
    if geometry.is_some() {
        list.push(veka_graphics::DrawCommand::PopTransform);
    }
    crate::anim::pop(list, pops);
}
