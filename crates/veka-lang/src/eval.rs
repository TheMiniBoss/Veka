//! L'interpréteur : une vue `.vk` est une vue composée de Veka dont le corps parcourt l'arbre du
//! fichier. Les vues sont construites par le schéma, les actions deviennent des `Action`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::rc::Rc;
use std::sync::Arc;

use veka_colora::{Paint, Theme, Token};
use veka_ui::schema::{Children, EntryKind, Params, Schema, Value, ValueType};
use veka_ui::{Action, Element, Key, State, TimelineState, with_animation};

pub(crate) use crate::ast::*;
use crate::value::{Sym, Val, gradient, to_animation, to_paint, to_schema};
use crate::{Diagnostic, Span};

pub(crate) type Vars = HashMap<String, Val, BuildHasherDefault<FxHasher>>;

/// Des arguments évalués : sur la pile jusqu'à 4.
type ArgVals = smallvec::SmallVec<[(Option<String>, Val); 4]>;

type R<T> = Result<T, Diagnostic>;

/// Les derniers résultats d'une fonction pure, à un site d'appel : par arguments exacts. Petit et borné :
/// au-delà, on repart d'une table vide.
#[derive(Default)]
pub(crate) struct Memo {
    map: HashMap<[ArgKey; MEMO_ARGS], Val, BuildHasherDefault<FxHasher>>,
}

const MEMO_MAX: usize = 4096;
/// Au-delà de 4 arguments, pas de mémoire (la clé reste sur la pile).
const MEMO_ARGS: usize = 4;

impl Memo {
    fn key(args: &[(Option<String>, Val)]) -> Option<[ArgKey; MEMO_ARGS]> {
        if args.len() > MEMO_ARGS {
            return None;
        }
        let mut key = [const { ArgKey::Unit }; MEMO_ARGS];
        for (k, (_, v)) in key.iter_mut().zip(args) {
            *k = ArgKey::of(v)?;
        }
        Some(key)
    }
}

/// Un argument comparable bit à bit (les flottants par leurs bits : `0.1 + 0.2` n'est pas `0.3`).
#[derive(Clone, PartialEq, Eq, Hash)]
enum ArgKey {
    Unit,
    Bool(bool),
    Int(i64),
    Float(u64),
    Str(Rc<str>),
}

impl ArgKey {
    fn of(v: &Val) -> Option<ArgKey> {
        Some(match v {
            Val::Unit => ArgKey::Unit,
            Val::Bool(b) => ArgKey::Bool(*b),
            Val::Int(i) => ArgKey::Int(*i),
            Val::Float(f) => ArgKey::Float(f.to_bits()),
            Val::Str(s) => ArgKey::Str(s.clone()),
            _ => return None,
        })
    }
}

/// Ce qu'un nom appelé désigne (gardé par site d'appel).
#[derive(Clone, Copy, Debug)]
pub(crate) enum Callee {
    /// Une fonction du fichier ; `pure` : elle peut être mémorisée.
    Func { pure: bool },
    /// Une fonction de base (ou une erreur, signalée à chaque appel).
    Other,
}

/// Une expression constante : littéraux, symboles, et ce qui ne combine qu'eux (opérations, fonctions
/// pures du fichier, fonctions de base). Aucun nom n'y est lu : une variable peut tout masquer.
fn is_const(module: &Module, e: &Expr) -> bool {
    let k = |e: &Expr| is_const(module, e);
    match &e.kind {
        ExprKind::Bool(_)
        | ExprKind::Int(_)
        | ExprKind::Float(_)
        | ExprKind::Color(_)
        | ExprKind::Symbol(_) => true,
        ExprKind::Ident(_) | ExprKind::Binding(_) => false,
        ExprKind::Str(parts) => parts.iter().all(|p| match p {
            StrPart::Text(_) => true,
            StrPart::Expr(e) => k(e),
        }),
        ExprKind::Array(items) => items.iter().all(k),
        ExprKind::Record(fields) => fields.iter().all(|(_, e)| k(e)),
        ExprKind::Index { recv, index } => k(recv) && k(index),
        ExprKind::Range { lo, hi, .. } => k(lo) && k(hi),
        ExprKind::Unary { expr, .. } => k(expr),
        ExprKind::Binary { lhs, rhs, .. } => k(lhs) && k(rhs),
        ExprKind::Ternary { cond, then, els } => k(cond) && k(then) && k(els),
        ExprKind::Member { recv, .. } => k(recv),
        ExprKind::Call {
            callee,
            args,
            trailing,
        } => {
            trailing.is_none()
                && args.iter().all(|a| k(&a.value))
                && match &callee.kind {
                    ExprKind::Symbol(_) => true,
                    ExprKind::Member { recv, .. } => k(recv),
                    ExprKind::Ident(n) => match module.func(n) {
                        Some(f) => func_is_pure(module, f, 0),
                        None => module.timeline(n).is_none() && n != "model",
                    },
                    _ => false,
                }
        }
    }
}

/// Une fonction est pure si ni elle ni celles qu'elle appelle ne lisent `model` (les données de
/// l'hôte, qui changent). Le reste de ce qu'elle voit (paramètres, `let`, thèmes, `pi`) est fixe.
fn func_is_pure(module: &Module, f: &FuncDecl, depth: u32) -> bool {
    fn expr(module: &Module, e: &Expr, depth: u32) -> bool {
        let all = |es: &mut dyn Iterator<Item = &Expr>| {
            let mut ok = true;
            for e in es {
                ok &= expr(module, e, depth);
            }
            ok
        };
        match &e.kind {
            ExprKind::Ident(n) => n != "model",
            // Une fonction ne reçoit pas d'état de vue.
            ExprKind::Binding(_) => false,
            ExprKind::Bool(_)
            | ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::Color(_)
            | ExprKind::Symbol(_) => true,
            ExprKind::Str(parts) => parts.iter().all(|p| match p {
                StrPart::Text(_) => true,
                StrPart::Expr(e) => expr(module, e, depth),
            }),
            ExprKind::Array(items) => all(&mut items.iter()),
            ExprKind::Record(fields) => all(&mut fields.iter().map(|(_, e)| e)),
            ExprKind::Index { recv, index } => expr(module, recv, depth) && expr(module, index, depth),
            ExprKind::Range { lo, hi, .. } => expr(module, lo, depth) && expr(module, hi, depth),
            ExprKind::Unary { expr: x, .. } => expr(module, x, depth),
            ExprKind::Binary { lhs, rhs, .. } => expr(module, lhs, depth) && expr(module, rhs, depth),
            ExprKind::Ternary { cond, then, els } => {
                expr(module, cond, depth) && expr(module, then, depth) && expr(module, els, depth)
            }
            ExprKind::Member { recv, .. } => expr(module, recv, depth),
            ExprKind::Call {
                callee,
                args,
                trailing,
            } => {
                if trailing.is_some() {
                    return false;
                }
                let callee_ok = match &callee.kind {
                    ExprKind::Ident(n) => match module.func(n) {
                        Some(g) => func_is_pure(module, g, depth + 1),
                        None => n != "model",
                    },
                    _ => expr(module, callee, depth),
                };
                callee_ok && all(&mut args.iter().map(|a| &a.value))
            }
        }
    }
    // Une récursion profonde : on ne mémorise pas (la vraie erreur viendra à l'exécution).
    if depth > 16 {
        return false;
    }
    f.body.iter().all(|st| match st {
        Stmt::Let { value, .. } | Stmt::Expr(value) => expr(module, value, depth),
        _ => false,
    })
}

/// Un hachage rapide (celui de rustc) : les clés sont petites et ne viennent pas de l'extérieur.
#[derive(Default)]
pub(crate) struct FxHasher(u64);

