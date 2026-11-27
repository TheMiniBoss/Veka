//! Les valeurs du langage, et leur conversion vers les valeurs du schéma de Veka.

use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

use veka_anima::{Animation, Edge, Transition};
use veka_colora::{
    AngularGradient, EllipticalGradient, Hierarchy, Ink, LinearGradient, MeshGradient, Paint, RadialGradient,
    Theme, Token, UnitPoint,
};
use veka_foundation::Color;
use veka_ui::TimelineState;
use veka_ui::schema::{Value, ValueType};

/// Une valeur `.vk` : ce que calculent les expressions, et les données de l'hôte (`model`).
///
/// ```
/// use veka_lang::Val;
/// let article = Val::record([("nom", Val::from("Épée")), ("prix", Val::from(12))]);
/// let model = Val::record([("articles", Val::from(vec![article]))]);
/// assert_eq!(model.get("articles").and_then(|a| a.index(0)).and_then(|a| a.get("prix")), Some(Val::Int(12)));
/// ```
#[derive(Clone, Debug, Default)]
pub enum Val {
    #[default]
    Unit,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(Rc<str>),
    Color(Color),
    Paint(Paint),
    /// `.nom`, `.nom(args)`, `.nom.autre(…)` : interprété selon ce qu'attend l'endroit où il va.
    Sym(Rc<Sym>),
    Array(Rc<Vec<Val>>),
    /// `lo..<hi` (fin exclue).
    Range(i64, i64),
    Theme(Arc<Theme>),
    /// `{ nom: valeur, … }`, dans l'ordre d'écriture.
    Record(Rc<Vec<(Rc<str>, Val)>>),
    /// Une `@Timeline` (ou une `@Clock`) de la vue.
    Timeline(TimelineState),
}

impl Val {
    /// Un enregistrement.
    pub fn record<K: Into<Rc<str>>>(fields: impl IntoIterator<Item = (K, Val)>) -> Val {
        Val::Record(Rc::new(fields.into_iter().map(|(k, v)| (k.into(), v)).collect()))
    }

    /// Le champ `name` d'un enregistrement.
    pub fn get(&self, name: &str) -> Option<Val> {
        match self {
            Val::Record(r) => r.iter().find(|(k, _)| &**k == name).map(|(_, v)| v.clone()),
            _ => None,
        }
    }

    /// L'élément `i` d'un tableau.
    pub fn index(&self, i: usize) -> Option<Val> {
        match self {
            Val::Array(a) => a.get(i).cloned(),
            Val::Range(lo, hi) => {
                (*lo + i as i64 >= *lo && (*lo + i as i64) < *hi).then(|| Val::Int(*lo + i as i64))
            }
            _ => None,
        }
    }

    /// Un enregistrement avec le champ `name` remplacé (ou ajouté).
    pub fn with(&self, name: &str, value: Val) -> Val {
        let mut fields = match self {
            Val::Record(r) => (**r).clone(),
            _ => Vec::new(),
        };
        match fields.iter_mut().find(|(k, _)| &**k == name) {
            Some(f) => f.1 = value,
            None => fields.push((name.into(), value)),
        }
        Val::Record(Rc::new(fields))
    }
}

impl From<bool> for Val {
    fn from(v: bool) -> Self {
        Val::Bool(v)
    }
}
impl From<i64> for Val {
    fn from(v: i64) -> Self {
        Val::Int(v)
    }
}
impl From<i32> for Val {
    fn from(v: i32) -> Self {
        Val::Int(v as i64)
    }
}
impl From<f64> for Val {
    fn from(v: f64) -> Self {
        Val::Float(v)
    }
}
impl From<f32> for Val {
    fn from(v: f32) -> Self {
        Val::Float(v as f64)
    }
}
impl From<&str> for Val {
    fn from(v: &str) -> Self {
        Val::Str(v.into())
    }
}
impl From<String> for Val {
    fn from(v: String) -> Self {
        Val::Str(v.into())
    }
}
impl From<Color> for Val {
    fn from(v: Color) -> Self {
        Val::Color(v)
    }
}
impl From<Vec<Val>> for Val {
    fn from(v: Vec<Val>) -> Self {
        Val::Array(Rc::new(v))
    }
}

/// Un symbole et ce qu'on lui a appliqué.
#[derive(Clone, Debug, PartialEq)]
pub struct Sym {
    pub name: String,
    pub args: Option<Vec<(Option<String>, Val)>>,
    /// `.combined(…)`, `.delay(…)`, `.secondary`…
    pub chain: Vec<(String, Option<ArgList>)>,
}

