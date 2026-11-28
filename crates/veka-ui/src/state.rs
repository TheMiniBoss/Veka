//! L'état des vues.
//!
//! Un [`State<T>`] est une **poignée** (`Copy`) vers une valeur gardée par Veka pour la vue qui l'a
//! créée. Une vue retrouve ses états par **ordre d'appel** : les `State::new` d'une vue doivent
//! toujours être appelés dans le même ordre (jamais dans un `if`).
//!
//! L'interface vit sur un seul thread : Veka garde ici, dans des variables locales au thread, le
//! contexte courant. C'est ce qui permet d'écrire `clics += 1` sans contexte à passer.

use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::ops::{AddAssign, MulAssign, SubAssign};
use std::rc::Rc;

use slotmap::{SlotMap, new_key_type};

use veka_anima::Animation;

use crate::tree::NodeId;

new_key_type! {
    /// Identifiant d'une valeur d'état.
    pub struct StateKey;
}

/// Vue propriétaire d'un état : surface + nœud.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Owner {
    pub surface: u32,
    pub node: NodeId,
}

struct Slot {
    value: Box<dyn Any>,
    type_id: TypeId,
    owner: Owner,
}

/// Les états d'une vue pendant qu'elle est évaluée.
pub(crate) struct HookFrame {
    pub owner: Owner,
    pub hooks: Vec<StateKey>,
    pub index: usize,
}

thread_local! {
    static STATES: RefCell<SlotMap<StateKey, Slot>> = RefCell::new(SlotMap::with_key());
    static FRAME: RefCell<Option<HookFrame>> = const { RefCell::new(None) };
    static DIRTY: RefCell<Vec<Owner>> = const { RefCell::new(Vec::new()) };
    static WAKERS: RefCell<HashMap<u32, Rc<dyn Fn()>>> = RefCell::new(HashMap::new());
    static NEXT_SURFACE: Cell<u32> = const { Cell::new(1) };
    /// La spec du `with_animation` en cours.
    static TRANSACTION: Cell<Option<Animation>> = const { Cell::new(None) };
    /// La spec avec laquelle animer la prochaine image de chaque surface.
    static PENDING: RefCell<HashMap<u32, Animation>> = RefCell::new(HashMap::new());
}

/// Exécute `f` ; ce que changent les états modifiés dedans s'anime avec `spec` (layout, couleurs,
/// opacité…, entrées et sorties de vues). Comme `withAnimation` de SwiftUI.
///
/// ```
/// # use veka_ui::prelude::*;
/// # fn vue() -> impl View {
/// let ouvert = State::new(false);
/// Button("Ouvrir", move || with_animation(Animation::bouncy(), || ouvert.set(true)))
/// # }
/// ```
pub fn with_animation<R>(spec: Animation, f: impl FnOnce() -> R) -> R {
    let prev = TRANSACTION.with(|t| t.replace(Some(spec)));
    let r = f();
    TRANSACTION.with(|t| t.set(prev));
    r
}

/// L'animation du `with_animation` en cours, s'il y en a un.
pub(crate) fn current_transaction() -> Option<Animation> {
    TRANSACTION.with(|t| t.get())
}

/// Réveille toutes les surfaces (une demande qui ne vient pas d'un état, comme `scroll_to`).
pub(crate) fn wake_all() {
    let wakers: Vec<Rc<dyn Fn()>> = WAKERS.with(|w| w.borrow().values().cloned().collect());
    for w in wakers {
        w();
    }
}

/// La spec à appliquer à la prochaine image de `surface` (et l'oublie).
pub(crate) fn take_transaction(surface: u32) -> Option<Animation> {
    PENDING.with(|p| p.borrow_mut().remove(&surface))
}

pub(crate) fn new_surface_id() -> u32 {
    NEXT_SURFACE.with(|n| {
        let id = n.get();
        n.set(id + 1);
        id
    })
}

pub(crate) fn set_waker(surface: u32, waker: Option<Rc<dyn Fn()>>) {
    WAKERS.with(|w| match waker {
        Some(f) => {
            w.borrow_mut().insert(surface, f);
        }
        None => {
            w.borrow_mut().remove(&surface);
        }
    });
}

