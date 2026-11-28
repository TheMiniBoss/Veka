//! [`Program`] : un fichier `.vk` chargé, ses vues, ses données d'hôte, son rechargement à chaud.

use std::cell::RefCell;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use veka_ui::schema::{Params, Schema};
use veka_ui::{Color, Element, State, TimelineState, component_tagged, views};

use crate::ast::{Item, Module, ViewDecl};
use crate::eval::{Cx, ExprKind, Host, Instance, Shared, Vars, build_timeline, check_type, themes};
use crate::{Diagnostic, Val, parse};

thread_local! {
    /// La dernière erreur d'une action (les actions n'ont personne à qui la rendre).
    static RUNTIME: RefCell<Option<Diagnostic>> = const { RefCell::new(None) };
}

pub(crate) fn report(d: Diagnostic) {
    RUNTIME.with(|r| *r.borrow_mut() = Some(d));
}

/// Lit un fichier importé : `import "boutons.vk"` → son texte.
pub type Loader = dyn Fn(&str) -> Result<String, String>;

/// Un fichier `.vk` chargé. Bon marché à cloner : les clones partagent le même fichier.
#[derive(Clone)]
pub struct Program {
    inner: Rc<Inner>,
}

impl std::fmt::Debug for Program {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Program")
            .field("views", &self.view_names())
            .finish()
    }
}

struct Inner {
    shared: RefCell<Rc<Shared>>,
    schema: Rc<Schema>,
    errors: RefCell<Vec<Diagnostic>>,
    /// Les états des vues affichées, pour les ré-évaluer après un rechargement.
    live: RefCell<Vec<State<Vars>>>,
    known: RefCell<HashSet<u64>>,
    host: Rc<Host>,
    loader: Option<Rc<Loader>>,
    imports: RefCell<Vec<String>>,
}

/// Lit et construit un fichier et ses imports : l'arbre et ses thèmes, ou ses erreurs.
fn compile(
    source: &str,
    schema: &Rc<Schema>,
    loader: Option<&Loader>,
) -> (Rc<Shared>, Vec<Diagnostic>, Vec<String>) {
    let (mut module, mut errors) = parse(source);
    let mut imported = Vec::new();
    let mut todo: Vec<(String, crate::Span)> = module.imports().map(|i| (i.path.clone(), i.span)).collect();
    while let Some((path, at)) = todo.pop() {
        if imported.contains(&path) {
            continue;
        }
        imported.push(path.clone());
        let Some(load) = loader else {
            errors.push(Diagnostic::error(
                at,
                format!("`import \"{path}\"` : cet hôte ne lit pas de fichiers"),
            ));
            continue;
        };
        match load(&path) {
            Ok(text) => {
                let (m, e) = parse(&text);
                errors.extend(e.into_iter().map(|d| Diagnostic {
                    message: format!("{path} : {}", d.message),
                    ..d
                }));
                todo.extend(m.imports().map(|i| (i.path.clone(), at)));
                // Le fichier principal l'emporte sur un nom importé.
                for item in m.items {
                    let clash = match &item {
                        Item::View(v) => module.view(&v.name).is_some(),
                        Item::Func(f) => module.func(&f.name).is_some(),
                        Item::Timeline(t) => module.timeline(&t.name).is_some(),
                        Item::Theme(t) => module.themes().any(|x| x.name == t.name),
                        Item::Import(_) => true,
                    };
                    if !clash {
                        module.items.push(item);
                    }
                }
            }
            Err(e) => errors.push(Diagnostic::error(at, format!("`import \"{path}\"` : {e}"))),
        }
    }
    let (themes, theme_errors) = themes(&module, schema);
    errors.extend(theme_errors);
    errors.extend(crate::eval::check(&module, schema));
    let shared = Shared {
        module: Rc::new(module),
        schema: schema.clone(),
        themes,
    };
    (Rc::new(shared), errors, imported)
}

impl Program {
    /// Charge un fichier avec le schéma de Veka. Les erreurs sont dans [`errors`](Self::errors) :
    /// les vues d'un fichier faux affichent leur erreur.
    pub fn new(source: &str) -> Self {
        Self::build(source, Schema::builtin(), None)
    }

