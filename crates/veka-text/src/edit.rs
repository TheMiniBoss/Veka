//! Un texte qu'on édite (étape 9) : curseur, sélection, composition IME, avec le `PlainEditor` de Parley.
//! Le champ de texte de l'interface (`TextField`) n'en voit que [`EditBuffer`] et [`EditOp`].

use std::borrow::Cow;
use std::ops::Range;

use parley::{FontFamily, FontFamilyName, FontWeight, PlainEditor, StyleProperty};
use veka_foundation::{Color, Point, Rect, Size};

use crate::{TextLayout, TextStyle, TextSystem, convert};

/// Le caractère qui cache le texte d'un champ secret.
const BULLET: char = '•';

/// Une modification ou un déplacement dans un [`EditBuffer`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EditOp<'a> {
    /// Écrit à la place de la sélection.
    Insert(&'a str),
    /// Efface le caractère avant le curseur (ou la sélection).
    Backspace,
    /// Efface le caractère après le curseur (ou la sélection).
    Delete,
    /// Efface le mot avant le curseur.
    BackspaceWord,
    /// Efface le mot après le curseur.
    DeleteWord,
    /// Un caractère (ou un mot) à gauche ; `select` étend la sélection.
    Left {
        select: bool,
        word: bool,
    },
    Right {
        select: bool,
        word: bool,
    },
    /// Début et fin de la ligne.
    Home {
        select: bool,
    },
    End {
        select: bool,
    },
    SelectAll,
    /// Place le curseur au point (coordonnées du texte, depuis son coin haut-gauche).
    MoveTo(Point),
    /// Étend la sélection jusqu'au point (glisser, Maj+clic).
    ExtendTo(Point),
    /// Sélectionne le mot sous le point (double-clic).
    SelectWordAt(Point),
    /// Texte en cours de composition (IME), avec la place du curseur dedans (octets).
    Compose {
        text: &'a str,
        cursor: Option<(usize, usize)>,
    },
    /// La composition est validée telle quelle.
    FinishCompose,
    /// La composition est abandonnée.
    ClearCompose,
}

/// Ce qu'il faut pour dessiner un [`EditBuffer`], en coordonnées du texte.
#[derive(Clone, Debug, Default)]
pub struct EditView {
    pub layout: TextLayout,
    /// Le curseur (barre fine), s'il est visible.
    pub cursor: Option<Rect>,
    /// Les rectangles de la sélection.
    pub selection: Vec<Rect>,
    /// Le texte en composition, à souligner.
    pub compose: Option<Rect>,
    /// La zone à donner à la fenêtre de l'IME.
    pub ime_area: Rect,
}

/// Un texte qu'on édite.
pub struct EditBuffer {
    editor: PlainEditor<Color>,
    style: TextStyle,
    /// Le style a changé depuis la dernière mise en page.
    restyle: bool,
    /// Champ secret : chaque caractère est dessiné comme un point.
    secure: bool,
}

impl std::fmt::Debug for EditBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditBuffer")
            .field("text", &self.text())
            .field("selection", &self.selection())
            .finish()
    }
}

impl EditBuffer {
    pub fn new(style: TextStyle, secure: bool) -> Self {
        let mut editor = PlainEditor::new(style.size);
        // Une seule ligne : pas de retour à la ligne automatique.
        editor.set_width(None);
        Self {
            editor,
            style,
            restyle: true,
            secure,
        }
    }

    pub fn set_style(&mut self, style: &TextStyle) {
        if *style != self.style {
            self.style = style.clone();
            self.restyle = true;
        }
    }

    pub fn is_secure(&self) -> bool {
        self.secure
    }

    /// Le texte (sans la composition en cours).
    pub fn text(&self) -> String {
        self.editor.text().chars().collect()
    }

    /// Remplace le texte (s'il est différent) ; la sélection est gardée, ramenée dans le texte.
    pub fn set_text(&mut self, text: &str) {
        if self.editor.raw_text() != text && !self.editor.is_composing() {
            self.editor.set_text(text);
        }
    }

    /// La sélection, en octets du texte (vide : le curseur).
    pub fn selection(&self) -> Range<usize> {
        self.editor.raw_selection().text_range()
    }

    /// Le texte sélectionné (jamais pour un champ secret).
    pub fn selected_text(&self) -> Option<String> {
        if self.secure {
            return None;
        }
        self.editor.selected_text().map(str::to_owned)
    }

    pub fn is_composing(&self) -> bool {
        self.editor.is_composing()
    }