/// Lance l'évaluation d'une vue : ses `State::new` retrouveront `hooks` dans l'ordre.
pub(crate) fn begin_frame(owner: Owner, hooks: Vec<StateKey>) {
    FRAME.with(|f| {
        let prev = f.borrow_mut().replace(HookFrame {
            owner,
            hooks,
            index: 0,
        });
        assert!(
            prev.is_none(),
            "Veka : une vue est évaluée pendant l'évaluation d'une autre"
        );
    });
}

/// Termine l'évaluation ; renvoie les états de la vue.
pub(crate) fn end_frame() -> Vec<StateKey> {
    let frame = FRAME
        .with(|f| f.borrow_mut().take())
        .expect("évaluation en cours");
    if frame.index < frame.hooks.len() {
        // Moins de `State::new` que la dernière fois : un état a été créé dans une condition.
        debug_assert!(
            false,
            "Veka : cette vue a appelé State::new {} fois au lieu de {} ; \
             appelez toujours les State::new dans le même ordre (pas dans un `if`)",
            frame.index,
            frame.hooks.len()
        );
    }
    frame.hooks
}

/// Supprime les états d'une vue retirée de l'arbre.
pub(crate) fn drop_states(keys: &[StateKey]) {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        for k in keys {
            s.remove(*k);
        }
    });
}

/// Vues à ré-évaluer pour une surface (vidé à la lecture).
pub(crate) fn take_dirty(surface: u32) -> Vec<NodeId> {
    DIRTY.with(|d| {
        let mut d = d.borrow_mut();
        let mut out = Vec::new();
        d.retain(|o| {
            if o.surface == surface {
                if !out.contains(&o.node) {
                    out.push(o.node);
                }
                false
            } else {
                true
            }
        });
        out
    })
}

pub(crate) fn has_dirty(surface: u32) -> bool {
    DIRTY.with(|d| d.borrow().iter().any(|o| o.surface == surface))
}

fn mark_dirty(owner: Owner) {
    if let Some(spec) = TRANSACTION.with(|t| t.get()) {
        PENDING.with(|p| p.borrow_mut().insert(owner.surface, spec));
    }
    let first = DIRTY.with(|d| {
        let mut d = d.borrow_mut();
        let first = !d.iter().any(|o| o.surface == owner.surface);
        if !d.contains(&owner) {
            d.push(owner);
        }
        first
    });
    if first {
        // Hors de tout emprunt : la plateforme peut demander une image tout de suite.
        if let Some(w) = WAKERS.with(|w| w.borrow().get(&owner.surface).cloned()) {
            w();
        }
    }
}

/// Une valeur d'état d'une vue. Voir le [module](self).
pub struct State<T> {
    key: StateKey,
    _t: PhantomData<*const T>,
}

impl<T> Clone for State<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for State<T> {}