impl PartialEq for Val {
    fn eq(&self, other: &Val) -> bool {
        use Val::*;
        match (self, other) {
            (Unit, Unit) => true,
            (Bool(a), Bool(b)) => a == b,
            (Int(a), Int(b)) => a == b,
            (Int(a), Float(b)) | (Float(b), Int(a)) => (*a as f64) == *b,
            (Float(a), Float(b)) => a == b,
            (Str(a), Str(b)) => a == b,
            (Color(a), Color(b)) => a == b,
            (Paint(a), Paint(b)) => a == b,
            (Sym(a), Sym(b)) => a == b,
            (Array(a), Array(b)) => a == b,
            (Range(a, b), Range(c, d)) => a == c && b == d,
            (Theme(a), Theme(b)) => Arc::ptr_eq(a, b),
            (Record(a), Record(b)) => a == b,
            (Timeline(a), Timeline(b)) => a.to_bits() == b.to_bits(),
            _ => false,
        }
    }
}

impl fmt::Display for Val {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Val::Unit => Ok(()),
            Val::Bool(b) => write!(f, "{b}"),
            Val::Int(v) => write!(f, "{v}"),
            Val::Float(v) => {
                if v.fract() == 0.0 && v.abs() < 1e15 {
                    write!(f, "{v:.1}")
                } else {
                    write!(f, "{v}")
                }
            }
            Val::Str(s) => f.write_str(s),
            Val::Color(c) => write!(f, "{c:?}"),
            Val::Sym(s) => write!(f, ".{}", s.name),
            Val::Array(a) => {
                f.write_str("[")?;
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{v}")?;
                }
                f.write_str("]")
            }
            Val::Range(a, b) => write!(f, "{a}..<{b}"),
            Val::Paint(_) => f.write_str("<peinture>"),
            Val::Theme(t) => write!(f, "<thème {}>", t.name),
            Val::Record(r) => {
                f.write_str("{ ")?;
                for (i, (k, v)) in r.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                f.write_str(" }")
            }
            Val::Timeline(_) => f.write_str("<timeline>"),
        }
    }
}

impl Val {
    /// Le nom du type, pour les messages et `@State var x: Type`.
    pub fn type_name(&self) -> &'static str {
        match self {
            Val::Unit => "Void",
            Val::Bool(_) => "Bool",
            Val::Int(_) => "Int",
            Val::Float(_) => "Float",
            Val::Str(_) => "String",
            Val::Color(_) => "Color",
            Val::Paint(_) => "Paint",
            Val::Sym(_) => "Symbol",
            Val::Array(_) => "Array",
            Val::Range(..) => "Range",
            Val::Theme(_) => "Theme",
            Val::Record(_) => "Record",
            Val::Timeline(_) => "Timeline",
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Val::Int(v) => Some(*v as f64),
            Val::Float(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Val::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Les éléments d'une collection (`ForEach`).
    pub fn items(&self) -> Option<Vec<Val>> {
        match self {
            Val::Array(a) => Some(a.to_vec()),
            Val::Range(lo, hi) => Some((*lo..*hi).map(Val::Int).collect()),
            _ => None,
        }
    }

    pub(crate) fn sym(&self) -> Option<&Sym> {
        match self {
            Val::Sym(s) => Some(s),
            _ => None,
        }
    }

    /// Le nom d'un symbole simple (`.leading`) ou une chaîne.
    pub fn enum_name(&self) -> Option<&str> {
        match self {
            Val::Sym(s) if s.args.is_none() && s.chain.is_empty() => Some(&s.name),
            Val::Str(s) => Some(s),
            _ => None,
        }
    }
}

pub type Args = [(Option<String>, Val)];
/// Des arguments évalués : `(étiquette, valeur)`.
pub type ArgList = Vec<(Option<String>, Val)>;

fn arg<'a>(args: &'a Args, i: usize, label: &str) -> Option<&'a Val> {
    args.iter()
        .find(|(l, _)| l.as_deref() == Some(label))
        .map(|(_, v)| v)
        .or_else(|| args.iter().filter(|(l, _)| l.is_none()).nth(i).map(|(_, v)| v))
}

fn num(args: &Args, i: usize, label: &str) -> Option<f32> {
    arg(args, i, label).and_then(Val::as_f64).map(|v| v as f32)
}

fn hierarchy(name: &str) -> Option<Hierarchy> {
    Some(match name {
        "primary" => Hierarchy::Primary,
        "secondary" => Hierarchy::Secondary,
        "tertiary" => Hierarchy::Tertiary,
        "quaternary" => Hierarchy::Quaternary,
        _ => return None,
    })
}

