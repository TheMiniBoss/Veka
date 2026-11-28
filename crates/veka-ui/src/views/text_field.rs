//! Champs de texte (étape 9) : `TextField` et `SecureField`, avec l'éditeur de `veka-text`.

use std::ops::Range;
use std::rc::Rc;

use veka_colora::Token;
use veka_foundation::{Insets, Key, Modifiers, Point, Rect, Size};
use veka_graphics::{CornerRadii, DrawCommand, Shape};
use veka_text::{EditBuffer, EditOp, EditView, TextLayout, TextStyle, TextSystem};

use crate::element::{Action, Element};
use crate::primitive::{
    EventCx, EventResult, KeyEvent, LayoutCx, PaintCx, PointerEvent, Primitive, Proposal, TextInput,
};
use crate::state::State;

/// Marge du cadre autour du texte (`.roundedBorder`).
const PADDING: Insets = Insets::new(4.0, 7.0, 4.0, 7.0);
/// Largeur idéale d'un champ (quand le parent ne propose rien).
const IDEAL_WIDTH: f32 = 180.0;
/// Annulations gardées.
const UNDO_DEPTH: usize = 100;

/// Le primitive des champs de texte.
#[derive(Clone)]
pub struct TextFieldPrim {
    pub placeholder: String,
    /// Le texte de l'état relié.
    pub get: Rc<dyn Fn() -> String>,
    /// Écrit le texte dans l'état relié.
    pub set: Rc<dyn Fn(String)>,
    pub secure: bool,
}

impl std::fmt::Debug for TextFieldPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextFieldPrim")
            .field("placeholder", &self.placeholder)
            .field("secure", &self.secure)
            .finish()
    }
}

/// L'état d'un champ : l'éditeur, ce qui est dessiné, le défilement et les annulations.
#[derive(Default)]
pub struct TextFieldState {
    buffer: Option<EditBuffer>,
    view: EditView,
    placeholder: Option<Rc<TextLayout>>,
    /// Décalage du texte vers la gauche, pour garder le curseur visible.
    scroll: f32,
    /// La zone du texte (sans le cadre), au dernier placement.
    area: Rect,
    dragging: bool,
    undo: Vec<(String, Range<usize>)>,
    redo: Vec<(String, Range<usize>)>,
    /// La dernière modification était une frappe (les frappes d'affilée s'annulent ensemble).
    typing: bool,
}

impl TextFieldState {
    fn buffer(&mut self, style: &TextStyle, secure: bool) -> &mut EditBuffer {
        let b = self
            .buffer
            .get_or_insert_with(|| EditBuffer::new(style.clone(), secure));
        b.set_style(style);
        b
    }

    /// Recalcule ce qui est dessiné et garde le curseur visible.
    fn refresh(&mut self, text: &mut TextSystem) {
        let Some(b) = self.buffer.as_mut() else { return };
        self.view = b.view(text);
        let width = self.area.width().max(1.0);
        let content = self.view.layout.size.width;
        if let Some(c) = self.view.cursor {
            if c.x() - self.scroll > width - 2.0 {
                self.scroll = c.x() - width + 2.0;
            } else if c.x() < self.scroll {
                self.scroll = c.x();
            }
        }
        self.scroll = self.scroll.clamp(0.0, (content - width + 2.0).max(0.0));
    }
}

impl TextFieldPrim {
    fn style(&self, env: &crate::env::Environment) -> TextStyle {
        let mut style = env.text_style();
        if !env.text_field_plain {
            style.color = env.color(&Token::TextFieldText.into());
        }
        style
    }

    fn insets(env: &crate::env::Environment) -> Insets {
        if env.text_field_plain {
            Insets::all(0.0)
        } else {
            PADDING
        }
    }

