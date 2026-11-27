//! Le langage `.vk` de Veka : une interface décrite dans un fichier texte, relue à chaud.
//!
//! ```text
//! view Compteur {
//!     @State var clics = 0
//!
//!     VStack(spacing: 16) {
//!         Text("Bonjour Veka")
//!             .font(.title)
//!         Button("Clics : \(clics)") {
//!             clics += 1
//!         }
//!     }
//!     .padding(24)
//! }
//! ```
//!
//! [`Program`] charge un fichier et donne ses vues comme des [`Element`](veka_ui::Element) ;
//! [`Program::reload`] remplace le fichier sans perdre les états. Le parseur ([`parse`]) et
//! l'imprimeur ([`print`]) sont publics : relire le texte imprimé redonne le même arbre.

pub mod ast;
mod eval;
mod lexer;
mod parser;
mod printer;
mod program;
mod value;

pub use ast::Span;
pub use parser::{parse, parse_expr};
pub use printer::{print, print_expr};
pub use program::Program;

/// Gravité d'un [`Diagnostic`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// Une erreur (ou un avertissement) à une position du fichier.
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub span: Span,
    pub message: String,
    pub severity: Severity,
}

impl Diagnostic {
    pub fn error(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
            severity: Severity::Error,
        }
    }

    pub fn warning(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
            severity: Severity::Warning,
        }
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} : {}", self.span, self.message)
    }
}

impl std::error::Error for Diagnostic {}