/// Les couleurs fixes (`.blue`), comme `Color::Blue` en Rust : elles ne suivent pas le thème.
pub fn fixed_color(name: &str) -> Option<Color> {
    Some(match name {
        "clear" => Color::Clear,
        "black" => Color::Black,
        "white" => Color::White,
        "gray" => Color::Gray,
        "red" => Color::Red,
        "orange" => Color::Orange,
        "yellow" => Color::Yellow,
        "green" => Color::Green,
        "teal" => Color::Teal,
        "blue" => Color::Blue,
        "indigo" => Color::Indigo,
        "purple" => Color::Purple,
        "pink" => Color::Pink,
        _ => return None,
    })
}

/// Un jeton Colora : `.Accent`, `.Blue.secondary`, `.Marque.Fond` ; ou une couleur fixe : `.blue`.
fn token_ink(s: &Sym) -> Result<Ink, String> {
    if s.args.is_some() {
        return Err(format!("`.{}(…)` n'est pas une couleur", s.name));
    }
    if let Some(c) = fixed_color(&s.name) {
        return match s.chain.as_slice() {
            [] => Ok(Ink::Color(c)),
            [(m, Some(a))] if m == "opacity" => match num(a, 0, "opacity") {
                Some(o) => Ok(Ink::Color(c.opacity(o))),
                None => Err("`.opacity(…)` attend un nombre".into()),
            },
            _ => Err(format!(
                "`.{}` est une couleur fixe : seul `.opacity(…)` la suit",
                s.name
            )),
        };
    }
    let mut name = s.name.clone();
    let mut h = Hierarchy::Primary;
    for (i, (part, args)) in s.chain.iter().enumerate() {
        if args.is_some() {
            return Err(format!("`.{part}(…)` : une couleur ne prend pas d'arguments"));
        }
        match hierarchy(part) {
            Some(x) if i + 1 == s.chain.len() => h = x,
            _ => {
                name.push('.');
                name.push_str(part);
            }
        }
    }
    if !name.chars().next().is_some_and(char::is_uppercase) {
        return Err(format!(
            "`.{name}` : un jeton de couleur commence par une majuscule (`.Accent`, `.Blue`)"
        ));
    }
    Ok(Token::named(&name).hierarchy(h))
}

pub fn to_ink(v: &Val) -> Result<Ink, String> {
    match v {
        Val::Color(c) => Ok(Ink::Color(*c)),
        Val::Sym(s) => token_ink(s),
        _ => Err(format!("couleur ou jeton attendu, trouvé {}", v.type_name())),
    }
}

pub fn to_paint(v: &Val) -> Result<Paint, String> {
    match v {
        Val::Paint(p) => Ok(p.clone()),
        Val::Color(c) => Ok(Paint::Solid(*c)),
        Val::Sym(s) => token_ink(s).map(Paint::from),
        _ => Err(format!(
            "couleur, jeton ou dégradé attendu, trouvé {}",
            v.type_name()
        )),
    }
}

pub fn to_unit_point(v: &Val) -> Result<UnitPoint, String> {
    let name = v.enum_name().ok_or("point attendu (`.topLeading`, `.center`…)")?;
    Ok(match name {
        "topLeading" => UnitPoint::TOP_LEADING,
        "top" => UnitPoint::TOP,
        "topTrailing" => UnitPoint::TOP_TRAILING,
        "leading" => UnitPoint::LEADING,
        "center" => UnitPoint::CENTER,
        "trailing" => UnitPoint::TRAILING,
        "bottomLeading" => UnitPoint::BOTTOM_LEADING,
        "bottom" => UnitPoint::BOTTOM,
        "bottomTrailing" => UnitPoint::BOTTOM_TRAILING,
        _ => return Err(format!("point inconnu `.{name}`")),
    })
}

fn to_edge(v: &Val) -> Result<Edge, String> {
    Ok(match v.enum_name().ok_or("bord attendu (`.top`, `.leading`…)")? {
        "top" => Edge::Top,
        "bottom" => Edge::Bottom,
        "leading" => Edge::Leading,
        "trailing" => Edge::Trailing,
        n => return Err(format!("bord inconnu `.{n}`")),
    })
}

