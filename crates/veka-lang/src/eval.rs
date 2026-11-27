//! L'interpréteur : une vue `.vk` est une vue composée de Veka dont le corps parcourt l'arbre du
//! fichier. Les vues sont construites par le schéma, les actions deviennent des `Action`.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use veka_colora::{Paint, Theme, Token};
use veka_ui::schema::{Children, EntryKind, Params, Schema, Value, ValueType};
use veka_ui::{Action, Element, Key, State, with_animation};

use crate::ast::*;
use crate::value::{Sym, Val, gradient, to_animation, to_paint, to_schema};
use crate::{Diagnostic, Span};

pub(crate) type Vars = HashMap<String, Val>;

type R<T> = Result<T, Diagnostic>;

/// Ce que l'évaluation partage : le fichier, le schéma, les thèmes construits.
pub(crate) struct Shared {
    pub module: Rc<Module>,
    pub schema: Rc<Schema>,
    pub themes: HashMap<String, Arc<Theme>>,
}

/// Le contexte d'évaluation d'une vue (ou d'une action).
#[derive(Clone)]
pub(crate) struct Cx {
    pub shared: Rc<Shared>,
    /// Les états de la vue (absents pendant le calcul de leurs valeurs initiales).
    pub vars: Option<State<Vars>>,
    /// Les états déjà calculés pendant l'initialisation.
    pub pre: Rc<Vars>,
    /// `let`, paramètres de `ForEach` et de fermetures.
    pub locals: Vec<(Rc<str>, Val)>,
    /// Les vues du fichier, pour en construire depuis un corps.
    pub views: Rc<dyn Fn(&str) -> Element>,
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
    fn lookup(&self, name: &str) -> Option<Val> {
        if let Some((_, v)) = self.locals.iter().rev().find(|(n, _)| &**n == name) {
            return Some(v.clone());
        }
        if let Some(v) = self.pre.get(name) {
            return Some(v.clone());
        }
        if let Some(t) = self.shared.themes.get(name) {
            return Some(Val::Theme(t.clone()));
        }
        self.vars.and_then(|s| s.with(|m| m.get(name).cloned()))
    }

    fn is_state(&self, name: &str) -> bool {
        self.vars.is_some_and(|s| s.with(|m| m.contains_key(name)))
            && !self.locals.iter().any(|(n, _)| &**n == name)
    }

    // --- Expressions ---------------------------------------------------------------------------