impl std::hash::Hasher for FxHasher {
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for c in &mut chunks {
            self.add(u64::from_le_bytes(c.try_into().expect("8 octets")));
        }
        for &b in chunks.remainder() {
            self.add(u64::from(b));
        }
    }
    fn write_u8(&mut self, i: u8) {
        self.add(u64::from(i));
    }
    fn write_u64(&mut self, i: u64) {
        self.add(i);
    }
    fn write_i64(&mut self, i: i64) {
        self.add(i as u64);
    }
    fn write_usize(&mut self, i: usize) {
        self.add(i as u64);
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

impl FxHasher {
    fn add(&mut self, i: u64) {
        self.0 = (self.0.rotate_left(5) ^ i).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

/// Ce que l'évaluation partage : le fichier, le schéma, les thèmes construits.
pub(crate) struct Shared {
    pub module: Rc<Module>,
    pub schema: Rc<Schema>,
    pub themes: HashMap<String, Arc<Theme>>,
}

/// Un rappel d'événement (`.on(acheter) { prix in … }`).
pub(crate) type Handler = Rc<dyn Fn(Val)>;

/// Ce que l'hôte partage avec toutes les vues : ses données et son rappel d'événements.
#[derive(Default)]
pub(crate) struct Host {
    pub model: RefCell<Val>,
    #[allow(clippy::type_complexity)]
    pub on_event: RefCell<Option<Rc<dyn Fn(&str, &str, &Val)>>>,
}

/// Le contenu d'un `@Slot`, évalué chez l'appelant.
#[derive(Clone)]
pub(crate) enum SlotVal {
    Body(Box<Cx>, Rc<Vec<Stmt>>),
    Expr(Box<Cx>, Rc<Expr>),
}

/// Ce que l'appelant donne à une vue du fichier.
#[derive(Clone, Default)]
pub(crate) struct Instance {
    pub params: Vars,
    pub slots: HashMap<String, SlotVal>,
    pub handlers: HashMap<String, Handler>,
}

/// Construit une vue du fichier.
pub(crate) type ViewFn = Rc<dyn Fn(&str, Instance) -> Element>;

/// Les rappels `.onTimelineEvent` relevés pendant une évaluation : `(timeline, événement, rappel)`.
pub(crate) type TimelineHandlers = Rc<RefCell<Vec<(TimelineState, Option<String>, Handler)>>>;

/// Le contexte d'évaluation d'une vue (ou d'une action).
#[derive(Clone)]
pub(crate) struct Cx {
    pub shared: Rc<Shared>,
    /// Les états de la vue (absents pendant le calcul de leurs valeurs initiales).
    pub vars: Option<State<Vars>>,
    /// Les états déjà calculés pendant l'initialisation.
    pub pre: Rc<Vars>,
    /// `let`, paramètres de `ForEach` et de fermetures, timelines et horloges.
    pub locals: Vec<(Rc<str>, Val)>,
    /// Les vues du fichier, pour en construire depuis un corps.
    pub views: ViewFn,
    pub host: Rc<Host>,
    /// La vue évaluée (pour `emit`).
    pub view: Rc<str>,
    pub params: Rc<Vars>,
    pub slots: Rc<HashMap<String, SlotVal>>,
    pub handlers: Rc<HashMap<String, Handler>>,
    pub tl_handlers: TimelineHandlers,
    /// Profondeur des appels de fonctions (contre la récursion sans fin).
    pub depth: u32,
}

impl Cx {
    /// Un contexte sans vue (thèmes, valeurs initiales).
    pub fn root(shared: Rc<Shared>, host: Rc<Host>, views: ViewFn) -> Self {
        Self::bare(shared, host, views, Rc::new(RefCell::new(Vec::new())))
    }

    /// Comme [`root`](Self::root), avec la liste des rappels de timeline donnée.
    fn bare(shared: Rc<Shared>, host: Rc<Host>, views: ViewFn, tl_handlers: TimelineHandlers) -> Self {
        // Les parties vides sont partagées : un appel de fonction (souvent, à chaque image) n'alloue pas.
        struct Empty {
            vars: Rc<Vars>,
            name: Rc<str>,
            slots: Rc<HashMap<String, SlotVal>>,
            handlers: Rc<HashMap<String, Handler>>,
        }
        thread_local! {
            static EMPTY: Empty = Empty {
                vars: Rc::new(Vars::default()),
                name: Rc::from(""),
                slots: Rc::new(HashMap::new()),
                handlers: Rc::new(HashMap::new()),
            };
        }
        let (vars, name, slots, handlers) = EMPTY.with(|e| {
            (
                e.vars.clone(),
                e.name.clone(),
                e.slots.clone(),
                e.handlers.clone(),
            )
        });
        Cx {
            shared,
            vars: None,
            pre: vars.clone(),
            locals: Vec::new(),
            views,
            host,
            view: name,
            params: vars,
            slots,
            handlers,
            tl_handlers,
            depth: 0,
        }
    }
}

fn err(span: Span, msg: impl Into<String>) -> Diagnostic {
    Diagnostic::error(span, msg)
}

/// La distance d'édition (pour « vouliez-vous dire »).
pub(crate) fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(!ca.eq_ignore_ascii_case(cb));
            cur.push(sub.min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

pub(crate) fn suggest<'a>(name: &str, candidates: impl Iterator<Item = &'a str>) -> String {
    let best = candidates
        .map(|c| (distance(name, c), c))
        .filter(|(d, c)| *d <= 2.max(c.len() / 3))
        .min_by_key(|(d, _)| *d);
    match best {
        Some((_, c)) => format!(" ; vouliez-vous dire `{c}` ?"),
        None => String::new(),
    }
}

impl Cx {
    /// Comme [`lookup`](Self::lookup), en retenant dans `back` la place d'une locale : au même endroit
    /// du fichier, elle est à la même distance du sommet de la pile (vérifié par son nom).
    fn lookup_at(&self, name: &str, back: &std::cell::Cell<u32>) -> Option<Val> {
        let b = back.get() as usize;
        if b > 0
            && let Some(i) = self.locals.len().checked_sub(b)
            && *self.locals[i].0 == *name
        {
            return Some(self.locals[i].1.clone());
        }
        if let Some(i) = self.locals.iter().rposition(|(n, _)| &**n == name) {
            back.set((self.locals.len() - i) as u32);
            return Some(self.locals[i].1.clone());
        }
        self.lookup_outer(name)
    }

    fn lookup(&self, name: &str) -> Option<Val> {
        if let Some((_, v)) = self.locals.iter().rev().find(|(n, _)| &**n == name) {
            return Some(v.clone());
        }
        self.lookup_outer(name)
    }

    /// Ce qui n'est pas une locale : états, paramètres, thèmes, `model`, `pi`.
    fn lookup_outer(&self, name: &str) -> Option<Val> {
        if let Some(v) = self.pre.get(name) {
            return Some(v.clone());
        }
        if let Some(v) = self.params.get(name) {
            return Some(v.clone());
        }
        if let Some(v) = self.vars.and_then(|s| s.with(|m| m.get(name).cloned())) {
            return Some(v);
        }
        if let Some(t) = self.shared.themes.get(name) {
            return Some(Val::Theme(t.clone()));
        }
        match name {
            "model" => Some(self.host.model.borrow().clone()),
            "pi" => Some(Val::Float(std::f64::consts::PI)),
            _ => None,
        }
    }

    /// `$nom` : le `@State` lu et écrit par une vue, en valeurs simples du schéma.
    fn binding(&self, name: &str, span: Span) -> R<Val> {
        if !self.is_state(name) {
            return Err(err(
                span,
                format!("`${name}` : `{name}` n'est pas un `@State` de cette vue"),
            ));
        }
        let vars = self.vars.expect("is_state");
        let (n1, n2): (Rc<str>, Rc<str>) = (Rc::from(name), Rc::from(name));
        let get = move || {
            if !vars.is_alive() {
                return veka_ui::schema::Value::Bool(false);
            }
            vars.with(|m| m.get(&*n1).map(crate::value::to_simple))
                .unwrap_or(veka_ui::schema::Value::Bool(false))
        };
        let set = move |v: veka_ui::schema::Value| {
            if !vars.is_alive() {
                return;
            }
            let Some(old) = vars.with(|m| m.get(&*n2).cloned()) else {
                return;
            };
            let new = crate::value::from_simple(&v, &old);
            if let Some(new) = new.filter(|n| *n != old) {
                vars.update(|m| {
                    m.insert(n2.to_string(), new);
                });
            }
        };
        Ok(Val::Binding(veka_ui::schema::Binding {
            get: Rc::new(get),
            set: Rc::new(set),
        }))
    }

    fn is_state(&self, name: &str) -> bool {
        self.vars.is_some_and(|s| s.with(|m| m.contains_key(name)))
            && !self.locals.iter().any(|(n, _)| &**n == name)
    }

    // --- Expressions ---------------------------------------------------------------------------

    pub fn eval(&self, e: &Expr) -> R<Val> {
        match e.cache.konst.get() {
            Some(Some(v)) => Ok(v.clone()),
            Some(None) => self.eval_raw(e),
            None => {
                // Première évaluation : une expression constante est calculée une fois pour toutes.
                let k = if is_const(&self.shared.module, e) {
                    self.eval_raw(e).ok()
                } else {
                    None
                };
                let _ = e.cache.konst.set(k.clone());
                match k {
                    Some(v) => Ok(v),
                    None => self.eval_raw(e),
                }
            }
        }
    }

    fn eval_raw(&self, e: &Expr) -> R<Val> {
        let span = e.span;
        Ok(match &e.kind {
            ExprKind::Bool(b) => Val::Bool(*b),
            ExprKind::Int(v) => Val::Int(*v),
            ExprKind::Float(v) => Val::Float(*v),
            ExprKind::Color(c) => Val::Color(*c),
            ExprKind::Str(parts) => {
                let mut s = String::new();
                for p in parts {
                    match p {
                        StrPart::Text(t) => s.push_str(t),
                        StrPart::Expr(e) => s.push_str(&self.eval(e)?.to_string()),
                    }
                }
                Val::Str(Rc::from(s))
            }
            ExprKind::Ident(n) => self.lookup_at(n, &e.cache.back).ok_or_else(|| {
                let mut names: Vec<String> = self.locals.iter().map(|(n, _)| n.to_string()).collect();
                names.extend(self.pre.keys().cloned());
                names.extend(self.params.keys().cloned());
                if let Some(s) = self.vars {
                    s.with(|m| names.extend(m.keys().cloned()));
                }
                err(
                    span,
                    format!("`{n}` inconnu{}", suggest(n, names.iter().map(String::as_str))),
                )
            })?,
            ExprKind::Symbol(n) => Val::Sym(Rc::new(Sym {
                name: n.clone(),
                args: None,
                chain: Vec::new(),
            })),
            ExprKind::Binding(n) => self.binding(n, span)?,
            ExprKind::Array(items) => {
                Val::Array(Rc::new(items.iter().map(|i| self.eval(i)).collect::<R<_>>()?))
            }
            ExprKind::Record(fields) => Val::Record(Rc::new(
                fields
                    .iter()
                    .map(|(n, e)| Ok((Rc::from(n.as_str()), self.eval(e)?)))
                    .collect::<R<_>>()?,
            )),
            ExprKind::Index { recv, index } => {
                let r = self.eval(recv)?;
                let i = self.eval(index)?;
                match (&r, &i) {
                    (Val::Array(_) | Val::Range(..), Val::Int(k)) => {
                        let len = r.items().map_or(0, |v| v.len());
                        usize::try_from(*k).ok().and_then(|k| r.index(k)).ok_or_else(|| {
                            err(index.span, format!("indice {k} hors du tableau ({len} éléments)"))
                        })?
                    }
                    (Val::Record(_), Val::Str(k)) => r
                        .get(k)
                        .ok_or_else(|| err(index.span, format!("champ `{k}` absent")))?,
                    _ => {
                        return Err(err(
                            span,
                            format!(
                                "`[…]` : un tableau et un entier attendus, trouvé {} et {}",
                                r.type_name(),
                                i.type_name()
                            ),
                        ));
                    }
                }
            }
            ExprKind::Range { lo, hi, inclusive } => {
                let int = |e: &Expr| -> R<i64> {
                    match self.eval(e)? {
                        Val::Int(v) => Ok(v),
                        v => Err(err(
                            e.span,
                            format!("une plage va d'un entier à un autre, trouvé {}", v.type_name()),
                        )),
                    }
                };
                let (lo, hi) = (int(lo)?, int(hi)?);
                Val::Range(lo, if *inclusive { hi + 1 } else { hi })
            }
            ExprKind::Unary { op, expr } => {
                let v = self.eval(expr)?;
                match (op, v) {
                    (UnaryOp::Neg, Val::Int(i)) => Val::Int(-i),
                    (UnaryOp::Neg, Val::Float(f)) => Val::Float(-f),
                    (UnaryOp::Not, Val::Bool(b)) => Val::Bool(!b),
                    (op, v) => {
                        return Err(err(
                            span,
                            format!(
                                "`{}` ne s'applique pas à {}",
                                if *op == UnaryOp::Neg { "-" } else { "!" },
                                v.type_name()
                            ),
                        ));
                    }
                }
            }
            ExprKind::Binary { op, lhs, rhs } => {
                // `&&` et `||` n'évaluent leur droite que si besoin.
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    let l = self.bool(lhs)?;
                    return Ok(Val::Bool(match op {
                        BinaryOp::And => l && self.bool(rhs)?,
                        _ => l || self.bool(rhs)?,
                    }));
                }
                let (l, r) = (self.eval(lhs)?, self.eval(rhs)?);
                binary(*op, &l, &r).map_err(|m| err(span, m))?
            }
            ExprKind::Ternary { cond, then, els } => {
                if self.bool(cond)? {
                    self.eval(then)?
                } else {
                    self.eval(els)?
                }
            }
            ExprKind::Member { recv, name } => {
                let r = self.eval(recv)?;
                member(&r, name, None).map_err(|m| err(span, m))?
            }
            ExprKind::Call {
                callee,
                args,
                trailing,
            } => {
                if trailing.is_some() {
                    return Err(err(span, "une fermeture `{ }` n'a pas de valeur ici"));
                }
                let mut vals = ArgVals::new();
                self.args_into(args, &mut vals)?;
                let args = vals;
                match &callee.kind {
                    ExprKind::Symbol(n) => Val::Sym(Rc::new(Sym {
                        name: n.clone(),
                        args: Some(args.into_vec()),
                        chain: Vec::new(),
                    })),
                    ExprKind::Member { recv, name } => {
                        let r = self.eval(recv)?;
                        member(&r, name, Some(args.into_vec())).map_err(|m| err(span, m))?
                    }
                    ExprKind::Ident(n) => {
                        let shared = self.shared.clone();
                        let module = &shared.module;
                        // Fonction du fichier ou de base : décidé une fois par site d'appel.
                        let callee = *e.cache.callee.get_or_init(|| match module.func(n) {
                            Some(f) => Callee::Func {
                                pure: func_is_pure(module, f, 0),
                            },
                            None => Callee::Other,
                        });
                        if let Callee::Func { pure } = callee {
                            // Pure : même arguments, même résultat (seulement des `let` et une valeur).
                            let key = Memo::key(&args).filter(|_| pure);
                            if let Some(key) = &key
                                && let Some(v) = e.cache.memo.borrow().map.get(key)
                            {
                                return Ok(v.clone());
                            }
                            let f = module.func(n).expect("trouvée au premier appel");
                            let v = self.call_func(f, args, span)?;
                            if let Some(key) = key {
                                let mut memo = e.cache.memo.borrow_mut();
                                if memo.map.len() >= MEMO_MAX {
                                    memo.map.clear();
                                }
                                memo.map.insert(key, v.clone());
                            }
                            return Ok(v);
                        }
                        if shared.module.timeline(n).is_some() {
                            return Err(err(
                                span,
                                format!("`{n}` est une timeline : `@Timeline var x = {n}(…)`"),
                            ));
                        }
                        builtin(n, &args).map_err(|m| {
                            if m.starts_with("fonction `") {
                                let funcs = shared.module.funcs().map(|f| f.name.as_str());
                                err(
                                    span,
                                    format!("{m}{}", suggest(n, funcs.chain(BUILTINS.iter().copied()))),
                                )
                            } else {
                                err(span, m)
                            }
                        })?
                    }
                    _ => return Err(err(span, "on ne peut appeler que des noms")),
                }
            }
        })
    }

    /// Une fonction du fichier : pure, elle ne voit que ses paramètres (et les autres fonctions).
    fn call_func(&self, f: &FuncDecl, args: ArgVals, span: Span) -> R<Val> {
        if self.depth > 64 {
            return Err(err(
                span,
                format!("`{}` : trop d'appels imbriqués (récursion sans fin ?)", f.name),
            ));
        }
        if args.len() != f.params.len() {
            return Err(err(
                span,
                format!(
                    "`{}` attend {} argument(s), {} donné(s)",
                    f.name,
                    f.params.len(),
                    args.len()
                ),
            ));
        }
        // Une fonction ne relève pas de rappels : elle partage la liste de l'appelant (sans allouer).
        let mut cx = Cx::bare(
            self.shared.clone(),
            self.host.clone(),
            self.views.clone(),
            self.tl_handlers.clone(),
        );
        cx.locals.reserve(f.params.len() + f.body.len());
        cx.depth = self.depth + 1;
        for ((name, ty), (label, v)) in f.params.iter().zip(args) {
            if label.as_ref().is_some_and(|l| **l != **name) {
                return Err(err(
                    span,
                    format!("`{}` : paramètre `{name}` attendu ici", f.name),
                ));
            }
            let v = check_type(ty, v, span)?;
            cx.locals.push((name.clone(), v));
        }
        let mut result = Val::Unit;
        for st in &f.body {
            match st {
                Stmt::Let { name, value, .. } => {
                    let v = cx.eval(value)?;
                    cx.locals.push((name.clone(), v));
                }
                Stmt::Expr(e) => result = cx.eval(e)?,
                Stmt::If { span, .. } | Stmt::Assign { span, .. } | Stmt::Emit { span, .. } => {
                    return Err(err(
                        *span,
                        "une fonction ne contient que des `let` et sa valeur (`a ? b : c` pour choisir)",
                    ));
                }
            }
        }
        check_type(&f.ret, result, span)
    }

    fn bool(&self, e: &Expr) -> R<bool> {
        match self.eval(e)? {
            Val::Bool(b) => Ok(b),
            v => Err(err(
                e.span,
                format!("un booléen attendu, trouvé {} ({v})", v.type_name()),
            )),
        }
    }

    /// Les arguments évalués, sur la pile jusqu'à 4 (la plupart des appels).
    fn args(&self, args: &[Arg]) -> R<ArgVals> {
        let mut out = ArgVals::new();
        self.args_into(args, &mut out)?;
        Ok(out)
    }

    /// Comme [`args`](Self::args), dans `out` (sans recopier la liste au retour).
    fn args_into(&self, args: &[Arg], out: &mut ArgVals) -> R<()> {
        for a in args {
            out.push((a.label.clone(), self.eval(&a.value)?));
        }
        Ok(())
    }

    // --- Vues ----------------------------------------------------------------------------------

    /// Un corps : les vues qu'il donne.
    pub fn body(&mut self, stmts: &[Stmt]) -> R<Vec<Element>> {
        let mut out = Vec::new();
        self.body_into(stmts, &mut out)?;
        Ok(out)
    }

    /// Comme [`body`](Self::body), en ajoutant à `out` (un `ForEach` remplit une seule liste).
    fn body_into(&mut self, stmts: &[Stmt], out: &mut Vec<Element>) -> R<()> {
        let mark = self.locals.len();
        for s in stmts {
            match s {
                Stmt::Expr(e) => {
                    if let ExprKind::Call {
                        callee,
                        args,
                        trailing,
                    } = &e.kind
                        && matches!(&callee.kind, ExprKind::Ident(n) if n == "ForEach")
                    {
                        self.for_each(e.span, args, trailing.as_ref(), out)?;
                    } else if let ExprKind::Call {
                        callee, trailing, ..
                    } = &e.kind
                        && matches!(&callee.kind, ExprKind::Ident(n) if n == "ScrollViewReader")
                    {
                        // `ScrollViewReader { proxy in … }` : le contenu, avec `proxy.scrollTo(…)`.
                        let body = trailing.as_ref().ok_or_else(|| {
                            err(e.span, "`ScrollViewReader { proxy in … }` : la fermeture manque")
                        })?;
                        let mark = self.locals.len();
                        if let Some(p) = body.params.first() {
                            self.locals.push((p.clone(), Val::Str(Rc::from(SCROLL_PROXY))));
                        }
                        let r = self.body_into(&body.body, out);
                        self.locals.truncate(mark);
                        r?;
                    } else if let Some(name) = slot_name(e) {
                        out.extend(self.slot(name, e.span)?);
                    } else {
                        out.push(self.element(e)?);
                    }
                }
                Stmt::If { cond, then, els, .. } => {
                    if self.bool(cond)? {
                        self.body_into(then, out)?;
                    } else if let Some(els) = els {
                        self.body_into(els, out)?;
                    }
                }
                Stmt::Let { name, value, .. } => {
                    let v = self.eval(value)?;
                    self.locals.push((name.clone(), v));
                }
                Stmt::Assign { span, .. } | Stmt::Emit { span, .. } => {
                    return Err(err(
                        *span,
                        "on n'agit pas en décrivant la vue : faites-le dans une action (`Button { … }`)",
                    ));
                }
            }
        }
        self.locals.truncate(mark);
        Ok(())
    }

    /// Le contenu de `Slot(nom)` (rien si l'appelant n'en a pas donné).
    fn slot(&self, name: &str, span: Span) -> R<Vec<Element>> {
        let declared = self
            .shared
            .module
            .view(&self.view)
            .is_some_and(|v| v.slots.iter().any(|s| s == name));
        if !declared {
            return Err(err(span, format!("`{name}` n'est pas un `@Slot` de cette vue")));
        }
        match self.slots.get(name) {
            None => Ok(Vec::new()),
            Some(SlotVal::Body(cx, body)) => cx.as_ref().clone().body(body),
            Some(SlotVal::Expr(cx, e)) => Ok(vec![cx.as_ref().clone().element(e)?]),
        }
    }

    fn for_each(
        &mut self,
        span: Span,
        args: &[Arg],
        trailing: Option<&Closure>,
        out: &mut Vec<Element>,
    ) -> R<()> {
        let body =
            trailing.ok_or_else(|| err(span, "`ForEach(…) { élément in … }` : la fermeture manque"))?;
        let coll = args
            .first()
            .ok_or_else(|| err(span, "`ForEach` : une collection attendue"))?;
        let items = self
            .eval(&coll.value)?
            .items()
            .ok_or_else(|| err(coll.value.span, "`ForEach` parcourt un tableau ou une plage"))?;
        for (i, v) in items.into_iter().enumerate() {
            let mark = self.locals.len();
            if let Some(p) = body.params.first() {
                self.locals.push((p.clone(), v));
            }
            if let Some(p) = body.params.get(1) {
                self.locals.push((p.clone(), Val::Int(i as i64)));
            }
            let r = self.body_into(&body.body, out);
            self.locals.truncate(mark);
            r?;
        }
        Ok(())
    }

    /// Une expression qui décrit une vue.
    pub fn element(&mut self, e: &Expr) -> R<Element> {
        let mut handlers = Vec::new();
        self.element_h(e, &mut handlers)
    }

    /// Comme [`element`](Self::element) ; les `.on(…)` relevés en chemin vont à la vue du fichier
    /// qu'ils suivent.
    fn element_h(&mut self, e: &Expr, handlers: &mut Vec<(String, Span, Handler)>) -> R<Element> {
        let span = e.span;
        match &e.kind {
            ExprKind::Ident(n) => self.entry_or_view(n, &[], None, handlers, span, &e.cache),
            ExprKind::Call {
                callee,
                args,
                trailing,
            } => match &callee.kind {
                ExprKind::Ident(n) => {
                    self.entry_or_view(n, args, trailing.as_ref(), handlers, span, &e.cache)
                }
                ExprKind::Member { recv, name } if name == "on" => {
                    let event = match args.first().map(|a| &a.value.kind) {
                        Some(ExprKind::Ident(ev)) if args.len() == 1 => ev.clone(),
                        _ => {
                            return Err(err(
                                callee.span,
                                "`.on(événement) { valeur in … }` : un nom d'événement attendu",
                            ));
                        }
                    };
                    let c = trailing
                        .as_ref()
                        .ok_or_else(|| err(callee.span, "`.on(…)` : la fermeture `{ … }` manque"))?;
                    handlers.push((event, callee.span, self.handler(c)));
                    self.element_h(recv, handlers)
                }
                ExprKind::Member { recv, name } if name == "onTimelineEvent" => {
                    let tl = match args.first() {
                        Some(a) => match self.eval(&a.value)? {
                            Val::Timeline(t) => t,
                            v => {
                                return Err(err(
                                    a.value.span,
                                    format!("une `@Timeline` attendue, trouvé {}", v.type_name()),
                                ));
                            }
                        },
                        None => {
                            return Err(err(callee.span, "`.onTimelineEvent(timeline, événement) { … }`"));
                        }
                    };
                    let filter = match args.get(1).map(|a| &a.value.kind) {
                        Some(ExprKind::Ident(n)) => Some(n.clone()),
                        None => None,
                        Some(_) => {
                            return Err(err(
                                callee.span,
                                "`.onTimelineEvent` : un nom d'événement attendu",
                            ));
                        }
                    };
                    let c = trailing.as_ref().ok_or_else(|| {
                        err(callee.span, "`.onTimelineEvent(…)` : la fermeture `{ … }` manque")
                    })?;
                    let h = self.handler(c);
                    self.tl_handlers.borrow_mut().push((tl, filter, h));
                    self.element_h(recv, handlers)
                }
                ExprKind::Member { recv, name } => {
                    let child = self.element_h(recv, handlers)?;
                    self.modifier(child, name, args, trailing.as_ref(), callee.span, &e.cache)
                }
                _ => Err(err(span, "une vue attendue")),
            },
            ExprKind::Member { recv, name } => {
                let child = self.element_h(recv, handlers)?;
                self.modifier(child, name, &[], None, span, &e.cache)
            }
            ExprKind::Ternary { cond, then, els } => {
                if self.bool(cond)? {
                    self.element_h(then, handlers)
                } else {
                    self.element_h(els, handlers)
                }
            }
            _ => Err(err(span, "une vue attendue (`Text(…)`, `VStack { … }`…)")),
        }
    }

    /// Une fermeture `{ valeur in … }` qui agit dans ce contexte.
    fn handler(&self, c: &Closure) -> Handler {
        let cx = self.clone();
        let body = Rc::new(c.body.clone());
        let param = c.params.first().cloned();
        Rc::new(move |v| {
            let mut cx = cx.clone();
            if let Some(p) = &param {
                cx.locals.push((p.clone(), v));
            }
            if let Err(d) = cx.exec(&body) {
                crate::program::report(d);
            }
        })
    }

    /// `Article(titre: …, prix: …) { contenu du slot }` : ce que l'appelant donne à une vue du fichier.
    fn instance(
        &mut self,
        decl: &ViewDecl,
        args: &[Arg],
        trailing: Option<&Closure>,
        handlers: &mut Vec<(String, Span, Handler)>,
    ) -> R<Instance> {
        let mut inst = Instance::default();
        let mut positional = decl.params.iter();
        for a in args {
            let name = match &a.label {
                Some(l) => l.clone(),
                None => match positional.find(|p| !inst.params.contains_key(&p.name)) {
                    Some(p) => p.name.clone(),
                    None => return Err(err(a.value.span, format!("`{}` : trop d'arguments", decl.name))),
                },
            };
            if let Some(p) = decl.params.iter().find(|p| p.name == name) {
                if inst.params.contains_key(&name) {
                    return Err(err(
                        a.value.span,
                        format!("`{}` : `{name}` donné deux fois", decl.name),
                    ));
                }
                let v = self.eval(&a.value)?;
                let v = check_type(&p.ty, v, a.value.span)
                    .map_err(|d| err(d.span, format!("`{}` : `{name}` : {}", decl.name, d.message)))?;
                inst.params.insert(name, v);
            } else if decl.slots.contains(&name) {
                inst.slots.insert(
                    name,
                    SlotVal::Expr(Box::new(self.clone()), Rc::new(a.value.clone())),
                );
            } else {
                let names = decl
                    .params
                    .iter()
                    .map(|p| p.name.as_str())
                    .chain(decl.slots.iter().map(String::as_str));
                return Err(err(
                    a.value.span,
                    format!(
                        "`{}` n'a pas de paramètre `{name}`{}",
                        decl.name,
                        suggest(&name, names)
                    ),
                ));
            }
        }
        if let Some(c) = trailing {
            let Some(first) = decl.slots.first() else {
                return Err(err(
                    c_span(c, a_span(args)),
                    format!("`{}` n'a pas de `@Slot` : pas de `{{ … }}`", decl.name),
                ));
            };
            inst.slots.insert(
                first.clone(),
                SlotVal::Body(Box::new(self.clone()), Rc::new(c.body.clone())),
            );
        }
        for p in &decl.params {
            if p.default.is_none() && !inst.params.contains_key(&p.name) {
                return Err(err(
                    a_span(args),
                    format!("`{}` : le paramètre `{}` manque", decl.name, p.name),
                ));
            }
        }
        for (name, at, h) in handlers.drain(..) {
            if !decl.events.iter().any(|e| e.name == name) {
                let names = decl.events.iter().map(|e| e.name.as_str());
                return Err(err(
                    at,
                    format!(
                        "`{}` n'a pas d'`@Event {name}`{}",
                        decl.name,
                        suggest(&name, names)
                    ),
                ));
            }
            inst.handlers.insert(name, h);
        }
        Ok(inst)
    }

    fn modifier(
        &mut self,
        child: Element,
        name: &str,
        args: &[Arg],
        trailing: Option<&Closure>,
        span: Span,
        cache: &Cache,
    ) -> R<Element> {
        // L'entrée du schéma, trouvée une fois pour ce site.
        if let Some(Some(entry)) = cache.entry.get()
            && entry.kind == EntryKind::Modifier
        {
            return self.build(entry, args, trailing, vec![child], span);
        }
        if name == "id" {
            let a = args
                .first()
                .ok_or_else(|| err(span, "`.id(…)` : un identifiant attendu"))?;
            let key = match self.eval(&a.value)? {
                Val::Int(i) => Key::Int(i),
                Val::Str(s) => Key::Str(s),
                v => Key::Str(Rc::from(v.to_string())),
            };
            return Ok(child.id(key));
        }
        let shared = self.shared.clone();
        match shared.schema.get(name) {
            Some(entry) if entry.kind == EntryKind::Modifier => {
                let _ = cache.entry.set(Some(entry.clone()));
                self.build(entry, args, trailing, vec![child], span)
            }
            Some(_) => Err(err(span, format!("`{name}` est une vue, pas un modifier"))),
            None => {
                let mods = shared
                    .schema
                    .entries()
                    .filter(|e| e.kind == EntryKind::Modifier)
                    .map(|e| e.name);
                Err(err(
                    span,
                    format!("modifier `.{name}` inconnu{}", suggest(name, mods.chain(["id"]))),
                ))
            }
        }
    }

    fn entry_or_view(
        &mut self,
        name: &str,
        args: &[Arg],
        trailing: Option<&Closure>,
        handlers: &mut Vec<(String, Span, Handler)>,
        span: Span,
        cache: &Cache,
    ) -> R<Element> {
        // Une primitive du schéma déjà vue ici (le nom n'est donc pas une vue du fichier).
        if let Some(Some(entry)) = cache.entry.get()
            && entry.kind == EntryKind::Primitive
            && handlers.is_empty()
        {
            return self.build(entry, args, trailing, Vec::new(), span);
        }
        let shared = self.shared.clone();
        if let Some(decl) = shared.module.view(name) {
            let inst = self.instance(decl, args, trailing, handlers)?;
            return Ok((self.views)(name, inst));
        }
        if let Some((ev, at, _)) = handlers.first() {
            return Err(err(
                *at,
                format!(
                    "`.on({ev})` : `{name}` n'envoie pas d'événements (seule une vue du fichier le peut)"
                ),
            ));
        }
        if name == "Slot" {
            let Some(slot) = slot_name_args(args) else {
                return Err(err(span, "`Slot(nom)` : le nom d'un `@Slot` attendu"));
            };
            let mut items = self.slot(slot, span)?;
            return Ok(match items.len() {
                1 => items.pop().expect("un élément"),
                _ => shared
                    .schema
                    .build("VStack", Params::new(), items)
                    .map_err(|e| err(span, e.to_string()))?,
            });
        }
        match shared.schema.get(name) {
            Some(entry) if entry.kind == EntryKind::Primitive => {
                let _ = cache.entry.set(Some(entry.clone()));
                self.build(entry, args, trailing, Vec::new(), span)
            }
            Some(_) => Err(err(
                span,
                format!("`{name}` est un modifier : écrivez `.{name}(…)` après une vue"),
            )),
            None => {
                let views = shared.module.views().map(|v| v.name.as_str());
                let prims = shared
                    .schema
                    .entries()
                    .filter(|e| e.kind == EntryKind::Primitive)
                    .map(|e| e.name);
                Err(err(
                    span,
                    format!("vue `{name}` inconnue{}", suggest(name, prims.chain(views))),
                ))
            }
        }
    }

    /// Construit une entrée du schéma : arguments, fermeture (action ou enfants).
    fn build(
        &mut self,
        entry: &veka_ui::schema::Entry,
        args: &[Arg],
        trailing: Option<&Closure>,
        mut children: Vec<Element>,
        span: Span,
    ) -> R<Element> {
        let mut params = Params(Vec::with_capacity(args.len() + usize::from(trailing.is_some())));
        // Les paramètres déjà donnés, par nom (une petite liste sur la pile).
        let mut given: smallvec::SmallVec<[&str; 8]> = smallvec::SmallVec::new();
        let mut positional = entry.params.iter().filter(|p| p.ty != ValueType::Action);
        for a in args {
            let spec = match &a.label {
                Some(l) => entry.params.iter().find(|p| p.name == l).ok_or_else(|| {
                    err(
                        a.value.span,
                        format!(
                            "`{}` n'a pas de paramètre `{l}`{}",
                            entry.name,
                            suggest(l, entry.params.iter().map(|p| p.name))
                        ),
                    )
                })?,
                None => loop {
                    match positional.next() {
                        Some(p) if given.contains(&p.name) => continue,
                        Some(p) => break p,
                        None => {
                            return Err(err(a.value.span, format!("`{}` : trop d'arguments", entry.name)));
                        }
                    }
                },
            };
            if given.contains(&spec.name) {
                return Err(err(
                    a.value.span,
                    format!("`{}` : `{}` donné deux fois", entry.name, spec.name),
                ));
            }
            given.push(spec.name);
            let v = self.eval(&a.value)?;
            let value =
                to_schema(&v, spec.ty).map_err(|m| err(a.value.span, format!("`{}` : {m}", spec.name)))?;
            params = params.with_static(spec.name, value);
        }
        if let Some(c) = trailing {
            let action = entry
                .params
                .iter()
                .find(|p| p.ty == ValueType::Action && !given.contains(&p.name));
            match action {
                Some(p) => {
                    params = params.with_static(p.name, Value::Action(self.action(c)));
                }
                None if entry.children == Children::Many && entry.kind == EntryKind::Primitive => {
                    children = self.body(&c.body)?;
                }
                None => return Err(err(span, format!("`{}` ne prend pas de `{{ … }}`", entry.name))),
            }
        }
        Schema::build_entry(entry, params, children).map_err(|e| err(span, e.to_string()))
    }

    // --- Actions -------------------------------------------------------------------------------

    fn action(&self, c: &Closure) -> veka_ui::Action {
        let cx = self.clone();
        let body = Rc::new(c.body.clone());
        Action::new(move || {
            let mut cx = cx.clone();
            if let Err(d) = cx.exec(&body) {
                crate::program::report(d);
            }
        })
    }

    pub fn exec(&mut self, stmts: &[Stmt]) -> R<()> {
        let mark = self.locals.len();
        for s in stmts {
            match s {
                Stmt::Expr(e) => self.exec_expr(e)?,
                Stmt::If { cond, then, els, .. } => {
                    if self.bool(cond)? {
                        self.exec(then)?;
                    } else if let Some(els) = els {
                        self.exec(els)?;
                    }
                }
                Stmt::Let { name, value, .. } => {
                    let v = self.eval(value)?;
                    self.locals.push((name.clone(), v));
                }
                Stmt::Emit { name, value, span } => {
                    let declared = self
                        .shared
                        .module
                        .view(&self.view)
                        .is_some_and(|v| v.events.iter().any(|e| &e.name == name));
                    if !declared {
                        return Err(err(*span, format!("`{name}` n'est pas un `@Event` de cette vue")));
                    }
                    let v = match value {
                        Some(e) => self.eval(e)?,
                        None => Val::Unit,
                    };
                    if let Some(h) = self.handlers.get(name).cloned() {
                        h(v);
                    } else if let Some(f) = self.host.on_event.borrow().clone() {
                        f(&self.view, name, &v);
                    }
                }
                Stmt::Assign {
                    target,
                    op,
                    value,
                    span,
                } => {
                    let ExprKind::Ident(name) = &target.kind else {
                        return Err(err(*span, "on ne peut changer qu'un état (`nom = …`)"));
                    };
                    let v = self.eval(value)?;
                    let v = match op {
                        AssignOp::Set => v,
                        op => {
                            let old = self.eval(target)?;
                            let b = match op {
                                AssignOp::Add => BinaryOp::Add,
                                AssignOp::Sub => BinaryOp::Sub,
                                AssignOp::Mul => BinaryOp::Mul,
                                _ => BinaryOp::Div,
                            };
                            binary(b, &old, &v).map_err(|m| err(*span, m))?
                        }
                    };
                    self.set(name, v, *span)?;
                }
            }
        }
        self.locals.truncate(mark);
        Ok(())
    }

    fn set(&self, name: &str, v: Val, span: Span) -> R<()> {
        if !self.is_state(name) {
            return Err(err(span, format!("`{name}` n'est pas un `@State` de cette vue")));
        }
        let vars = self.vars.expect("is_state");
        let old = vars.with(|m| m.get(name).cloned()).unwrap_or(Val::Unit);
        // Un `Float` reste un `Float` quand on lui donne un entier.
        let v = match (&old, v) {
            (Val::Float(_), Val::Int(i)) => Val::Float(i as f64),
            (_, v) => v,
        };
        if std::mem::discriminant(&old) != std::mem::discriminant(&v) {
            return Err(err(
                span,
                format!(
                    "`{name}` est un {}, on ne peut pas lui donner un {}",
                    old.type_name(),
                    v.type_name()
                ),
            ));
        }
        vars.update(|m| {
            m.insert(name.to_owned(), v);
        });
        Ok(())
    }

    fn exec_expr(&mut self, e: &Expr) -> R<()> {
        let span = e.span;
        let ExprKind::Call {
            callee,
            args,
            trailing,
        } = &e.kind
        else {
            return Err(err(
                span,
                "une action attendue (`x = …`, `x.toggle()`, `withAnimation { … }`)",
            ));
        };
        match &callee.kind {
            ExprKind::Ident(n) if n == "withAnimation" => {
                let spec = match args.first() {
                    Some(a) => to_animation(&self.eval(&a.value)?).map_err(|m| err(a.value.span, m))?,
                    None => veka_anima::Animation::default(),
                };
                let body = trailing
                    .as_ref()
                    .ok_or_else(|| err(span, "`withAnimation { … }` : la fermeture manque"))?;
                let mut cx = self.clone();
                with_animation(spec, || cx.exec(&body.body))
            }
            ExprKind::Member { recv, name } => {
                let ExprKind::Ident(state) = &recv.kind else {
                    return Err(err(span, "une méthode d'état attendue (`liste.append(x)`)"));
                };
                let args = self.args(args)?;
                let old = self
                    .lookup(state)
                    .ok_or_else(|| err(recv.span, format!("`{state}` inconnu")))?;
                if let Val::Timeline(tl) = old {
                    return timeline_method(tl, name, &args).map_err(|m| err(span, m));
                }
                if matches!(&old, Val::Str(s) if &**s == SCROLL_PROXY) {
                    return scroll_to(name, &args).map_err(|m| err(span, m));
                }
                let new = mutate(old, name, &args).map_err(|m| err(span, m))?;
                self.set(state, new, span)
            }
            _ => Err(err(
                span,
                "une action attendue (`x = …`, `x.toggle()`, `withAnimation { … }`)",
            )),
        }
    }
}

/// La valeur de `proxy` dans `ScrollViewReader { proxy in … }`.
const SCROLL_PROXY: &str = "\u{0}ScrollViewProxy";

/// `proxy.scrollTo(id, anchor: .top)`.
fn scroll_to(method: &str, args: &[(Option<String>, Val)]) -> Result<(), String> {
    if method != "scrollTo" {
        return Err(format!("`proxy.{method}` inconnu : `proxy.scrollTo(id, anchor: .top)`"));
    }
    let id = args
        .iter()
        .find(|(l, _)| l.is_none())
        .map(|(_, v)| v.clone())
        .ok_or("`scrollTo` : l'identifiant (`.id(…)`) de la vue attendu")?;
    let key = match id {
        Val::Int(i) => veka_ui::Key::Int(i),
        Val::Str(s) => veka_ui::Key::Str(s),
        v => veka_ui::Key::Str(Rc::from(v.to_string())),
    };
    let anchor = match args.iter().find(|(l, _)| l.as_deref() == Some("anchor")) {
        Some((_, v)) => Some(crate::value::to_unit_point(v)?),
        None => None,
    };
    veka_ui::views::ScrollViewProxy.scroll_to(key, anchor);
    Ok(())
}

/// Les méthodes qui changent un état : `toggle`, `append`, `insert`, `remove`…
fn mutate(v: Val, method: &str, args: &[(Option<String>, Val)]) -> Result<Val, String> {
    let pos = |i: usize| {
        args.iter()
            .filter(|(l, _)| l.is_none())
            .nth(i)
            .map(|(_, v)| v.clone())
    };
    let at = || -> Result<usize, String> {
        match args
            .iter()
            .find(|(l, _)| l.as_deref() == Some("at"))
            .map(|(_, v)| v)
        {
            Some(Val::Int(i)) if *i >= 0 => Ok(*i as usize),
            _ => Err(format!("`{method}` : `at:` (un indice) attendu")),
        }
    };
    match (v, method) {
        (Val::Bool(b), "toggle") => Ok(Val::Bool(!b)),
        (Val::Array(a), m) => {
            let mut a = (*a).clone();
            match m {
                "append" => a.push(pos(0).ok_or("`append` : une valeur attendue")?),
                "insert" => {
                    let i = at()?;
                    if i > a.len() {
                        return Err(format!(
                            "`insert` : indice {i} hors du tableau ({} éléments)",
                            a.len()
                        ));
                    }
                    a.insert(i, pos(0).ok_or("`insert` : une valeur attendue")?);
                }
                "remove" => {
                    let i = at()?;
                    if i >= a.len() {
                        return Err(format!(
                            "`remove` : indice {i} hors du tableau ({} éléments)",
                            a.len()
                        ));
                    }
                    a.remove(i);
                }
                "removeLast" => {
                    a.pop();
                }
                "removeFirst" => {
                    if !a.is_empty() {
                        a.remove(0);
                    }
                }
                "removeAll" => a.clear(),
                m => {
                    return Err(format!(
                        "`{m}` : inconnu sur un tableau (append, insert, remove, removeLast…)"
                    ));
                }
            }
            Ok(Val::Array(Rc::new(a)))
        }
        (v, m) => Err(format!("`{m}` : inconnu sur un {}", v.type_name())),
    }
}

/// `p.play()`, `p.seek(0.5)`… dans une action.
fn timeline_method(tl: TimelineState, name: &str, args: &[(Option<String>, Val)]) -> Result<(), String> {
    match name {
        "play" => tl.play(),
        "pause" => tl.pause(),
        "stop" => tl.stop(),
        "reverse" => tl.reverse(),
        "toggle" => {
            if tl.is_playing() {
                tl.pause()
            } else {
                tl.play()
            }
        }
        "seek" => {
            let t = args
                .first()
                .and_then(|(_, v)| v.as_f64())
                .ok_or("`seek` : un instant (secondes) attendu")?;
            tl.seek(t as f32)
        }
        n => {
            return Err(format!(
                "`{n}` : inconnu sur une timeline (play, pause, stop, reverse, toggle, seek)"
            ));
        }
    }
    Ok(())
}

/// `recv.name` ou `recv.name(args)`.
fn member(r: &Val, name: &str, args: Option<Vec<(Option<String>, Val)>>) -> Result<Val, String> {
    match r {
        Val::Record(_) if args.is_none() => r.get(name).ok_or_else(|| {
            let Val::Record(f) = r else { unreachable!() };
            format!(
                "champ `{name}` absent{}",
                suggest(name, f.iter().map(|(k, _)| &**k))
            )
        }),
        Val::Timeline(t) if args.is_none() => Ok(match name {
            "time" => Val::Float(t.time() as f64),
            "duration" => Val::Float(t.duration() as f64),
            "isPlaying" => Val::Bool(t.is_playing()),
            "isReversed" => Val::Bool(t.is_reversed()),
            track => Val::Float(t.value(track) as f64),
        }),
        Val::Sym(s) => {
            let mut s = (**s).clone();
            s.chain.push((name.to_owned(), args));
            Ok(Val::Sym(Rc::new(s)))
        }
        Val::Array(a) if args.is_none() => match name {
            "count" => Ok(Val::Int(a.len() as i64)),
            "isEmpty" => Ok(Val::Bool(a.is_empty())),
            "first" => a.first().cloned().ok_or_else(|| "`first` : tableau vide".into()),
            "last" => a.last().cloned().ok_or_else(|| "`last` : tableau vide".into()),
            _ => Err(format!(
                "`{name}` : inconnu sur un tableau (count, isEmpty, first, last)"
            )),
        },
        Val::Array(a) if name == "contains" => {
            let x = args
                .and_then(|v| v.into_iter().next())
                .map(|(_, v)| v)
                .ok_or("`contains` : une valeur attendue")?;
            Ok(Val::Bool(a.contains(&x)))
        }
        Val::Str(s) if name == "count" && args.is_none() => Ok(Val::Int(s.chars().count() as i64)),
        Val::Str(s) if name == "isEmpty" && args.is_none() => Ok(Val::Bool(s.is_empty())),
        v => Err(format!("`.{name}` : inconnu sur un {}", v.type_name())),
    }
}

/// Les noms de la bibliothèque (pour « vouliez-vous dire »).
const BUILTINS: &[&str] = &[
    "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "sqrt", "pow", "exp", "log", "floor", "ceil",
    "round", "abs", "min", "max", "clamp", "lerp", "random", "len", "format", "rgb", "rgba", "token",
    "String", "Int", "Float",
];

/// `random(graine)` : un nombre de 0 à 1, toujours le même pour la même graine (SplitMix64).
pub(crate) fn random01(seed: &Val) -> f64 {
    let bits = match seed {
        Val::Int(i) => *i as u64,
        Val::Float(f) => f.to_bits(),
        Val::Bool(b) => *b as u64,
        Val::Str(s) => s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
            (h ^ b as u64).wrapping_mul(0x100_0000_01b3)
        }),
        _ => 0,
    };
    let mut z = bits.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

/// `format("%.1f €", x)` : `%d`, `%f`, `%.Nf`, `%s`, `%%`.
fn format_text(fmt: &str, args: &[Val]) -> Result<String, String> {
    let mut out = String::new();
    let mut it = fmt.chars().peekable();
    let mut next = args.iter();
    while let Some(c) = it.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let mut prec = None;
        if it.peek() == Some(&'.') {
            it.next();
            let mut digits = String::new();
            while let Some(d) = it.peek().filter(|d| d.is_ascii_digit()) {
                digits.push(*d);
                it.next();
            }
            prec = digits.parse::<usize>().ok();
        }
        let spec = it.next().ok_or("`format` : `%` en fin de texte")?;
        if spec == '%' {
            out.push('%');
            continue;
        }
        let v = next.next().ok_or("`format` : pas assez de valeurs")?;
        match spec {
            'd' => out.push_str(&format!(
                "{}",
                v.as_f64().ok_or("`%d` : un nombre attendu")?.round() as i64
            )),
            'f' => out.push_str(&format!(
                "{:.*}",
                prec.unwrap_or(6),
                v.as_f64().ok_or("`%f` : un nombre attendu")?
            )),
            's' => out.push_str(&v.to_string()),
            c => return Err(format!("`format` : `%{c}` inconnu (%d, %f, %.2f, %s, %%)")),
        }
    }
    Ok(out)
}

