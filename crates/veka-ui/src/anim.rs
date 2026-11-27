//! Anima dans l'arbre : animations de layout, de valeurs et transitions d'entrée / de sortie.
//!
//! Après chaque layout, [`after_layout`] compare l'arbre à ce qu'il affichait : sous une animation
//! (`with_animation` ou `.animation(spec, valeur)`), les rectangles et les valeurs animables qui ont
//! changé partent de ce qui est affiché vers leur nouvelle cible, les nœuds insérés entrent et les
//! nœuds retirés sortent avec leur transition. [`tick`] fait avancer le tout à chaque image.

use std::any::Any;
use std::rc::Rc;

use veka_anima::{AnimatableData, Animation, Animator, Transition, TransitionEffect, TransitionFrame};
use veka_foundation::{Rect, Transform2D};
use veka_graphics::{DrawCommand, DrawList};

use crate::tree::{NodeId, Tree};
use crate::views::{AnimationPrim, TransitionPrim};

/// Ce qui s'anime sur un nœud.
#[derive(Default)]
pub(crate) struct NodeAnims {
    rect: Option<Animator>,
    values: Option<Animator>,
    /// Signature des valeurs animées (voir `AnimatableData`).
    values_sig: u64,
    transition: Option<TransitionAnim>,
}

struct TransitionAnim {
    animator: Animator,
    effects: Vec<TransitionEffect>,
    removal: bool,
    /// 1 = vue normale, 0 = absente.
    progress: f32,
}

impl NodeAnims {
    fn is_empty(&self) -> bool {
        self.rect.is_none() && self.values.is_none() && self.transition.is_none()
    }

    pub(crate) fn in_transition(&self) -> bool {
        self.transition.is_some()
    }

    /// L'état de la transition en cours, pour une vue affichée dans `rect`.
    pub(crate) fn transition_frame(&self, rect: Rect) -> Option<TransitionFrame> {
        let t = self.transition.as_ref()?;
        Some(TransitionFrame::evaluate(
            &t.effects,
            t.progress,
            (rect.width(), rect.height()),
        ))
    }
}

fn rect_data(r: Rect) -> Vec<f32> {
    vec![r.x(), r.y(), r.width(), r.height()]
}

fn rect_from(v: &[f32]) -> Rect {
    Rect::new(v[0], v[1], v[2].max(0.0), v[3].max(0.0))
}

fn downcast<P: 'static>(p: &Rc<dyn crate::Primitive>) -> Option<&P> {
    (p.as_ref() as &dyn Any).downcast_ref::<P>()
}

/// La vitesse d'une animation en cours, si la nouvelle est un ressort (il la garde).
fn carried_velocity(running: Option<&Animator>, spec: &Animation, now: f64) -> Option<Vec<f32>> {
    if !spec.is_spring() {
        return None;
    }
    running.map(|a| a.sample(now).velocity)
}

/// Après un layout : lance les animations de ce qui a changé. `spec` : la transaction de l'image
/// (`with_animation`), s'il y en a une.
pub(crate) fn after_layout(tree: &mut Tree, root: NodeId, spec: Option<Animation>, now: f64) {
    let mut gone = Vec::new();
    walk(tree, root, spec, false, now, &mut gone);
    for id in gone {
        tree.detach(id);
    }
}

