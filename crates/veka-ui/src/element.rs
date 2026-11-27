//! `Element` : la description légère d'un morceau d'interface.

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use crate::primitive::Primitive;

/// Identité explicite d'un élément parmi ses frères (listes).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Int(i64),
    Str(Rc<str>),
}

impl From<i64> for Key {
    fn from(v: i64) -> Self {
        Key::Int(v)
    }
}
impl From<i32> for Key {
    fn from(v: i32) -> Self {
        Key::Int(v as i64)
    }
}
impl From<&str> for Key {
    fn from(v: &str) -> Self {
        Key::Str(v.into())
    }
}

/// Une action (clic d'un bouton…). Clonable ; l'appel passe par un `RefCell` (un seul à la fois).
#[derive(Clone)]
pub struct Action(Rc<RefCell<dyn FnMut()>>);

impl Action {
    pub fn new(f: impl FnMut() + 'static) -> Self {
        Self(Rc::new(RefCell::new(f)))
    }

    pub fn call(&self) {
        // Une action qui se relance elle-même est ignorée plutôt que de paniquer.
        if let Ok(mut f) = self.0.try_borrow_mut() {
            f();
        }
    }
}

impl fmt::Debug for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Action")
    }
}

pub(crate) enum ElementKind {
    Primitive {
        prim: Rc<dyn Primitive>,
        children: Vec<Element>,
    },
    Component {
        render: Rc<dyn Fn() -> Element>,
        type_id: TypeId,
        /// Distingue des vues qui partagent la même fonction (vues d'un fichier `.vk`).
        tag: u64,
        name: &'static str,
    },
}

/// Description d'un morceau d'interface : un primitive (avec ses enfants) ou une vue composée.
pub struct Element {
    pub(crate) kind: ElementKind,
    pub(crate) key: Option<Key>,
}

impl fmt::Debug for Element {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ElementKind::Primitive { prim, children } => {
                let mut t = f.debug_tuple(prim.name());
                for c in children {
                    t.field(c);
                }
                t.finish()
            }
            ElementKind::Component { name, .. } => write!(f, "component({name})"),
        }
    }
}

impl Element {
    /// Un primitive et ses enfants.
    pub fn new(prim: impl Primitive, children: Vec<Element>) -> Self {
        Self {
            kind: ElementKind::Primitive {
                prim: Rc::new(prim),
                children,
            },
            key: None,
        }
    }

    /// Enveloppe cet élément dans `prim` (c'est ainsi que sont faits les modifiers). L'identité
    /// (`.id()`) passe à l'enveloppe : c'est elle qui est rangée parmi les frères.
    pub fn wrap(mut self, prim: impl Primitive) -> Self {
        let key = self.key.take();
        let mut e = Element::new(prim, vec![self]);
        e.key = key;
        e
    }

    /// Identité explicite parmi les frères.
    pub fn id(mut self, key: impl Into<Key>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// Le primitive, s'il y en a un et s'il est du type `P`.
    pub fn primitive<P: Primitive>(&self) -> Option<&P> {
        match &self.kind {
            ElementKind::Primitive { prim, .. } => (prim.as_ref() as &dyn Any).downcast_ref::<P>(),
            ElementKind::Component { .. } => None,
        }
    }

    /// Remplace le primitive s'il est du type `P` (sert aux modifiers propres à un type, comme `fill`).
    pub(crate) fn map_primitive<P: Primitive + Clone>(self, f: impl FnOnce(P) -> P) -> Result<Self, Self> {
        if let Some(p) = self.primitive::<P>() {
            let new = f(p.clone());
            let ElementKind::Primitive { children, .. } = self.kind else {
                unreachable!()
            };
            return Ok(Element {
                kind: ElementKind::Primitive {
                    prim: Rc::new(new),
                    children,
                },
                key: self.key,
            });
        }
        Err(self)
    }

    /// Noms de tous les primitives de l'arbre (les vues composées ne sont pas déroulées).
    pub fn primitive_names(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        fn walk(e: &Element, out: &mut Vec<&'static str>) {
            if let ElementKind::Primitive { prim, children } = &e.kind {
                out.push(prim.name());
                children.iter().for_each(|c| walk(c, out));
            }
        }
        walk(self, &mut out);
        out
    }
}

/// Tout ce qui peut devenir un [`Element`]. Une vue s'écrit `fn ma_vue() -> impl View`.
pub trait View {
    fn into_element(self) -> Element;
}

impl View for Element {
    fn into_element(self) -> Element {
        self
    }
}

/// Une vue composée avec son propre état : la fonction est ré-appelée quand cet état change.
///
/// ```
/// # use veka_ui::prelude::*;
/// fn ligne() -> impl View { Text("Bonjour") }
/// let e = VStack![component(ligne), component(ligne)];
/// ```
pub fn component<F, V>(render: F) -> Element
where
    F: Fn() -> V + 'static,
    V: View,
{
    Element {
        kind: ElementKind::Component {
            render: Rc::new(move || render().into_element()),
            type_id: TypeId::of::<F>(),
            tag: 0,
            name: std::any::type_name::<F>(),
        },
        key: None,
    }
}

/// Comme [`component`], pour des vues décrites par des données (un fichier `.vk`) : deux vues de même
/// `tag` sont la même vue (elles gardent leurs états), deux `tag` différents ne le sont jamais.
pub fn component_tagged(tag: u64, name: &'static str, render: impl Fn() -> Element + 'static) -> Element {
    Element {
        kind: ElementKind::Component {
            render: Rc::new(render),
            type_id: TypeId::of::<ComponentTagged>(),
            tag,
            name,
        },
        key: None,
    }
}

struct ComponentTagged;