    /// Comme [`new`](Self::new), avec un schéma augmenté (vues de l'hôte).
    pub fn with_schema(source: &str, schema: Schema) -> Self {
        Self::build(source, schema, None)
    }

    /// Comme [`with_schema`](Self::with_schema) ; `loader` lit les `import "…"` (le cœur ne lit
    /// aucun fichier lui-même).
    pub fn with_loader(
        source: &str,
        schema: Schema,
        loader: impl Fn(&str) -> Result<String, String> + 'static,
    ) -> Self {
        Self::build(source, schema, Some(Rc::new(loader)))
    }

    fn build(source: &str, schema: Schema, loader: Option<Rc<Loader>>) -> Self {
        let schema = Rc::new(schema);
        let (shared, errors, imports) = compile(source, &schema, loader.as_deref());
        Self {
            inner: Rc::new(Inner {
                shared: RefCell::new(shared),
                schema,
                errors: RefCell::new(errors),
                live: RefCell::new(Vec::new()),
                known: RefCell::new(HashSet::new()),
                host: Rc::new(Host::default()),
                loader,
                imports: RefCell::new(imports),
            }),
        }
    }

    /// Charge un fichier ; ses erreurs s'il en a.
    pub fn load(source: &str) -> Result<Self, Vec<Diagnostic>> {
        let p = Self::new(source);
        let errors = p.errors();
        if errors.iter().any(Diagnostic::is_error) {
            Err(errors)
        } else {
            Ok(p)
        }
    }

    /// Remplace le fichier. S'il est faux, l'ancien reste affiché et ses erreurs sont rendues.
    /// Les états gardent leurs valeurs (retrouvés par nom) ; les nouveaux prennent la leur.
    pub fn reload(&self, source: &str) -> Result<(), Vec<Diagnostic>> {
        let (shared, errors, imports) = compile(source, &self.inner.schema, self.inner.loader.as_deref());
        *self.inner.errors.borrow_mut() = errors.clone();
        *self.inner.imports.borrow_mut() = imports;
        if errors.iter().any(Diagnostic::is_error) {
            return Err(errors);
        }
        *self.inner.shared.borrow_mut() = shared;
        self.refresh();
        Ok(())
    }

    /// Ré-évalue toutes les vues affichées.
    fn refresh(&self) {
        let live: Vec<State<Vars>> = std::mem::take(&mut *self.inner.live.borrow_mut());
        let mut keep = Vec::new();
        for s in live {
            if s.is_alive() {
                s.update(|_| ());
                keep.push(s);
            } else {
                self.inner.known.borrow_mut().remove(&s.to_bits());
            }
        }
        self.inner.live.borrow_mut().extend(keep);
    }

    /// Les données de l'hôte, lues par `model` dans le fichier. Les vues affichées sont ré-évaluées.
    pub fn set_model(&self, model: Val) {
        *self.inner.host.model.borrow_mut() = model;
        self.refresh();
    }

    pub fn model(&self) -> Val {
        self.inner.host.model.borrow().clone()
    }

    /// Reçoit les `emit` qu'aucune vue du fichier n'écoute : `(vue, événement, valeur)`.
    pub fn on_event(&self, f: impl Fn(&str, &str, &Val) + 'static) {
        *self.inner.host.on_event.borrow_mut() = Some(Rc::new(f));
    }

    /// Les erreurs du dernier chargement.
    pub fn errors(&self) -> Vec<Diagnostic> {
        self.inner.errors.borrow().clone()
    }

    /// Les fichiers importés au dernier chargement (à surveiller pour le rechargement à chaud).
    pub fn imports(&self) -> Vec<String> {
        self.inner.imports.borrow().clone()
    }

    /// La dernière erreur d'une action, effacée à la lecture.
    pub fn take_runtime_error() -> Option<Diagnostic> {
        RUNTIME.with(|r| r.borrow_mut().take())
    }

    /// Le module chargé (avec ses imports).
    pub fn module(&self) -> Rc<Module> {
        self.inner.shared.borrow().module.clone()
    }

