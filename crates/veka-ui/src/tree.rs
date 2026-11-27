//! L'arbre retenu : un nœud par primitive ou vue composée. Il garde l'état, l'environnement résolu,
//! le layout calculé, et n'est mis à jour que là où les `Element` ont changé.

use std::any::{Any, TypeId};
use std::rc::Rc;

use slotmap::{SlotMap, new_key_type};
use veka_foundation::{Rect, Size};
use veka_graphics::DrawList;
use veka_text::TextSystem;

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
    pub type_id: TypeId,
    pub key: Option<Key>,
    pub component: Option<ComponentData>,
    pub children: Vec<NodeId>,
    pub parent: Option<NodeId>,
    pub state: Box<dyn Any>,
    pub env: Rc<Environment>,
    pub rect: Rect,
    /// Mesures de l'image en cours.
    pub cache: Vec<(Proposal, Size)>,
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
}

impl Tree {
    pub fn new(surface: u32) -> Self {
        Self {
            nodes: SlotMap::with_key(),
            surface,
            created: 0,
        }
    }

    /// Crée le nœud d'un élément et tout son sous-arbre.
    pub fn insert(&mut self, element: Element, parent: Option<NodeId>) -> NodeId {
        let key = element.key;
        let (prim, type_id, component, children): (Rc<dyn Primitive>, TypeId, _, Vec<Element>) =
            match element.kind {
                ElementKind::Primitive { prim, children } => {
                    let t = (prim.as_ref() as &dyn Any).type_id();
                    (prim, t, None, children)
                }
                ElementKind::Component { render, type_id, .. } => (
                    Rc::new(ComponentPrim),
                    type_id,
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
        if let Some(node) = self.nodes.remove(id) {
            if let Some(c) = &node.component {
                state::drop_states(&c.hooks);
            }
            for child in node.children {
                self.remove(child);
            }
        }
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
                self.nodes[id].prim = prim;
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
        let mut old = std::mem::take(&mut self.nodes[id].children);
        let mut new = Vec::with_capacity(elements.len());
        for (i, element) in elements.into_iter().enumerate() {
            let type_id = element_type(&element);
            let found = match &element.key {
                Some(k) => old.iter().position(|o| self.nodes[*o].key.as_ref() == Some(k)),
                None => old
                    .get(i)
                    .and_then(|o| self.nodes.get(*o))
                    .and_then(|n| n.key.is_none().then_some(i)),
            };
            let reuse = found.filter(|&j| self.nodes.get(old[j]).is_some_and(|n| n.type_id == type_id));
            match reuse {
                Some(j) => {
                    let node = old[j];
                    // Garde la place (positions des suivants) : remplacé par une valeur invalide.
                    old[j] = NodeId::default();
                    self.update(node, element);
                    new.push(node);
                }
                None => new.push(self.insert(element, Some(id))),
            }
        }
        for o in old {
            if o != NodeId::default() {
                self.remove(o);
            }
        }
        self.nodes[id].children = new;
    }

    /// Calcule l'environnement de chaque nœud.
    pub fn resolve_env(&mut self, id: NodeId, parent: &Rc<Environment>) {
        let node = &mut self.nodes[id];
        let mut env = (**parent).clone();
        node.env = if node.prim.update_env(&mut env) {
            Rc::new(env)
        } else {
            parent.clone()
        };
        node.cache.clear();
        let env = node.env.clone();
        for child in node.children.clone() {
            self.resolve_env(child, &env);
        }
    }
}

fn element_type(e: &Element) -> TypeId {
    match &e.kind {
        ElementKind::Primitive { prim, .. } => (prim.as_ref() as &dyn Any).type_id(),
        ElementKind::Component { type_id, .. } => *type_id,
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
    tree.nodes[id].rect = rect;
    let prim = tree.nodes[id].prim.clone();
    prim.place(&mut LayoutCx { tree, text, node: id }, rect);
}

pub(crate) fn paint(tree: &Tree, list: &mut DrawList, id: NodeId) {
    let node = &tree.nodes[id];
    node.prim.paint(&mut PaintCx { tree, list, node: id }, node.rect);
}
