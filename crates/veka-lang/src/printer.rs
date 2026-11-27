//! L'imprimeur : [`Module`] → texte canonique (4 espaces, un modifier par ligne). Relire le texte
//! imprimé redonne le même arbre (testé) : c'est ce dont l'éditeur aura besoin.

use std::fmt::Write;

use veka_foundation::Color;

use crate::ast::*;

/// Le texte canonique d'un module.
pub fn print(module: &Module) -> String {
    let mut p = Printer { out: String::new() };
    for (i, item) in module.items.iter().enumerate() {
        if i > 0 {
            p.out.push('\n');
        }
        match item {
            Item::View(v) => p.view(v),
            Item::Theme(t) => p.theme(t),
        }
    }
    p.out
}

/// Le texte d'une expression.
pub fn print_expr(e: &Expr) -> String {
    let mut p = Printer { out: String::new() };
    p.expr(e, 0, 0);
    p.out
}

struct Printer {
    out: String,
}

fn pad(n: usize) -> String {
    " ".repeat(n * 4)
}

impl Printer {
    fn view(&mut self, v: &ViewDecl) {
        let _ = writeln!(self.out, "view {} {{", v.name);
        for s in &v.states {
            let _ = write!(self.out, "    @State var {}", s.name);
            if let Some(t) = &s.ty {
                let _ = write!(self.out, ": {}", type_text(t));
            }
            self.out.push_str(" = ");
            self.expr(&s.init, 0, 1);
            self.out.push('\n');
        }
        if !v.states.is_empty() && !v.body.is_empty() {
            self.out.push('\n');
        }
        for s in &v.body {
            self.stmt(s, 1);
        }
        self.out.push_str("}\n");
    }

    fn theme(&mut self, t: &ThemeDecl) {
        let _ = write!(self.out, "theme {}", t.name);
        if let Some(p) = &t.parent {
            let _ = write!(self.out, ": {p}");
        }
        self.out.push_str(" {\n");
        for e in &t.entries {
            let _ = write!(self.out, "    {}: ", e.token);
            self.expr(&e.light, 0, 1);
            if let Some(d) = &e.dark {
                self.out.push_str(" / ");
                self.expr(d, 0, 1);
            }
            self.out.push('\n');
        }
        self.out.push_str("}\n");
    }

    fn block(&mut self, body: &[Stmt], indent: usize) {
        self.out.push_str("{\n");
        for s in body {
            self.stmt(s, indent + 1);
        }
        self.out.push_str(&pad(indent));
        self.out.push('}');
    }

    fn stmt(&mut self, s: &Stmt, indent: usize) {
        self.out.push_str(&pad(indent));
        match s {
            Stmt::Expr(e) => self.chain(e, indent),
            Stmt::If { cond, then, els, .. } => self.if_stmt(cond, then, els.as_deref(), indent),
            Stmt::Let { name, value, .. } => {
                let _ = write!(self.out, "let {name} = ");
                self.expr(value, 0, indent);
            }
            Stmt::Assign {
                target, op, value, ..
            } => {
                self.expr(target, 0, indent);
                let _ = write!(self.out, " {} ", op.text());
                self.expr(value, 0, indent);
            }
        }
        self.out.push('\n');
    }

    fn if_stmt(&mut self, cond: &Expr, then: &[Stmt], els: Option<&[Stmt]>, indent: usize) {
        self.out.push_str("if ");
        self.expr(cond, 0, indent);
        self.out.push(' ');
        self.block(then, indent);
        if let Some(els) = els {
            self.out.push_str(" else ");
            match els {
                [Stmt::If { cond, then, els, .. }] => self.if_stmt(cond, then, els.as_deref(), indent),
                _ => self.block(els, indent),
            }
        }
    }

    /// Une vue et ses modifiers, un par ligne.
    fn chain(&mut self, e: &Expr, indent: usize) {
        let mut mods = Vec::new();
        let mut base = e;
        loop {
            match &base.kind {
                ExprKind::Call {
                    callee,
                    args,
                    trailing,
                } if matches!(callee.kind, ExprKind::Member { .. }) => {
                    let ExprKind::Member { recv, name } = &callee.kind else {
                        unreachable!()
                    };
                    if !is_view_like(recv) {
                        break;
                    }
                    mods.push((name.as_str(), Some((args.as_slice(), trailing.as_ref()))));
                    base = recv;
                }
                ExprKind::Member { recv, name } if is_view_like(recv) => {
                    mods.push((name.as_str(), None));
                    base = recv;
                }
                _ => break,
            }
        }
        self.expr(base, 0, indent);
        for (name, call) in mods.into_iter().rev() {
            self.out.push('\n');
            self.out.push_str(&pad(indent + 1));
            let _ = write!(self.out, ".{name}");
            if let Some((args, trailing)) = call {
                self.call_rest(args, trailing, indent + 1);
            }
        }
    }