fn walk(
    tree: &mut Tree,
    id: NodeId,
    mut spec: Option<Animation>,
    fresh: bool,
    now: f64,
    gone: &mut Vec<NodeId>,
) {
    // `.animation(spec, valeur)` : si la valeur a changé, ce sous-arbre prend sa spec.
    {
        let node = &tree.nodes[id];
        if let Some(a) = downcast::<AnimationPrim>(&node.prim)
            && let Some(prev) = node.prev.as_ref().and_then(downcast::<AnimationPrim>)
            && prev.value != a.value
        {
            spec = a.spec;
        }
    }

    let inserted = tree.nodes[id].last_layout.is_none();
    if fresh || inserted {
        let node = &mut tree.nodes[id];
        node.last_layout = Some(node.layout_rect);
        node.rect = node.layout_rect;
        node.prev = None;
        node.shown = None;
        // Seule la racine d'un sous-arbre inséré joue la transition d'entrée.
        if !fresh
            && node.parent.is_some()
            && let Some(s) = spec
        {
            let effects = transition_of(&node.prim).insertion;
            if !effects.is_empty() {
                tree.anims.entry(id).or_default().transition = Some(TransitionAnim {
                    animator: Animator::new(s, now, vec![0.0], vec![1.0], None),
                    effects,
                    removal: false,
                    progress: 0.0,
                });
            }
        }
        for c in tree.nodes[id].children.clone() {
            walk(tree, c, spec, true, now, gone);
        }
        return;
    }

    animate_values(tree, id, spec, now);
    animate_rect(tree, id, spec, now);

    for c in tree.nodes[id].children.clone() {
        if tree.nodes[c].leaving {
            let started = tree
                .anims
                .get(&c)
                .and_then(|a| a.transition.as_ref())
                .is_some_and(|t| t.removal);
            if !started {
                start_removal(tree, c, spec, now, gone);
            }
            continue;
        }
        walk(tree, c, spec, false, now, gone);
    }
}

fn start_removal(tree: &mut Tree, id: NodeId, spec: Option<Animation>, now: f64, gone: &mut Vec<NodeId>) {
    let effects = transition_of(&tree.nodes[id].prim).removal;
    let Some(s) = spec.filter(|_| !effects.is_empty()) else {
        gone.push(id);
        return;
    };
    let a = tree.anims.entry(id).or_default();
    // Une vue qui sort pendant son entrée repart d'où elle en est.
    let from = a.transition.as_ref().map_or(1.0, |t| t.progress);
    a.transition = Some(TransitionAnim {
        animator: Animator::new(s, now, vec![from], vec![0.0], None),
        effects,
        removal: true,
        progress: from,
    });
}

fn transition_of(prim: &Rc<dyn crate::Primitive>) -> Transition {
    downcast::<TransitionPrim>(prim)
        .map(|t| t.transition.clone())
        .unwrap_or_default()
}

fn animate_values(tree: &mut Tree, id: NodeId, spec: Option<Animation>, now: f64) {
    let Some(prev) = tree.nodes[id].prev.take() else {
        return;
    };
    let node = &tree.nodes[id];
    let env = node.env.clone();
    let target = node.prim.clone();
    let to = target.animatable(&env);
    let running = tree
        .anims
        .get(&id)
        .and_then(|a| a.values.as_ref().map(|v| (v.clone(), a.values_sig)));
    // Même cible qu'en ce moment : l'animation continue.
    if let (Some((r, sig)), Some(t)) = (&running, &to)
        && r.to == t.values
        && *sig == t.signature
    {
        return;
    }
    let from = match &running {
        Some((r, sig)) => Some(AnimatableData::with_values(r.sample(now).value, *sig)),
        None => prev.animatable(&env),
    };
    match (spec, from, to) {
        (Some(s), Some(f), Some(t)) if f.compatible(&t) && f != t => {
            let velocity = carried_velocity(running.as_ref().map(|(r, _)| r), &s, now);
            let shown = target.with_animatable(&env, &f);
            let a = tree.anims.entry(id).or_default();
            a.values = Some(Animator::new(s, now, f.values, t.values, velocity));
            a.values_sig = t.signature;
            tree.nodes[id].shown = shown;
        }
        _ => {
            if let Some(a) = tree.anims.get_mut(&id) {
                a.values = None;
            }
            tree.nodes[id].shown = None;
        }
    }
}

