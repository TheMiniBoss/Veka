//! Clavier et focus (étape 9) : `.focusable`, `.focused`, `.on_key_press`, `.keyboard_shortcut`.

use std::rc::Rc;

use veka_foundation::{Key, Modifiers, Rect};
use veka_graphics::{CornerRadii, Shape};

use crate::element::{Action, Element};
use crate::primitive::{EventCx, EventResult, KeyEvent, PaintCx, PointerEvent, Primitive};
use crate::state::State;

/// Ce que renvoie un `.on_key_press` : la touche est prise, ou laissée aux vues au-dessus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyPressResult {
    Handled,
    Ignored,
}

/// Une touche, telle que la reçoit `.on_key_press`.
pub type KeyPress = KeyEvent;

/// Le primitive de `.focusable()` : la vue prend le focus et l'anneau d'accent l'entoure.
#[derive(Clone, Debug, PartialEq)]
pub struct Focusable;

impl Primitive for Focusable {
    fn name(&self) -> &'static str {
        "focusable"
    }

    fn focusable(&self) -> bool {
        true
    }

    // Le clic sur la vue lui donne le focus, sans prendre le pointeur à ce qui est dedans.
    fn hit_shape(&self) -> Option<Shape> {
        Some(Shape::Rectangle)
    }

    fn pointer(&self, _cx: &mut EventCx<'_>, _event: PointerEvent) -> EventResult {
        EventResult::default()
    }

    fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        cx.paint_children();
        if cx.is_focused() {
            paint_focus_ring(cx, rect);
        }
    }
}

/// L'anneau du focus, comme sur macOS : la couleur d'accent, à moitié transparente, autour de la vue.
pub fn paint_focus_ring(cx: &mut PaintCx<'_>, rect: Rect) {
    let env = cx.env().clone();
    let accent = env.color(&env.accent).with_alpha(0.5);
    let r = rect.inset(veka_foundation::Insets::all(-2.0));
    cx.list()
        .stroke(Shape::RoundedRectangle(CornerRadii::all(6.0)), r, accent, 3.0);
}

/// Le primitive de `.focused(…)` : relie le focus du sous-arbre à un état.
#[derive(Clone)]
pub struct FocusBinding {
    /// L'état demande-t-il le focus pour ce sous-arbre ?
    pub is_on: Rc<dyn Fn() -> bool>,
    /// Le sous-arbre prend (`true`) ou perd (`false`) le focus.
    pub set: Rc<dyn Fn(bool)>,
}

impl std::fmt::Debug for FocusBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FocusBinding")
    }
}

impl Primitive for FocusBinding {
    fn name(&self) -> &'static str {
        "focused"
    }
}

/// Le primitive de `.on_key_press`.
#[derive(Clone)]
pub struct KeyPressPrim {
    /// La touche attendue (`None` : toutes).
    pub key: Option<Key>,
    pub action: Rc<dyn Fn(KeyPress) -> KeyPressResult>,
}

impl std::fmt::Debug for KeyPressPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyPressPrim").field("key", &self.key).finish()
    }
}

impl Primitive for KeyPressPrim {
    fn name(&self) -> &'static str {
        "onKeyPress"
    }

    fn key(&self, _cx: &mut EventCx<'_>, event: &KeyEvent) -> EventResult {
        if self.key.is_some_and(|k| k != event.key) {
            return EventResult::default();
        }
        // L'action peut changer un état : elle tourne tout de suite (rien n'est emprunté ici).
        let handled = (self.action)(*event) == KeyPressResult::Handled;
        EventResult {
            handled,
            ..Default::default()
        }
    }
}

/// Le primitive de `.keyboard_shortcut`.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyboardShortcut {
    pub key: Key,
    pub modifiers: Modifiers,
}

impl Primitive for KeyboardShortcut {
    fn name(&self) -> &'static str {
        "keyboardShortcut"
    }

    fn keyboard_shortcut(&self) -> Option<(Key, Modifiers)> {
        Some((self.key, self.modifiers))
    }
}

impl Element {
    /// La vue peut prendre le focus du clavier (clic, Tab), et reçoit alors les touches de
    /// `.on_key_press`. Un anneau à la couleur d'accent l'entoure tant qu'elle l'a.
    pub fn focusable(self) -> Element {
        self.wrap(Focusable)
    }

    /// Relie le focus à un booléen, comme `.focused($estActif)` de SwiftUI : il passe à `true` quand
    /// le sous-arbre prend le focus, et le mettre à `true` donne le focus à la première vue du sous-arbre
    /// qui peut le prendre.
    pub fn focused(self, state: State<bool>) -> Element {
        self.wrap(FocusBinding {
            is_on: Rc::new(move || state.try_with(|v| *v).unwrap_or(false)),
            set: Rc::new(move |on| {
                if state.try_with(|v| *v) != Some(on) && state.is_alive() {
                    state.set(on);
                }
            }),
        })
    }

    /// Relie le focus à une valeur, comme `.focused($focus, equals: .nom)` de SwiftUI : `state` vaut
    /// `Some(value)` quand le sous-arbre a le focus, `None` quand plus rien de relié ne l'a.
    pub fn focused_equals<T: Clone + PartialEq + 'static>(
        self,
        state: State<Option<T>>,
        value: T,
    ) -> Element {
        let wanted = value.clone();
        self.wrap(FocusBinding {
            is_on: Rc::new(move || state.try_with(|v| v.as_ref() == Some(&wanted)).unwrap_or(false)),
            set: Rc::new(move |on| {
                let now = state.try_with(|v| v.clone());
                match (on, now) {
                    (true, Some(v)) if v.as_ref() != Some(&value) => state.set(Some(value.clone())),
                    (false, Some(Some(v))) if v == value => state.set(None),
                    _ => {}
                }
            }),
        })
    }

    /// Réagit à une touche, comme `.onKeyPress(.return) { … }` de SwiftUI. La touche vient de la vue qui
    /// a le focus et remonte jusqu'à la racine ; `KeyPressResult::Handled` l'arrête là.
    pub fn on_key_press(self, key: Key, action: impl Fn(KeyPress) -> KeyPressResult + 'static) -> Element {
        self.wrap(KeyPressPrim {
            key: Some(key),
            action: Rc::new(action),
        })
    }

    /// Comme [`on_key_press`](Self::on_key_press), pour toutes les touches.
    pub fn on_any_key_press(self, action: impl Fn(KeyPress) -> KeyPressResult + 'static) -> Element {
        self.wrap(KeyPressPrim {
            key: None,
            action: Rc::new(action),
        })
    }

    /// Déclenche le bouton (ou le `.on_tap_gesture`) de n'importe où dans la fenêtre, comme
    /// `.keyboardShortcut` de SwiftUI : `Key::Enter` pour le bouton par défaut, `Key::Escape` pour
    /// annuler, `(Key::Character('s'), Modifiers::COMMAND)` pour ⌘S / Ctrl+S.
    pub fn keyboard_shortcut(self, key: Key, modifiers: Modifiers) -> Element {
        self.wrap(KeyboardShortcut { key, modifiers })
    }
}

/// Une action sans valeur de retour, prise comme rappel de touche qui traite toujours la touche
/// (`.onKeyPress` du schéma et du `.vk`).
pub(crate) fn handled(action: Action) -> Rc<dyn Fn(KeyPress) -> KeyPressResult> {
    Rc::new(move |_| {
        action.call();
        KeyPressResult::Handled
    })
}