/// Les maths : `None` si `name` n'en est pas.
fn math(name: &str, args: &[(Option<String>, Val)]) -> Option<Result<Val, String>> {
    let n = |i: usize| -> Result<f64, String> {
        args.get(i)
            .and_then(|(_, v)| v.as_f64())
            .ok_or_else(|| format!("`{name}` : un nombre attendu en argument {}", i + 1))
    };
    let one = |f: fn(f64) -> f64| n(0).map(|x| Val::Float(f(x)));
    Some(match name {
        "sin" => one(f64::sin),
        "cos" => one(f64::cos),
        "tan" => one(f64::tan),
        "asin" => one(f64::asin),
        "acos" => one(f64::acos),
        "atan" => one(f64::atan),
        "sqrt" => one(f64::sqrt),
        "exp" => one(f64::exp),
        "log" => one(f64::ln),
        "atan2" => (|| Ok(Val::Float(n(0)?.atan2(n(1)?))))(),
        "pow" => (|| Ok(Val::Float(n(0)?.powf(n(1)?))))(),
        "floor" | "ceil" | "round" => n(0).map(|x| {
            Val::Int(match name {
                "floor" => x.floor(),
                "ceil" => x.ceil(),
                _ => x.round(),
            } as i64)
        }),
        "clamp" => match args {
            [(_, Val::Int(x)), (_, Val::Int(lo)), (_, Val::Int(hi))] => {
                Ok(Val::Int((*x).clamp(*lo, (*hi).max(*lo))))
            }
            _ => (|| {
                let (x, lo, hi) = (n(0)?, n(1)?, n(2)?);
                Ok(Val::Float(x.max(lo).min(hi.max(lo))))
            })(),
        },
        "lerp" => (|| {
            let (a, b, t) = (n(0)?, n(1)?, n(2)?);
            Ok(Val::Float(a + (b - a) * t))
        })(),
        "random" => match args.len() {
            1 => Ok(Val::Float(random01(&args[0].1))),
            3 => (|| {
                let (lo, hi) = (n(1)?, n(2)?);
                Ok(Val::Float(lo + (hi - lo) * random01(&args[0].1)))
            })(),
            _ => Err("`random(graine)` ou `random(graine, min, max)`".into()),
        },
        "len" => match args.first().map(|a| &a.1) {
            Some(Val::Str(s)) => Ok(Val::Int(s.chars().count() as i64)),
            Some(v) => v
                .items()
                .map(|i| Val::Int(i.len() as i64))
                .ok_or_else(|| format!("`len` : un tableau attendu, trouvé {}", v.type_name())),
            None => Err("`len(tableau)`".into()),
        },
        "format" => match args.split_first() {
            Some(((_, Val::Str(f)), rest)) => {
                let rest: Vec<Val> = rest.iter().map(|(_, v)| v.clone()).collect();
                format_text(f, &rest).map(|s| Val::Str(s.into()))
            }
            _ => Err("`format(\"%.1f\", x)` : un texte de format attendu".into()),
        },
        _ => return None,
    })
}