fn animate_rect(tree: &mut Tree, id: NodeId, spec: Option<Animation>, now: f64) {
    let node = &mut tree.nodes[id];
    let target = node.layout_rect;
    let last = node.last_layout.unwrap_or(target);
    node.last_layout = Some(target);
    let running = tree.anims.get(&id).and_then(|a| a.rect.as_ref());
    if target == last {
        if running.is_none() {
            tree.nodes[id].rect = target;
        }
        return;
    }
    match spec {
        Some(s) => {
            let from = rect_data(tree.nodes[id].rect);
            if from == rect_data(target) {
                if let Some(a) = tree.anims.get_mut(&id) {
                    a.rect = None;
                }
                return;
            }
            let velocity = carried_velocity(running, &s, now);
            tree.anims.entry(id).or_default().rect =
                Some(Animator::new(s, now, from, rect_data(target), velocity));
        }
        None => {
            if let Some(a) = tree.anims.get_mut(&id) {
                a.rect = None;
            }
            tree.nodes[id].rect = target;
        }
    }
}

/// Ce que l'image doit refaire après [`tick`].
#[derive(Default)]
pub(crate) struct Ticked {
    pub paint: bool,
    /// Une valeur animée change l'environnement (couleur du texte…) : refaire le layout.
    pub layout: bool,
}

/// Fait avancer les animations jusqu'à `now`.
pub(crate) fn tick(tree: &mut Tree, now: f64) -> Ticked {
    let mut out = Ticked::default();
    if tree.anims.is_empty() {
        return out;
    }
    out.paint = true;
    let ids: Vec<NodeId> = tree.anims.keys().copied().collect();
    let mut gone = Vec::new();
    for id in ids {
        if !tree.nodes.contains_key(id) {
            tree.anims.remove(&id);
            continue;
        }
        let a = tree.anims.get_mut(&id).expect("présent");
        if let Some(r) = &a.rect {
            let s = r.sample(now);
            let node = &mut tree.nodes[id];
            if s.done {
                node.rect = node.layout_rect;
                a.rect = None;
            } else {
                node.rect = rect_from(&s.value);
            }
        }
        if let Some(v) = &a.values {
            let s = v.sample(now);
            let node = &mut tree.nodes[id];
            if s.done {
                node.shown = None;
                a.values = None;
            } else {
                let data = AnimatableData::with_values(s.value, a.values_sig);
                node.shown = node.prim.with_animatable(&node.env, &data);
            }
            let mut env = (*node.env).clone();
            if node.prim.update_env(&mut env) {
                out.layout = true;
            }
        }
        if let Some(t) = &mut a.transition {
            let s = t.animator.sample(now);
            t.progress = s.value[0];
            if s.done {
                if t.removal {
                    gone.push(id);
                }
                a.transition = None;
            }
        }
        if a.is_empty() {
            tree.anims.remove(&id);
        }
    }
    for id in gone {
        tree.detach(id);
        out.layout = true;
    }
    out
}

/// Opacité, décalage et échelle d'une transition autour de `rect`. Renvoie le nombre de commandes à
/// refermer avec [`pop`].
pub(crate) fn push_frame(list: &mut DrawList, f: &TransitionFrame, rect: Rect) -> u8 {
    let mut pops = 0;
    if f.opacity < 1.0 {
        list.push(DrawCommand::PushLayer { opacity: f.opacity });
        pops |= 1;
    }
    if f.scale != 1.0 || f.offset != (0.0, 0.0) {
        let c = rect.center();
        let t = Transform2D::translation(-c.x, -c.y)
            .then(&Transform2D::scale(f.scale, f.scale))
            .then(&Transform2D::translation(c.x + f.offset.0, c.y + f.offset.1));
        list.push(DrawCommand::PushTransform(t));
        pops |= 2;
    }
    pops
}

pub(crate) fn pop(list: &mut DrawList, pops: u8) {
    if pops & 2 != 0 {
        list.push(DrawCommand::PopTransform);
    }
    if pops & 1 != 0 {
        list.push(DrawCommand::PopLayer);
    }
}