impl<T: 'static> State<T> {
    /// Crée l'état (la première fois) ou retrouve celui de l'évaluation précédente.
    ///
    /// À appeler pendant l'évaluation d'une vue, toujours dans le même ordre.
    pub fn new(initial: T) -> Self {
        Self::new_with(|| initial)
    }

    /// Comme [`new`](Self::new), la valeur initiale n'étant calculée que la première fois.
    pub fn new_with(initial: impl FnOnce() -> T) -> Self {
        let key = FRAME.with(|f| {
            let mut f = f.borrow_mut();
            let frame = f.as_mut().expect(
                "Veka : State::new doit être appelé pendant l'évaluation d'une vue \
                 (dans une fonction passée à `component` ou à la fenêtre)",
            );
            let i = frame.index;
            frame.index += 1;
            if let Some(&key) = frame.hooks.get(i) {
                let ok = STATES.with(|s| {
                    s.borrow()
                        .get(key)
                        .is_some_and(|slot| slot.type_id == TypeId::of::<T>())
                });
                assert!(
                    ok,
                    "Veka : l'état n° {} de cette vue a changé de type ; \
                     appelez toujours les State::new dans le même ordre (pas dans un `if`)",
                    i + 1
                );
                key
            } else {
                let owner = frame.owner;
                let key = STATES.with(|s| {
                    s.borrow_mut().insert(Slot {
                        value: Box::new(initial()),
                        type_id: TypeId::of::<T>(),
                        owner,
                    })
                });
                frame.hooks.push(key);
                key
            }
        });
        Self { key, _t: PhantomData }
    }

    /// Lit la valeur sans la copier.
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        STATES.with(|s| {
            let s = s.borrow();
            let slot = s
                .get(self.key)
                .expect("Veka : cet état appartient à une vue qui n'existe plus");
            f(slot.value.downcast_ref::<T>().expect("type d'état"))
        })
    }

    /// Modifie la valeur ; la vue propriétaire sera ré-évaluée avant la prochaine image.
    pub fn update<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        let (r, owner) = STATES.with(|s| {
            let mut s = s.borrow_mut();
            let slot = s
                .get_mut(self.key)
                .expect("Veka : cet état appartient à une vue qui n'existe plus");
            let r = f(slot.value.downcast_mut::<T>().expect("type d'état"));
            (r, slot.owner)
        });
        mark_dirty(owner);
        r
    }

    /// Modifie la valeur sans ré-évaluer la vue (ce qui n'est pas affiché : un rappel…).
    pub(crate) fn update_silent<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        STATES.with(|s| {
            let mut s = s.borrow_mut();
            let slot = s
                .get_mut(self.key)
                .expect("Veka : cet état appartient à une vue qui n'existe plus");
            f(slot.value.downcast_mut::<T>().expect("type d'état"))
        })
    }

    pub fn set(&self, value: T) {
        self.update(|v| *v = value);
    }

    /// Vrai tant que la vue propriétaire existe.
    pub fn is_alive(&self) -> bool {
        STATES.with(|s| s.borrow().contains_key(self.key))
    }

    /// La surface de la vue propriétaire, si elle existe encore.
    pub(crate) fn surface(&self) -> Option<u32> {
        STATES.with(|s| s.borrow().get(self.key).map(|slot| slot.owner.surface))
    }

    /// Lit la valeur si l'état existe encore.
    pub(crate) fn try_with<R>(&self, f: impl FnOnce(&T) -> R) -> Option<R> {
        STATES.with(|s| {
            s.borrow()
                .get(self.key)
                .and_then(|slot| slot.value.downcast_ref::<T>())
                .map(f)
        })
    }
}

impl<T> State<T> {
    /// La poignée sous forme d'entier, pour la passer à travers une interface C.
    #[doc(hidden)]
    pub fn to_bits(self) -> u64 {
        slotmap::Key::data(&self.key).as_ffi()
    }

    /// Retrouve une poignée créée par [`to_bits`](Self::to_bits). Une poignée périmée ou d'un autre
    /// type fait paniquer à l'usage, sans comportement indéfini.
    #[doc(hidden)]
    pub fn from_bits(bits: u64) -> Self {
        Self {
            key: slotmap::KeyData::from_ffi(bits).into(),
            _t: PhantomData,
        }
    }
}

impl<T: Clone + 'static> State<T> {
    pub fn get(&self) -> T {
        self.with(T::clone)
    }
}

impl<T: fmt::Display + 'static> fmt::Display for State<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.with(|v| v.fmt(f))
    }
}

impl<T: fmt::Debug + 'static> fmt::Debug for State<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_alive() {
            self.with(|v| f.debug_tuple("State").field(v).finish())
        } else {
            f.write_str("State(<supprimé>)")
        }
    }
}

impl<T: AddAssign<R> + 'static, R> AddAssign<R> for State<T> {
    fn add_assign(&mut self, rhs: R) {
        self.update(|v| *v += rhs);
    }
}

impl<T: SubAssign<R> + 'static, R> SubAssign<R> for State<T> {
    fn sub_assign(&mut self, rhs: R) {
        self.update(|v| *v -= rhs);
    }
}

impl<T: MulAssign<R> + 'static, R> MulAssign<R> for State<T> {
    fn mul_assign(&mut self, rhs: R) {
        self.update(|v| *v *= rhs);
    }
}

impl State<bool> {
    pub fn toggle(&self) {
        self.update(|v| *v = !*v);
    }
}
