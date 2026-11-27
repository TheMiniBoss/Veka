//! [`Program`] : un fichier `.vk` chargé, ses vues, son rechargement à chaud.

use std::cell::RefCell;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use veka_ui::schema::{Params, Schema};
use veka_ui::{Color, Element, State, component_tagged, views};

use crate::ast::Module;
use crate::eval::{Cx, Shared, Vars, themes};
use crate::{Diagnostic, parse};

thread_local! {
    /// La dernière erreur d'une action (les actions n'ont personne à qui la rendre).
    static RUNTIME: RefCell<Option<Diagnostic>> = const { RefCell::new(None) };
}

pub(crate) fn report(d: Diagnostic) {
    RUNTIME.with(|r| *r.borrow_mut() = Some(d));
}

/// Un fichier `.vk` chargé. Bon marché à cloner : les clones partagent le même fichier.
#[derive(Clone)]
pub struct Program {
    inner: Rc<Inner>,
}

struct Inner {
    shared: RefCell<Rc<Shared>>,
    schema: Rc<Schema>,
    errors: RefCell<Vec<Diagnostic>>,
    /// Les états des vues affichées, pour les ré-évaluer après un rechargement.
    live: RefCell<Vec<State<Vars>>>,
    known: RefCell<HashSet<u64>>,
}

/// Lit et construit un fichier : l'arbre et ses thèmes, ou ses erreurs.
fn compile(source: &str, schema: &Rc<Schema>) -> (Rc<Shared>, Vec<Diagnostic>) {
    let (module, mut errors) = parse(source);
    let (themes, theme_errors) = themes(&module, schema);
    errors.extend(theme_errors);
    errors.extend(crate::eval::check(&module, schema));
    let shared = Shared {
        module: Rc::new(module),
        schema: schema.clone(),
        themes,
    };
    (Rc::new(shared), errors)
}

impl Program {
    /// Charge un fichier avec le schéma de Veka. Les erreurs sont dans [`errors`](Self::errors) :
    /// les vues d'un fichier faux affichent leur erreur.
    pub fn new(source: &str) -> Self {
        Self::with_schema(source, Schema::builtin())
    }

    /// Comme [`new`](Self::new), avec un schéma augmenté (vues de l'hôte).
    pub fn with_schema(source: &str, schema: Schema) -> Self {
        let schema = Rc::new(schema);
        let (shared, errors) = compile(source, &schema);
        Self {
            inner: Rc::new(Inner {
                shared: RefCell::new(shared),
                schema,
                errors: RefCell::new(errors),
                live: RefCell::new(Vec::new()),
                known: RefCell::new(HashSet::new()),
            }),
        }
    }

    /// Charge un fichier ; la première erreur s'il en a.
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
        let (shared, errors) = compile(source, &self.inner.schema);
        *self.inner.errors.borrow_mut() = errors.clone();
        if errors.iter().any(Diagnostic::is_error) {
            return Err(errors);
        }
        *self.inner.shared.borrow_mut() = shared;
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
        *self.inner.live.borrow_mut() = keep;
        Ok(())
    }

    /// Les erreurs du dernier chargement.
    pub fn errors(&self) -> Vec<Diagnostic> {
        self.inner.errors.borrow().clone()
    }

    /// La dernière erreur d'une action, effacée à la lecture.
    pub fn take_runtime_error() -> Option<Diagnostic> {
        RUNTIME.with(|r| r.borrow_mut().take())
    }

    /// Le module chargé.
    pub fn module(&self) -> Rc<Module> {
        self.inner.shared.borrow().module.clone()
    }

    /// Les noms des vues du fichier.
    pub fn view_names(&self) -> Vec<String> {
        self.module().views().map(|v| v.name.clone()).collect()
    }

    /// La vue `name` du fichier, comme une vue composée de Veka (avec ses états).
    pub fn view(&self, name: &str) -> Element {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        name.hash(&mut h);
        let program = self.clone();
        let name: Rc<str> = name.into();
        component_tagged(h.finish(), "vk", move || program.render(&name))
    }

    fn render(&self, name: &str) -> Element {
        let shared = self.inner.shared.borrow().clone();
        let Some(decl) = shared.module.view(name) else {
            let names = shared.module.views().map(|v| v.name.as_str());
            let msg = format!(
                "vue `{name}` absente du fichier{}",
                crate::eval::suggest(name, names)
            );
            return error_element(&msg);
        };
        let views: Rc<dyn Fn(&str) -> Element> = {
            let p = self.clone();
            Rc::new(move |n| p.view(n))
        };
        let base = Cx {
            shared: shared.clone(),
            vars: None,
            pre: Rc::new(Vars::new()),
            locals: Vec::new(),
            views,
        };
        // Les valeurs initiales, dans l'ordre : chacune voit les précédentes.
        let init = |only_missing: Option<&Vars>| -> Result<Vars, Diagnostic> {
            let mut pre = Vars::new();
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
                let v = crate::eval::check_type(&s.ty, v, s.init.span)?;
                pre.insert(s.name.clone(), v);
            }
            Ok(pre)
        };
        let mut first_err = None;
        let vars = State::new_with(|| match init(None) {
            Ok(v) => v,
            Err(d) => {
                first_err = Some(d);
                Vars::new()
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
        let mut cx = Cx {
            vars: Some(vars),
            ..base
        };
        match cx.body(&decl.body) {
            Ok(mut items) if items.len() == 1 => items.pop().unwrap_or_else(|| views::Text("")),
            Ok(items) => shared
                .schema
                .build("VStack", Params::new(), items)
                .unwrap_or_else(|e| error_element(&e.to_string())),
            Err(d) => error_element(&d.to_string()),
        }
    }
}

/// Ce qu'affiche une vue fausse : son erreur, en rouge.
pub(crate) fn error_element(msg: &str) -> Element {
    views::Text(msg.to_string())
        .foreground(Color::White)
        .padding(8)
        .background(Color::Red)
}
