//! L'arbre d'un fichier `.vk`. Les positions (`Span`) ne comptent pas dans les comparaisons : deux
//! arbres égaux décrivent la même chose, où qu'elle soit écrite.

use veka_foundation::Color;

/// Une position dans le fichier (ligne et colonne à partir de 1).
#[derive(Clone, Copy, Debug, Default, Eq)]
pub struct Span {
    pub line: u32,
    pub col: u32,
}

impl PartialEq for Span {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Span {
    pub const START: Span = Span { line: 1, col: 1 };
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

/// Un fichier : ses vues et ses thèmes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Module {
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    View(ViewDecl),
    Theme(ThemeDecl),
}

impl Module {
    pub fn views(&self) -> impl Iterator<Item = &ViewDecl> {
        self.items.iter().filter_map(|i| match i {
            Item::View(v) => Some(v),
            _ => None,
        })
    }

    pub fn view(&self, name: &str) -> Option<&ViewDecl> {
        self.views().find(|v| v.name == name)
    }

    pub fn themes(&self) -> impl Iterator<Item = &ThemeDecl> {
        self.items.iter().filter_map(|i| match i {
            Item::Theme(t) => Some(t),
            _ => None,
        })
    }
}

/// `view Nom { @State var … ; corps }`.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewDecl {
    pub name: String,
    pub states: Vec<StateDecl>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

/// `@State var nom: Type = valeur`.
#[derive(Clone, Debug, PartialEq)]
pub struct StateDecl {
    pub name: String,
    pub ty: Option<TypeName>,
    pub init: Expr,
    pub span: Span,
}

/// Un type écrit dans le fichier : `Int`, `[Int]`…
#[derive(Clone, Debug, PartialEq)]
pub enum TypeName {
    Named(String),
    Array(Box<TypeName>),
}

/// `theme Nom { Jeton: clair / sombre ; métrique: nombre }`.
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeDecl {
    pub name: String,
    pub parent: Option<String>,
    pub entries: Vec<ThemeEntry>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ThemeEntry {
    /// `Tint`, `Marque.Fond`.
    pub token: String,
    pub light: Expr,
    pub dark: Option<Expr>,
    pub span: Span,
}

/// Une instruction : dans un corps de vue, elle donne des vues ; dans une action, elle agit.
#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    Expr(Expr),
    If {
        cond: Expr,
        then: Vec<Stmt>,
        els: Option<Vec<Stmt>>,
        span: Span,
    },
    Let {
        name: String,
        value: Expr,
        span: Span,
    },
    /// `cible = valeur`, `cible += valeur`…
    Assign {
        target: Expr,
        op: AssignOp,
        value: Expr,
        span: Span,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssignOp {
    Set,
    Add,
    Sub,
    Mul,
    Div,
}

impl AssignOp {
    pub fn text(self) -> &'static str {
        match self {
            AssignOp::Set => "=",
            AssignOp::Add => "+=",
            AssignOp::Sub => "-=",
            AssignOp::Mul => "*=",
            AssignOp::Div => "/=",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(Vec<StrPart>),
    Color(Color),
    Ident(String),
    /// `.nom` : énumération, jeton, animation… selon l'endroit.
    Symbol(String),
    Array(Vec<Expr>),
    Range {
        lo: Box<Expr>,
        hi: Box<Expr>,
        inclusive: bool,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Ternary {
        cond: Box<Expr>,
        then: Box<Expr>,
        els: Box<Expr>,
    },
    Member {
        recv: Box<Expr>,
        name: String,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Arg>,
        trailing: Option<Closure>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum StrPart {
    Text(String),
    Expr(Expr),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Arg {
    pub label: Option<String>,
    pub value: Expr,
}

/// `{ a, b in … }` après un appel.
#[derive(Clone, Debug, PartialEq)]
pub struct Closure {
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

impl BinaryOp {
    pub fn text(self) -> &'static str {
        match self {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Rem => "%",
            BinaryOp::Eq => "==",
            BinaryOp::Ne => "!=",
            BinaryOp::Lt => "<",
            BinaryOp::Le => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::Ge => ">=",
            BinaryOp::And => "&&",
            BinaryOp::Or => "||",
        }
    }

    /// Plus grand : lie plus fort.
    pub fn precedence(self) -> u8 {
        match self {
            BinaryOp::Or => 1,
            BinaryOp::And => 2,
            BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => 3,
            BinaryOp::Add | BinaryOp::Sub => 5,
            BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => 6,
        }
    }
}
