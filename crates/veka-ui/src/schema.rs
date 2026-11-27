//! Le schéma : chaque primitive et chaque modifier de Veka y est déclaré, avec ses paramètres typés.
//!
//! C'est la règle « rien n'est réservé au Rust » (`Docs/VEKA_ETAPE1.md` §5.7) : tout ce que le Rust
//! construit peut aussi être construit **à partir de données** (nom + paramètres), le chemin que
//! prendront les fichiers `.vk`, l'interface C et les autres langages.

use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use veka_foundation::{Color, WindowControl};
use veka_graphics::CornerRadii;
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
    /// Une valeur d'énumération par son nom (`"title"`, `"leading"`…).
    Enum(Rc<str>),
    Action(Action),
}

/// Type d'un paramètre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueType {
    Bool,
    Number,
    String,
    Color,
    Enum(&'static [&'static str]),
    Action,
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

/// Paramètres reçus.
#[derive(Clone, Debug, Default)]
pub struct Params(pub Vec<(String, Value)>);

impl Params {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, name: &str, value: Value) -> Self {
        self.0.push((name.to_owned(), value));
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

    pub fn action(&self, name: &str) -> Option<Action> {
        match self.get(name)? {
            Value::Action(a) => Some(a.clone()),
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
                param: n.clone(),
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
        | (ValueType::Action, Value::Action(_)) => true,
        (ValueType::Enum(names), Value::Enum(s) | Value::String(s)) => names.contains(&&**s),
        _ => false,
    }
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
            params: const { &[p("color", ValueType::Color, true, "couleur")] },
            build: |p, c| one(c).fill(p.color("color").unwrap_or_default()),
        },
        Entry {
            name: "stroke",
            kind: Modifier,
            children: One,
            doc: "contour d'une forme",
            params: const {
                &[
                    p("color", ValueType::Color, true, "couleur"),
                    p("lineWidth", ValueType::Number, false, "épaisseur"),
                ]
            },
            build: |p, c| {
                one(c).stroke(
                    p.color("color").unwrap_or_default(),
                    p.number("lineWidth").unwrap_or(1.0),
                )
            },
        },
        Entry {
            name: "foregroundColor",
            kind: Modifier,
            children: One,
            doc: "couleur du texte et des formes",
            params: const { &[p("color", ValueType::Color, true, "couleur")] },
            build: |p, c| one(c).foreground(p.color("color").unwrap_or_default()),
        },
        Entry {
            name: "tint",
            kind: Modifier,
            children: One,
            doc: "couleur d'accent",
            params: const { &[p("color", ValueType::Color, true, "couleur")] },
            build: |p, c| one(c).tint(p.color("color").unwrap_or_default()),
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
            params: const { &[p("color", ValueType::Color, true, "couleur")] },
            build: |p, c| one(c).background(p.color("color").unwrap_or_default()),
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