    fn apply_style(&mut self, ts: &TextSystem) {
        if !self.restyle {
            return;
        }
        self.restyle = false;
        let family = ts.family_of(&self.style).to_string();
        let styles = self.editor.edit_styles();
        styles.insert(StyleProperty::FontFamily(FontFamily::Single(
            FontFamilyName::Named(Cow::Owned(family)),
        )));
        styles.insert(StyleProperty::FontSize(self.style.size));
        styles.insert(StyleProperty::FontWeight(FontWeight::new(self.style.weight.0)));
        styles.insert(StyleProperty::Brush(self.style.color));
    }

    /// Un point du texte affiché vers un point du vrai texte (le même, sauf pour un champ secret, où
    /// la position est retrouvée par le nombre de points).
    fn real_point(&mut self, ts: &mut TextSystem, p: Point) -> Option<usize> {
        if !self.secure {
            return None;
        }
        let bullet = ts
            .layout(&BULLET.to_string(), &self.style, None)
            .size
            .width
            .max(1.0);
        let n = (p.x / bullet).round().max(0.0) as usize;
        let text = self.text();
        Some(text.char_indices().nth(n).map_or(text.len(), |(i, _)| i))
    }

    /// Applique une modification ou un déplacement.
    pub fn apply(&mut self, ts: &mut TextSystem, op: EditOp<'_>) {
        self.apply_style(ts);
        // Champ secret : les points visés à la souris sont retrouvés dans le vrai texte.
        let secure_point = match op {
            EditOp::MoveTo(p) | EditOp::ExtendTo(p) | EditOp::SelectWordAt(p) => self.real_point(ts, p),
            _ => None,
        };
        let (fonts, layout) = ts.contexts();
        let mut d = self.editor.driver(fonts, layout);
        match op {
            EditOp::Insert(s) => d.insert_or_replace_selection(s),
            EditOp::Backspace => d.backdelete(),
            EditOp::Delete => d.delete(),
            EditOp::BackspaceWord => d.backdelete_word(),
            EditOp::DeleteWord => d.delete_word(),
            EditOp::Left { select, word } => match (select, word) {
                (false, false) => d.move_left(),
                (false, true) => d.move_word_left(),
                (true, false) => d.select_left(),
                (true, true) => d.select_word_left(),
            },
            EditOp::Right { select, word } => match (select, word) {
                (false, false) => d.move_right(),
                (false, true) => d.move_word_right(),
                (true, false) => d.select_right(),
                (true, true) => d.select_word_right(),
            },
            EditOp::Home { select: false } => d.move_to_text_start(),
            EditOp::Home { select: true } => d.select_to_text_start(),
            EditOp::End { select: false } => d.move_to_text_end(),
            EditOp::End { select: true } => d.select_to_text_end(),
            EditOp::SelectAll => d.select_all(),
            EditOp::MoveTo(p) => match secure_point {
                Some(i) => d.move_to_byte(i),
                None => d.move_to_point(p.x, p.y),
            },
            EditOp::ExtendTo(p) => match secure_point {
                Some(i) => d.extend_selection_to_byte(i),
                None => d.extend_selection_to_point(p.x, p.y),
            },
            // Pas de mots dans un champ secret : tout est sélectionné.
            EditOp::SelectWordAt(_) if self.secure => d.select_all(),
            EditOp::SelectWordAt(p) => d.select_word_at_point(p.x, p.y),
            EditOp::Compose { text, cursor } => d.set_compose(text, cursor),
            EditOp::FinishCompose => d.finish_compose(),
            EditOp::ClearCompose => d.clear_compose(),
        }
    }

    /// La mise en page, le curseur et la sélection, en coordonnées du texte.
    pub fn view(&mut self, ts: &mut TextSystem) -> EditView {
        self.apply_style(ts);
        let (fonts, layout_cx) = ts.contexts();
        let layout = self.editor.layout(fonts, layout_cx).clone();
        let height = layout.height().max(self.style.size * 1.2);
        if self.secure {
            return self.secure_view(ts, height);
        }
        let rect = |b: parley::BoundingBox| {
            Rect::new(
                b.x0 as f32,
                b.y0 as f32,
                (b.x1 - b.x0) as f32,
                (b.y1 - b.y0) as f32,
            )
        };
        let cursor = if self.editor.is_cursor_visible() {
            self.editor.cursor_geometry(1.5).map(rect)
        } else {
            None
        };
        let selection = self
            .editor
            .selection_geometry()
            .into_iter()
            .map(|(b, _)| rect(b))
            .collect();
        let compose = self.editor.raw_compose().clone().map(|r| {
            let start = parley::Cursor::from_byte_index(&layout, r.start, parley::Affinity::Downstream);
            let end = parley::Cursor::from_byte_index(&layout, r.end, parley::Affinity::Upstream);
            let g = parley::Selection::new(start, end).geometry(&layout);
            g.into_iter()
                .map(|(b, _)| rect(b))
                .reduce(|a, b| a.union(&b))
                .unwrap_or_default()
        });
        let ime_area = rect(self.editor.ime_cursor_area());
        let mut layout = convert(&layout);
        layout.size.height = height;
        EditView {
            layout,
            cursor,
            selection,
            compose,
            ime_area,
        }
    }

