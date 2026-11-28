//! Le lexer : texte → jetons, avec leur position.

use crate::{Diagnostic, Span};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Tok {
    Ident(String),
    /// `@State`, `@Param`… (sans le `@`).
    Attr(String),
    /// `$nom` : un état passé à une vue qui le lit et l'écrit (sans le `$`).
    Binding(String),
    Int(i64),
    Float(f64),
    /// Une chaîne : morceaux de texte et expressions `\( … )` (leur source et sa position).
    Str(Vec<StrPiece>),
    /// `#RRGGBB` ou `#RRGGBBAA`.
    Color(u32, bool),
    Punct(&'static str),
    Newline,
    Eof,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum StrPiece {
    Text(String),
    Code(String, Span),
}

#[derive(Clone, Debug)]
pub(crate) struct Token {
    pub tok: Tok,
    pub span: Span,
}

/// Du plus long au plus court : le premier qui correspond gagne.
const PUNCTS: &[&str] = &[
    "..<", "...", "->", "==", "!=", "<=", ">=", "&&", "||", "+=", "-=", "*=", "/=", "(", ")", "{", "}", "[",
    "]", ",", ":", ".", ";", "+", "-", "*", "/", "%", "=", "<", ">", "!", "?",
];

struct Lexer<'a> {
    src: &'a [u8],
    text: &'a str,
    pos: usize,
    line: u32,
    col: u32,
    out: Vec<Token>,
    errors: Vec<Diagnostic>,
    /// Parenthèses ouvertes : 1 pour `(` et `[` (retours à la ligne ignorés), 0 pour `{`.
    depth: Vec<u8>,
}

/// Découpe `text` ; `base` : position de son premier caractère.
pub(crate) fn lex(text: &str, base: Span) -> (Vec<Token>, Vec<Diagnostic>) {
    let mut l = Lexer {
        src: text.as_bytes(),
        text,
        pos: 0,
        line: base.line,
        col: base.col,
        out: Vec::new(),
        errors: Vec::new(),
        depth: Vec::new(),
    };
    l.run();
    (l.out, l.errors)
}

impl Lexer<'_> {
    fn span(&self) -> Span {
        Span {
            line: self.line,
            col: self.col,
        }
    }

    fn peek(&self, k: usize) -> Option<u8> {
        self.src.get(self.pos + k).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.text[self.pos..].chars().next()?;
        self.pos += c.len_utf8();
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn push(&mut self, tok: Tok, span: Span) {
        self.out.push(Token { tok, span });
    }

    fn error(&mut self, span: Span, msg: impl Into<String>) {
        self.errors.push(Diagnostic::error(span, msg));
    }

    fn run(&mut self) {
        while let Some(c) = self.peek(0) {
            let start = self.span();
            match c {
                b'\n' => {
                    self.bump();
                    // Dans `( )` et `[ ]` on ignore les retours à la ligne ; dans `{ }` ils comptent.
                    if self.depth.last() != Some(&1)
                        && !matches!(self.out.last().map(|t| &t.tok), Some(Tok::Newline) | None)
                    {
                        self.push(Tok::Newline, start);
                    }
                }
                b' ' | b'\t' | b'\r' => {
                    self.bump();
                }
                b'/' if self.peek(1) == Some(b'/') => {
                    while self.peek(0).is_some_and(|c| c != b'\n') {
                        self.bump();
                    }
                }
                b'/' if self.peek(1) == Some(b'*') => {
                    self.bump();
                    self.bump();
                    loop {
                        match self.peek(0) {
                            None => {
                                self.error(start, "commentaire `/*` sans `*/`");
                                break;
                            }
                            Some(b'*') if self.peek(1) == Some(b'/') => {
                                self.bump();
                                self.bump();
                                break;
                            }
                            _ => {
                                self.bump();
                            }
                        }
                    }
                }
                b'"' => self.string(start),
                b'#' => self.color(start),
                b'@' => {
                    self.bump();
                    let name = self.ident_text();
                    if name.is_empty() {
                        self.error(start, "`@` doit être suivi d'un nom (`@State`…)");
                    }
                    self.push(Tok::Attr(name), start);
                }
                b'$' => {
                    self.bump();
                    let name = self.ident_text();
                    if name.is_empty() {
                        self.error(start, "`$` doit être suivi du nom d'un `@State` (`$nom`)");
                    }
                    self.push(Tok::Binding(name), start);
                }
                b'0'..=b'9' => self.number(start),
                c if c == b'_' || c.is_ascii_alphabetic() || c >= 0x80 => {
                    let name = self.ident_text();
                    self.push(Tok::Ident(name), start);
                }
                _ => {
                    let rest = &self.text[self.pos..];
                    match PUNCTS.iter().find(|p| rest.starts_with(**p)) {
                        Some(p) => {
                            for _ in 0..p.len() {
                                self.bump();
                            }
                            match *p {
                                "(" | "[" => self.depth.push(1),
                                "{" => self.depth.push(0),
                                ")" | "]" | "}" => {
                                    self.depth.pop();
                                }
                                _ => {}
                            }
                            self.push(Tok::Punct(p), start);
                        }
                        None => {
                            let ch = self.bump().unwrap_or('?');
                            self.error(start, format!("caractère inattendu `{ch}`"));
                        }
                    }
                }
            }
        }
        let end = self.span();
        self.push(Tok::Newline, end);
        self.push(Tok::Eof, end);
    }

    fn ident_text(&mut self) -> String {
        let start = self.pos;
        while let Some(c) = self.text[self.pos..].chars().next() {
            if c == '_' || c.is_alphanumeric() {
                self.bump();
            } else {
                break;
            }
        }
        self.text[start..self.pos].to_owned()
    }

    fn number(&mut self, start: Span) {
        let s = self.pos;
        while self.peek(0).is_some_and(|c| c.is_ascii_digit() || c == b'_') {
            self.bump();
        }
        let mut float = false;
        // `1.5` mais pas `0..<10` ni `1.foo`.
        if self.peek(0) == Some(b'.') && self.peek(1).is_some_and(|c| c.is_ascii_digit()) {
            float = true;
            self.bump();
            while self.peek(0).is_some_and(|c| c.is_ascii_digit() || c == b'_') {
                self.bump();
            }
        }
        if matches!(self.peek(0), Some(b'e' | b'E'))
            && (self.peek(1).is_some_and(|c| c.is_ascii_digit())
                || (matches!(self.peek(1), Some(b'-' | b'+'))
                    && self.peek(2).is_some_and(|c| c.is_ascii_digit())))
        {
            float = true;
            self.bump();
            self.bump();
            while self.peek(0).is_some_and(|c| c.is_ascii_digit()) {
                self.bump();
            }
        }
        let text: String = self.text[s..self.pos].chars().filter(|&c| c != '_').collect();
        if float {
            match text.parse() {
                Ok(v) => self.push(Tok::Float(v), start),
                Err(_) => self.error(start, format!("nombre invalide `{text}`")),
            }
        } else {
            match text.parse() {
                Ok(v) => self.push(Tok::Int(v), start),
                Err(_) => self.error(start, format!("entier trop grand `{text}`")),
            }
        }
    }

    fn color(&mut self, start: Span) {
        self.bump();
        let s = self.pos;
        while self.peek(0).is_some_and(|c| c.is_ascii_hexdigit()) {
            self.bump();
        }
        let hex = &self.text[s..self.pos];
        match (hex.len(), u32::from_str_radix(hex, 16)) {
            (6, Ok(v)) => self.push(Tok::Color(v, false), start),
            (8, Ok(v)) => self.push(Tok::Color(v, true), start),
            _ => self.error(
                start,
                format!("couleur `#{hex}` : 6 ou 8 chiffres hexadécimaux attendus"),
            ),
        }
    }

    fn string(&mut self, start: Span) {
        self.bump();
        let mut pieces = Vec::new();
        let mut text = String::new();
        loop {
            match self.peek(0) {
                None | Some(b'\n') => {
                    self.error(start, "chaîne sans `\"` de fin");
                    break;
                }
                Some(b'"') => {
                    self.bump();
                    break;
                }
                Some(b'\\') => {
                    let esc = self.span();
                    self.bump();
                    match self.bump() {
                        Some('n') => text.push('\n'),
                        Some('t') => text.push('\t'),
                        Some('"') => text.push('"'),
                        Some('\\') => text.push('\\'),
                        Some('(') => {
                            if !text.is_empty() {
                                pieces.push(StrPiece::Text(std::mem::take(&mut text)));
                            }
                            let code_start = self.span();
                            let s = self.pos;
                            let mut depth = 1;
                            loop {
                                match self.peek(0) {
                                    None | Some(b'\n') => {
                                        self.error(esc, "`\\(` sans `)` de fin");
                                        break;
                                    }
                                    Some(b'(') => depth += 1,
                                    Some(b')') => {
                                        depth -= 1;
                                        if depth == 0 {
                                            break;
                                        }
                                    }
                                    _ => {}
                                }
                                self.bump();
                            }
                            let code = self.text[s..self.pos].to_owned();
                            self.bump();
                            pieces.push(StrPiece::Code(code, code_start));
                        }
                        Some(c) => self.error(esc, format!("échappement inconnu `\\{c}`")),
                        None => {}
                    }
                }
                Some(_) => {
                    if let Some(c) = self.bump() {
                        text.push(c);
                    }
                }
            }
        }
        if !text.is_empty() || pieces.is_empty() {
            pieces.push(StrPiece::Text(text));
        }
        self.push(Tok::Str(pieces), start);
    }
}