    pub fn eval(&self, e: &Expr) -> R<Val> {
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
            ExprKind::Ident(n) => self.lookup(n).ok_or_else(|| {
                let mut names: Vec<String> = self.locals.iter().map(|(n, _)| n.to_string()).collect();
                names.extend(self.pre.keys().cloned());
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
            ExprKind::Array(items) => {
                Val::Array(Rc::new(items.iter().map(|i| self.eval(i)).collect::<R<_>>()?))
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
                let args = self.args(args)?;
                match &callee.kind {
                    ExprKind::Symbol(n) => Val::Sym(Rc::new(Sym {
                        name: n.clone(),
                        args: Some(args),
                        chain: Vec::new(),
                    })),
                    ExprKind::Member { recv, name } => {
                        let r = self.eval(recv)?;
                        member(&r, name, Some(args)).map_err(|m| err(span, m))?
                    }
                    ExprKind::Ident(n) => builtin(n, &args).map_err(|m| err(span, m))?,
                    _ => return Err(err(span, "on ne peut appeler que des noms")),
                }
            }
        })
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

    fn args(&self, args: &[Arg]) -> R<Vec<(Option<String>, Val)>> {
        args.iter()
            .map(|a| Ok((a.label.clone(), self.eval(&a.value)?)))
            .collect()
    }

    // --- Vues ----------------------------------------------------------------------------------

    /// Un corps : les vues qu'il donne.
    pub fn body(&mut self, stmts: &[Stmt]) -> R<Vec<Element>> {
        let mark = self.locals.len();
        let mut out = Vec::new();
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
                        self.for_each(e.span, args, trailing.as_ref(), &mut out)?;
                    } else {
                        out.push(self.element(e)?);
                    }
                }
                Stmt::If { cond, then, els, .. } => {
                    if self.bool(cond)? {
                        out.extend(self.body(then)?);
                    } else if let Some(els) = els {
                        out.extend(self.body(els)?);
                    }
                }
                Stmt::Let { name, value, .. } => {
                    let v = self.eval(value)?;
                    self.locals.push((Rc::from(name.as_str()), v));
                }
                Stmt::Assign { span, .. } => {
                    return Err(err(
                        *span,
                        "on ne change pas un état en décrivant la vue : faites-le dans une action (`Button { … }`)",
                    ));
                }
            }
        }
        self.locals.truncate(mark);
        Ok(out)
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
                self.locals.push((Rc::from(p.as_str()), v));
            }
            if let Some(p) = body.params.get(1) {
                self.locals.push((Rc::from(p.as_str()), Val::Int(i as i64)));
            }
            let r = self.body(&body.body);
            self.locals.truncate(mark);
            out.extend(r?);
        }
        Ok(())
    }

    /// Une expression qui décrit une vue.
    pub fn element(&mut self, e: &Expr) -> R<Element> {
        let span = e.span;
        match &e.kind {
            ExprKind::Ident(n) => self.entry_or_view(n, &[], None, None, span),
            ExprKind::Call {
                callee,
                args,
                trailing,
            } => match &callee.kind {
                ExprKind::Ident(n) => self.entry_or_view(n, args, trailing.as_ref(), None, span),
                ExprKind::Member { recv, name } => {
                    let child = self.element(recv)?;
                    self.modifier(child, name, args, trailing.as_ref(), callee.span)
                }
                _ => Err(err(span, "une vue attendue")),
            },
            ExprKind::Member { recv, name } => {
                let child = self.element(recv)?;
                self.modifier(child, name, &[], None, span)
            }
            ExprKind::Ternary { cond, then, els } => {
                if self.bool(cond)? {
                    self.element(then)
                } else {
                    self.element(els)
                }
            }
            _ => Err(err(span, "une vue attendue (`Text(…)`, `VStack { … }`…)")),
        }
    }

    fn modifier(
        &mut self,
        child: Element,
        name: &str,
        args: &[Arg],
        trailing: Option<&Closure>,
        span: Span,
    ) -> R<Element> {
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
        _child: Option<Element>,
        span: Span,
    ) -> R<Element> {
        let shared = self.shared.clone();
        if shared.module.view(name).is_some() {
            if !args.is_empty() || trailing.is_some() {
                return Err(err(
                    span,
                    format!("`{name}` : les paramètres de vue arrivent avec `@Param`"),
                ));
            }
            return Ok((self.views)(name));
        }
        match shared.schema.get(name) {
            Some(entry) if entry.kind == EntryKind::Primitive => {
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
        let mut params = Params::new();
        let mut given: Vec<&str> = Vec::new();
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
            params = params.with(spec.name, value);
        }
        if let Some(c) = trailing {
            let action = entry
                .params
                .iter()
                .find(|p| p.ty == ValueType::Action && !given.contains(&p.name));
            match action {
                Some(p) => {
                    params = params.with(p.name, Value::Action(self.action(c)));
                }
                None if entry.children == Children::Many && entry.kind == EntryKind::Primitive => {
                    children = self.body(&c.body)?;
                }
                None => return Err(err(span, format!("`{}` ne prend pas de `{{ … }}`", entry.name))),
            }
        }
        self.shared
            .schema
            .build(entry.name, params, children)
            .map_err(|e| err(span, e.to_string()))
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
                    self.locals.push((Rc::from(name.as_str()), v));
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

/// `recv.name` ou `recv.name(args)`.
fn member(r: &Val, name: &str, args: Option<Vec<(Option<String>, Val)>>) -> Result<Val, String> {
    match r {
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

/// Les fonctions de base.
fn builtin(name: &str, args: &[(Option<String>, Val)]) -> Result<Val, String> {
    if let Some(p) = gradient(name, args) {
        return p.map(Val::Paint);
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
            let cx = Cx {
                shared: Rc::new(Shared {
                    module: Rc::new(Module::default()),
                    schema: schema.clone(),
                    themes: out.clone(),
                }),
                vars: None,
                pre: Rc::new(Vars::new()),
                locals: Vec::new(),
                views: Rc::new(|_| veka_ui::views::Text("")),
            };
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
    let known = |name: &str| module.view(name).is_some() || schema.get(name).is_some();
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
            ExprKind::Ident(n) if n == "ForEach" || n == "withAnimation" => {
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
                if name == "id" {
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