    /// Un champ secret : des points, un par caractère ; curseur et sélection retrouvés par leur rang.
    fn secure_view(&mut self, ts: &mut TextSystem, height: f32) -> EditView {
        let text = self.text();
        let count = text.chars().count();
        let shown: String = std::iter::repeat_n(BULLET, count).collect();
        let mut layout = ts.layout(&shown, &self.style, None);
        let bullet = ts.layout(&BULLET.to_string(), &self.style, None).size.width;
        let rank = |byte: usize| text[..byte.min(text.len())].chars().count() as f32;
        let sel = self.selection();
        let x0 = rank(sel.start) * bullet;
        let x1 = rank(sel.end) * bullet;
        let focus = self.editor.raw_selection().focus().index();
        let cursor = Rect::new(rank(focus) * bullet, 0.0, 1.5, height);
        layout.size = Size::new(layout.size.width, height);
        EditView {
            layout,
            cursor: self.editor.is_cursor_visible().then_some(cursor),
            selection: if sel.is_empty() {
                Vec::new()
            } else {
                vec![Rect::new(x0, 0.0, x1 - x0, height)]
            },
            compose: None,
            ime_area: cursor,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer(secure: bool) -> (TextSystem, EditBuffer) {
        (TextSystem::new(), EditBuffer::new(TextStyle::default(), secure))
    }

    #[test]
    fn ecrire_effacer_deplacer() {
        let (mut ts, mut b) = buffer(false);
        b.apply(&mut ts, EditOp::Insert("bonjour monde"));
        assert_eq!(b.text(), "bonjour monde");
        b.apply(&mut ts, EditOp::Backspace);
        assert_eq!(b.text(), "bonjour mond");
        b.apply(
            &mut ts,
            EditOp::Left {
                select: false,
                word: true,
            },
        );
        b.apply(&mut ts, EditOp::Insert("le "));
        assert_eq!(b.text(), "bonjour le mond");
        b.apply(&mut ts, EditOp::Home { select: false });
        b.apply(
            &mut ts,
            EditOp::Right {
                select: true,
                word: true,
            },
        );
        assert_eq!(b.selected_text().as_deref(), Some("bonjour"));
        b.apply(&mut ts, EditOp::Insert("salut"));
        assert_eq!(b.text(), "salut le mond");
        b.apply(&mut ts, EditOp::SelectAll);
        b.apply(&mut ts, EditOp::Delete);
        assert_eq!(b.text(), "");
    }

    #[test]
    fn composition() {
        let (mut ts, mut b) = buffer(false);
        b.apply(&mut ts, EditOp::Insert("a"));
        b.apply(
            &mut ts,
            EditOp::Compose {
                text: "に",
                cursor: None,
            },
        );
        assert!(b.is_composing());
        // La composition n'est pas encore dans le texte.
        assert_eq!(b.text(), "a");
        assert!(b.view(&mut ts).compose.is_some());
        b.apply(&mut ts, EditOp::FinishCompose);
        assert_eq!(b.text(), "aに");
    }

    #[test]
    fn la_vue_suit_le_curseur() {
        let (mut ts, mut b) = buffer(false);
        b.apply(&mut ts, EditOp::Insert("abc"));
        let v = b.view(&mut ts);
        let fin = v.cursor.expect("curseur").x();
        b.apply(&mut ts, EditOp::Home { select: false });
        let debut = b.view(&mut ts).cursor.expect("curseur").x();
        assert!(fin > debut + 10.0, "{debut} {fin}");
        assert!(!v.layout.runs.is_empty());
    }

    #[test]
    fn champ_secret() {
        let (mut ts, mut b) = buffer(true);
        b.apply(&mut ts, EditOp::Insert("mot2passe"));
        assert_eq!(b.text(), "mot2passe");
        assert_eq!(b.selected_text(), None);
        let v = b.view(&mut ts);
        assert!(v.cursor.expect("curseur").x() > 0.0);
        // Un clic au début met le curseur au début du vrai texte.
        b.apply(&mut ts, EditOp::MoveTo(Point::new(0.0, 5.0)));
        assert_eq!(b.selection(), 0..0);
    }
}