    /// Applique une modification, puis renvoie l'action qui écrit le nouveau texte s'il a changé.
    fn edit(&self, cx: &mut EventCx<'_>, ops: &[EditOp<'_>], typing: bool) -> Option<Action> {
        let style = self.style(cx.env());
        let secure = self.secure;
        let (st, text) = cx.state_and_text::<TextFieldState>();
        let b = st.buffer(&style, secure);
        let before = (b.text(), b.selection());
        for op in ops {
            b.apply(text, *op);
        }
        let after = b.text();
        let changed = after != before.0;
        if changed {
            // Une frappe qui suit une frappe s'annule avec elle.
            if !(typing && st.typing) {
                st.undo.push(before);
                if st.undo.len() > UNDO_DEPTH {
                    st.undo.remove(0);
                }
            }
            st.redo.clear();
        }
        st.typing = typing && changed;
        st.refresh(text);
        changed.then(|| self.write(after))
    }

    fn write(&self, text: String) -> Action {
        let set = self.set.clone();
        Action::new(move || set(text.clone()))
    }

    /// Annule (ou refait) la dernière modification.
    fn undo(&self, cx: &mut EventCx<'_>, redo: bool) -> Option<Action> {
        let style = self.style(cx.env());
        let secure = self.secure;
        let (st, text) = cx.state_and_text::<TextFieldState>();
        let (from, to) = if redo {
            (&mut st.redo, &mut st.undo)
        } else {
            (&mut st.undo, &mut st.redo)
        };
        let (old, sel) = from.pop()?;
        let b = st
            .buffer
            .get_or_insert_with(|| EditBuffer::new(style.clone(), secure));
        to.push((b.text(), b.selection()));
        b.set_text(&old);
        b.apply(text, EditOp::Home { select: false });
        for _ in 0..old[..sel.end.min(old.len())].chars().count() {
            b.apply(
                text,
                EditOp::Right {
                    select: false,
                    word: false,
                },
            );
        }
        st.typing = false;
        st.refresh(text);
        Some(self.write(old))
    }

    /// Un point de la fenêtre vers le texte (coordonnées de l'éditeur).
    fn text_point(st: &TextFieldState, p: Point) -> Point {
        Point::new(p.x - st.area.x() + st.scroll, (p.y - st.area.y()).max(0.0))
    }
}

fn result(handled: bool, action: Option<Action>) -> EventResult {
    EventResult {
        handled,
        repaint: true,
        action,
    }
}