    /// Les noms des vues du fichier.
    pub fn view_names(&self) -> Vec<String> {
        self.module().views().map(|v| v.name.clone()).collect()
    }

    /// La vue `name` du fichier, comme une vue composée de Veka (avec ses états). Ses `@Param`
    /// prennent leur valeur par défaut.
    pub fn view(&self, name: &str) -> Element {
        self.view_with(name, Instance::default())
    }

    /// La vue `name` avec des valeurs pour ses `@Param`.
    pub fn view_with_params<K: Into<String>>(
        &self,
        name: &str,
        params: impl IntoIterator<Item = (K, Val)>,
    ) -> Element {
        let inst = Instance {
            params: params.into_iter().map(|(k, v)| (k.into(), v)).collect(),
            ..Default::default()
        };
        self.view_with(name, inst)
    }

    pub(crate) fn view_with(&self, name: &str, inst: Instance) -> Element {
        let program = self.clone();
        let name: Rc<str> = name.into();
        let inst = Rc::new(inst);
        component_tagged(tag(&name), "vk", move || program.render(&name, &inst))
    }

    fn render(&self, name: &str, inst: &Instance) -> Element {
        let shared = self.inner.shared.borrow().clone();
        let Some(decl) = shared.module.view(name) else {
            let names = shared.module.views().map(|v| v.name.as_str());
            let msg = format!(
                "vue `{name}` absente du fichier{}",
                crate::eval::suggest(name, names)
            );
            return error_element(&msg);
        };
        let views: crate::eval::ViewFn = {
            let p = self.clone();
            Rc::new(move |n, inst| p.view_with(n, inst))
        };
        let mut base = Cx::root(shared.clone(), self.inner.host.clone(), views);
        base.view = Rc::from(name);
        base.slots = Rc::new(inst.slots.clone());
        base.handlers = Rc::new(inst.handlers.clone());
        // Les paramètres : ceux de l'appelant, sinon leur défaut.
        let mut params = Vars::default();
        for p in &decl.params {
            let v = match inst.params.get(&p.name) {
                Some(v) => v.clone(),
                None => match &p.default {
                    Some(d) => {
                        let cx = Cx {
                            params: Rc::new(params.clone()),
                            ..base.clone()
                        };
                        match cx.eval(d).and_then(|v| check_type(&p.ty, v, d.span)) {
                            Ok(v) => v,
                            Err(e) => return error_element(&e.to_string()),
                        }
                    }
                    None => return error_element(&format!("`{name}` : le paramètre `{}` manque", p.name)),
                },
            };
            params.insert(p.name.clone(), v);
        }
        base.params = Rc::new(params);
        // Les valeurs initiales, dans l'ordre : chacune voit les précédentes.
        let init = |only_missing: Option<&Vars>| -> Result<Vars, Diagnostic> {
            let mut pre = Vars::default();
            for s in &decl.states {
                if let Some(v) = only_missing.and_then(|m| m.get(&s.name)) {
                    pre.insert(s.name.clone(), v.clone());
                    continue;
                }
                let cx = Cx {
                    pre: Rc::new(pre.clone()),
                    ..base.clone()
                };
                let v = cx.eval(&s.init)?;
                let v = check_type(&s.ty, v, s.init.span)?;
                pre.insert(s.name.clone(), v);
            }
            Ok(pre)
        };
        let mut first_err = None;
        let vars = State::new_with(|| match init(None) {
            Ok(v) => v,
            Err(d) => {
                first_err = Some(d);
                Vars::default()
            }
        });
        if let Some(d) = first_err {
            return error_element(&d.to_string());
        }
        if self.inner.known.borrow_mut().insert(vars.to_bits()) {
            self.inner.live.borrow_mut().push(vars);
        }
        // Après un rechargement : les états nouveaux prennent leur valeur initiale.
        let missing = decl
            .states
            .iter()
            .any(|s| !vars.with(|m| m.contains_key(&s.name)));
        if missing {
            match init(Some(&vars.get())) {
                Ok(all) => vars.update(|m| {
                    for (k, v) in all {
                        m.entry(k).or_insert(v);
                    }
                }),
                Err(d) => return error_element(&d.to_string()),
            }
        }
        let cx = Cx {
            vars: Some(vars),
            ..base
        };
        if decl.timelines.is_empty() && decl.clocks.is_empty() {
            return body(cx, decl);
        }
        // Les timelines et horloges sont des états par ordre : une vue intérieure, recréée si leur
        // liste change, les garde (les `@State` restent dans la vue extérieure).
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for t in &decl.timelines {
            t.name.hash(&mut h);
            crate::print_expr(&t.init).hash(&mut h);
        }
        decl.clocks.hash(&mut h);
        let decl = Rc::new(decl.clone());
        component_tagged(h.finish(), "vk-timelines", move || timelines(cx.clone(), &decl))
    }
}

