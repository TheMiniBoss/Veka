use std::rc::Rc;

use veka_foundation::{Rect, Size};
use veka_text::{TextLayout, TextStyle};

use crate::primitive::{LayoutCx, PaintCx, Primitive, Proposal};

/// Le primitive `Text`.
#[derive(Clone, Debug, PartialEq)]
pub struct TextPrim {
    pub content: String,
}

/// Cache de mise en page d'un `Text` : quelques largeurs récentes + celle retenue au placement.
#[derive(Default)]
pub struct TextState {
    cache: Vec<(String, TextStyle, Option<u32>, Rc<TextLayout>)>,
    placed: Option<Rc<TextLayout>>,
}

impl TextState {
    fn layout(
        &mut self,
        text: &mut veka_text::TextSystem,
        content: &str,
        style: &TextStyle,
        width: Option<f32>,
    ) -> Rc<TextLayout> {
        // Largeurs arrondies au 1/64 de point pour retrouver les mises en page déjà faites.
        let w = width
            .filter(|w| w.is_finite())
            .map(|w| (w.max(0.0) * 64.0) as u32);
        if let Some((.., l)) = self
            .cache
            .iter()
            .find(|(c, s, cw, _)| c == content && s == style && *cw == w)
        {
            return l.clone();
        }
        let l = Rc::new(text.layout(content, style, w.map(|w| w as f32 / 64.0)));
        if self.cache.len() >= 4 {
            self.cache.remove(0);
        }
        self.cache.push((content.to_owned(), style.clone(), w, l.clone()));
        l
    }
}

/// Écart de largeur (en points) en dessous duquel un texte reste sur une ligne.
const FIT_TOLERANCE: f32 = 0.01;

impl Primitive for TextPrim {
    fn name(&self) -> &'static str {
        "Text"
    }

    fn create_state(&self) -> Box<dyn std::any::Any> {
        Box::<TextState>::default()
    }

    fn size_that_fits(&self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        cx.state_and_text(|st: &mut TextState, text, env| {
            let style = env.text_style();
            let natural = st.layout(text, &self.content, &style, None);
            match p.width {
                // Assez de place (ou taille idéale) : une seule ligne.
                None => natural.size,
                // Tolérance : la largeur rendue par le parent peut différer d'un arrondi.
                Some(w) if w + FIT_TOLERANCE >= natural.size.width => natural.size,
                Some(w) => st.layout(text, &self.content, &style, Some(w)).size,
            }
        })
    }

    fn place(&self, cx: &mut LayoutCx<'_>, rect: Rect) {
        cx.state_and_text(|st: &mut TextState, text, env| {
            let style = env.text_style();
            let natural = st.layout(text, &self.content, &style, None);
            let l = if rect.width() + FIT_TOLERANCE >= natural.size.width {
                natural
            } else {
                st.layout(text, &self.content, &style, Some(rect.width()))
            };
            st.placed = Some(l);
        });
    }

    fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        if let Some(l) = cx.state::<TextState>().placed.clone() {
            l.draw(cx.list(), rect.origin);
        }
    }
}
