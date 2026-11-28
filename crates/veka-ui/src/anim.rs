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
use std::collections::HashMap;

use crate::views::{
    AnimationPrim, MatchedGeometryPrim, OffsetPrim, RotationPrim, ScaleEffectPrim, TransitionPrim,
};

/// Ce qui s'anime sur un nœud.
#[derive(Default)]
pub(crate) struct NodeAnims {
    rect: Option<Animator>,
    values: Option<Animator>,
    /// Signature des valeurs animées (voir `AnimatableData`).
    values_sig: u64,
    transition: Option<TransitionAnim>,
    /// `matched_geometry_effect` : le rectangle d'où la vue part, ramené vers le sien.
    geometry: Option<Animator>,
    geometry_rect: Option<Rect>,
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
        self.rect.is_none() && self.values.is_none() && self.transition.is_none() && self.geometry.is_none()
    }

    /// Entre, sort ou vient d'ailleurs : dessiné au-dessus des frères.
    pub(crate) fn in_transition(&self) -> bool {
        self.transition.is_some() || self.geometry.is_some()
    }

    /// La transformation qui amène `rect` (la place de la vue) sur le rectangle animé de
    /// `matched_geometry_effect`.
    pub(crate) fn geometry_transform(&self, rect: Rect) -> Option<Transform2D> {
        let g = self.geometry_rect?;
        let sx = if rect.width() > 0.0 {
            g.width() / rect.width()
        } else {
            1.0
        };
        let sy = if rect.height() > 0.0 {
            g.height() / rect.height()
        } else {
            1.0
        };
        Some(
            Transform2D::translation(-rect.x(), -rect.y())
                .then(&Transform2D::scale(sx, sy))
                .then(&Transform2D::translation(g.x(), g.y())),
        )
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
    let mut cx = Walk {
        now,
        gone: Vec::new(),
        sources: HashMap::new(),
        deferred: Vec::new(),
    };
    collect_sources(tree, root, &mut cx.sources);
    walk(tree, root, spec, false, &mut cx);
    // Les vues liées qui ne sont pas réapparues sortent normalement.
    for (id, spec) in std::mem::take(&mut cx.deferred) {
        if !cx.sources.values().any(|s| s.1 == id && s.2) {
            start_removal(tree, id, spec, now, &mut cx.gone);
        }
    }
    cx.gone.sort();
    cx.gone.dedup();
    for id in cx.gone {
        tree.detach(id);
    }
}

struct Walk {
    now: f64,
    gone: Vec<NodeId>,
    /// `matched_geometry_effect` : clé → (rectangle affiché, racine du sous-arbre qui sort, utilisée).
    sources: HashMap<u64, (Rect, NodeId, bool)>,
    /// Les sorties des sources, décidées après le parcours.
    deferred: Vec<(NodeId, Option<Animation>)>,
}

fn matched_key(tree: &Tree, id: NodeId) -> Option<u64> {
    downcast::<MatchedGeometryPrim>(&tree.nodes[id].prim).map(|m| m.key)
}

/// Les vues à géométrie liée qui viennent de sortir de l'arbre (sortie pas encore lancée).
fn collect_sources(tree: &Tree, id: NodeId, out: &mut HashMap<u64, (Rect, NodeId, bool)>) {
    fn inside(tree: &Tree, id: NodeId, root: NodeId, out: &mut HashMap<u64, (Rect, NodeId, bool)>) {
        if let Some(k) = matched_key(tree, id) {
            out.insert(k, (tree.nodes[id].rect, root, false));
        }
        for &c in &tree.nodes[id].children {
            inside(tree, c, root, out);
        }
    }
    for &c in &tree.nodes[id].children {
        let node = &tree.nodes[c];
        if node.leaving {
            let started = tree
                .anims
                .get(&c)
                .and_then(|a| a.transition.as_ref())
                .is_some_and(|t| t.removal);
            if !started {
                inside(tree, c, c, out);
            }
        } else if node.last_layout.is_some() {
            collect_sources(tree, c, out);
        }
    }
}

