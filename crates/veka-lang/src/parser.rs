//! Le parseur : jetons → [`Module`]. Descente récursive écrite à la main ; une erreur dans une ligne
//! n'empêche pas de lire les suivantes.

use veka_foundation::Color;

use crate::ast::*;
use crate::lexer::{StrPiece, Tok, Token, lex};
use crate::{Diagnostic, Span};

type R<T> = Result<T, Diagnostic>;

/// Lit un fichier `.vk`. Renvoie le module (au mieux, même avec des erreurs) et les erreurs.
pub fn parse(source: &str) -> (Module, Vec<Diagnostic>) {
    let (tokens, mut errors) = lex(source, Span::START);
    let mut p = Parser {
        t: tokens,
        i: 0,
        errors: Vec::new(),
    };
    let module = p.module();
    errors.append(&mut p.errors);
    (module, errors)
}

/// Lit une seule expression.
pub fn parse_expr(source: &str) -> R<Expr> {
    parse_expr_at(source, Span::START)
}

fn parse_expr_at(source: &str, at: Span) -> R<Expr> {
    let (tokens, errors) = lex(source, at);
    if let Some(e) = errors.into_iter().next() {
        return Err(e);
    }
    let mut p = Parser {
        t: tokens,
        i: 0,
        errors: Vec::new(),
    };
    p.skip_newlines();
    let e = p.expr(false)?;
    p.skip_newlines();
    if !p.at_eof() {
        return Err(p.unexpected("la fin de l'expression"));
    }
    Ok(e)
}

struct Parser {
    t: Vec<Token>,
    i: usize,
    errors: Vec<Diagnostic>,
}