pub fn to_animation(v: &Val) -> Result<Animation, String> {
    let s = v
        .sym()
        .ok_or_else(|| format!("animation attendue (`.smooth`…), trouvé {}", v.type_name()))?;
    let a: &Args = s.args.as_deref().unwrap_or(&[]);
    let d = |i| num(a, i, "duration").unwrap_or(0.35);
    let mut anim = match s.name.as_str() {
        "smooth" => Animation::smooth(),
        "snappy" => Animation::snappy(),
        "bouncy" => Animation::bouncy(),
        "default" => Animation::default(),
        "linear" => Animation::linear(d(0)),
        "easeIn" => Animation::ease_in(d(0)),
        "easeOut" => Animation::ease_out(d(0)),
        "easeInOut" => Animation::ease_in_out(d(0)),
        "circEaseIn" => Animation::circ_ease_in(d(0)),
        "circEaseOut" => Animation::circ_ease_out(d(0)),
        "circEaseInOut" => Animation::circ_ease_in_out(d(0)),
        "timingCurve" => Animation::timing_curve(
            num(a, 0, "x1").unwrap_or(0.25),
            num(a, 1, "y1").unwrap_or(0.1),
            num(a, 2, "x2").unwrap_or(0.25),
            num(a, 3, "y2").unwrap_or(1.0),
            num(a, 4, "duration").unwrap_or(0.35),
        ),
        "spring" => Animation::spring(
            num(a, 0, "response").unwrap_or(0.5),
            num(a, 1, "damping").unwrap_or(0.825),
        ),
        "interpolatingSpring" => Animation::interpolating_spring(
            num(a, 0, "mass").unwrap_or(1.0),
            num(a, 1, "stiffness").unwrap_or(170.0),
            num(a, 2, "damping").unwrap_or(15.0),
        ),
        n => return Err(format!("animation inconnue `.{n}`")),
    };
    for (m, args) in &s.chain {
        let a: &Args = args.as_deref().unwrap_or(&[]);
        let rev = arg(a, 1, "autoreverses").and_then(Val::as_bool).unwrap_or(false);
        anim = match m.as_str() {
            "delay" => anim.delay(num(a, 0, "seconds").unwrap_or(0.0)),
            "speed" => anim.speed(num(a, 0, "speed").unwrap_or(1.0)),
            "repeatCount" | "repeat" => anim.repeat(num(a, 0, "count").unwrap_or(1.0).max(0.0) as u32, rev),
            "repeatForever" => {
                anim.repeat_forever(arg(a, 0, "autoreverses").and_then(Val::as_bool).unwrap_or(false))
            }
            n => return Err(format!("`.{n}` : inconnu sur une animation")),
        };
    }
    Ok(anim)
}

pub fn to_transition(v: &Val) -> Result<Transition, String> {
    let s = v
        .sym()
        .ok_or_else(|| format!("transition attendue (`.opacity`…), trouvé {}", v.type_name()))?;
    let a: &Args = s.args.as_deref().unwrap_or(&[]);
    let need =
        |i: usize, label: &str| arg(a, i, label).ok_or_else(|| format!("`.{}` : `{label}` manquant", s.name));
    let mut t = match s.name.as_str() {
        "opacity" => Transition::opacity(),
        "identity" => Transition::identity(),
        "slide" => Transition::slide(),
        "scale" => Transition::scale(num(a, 0, "scale").unwrap_or(0.0)),
        "move" => Transition::move_edge(to_edge(need(0, "edge")?)?),
        "push" => Transition::push(to_edge(need(0, "from")?)?),
        "offset" => Transition::offset(num(a, 0, "x").unwrap_or(0.0), num(a, 1, "y").unwrap_or(0.0)),
        "blur" => Transition::blur(num(a, 0, "radius").unwrap_or(8.0)),
        "asymmetric" => Transition::asymmetric(
            to_transition(need(0, "insertion")?)?,
            to_transition(need(1, "removal")?)?,
        ),
        n => return Err(format!("transition inconnue `.{n}`")),
    };
    for (m, args) in &s.chain {
        let a: &Args = args.as_deref().unwrap_or(&[]);
        t = match m.as_str() {
            "combined" => t.combined(to_transition(
                arg(a, 0, "with").ok_or("`.combined` : une transition attendue")?,
            )?),
            n => return Err(format!("`.{n}` : inconnu sur une transition")),
        };
    }
    Ok(t)
}