/// Les fonctions de base.
fn builtin(name: &str, args: &[(Option<String>, Val)]) -> Result<Val, String> {
    if let Some(p) = gradient(name, args) {
        return p.map(Val::Paint);
    }
    if let Some(r) = math(name, args) {
        return r;
    }
    let n = |i: usize| -> Result<f64, String> {
        args.get(i)
            .and_then(|(_, v)| v.as_f64())
            .ok_or_else(|| format!("`{name}` : un nombre attendu en argument {}", i + 1))
    };
    let f = |v: f64| v as f32;
    Ok(match name {
        "rgb" => Val::Color(veka_foundation::Color::rgb(f(n(0)?), f(n(1)?), f(n(2)?))),
        "rgba" => Val::Color(veka_foundation::Color::rgba(
            f(n(0)?),
            f(n(1)?),
            f(n(2)?),
            f(n(3)?),
        )),
        "token" => match args.first() {
            Some((_, Val::Str(s))) => Val::Paint(Paint::from(Token::named(s))),
            _ => return Err("`token(\"Nom\")` : un nom de jeton attendu".into()),
        },
        "min" | "max" => {
            let (a, b) = (args.first().map(|x| &x.1), args.get(1).map(|x| &x.1));
            match (a, b) {
                (Some(Val::Int(a)), Some(Val::Int(b))) => {
                    Val::Int(if name == "min" { *a.min(b) } else { *a.max(b) })
                }
                _ => {
                    let (a, b) = (n(0)?, n(1)?);
                    Val::Float(if name == "min" { a.min(b) } else { a.max(b) })
                }
            }
        }
        "abs" => match args.first().map(|x| &x.1) {
            Some(Val::Int(i)) => Val::Int(i.abs()),
            _ => Val::Float(n(0)?.abs()),
        },
        "String" => Val::Str(Rc::from(
            args.first().map(|x| x.1.to_string()).unwrap_or_default(),
        )),
        "Int" => Val::Int(n(0)? as i64),
        "Float" => Val::Float(n(0)?),
        _ => return Err(format!("fonction `{name}` inconnue")),
    })
}