fn describe(t: &Tok) -> String {
    match t {
        Tok::Ident(n) => format!("`{n}`"),
        Tok::Attr(n) => format!("`@{n}`"),
        Tok::Int(v) => format!("`{v}`"),
        Tok::Float(v) => format!("`{v}`"),
        Tok::Str(_) => "une chaîne".into(),
        Tok::Color(..) => "une couleur".into(),
        Tok::Punct(p) => format!("`{p}`"),
        Tok::Newline => "une fin de ligne".into(),
        Tok::Eof => "la fin du fichier".into(),
    }
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.t[self.i.min(self.t.len() - 1)].tok
    }

    fn peek_at(&self, k: usize) -> &Tok {
        &self.t[(self.i + k).min(self.t.len() - 1)].tok
    }

    fn span(&self) -> Span {
        self.t[self.i.min(self.t.len() - 1)].span
    }

    fn bump(&mut self) -> Token {
        let t = self.t[self.i.min(self.t.len() - 1)].clone();
        if self.i < self.t.len() - 1 {
            self.i += 1;
        }
        t
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek(), Tok::Eof)
    }

    fn is(&self, p: &str) -> bool {
        matches!(self.peek(), Tok::Punct(q) if *q == p)
    }

    fn is_ident(&self, name: &str) -> bool {
        matches!(self.peek(), Tok::Ident(n) if n == name)
    }

    fn eat(&mut self, p: &str) -> bool {
        if self.is(p) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn unexpected(&self, wanted: &str) -> Diagnostic {
        Diagnostic::error(
            self.span(),
            format!("{wanted} attendu, trouvé {}", describe(self.peek())),
        )
    }

    fn expect(&mut self, p: &str) -> R<()> {
        if self.eat(p) {
            Ok(())
        } else {
            Err(self.unexpected(&format!("`{p}`")))
        }
    }

    fn ident(&mut self, what: &str) -> R<String> {
        match self.peek().clone() {
            Tok::Ident(n) => {
                self.bump();
                Ok(n)
            }
            _ => Err(self.unexpected(what)),
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek(), Tok::Newline | Tok::Punct(";")) {
            self.bump();
        }
    }

    /// Après une erreur : saute jusqu'à la fin de la ligne (ou à la `}` qui ferme le bloc).
    fn sync(&mut self) {
        let mut depth = 0i32;
        loop {
            match self.peek() {
                Tok::Eof => return,
                Tok::Newline if depth <= 0 => return,
                Tok::Punct("{") => depth += 1,
                Tok::Punct("}") => {
                    if depth <= 0 {
                        return;
                    }
                    depth -= 1;
                }
                _ => {}
            }
            self.bump();
        }
    }

    fn module(&mut self) -> Module {
        let mut items = Vec::new();
        loop {
            self.skip_newlines();
            if self.at_eof() {
                break;
            }
            let r = if self.is_ident("view") {
                self.view().map(Item::View)
            } else if self.is_ident("theme") {
                self.theme().map(Item::Theme)
            } else if self.is_ident("func") {
                self.func().map(Item::Func)
            } else if self.is_ident("timeline") {
                self.timeline().map(Item::Timeline)
            } else if self.is_ident("import") {
                self.import().map(Item::Import)
            } else {
                Err(self.unexpected("`view`, `theme`, `func`, `timeline` ou `import`"))
            };
            match r {
                Ok(item) => items.push(item),
                Err(e) => {
                    self.errors.push(e);
                    // Saute jusqu'à la prochaine déclaration.
                    while !self.at_eof() && !TOP.iter().any(|k| self.is_ident(k)) {
                        self.bump();
                    }
                }
            }
        }
        Module { items }
    }

    fn import(&mut self) -> R<ImportDecl> {
        let span = self.span();
        self.bump();
        match self.peek().clone() {
            Tok::Str(pieces) => {
                self.bump();
                let mut path = String::new();
                for p in pieces {
                    match p {
                        StrPiece::Text(t) => path.push_str(&t),
                        StrPiece::Code(_, at) => {
                            return Err(Diagnostic::error(at, "`import` : un chemin sans `\\(…)`"));
                        }
                    }
                }
                Ok(ImportDecl { path, span })
            }
            _ => Err(self.unexpected("un chemin entre guillemets")),
        }
    }

    fn func(&mut self) -> R<FuncDecl> {
        let span = self.span();
        self.bump();
        let name = self.ident("le nom de la fonction")?;
        self.expect("(")?;
        let mut params = Vec::new();
        loop {
            if self.eat(")") {
                break;
            }
            let p = self.ident("un paramètre")?;
            let ty = if self.eat(":") {
                Some(self.type_name()?)
            } else {
                None
            };
            params.push((std::rc::Rc::from(p), ty));
            if !self.eat(",") {
                self.expect(")")?;
                break;
            }
        }
        let ret = if self.eat("->") {
            Some(self.type_name()?)
        } else {
            None
        };
        let body = self.block()?;
        if !matches!(body.last(), Some(Stmt::Expr(_))) {
            return Err(Diagnostic::error(
                span,
                format!("`func {name}` : la dernière ligne doit être une valeur"),
            ));
        }
        Ok(FuncDecl {
            name,
            params,
            ret,
            body,
            span,
        })
    }

    fn timeline(&mut self) -> R<TimelineDecl> {
        let span = self.span();
        self.bump();
        let name = self.ident("le nom de la timeline")?;
        let args = if self.eat("(") { self.args()? } else { Vec::new() };
        self.expect("{")?;
        let mut tracks = Vec::new();
        let mut events = Vec::new();
        loop {
            self.skip_newlines();
            if self.eat("}") {
                break;
            }
            if self.at_eof() {
                return Err(Diagnostic::error(
                    span,
                    format!("`timeline {name}` : `}}` de fin manquante"),
                ));
            }
            let r = if self.is_ident("track") {
                self.track().map(|t| tracks.push(t))
            } else if self.is_ident("event") {
                self.bump();
                self.expr(true).and_then(|t| {
                    let n = self.ident("le nom de l'événement")?;
                    events.push((t, n));
                    Ok(())
                })
            } else {
                Err(self.unexpected("`track` ou `event`"))
            };
            if let Err(e) = r {
                self.errors.push(e);
                self.sync();
            }
        }
        Ok(TimelineDecl {
            name,
            args,
            tracks,
            events,
            span,
        })
    }

    fn track(&mut self) -> R<TrackDecl> {
        self.bump();
        let name = self.ident("le nom de la piste")?;
        self.expect("{")?;
        let mut keys = Vec::new();
        loop {
            self.skip_newlines();
            while self.eat(";") {
                self.skip_newlines();
            }
            if self.eat("}") {
                break;
            }
            let time = self.unary(true)?;
            self.expect(":")?;
            let value = self.expr(true)?;
            // `1 .easeOut` : la courbe se lit comme un membre de la valeur.
            let (value, curve) = match value.kind {
                ExprKind::Member { recv, name } if is_curve(&name) => (*recv, Some(name)),
                kind => (
                    Expr {
                        kind,
                        span: value.span,
                    },
                    None,
                ),
            };
            keys.push(KeyDecl { time, value, curve });
            if !matches!(self.peek(), Tok::Punct(";") | Tok::Punct("}") | Tok::Newline) {
                return Err(self.unexpected("`;` ou une fin de ligne"));
            }
        }
        Ok(TrackDecl { name, keys })
    }

    fn view(&mut self) -> R<ViewDecl> {
        let span = self.span();
        self.bump();
        let name = self.ident("le nom de la vue")?;
        self.expect("{")?;
        let mut states = Vec::new();
        let mut params = Vec::new();
        let mut slots = Vec::new();
        let mut events = Vec::new();
        let mut timelines = Vec::new();
        let mut clocks = Vec::new();
        let mut body = Vec::new();
        loop {
            self.skip_newlines();
            if self.eat("}") {
                break;
            }
            if self.at_eof() {
                return Err(Diagnostic::error(
                    span,
                    format!("`view {name}` : `}}` de fin manquante"),
                ));
            }
            let r = match self.peek().clone() {
                Tok::Attr(a) if a == "State" => self.state().map(|s| states.push(s)),
                Tok::Attr(a) if a == "Timeline" => self.state().map(|s| timelines.push(s)),
                Tok::Attr(a) if a == "Param" => self.param().map(|p| params.push(p)),
                Tok::Attr(a) if a == "Slot" => {
                    self.bump();
                    self.ident("le nom de l'emplacement").map(|n| slots.push(n))
                }
                Tok::Attr(a) if a == "Clock" => {
                    self.bump();
                    self.is_ident("var").then(|| self.bump());
                    self.ident("le nom de l'horloge").map(|n| clocks.push(n))
                }
                Tok::Attr(a) if a == "Event" => {
                    let at = self.span();
                    self.bump();
                    self.ident("le nom de l'événement").and_then(|n| {
                        let ty = if self.eat("(") {
                            let t = self.type_name()?;
                            self.expect(")")?;
                            Some(t)
                        } else {
                            None
                        };
                        events.push(EventDecl {
                            name: n,
                            ty,
                            span: at,
                        });
                        Ok(())
                    })
                }
                Tok::Attr(a) => Err(Diagnostic::error(
                    self.span(),
                    format!("`@{a}` inconnu (`@State`, `@Param`, `@Slot`, `@Event`, `@Timeline`, `@Clock`)"),
                )),
                _ => self.stmt().map(|s| body.push(s)),
            };
            if let Err(e) = r {
                self.errors.push(e);
                self.sync();
            }
        }
        Ok(ViewDecl {
            name,
            params,
            slots,
            events,
            states,
            timelines,
            clocks,
            body,
            span,
        })
    }

    fn state(&mut self) -> R<StateDecl> {
        let span = self.span();
        self.bump();
        if !self.is_ident("var") {
            return Err(self.unexpected("`var`"));
        }
        self.bump();
        let name = self.ident("le nom de l'état")?;
        let ty = if self.eat(":") {
            Some(self.type_name()?)
        } else {
            None
        };
        self.expect("=")?;
        let init = self.expr(false)?;
        Ok(StateDecl { name, ty, init, span })
    }

    fn param(&mut self) -> R<ParamDecl> {
        let span = self.span();
        self.bump();
        if !self.is_ident("var") {
            return Err(self.unexpected("`var`"));
        }
        self.bump();
        let name = self.ident("le nom du paramètre")?;
        let ty = if self.eat(":") {
            Some(self.type_name()?)
        } else {
            None
        };
        let default = if self.eat("=") {
            Some(self.expr(false)?)
        } else {
            None
        };
        Ok(ParamDecl {
            name,
            ty,
            default,
            span,
        })
    }

    fn type_name(&mut self) -> R<TypeName> {
        if self.eat("[") {
            let inner = self.type_name()?;
            self.expect("]")?;
            return Ok(TypeName::Array(Box::new(inner)));
        }
        Ok(TypeName::Named(self.ident("un type")?))
    }

    fn theme(&mut self) -> R<ThemeDecl> {
        let span = self.span();
        self.bump();
        let name = self.ident("le nom du thème")?;
        let parent = if self.eat(":") {
            Some(self.ident("le thème parent")?)
        } else {
            None
        };
        self.expect("{")?;
        let mut entries = Vec::new();
        loop {
            self.skip_newlines();
            if self.eat("}") {
                break;
            }
            if self.at_eof() {
                return Err(Diagnostic::error(
                    span,
                    format!("`theme {name}` : `}}` de fin manquante"),
                ));
            }
            match self.theme_entry() {
                Ok(e) => entries.push(e),
                Err(e) => {
                    self.errors.push(e);
                    self.sync();
                }
            }
        }
        Ok(ThemeDecl {
            name,
            parent,
            entries,
            span,
        })
    }

    fn theme_entry(&mut self) -> R<ThemeEntry> {
        let span = self.span();
        let mut token = self.ident("un jeton")?;
        while self.eat(".") {
            token.push('.');
            token.push_str(&self.ident("un nom")?);
        }
        self.expect(":")?;
        let value = self.expr(false)?;
        // `clair / sombre`.
        let (light, dark) = match value.kind {
            ExprKind::Binary {
                op: BinaryOp::Div,
                lhs,
                rhs,
            } if !matches!(lhs.kind, ExprKind::Int(_) | ExprKind::Float(_)) => (*lhs, Some(*rhs)),
            kind => (
                Expr {
                    kind,
                    span: value.span,
                },
                None,
            ),
        };
        Ok(ThemeEntry {
            token,
            light,
            dark,
            span,
        })
    }

    fn block(&mut self) -> R<Vec<Stmt>> {
        self.expect("{")?;
        self.block_rest()
    }

    /// Après `{` : les instructions jusqu'à `}`.
    fn block_rest(&mut self) -> R<Vec<Stmt>> {
        let start = self.span();
        let mut out = Vec::new();
        loop {
            self.skip_newlines();
            if self.eat("}") {
                return Ok(out);
            }
            if self.at_eof() {
                return Err(Diagnostic::error(start, "`}` de fin manquante"));
            }
            match self.stmt() {
                Ok(s) => out.push(s),
                Err(e) => {
                    self.errors.push(e);
                    self.sync();
                }
            }
        }
    }

    fn stmt(&mut self) -> R<Stmt> {
        let span = self.span();
        if self.is_ident("if") {
            return self.if_stmt();
        }
        if self.is_ident("emit") {
            self.bump();
            let name = self.ident("le nom de l'événement")?;
            let value = if self.eat("(") {
                let v = if self.is(")") {
                    None
                } else {
                    Some(self.expr(false)?)
                };
                self.expect(")")?;
                v
            } else {
                None
            };
            return Ok(Stmt::Emit { name, value, span });
        }
        if self.is_ident("let") {
            self.bump();
            let name = self.ident("un nom")?;
            self.expect("=")?;
            let value = self.expr(false)?;
            return Ok(Stmt::Let {
                name: name.into(),
                value,
                span,
            });
        }
        let e = self.expr(false)?;
        let op = match self.peek() {
            Tok::Punct("=") => Some(AssignOp::Set),
            Tok::Punct("+=") => Some(AssignOp::Add),
            Tok::Punct("-=") => Some(AssignOp::Sub),
            Tok::Punct("*=") => Some(AssignOp::Mul),
            Tok::Punct("/=") => Some(AssignOp::Div),
            _ => None,
        };
        let stmt = match op {
            Some(op) => {
                self.bump();
                let value = self.expr(false)?;
                Stmt::Assign {
                    target: e,
                    op,
                    value,
                    span,
                }
            }
            None => Stmt::Expr(e),
        };
        if !matches!(
            self.peek(),
            Tok::Newline | Tok::Eof | Tok::Punct("}") | Tok::Punct(";")
        ) {
            return Err(self.unexpected("une fin de ligne"));
        }
        Ok(stmt)
    }

    fn if_stmt(&mut self) -> R<Stmt> {
        let span = self.span();
        self.bump();
        let cond = self.expr(true)?;
        let then = self.block()?;
        // `else` peut être sur la ligne suivante.
        let save = self.i;
        self.skip_newlines();
        let els = if self.is_ident("else") {
            self.bump();
            if self.is_ident("if") {
                Some(vec![self.if_stmt()?])
            } else {
                Some(self.block()?)
            }
        } else {
            self.i = save;
            None
        };
        Ok(Stmt::If {
            cond,
            then,
            els,
            span,
        })
    }

    /// `no_trailing` : dans une condition (`if x { … }`), `{` ouvre le bloc, pas une fermeture.
    fn expr(&mut self, no_trailing: bool) -> R<Expr> {
        let cond = self.binary(0, no_trailing)?;
        if self.is("?") {
            let span = cond.span;
            self.bump();
            let then = self.expr(no_trailing)?;
            self.expect(":")?;
            let els = self.expr(no_trailing)?;
            return Ok(Expr {
                kind: ExprKind::Ternary {
                    cond: Box::new(cond),
                    then: Box::new(then),
                    els: Box::new(els),
                },
                span,
            });
        }
        Ok(cond)
    }

    fn binary_op(&self) -> Option<(BinaryOp, u8)> {
        let op = match self.peek() {
            Tok::Punct("+") => BinaryOp::Add,
            Tok::Punct("-") => BinaryOp::Sub,
            Tok::Punct("*") => BinaryOp::Mul,
            Tok::Punct("/") => BinaryOp::Div,
            Tok::Punct("%") => BinaryOp::Rem,
            Tok::Punct("==") => BinaryOp::Eq,
            Tok::Punct("!=") => BinaryOp::Ne,
            Tok::Punct("<") => BinaryOp::Lt,
            Tok::Punct("<=") => BinaryOp::Le,
            Tok::Punct(">") => BinaryOp::Gt,
            Tok::Punct(">=") => BinaryOp::Ge,
            Tok::Punct("&&") => BinaryOp::And,
            Tok::Punct("||") => BinaryOp::Or,
            _ => return None,
        };
        Some((op, op.precedence()))
    }

    fn binary(&mut self, min: u8, nt: bool) -> R<Expr> {
        let mut lhs = self.range(nt)?;
        while let Some((op, prec)) = self.binary_op() {
            if prec < min || prec == 4 {
                break;
            }
            self.bump();
            let rhs = self.binary(prec + 1, nt)?;
            let span = lhs.span;
            lhs = Expr {
                kind: ExprKind::Binary {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                span,
            };
        }
        Ok(lhs)
    }

    /// `a..<b`, `a...b` : entre les comparaisons et les additions.
    fn range(&mut self, nt: bool) -> R<Expr> {
        let lo = self.additive(nt)?;
        let inclusive = match self.peek() {
            Tok::Punct("..<") => false,
            Tok::Punct("...") => true,
            _ => return Ok(lo),
        };
        self.bump();
        let hi = self.additive(nt)?;
        let span = lo.span;
        Ok(Expr {
            kind: ExprKind::Range {
                lo: Box::new(lo),
                hi: Box::new(hi),
                inclusive,
            },
            span,
        })
    }

    fn additive(&mut self, nt: bool) -> R<Expr> {
        let mut lhs = self.unary(nt)?;
        while let Some((op, prec)) = self.binary_op() {
            if prec < 5 {
                break;
            }
            self.bump();
            let rhs = self.unary(nt)?;
            // Multiplications d'abord.
            let rhs = if prec == 5 { self.mul_tail(rhs, nt)? } else { rhs };
            lhs = bin(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn mul_tail(&mut self, mut lhs: Expr, nt: bool) -> R<Expr> {
        while let Some((op, 6)) = self.binary_op() {
            self.bump();
            let rhs = self.unary(nt)?;
            lhs = bin(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn unary(&mut self, nt: bool) -> R<Expr> {
        let span = self.span();
        let op = match self.peek() {
            Tok::Punct("-") => UnaryOp::Neg,
            Tok::Punct("!") => UnaryOp::Not,
            _ => return self.postfix(nt),
        };
        self.bump();
        let expr = self.unary(nt)?;
        // `-3` reste un littéral.
        if op == UnaryOp::Neg {
            match expr.kind {
                ExprKind::Int(v) => {
                    return Ok(Expr {
                        kind: ExprKind::Int(-v),
                        span,
                    });
                }
                ExprKind::Float(v) => {
                    return Ok(Expr {
                        kind: ExprKind::Float(-v),
                        span,
                    });
                }
                _ => {}
            }
        }
        Ok(Expr {
            kind: ExprKind::Unary {
                op,
                expr: Box::new(expr),
            },
            span,
        })
    }

    fn postfix(&mut self, nt: bool) -> R<Expr> {
        let mut e = self.primary(nt)?;
        loop {
            if self.is("(") {
                self.bump();
                let args = self.args()?;
                let trailing = if !nt && self.is("{") {
                    Some(self.closure()?)
                } else {
                    None
                };
                let span = e.span;
                e = Expr {
                    kind: ExprKind::Call {
                        callee: Box::new(e),
                        args,
                        trailing,
                    },
                    span,
                };
                continue;
            }
            if self.is("[") {
                self.bump();
                let index = self.expr(false)?;
                self.expect("]")?;
                let span = e.span;
                e = Expr {
                    kind: ExprKind::Index {
                        recv: Box::new(e),
                        index: Box::new(index),
                    },
                    span,
                };
                continue;
            }
            // `.nom`, aussi au début de la ligne suivante (les modifiers).
            let mut k = 0;
            while matches!(self.peek_at(k), Tok::Newline) {
                k += 1;
            }
            if matches!(self.peek_at(k), Tok::Punct(".")) && matches!(self.peek_at(k + 1), Tok::Ident(_)) {
                for _ in 0..=k {
                    self.bump();
                }
                let span = self.span();
                let name = self.ident("un nom")?;
                e = Expr {
                    kind: ExprKind::Member {
                        recv: Box::new(e),
                        name,
                    },
                    span,
                };
                // `.onTapGesture { … }` : un appel sans argument, avec une fermeture.
                if !nt && self.is("{") {
                    let trailing = Some(self.closure()?);
                    e = Expr {
                        kind: ExprKind::Call {
                            callee: Box::new(e),
                            args: Vec::new(),
                            trailing,
                        },
                        span,
                    };
                }
                continue;
            }
            return Ok(e);
        }
    }

    fn args(&mut self) -> R<Vec<Arg>> {
        let mut args = Vec::new();
        loop {
            if self.eat(")") {
                return Ok(args);
            }
            let label = match (self.peek().clone(), self.peek_at(1)) {
                (Tok::Ident(n), Tok::Punct(":")) => {
                    self.bump();
                    self.bump();
                    Some(n)
                }
                _ => None,
            };
            let value = self.expr(false)?;
            args.push(Arg { label, value });
            if !self.eat(",") {
                self.expect(")")?;
                return Ok(args);
            }
        }
    }

    fn closure(&mut self) -> R<Closure> {
        self.expect("{")?;
        // `{ a, b in … }`.
        let mut params = Vec::new();
        let mut k = 0;
        loop {
            match (self.peek_at(k), self.peek_at(k + 1)) {
                (Tok::Ident(_), Tok::Punct(",")) => k += 2,
                (Tok::Ident(_), Tok::Ident(n)) if n == "in" => {
                    for _ in 0..=k / 2 {
                        if let Tok::Ident(n) = self.bump().tok {
                            params.push(n.into());
                        }
                        self.eat(",");
                    }
                    self.bump();
                    break;
                }
                _ => break,
            }
        }
        let body = self.block_rest()?;
        Ok(Closure { params, body })
    }

    fn primary(&mut self, nt: bool) -> R<Expr> {
        let span = self.span();
        let kind = match self.peek().clone() {
            Tok::Int(v) => {
                self.bump();
                ExprKind::Int(v)
            }
            Tok::Float(v) => {
                self.bump();
                ExprKind::Float(v)
            }
            Tok::Color(v, alpha) => {
                self.bump();
                let c = if alpha {
                    let a = (v & 0xFF) as f32 / 255.0;
                    let rgb = Color::hex(v >> 8);
                    Color::rgba(rgb.r, rgb.g, rgb.b, a)
                } else {
                    Color::hex(v)
                };
                ExprKind::Color(c)
            }
            Tok::Str(pieces) => {
                self.bump();
                let mut parts = Vec::new();
                for p in pieces {
                    match p {
                        StrPiece::Text(t) => parts.push(StrPart::Text(t)),
                        StrPiece::Code(code, at) => parts.push(StrPart::Expr(parse_expr_at(&code, at)?)),
                    }
                }
                ExprKind::Str(parts)
            }
            Tok::Ident(n) => {
                self.bump();
                match n.as_str() {
                    "true" => ExprKind::Bool(true),
                    "false" => ExprKind::Bool(false),
                    _ => {
                        let e = Expr {
                            kind: ExprKind::Ident(n),
                            span,
                        };
                        // `VStack { … }` : une fermeture sans parenthèses.
                        if !nt && self.is("{") {
                            let trailing = Some(self.closure()?);
                            return Ok(Expr {
                                kind: ExprKind::Call {
                                    callee: Box::new(e),
                                    args: Vec::new(),
                                    trailing,
                                },
                                span,
                            });
                        }
                        return Ok(e);
                    }
                }
            }
            Tok::Punct(".") => {
                self.bump();
                ExprKind::Symbol(self.ident("un nom après `.`")?)
            }
            // `{ x: 1, y: 2 }` : un enregistrement.
            Tok::Punct("{")
                if matches!(
                    (self.peek_at(1), self.peek_at(2)),
                    (Tok::Ident(_), Tok::Punct(":"))
                ) =>
            {
                self.bump();
                let mut fields = Vec::new();
                loop {
                    if self.eat("}") {
                        break;
                    }
                    let n = self.ident("un champ")?;
                    self.expect(":")?;
                    fields.push((n, self.expr(false)?));
                    if !self.eat(",") {
                        self.expect("}")?;
                        break;
                    }
                }
                ExprKind::Record(fields)
            }
            Tok::Punct("(") => {
                self.bump();
                let e = self.expr(false)?;
                self.expect(")")?;
                return Ok(e);
            }
            Tok::Punct("[") => {
                self.bump();
                let mut items = Vec::new();
                loop {
                    if self.eat("]") {
                        break;
                    }
                    items.push(self.expr(false)?);
                    if !self.eat(",") {
                        self.expect("]")?;
                        break;
                    }
                }
                ExprKind::Array(items)
            }
            _ => return Err(self.unexpected("une expression")),
        };
        Ok(Expr { kind, span })
    }
}

/// Les mots qui commencent une déclaration (pour reprendre après une erreur).
const TOP: &[&str] = &["view", "theme", "func", "timeline", "import"];

pub(crate) fn is_curve(name: &str) -> bool {
    matches!(
        name,
        "linear" | "easeIn" | "easeOut" | "easeInOut" | "circEaseIn" | "circEaseOut" | "circEaseInOut"
    )
}

fn bin(op: BinaryOp, lhs: Expr, rhs: Expr) -> Expr {
    let span = lhs.span;
    Expr {
        kind: ExprKind::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
        span,
    }
}