/// Les dégradés de Colora : `LinearGradient([.Blue, .Teal], from: .top, to: .bottom)`…
pub fn gradient(name: &str, a: &Args) -> Option<Result<Paint, String>> {
    if !matches!(
        name,
        "LinearGradient" | "RadialGradient" | "EllipticalGradient" | "AngularGradient" | "MeshGradient"
    ) {
        return None;
    }
    let inks = || -> Result<Vec<Ink>, String> {
        let list = arg(a, 0, "colors").ok_or("une liste de couleurs attendue")?;
        list.items()
            .ok_or("une liste de couleurs attendue")?
            .iter()
            .map(to_ink)
            .collect()
    };
    let point = |label: &str| arg(a, usize::MAX, label).map(to_unit_point).transpose();
    // Les valeurs par défaut sont celles de Rust : un argument absent ne change rien.
    let r = (|| -> Result<Paint, String> {
        Ok(match name {
            "LinearGradient" => {
                let mut g = LinearGradient::new(inks()?);
                if let Some(p) = point("from")? {
                    g = g.from(p);
                }
                if let Some(p) = point("to")? {
                    g = g.to(p);
                }
                g.into()
            }
            "RadialGradient" => {
                let mut g = RadialGradient::new(inks()?, num(a, 1, "radius").unwrap_or(100.0));
                if let Some(p) = point("center")? {
                    g = g.center(p);
                }
                if let Some(r) = num(a, usize::MAX, "startRadius") {
                    g = g.start_radius(r);
                }
                g.into()
            }
            "EllipticalGradient" => {
                let mut g = EllipticalGradient::new(inks()?);
                if let Some(p) = point("center")? {
                    g = g.center(p);
                }
                g.into()
            }
            "AngularGradient" => {
                let mut g = AngularGradient::new(inks()?);
                if let Some(p) = point("center")? {
                    g = g.center(p);
                }
                let (start, end) = (num(a, usize::MAX, "startAngle"), num(a, usize::MAX, "endAngle"));
                if start.is_some() || end.is_some() {
                    g = g.angles(start.unwrap_or(0.0), end.unwrap_or(360.0));
                }
                g.into()
            }
            "MeshGradient" => {
                let w = num(a, 0, "width").ok_or("`width` manquant")? as usize;
                let h = num(a, 1, "height").ok_or("`height` manquant")? as usize;
                let list = arg(a, 2, "colors")
                    .and_then(Val::items)
                    .ok_or("une liste de couleurs attendue")?;
                let inks: Vec<Ink> = list.iter().map(to_ink).collect::<Result<_, _>>()?;
                let m = MeshGradient::grid(w, h, inks);
                if !m.is_valid() {
                    return Err(format!("MeshGradient : {w} × {h} points attendus"));
                }
                m.into()
            }
            _ => unreachable!(),
        })
    })();
    Some(r)
}

/// Convertit une valeur pour un paramètre du schéma de type `ty`.
pub fn to_schema(v: &Val, ty: ValueType) -> Result<Value, String> {
    let wrong = || format!("{} attendu, trouvé {} ({v})", type_label(ty), v.type_name());
    Ok(match ty {
        ValueType::Bool => Value::Bool(v.as_bool().ok_or_else(wrong)?),
        ValueType::Number => match v {
            Val::Int(i) => Value::Int(*i),
            Val::Float(f) => Value::Float(*f),
            _ => return Err(wrong()),
        },
        ValueType::String => match v {
            Val::Str(s) => Value::String(s.clone()),
            _ => return Err(wrong()),
        },
        ValueType::Color => match v {
            Val::Color(c) => Value::Color(*c),
            _ => return Err(wrong()),
        },
        ValueType::Paint => Value::Paint(to_paint(v)?),
        ValueType::Theme => match v {
            Val::Theme(t) => Value::Theme(t.clone()),
            _ => return Err(wrong()),
        },
        ValueType::Enum(names) => {
            let n = v.enum_name().ok_or_else(wrong)?;
            if !names.contains(&n) {
                return Err(format!(
                    "`.{n}` inconnu ; possibles : {}",
                    names
                        .iter()
                        .map(|n| format!(".{n}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            Value::Enum(Rc::from(n))
        }
        ValueType::Animation => Value::Animation(to_animation(v)?),
        ValueType::Transition => Value::Transition(to_transition(v)?),
        ValueType::Action => return Err("une action s'écrit entre `{ }`".into()),
        ValueType::Any => match v {
            Val::Bool(b) => Value::Bool(*b),
            Val::Int(i) => Value::Int(*i),
            Val::Float(f) => Value::Float(*f),
            Val::Str(s) => Value::String(s.clone()),
            Val::Color(c) => Value::Color(*c),
            other => Value::String(Rc::from(other.to_string())),
        },
    })
}

fn type_label(ty: ValueType) -> &'static str {
    match ty {
        ValueType::Bool => "un booléen",
        ValueType::Number => "un nombre",
        ValueType::String => "une chaîne",
        ValueType::Color => "une couleur",
        ValueType::Paint => "une peinture",
        ValueType::Theme => "un thème",
        ValueType::Enum(_) => "un nom (`.leading`…)",
        ValueType::Action => "une action",
        ValueType::Animation => "une animation",
        ValueType::Transition => "une transition",
        ValueType::Any => "une valeur",
    }
}