fn walk(tree: &mut Tree, id: NodeId, mut spec: Option<Animation>, fresh: bool, cx: &mut Walk) {
    let now = cx.now;
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
        let target = {
            let node = &mut tree.nodes[id];
            node.last_layout = Some(node.layout_rect);
            node.rect = node.layout_rect;
            node.prev = None;
            node.shown = None;
            node.layout_rect
        };
        let reduce = tree.nodes[id].env.reduces_motion();
        let mut matched = false;
        if let Some(s) = spec.filter(|_| !reduce)
            && let Some(k) = matched_key(tree, id)
            && let Some(src) = cx.sources.get_mut(&k)
            && !src.2
        {
            // La vue part de la place de celle qui vient de disparaître ; celle-ci est retirée.
            src.2 = true;
            matched = true;
            let a = tree.anims.entry(id).or_default();
            a.geometry = Some(Animator::new(s, now, rect_data(src.0), rect_data(target), None));
            a.geometry_rect = Some(src.0);
            cx.gone.push(src.1);
        }
        // Seule la racine d'un sous-arbre inséré joue la transition d'entrée.
        if !fresh
            && !matched
            && tree.nodes[id].parent.is_some()
            && let Some(s) = spec
        {
            let effects = reduced(transition_of(&tree.nodes[id].prim).insertion, reduce);
            if !effects.is_empty() {
                tree.anims.entry(id).or_default().transition = Some(TransitionAnim {
                    animator: Animator::new(s, now, vec![0.0], vec![1.0], None),
                    effects,
                    removal: false,
                    progress: 0.0,
                });
            }
            // Le parent se déplace encore (une liste qui se recentre…) : la vue insérée part avec
            // lui, au lieu d'apparaître d'emblée à sa place finale, par-dessus ses voisins.
            let parent = &tree.nodes[tree.nodes[id].parent.expect("vérifié")];
            let (dx, dy) = (
                parent.rect.x() - parent.layout_rect.x(),
                parent.rect.y() - parent.layout_rect.y(),
            );
            if !reduce && (dx != 0.0 || dy != 0.0) {
                let from = Rect::new(target.x() + dx, target.y() + dy, target.width(), target.height());
                let a = tree.anims.entry(id).or_default();
                a.geometry = Some(Animator::new(s, now, rect_data(from), rect_data(target), None));
                a.geometry_rect = Some(from);
            }
        }
        let mut i = 0;
        while let Some(&c) = tree.nodes[id].children.get(i) {
            walk(tree, c, spec, true, cx);
            i += 1;
        }
        return;
    }

    // « Réduire les animations » : rien ne se déplace ni ne change de taille ; couleurs et opacité
    // s'animent toujours.
    let reduce = tree.nodes[id].env.reduces_motion();
    let moves = {
        let p = &tree.nodes[id].prim;
        downcast::<OffsetPrim>(p).is_some()
            || downcast::<ScaleEffectPrim>(p).is_some()
            || downcast::<RotationPrim>(p).is_some()
    };
    animate_values(tree, id, spec.filter(|_| !(reduce && moves)), now);
    animate_rect(tree, id, spec.filter(|_| !reduce), now);

    // Par indice, sans copier la liste : le parcours ne change pas les enfants (les sorties sont
    // détachées après).
    let mut i = 0;
    while let Some(&c) = tree.nodes[id].children.get(i) {
        i += 1;
        if tree.nodes[c].leaving {
            let started = tree
                .anims
                .get(&c)
                .and_then(|a| a.transition.as_ref())
                .is_some_and(|t| t.removal);
            if !started {
                if cx.sources.values().any(|s| s.1 == c) {
                    cx.deferred.push((c, spec));
                } else {
                    start_removal(tree, c, spec, now, &mut cx.gone);
                }
            }
            continue;
        }
        walk(tree, c, spec, false, cx);
    }
}

fn start_removal(tree: &mut Tree, id: NodeId, spec: Option<Animation>, now: f64, gone: &mut Vec<NodeId>) {
    let reduce = tree.nodes[id].env.reduces_motion();
    let effects = reduced(transition_of(&tree.nodes[id].prim).removal, reduce);
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

/// Sous « Réduire les animations », une transition ne garde que son fondu.
fn reduced(effects: Vec<TransitionEffect>, reduce: bool) -> Vec<TransitionEffect> {
    if !reduce {
        return effects;
    }
    effects
        .into_iter()
        .filter(|e| matches!(e, TransitionEffect::Opacity | TransitionEffect::Blur(_)))
        .collect()
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
    // Sans animation demandée ni en cours, la valeur affichée est la cible : rien à comparer (une vue
    // `.vk` réévaluée à chaque image passe ici pour chaque nœud).
    if spec.is_none() && tree.anims.get(&id).is_none_or(|a| a.values.is_none()) {
        tree.nodes[id].shown = None;
        return;
    }
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
        if let Some(g) = &a.geometry {
            let s = g.sample(now);
            if s.done {
                a.geometry = None;
                a.geometry_rect = None;
            } else {
                a.geometry_rect = Some(rect_from(&s.value));
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