    fn call_rest(&mut self, args: &[Arg], trailing: Option<&Closure>, indent: usize) {
        if !args.is_empty() || trailing.is_none() {
            self.out.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    self.out.push_str(", ");
                }
                if let Some(l) = &a.label {
                    let _ = write!(self.out, "{l}: ");
                }
                self.expr(&a.value, 0, indent);
            }
            self.out.push(')');
        }
        if let Some(c) = trailing {
            self.out.push_str(" {");
            if !c.params.is_empty() {
                let _ = write!(self.out, " {} in", c.params.join(", "));
            }
            self.out.push('\n');
            for s in &c.body {
                self.stmt(s, indent + 1);
            }
            self.out.push_str(&pad(indent));
            self.out.push('}');
        }
    }

    /// `min` : précédence minimale sans parenthèses.
    fn expr(&mut self, e: &Expr, min: u8, indent: usize) {
        match &e.kind {
            ExprKind::Bool(b) => self.out.push_str(if *b { "true" } else { "false" }),
            ExprKind::Int(v) => {
                let _ = write!(self.out, "{v}");
            }
            ExprKind::Float(v) => self.out.push_str(&float_text(*v)),
            ExprKind::Str(parts) => {
                self.out.push('"');
                for p in parts {
                    match p {
                        StrPart::Text(t) => {
                            for c in t.chars() {
                                match c {
                                    '"' => self.out.push_str("\\\""),
                                    '\\' => self.out.push_str("\\\\"),
                                    '\n' => self.out.push_str("\\n"),
                                    '\t' => self.out.push_str("\\t"),
                                    c => self.out.push(c),
                                }
                            }
                        }
                        StrPart::Expr(e) => {
                            self.out.push_str("\\(");
                            self.expr(e, 0, indent);
                            self.out.push(')');
                        }
                    }
                }
                self.out.push('"');
            }
            ExprKind::Color(c) => self.out.push_str(&color_text(*c)),
            ExprKind::Ident(n) => self.out.push_str(n),
            ExprKind::Symbol(n) => {
                let _ = write!(self.out, ".{n}");
            }
            ExprKind::Array(items) => {
                self.out.push('[');
                for (i, it) in items.iter().enumerate() {
                    if i > 0 {
                        self.out.push_str(", ");
                    }
                    self.expr(it, 0, indent);
                }
                self.out.push(']');
            }
            ExprKind::Range { lo, hi, inclusive } => {
                let paren = min > 4;
                if paren {
                    self.out.push('(');
                }
                self.expr(lo, 5, indent);
                self.out.push_str(if *inclusive { "..." } else { "..<" });
                self.expr(hi, 5, indent);
                if paren {
                    self.out.push(')');
                }
            }
            ExprKind::Unary { op, expr } => {
                self.out.push_str(match op {
                    UnaryOp::Neg => "-",
                    UnaryOp::Not => "!",
                });
                self.expr(expr, 7, indent);
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let prec = op.precedence();
                let paren = prec < min;
                if paren {
                    self.out.push('(');
                }
                self.expr(lhs, prec, indent);
                let _ = write!(self.out, " {} ", op.text());
                self.expr(rhs, prec + 1, indent);
                if paren {
                    self.out.push(')');
                }
            }
            ExprKind::Ternary { cond, then, els } => {
                let paren = min > 0;
                if paren {
                    self.out.push('(');
                }
                self.expr(cond, 1, indent);
                self.out.push_str(" ? ");
                self.expr(then, 0, indent);
                self.out.push_str(" : ");
                self.expr(els, 0, indent);
                if paren {
                    self.out.push(')');
                }
            }
            ExprKind::Member { recv, name } => {
                self.expr(recv, 8, indent);
                let _ = write!(self.out, ".{name}");
            }
            ExprKind::Call {
                callee,
                args,
                trailing,
            } => {
                self.expr(callee, 8, indent);
                self.call_rest(args, trailing.as_ref(), indent);
            }
        }
    }
}

/// Une vue (ou une vue modifiée) : ses modifiers vont à la ligne.
pub(crate) fn is_view_like(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Ident(n) => n.chars().next().is_some_and(char::is_uppercase),
        ExprKind::Call { callee, .. } => is_view_like(callee),
        ExprKind::Member { recv, .. } => is_view_like(recv),
        _ => false,
    }
}

pub(crate) fn type_text(t: &TypeName) -> String {
    match t {
        TypeName::Named(n) => n.clone(),
        TypeName::Array(t) => format!("[{}]", type_text(t)),
    }
}

fn float_text(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{v:.1}")
    } else {
        format!("{v}")
    }
}

fn color_text(c: Color) -> String {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    let rgb = (b(c.r) << 16) | (b(c.g) << 8) | b(c.b);
    if c.a >= 1.0 {
        format!("#{rgb:06X}")
    } else {
        format!("#{rgb:06X}{:02X}", b(c.a))
    }
}
