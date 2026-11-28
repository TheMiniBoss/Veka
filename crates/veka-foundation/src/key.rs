//! Clavier (étape 9) : touches logiques et modificateurs, communs à la plateforme et à l'interface.

use core::ops::{BitOr, BitOrAssign};

/// Une touche, selon la disposition du clavier (sur un AZERTY, la touche à droite de Tab donne `'a'`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    /// Une touche qui écrit un caractère, en minuscule (`'a'`, `'1'`, `' '`, `'é'`…). Maj ne change pas
    /// le caractère : elle est dans les [`Modifiers`].
    Character(char),
    Enter,
    Tab,
    Escape,
    Backspace,
    Delete,
    Insert,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Home,
    End,
    PageUp,
    PageDown,
    /// F1 à F24.
    F(u8),
}

impl Key {
    /// L'espace.
    pub const SPACE: Key = Key::Character(' ');

    /// Le nom de la touche dans le schéma et en `.vk` (`"return"`, `"escape"`, `"a"`…), comme `KeyEquivalent`
    /// de SwiftUI.
    pub fn name(self) -> &'static str {
        match self {
            Key::Character(_) => "character",
            Key::Enter => "return",
            Key::Tab => "tab",
            Key::Escape => "escape",
            Key::Backspace => "delete",
            Key::Delete => "deleteForward",
            Key::Insert => "insert",
            Key::ArrowLeft => "leftArrow",
            Key::ArrowRight => "rightArrow",
            Key::ArrowUp => "upArrow",
            Key::ArrowDown => "downArrow",
            Key::Home => "home",
            Key::End => "end",
            Key::PageUp => "pageUp",
            Key::PageDown => "pageDown",
            Key::F(_) => "function",
        }
    }

    /// Une touche à partir de son nom (`"return"`, `"escape"`, `"space"`, `"f5"`) ou d'un caractère seul.
    pub fn from_name(name: &str) -> Option<Key> {
        let mut chars = name.chars();
        if let (Some(c), None) = (chars.next(), chars.clone().next()) {
            return Some(Key::Character(c.to_lowercase().next().unwrap_or(c)));
        }
        Some(match name {
            "return" | "enter" => Key::Enter,
            "tab" => Key::Tab,
            "escape" => Key::Escape,
            "delete" | "backspace" => Key::Backspace,
            "deleteForward" => Key::Delete,
            "insert" => Key::Insert,
            "leftArrow" => Key::ArrowLeft,
            "rightArrow" => Key::ArrowRight,
            "upArrow" => Key::ArrowUp,
            "downArrow" => Key::ArrowDown,
            "home" => Key::Home,
            "end" => Key::End,
            "pageUp" => Key::PageUp,
            "pageDown" => Key::PageDown,
            "space" => Key::SPACE,
            _ => {
                let n: u8 = name.strip_prefix('f')?.parse().ok()?;
                if (1..=24).contains(&n) {
                    Key::F(n)
                } else {
                    return None;
                }
            }
        })
    }
}

/// Les touches de modification tenues.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers(u8);

impl Modifiers {
    pub const NONE: Modifiers = Modifiers(0);
    pub const SHIFT: Modifiers = Modifiers(1);
    pub const CONTROL: Modifiers = Modifiers(2);
    /// Alt sur Windows, ⌥ sur le Mac.
    pub const ALT: Modifiers = Modifiers(4);
    /// ⌘ sur le Mac, touche Windows sur Windows.
    pub const META: Modifiers = Modifiers(8);
    /// La touche des raccourcis : ⌘ sur le Mac, Ctrl ailleurs (un seul raccourci pour les deux).
    #[cfg(target_os = "macos")]
    pub const COMMAND: Modifiers = Modifiers::META;
    /// La touche des raccourcis : ⌘ sur le Mac, Ctrl ailleurs (un seul raccourci pour les deux).
    #[cfg(not(target_os = "macos"))]
    pub const COMMAND: Modifiers = Modifiers::CONTROL;

    pub const fn contains(self, other: Modifiers) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn from_bits(bits: u8) -> Modifiers {
        Modifiers(bits & 15)
    }

    /// Sans Maj (pour comparer un raccourci quand Maj ne compte pas).
    pub const fn without_shift(self) -> Modifiers {
        Modifiers(self.0 & !1)
    }
}

impl BitOr for Modifiers {
    type Output = Modifiers;
    fn bitor(self, rhs: Modifiers) -> Modifiers {
        Modifiers(self.0 | rhs.0)
    }
}

impl BitOrAssign for Modifiers {
    fn bitor_assign(&mut self, rhs: Modifiers) {
        self.0 |= rhs.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noms() {
        assert_eq!(Key::from_name("return"), Some(Key::Enter));
        assert_eq!(Key::from_name("A"), Some(Key::Character('a')));
        assert_eq!(Key::from_name("space"), Some(Key::SPACE));
        assert_eq!(Key::from_name("f5"), Some(Key::F(5)));
        assert_eq!(Key::from_name("f30"), None);
        assert_eq!(Key::from_name("rien"), None);
        assert_eq!(Key::from_name(Key::Escape.name()), Some(Key::Escape));
    }

    #[test]
    fn modificateurs() {
        let m = Modifiers::SHIFT | Modifiers::CONTROL;
        assert!(m.contains(Modifiers::SHIFT));
        assert!(!m.contains(Modifiers::ALT));
        assert_eq!(m.without_shift(), Modifiers::CONTROL);
    }
}

/// La phase d'un défilement (étape 10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScrollPhase {
    /// Un cran de molette : Veka l'anime pour qu'il glisse en douceur.
    Wheel,
    /// Les doigts se posent sur le pavé tactile : l'élan en cours s'arrête.
    Began,
    /// Le geste continue : le contenu suit le doigt. Un pavé qui ne dit pas quand le geste commence et
    /// finit (Windows) n'envoie que cette phase.
    Changed,
    /// Les doigts se lèvent : Veka lance son élan, puis le ressort des bords.
    Ended,
}