impl Primitive for TextFieldPrim {
    fn name(&self) -> &'static str {
        if self.secure { "SecureField" } else { "TextField" }
    }

    fn create_state(&self) -> Box<dyn std::any::Any> {
        Box::<TextFieldState>::default()
    }

    fn focusable(&self) -> bool {
        true
    }

    fn accepts_text(&self) -> bool {
        true
    }

    fn hit_shape(&self) -> Option<Shape> {
        Some(Shape::Rectangle)
    }

    fn size_that_fits(&self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        cx.state_and_text(|_: &mut TextFieldState, text, env| {
            let insets = Self::insets(env);
            let style = self.style(env);
            let line = text.layout("Ag", &style, None).size.height;
            Size::new(p.width.unwrap_or(IDEAL_WIDTH), line + insets.vertical())
        })
    }

    fn place(&self, cx: &mut LayoutCx<'_>, rect: Rect) {
        let value = (self.get)();
        cx.state_and_text(|st: &mut TextFieldState, text, env| {
            let insets = Self::insets(env);
            let style = self.style(env);
            st.area = rect.inset(insets);
            // L'état a pu changer ailleurs : l'éditeur le suit.
            st.buffer(&style, self.secure).set_text(&value);
            let mut ph = style.clone();
            ph.color = env.color(&Token::Placeholder.into());
            st.placeholder = Some(Rc::new(text.layout(&self.placeholder, &ph, None)));
            st.refresh(text);
        });
    }

    fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        let env = cx.env().clone();
        let focused = cx.is_focused();
        let plain = env.text_field_plain;
        if !plain {
            let shape = Shape::RoundedRectangle(CornerRadii::all(6.0));
            cx.list()
                .fill(shape, rect, env.color(&Token::TextFieldBackground.into()));
            cx.list()
                .stroke(shape, rect, env.color(&Token::Separator.into()), 1.0);
        }
        let st = cx.state::<TextFieldState>();
        let area = st.area;
        let origin = Point::new(area.x() - st.scroll, area.y());
        let view = st.view.clone();
        let placeholder = st.placeholder.clone();
        let empty = view.layout.runs.is_empty() && view.compose.is_none();
        let list = cx.list();
        list.push(DrawCommand::PushClip {
            shape: Shape::Rectangle,
            rect: area,
        });
        if focused {
            let sel = env.color(&Token::Selection.into());
            for r in &view.selection {
                list.fill(
                    Shape::Rectangle,
                    r.translate(veka_foundation::Vector::new(origin.x, origin.y)),
                    sel,
                );
            }
        }
        if empty {
            if let Some(p) = placeholder {
                p.draw(list, area.origin);
            }
        } else {
            view.layout.draw(list, origin);
        }
        if let Some(c) = view.compose {
            let line = Rect::new(c.x(), c.max_y() - 1.5, c.width(), 1.0)
                .translate(veka_foundation::Vector::new(origin.x, origin.y));
            list.fill(Shape::Rectangle, line, env.text_style().color);
        }
        if focused && let Some(c) = view.cursor {
            let accent = env.color(&Token::TextFieldAccent.into());
            list.fill(
                Shape::Rectangle,
                c.translate(veka_foundation::Vector::new(origin.x, origin.y)),
                accent,
            );
        }
        list.push(DrawCommand::PopClip);
        if focused && !plain {
            crate::views::paint_focus_ring(cx, rect);
        }
    }

    fn pointer(&self, cx: &mut EventCx<'_>, event: PointerEvent) -> EventResult {
        match event {
            PointerEvent::Down { position } => {
                let p = Self::text_point(cx.state::<TextFieldState>(), position);
                let a = self.edit(cx, &[EditOp::MoveTo(p)], false);
                cx.state::<TextFieldState>().dragging = true;
                result(true, a)
            }
            PointerEvent::Move { position, .. } if cx.state::<TextFieldState>().dragging => {
                let p = Self::text_point(cx.state::<TextFieldState>(), position);
                let a = self.edit(cx, &[EditOp::ExtendTo(p)], false);
                result(true, a)
            }
            PointerEvent::Up { .. } => {
                cx.state::<TextFieldState>().dragging = false;
                result(true, None)
            }
            _ => EventResult::default(),
        }
    }

    fn key(&self, cx: &mut EventCx<'_>, event: &KeyEvent) -> EventResult {
        if !cx.is_focused() {
            return EventResult::default();
        }
        let m = event.modifiers;
        let shift = m.contains(Modifiers::SHIFT);
        let command = m.contains(Modifiers::COMMAND);
        // Aller de mot en mot : ⌥ sur le Mac, Ctrl ailleurs.
        let word_mod = if cfg!(target_os = "macos") {
            Modifiers::ALT
        } else {
            Modifiers::CONTROL
        };
        let word = m.contains(word_mod);
        // ⌘← et ⌘→ vont au début et à la fin de la ligne sur le Mac.
        let line = cfg!(target_os = "macos") && m.contains(Modifiers::META);
        let ops: &[EditOp<'_>] = match event.key {
            Key::ArrowLeft if line => &[EditOp::Home { select: shift }],
            Key::ArrowRight if line => &[EditOp::End { select: shift }],
            Key::ArrowLeft => &[EditOp::Left { select: shift, word }],
            Key::ArrowRight => &[EditOp::Right { select: shift, word }],
            Key::Home | Key::ArrowUp => &[EditOp::Home { select: shift }],
            Key::End | Key::ArrowDown => &[EditOp::End { select: shift }],
            Key::Backspace if line => &[EditOp::Home { select: true }, EditOp::Backspace],
            Key::Backspace if word => &[EditOp::BackspaceWord],
            Key::Backspace => &[EditOp::Backspace],
            Key::Delete if word => &[EditOp::DeleteWord],
            Key::Delete => &[EditOp::Delete],
            Key::Enter => {
                let submit = cx.env().on_submit.clone();
                return EventResult {
                    handled: submit.is_some(),
                    repaint: false,
                    action: submit,
                };
            }
            Key::Character(c) if command => {
                return match c {
                    'a' => result(true, self.edit(cx, &[EditOp::SelectAll], false)),
                    'c' | 'x' => {
                        let style = self.style(cx.env());
                        let secure = self.secure;
                        let selected = cx
                            .state::<TextFieldState>()
                            .buffer(&style, secure)
                            .selected_text();
                        let Some(s) = selected else {
                            return result(true, None);
                        };
                        crate::set_clipboard_text(&s);
                        let a = if c == 'x' {
                            self.edit(cx, &[EditOp::Backspace], false)
                        } else {
                            None
                        };
                        result(true, a)
                    }
                    'v' => {
                        let pasted = crate::clipboard_text().unwrap_or_default();
                        // Une seule ligne : les retours à la ligne collés deviennent des espaces.
                        let one_line = pasted.replace(['\r', '\n'], " ");
                        result(true, self.edit(cx, &[EditOp::Insert(&one_line)], false))
                    }
                    'z' => result(true, self.undo(cx, shift)),
                    'y' if !cfg!(target_os = "macos") => result(true, self.undo(cx, true)),
                    // Les autres raccourcis (⌘S…) restent aux boutons.
                    _ => EventResult::default(),
                };
            }
            // Les caractères arrivent par `text_input` : la touche est prise ici pour ne pas
            // déclencher un raccourci sans modificateur pendant la frappe.
            Key::Character(_) if !m.contains(Modifiers::CONTROL) && !m.contains(Modifiers::META) => {
                return result(true, None);
            }
            _ => return EventResult::default(),
        };
        let a = self.edit(cx, ops, false);
        result(true, a)
    }

    fn text_input(&self, cx: &mut EventCx<'_>, input: &TextInput) -> EventResult {
        let a = match input {
            TextInput::Char(c) => {
                let mut buf = [0u8; 4];
                self.edit(cx, &[EditOp::Insert(c.encode_utf8(&mut buf))], true)
            }
            TextInput::Compose { text, cursor } if text.is_empty() => {
                let _ = cursor;
                self.edit(cx, &[EditOp::ClearCompose], false)
            }
            TextInput::Compose { text, cursor } => self.edit(
                cx,
                &[EditOp::Compose {
                    text,
                    cursor: *cursor,
                }],
                false,
            ),
            TextInput::Commit(text) => self.edit(cx, &[EditOp::ClearCompose, EditOp::Insert(text)], false),
        };
        result(true, a)
    }
}