pub(crate) fn binary(op: BinaryOp, l: &Val, r: &Val) -> Result<Val, String> {
    use BinaryOp::*;
    let bad = || format!("`{}` entre {} et {}", op.text(), l.type_name(), r.type_name());
    Ok(match (op, l, r) {
        (Eq, a, b) => Val::Bool(a == b),
        (Ne, a, b) => Val::Bool(a != b),
        (Add, Val::Str(a), b) => Val::Str(Rc::from(format!("{a}{b}"))),
        (Add, a, Val::Str(b)) => Val::Str(Rc::from(format!("{a}{b}"))),
        (Add, Val::Array(a), Val::Array(b)) => {
            Val::Array(Rc::new(a.iter().chain(b.iter()).cloned().collect()))
        }
        (Lt | Le | Gt | Ge, Val::Str(a), Val::Str(b)) => Val::Bool(match op {
            Lt => a < b,
            Le => a <= b,
            Gt => a > b,
            _ => a >= b,
        }),
        (_, Val::Int(a), Val::Int(b)) => match op {
            Add => Val::Int(a.wrapping_add(*b)),
            Sub => Val::Int(a.wrapping_sub(*b)),
            Mul => Val::Int(a.wrapping_mul(*b)),
            Div if *b == 0 => return Err("division par zéro".into()),
            Div => Val::Int(a / b),
            Rem if *b == 0 => return Err("division par zéro".into()),
            Rem => Val::Int(a.rem_euclid(*b)),
            Lt => Val::Bool(a < b),
            Le => Val::Bool(a <= b),
            Gt => Val::Bool(a > b),
            Ge => Val::Bool(a >= b),
            _ => return Err(bad()),
        },
        (_, a, b) => {
            let (Some(a), Some(b)) = (a.as_f64(), b.as_f64()) else {
                return Err(bad());
            };
            match op {
                Add => Val::Float(a + b),
                Sub => Val::Float(a - b),
                Mul => Val::Float(a * b),
                Div => Val::Float(a / b),
                Rem => Val::Float(a.rem_euclid(b)),
                Lt => Val::Bool(a < b),
                Le => Val::Bool(a <= b),
                Gt => Val::Bool(a > b),
                Ge => Val::Bool(a >= b),
                _ => return Err(bad()),
            }
        }
    })
}

