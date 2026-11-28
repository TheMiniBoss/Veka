//! Le presse-papiers (étape 9). L'hôte (l'app native) branche celui du système ; sans lui, un
//! presse-papiers interne au fil sert (tests, moteurs de jeu qui n'en donnent pas).

use std::cell::RefCell;
use std::rc::Rc;

/// Un presse-papiers de texte.
pub trait Clipboard {
    fn get(&self) -> Option<String>;
    fn set(&self, text: &str);
}

#[derive(Default)]
struct Internal(RefCell<Option<String>>);

impl Clipboard for Internal {
    fn get(&self) -> Option<String> {
        self.0.borrow().clone()
    }

    fn set(&self, text: &str) {
        *self.0.borrow_mut() = Some(text.to_owned());
    }
}

thread_local! {
    static CLIPBOARD: RefCell<Rc<dyn Clipboard>> = RefCell::new(Rc::new(Internal::default()));
}

/// Branche le presse-papiers du système pour ce fil.
pub fn set_clipboard(clipboard: Rc<dyn Clipboard>) {
    CLIPBOARD.with(|c| *c.borrow_mut() = clipboard);
}

/// Le texte du presse-papiers.
pub fn clipboard_text() -> Option<String> {
    let c = CLIPBOARD.with(|c| c.borrow().clone());
    c.get()
}

/// Met du texte dans le presse-papiers.
pub fn set_clipboard_text(text: &str) {
    let c = CLIPBOARD.with(|c| c.borrow().clone());
    c.set(text);
}