fn tag(name: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut h);
    h.finish()
}

/// Crée les `@Timeline` et `@Clock`, puis évalue le corps.
fn timelines(mut cx: Cx, decl: &ViewDecl) -> Element {
    for t in &decl.timelines {
        let made = match &t.init.kind {
            ExprKind::Call {
                callee,
                args,
                trailing: None,
            } => match &callee.kind {
                ExprKind::Ident(n) => match cx.shared.module.timeline(n) {
                    Some(td) => build_timeline(&cx, td, args),
                    None => Err(Diagnostic::error(callee.span, format!("timeline `{n}` inconnue"))),
                },
                _ => Err(Diagnostic::error(
                    t.init.span,
                    "`@Timeline var p = Nom(…)` attendu",
                )),
            },
            ExprKind::Ident(n) => match cx.shared.module.timeline(n) {
                Some(td) => build_timeline(&cx, td, &[]),
                None => Err(Diagnostic::error(t.init.span, format!("timeline `{n}` inconnue"))),
            },
            _ => Err(Diagnostic::error(
                t.init.span,
                "`@Timeline var p = Nom(…)` attendu",
            )),
        };
        let (timeline, autoplay) = match made {
            Ok(x) => x,
            Err(d) => return error_element(&d.to_string()),
        };
        let tl = if autoplay {
            TimelineState::playing(timeline)
        } else {
            TimelineState::new(timeline)
        };
        cx.locals.push((Rc::from(t.name.as_str()), Val::Timeline(tl)));
    }
    for c in &decl.clocks {
        let tl = TimelineState::playing(veka_anima::Timeline::new().length(1.0e7));
        cx.locals
            .push((Rc::from(c.as_str()), Val::Float(tl.time() as f64)));
    }
    body(cx, decl)
}

/// Le corps d'une vue : un élément, ou une pile verticale s'il y en a plusieurs.
fn body(mut cx: Cx, decl: &ViewDecl) -> Element {
    cx.tl_handlers = Rc::new(RefCell::new(Vec::new()));
    let r = cx.body(&decl.body);
    // Un seul rappel par timeline : il appelle ceux de ses `.onTimelineEvent`.
    let mut handlers = cx.tl_handlers.borrow_mut().drain(..).collect::<Vec<_>>();
    while let Some((tl, _, _)) = handlers.first().cloned() {
        let (mine, rest): (Vec<_>, Vec<_>) = handlers
            .into_iter()
            .partition(|(t, _, _)| t.to_bits() == tl.to_bits());
        handlers = rest;
        tl.on_event(move |name| {
            for (_, filter, h) in &mine {
                if filter.as_deref().is_none_or(|f| f == name) {
                    h(Val::Str(name.into()));
                }
            }
        });
    }
    match r {
        Ok(mut items) if items.len() == 1 => items.pop().unwrap_or_else(|| views::Text("")),
        Ok(items) => cx
            .shared
            .schema
            .build("VStack", Params::new(), items)
            .unwrap_or_else(|e| error_element(&e.to_string())),
        Err(d) => error_element(&d.to_string()),
    }
}

/// Ce qu'affiche une vue fausse : son erreur, en rouge.
pub(crate) fn error_element(msg: &str) -> Element {
    views::Text(msg.to_string())
        .foreground(Color::White)
        .padding(8)
        .background(Color::Red)
}