/// Construit les thèmes du fichier (dans l'ordre : un parent avant ses enfants).
pub(crate) fn themes(module: &Module, schema: &Rc<Schema>) -> (HashMap<String, Arc<Theme>>, Vec<Diagnostic>) {
    let mut out: HashMap<String, Arc<Theme>> = HashMap::new();
    let mut errors = Vec::new();
    let decls: Vec<&ThemeDecl> = module.themes().collect();
    let mut left: Vec<&ThemeDecl> = decls.clone();
    while !left.is_empty() {
        let before = left.len();
        left.retain(|t| {
            let parent = match &t.parent {
                Some(p) => match out.get(p) {
                    Some(p) => Some(p.clone()),
                    None if decls.iter().any(|d| &d.name == p) => return true,
                    None => {
                        errors.push(err(t.span, format!("thème parent `{p}` inconnu")));
                        None
                    }
                },
                None => None,
            };
            let cx = Cx::root(
                Rc::new(Shared {
                    module: Rc::new(Module::default()),
                    schema: schema.clone(),
                    themes: out.clone(),
                }),
                Rc::new(Host::default()),
                Rc::new(|_, _| veka_ui::views::Text("")),
            );
            let mut theme = Theme::new(t.name.clone());
            if let Some(p) = parent {
                theme = theme.parent(p);
            }
            for e in &t.entries {
                let light = match cx.eval(&e.light) {
                    Ok(v) => v,
                    Err(d) => {
                        errors.push(d);
                        continue;
                    }
                };
                if let (Some(v), None) = (light.as_f64(), &e.dark) {
                    theme = theme.metric(e.token.clone(), v as f32);
                    continue;
                }
                let paint = |v: &Val, at: Span| to_paint(v).map_err(|m| err(at, m));
                let dark = match &e.dark {
                    Some(d) => cx.eval(d).and_then(|v| paint(&v, d.span)),
                    None => paint(&light, e.light.span),
                };
                match (paint(&light, e.light.span), dark) {
                    (Ok(l), Ok(d)) => theme = theme.color(Token::named(&e.token), l, d),
                    (Err(x), _) | (_, Err(x)) => errors.push(x),
                }
            }
            out.insert(t.name.clone(), Arc::new(theme));
            false
        });
        if left.len() == before {
            for t in &left {
                errors.push(err(t.span, format!("thème `{}` : parent en boucle", t.name)));
            }
            break;
        }
    }
    (out, errors)
}