impl TextFieldPrim {
    /// La zone à donner à la fenêtre de l'IME, en coordonnées de la fenêtre.
    pub fn ime_area(st: &TextFieldState) -> Rect {
        let origin = Point::new(st.area.x() - st.scroll, st.area.y());
        let a = st
            .view
            .ime_area
            .translate(veka_foundation::Vector::new(origin.x, origin.y));
        if a.width() <= 0.0 && a.height() <= 0.0 {
            Rect::new(st.area.x(), st.area.y(), 1.0, st.area.height())
        } else {
            a
        }
    }
}

/// Un champ de texte d'une ligne, relié à un état : `TextField("Nom", nom)`, comme
/// `TextField("Nom", text: $nom)` de SwiftUI.
pub fn TextField(placeholder: impl Into<String>, text: State<String>) -> Element {
    field(placeholder.into(), text, false)
}

/// Un champ dont le texte est caché par des points, comme `SecureField` de SwiftUI. Son texte ne se copie pas.
pub fn SecureField(placeholder: impl Into<String>, text: State<String>) -> Element {
    field(placeholder.into(), text, true)
}

fn field(placeholder: String, text: State<String>, secure: bool) -> Element {
    Element::new(
        TextFieldPrim {
            placeholder,
            get: Rc::new(move || text.try_with(String::clone).unwrap_or_default()),
            set: Rc::new(move |v| {
                if text.is_alive() && text.try_with(|t| *t != v).unwrap_or(false) {
                    text.set(v);
                }
            }),
            secure,
        },
        Vec::new(),
    )
}

/// Le primitive de `.on_submit` et `.text_field_style` : une valeur d'environnement pour les champs.
#[derive(Clone, Debug, PartialEq)]
pub enum FieldEnv {
    OnSubmit(Action),
    Plain(bool),
}

impl Primitive for FieldEnv {
    fn name(&self) -> &'static str {
        match self {
            FieldEnv::OnSubmit(_) => "onSubmit",
            FieldEnv::Plain(_) => "textFieldStyle",
        }
    }

    fn update_env(&self, env: &mut crate::env::Environment) -> bool {
        match self {
            FieldEnv::OnSubmit(a) => env.on_submit = Some(a.clone()),
            FieldEnv::Plain(p) => env.text_field_plain = *p,
        }
        true
    }
}

impl Element {
    /// Action à l'appui sur Entrée dans un champ de texte du sous-arbre, comme `.onSubmit` de SwiftUI.
    pub fn on_submit(self, action: impl FnMut() + 'static) -> Element {
        self.wrap(FieldEnv::OnSubmit(Action::new(action)))
    }

    /// Les champs de texte du sous-arbre sans cadre (`true`, `.textFieldStyle(.plain)`), ou avec le cadre
    /// arrondi (`false`, `.roundedBorder`, par défaut).
    pub fn text_field_style_plain(self, plain: bool) -> Element {
        self.wrap(FieldEnv::Plain(plain))
    }
}
