//! Le schéma : chaque primitive et chaque modifier de Veka y est déclaré, avec ses paramètres typés.
//!
//! C'est la règle « rien n'est réservé au Rust » (`Docs/VEKA_ETAPE1.md` §5.7) : tout ce que le Rust
//! construit peut aussi être construit **à partir de données** (nom + paramètres), le chemin que
//! prendront les fichiers `.vk`, l'interface C et les autres langages.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

use veka_anima::{Animation, Transition};
use veka_colora::{ColorScheme, Paint, Theme};
use veka_foundation::{Color, WindowControl};
use veka_graphics::{CornerRadii, Shape};
use veka_text::Weight;

use crate::element::{Action, Element};
use crate::env::Font;
use crate::layout::{Alignment, Axis, DEFAULT_SPACER_MIN, DEFAULT_SPACING, Spacer, Stack};
use crate::views;
use crate::window;

/// Une valeur de paramètre.
#[derive(Clone, Debug)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(Rc<str>),
    Color(Color),
    /// Une peinture Colora : couleur, jeton ou dégradé.
    Paint(Paint),
    /// Un thème Colora.
    Theme(Arc<Theme>),
    /// Une valeur d'énumération par son nom (`"title"`, `"leading"`…).
    Enum(Rc<str>),
    Action(Action),
    /// Une spec d'animation Anima (courbe ou ressort).
    Animation(Animation),
    /// Une transition d'entrée et de sortie.
    Transition(Transition),
    /// Un état lu et écrit par la vue (`$nom` en `.vk`), comme un `Binding` de SwiftUI (étape 9).
    Binding(Binding),
}

/// Un état lu et écrit par une vue (`.focused($focus)`, `TextField(text: $nom)`), sous forme de valeurs
/// simples du schéma.
#[derive(Clone)]
pub struct Binding {
    pub get: Rc<dyn Fn() -> Value>,
    pub set: Rc<dyn Fn(Value)>,
}

impl fmt::Debug for Binding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Binding({:?})", (self.get)())
    }
}

impl Value {
    /// L'empreinte d'une valeur simple (pour `animation` et `matchedGeometryEffect`).
    pub fn key(&self) -> u64 {
        match self {
            Value::Bool(b) => views::value_key(&(0u8, *b)),
            // Un entier et le même nombre en flottant ont la même empreinte.
            Value::Int(i) => views::value_key(&(1u8, (*i as f64).to_bits())),
            Value::Float(f) => views::value_key(&(1u8, f.to_bits())),
            Value::String(s) | Value::Enum(s) => views::value_key(&(2u8, &**s)),
            Value::Color(c) => views::value_key(&(3u8, [c.r, c.g, c.b, c.a].map(f32::to_bits))),
            _ => 0,
        }
    }
}

/// Type d'un paramètre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueType {
    Bool,
    Number,
    String,
    Color,
    /// Une peinture : accepte aussi une couleur.
    Paint,
    Theme,
    Enum(&'static [&'static str]),
    Action,
    Animation,
    Transition,
    /// Un état lu et écrit par la vue (`$nom`).
    Binding,
    /// Une valeur simple quelconque (booléen, nombre, texte, couleur), comparée par empreinte.
    Any,
}

/// Un paramètre déclaré.
#[derive(Clone, Copy, Debug)]
pub struct ParamSpec {
    pub name: &'static str,
    pub ty: ValueType,
    pub required: bool,
    pub doc: &'static str,
}

/// Enfants acceptés.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Children {
    None,
    /// Exactement un : l'élément modifié (pour les modifiers).
    One,
    Many,
}

/// Primitive ou modifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Primitive,
    Modifier,
}