/// `Slot(nom)` dans un corps.
fn slot_name(e: &Expr) -> Option<&str> {
    match &e.kind {
        ExprKind::Call {
            callee,
            args,
            trailing: None,
        } if matches!(&callee.kind, ExprKind::Ident(n) if n == "Slot") => slot_name_args(args),
        _ => None,
    }
}

fn slot_name_args(args: &[Arg]) -> Option<&str> {
    match args {
        [Arg { label: None, value }] => match &value.kind {
            ExprKind::Ident(n) => Some(n),
            _ => None,
        },
        _ => None,
    }
}

fn a_span(args: &[Arg]) -> Span {
    args.first().map_or(Span::START, |a| a.value.span)
}

fn c_span(c: &Closure, fallback: Span) -> Span {
    match c.body.first() {
        Some(Stmt::Expr(e)) => e.span,
        _ => fallback,
    }
}

/// Construit la `Timeline` d'une déclaration ; `args` (de `@Timeline var p = Nom(…)`) remplace ses
/// réglages. Rend aussi `autoplay`.
pub(crate) fn build_timeline(cx: &Cx, decl: &TimelineDecl, args: &[Arg]) -> R<(veka_anima::Timeline, bool)> {
    use veka_anima::{Curve, Keyframe, Timeline};
    let mut tl = Timeline::new();
    let mut autoplay = false;
    for a in decl.args.iter().chain(args) {
        let v = cx.eval(&a.value)?;
        let num = || {
            v.as_f64()
                .map(|x| x as f32)
                .ok_or_else(|| err(a.value.span, "un nombre attendu"))
        };
        match a.label.as_deref() {
            Some("loop") | Some("looping") => {
                tl = tl.looping(
                    v.as_bool()
                        .ok_or_else(|| err(a.value.span, "`true` ou `false` attendu"))?,
                )
            }
            Some("length") => tl = tl.length(num()?),
            Some("speed") => tl = tl.speed(num()?),
            Some("autoplay") => {
                autoplay = v
                    .as_bool()
                    .ok_or_else(|| err(a.value.span, "`true` ou `false` attendu"))?
            }
            other => {
                return Err(err(
                    a.value.span,
                    format!(
                        "timeline : réglage `{}` inconnu (loop, length, speed, autoplay)",
                        other.unwrap_or("?")
                    ),
                ));
            }
        }
    }
    for tr in &decl.tracks {
        let mut keys = Vec::new();
        for k in &tr.keys {
            let t = cx
                .eval(&k.time)?
                .as_f64()
                .ok_or_else(|| err(k.time.span, "un instant attendu"))?;
            let v = cx
                .eval(&k.value)?
                .as_f64()
                .ok_or_else(|| err(k.value.span, "une valeur attendue"))?;
            let mut key = Keyframe::new(t as f32, v as f32);
            if let Some(c) = &k.curve {
                key = key.curve(match c.as_str() {
                    "linear" => Curve::Linear,
                    "easeIn" => Curve::EaseIn,
                    "easeOut" => Curve::EaseOut,
                    "easeInOut" => Curve::EaseInOut,
                    "circEaseIn" => Curve::CircEaseIn,
                    "circEaseOut" => Curve::CircEaseOut,
                    _ => Curve::CircEaseInOut,
                });
            }
            keys.push(key);
        }
        tl = tl.track(tr.name.clone(), keys);
    }
    for (t, name) in &decl.events {
        let t = cx
            .eval(t)?
            .as_f64()
            .ok_or_else(|| err(t.span, "un instant attendu"))?;
        tl = tl.event(t as f32, name.clone());
    }
    Ok((tl, autoplay))
}

/// Vérifie (et convertit) la valeur initiale d'un état contre son type écrit.
pub(crate) fn check_type(ty: &Option<TypeName>, v: Val, span: Span) -> Result<Val, Diagnostic> {
    let Some(ty) = ty else { return Ok(v) };
    fn ok(ty: &TypeName, v: &Val) -> bool {
        match (ty, v) {
            (TypeName::Array(t), Val::Array(items)) => items.iter().all(|i| ok(t, i)),
            (TypeName::Array(_), _) => false,
            (TypeName::Named(n), v) => match n.as_str() {
                "Int" => matches!(v, Val::Int(_)),
                "Float" | "Double" => matches!(v, Val::Float(_) | Val::Int(_)),
                "Bool" => matches!(v, Val::Bool(_)),
                "String" => matches!(v, Val::Str(_)),
                "Color" => matches!(v, Val::Color(_)),
                _ => true,
            },
        }
    }
    fn conv(ty: &TypeName, v: Val) -> Val {
        match (ty, v) {
            (TypeName::Named(n), Val::Int(i)) if n == "Float" || n == "Double" => Val::Float(i as f64),
            (TypeName::Array(t), Val::Array(items)) => {
                Val::Array(Rc::new(items.iter().cloned().map(|i| conv(t, i)).collect()))
            }
            (_, v) => v,
        }
    }
    if !ok(ty, &v) {
        return Err(err(
            span,
            format!(
                "valeur de type `{}`, `{}` attendu",
                v.type_name(),
                crate::printer::type_text(ty)
            ),
        ));
    }
    Ok(conv(ty, v))
}

/// L'analyse au chargement : les vues et entrées inconnues, les paramètres mal nommés.
pub(crate) fn check(module: &Module, schema: &Schema) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for v in module.views() {
        if !seen.insert(v.name.as_str()) {
            out.push(err(v.span, format!("vue `{}` déclarée deux fois", v.name)));
        }
        let mut states = std::collections::HashSet::new();
        for s in &v.states {
            if !states.insert(s.name.as_str()) {
                out.push(err(s.span, format!("état `{}` déclaré deux fois", s.name)));
            }
        }
        for s in &v.body {
            check_stmt(s, module, schema, &mut out);
        }
    }
    out
}

fn check_stmt(s: &Stmt, module: &Module, schema: &Schema, out: &mut Vec<Diagnostic>) {
    match s {
        Stmt::Expr(e) => check_view_expr(e, module, schema, out),
        Stmt::If { then, els, .. } => {
            for s in then.iter().chain(els.iter().flatten()) {
                check_stmt(s, module, schema, out);
            }
        }
        Stmt::Let { .. } => {}
        Stmt::Assign { span, .. } => out.push(err(*span, "une affectation n'a sa place que dans une action")),
        Stmt::Emit { span, .. } => out.push(err(*span, "`emit` n'a sa place que dans une action")),
    }
}

/// Une expression en position de vue : `Nom(…)`, `vue.modifier(…)`.
fn check_view_expr(e: &Expr, module: &Module, schema: &Schema, out: &mut Vec<Diagnostic>) {
    let labels = |name: &str, args: &[Arg], out: &mut Vec<Diagnostic>| {
        let Some(entry) = schema.get(name) else { return };
        for a in args {
            if let Some(l) = &a.label
                && !entry.params.iter().any(|p| p.name == l)
            {
                let names = entry.params.iter().map(|p| p.name);
                out.push(err(
                    a.value.span,
                    format!("`{name}` n'a pas de paramètre `{l}`{}", suggest(l, names)),
                ));
            }
        }
    };
    let known = |name: &str| name == "Slot" || module.view(name).is_some() || schema.get(name).is_some();
    let unknown = |name: &str, span: Span, out: &mut Vec<Diagnostic>| {
        let views = module.views().map(|v| v.name.as_str());
        let prims = schema
            .entries()
            .filter(|e| e.kind == EntryKind::Primitive)
            .map(|e| e.name);
        out.push(err(
            span,
            format!("vue `{name}` inconnue{}", suggest(name, prims.chain(views))),
        ));
    };
    match &e.kind {
        ExprKind::Ident(n) if n.starts_with(char::is_uppercase) && !known(n) => unknown(n, e.span, out),
        ExprKind::Call {
            callee,
            args,
            trailing,
        } => match &callee.kind {
            ExprKind::Ident(n) if n == "ForEach" || n == "withAnimation" || n == "ScrollViewReader" => {
                if let Some(c) = trailing {
                    for s in &c.body {
                        check_stmt(s, module, schema, out);
                    }
                }
            }
            ExprKind::Ident(n) => {
                if !known(n) {
                    unknown(n, callee.span, out);
                    return;
                }
                labels(n, args, out);
                if let (Some(c), Some(_)) = (trailing, module.view(n)) {
                    for s in &c.body {
                        check_stmt(s, module, schema, out);
                    }
                }
                if let Some(c) = trailing
                    && schema.get(n).is_some_and(|e| e.children != Children::None)
                    && !schema
                        .get(n)
                        .is_some_and(|e| e.params.iter().any(|p| p.ty == ValueType::Action))
                {
                    for s in &c.body {
                        check_stmt(s, module, schema, out);
                    }
                }
            }
            ExprKind::Member { recv, name } => {
                check_view_expr(recv, module, schema, out);
                if matches!(name.as_str(), "id" | "on" | "onTimelineEvent") {
                    return;
                }
                match schema.get(name) {
                    Some(entry) if entry.kind == EntryKind::Modifier => labels(name, args, out),
                    _ => {
                        let mods = schema
                            .entries()
                            .filter(|e| e.kind == EntryKind::Modifier)
                            .map(|e| e.name);
                        out.push(err(
                            callee.span,
                            format!("modifier `.{name}` inconnu{}", suggest(name, mods)),
                        ));
                    }
                }
            }
            _ => {}
        },
        ExprKind::Member { recv, name } if crate::printer::is_view_like(recv) => {
            check_view_expr(recv, module, schema, out);
            if schema.get(name).is_none_or(|e| e.kind != EntryKind::Modifier) && name != "id" {
                let mods = schema
                    .entries()
                    .filter(|e| e.kind == EntryKind::Modifier)
                    .map(|e| e.name);
                out.push(err(
                    e.span,
                    format!("modifier `.{name}` inconnu{}", suggest(name, mods)),
                ));
            }
        }
        ExprKind::Ternary { then, els, .. } => {
            check_view_expr(then, module, schema, out);
            check_view_expr(els, module, schema, out);
        }
        _ => {}
    }
}