/// Paramètres reçus. Les noms du schéma sont `'static` : les passer par [`with_static`](Self::with_static)
/// n'alloue pas.
#[derive(Clone, Debug, Default)]
pub struct Params(pub Vec<(Cow<'static, str>, Value)>);

impl Params {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, name: &str, value: Value) -> Self {
        self.0.push((Cow::Owned(name.to_owned()), value));
        self
    }

    /// Comme [`with`](Self::with), pour un nom du schéma (sans allocation).
    pub fn with_static(mut self, name: &'static str, value: Value) -> Self {
        self.0.push((Cow::Borrowed(name), value));
        self
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.0.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }

    pub fn number(&self, name: &str) -> Option<f32> {
        match self.get(name)? {
            Value::Int(i) => Some(*i as f32),
            Value::Float(f) => Some(*f as f32),
            _ => None,
        }
    }

    pub fn string(&self, name: &str) -> Option<&str> {
        match self.get(name)? {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn bool(&self, name: &str) -> Option<bool> {
        match self.get(name)? {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn enumeration(&self, name: &str) -> Option<&str> {
        match self.get(name)? {
            Value::Enum(s) | Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn color(&self, name: &str) -> Option<Color> {
        match self.get(name)? {
            Value::Color(c) => Some(*c),
            _ => None,
        }
    }

    /// Une peinture (une couleur en est une).
    pub fn paint(&self, name: &str) -> Option<Paint> {
        match self.get(name)? {
            Value::Color(c) => Some(Paint::Solid(*c)),
            Value::Paint(p) => Some(p.clone()),
            _ => None,
        }
    }

    pub fn theme(&self, name: &str) -> Option<Arc<Theme>> {
        match self.get(name)? {
            Value::Theme(t) => Some(t.clone()),
            _ => None,
        }
    }

    pub fn action(&self, name: &str) -> Option<Action> {
        match self.get(name)? {
            Value::Action(a) => Some(a.clone()),
            _ => None,
        }
    }

    pub fn animation(&self, name: &str) -> Option<Animation> {
        match self.get(name)? {
            Value::Animation(a) => Some(*a),
            _ => None,
        }
    }

    pub fn binding(&self, name: &str) -> Option<Binding> {
        match self.get(name)? {
            Value::Binding(b) => Some(b.clone()),
            _ => None,
        }
    }

    /// Une touche : un nom (`"return"`, `".escape"`) ou un caractère (`"s"`).
    pub fn key(&self, name: &str) -> Option<veka_foundation::Key> {
        let text = match self.get(name)? {
            Value::String(s) | Value::Enum(s) => s.clone(),
            _ => return None,
        };
        veka_foundation::Key::from_name(text.strip_prefix('.').unwrap_or(&text))
    }

    pub fn transition(&self, name: &str) -> Option<Transition> {
        match self.get(name)? {
            Value::Transition(t) => Some(t.clone()),
            _ => None,
        }
    }
}

/// Erreur de construction depuis des données.
#[derive(Clone, Debug, PartialEq)]
pub enum SchemaError {
    Unknown(String),
    MissingParam {
        entry: &'static str,
        param: &'static str,
    },
    WrongType {
        entry: &'static str,
        param: String,
    },
    UnknownParam {
        entry: &'static str,
        param: String,
    },
    Children {
        entry: &'static str,
        expected: Children,
        got: usize,
    },
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SchemaError::Unknown(n) => write!(f, "composant ou modifier inconnu : `{n}`"),
            SchemaError::MissingParam { entry, param } => {
                write!(f, "`{entry}` : paramètre `{param}` manquant")
            }
            SchemaError::WrongType { entry, param } => write!(f, "`{entry}` : mauvais type pour `{param}`"),
            SchemaError::UnknownParam { entry, param } => {
                write!(f, "`{entry}` : paramètre inconnu `{param}`")
            }
            SchemaError::Children { entry, expected, got } => {
                write!(f, "`{entry}` : enfants attendus {expected:?}, reçus {got}")
            }
        }
    }
}

impl std::error::Error for SchemaError {}

type BuildFn = fn(&Params, Vec<Element>) -> Element;

/// Une entrée du schéma.
#[derive(Clone)]
pub struct Entry {
    pub name: &'static str,
    pub kind: EntryKind,
    pub params: &'static [ParamSpec],
    pub children: Children,
    pub doc: &'static str,
    pub build: BuildFn,
}

impl fmt::Debug for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Entry")
            .field("name", &self.name)
            .field("kind", &self.kind)
            .finish()
    }
}

/// Le registre.
#[derive(Clone, Debug, Default)]
pub struct Schema {
    entries: HashMap<&'static str, Entry>,
}

impl Schema {
    /// Le schéma avec tous les composants et modifiers de Veka.
    pub fn builtin() -> Self {
        let mut s = Self::default();
        for e in builtin_entries() {
            s.register(e);
        }
        s
    }

    /// Ajoute (ou remplace) une entrée : un composant écrit par l'utilisateur s'enregistre ainsi.
    pub fn register(&mut self, entry: Entry) {
        self.entries.insert(entry.name, entry);
    }

    pub fn get(&self, name: &str) -> Option<&Entry> {
        self.entries.get(name)
    }

    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.values()
    }

    /// Construit un élément à partir de données, en vérifiant paramètres et enfants.
    pub fn build(&self, name: &str, params: Params, children: Vec<Element>) -> Result<Element, SchemaError> {
        let entry = self
            .get(name)
            .ok_or_else(|| SchemaError::Unknown(name.to_owned()))?;
        Self::build_entry(entry, params, children)
    }

    /// Comme [`build`](Self::build), avec l'entrée déjà trouvée (un interpréteur la garde d'une fois sur
    /// l'autre).
    pub fn build_entry(
        entry: &Entry,
        params: Params,
        children: Vec<Element>,
    ) -> Result<Element, SchemaError> {
        for spec in entry.params {
            match params.get(spec.name) {
                None if spec.required => {
                    return Err(SchemaError::MissingParam {
                        entry: entry.name,
                        param: spec.name,
                    });
                }
                None => {}
                Some(v) if !type_matches(spec.ty, v) => {
                    return Err(SchemaError::WrongType {
                        entry: entry.name,
                        param: spec.name.to_owned(),
                    });
                }
                Some(_) => {}
            }
        }
        if let Some((n, _)) = params
            .0
            .iter()
            .find(|(n, _)| !entry.params.iter().any(|p| p.name == n))
        {
            return Err(SchemaError::UnknownParam {
                entry: entry.name,
                param: n.to_string(),
            });
        }
        let ok = match entry.children {
            Children::None => children.is_empty(),
            Children::One => children.len() == 1,
            Children::Many => true,
        };
        if !ok {
            return Err(SchemaError::Children {
                entry: entry.name,
                expected: entry.children,
                got: children.len(),
            });
        }
        Ok((entry.build)(&params, children))
    }
}

fn type_matches(ty: ValueType, v: &Value) -> bool {
    match (ty, v) {
        (ValueType::Bool, Value::Bool(_))
        | (ValueType::Number, Value::Int(_) | Value::Float(_))
        | (ValueType::String, Value::String(_))
        | (ValueType::Color, Value::Color(_))
        | (ValueType::Paint, Value::Color(_) | Value::Paint(_))
        | (ValueType::Theme, Value::Theme(_))
        | (ValueType::Action, Value::Action(_))
        | (ValueType::Animation, Value::Animation(_))
        | (ValueType::Transition, Value::Transition(_))
        | (ValueType::Binding, Value::Binding(_))
        | (
            ValueType::Any,
            Value::Bool(_)
            | Value::Int(_)
            | Value::Float(_)
            | Value::String(_)
            | Value::Enum(_)
            | Value::Color(_),
        ) => true,
        (ValueType::Enum(names), Value::Enum(s) | Value::String(s)) => names.contains(&&**s),
        _ => false,
    }
}

/// Les valeurs de `.scrollIndicators`.
const SCROLL_INDICATORS: &[&str] = &["automatic", "visible", "hidden", "never"];
/// Les valeurs de `.scrollBounceBehavior`.
const SCROLL_BOUNCES: &[&str] = &["always", "automatic", "basedOnSize", "never"];

/// Les styles de `.textFieldStyle`.
const FIELD_STYLES: &[&str] = &["roundedBorder", "plain", "automatic"];

/// Un `TextField` ou un `SecureField` relié à un état du schéma.
fn text_field(p: &Params, secure: bool) -> Element {
    let b = p.binding("text").expect("vérifié par le schéma");
    let get = b.get.clone();
    let set = b.set.clone();
    Element::new(
        views::TextFieldPrim {
            placeholder: p.string("placeholder").unwrap_or_default().to_owned(),
            get: Rc::new(move || match get() {
                Value::String(s) => s.to_string(),
                _ => String::new(),
            }),
            set: Rc::new(move |text: String| {
                if !matches!(&(b.get)(), Value::String(s) if **s == *text) {
                    set(Value::String(Rc::from(text)));
                }
            }),
            secure,
        },
        Vec::new(),
    )
}

/// Le primitive de `.focused` à partir d'un état du schéma.
fn focus_binding(b: Binding, equals: Option<Value>) -> views::FocusBinding {
    let get = b.get.clone();
    match equals {
        // Un booléen.
        None => views::FocusBinding {
            is_on: Rc::new(move || matches!(get(), Value::Bool(true))),
            set: Rc::new(move |on| {
                if !matches!((b.get)(), Value::Bool(v) if v == on) {
                    (b.set)(Value::Bool(on));
                }
            }),
        },
        // Une valeur : l'état la prend avec le focus, et revient à « rien » (texte vide, 0, faux) sans.
        Some(want) => {
            let key = want.key();
            let empty = match want {
                Value::String(_) | Value::Enum(_) => Value::String(Rc::from("")),
                Value::Int(_) | Value::Float(_) => Value::Int(0),
                _ => Value::Bool(false),
            };
            views::FocusBinding {
                is_on: Rc::new(move || get().key() == key),
                set: Rc::new(move |on| {
                    let now = (b.get)().key() == key;
                    if on && !now {
                        (b.set)(want.clone());
                    } else if !on && now {
                        (b.set)(empty.clone());
                    }
                }),
            }
        }
    }
}

/// Les modificateurs d'un raccourci : `.command` par défaut, ou les noms écrits (`[.command, .shift]`).
fn modifiers(v: Option<&Value>) -> veka_foundation::Modifiers {
    use veka_foundation::Modifiers as M;
    let text = match v {
        None => return M::COMMAND,
        Some(Value::String(s) | Value::Enum(s)) => s.clone(),
        Some(_) => return M::COMMAND,
    };
    let mut m = M::NONE;
    for (name, bit) in [
        ("command", M::COMMAND),
        ("shift", M::SHIFT),
        ("option", M::ALT),
        ("control", M::CONTROL),
    ] {
        if text.contains(name) {
            m |= bit;
        }
    }
    m
}

const fn p(name: &'static str, ty: ValueType, required: bool, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty,
        required,
        doc,
    }
}

const ALIGNMENTS: &[&str] = &["leading", "center", "trailing"];
const FONTS: &[&str] = &[
    "largeTitle",
    "title",
    "title2",
    "title3",
    "headline",
    "body",
    "callout",
    "subheadline",
    "footnote",
    "caption",
    "caption2",
];
const WEIGHTS: &[&str] = &[
    "thin",
    "ultraLight",
    "light",
    "regular",
    "medium",
    "semibold",
    "bold",
    "heavy",
    "black",
];

fn alignment(params: &Params) -> Alignment {
    match params.enumeration("alignment") {
        Some("leading") => Alignment::Leading,
        Some("trailing") => Alignment::Trailing,
        _ => Alignment::Center,
    }
}

fn weight(name: &str) -> Weight {
    match name {
        "thin" => Weight::Thin,
        "ultraLight" => Weight::ExtraLight,
        "light" => Weight::Light,
        "medium" => Weight::Medium,
        "semibold" => Weight::SemiBold,
        "bold" => Weight::Bold,
        "heavy" => Weight::ExtraBold,
        "black" => Weight::Black,
        _ => Weight::Regular,
    }
}

const STACK_PARAMS: &[ParamSpec] = &[
    p(
        "spacing",
        ValueType::Number,
        false,
        "espace entre les enfants (8 par défaut)",
    ),
    p(
        "alignment",
        ValueType::Enum(ALIGNMENTS),
        false,
        "alignement sur l'axe croisé",
    ),
];
const NO_PARAMS: &[ParamSpec] = &[];

fn stack(axis: Axis, params: &Params, children: Vec<Element>) -> Element {
    Element::new(
        Stack {
            axis,
            spacing: params.number("spacing").unwrap_or(DEFAULT_SPACING),
            alignment: alignment(params),
        },
        children,
    )
}

fn one(children: Vec<Element>) -> Element {
    children
        .into_iter()
        .next()
        .expect("un enfant (vérifié par le schéma)")
}

fn builtin_entries() -> Vec<Entry> {
    use Children::*;
    use EntryKind::*;
    vec![
        Entry {
            name: "VStack",
            kind: Primitive,
            params: STACK_PARAMS,
            children: Many,
            doc: "pile verticale",
            build: |p, c| stack(Axis::Vertical, p, c),
        },
        Entry {
            name: "HStack",
            kind: Primitive,
            params: STACK_PARAMS,
            children: Many,
            doc: "pile horizontale",
            build: |p, c| stack(Axis::Horizontal, p, c),
        },
        Entry {
            name: "ZStack",
            kind: Primitive,
            params: STACK_PARAMS,
            children: Many,
            doc: "enfants superposés",
            build: |p, c| stack(Axis::Depth, p, c),
        },
        Entry {
            name: "Spacer",
            kind: Primitive,
            children: None,
            doc: "espace flexible",
            params: const { &[p("minLength", ValueType::Number, false, "longueur minimale")] },
            build: |p, _| {
                Element::new(
                    Spacer {
                        min_length: p.number("minLength").unwrap_or(DEFAULT_SPACER_MIN),
                    },
                    vec![],
                )
            },
        },
        Entry {
            name: "Text",
            kind: Primitive,
            children: None,
            doc: "du texte",
            params: const { &[p("content", ValueType::String, true, "le texte")] },
            build: |p, _| views::Text(p.string("content").unwrap_or_default()),
        },
        Entry {
            name: "Rectangle",
            kind: Primitive,
            params: NO_PARAMS,
            children: None,
            doc: "rectangle",
            build: |_, _| views::Rectangle(),
        },
        Entry {
            name: "RoundedRectangle",
            kind: Primitive,
            children: None,
            doc: "rectangle aux coins arrondis",
            params: const { &[p("cornerRadius", ValueType::Number, true, "rayon des coins")] },
            build: |p, _| {
                views::UnevenRoundedRectangle(CornerRadii::all(p.number("cornerRadius").unwrap_or(0.0)))
            },
        },
        Entry {
            name: "Capsule",
            kind: Primitive,
            params: NO_PARAMS,
            children: None,
            doc: "capsule",
            build: |_, _| views::Capsule(),
        },
        Entry {
            name: "Circle",
            kind: Primitive,
            params: NO_PARAMS,
            children: None,
            doc: "cercle",
            build: |_, _| views::Circle(),
        },
        Entry {
            name: "Ellipse",
            kind: Primitive,
            params: NO_PARAMS,
            children: None,
            doc: "ellipse",
            build: |_, _| views::Ellipse(),
        },
        Entry {
            name: "Button",
            kind: Primitive,
            children: Many,
            doc: "bouton (libellé `title` ou un enfant)",
            params: const {
                &[
                    p("title", ValueType::String, false, "libellé"),
                    p("action", ValueType::Action, true, "action au clic"),
                ]
            },
            build: |p, c| {
                let action = p.action("action").expect("vérifié par le schéma");
                let label = match p.string("title") {
                    Some(t) => views::Text(t),
                    Option::None => c.into_iter().next().unwrap_or_else(|| views::Text("")),
                };
                Element::new(views::ButtonPrim { action }, vec![label])
            },
        },
        Entry {
            name: "TextField",
            kind: Primitive,
            children: None,
            doc: "champ de texte d'une ligne relié à un état (`TextField(\"Nom\", text: $nom)`)",
            params: const {
                &[
                    p(
                        "placeholder",
                        ValueType::String,
                        false,
                        "texte affiché quand le champ est vide",
                    ),
                    p("text", ValueType::Binding, true, "état relié (`$nom`)"),
                ]
            },
            build: |p, _| text_field(p, false),
        },
        Entry {
            name: "SecureField",
            kind: Primitive,
            children: None,
            doc: "champ de mot de passe : des points à la place des caractères, sans copier",
            params: const {
                &[
                    p(
                        "placeholder",
                        ValueType::String,
                        false,
                        "texte affiché quand le champ est vide",
                    ),
                    p("text", ValueType::Binding, true, "état relié (`$motDePasse`)"),
                ]
            },
            build: |p, _| text_field(p, true),
        },
        Entry {
            name: "ScrollView",
            kind: Primitive,
            children: Many,
            doc: "vue qui défile : `.vertical` (par défaut), `.horizontal` ou `[.horizontal, .vertical]`",
            params: const {
                &[
                    p("axes", ValueType::Any, false, "axes qui défilent"),
                    p("showsIndicators", ValueType::Bool, false, "barres de défilement (vrai par défaut)"),
                ]
            },
            build: |p, c| {
                let axes = match p.get("axes") {
                    Some(Value::String(s) | Value::Enum(s)) => views::ScrollAxes {
                        horizontal: s.contains("horizontal"),
                        vertical: s.contains("vertical") || !s.contains("horizontal"),
                    },
                    _ => views::ScrollAxes::VERTICAL,
                };
                // Plusieurs enfants : une pile dans le sens du défilement, comme SwiftUI.
                let content = if c.len() == 1 {
                    one(c)
                } else {
                    let axis = if axes.vertical { Axis::Vertical } else { Axis::Horizontal };
                    Element::new(
                        Stack {
                            axis,
                            spacing: DEFAULT_SPACING,
                            alignment: Alignment::Center,
                        },
                        c,
                    )
                };
                let sv = views::scroll_view(axes, content);
                if p.bool("showsIndicators") == Some(false) {
                    sv.scroll_indicators(views::ScrollIndicators::Hidden)
                } else {
                    sv
                }
            },
        },
        // --- Modifiers ---
        Entry {
            name: "padding",
            kind: Modifier,
            children: One,
            doc: "marges",
            params: const {
                &[
                    p("all", ValueType::Number, false, "les quatre côtés"),
                    p("horizontal", ValueType::Number, false, "gauche et droite"),
                    p("vertical", ValueType::Number, false, "haut et bas"),
                ]
            },
            build: |p, c| {
                let all = p.number("all").unwrap_or(0.0);
                one(c).padding_hv(
                    p.number("horizontal").unwrap_or(all),
                    p.number("vertical").unwrap_or(all),
                )
            },
        },
        Entry {
            name: "frame",
            kind: Modifier,
            children: One,
            doc: "taille imposée",
            params: const {
                &[
                    p("width", ValueType::Number, false, "largeur"),
                    p("height", ValueType::Number, false, "hauteur"),
                    p(
                        "alignment",
                        ValueType::Enum(ALIGNMENTS),
                        false,
                        "alignement du contenu",
                    ),
                ]
            },
            build: |p, c| {
                one(c).wrap(crate::layout::Frame {
                    width: p.number("width"),
                    height: p.number("height"),
                    alignment: alignment(p),
                })
            },
        },
        Entry {
            name: "fill",
            kind: Modifier,
            children: One,
            doc: "remplissage d'une forme",
            params: const { &[p("paint", ValueType::Paint, true, "couleur, jeton ou dégradé")] },
            build: |p, c| one(c).fill(p.paint("paint").unwrap_or_default()),
        },
        Entry {
            name: "stroke",
            kind: Modifier,
            children: One,
            doc: "contour d'une forme",
            params: const {
                &[
                    p("paint", ValueType::Paint, true, "couleur, jeton ou dégradé"),
                    p("lineWidth", ValueType::Number, false, "épaisseur"),
                ]
            },
            build: |p, c| {
                one(c).stroke(
                    p.paint("paint").unwrap_or_default(),
                    p.number("lineWidth").unwrap_or(1.0),
                )
            },
        },
        Entry {
            name: "foregroundColor",
            kind: Modifier,
            children: One,
            doc: "couleur du texte et des formes",
            params: const { &[p("paint", ValueType::Paint, true, "couleur, jeton ou dégradé")] },
            build: |p, c| one(c).foreground(p.paint("paint").unwrap_or_default()),
        },
        Entry {
            name: "tint",
            kind: Modifier,
            children: One,
            doc: "couleur d'accent",
            params: const { &[p("paint", ValueType::Paint, true, "couleur, jeton ou dégradé")] },
            build: |p, c| one(c).tint(p.paint("paint").unwrap_or_default()),
        },
        Entry {
            name: "font",
            kind: Modifier,
            children: One,
            doc: "style de texte",
            params: const { &[p("style", ValueType::Enum(FONTS), true, "style")] },
            build: |p, c| {
                one(c).font(
                    p.enumeration("style")
                        .and_then(Font::from_name)
                        .unwrap_or(Font::Body),
                )
            },
        },
        Entry {
            name: "fontWeight",
            kind: Modifier,
            children: One,
            doc: "graisse",
            params: const { &[p("weight", ValueType::Enum(WEIGHTS), true, "graisse")] },
            build: |p, c| one(c).font_weight(weight(p.enumeration("weight").unwrap_or("regular"))),
        },
        Entry {
            name: "background",
            kind: Modifier,
            children: One,
            doc: "fond",
            params: const {
                &[
                    p("paint", ValueType::Paint, true, "couleur, jeton ou dégradé"),
                    p(
                        "shape",
                        ValueType::Enum(BACKGROUND_SHAPES),
                        false,
                        "forme du fond (rectangle par défaut)",
                    ),
                ]
            },
            build: |p, c| {
                let paint = p.paint("paint").unwrap_or_default();
                let shape = match p.enumeration("shape") {
                    Some("capsule") => Shape::Capsule,
                    Some("circle") => Shape::Circle,
                    Some("ellipse") => Shape::Ellipse,
                    _ => Shape::Rectangle,
                };
                one(c).background_shape(paint, shape)
            },
        },
        Entry {
            name: "theme",
            kind: Modifier,
            children: One,
            doc: "thème Colora du sous-arbre",
            params: const { &[p("theme", ValueType::Theme, true, "thème")] },
            build: |p, c| match p.theme("theme") {
                Some(t) => one(c).theme(t),
                Option::None => one(c),
            },
        },
        Entry {
            name: "colorScheme",
            kind: Modifier,
            children: One,
            doc: "force le mode clair ou sombre du sous-arbre",
            params: const { &[p("scheme", ValueType::Enum(SCHEMES), true, "mode")] },
            build: |p, c| {
                // `system` : comme le parent (rien n'est imposé).
                match p.enumeration("scheme") {
                    Some("dark") => one(c).color_scheme(ColorScheme::Dark),
                    Some("system") => one(c),
                    _ => one(c).color_scheme(ColorScheme::Light),
                }
            },
        },
        Entry {
            name: "preferredColorScheme",
            kind: Modifier,
            children: One,
            doc: "mode clair ou sombre de toute la fenêtre, barre de titre comprise",
            params: const {
                &[p(
                    "scheme",
                    ValueType::Enum(SCHEMES),
                    true,
                    "mode (system : celui du système)",
                )]
            },
            build: |p, c| {
                let scheme = match p.enumeration("scheme") {
                    Some("dark") => Some(ColorScheme::Dark),
                    Some("light") => Some(ColorScheme::Light),
                    _ => Option::None,
                };
                one(c).preferred_color_scheme(scheme)
            },
        },
        adjust("brightness", "ajoute la valeur (-1 à 1) à chaque composante"),
        adjust("contrast", "contraste (1 = inchangé)"),
        adjust("saturation", "saturation (0 = gris, 1 = inchangé)"),
        adjust("hueRotation", "rotation de teinte, en degrés"),
        adjust("grayscale", "niveaux de gris (0 = inchangé, 1 = gris)"),
        // --- Anima ---
        Entry {
            name: "opacity",
            kind: Modifier,
            children: One,
            doc: "opacité (0 à 1), animable",
            params: const { &[p("amount", ValueType::Number, true, "opacité")] },
            build: |p, c| one(c).opacity(p.number("amount").unwrap_or(1.0)),
        },
        Entry {
            name: "offset",
            kind: Modifier,
            children: One,
            doc: "décale le dessin sans changer le layout, animable",
            params: const {
                &[
                    p("x", ValueType::Number, false, "vers la droite"),
                    p("y", ValueType::Number, false, "vers le bas"),
                ]
            },
            build: |p, c| one(c).offset(p.number("x").unwrap_or(0.0), p.number("y").unwrap_or(0.0)),
        },
        Entry {
            name: "scaleEffect",
            kind: Modifier,
            children: One,
            doc: "agrandit le dessin autour de son centre, animable",
            params: const {
                &[
                    p("scale", ValueType::Number, false, "les deux axes"),
                    p("x", ValueType::Number, false, "horizontal"),
                    p("y", ValueType::Number, false, "vertical"),
                ]
            },
            build: |p, c| {
                let s = p.number("scale").unwrap_or(1.0);
                one(c).scale_effect_xy(p.number("x").unwrap_or(s), p.number("y").unwrap_or(s))
            },
        },
        Entry {
            name: "rotationEffect",
            kind: Modifier,
            children: One,
            doc: "tourne le dessin autour de son centre (degrés), animable",
            params: const { &[p("degrees", ValueType::Number, true, "angle, sens horaire")] },
            build: |p, c| one(c).rotation_effect(p.number("degrees").unwrap_or(0.0)),
        },
        Entry {
            name: "animation",
            kind: Modifier,
            children: One,
            doc: "anime le sous-arbre quand `value` change",
            params: const {
                &[
                    p(
                        "spec",
                        ValueType::Animation,
                        false,
                        "l'animation (absente : ne pas animer)",
                    ),
                    p("value", ValueType::Any, true, "la valeur surveillée"),
                ]
            },
            build: |p, c| {
                let key = p.get("value").map_or(0, Value::key);
                one(c).wrap(views::AnimationPrim {
                    spec: p.animation("spec"),
                    value: key,
                })
            },
        },
        Entry {
            name: "transition",
            kind: Modifier,
            children: One,
            doc: "entrée et sortie de la vue pendant une animation",
            params: const { &[p("transition", ValueType::Transition, true, "la transition")] },
            build: |p, c| one(c).transition(p.transition("transition").unwrap_or_default()),
        },
        Entry {
            name: "matchedGeometryEffect",
            kind: Modifier,
            children: One,
            doc: "la vue part de la place d'une vue de même `id` qui disparaît",
            params: const { &[p("id", ValueType::Any, true, "l'identifiant partagé")] },
            build: |p, c| {
                let key = p.get("id").map_or(0, Value::key);
                one(c).wrap(views::MatchedGeometryPrim { key })
            },
        },
        Entry {
            name: "backdropBlur",
            kind: Modifier,
            children: One,
            doc: "floute ce qui est dessiné derrière la vue",
            params: BACKDROP_PARAMS,
            build: |p, c| one(c).backdrop(backdrop_params(p, crate::views::BackdropBlur::default())),
        },
        Entry {
            name: "frostedGlass",
            kind: Modifier,
            children: One,
            doc: "verre dépoli : flou fort, teinte du fond de la fenêtre, un peu de grain",
            params: BACKDROP_PARAMS,
            build: |p, c| one(c).backdrop(backdrop_params(p, crate::views::BackdropBlur::frosted())),
        },
        Entry {
            name: "glass",
            kind: Modifier,
            children: One,
            doc: "verre liquide (façon Liquid Glass 27) ; dans un glassGroup, donne sa forme au groupe",
            params: GLASS_PARAMS,
            build: |p, c| one(c).glass_with(glass_params(p)),
        },
        Entry {
            name: "glassGroup",
            kind: Modifier,
            children: One,
            doc: "un seul verre pour les glass du sous-arbre, qui fusionnent comme des gouttes",
            params: GLASS_GROUP_PARAMS,
            build: |p, c| one(c).glass_group_with(p.number("smoothness").unwrap_or(16.0), glass_params(p)),
        },
        Entry {
            name: "blendMode",
            kind: Modifier,
            children: One,
            doc: "mélange la vue avec ce qui est derrière (multiply, screen, overlay…)",
            params: const {
                &[p(
                    "mode",
                    ValueType::Enum(&veka_graphics::BlendMode::NAMES),
                    true,
                    "le mode de fusion",
                )]
            },
            build: |p, c| {
                let mode = p
                    .enumeration("mode")
                    .and_then(veka_graphics::BlendMode::from_name)
                    .unwrap_or_default();
                one(c).blend_mode(mode)
            },
        },
        Entry {
            name: "compositingGroup",
            kind: Modifier,
            children: One,
            doc: "dessine le sous-arbre à part : une opacité ensuite s'applique d'un bloc",
            params: &[],
            build: |_, c| one(c).compositing_group(),
        },
        Entry {
            name: "liquidDistortion",
            kind: Modifier,
            children: One,
            doc: "le fond ondule dans la forme (eau, chaleur)",
            params: const {
                &[
                    p(
                        "strength",
                        ValueType::Number,
                        false,
                        "déplacement du fond, en points (8)",
                    ),
                    p(
                        "scale",
                        ValueType::Number,
                        false,
                        "taille des vagues, en points (120)",
                    ),
                    p("speed", ValueType::Number, false, "1 = normal, 0 = figé"),
                    p("shape", ValueType::Enum(BACKDROP_SHAPES), false, "forme"),
                    p("cornerRadius", ValueType::Number, false, "rayon des coins"),
                ]
            },
            build: |p, c| {
                one(c).liquid_distortion(
                    p.number("strength").unwrap_or(8.0),
                    p.number("scale").unwrap_or(120.0),
                    p.number("speed").unwrap_or(1.0),
                    shape_param(p).unwrap_or(Shape::Rectangle),
                )
            },
        },
        Entry {
            name: "ignoreReduceMotion",
            kind: Modifier,
            children: One,
            doc: "s'anime même si le système demande de réduire les animations",
            params: NO_PARAMS,
            build: |_, c| one(c).ignore_reduce_motion(),
        },
        Entry {
            name: "onAnimationEnd",
            kind: Modifier,
            children: One,
            doc: "action quand les animations du sous-arbre sont finies",
            params: const { &[p("action", ValueType::Action, true, "action")] },
            build: |p, c| {
                let a = p.action("action").expect("vérifié par le schéma");
                one(c).wrap(views::AnimationEndPrim { action: a })
            },
        },
        Entry {
            name: "onTapGesture",
            kind: Modifier,
            children: One,
            doc: "action au clic",
            params: const { &[p("action", ValueType::Action, true, "action")] },
            build: |p, c| {
                let a = p.action("action").expect("vérifié par le schéma");
                one(c).wrap(views::TapGesture { action: a })
            },
        },
        Entry {
            name: "focusable",
            kind: Modifier,
            children: One,
            doc: "la vue prend le focus du clavier (clic, Tab)",
            params: NO_PARAMS,
            build: |_, c| one(c).focusable(),
        },
        Entry {
            name: "focused",
            kind: Modifier,
            children: One,
            doc: "relie le focus à un état : un booléen, ou une valeur avec `equals`",
            params: const {
                &[
                    p("binding", ValueType::Binding, true, "état relié (`$focus`)"),
                    p(
                        "equals",
                        ValueType::Any,
                        false,
                        "valeur de l'état quand ce sous-arbre a le focus",
                    ),
                ]
            },
            build: |p, c| {
                let b = p.binding("binding").expect("vérifié par le schéma");
                one(c).wrap(focus_binding(b, p.get("equals").cloned()))
            },
        },
        Entry {
            name: "scrollIndicators",
            kind: Modifier,
            children: One,
            doc: "barres des vues qui défilent : `.automatic`, `.visible` ou `.hidden`",
            params: const { &[p("visibility", ValueType::Enum(SCROLL_INDICATORS), true, "visibilité")] },
            build: |p, c| {
                let v = match p.enumeration("visibility") {
                    Some("visible") => views::ScrollIndicators::Visible,
                    Some("hidden" | "never") => views::ScrollIndicators::Hidden,
                    _ => views::ScrollIndicators::Automatic,
                };
                one(c).scroll_indicators(v)
            },
        },
        Entry {
            name: "scrollDisabled",
            kind: Modifier,
            children: One,
            doc: "les vues qui défilent ne bougent plus",
            params: const { &[p("disabled", ValueType::Bool, false, "vrai par défaut")] },
            build: |p, c| one(c).scroll_disabled(p.bool("disabled").unwrap_or(true)),
        },
        Entry {
            name: "scrollBounceBehavior",
            kind: Modifier,
            children: One,
            doc: "bords élastiques : `.always` (par défaut), `.basedOnSize` ou `.never`",
            params: const { &[p("behavior", ValueType::Enum(SCROLL_BOUNCES), true, "comportement")] },
            build: |p, c| {
                let v = match p.enumeration("behavior") {
                    Some("basedOnSize") => views::ScrollBounce::BasedOnSize,
                    Some("never") => views::ScrollBounce::Never,
                    _ => views::ScrollBounce::Always,
                };
                one(c).scroll_bounce(v)
            },
        },
        Entry {
            name: "onSubmit",
            kind: Modifier,
            children: One,
            doc: "action à l'appui sur Entrée dans un champ de texte du sous-arbre",
            params: const { &[p("action", ValueType::Action, true, "action")] },
            build: |p, c| {
                let a = p.action("action").expect("vérifié par le schéma");
                one(c).wrap(views::FieldEnv::OnSubmit(a))
            },
        },
        Entry {
            name: "textFieldStyle",
            kind: Modifier,
            children: One,
            doc: "style des champs de texte du sous-arbre : `.roundedBorder` (par défaut) ou `.plain`",
            params: const { &[p("style", ValueType::Enum(FIELD_STYLES), true, "style")] },
            build: |p, c| {
                let plain = p.enumeration("style") == Some("plain");
                one(c).wrap(views::FieldEnv::Plain(plain))
            },
        },
        Entry {
            name: "onKeyPress",
            kind: Modifier,
            children: One,
            doc: "action à l'appui d'une touche (`.return`, `.escape`, `\"r\"`…), qui la prend",
            params: const {
                &[
                    p("key", ValueType::Any, true, "touche"),
                    p("action", ValueType::Action, true, "action"),
                ]
            },
            build: |p, c| {
                let a = p.action("action").expect("vérifié par le schéma");
                let Some(key) = p.key("key") else { return one(c) };
                one(c).wrap(views::KeyPressPrim {
                    key: Some(key),
                    action: views::key_handled(a),
                })
            },
        },
        Entry {
            name: "keyboardShortcut",
            kind: Modifier,
            children: One,
            doc: "déclenche le bouton au clavier (`.defaultAction`, `.cancelAction`, ou une touche et ses modificateurs)",
            params: const {
                &[
                    p(
                        "key",
                        ValueType::Any,
                        true,
                        "touche, `.defaultAction` ou `.cancelAction`",
                    ),
                    p(
                        "modifiers",
                        ValueType::Any,
                        false,
                        "`.command` (par défaut), `.shift`, `.option`, `.control`, `[]`",
                    ),
                ]
            },
            build: |p, c| {
                let name = match p.get("key") {
                    Some(Value::String(s) | Value::Enum(s)) => s.trim_start_matches('.').to_owned(),
                    _ => String::new(),
                };
                let (key, modifiers) = match name.as_str() {
                    "defaultAction" => (
                        Some(veka_foundation::Key::Enter),
                        veka_foundation::Modifiers::NONE,
                    ),
                    "cancelAction" => (
                        Some(veka_foundation::Key::Escape),
                        veka_foundation::Modifiers::NONE,
                    ),
                    _ => (p.key("key"), modifiers(p.get("modifiers"))),
                };
                match key {
                    Some(key) => one(c).keyboard_shortcut(key, modifiers),
                    Option::None => one(c),
                }
            },
        },
        Entry {
            name: "windowDragArea",
            kind: Modifier,
            children: One,
            doc: "la zone déplace la fenêtre",
            params: NO_PARAMS,
            build: |_, c| one(c).window_drag_area(),
        },
        Entry {
            name: "windowControl",
            kind: Modifier,
            children: One,
            doc: "la vue devient un bouton de la fenêtre",
            params: const { &[p("control", ValueType::Enum(CONTROLS), true, "bouton")] },
            build: |p, c| one(c).window_control(control(p)),
        },
        Entry {
            name: "WindowButton",
            kind: Primitive,
            children: None,
            doc: "bouton de fenêtre dessiné par Veka",
            params: const {
                &[
                    p("control", ValueType::Enum(CONTROLS), true, "bouton"),
                    p(
                        "style",
                        ValueType::Enum(TITLE_STYLES),
                        false,
                        "style (celui du système par défaut)",
                    ),
                ]
            },
            build: |p, _| window::WindowButtonWith(control(p), title_style(p)),
        },
        Entry {
            name: "TitleBar",
            kind: Primitive,
            children: None,
            doc: "barre de titre prête à l'emploi",
            params: const {
                &[
                    p("title", ValueType::String, true, "titre"),
                    p(
                        "style",
                        ValueType::Enum(TITLE_STYLES),
                        false,
                        "style (celui du système par défaut)",
                    ),
                ]
            },
            build: |p, _| window::TitleBarWith(p.string("title").unwrap_or_default(), title_style(p)),
        },
    ]
}

const SCHEMES: &[&str] = &["light", "dark", "system"];
const BACKGROUND_SHAPES: &[&str] = &["rectangle", "capsule", "circle", "ellipse"];

const BACKDROP_SHAPES: &[&str] = &["rectangle", "roundedRectangle", "capsule", "circle", "ellipse"];

const BACKDROP_PARAMS: &[ParamSpec] = &[
    p("radius", ValueType::Number, false, "rayon du flou, en points"),
    p("shape", ValueType::Enum(BACKDROP_SHAPES), false, "forme découpée"),
    p(
        "cornerRadius",
        ValueType::Number,
        false,
        "rayon des coins (roundedRectangle)",
    ),
    p("tint", ValueType::Paint, false, "teinte mélangée au flou"),
    p(
        "tintAmount",
        ValueType::Number,
        false,
        "part de la teinte, de 0 à 1",
    ),
    p("saturation", ValueType::Number, false, "1 = inchangée"),
    p("noise", ValueType::Number, false, "grain, de 0 à 1"),
];

const GLASS_STYLES: &[&str] = &["regular", "clear"];

macro_rules! glass_params {
    ($($extra:expr),*) => {
        &[
            $($extra,)*
            p("style", ValueType::Enum(GLASS_STYLES), false, "regular (par défaut) ou clear"),
            p("clarity", ValueType::Number, false, "de 0 (teinté) à 1 (clair)"),
            p("tint", ValueType::Paint, false, "teinte"),
            p("interactive", ValueType::Bool, false, "le reflet suit le pointeur"),
            p("shape", ValueType::Enum(BACKDROP_SHAPES), false, "forme (capsule par défaut)"),
            p("cornerRadius", ValueType::Number, false, "rayon des coins"),
            p("refraction", ValueType::Number, false, "déplacement du fond au bord, en points"),
            p("bezel", ValueType::Number, false, "largeur du bord courbe, en points"),
            p("aberration", ValueType::Number, false, "séparation des couleurs au bord, de 0 à 1"),
            p("highlight", ValueType::Number, false, "reflet du bord, de 0 à 1"),
            p("shadow", ValueType::Number, false, "ombre portée, de 0 à 1"),
        ]
    };
}

const GLASS_PARAMS: &[ParamSpec] = glass_params!();
const GLASS_GROUP_PARAMS: &[ParamSpec] = glass_params!(p(
    "smoothness",
    ValueType::Number,
    false,
    "distance de fusion, en points (16)"
));

/// La forme donnée par `shape` et `cornerRadius`.
fn shape_param(p: &Params) -> Option<Shape> {
    let corner = p.number("cornerRadius");
    match p.enumeration("shape") {
        Some("rectangle") => Some(Shape::Rectangle),
        Some("capsule") => Some(Shape::Capsule),
        Some("circle") => Some(Shape::Circle),
        Some("ellipse") => Some(Shape::Ellipse),
        Some("roundedRectangle") => Some(Shape::RoundedRectangle(CornerRadii::all(corner.unwrap_or(16.0)))),
        _ => corner.map(|r| Shape::RoundedRectangle(CornerRadii::all(r))),
    }
}

/// Le matériau d'un verre (`glass`, `glassGroup`).
fn glass_params(p: &Params) -> crate::views::Glass {
    let mut g = match p.enumeration("style") {
        Some("clear") => crate::views::Glass::clear(),
        _ => crate::views::Glass::regular(),
    };
    if let Some(v) = p.number("clarity") {
        g.clarity = v;
    }
    if let Some(t) = p.paint("tint") {
        g.tint = Some(t);
    }
    if let Some(Value::Bool(b)) = p.get("interactive") {
        g.interactive = *b;
    }
    g.shape = shape_param(p).or(g.shape);
    if let Some(v) = p.number("refraction") {
        g.refraction = v;
    }
    if let Some(v) = p.number("bezel") {
        g.bezel = v;
    }
    if let Some(v) = p.number("aberration") {
        g.aberration = v;
    }
    if let Some(v) = p.number("highlight") {
        g.highlight = v;
    }
    if let Some(v) = p.number("shadow") {
        g.shadow = v;
    }
    g
}

/// Les réglages donnés par-dessus `base` (`backdropBlur`, `frostedGlass`).
fn backdrop_params(p: &Params, base: crate::views::BackdropBlur) -> crate::views::BackdropBlur {
    let mut b = base;
    if let Some(r) = p.number("radius") {
        b.radius = r;
    }
    if let Some(shape) = shape_param(p) {
        b.shape = shape;
    }
    if let Some(t) = p.paint("tint") {
        b.tint = t;
    }
    if let Some(a) = p.number("tintAmount") {
        b.tint_amount = a;
    }
    if let Some(v) = p.number("saturation") {
        b.saturation = v;
    }
    if let Some(v) = p.number("noise") {
        b.noise = v;
    }
    b
}

/// Un réglage de couleur : un seul paramètre `amount`.
fn adjust(name: &'static str, doc: &'static str) -> Entry {
    // Le schéma veut une `fn` sans capture : chaque réglage a la sienne.
    let build: BuildFn = match name {
        "brightness" => |p, c| one(c).brightness(p.number("amount").unwrap_or(0.0)),
        "contrast" => |p, c| one(c).contrast(p.number("amount").unwrap_or(1.0)),
        "saturation" => |p, c| one(c).saturation(p.number("amount").unwrap_or(1.0)),
        "hueRotation" => |p, c| one(c).hue_rotation(p.number("amount").unwrap_or(0.0)),
        _ => |p, c| one(c).grayscale(p.number("amount").unwrap_or(1.0)),
    };
    Entry {
        name,
        kind: EntryKind::Modifier,
        children: Children::One,
        doc,
        params: const { &[p("amount", ValueType::Number, true, "valeur")] },
        build,
    }
}

const CONTROLS: &[&str] = &["close", "minimize", "maximize", "fullscreen"];
const TITLE_STYLES: &[&str] = &["windows", "mac"];

fn control(p: &Params) -> WindowControl {
    p.enumeration("control")
        .and_then(WindowControl::from_name)
        .unwrap_or(WindowControl::Close)
}

fn title_style(p: &Params) -> window::TitleBarStyle {
    p.enumeration("style")
        .and_then(window::TitleBarStyle::from_name)
        .unwrap_or(window::TitleBarStyle::native())
}
